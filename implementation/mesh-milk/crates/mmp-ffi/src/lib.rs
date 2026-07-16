//! mmp-ffi — UniFFI bindings: Kotlin/Swift (and, through them, React Native)
//! API surface over [`mmp_node::Node`] (MOBILE_INTEGRATION.md §3, doc 12 §9).
//!
//! # The one rule this crate exists to enforce
//!
//! The mobile platform owns the BLE radio and the clock; this crate owns
//! nothing but a thin, mutex-guarded shim around the sans-IO protocol core.
//! There are no threads, no tokio, no timers here — every side effect is
//! driven by an explicit host call: inject a [`TalEvent`] in, poll wire bytes
//! or an [`Event`](mmp_node::Event) out, or advance time via [`MmpNode::tick`].
//!
//! # Secret boundary (doc 12 §9)
//!
//! The 32-byte identity seed crosses the FFI exactly once, inward, at
//! [`MmpNode::new`]. Nothing secret ever crosses back out: every method here
//! returns public keys, NodeIDs, ciphertext-free app events, or wire bytes
//! that are already encrypted by the core before they reach [`MmpNode::poll_send`].

use std::sync::{Arc, Mutex, MutexGuard};

use mmp_core::MemStore;
use mmp_node::{DeliveryState, Event, Node, SendError, Tier};
use mmp_store::SnapshotStore;
use mmp_tal::{LinkQuality, TalEvent};
use mmp_types::authcert::AuthCert;
use mmp_types::claim::Claim;
use mmp_types::ids::{Id16, Key32};

use store::SharedStore;

mod store;

uniffi::setup_scaffolding!();

/// How often [`MmpNode::tick`] writes the store to disk when a
/// `store_path` was supplied at construction (doc 04 §7 storage policy).
/// Deliberately coarse: [`MmpNode::flush`] exists for callers (e.g. a host
/// `onPause`/`applicationWillResignActive` hook) that need a persisted state
/// sooner than this interval.
const PERSIST_INTERVAL_MS: u64 = 30_000;

// --------------------------------------------------------------- errors ----

/// FFI-facing error surface. Deliberately coarse: the host app cannot act
/// differently on the finer-grained [`SendError`] variants, and nothing about
/// *why* a call failed is secret, so a small enum keeps the generated
/// Kotlin/Swift bindings simple.
#[derive(uniffi::Error, Debug, Clone, Copy, PartialEq, Eq)]
pub enum FfiError {
    /// A byte slice had the wrong length (NodeId/Id16 must be exactly 16
    /// bytes, a seed exactly 32) or a claim/body failed to decode.
    BadArg,
    /// We don't hold the peer's exchange key / prekey batch / group key yet.
    UnknownPeer,
    /// An invariant inside the core was violated; should not happen in
    /// practice and indicates a bug rather than a normal protocol outcome.
    Internal,
}

impl std::fmt::Display for FfiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FfiError::BadArg => write!(f, "bad argument: wrong length or malformed encoding"),
            FfiError::UnknownPeer => write!(f, "unknown peer: no exchange key / prekey batch on file"),
            FfiError::Internal => write!(f, "internal error"),
        }
    }
}

impl std::error::Error for FfiError {}

fn map_send_error(e: SendError) -> FfiError {
    match e {
        SendError::UnknownPeer => FfiError::UnknownPeer,
        // "Too large" is caller-controlled input, not a protocol-unknown
        // condition — surface it as a bad argument.
        SendError::TooLarge => FfiError::BadArg,
    }
}

/// Parse a 16-byte NodeID / BundleID / GroupID out of an FFI byte vector.
fn id16_from(bytes: &[u8]) -> Result<Id16, FfiError> {
    let arr: [u8; 16] = bytes.try_into().map_err(|_| FfiError::BadArg)?;
    Ok(Id16(arr))
}

/// Parse a 32-byte Ed25519 public key (AuthCert subject/issuer, doc 18 §2)
/// out of an FFI byte vector.
fn key32_from(bytes: &[u8]) -> Result<Key32, FfiError> {
    let arr: [u8; 32] = bytes.try_into().map_err(|_| FfiError::BadArg)?;
    Ok(Key32(arr))
}

// ---------------------------------------------------------------- types ----

/// Runtime relay tier (mirrors [`Tier`]).
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum FfiTier {
    Leaf,
    Relay,
    Anchor,
}

impl From<FfiTier> for Tier {
    fn from(t: FfiTier) -> Self {
        match t {
            FfiTier::Leaf => Tier::Leaf,
            FfiTier::Relay => Tier::Relay,
            FfiTier::Anchor => Tier::Anchor,
        }
    }
}

impl From<Tier> for FfiTier {
    fn from(t: Tier) -> Self {
        match t {
            Tier::Leaf => FfiTier::Leaf,
            Tier::Relay => FfiTier::Relay,
            Tier::Anchor => FfiTier::Anchor,
        }
    }
}

/// Delivery status (mirrors [`DeliveryState`]).
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum FfiDeliveryState {
    Queued,
    InMesh,
    Delivered,
    Expired,
}

impl From<DeliveryState> for FfiDeliveryState {
    fn from(s: DeliveryState) -> Self {
        match s {
            DeliveryState::Queued => FfiDeliveryState::Queued,
            DeliveryState::InMesh => FfiDeliveryState::InMesh,
            DeliveryState::Delivered => FfiDeliveryState::Delivered,
            DeliveryState::Expired => FfiDeliveryState::Expired,
        }
    }
}

/// One outbound wire frame: write `bytes` to `link` (fragment to MTU on the
/// host side; the core already ordered them — write in poll order).
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct OutFrame {
    pub link: u64,
    pub bytes: Vec<u8>,
}

/// The event stream — a faithful, ciphertext-free mirror of [`Event`].
/// Every screen in the host app is a projection of this enum
/// (MOBILE_INTEGRATION.md §2).
#[derive(uniffi::Enum, Debug, Clone, PartialEq, Eq)]
pub enum FfiEvent {
    MessageReceived { conv: Vec<u8>, from: Vec<u8>, id: Vec<u8>, body: Vec<u8>, apptype: u8 },
    MessageGap { conv: Vec<u8>, parents: Vec<Vec<u8>> },
    Delivered { id: Vec<u8> },
    DeliveryProgress { id: Vec<u8>, state: FfiDeliveryState },
    ClaimUpdated { owner: Vec<u8>, field: u8 },
    CompromiseWarning { owner: Vec<u8> },
    ContactSeen { peer: Vec<u8> },
    Established { peer: Vec<u8> },
    GroupInvite { gid: Vec<u8>, from: Vec<u8> },
    GroupMessage { gid: Vec<u8>, from: Vec<u8>, id: Vec<u8>, body: Vec<u8> },
    GroupEpochChanged { gid: Vec<u8>, epoch: u16 },
    TierChanged { tier: FfiTier },
    /// Doc 18: a class-5 announcement for a scope we subscribed to. Apps
    /// should key notification prominence off `effective_priority`, not
    /// `official` alone (see `mmp_node::Event::Announcement`'s doc comment).
    Announcement {
        id: Vec<u8>,
        scope: Vec<u8>,
        ann_type: u8,
        not_after: u32,
        text: Vec<u8>,
        from: Vec<u8>,
        official: bool,
        effective_priority: u8,
        scope_name_hint: Option<Vec<u8>>,
    },
}

impl From<Event> for FfiEvent {
    fn from(e: Event) -> Self {
        match e {
            Event::MessageReceived { conv, from, id, body, apptype } => FfiEvent::MessageReceived {
                conv: conv.0.to_vec(),
                from: from.0.to_vec(),
                id: id.0.to_vec(),
                body,
                apptype,
            },
            Event::MessageGap { conv, parents } => FfiEvent::MessageGap {
                conv: conv.0.to_vec(),
                parents: parents.into_iter().map(|p| p.0.to_vec()).collect(),
            },
            Event::Delivered { id } => FfiEvent::Delivered { id: id.0.to_vec() },
            Event::DeliveryProgress { id, state } => {
                FfiEvent::DeliveryProgress { id: id.0.to_vec(), state: state.into() }
            }
            Event::ClaimUpdated { owner, field } => {
                FfiEvent::ClaimUpdated { owner: owner.0.to_vec(), field }
            }
            Event::CompromiseWarning { owner } => {
                FfiEvent::CompromiseWarning { owner: owner.0.to_vec() }
            }
            Event::ContactSeen { peer } => FfiEvent::ContactSeen { peer: peer.0.to_vec() },
            Event::Established { peer } => FfiEvent::Established { peer: peer.0.to_vec() },
            Event::GroupInvite { gid, from } => {
                FfiEvent::GroupInvite { gid: gid.0.to_vec(), from: from.0.to_vec() }
            }
            Event::GroupMessage { gid, from, id, body } => FfiEvent::GroupMessage {
                gid: gid.0.to_vec(),
                from: from.0.to_vec(),
                id: id.0.to_vec(),
                body,
            },
            Event::GroupEpochChanged { gid, epoch } => {
                FfiEvent::GroupEpochChanged { gid: gid.0.to_vec(), epoch }
            }
            Event::TierChanged { tier } => FfiEvent::TierChanged { tier: tier.into() },
            Event::Announcement {
                id,
                scope,
                ann_type,
                not_after,
                text,
                from,
                official,
                effective_priority,
                scope_name_hint,
            } => FfiEvent::Announcement {
                id: id.0.to_vec(),
                scope: scope.0.to_vec(),
                ann_type,
                not_after,
                text,
                from: from.0.to_vec(),
                official,
                effective_priority,
                scope_name_hint,
            },
        }
    }
}

// --------------------------------------------------------------- MmpNode ----

/// The mesh_milk node, exposed to Kotlin/Swift. Internally mutex-guarded so
/// the generated bindings are `Sync` from any language's perspective; there
/// is no threading contract beyond "one caller at a time" per
/// MOBILE_INTEGRATION.md §2.
///
/// # Persistence (doc 04 §7 drop-and-redownload philosophy)
///
/// Constructing with `store_path: None` backs the node with an in-memory
/// [`MemStore`]: everything is lost on process death, same as before this
/// was wired up.
///
/// Constructing with `store_path: Some(path)` backs the node with a
/// [`SnapshotStore`] at that path, periodically flushed by [`Self::tick`]
/// (every [`PERSIST_INTERVAL_MS`]) and on demand by [`Self::flush`]. What
/// that buys you, precisely:
///
/// - **Survives restart:** the bundle store — every queued/in-mesh
///   bundle, including our own outgoing messages and gossiped claims —
///   because that *is* the `BundleStore` contents; the full `ClaimStore`
///   contents (every claim currently held, not just what re-arrives as
///   class-3 gossip bundles — re-verified on load, a tampered claim is
///   dropped); and group membership (which groups we belong to, current
///   epoch, admin flag — handbook §4.3 steps 1–2).
/// - **Does NOT survive restart:** `SeenIndex` (dedup/replay window),
///   `ReceiptFilter`, and all session/ratchet state (conversation keys,
///   X3DH state, group sender-key ratchets — including group members'
///   sender-key distributions). This is intentional, not an oversight:
///   session-security state is designed to rebuild itself from the mesh
///   rather than persist across restarts — the network is the backup. A
///   restarted node re-establishes sessions, and re-derives a fresh local
///   sender key per restored group (remote members' sender keys re-sync via
///   the normal dist-gossip path on next contact) the same way a
///   freshly-invited member would.
#[derive(uniffi::Object)]
pub struct MmpNode {
    inner: Mutex<Node>,
    /// `Some` iff constructed with a `store_path`; a second handle onto the
    /// same [`SnapshotStore`] the node's `BundleStore` box delegates to
    /// (see [`store::SharedStore`]), used only to drive persistence timing.
    snapshot: Option<Arc<Mutex<SnapshotStore>>>,
    persist: Mutex<PersistState>,
}

/// Persistence bookkeeping, guarded by its own small mutex — never taken
/// together with the node or snapshot locks in a way that could invert the
/// node-then-snapshot ordering (see `store.rs` module docs).
#[derive(Default)]
struct PersistState {
    /// `now_ms` at the last successful-or-attempted persist.
    last_persist_ms: u64,
    /// `now_ms` from the most recent [`MmpNode::tick`] call, so
    /// [`MmpNode::flush`] (which takes no `now_ms` of its own) has a clock
    /// value to stamp `last_persist_ms` with.
    last_seen_now_ms: u64,
    /// The `Display` of the most recent `persist()` `io::Error`, if the most
    /// recent attempt failed.
    last_error: Option<String>,
}

impl MmpNode {
    fn lock(&self) -> MutexGuard<'_, Node> {
        self.inner.lock().unwrap_or_else(|poison| poison.into_inner())
    }

    /// Write the snapshot to disk now and record the outcome. Callers must
    /// not hold `self.inner`'s lock when calling this (lock-ordering rule).
    ///
    /// Stages the node's current claims and group blobs into the snapshot
    /// before writing (see the type-level doc comment for exactly what that
    /// covers) — the node lock is taken and released to collect them *before*
    /// the snapshot lock is touched, so the fixed node-then-snapshot order
    /// (`store.rs` module docs) holds even though this now touches both.
    fn persist_now(&self, snap: &Arc<Mutex<SnapshotStore>>, now_ms: u64) -> std::io::Result<()> {
        let (claims, group_blobs) = {
            let node = self.lock();
            (node.all_claims(), node.all_group_blobs())
        };
        let result = {
            let mut s = snap.lock().unwrap_or_else(|poison| poison.into_inner());
            s.set_claims(claims);
            s.set_group_blobs(group_blobs);
            s.persist()
        };
        let mut st = self.persist.lock().unwrap_or_else(|poison| poison.into_inner());
        st.last_persist_ms = now_ms;
        st.last_error = result.as_ref().err().map(std::string::ToString::to_string);
        result
    }
}

#[uniffi::export]
impl MmpNode {
    /// Construct a node from a 32-byte identity seed (loaded by the host from
    /// Android Keystore / iOS Keychain — doc 12 §9). The seed is consumed
    /// here and never returned across the FFI again.
    ///
    /// `store_path` selects the storage backend (see the type-level doc
    /// comment for exactly what is and is not persisted):
    /// - `None`: in-memory [`MemStore`], nothing survives process death.
    /// - `Some(path)`: file-backed [`SnapshotStore`] at `path`, opened (or
    ///   created) immediately so any prior snapshot's bundles are available
    ///   right away.
    #[uniffi::constructor]
    pub fn new(seed: Vec<u8>, tier: FfiTier, store_path: Option<String>) -> Result<Arc<Self>, FfiError> {
        let seed: [u8; 32] = seed.try_into().map_err(|_| FfiError::BadArg)?;
        let (store, snapshot): (Box<dyn mmp_core::store::BundleStore + Send>, Option<Arc<Mutex<SnapshotStore>>>) =
            match store_path {
                None => (Box::new(MemStore::new()), None),
                Some(path) => {
                    let snap = Arc::new(Mutex::new(SnapshotStore::open(path)));
                    (Box::new(SharedStore(snap.clone())), Some(snap))
                }
            };
        let mut node = Node::new(seed, store, tier.into());
        // Restore step (doc comment above): claims + group membership, if any
        // were loaded from a prior snapshot at this path. `ingest_claim`
        // re-verifies each signature (belt-and-braces on top of
        // `SnapshotStore::open`'s own check); `verified: false` because a
        // reloaded claim is not thereby out-of-band-verified — same trust
        // level it had before the restart.
        if let Some(snap) = &snapshot {
            let guard = snap.lock().unwrap_or_else(|poison| poison.into_inner());
            for claim in guard.claims() {
                node.ingest_claim(claim.clone(), false);
            }
            for blob in guard.group_blobs() {
                node.restore_group(blob);
            }
        }
        Ok(Arc::new(MmpNode {
            inner: Mutex::new(node),
            snapshot,
            persist: Mutex::new(PersistState::default()),
        }))
    }

    /// `BLAKE3(identity_pk)[0..16]` — our own NodeID.
    pub fn node_id(&self) -> Vec<u8> {
        self.lock().node_id().0.to_vec()
    }

    /// Send a text message to `dest`; returns the BundleID to track via
    /// [`MmpNode::message_state`] / [`FfiEvent::Delivered`].
    pub fn send_text(&self, dest: Vec<u8>, text: String) -> Result<Vec<u8>, FfiError> {
        let dest = id16_from(&dest)?;
        self.lock().send_text(&dest, &text).map(|id| id.0.to_vec()).map_err(map_send_error)
    }

    /// Current delivery state of a message we sent, if we're still tracking it.
    pub fn message_state(&self, id: Vec<u8>) -> Result<Option<FfiDeliveryState>, FfiError> {
        let id = id16_from(&id)?;
        Ok(self.lock().message_state(&id).map(FfiDeliveryState::from))
    }

    /// Create a group; we are its (immutable-set v1) admin. Returns the GroupID.
    pub fn create_group(&self) -> Vec<u8> {
        self.lock().create_group().0.to_vec()
    }

    /// Invite `peer` into group `gid`.
    pub fn group_invite(&self, gid: Vec<u8>, peer: Vec<u8>) -> Result<(), FfiError> {
        let gid = id16_from(&gid)?;
        let peer = id16_from(&peer)?;
        self.lock().group_invite(&gid, &peer).map_err(map_send_error)
    }

    /// Send text into group `gid` under our sender key. Returns the BundleID.
    pub fn group_send(&self, gid: Vec<u8>, text: String) -> Result<Vec<u8>, FfiError> {
        let gid = id16_from(&gid)?;
        self.lock().group_send(&gid, &text).map(|id| id.0.to_vec()).map_err(map_send_error)
    }

    /// Current known members of group `gid`.
    pub fn group_members(&self, gid: Vec<u8>) -> Result<Vec<Vec<u8>>, FfiError> {
        let gid = id16_from(&gid)?;
        Ok(self.lock().group_members(&gid).into_iter().map(|m| m.0.to_vec()).collect())
    }

    /// Publish a signed nickname claim (doc 06 §4), gossiped to the mesh.
    pub fn set_nickname(&self, name: String, seq: u32) {
        self.lock().set_nickname(&name, seq);
    }

    /// Our own signed claims (identity/exchange keys, prekey batch, nickname,
    /// …), encoded for out-of-band exchange (QR ceremony, doc 07 §6.1).
    pub fn own_claims(&self) -> Vec<Vec<u8>> {
        self.lock().own_claims().iter().map(Claim::encode).collect()
    }

    /// Ingest a claim received out-of-band (QR ceremony) or over the mesh.
    /// `verified` marks the owner as verified (non-TOFU) if the ceremony was
    /// completed — see MOBILE_INTEGRATION.md §8.
    pub fn ingest_claim(&self, claim: Vec<u8>, verified: bool) -> Result<(), FfiError> {
        let claim = Claim::decode(&claim).map_err(|_| FfiError::BadArg)?;
        self.lock().ingest_claim(claim, verified);
        Ok(())
    }

    // ----------------------------------------------------- announcements ----
    // Doc 18: authority certificates, class-5 announcements, priority clamp.

    /// Feed the host's wall clock (unix seconds) for AuthCert `not_after`
    /// checks (doc 18 §6.3) — everything else in this crate runs on
    /// monotonic `now_ms` via [`Self::tick`]. Never called ⇒ every AuthCert
    /// reads as expired (fail-closed default).
    pub fn set_wall_clock(&self, unix_s: u32) {
        self.lock().set_wall_clock(unix_s);
    }

    /// Pin a trusted operator root key for `scope` (doc 18 §2.2 QR-ceremony
    /// entry point — the "pinned," not "gossiped," trust level).
    pub fn pin_authority_root(&self, scope: Vec<u8>, issuer: Vec<u8>) -> Result<(), FfiError> {
        let scope = id16_from(&scope)?;
        let issuer = key32_from(&issuer)?;
        self.lock().pin_authority_root(scope, issuer);
        Ok(())
    }

    /// Subscribe to a scope by human-readable name (doc 18 §4, e.g.
    /// `"train/nationalrail/1234"`), returning its derived scope_id. Local
    /// filter only — never transmitted.
    pub fn subscribe_scope(&self, scope_name: String) -> Vec<u8> {
        self.lock().subscribe_scope(scope_name.as_bytes()).0.to_vec()
    }

    pub fn unsubscribe_scope(&self, scope: Vec<u8>) -> Result<(), FfiError> {
        let scope = id16_from(&scope)?;
        self.lock().unsubscribe_scope(&scope);
        Ok(())
    }

    /// Publish a class-5 announcement to `scope` (doc 18 §3). `cert`, when
    /// given, is an encoded [`AuthCert`] attached as TLV `0x81` (RECOMMENDED
    /// for `priority` ≤ 1/OPERATIONAL, doc 18 §5.2.1) so stranger receivers
    /// can elevate it without prior claim-gossip.
    pub fn publish_announcement(
        &self,
        scope: Vec<u8>,
        ann_type: u8,
        priority: u8,
        text: String,
        cert: Option<Vec<u8>>,
    ) -> Result<Vec<u8>, FfiError> {
        let scope = id16_from(&scope)?;
        let cert = cert.map(|c| AuthCert::decode(&c)).transpose().map_err(|_| FfiError::BadArg)?;
        self.lock()
            .publish_announcement(scope, ann_type, priority, text.as_bytes(), cert.as_ref())
            .map(|id| id.0.to_vec())
            .map_err(map_send_error)
    }

    /// Silently block `peer` (doc 07 §6.5): no frame, no error, just denial.
    pub fn block(&self, peer: Vec<u8>) -> Result<(), FfiError> {
        let peer = id16_from(&peer)?;
        self.lock().block(peer);
        Ok(())
    }

    /// Current 24-byte beacon payload to advertise over BLE.
    pub fn beacon(&self, now_ms: u64) -> Vec<u8> {
        self.lock().beacon(now_ms).to_vec()
    }

    /// A reliable, ordered link came up; `initiator` is true if we dialed.
    /// Link quality is not yet plumbed through the FFI, so we assume the
    /// pessimistic BLE-GATT floor (doc 02 Track B) via [`LinkQuality::default`].
    pub fn link_up(&self, link: u64, initiator: bool) {
        self.lock().on_tal_event(TalEvent::LinkUp { link, initiator, quality: LinkQuality::default() });
    }

    /// Bytes arrived on `link`.
    pub fn link_data(&self, link: u64, bytes: Vec<u8>) {
        self.lock().on_tal_event(TalEvent::LinkData { link, bytes });
    }

    /// `link` dropped (any reason — the protocol treats all alike).
    pub fn link_down(&self, link: u64) {
        self.lock().on_tal_event(TalEvent::LinkDown { link });
    }

    /// A BLE advertisement was scanned; `payload` is the raw beacon bytes
    /// (see [`MmpNode::beacon`]) and `rssi` the signal strength at receipt.
    /// The core decodes the ephemeral ID and folds it into the population
    /// estimate N̂ (doc 05 §3, contact-radar / dial-gating input) — this is
    /// bookkeeping only, not a peer identification, so it never surfaces a
    /// [`FfiEvent`] on its own.
    pub fn on_beacon(&self, payload: Vec<u8>, rssi: i8) {
        self.lock().on_tal_event(TalEvent::Beacon { payload, rssi });
    }

    /// Drain one outbound wire frame; call in a loop until `None`.
    pub fn poll_send(&self) -> Option<OutFrame> {
        self.lock().poll_send().map(|(link, bytes)| OutFrame { link, bytes })
    }

    /// Drain one app event; call in a loop until `None`.
    pub fn poll_event(&self) -> Option<FfiEvent> {
        self.lock().poll_event().map(FfiEvent::from)
    }

    /// Advance the monotonic clock: expiry, filter rotation, session timers,
    /// quota eviction (doc 11 §7). The host provides `now_ms`; the core never
    /// sleeps on its own timers.
    ///
    /// If constructed with a `store_path`, also persists the store to disk
    /// every [`PERSIST_INTERVAL_MS`] of simulated time. A persist failure is
    /// never fatal here — it is recorded and surfaced via
    /// [`Self::last_persist_error`], not returned or panicked on, since a
    /// host driving `tick` from a timer has no natural way to react to an
    /// `Err` return.
    pub fn tick(&self, now_ms: u64) {
        // Node lock is taken and released here, before any snapshot lock is
        // touched below — the fixed node-then-snapshot order (see
        // `store.rs`) is trivially satisfied because the two are never held
        // at once from this method.
        self.lock().tick(now_ms);

        let Some(snap) = self.snapshot.clone() else { return };
        let due = {
            let mut st = self.persist.lock().unwrap_or_else(|poison| poison.into_inner());
            st.last_seen_now_ms = now_ms;
            now_ms.saturating_sub(st.last_persist_ms) >= PERSIST_INTERVAL_MS
        };
        if due {
            let _ = self.persist_now(&snap, now_ms);
        }
    }

    /// Turn on the per-bundle journey log consumed by [`MmpNode::diag_dump`].
    /// Off by default (doc 12 §7.7).
    pub fn enable_diagnostics(&self) {
        self.lock().enable_diagnostics();
    }

    /// Line-delimited dump of the diagnostics journey log.
    pub fn diag_dump(&self) -> String {
        self.lock().diagnostics().dump()
    }

    /// Remove `member` from group `gid` (admin-only): epoch rotates, the
    /// remaining members re-key, the removed member reads nothing from the
    /// new epoch (doc 08 §6.3). Returns the new epoch.
    pub fn group_remove(&self, gid: Vec<u8>, member: Vec<u8>) -> Result<u16, FfiError> {
        let gid = id16_from(&gid)?;
        let member = id16_from(&member)?;
        self.lock().group_remove(&gid, &member).map_err(map_send_error)
    }

    /// Set the runtime relay tier (battery/charger/OS-mode driven, doc 11 §2).
    pub fn set_tier(&self, tier: FfiTier) {
        self.lock().set_tier(tier.into());
    }

    /// Our own 32-byte Ed25519 identity public key. This is derived from the
    /// seed but is, itself, public — never anything else derived from the
    /// seed crosses back out of this crate (see the secret-boundary note at
    /// the top of this file).
    pub fn public_key(&self) -> Vec<u8> {
        self.lock().public_key().0.to_vec()
    }

    /// Number of bundles currently held in the store (diagnostics / tests —
    /// mirrors [`Node::store_len`]).
    pub fn store_len(&self) -> u64 {
        self.lock().store_len() as u64
    }

    /// Force an immediate persist of the store to disk (e.g. from a host
    /// `onPause` / `applicationWillResignActive` hook), independent of the
    /// [`Self::tick`]-driven periodic flush. A no-op returning `Ok(())` if
    /// this node has no `store_path` (in-memory only).
    pub fn flush(&self) -> Result<(), FfiError> {
        let Some(snap) = self.snapshot.clone() else { return Ok(()) };
        let now_ms = self.persist.lock().unwrap_or_else(|poison| poison.into_inner()).last_seen_now_ms;
        self.persist_now(&snap, now_ms).map_err(|_| FfiError::Internal)
    }

    /// The `Display` of the most recent persist failure, if the most recent
    /// periodic-or-explicit persist attempt failed. `None` if the last
    /// attempt (if any) succeeded, or no `store_path` was configured.
    pub fn last_persist_error(&self) -> Option<String> {
        self.persist.lock().unwrap_or_else(|poison| poison.into_inner()).last_error.clone()
    }
}

#[cfg(test)]
mod tests;
