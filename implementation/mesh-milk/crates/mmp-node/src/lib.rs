//! mmp-node — std orchestration (doc 04, doc 11 §1–§2).
//!
//! Ties the sans-IO core, router, crypto and store into a driveable node:
//! identity + claims gossip, direct messaging with delivery receipts, group
//! messaging (sender keys + epochs), spray routing, session management over
//! any TAL, a tier ladder, and the event stream every app is a projection of
//! (HANDOFF_MOBILE §2).
//!
//! Drive model (Doze-honest, doc 12 §6): the node never sleeps on its own
//! timers. The host feeds [`TalEvent`]s and calls [`Node::tick`] with a
//! monotonic clock; the node emits wire bytes ([`Node::poll_send`]) and
//! [`Event`]s ([`Node::poll_event`]).
//!
//! Ownership note: `Node` splits into `identity` + `core` + `sessions` so the
//! engine (`&mut SessionEngine`) can call back into the host (`&mut NodeCore`)
//! while borrowing the identity immutably — three disjoint fields, no aliasing.

pub mod authority_bucket;
mod blob;
mod conv;
pub mod diag;
mod group;

pub use blob::BlobState;

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use rand::rngs::StdRng;
use rand::SeedableRng;

use mmp_core::authority::{effective_class5_priority, AuthorityStore};
use mmp_core::bloom::{Bloom, INTERNAL_SALT};
use mmp_core::ingest::{ingest, Disposition};
use mmp_core::store::{BundleMeta, BundleStore, SlotState};
use mmp_core::{
    ClaimStore, IdSource, ReceiptFilter, SeenIndex, SessionEngine, SessionEvent, SessionHost,
};
use mmp_crypto::group::SenderKeyDist;
use mmp_crypto::merkle::{compute_root, hash_block, merkle_path, merkle_root, MerklePath};
use mmp_crypto::prekey::{recipient_agree, sender_agree, PrekeyStore};
use mmp_crypto::{aead, conversation_id, ephem_id, receipt, Identity};
use mmp_router::{
    eviction_order, handoff_decision, origin_budget, Handoff, OriginBuckets, UtilityState,
};
use mmp_types::authcert::{scope_id_of, AnnouncementBody, AuthCert, RevocationList};
use mmp_types::bundle::{
    class, prio, Bundle, FLAG_ASYNC_KEYS, FLAG_HAS_PARENTS, FLAG_WANTS_RECEIPT, LIFETIME_OFFSET,
};
use mmp_types::claim::{field, friendship_signed_object, Claim, Friendship, PrekeyBatch, Revocation};
use mmp_types::consts::{
    BLOB_INTEREST_TTL_MS, BLOB_SERVE_MAX_PER_CONTACT, BLOOM_SUMMARY_MAX, EPHEM_SLOT_MS,
    FILE_MAX, FILE_MAX_STRIPES, FILE_OFFER_CACHE, FRIENDSHIP_TTL_MS, FRIEND_QUOTA,
    LIFETIME_CONTROL_MS, LIFETIME_MESSAGE_MS, L_RECEIPT, MAX_BUNDLE, MAX_PATRONS, MIME_MAX,
    NAME_MAX, OFFER_MIN_LIFETIME_MS, PREKEY_BATCH, QUOTA_ANCHOR, QUOTA_STANDARD, RS_DATA_K,
    RS_TOTAL_N, STRIPE_DATA_BYTES,
};
use mmp_types::domain::SigDomain;
use mmp_types::frame::{
    ClaimDigestEntry, Frame, FriendAckBody, FriendOfferBody, Hello, ProxySummaryBody, Summary,
};
use mmp_types::ids::{node_id_of, Id16, Key32, NodeId, Sig64};
use mmp_types::payload::{apptype, AsyncPayload, DirectPayload, GroupPayload, ReceiptPayload, SymbolPayload};
use mmp_types::tlv::{TLV_AUTHCERT_ATTACH, TLV_SCOPE_INFO};
use mmp_types::{Beacon, Ext, FileEntry, FileManifest};

use authority_bucket::AuthorityBuckets;
use blob::{BlobAssembly, FileOfferInfo};
use conv::{msg_context, Conv};
use diag::{DiagLog, JourneyEvent};
use group::GroupState;
use mmp_tal::{LinkId, TalEvent};

/// Well-known scope for claim gossip bundles (dest is routing-opaque).
const CLAIM_SCOPE: Id16 = Id16([0xff; 16]);

/// Node runtime tier (doc 04 §3), simplified ladder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    Leaf = 1,
    Relay = 2,
    Anchor = 3,
}

/// Delivery status, mapped 1:1 to protocol facts (HANDOFF_MOBILE §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryState {
    Queued,
    InMesh,
    Delivered,
    Expired,
}

/// The event stream — the spine of any app built on this node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    MessageReceived { conv: Id16, from: NodeId, id: Id16, body: Vec<u8>, apptype: u8 },
    /// Unknown parents → gap markers; repair flows via WANT (doc 06 §5.3).
    MessageGap { conv: Id16, parents: Vec<Id16> },
    Delivered { id: Id16 },
    DeliveryProgress { id: Id16, state: DeliveryState },
    ClaimUpdated { owner: NodeId, field: u8 },
    /// Equal-seq conflicting claims — normative UX duty (doc 07 §6.4).
    CompromiseWarning { owner: NodeId },
    /// A valid identity revocation (claim field `0x09`, handbook §4.6)
    /// arrived for `peer`. `successor` is `Some` only when the succession
    /// counter-signature verified (`ClaimStore::successor`) — this is the
    /// hook for the app to OFFER "trust successor," never a trigger to
    /// migrate a petname automatically (petname model: user confirmation
    /// only). Sessions with `peer` are already refused and its prekey
    /// claims already unusable by the time this fires — see `accept_peer`
    /// and `send_app`.
    PeerRevoked { peer: NodeId, successor: Option<Key32> },
    /// A friendship (handbook §4.5, claim field `0x05`) was established or
    /// renewed with `peer` — fires on BOTH sides: the patron (after
    /// countersigning an offer) and the dependent (after verifying the
    /// resulting FRIEND_ACK). `since` is the relationship's start time, used
    /// for the `FRIENDSHIP_TTL_MS` expiry check.
    FriendshipEstablished { peer: NodeId, since: u64 },
    ContactSeen { peer: NodeId },
    Established { peer: NodeId },
    GroupInvite { gid: Id16, from: NodeId },
    GroupMessage { gid: Id16, from: NodeId, id: Id16, body: Vec<u8> },
    /// Removal takes effect as the network reconverges (doc 06 §8.6).
    GroupEpochChanged { gid: Id16, epoch: u16 },
    TierChanged { tier: Tier },
    /// Doc 18: a class-5 bundle for a subscribed scope. `official` is the
    /// rendering predicate (doc 18 §5.2.1 item 2, independent of priority);
    /// `effective_priority` is the receiver-local admission clamp (doc 18
    /// §5.2) — apps should use it for notification prominence, never
    /// `official` alone (an unauthenticated sender can still be `official ==
    /// false` yet still get through at declared priority if never clamped,
    /// i.e. declared ≥ NORMAL already).
    Announcement {
        id: Id16,
        scope: Id16,
        ann_type: u8,
        not_after: u32,
        text: Vec<u8>,
        from: NodeId,
        official: bool,
        effective_priority: u8,
        /// TLV 0x82 SCOPE_INFO, display-only, never trusted (doc 18 §3).
        scope_name_hint: Option<Vec<u8>>,
    },
    /// A `FileManifest` (doc 16 §2, apptype 0x01) decrypted from a class 1/2
    /// body parsed and verified successfully — fired IN ADDITION to the
    /// ordinary `MessageReceived` (existing consumers unaffected). Any parse
    /// or verification failure is a silent no-op for this event path (doc 16
    /// §3.1 fail-safe: `MessageReceived` alone still fires, nothing errors).
    FileOffered { conv: Id16, from: NodeId, sharer: Key32, entries: Vec<FileOffer> },
    /// A pull-fetched file (`Node::fetch_file`) finished assembling and
    /// passed its whole-file BLAKE3 check; `path` is where it was written.
    FileReady { root: Id16, path: PathBuf },
    /// A blob assembly moved to ABANDONED (doc 11 §6): either the whole-file
    /// hash check failed after all stripes decoded, or the assembly's
    /// `BLOB_INTEREST_TTL_MS` lapsed / `Node::cancel_fetch` was called.
    FileFailed { root: Id16, reason: FileFailReason },
}

/// One file offer inside a verified `FileManifest` (doc 16 §2 `FileEntry`,
/// trimmed to what an app needs to render an offer / call `fetch_file`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileOffer {
    pub name: Vec<u8>,
    pub mime: Vec<u8>,
    pub size: u32,
    pub root: Id16,
    pub thumb_root: Option<Id16>,
}

/// Why a blob assembly reached ABANDONED (`Event::FileFailed`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileFailReason {
    /// All stripes decoded but the whole-file BLAKE3 check did not match the
    /// manifest's `full_hash` — a lying manifest or a corrupt symbol set that
    /// somehow passed per-symbol Merkle verification (decision D2 finalize).
    HashMismatch,
    /// TTL lapsed, `Node::cancel_fetch` was called, or a filesystem error
    /// prevented finalizing (rename/remove failure) — the assembly is dead
    /// but its partial symbols remain servable via `blob_index` until their
    /// own bundle-level expiry (doc 11 §6 residual, decision D3).
    Abandoned,
}

/// `Node::fetch_file`'s error surface — deliberately small: a fetch of an
/// already-active root is idempotent `Ok(())`, never an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FetchError {
    /// No cached, verified offer for this root (`Node::fetch_file` requires
    /// having seen the manifest first — doc 16 §7.1 role-gated pull policy).
    UnknownRoot,
    /// A filesystem error setting up the incremental `.part` write target.
    Io,
}

/// Result of a successful [`Node::send_file`] call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileSend {
    /// BundleID of the `FileManifest` message (track via `Node::message_state`).
    pub message_id: Id16,
    /// The file's content-addressed Merkle root — what `fetch_file` names.
    pub root: Id16,
}

/// Snapshot of one blob assembly's progress (`Node::file_progress`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileProgress {
    pub state: BlobState,
    pub have_symbols: u32,
    pub total_symbols: u32,
    pub stripes_done: u16,
    pub stripes: u16,
}

#[derive(Debug, Clone, Copy)]
struct SentMessage {
    conv: Id16,
    state: DeliveryState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendError {
    /// We lack the peer's exchange-key / prekey-batch claim (or group key).
    UnknownPeer,
    /// Body exceeds MAX_BUNDLE after framing.
    TooLarge,
    /// `peer` has published a valid identity revocation (handbook §4.6) —
    /// refuse to open a new conversation or use their prekey batch, even if
    /// one is still on file from before the revocation.
    PeerRevoked,
}

/// Domain-partitioned, offer-eligible snapshot of the store (RBSR seam).
pub struct SyncView {
    /// Sorted by id: (id, domain).
    entries: Vec<(Id16, u8)>,
}

impl IdSource for SyncView {
    fn ids_in(&mut self, domain: u8, lo: &Id16, hi: &Id16) -> Vec<Id16> {
        self.entries
            .iter()
            .filter(|(id, d)| *d == domain && id >= lo && id <= hi)
            .map(|(id, _)| *id)
            .collect()
    }
    fn has(&mut self, id: &Id16) -> bool {
        self.entries.binary_search_by(|(e, _)| e.cmp(id)).is_ok()
    }
}

/// Per-dependent bookkeeping on the patron side of a friendship (handbook
/// §4.5). `since` mirrors the offer/ack timestamp the countersig was made
/// over — used for `FRIENDSHIP_TTL_MS` expiry, doc 04 §4.4.
struct FriendState {
    since: u64,
    quota_used: u64,
    buffered: BTreeSet<Id16>,
}

impl FriendState {
    /// True iff this friendship is still live and has room left to buffer
    /// `size` more bytes (TTL: doc 04 §4.4; quota: handbook §4.5 item 2).
    fn has_room(&self, now: u64, size: u64) -> bool {
        now.saturating_sub(self.since) < FRIENDSHIP_TTL_MS && self.quota_used + size <= FRIEND_QUOTA
    }
}

/// Everything but the session map and the engines' identity — the SessionHost.
struct NodeCore {
    identity: Identity,
    node_id: NodeId,
    public: Key32,
    exchange: Key32,
    store: Box<dyn BundleStore + Send>,
    seen: SeenIndex,
    receipts: ReceiptFilter,
    claims: ClaimStore,
    utility: UtilityState,
    buckets: OriginBuckets,
    /// Doc 18: verification, sticky revocation, scope subscription. RAM-only,
    /// like `auth_buckets` below (see `authority_bucket.rs`'s module doc for
    /// the persistence-decision rationale).
    authority: AuthorityStore,
    auth_buckets: AuthorityBuckets,
    /// Wall clock (unix seconds), for `not_after` checks only (doc 18 §6.3).
    /// Everything else in this crate runs on the monotonic `now`. Fail-closed
    /// default 0: every cert reads as expired until the host calls
    /// `Node::set_wall_clock`, so an un-clocked node clamps everything to
    /// NORMAL and renders nothing official, rather than trusting anything.
    wall_unix_s: u32,
    convs: HashMap<Id16, Conv>,
    groups: HashMap<Id16, GroupState>,
    prekeys: PrekeyStore,
    /// (conv → X3DH material) consumed by the first outgoing message.
    pending_async: HashMap<Id16, (Key32, u16)>,
    /// Sender-key dists already sent, keyed (gid, member, epoch).
    sent_dists: HashSet<(Id16, NodeId, u16)>,
    events: VecDeque<Event>,
    sent: HashMap<Id16, SentMessage>,
    sprayed: HashSet<Id16>,
    blocklist: HashSet<NodeId>,
    tier: Tier,
    quota: u64,
    diag: DiagLog,
    rng: StdRng,
    now: u64,
    sync_view: SyncView,
    /// Identities learned via ID_PROOF (doc 08 §3.2, recorded in
    /// `accept_peer`) — resolves a session peer's `NodeId` back to its
    /// Ed25519 identity key for the friendship handshake, which the
    /// `SessionHost` seam only carries `NodeId` for (handbook §4.5).
    known_identities: HashMap<NodeId, Key32>,
    /// Dependents we buffer for, patron side (handbook §4.5).
    friends: HashMap<NodeId, FriendState>,
    /// Patrons buffering for us, dependent side: peer → since.
    patrons: HashMap<NodeId, u64>,
    /// Dependent side: peers we want as a patron, offer queued for the next
    /// Established session with them.
    pending_offers: HashSet<NodeId>,
    /// Dependent side: the `since` value of our most recently sent
    /// FRIEND_OFFER per peer — needed to reconstruct
    /// `friendship_signed_object` when the FRIEND_ACK arrives (the ack body
    /// carries only the countersig, not `since`).
    offered_since: HashMap<NodeId, u64>,
    /// Patron side allowlist: identities we'll countersign an offer from —
    /// no auto-accept (doc 04 §4 security note).
    friend_allowlist: HashSet<NodeId>,
    /// Advisory PROXY_SUMMARY hints: patron → dependents it claims to
    /// buffer for. Spray-suppression only, never authoritative (doc 06 §7
    /// MVP, same Hint/Purge asymmetry as AUDIT F11).
    proxy_hints: HashMap<NodeId, HashSet<NodeId>>,
    /// Dependent side: our own next `seq` for republishing the field-0x05
    /// claim (renewal bumps it — doc 06 §4.2 LWW rule).
    next_friendship_seq: u32,

    // -------------------------------------------------- blob plane (E5-S1) --
    /// Requester-side doc 11 §6 FSM instance per root (decision D2/D3).
    assemblies: HashMap<Id16, BlobAssembly>,
    /// Holder-side index: root → sym_no → BundleID, maintained on every
    /// class-4 `Stored` disposition and pruned on expiry/eviction (decision D2).
    blob_index: HashMap<Id16, BTreeMap<u16, Id16>>,
    /// Reverse index for O(1) prune: BundleID → (root, sym_no).
    symbol_of: HashMap<Id16, (Id16, u16)>,
    /// Cached, verified `FileManifest` facts per `file_root`, FIFO-capped at
    /// `FILE_OFFER_CACHE` (decision D2/D8).
    offers: HashMap<Id16, FileOfferInfo>,
    offer_order: VecDeque<Id16>,
    /// Per-peer-per-contact serve counter (decision D4/D5), reset in
    /// `accept_peer`.
    blob_served: HashMap<NodeId, u32>,
}

pub struct Node {
    identity: Identity,
    core: NodeCore,
    sessions: HashMap<LinkId, SessionEngine>,
    outbox: VecDeque<(LinkId, Vec<u8>)>,
    hello: Hello,
    beacon_salt: [u8; 16],
}

impl Node {
    /// Create a node from a 32-byte identity seed and a storage backend, and
    /// publish its identity claims (exchange key + prekey batch).
    pub fn new(seed: [u8; 32], store: Box<dyn BundleStore + Send>, tier: Tier) -> Node {
        let identity = Identity::from_seed(&seed);
        let mut rng = StdRng::from_seed(blake3::derive_key("mmp1 node-rng", &seed));
        let prekeys = PrekeyStore::generate(&mut rng, PREKEY_BATCH);
        let quota = if tier == Tier::Anchor { QUOTA_ANCHOR } else { QUOTA_STANDARD };
        let mut beacon_salt = [0u8; 16];
        beacon_salt.copy_from_slice(&blake3::derive_key("mmp1 beacon-seed", &seed)[..16]);
        let mut core = NodeCore {
            node_id: identity.node_id(),
            public: identity.public(),
            exchange: identity.exchange_public(),
            identity: identity.clone(),
            store,
            seen: SeenIndex::new(),
            receipts: ReceiptFilter::new(),
            claims: ClaimStore::new(),
            utility: UtilityState::new(),
            buckets: OriginBuckets::new(),
            authority: AuthorityStore::new(),
            auth_buckets: AuthorityBuckets::new(),
            wall_unix_s: 0,
            convs: HashMap::new(),
            groups: HashMap::new(),
            prekeys,
            pending_async: HashMap::new(),
            sent_dists: HashSet::new(),
            events: VecDeque::new(),
            sent: HashMap::new(),
            sprayed: HashSet::new(),
            blocklist: HashSet::new(),
            tier,
            quota,
            diag: DiagLog::new(false),
            rng,
            now: 0,
            sync_view: SyncView { entries: Vec::new() },
            known_identities: HashMap::new(),
            friends: HashMap::new(),
            patrons: HashMap::new(),
            pending_offers: HashSet::new(),
            offered_since: HashMap::new(),
            friend_allowlist: HashSet::new(),
            proxy_hints: HashMap::new(),
            next_friendship_seq: 1,
            assemblies: HashMap::new(),
            blob_index: HashMap::new(),
            symbol_of: HashMap::new(),
            offers: HashMap::new(),
            offer_order: VecDeque::new(),
            blob_served: HashMap::new(),
        };
        // Handbook §4.4 D2: rebuild the holder-side symbol index from
        // whatever the store already holds (a restored `SnapshotStore`, or —
        // in the MVP — simply nothing on a fresh `MemStore`). Bounded by
        // quota, runs once at startup, mirrors how claims/groups restore.
        core.rebuild_blob_index();
        let mut node = Node {
            identity,
            core,
            sessions: HashMap::new(),
            outbox: VecDeque::new(),
            hello: Hello { ver: 1, suites: vec![0x01], tier: tier as u8, flags: 0, credits: 8, psm: 0 },
            beacon_salt,
        };
        // Identity claims (doc 03 §3.3): exchange key + prekey batch, seq 1.
        node.publish_claim(field::EXCHANGE_KEY, 1, node.core.exchange.0.to_vec());
        let batch = node.core.prekeys.batch(&node.identity).encode();
        node.publish_claim(field::PREKEY_BATCH, 1, batch);
        node
    }

    pub fn enable_diagnostics(&mut self) {
        self.core.diag = DiagLog::new(true);
    }

    /// Override the store quota (bytes) that [`Node::tick`] enforces. The
    /// production quota is fixed by tier (doc 04 §2); this exists so tests and
    /// the simulator can drive real eviction pressure at a small, CI-fast
    /// scale instead of the tens of thousands of bundles a tier-sized quota
    /// (128 MiB) would require. Not part of the protocol surface — never
    /// called from the wire or FFI paths.
    pub fn set_store_quota(&mut self, quota_bytes: u64) {
        self.core.quota = quota_bytes;
    }

    pub fn node_id(&self) -> NodeId {
        self.core.node_id
    }

    pub fn public_key(&self) -> Key32 {
        self.core.public
    }

    pub fn tier(&self) -> Tier {
        self.core.tier
    }

    pub fn store_len(&self) -> usize {
        self.core.store.len()
    }

    pub fn diagnostics(&self) -> &DiagLog {
        &self.core.diag
    }

    pub fn nickname_of(&self, peer: &NodeId) -> Option<String> {
        self.core
            .claims
            .get(peer, field::NICKNAME)
            .and_then(|c| String::from_utf8(c.body.clone()).ok())
    }

    /// Set the runtime tier (battery/charger/OS-mode driven, doc 11 §2).
    pub fn set_tier(&mut self, tier: Tier) {
        if tier != self.core.tier {
            self.core.tier = tier;
            self.hello.tier = tier as u8;
            self.core.quota = if tier == Tier::Anchor { QUOTA_ANCHOR } else { QUOTA_STANDARD };
            self.core.events.push_back(Event::TierChanged { tier });
        }
    }

    pub fn block(&mut self, peer: NodeId) {
        // Silent (doc 07 §6.5): no frame, no error — just admission denial.
        self.core.blocklist.insert(peer);
    }

    /// Dependent side (handbook §4.5): solicit `peer_identity` as a patron —
    /// a FRIEND_OFFER is sent on the next Established session with them.
    /// Enforces `MAX_PATRONS` (doc 04 §4.4): a call once already at the cap
    /// across live patrons + outstanding offers is a silent no-op, the same
    /// fire-and-forget shape as [`Node::block`].
    pub fn request_friendship(&mut self, peer_identity: Key32) {
        let peer = node_id_of(&peer_identity);
        if self.core.patrons.contains_key(&peer) || self.core.pending_offers.contains(&peer) {
            return;
        }
        if self.core.patrons.len() + self.core.pending_offers.len() >= MAX_PATRONS {
            return;
        }
        self.core.pending_offers.insert(peer);
    }

    /// Patron side (handbook §4.5): whitelist `peer_identity` to countersign
    /// a friendship offer from them. No auto-accept (doc 04 §4 security
    /// note) — without this call, offers from `peer_identity` are ignored.
    pub fn allow_friendship_from(&mut self, peer_identity: Key32) {
        self.core.friend_allowlist.insert(node_id_of(&peer_identity));
    }

    /// Feed the host's wall clock (unix seconds) for doc 18 `not_after`
    /// checks — the only place in this crate that isn't driven by `tick`'s
    /// monotonic `now_ms`. Never called ⇒ every AuthCert reads as expired
    /// (fail-closed default 0).
    pub fn set_wall_clock(&mut self, unix_s: u32) {
        self.core.wall_unix_s = unix_s;
    }

    /// Pin a trusted operator root key for `scope` (doc 18 §2.2 QR-ceremony
    /// entry point — the "pinned," not "gossiped," trust level).
    pub fn pin_authority_root(&mut self, scope: Id16, issuer: Key32) {
        self.core.authority.pin_root(scope, issuer);
    }

    /// Subscribe to a scope by human-readable name (doc 18 §4), returning its
    /// derived `scope_id`. Local filter only — never transmitted.
    pub fn subscribe_scope(&mut self, scope_name: &[u8]) -> Id16 {
        let id = scope_id_of(scope_name);
        self.core.authority.subscribe(id);
        id
    }

    pub fn subscribe_scope_id(&mut self, scope: Id16) {
        self.core.authority.subscribe(scope);
    }

    pub fn unsubscribe_scope(&mut self, scope: &Id16) {
        self.core.authority.unsubscribe(scope);
    }

    /// Publish a signed profile claim about ourselves (doc 06 §4) and gossip
    /// it as a class-3 bundle. Returns the claim's encoded bytes (e.g. for
    /// [`Node::revoke_identity`], which needs them to hand back to the caller).
    pub fn publish_claim(&mut self, field_id: u8, seq: u32, body: Vec<u8>) -> Vec<u8> {
        self.core.publish_claim_now(field_id, seq, body)
    }

    pub fn set_nickname(&mut self, name: &str, seq: u32) {
        self.publish_claim(field::NICKNAME, seq, name.as_bytes().to_vec());
    }

    /// Publish a self-revocation claim (field `0x09`, handbook §4.6) —
    /// tombstones every other claim we own, in every node that adopts it:
    /// they refuse new sessions with us (`accept_peer`) and stop using our
    /// prekey batch for X3DH (`send_app`), via `ClaimStore::merge`'s
    /// tombstone rule. Irreversible once gossiped and adopted — there is no
    /// "un-revoke." No successor: this is a pure "this identity is dead"
    /// tombstone. A succession claim needs the successor's own
    /// counter-signature (`succession_signed_object`) and is out of scope for
    /// this single-call entry point — build a [`Revocation`] with `successor`
    /// / `successor_countersig` set and call [`Node::publish_claim`] directly
    /// if succession is needed. Returns the claim's encoded bytes.
    pub fn revoke_identity(&mut self, reason: u8) -> Vec<u8> {
        let body = Revocation {
            revoked_key: self.core.public,
            reason,
            successor: None,
            successor_countersig: None,
        }
        .encode();
        // A revocation is a one-shot tombstone (immutable once adopted, doc
        // 06 §4.6) — unlike `set_nickname`'s host-tracked seq, there is never
        // a second write to race against, so a fixed seq is correct here.
        self.publish_claim(field::REVOCATION, 1, body)
    }

    /// Inject an out-of-band-verified claim (QR ceremony, doc 07 §6.1) or a
    /// pre-shared bootstrap claim.
    pub fn ingest_claim(&mut self, claim: Claim, verified: bool) {
        if mmp_crypto::verify(SigDomain::Claim, &claim.owner, &claim.signed_object(), &claim.sig)
            .is_err()
        {
            return;
        }
        if verified {
            self.core.utility.mark_verified(&node_id_of(&claim.owner));
        }
        self.core.learn_claim(claim, verified);
    }

    /// Our own claims, for out-of-band exchange with a peer (tests, QR, sim).
    pub fn own_claims(&self) -> Vec<Claim> {
        self.core
            .claims
            .iter()
            .filter(|c| c.owner == self.core.public)
            .cloned()
            .collect()
    }

    /// Every claim currently held (ours and every peer's), for persistence
    /// (`mmp-store` snapshot, handbook §4.3 step 1). Unlike [`Node::own_claims`]
    /// this is not scoped to our own identity — the whole `ClaimStore` is
    /// what a restart needs to restore.
    pub fn all_claims(&self) -> Vec<Claim> {
        self.core.claims.iter().cloned().collect()
    }

    /// Every group's persistence blob (handbook §4.3 step 2) — membership +
    /// epoch only, see [`GroupState::encode`] for exactly what is and is not
    /// included.
    pub fn all_group_blobs(&self) -> Vec<Vec<u8>> {
        self.core.groups.values().map(GroupState::encode).collect()
    }

    /// Restore one group from a blob previously produced by
    /// [`Node::all_group_blobs`] (typically called once per persisted group,
    /// right after [`Node::new`], before any other traffic is processed).
    /// Returns `false` on a corrupt/malformed blob — mirrors the
    /// drop-don't-repair discipline `mmp-store` already applies to bundles
    /// and claims; the caller should simply skip it, not treat it as fatal.
    pub fn restore_group(&mut self, blob: &[u8]) -> bool {
        let Some(gs) = GroupState::decode(&mut self.core.rng, blob) else { return false };
        self.core.groups.insert(gs.gid, gs);
        true
    }

    // ------------------------------------------------------------- sending --

    /// Send a text message to `peer`; returns the BundleID to track.
    pub fn send_text(&mut self, peer: &NodeId, text: &str) -> Result<Id16, SendError> {
        self.send_body(peer, apptype::TEXT, text.as_bytes())
    }

    /// Send a raw application body (e.g. a FileManifest as apptype 0x01).
    pub fn send_body(&mut self, peer: &NodeId, app: u8, payload: &[u8]) -> Result<Id16, SendError> {
        let mut body = Vec::with_capacity(1 + payload.len());
        body.push(app);
        body.extend_from_slice(payload);
        self.core.send_direct(peer, &body)
    }

    pub fn message_state(&self, id: &Id16) -> Option<DeliveryState> {
        self.core.sent.get(id).map(|m| m.state)
    }

    // -------------------------------------------------- blob plane (§4.4) --

    /// Share `data` (≤ `FILE_MAX`) into the 1:1 conversation with `peer`:
    /// RS-encode → Merkle → mint class-4 symbol bundles (deduped by content
    /// address, decision D6) → send the signed `FileManifest` as an
    /// apptype-0x01 class-1 message (doc 16, decision D8).
    pub fn send_file(&mut self, peer: &NodeId, name: &str, mime: &str, data: &[u8]) -> Result<FileSend, SendError> {
        self.core.send_file(peer, name, mime, data)
    }

    /// Explicit user pull (doc 16 §7.1 role-gated policy): requires a cached
    /// verified offer for `root` (i.e. a `FileOffered` event must have
    /// arrived first) — registers the doc 11 §6 assembly (`Wanted`), absorbs
    /// any symbols already in the store, then repairs via BLOB_HAVE on the
    /// next session with a holder. Idempotent `Ok(())` if `root` already has
    /// an active (`Wanted`/`Fetching`) assembly.
    pub fn fetch_file(&mut self, root: &Id16, dest: &Path) -> Result<(), FetchError> {
        self.core.fetch_file(root, dest)
    }

    /// Cancel an in-progress fetch: → `Abandoned`. Already-downloaded
    /// partial symbols stay servable to other peers via the store's ordinary
    /// TTL, per doc 11 §6.
    pub fn cancel_fetch(&mut self, root: &Id16) {
        self.core.cancel_fetch(root);
    }

    /// Snapshot of one blob assembly's progress, or `None` if `root` has no
    /// assembly (never fetched, or GC'd after `Abandoned` with nothing left
    /// to serve).
    pub fn file_progress(&self, root: &Id16) -> Option<FileProgress> {
        self.core.file_progress(root)
    }

    // --------------------------------------------------------- announcements

    /// Publish a class-5 announcement to `scope` (doc 18 §3): plaintext,
    /// inner-signed by this node's identity as the authority SUBJECT key.
    /// When `cert` is given it's attached as TLV `0x81 AUTHCERT_ATTACH`
    /// (RECOMMENDED for `priority` ≤ OPERATIONAL, doc 18 §5.2.1) so stranger
    /// receivers can elevate on first contact without prior claim-gossip.
    pub fn publish_announcement(
        &mut self,
        scope: Id16,
        ann_type: u8,
        priority: u8,
        text: &[u8],
        cert: Option<&AuthCert>,
    ) -> Result<Id16, SendError> {
        self.core.publish_announcement(scope, ann_type, priority, text, cert)
    }

    // -------------------------------------------------------------- groups --

    /// Create a group; we are its (immutable-set v1) admin.
    pub fn create_group(&mut self) -> Id16 {
        use rand_core::RngCore;
        let mut gid = [0u8; 16];
        self.core.rng.fill_bytes(&mut gid);
        let gid = Id16(gid);
        let gs = GroupState::create(&mut self.core.rng, gid, self.core.node_id, true);
        self.core.groups.insert(gid, gs);
        gid
    }

    /// Invite `peer`: add to the member set and ship them every sender key we
    /// hold (ours + known members') through the pairwise channel.
    pub fn group_invite(&mut self, gid: &Id16, peer: &NodeId) -> Result<(), SendError> {
        let me = self.core.node_id;
        {
            let gs = self.core.groups.get_mut(gid).ok_or(SendError::UnknownPeer)?;
            gs.add_member(*peer);
        }
        self.core.send_own_dist(gid, peer)?;
        // Forward the dists of every other member we know (admin duty).
        let known: Vec<(NodeId, Vec<u8>)> = self
            .core
            .groups
            .get(gid)
            .map(|gs| {
                gs.dists
                    .iter()
                    .filter(|((m, e), _)| *m != *peer && *m != me && *e == gs.epoch)
                    .map(|((m, _), dist)| (*m, dist.encode()))
                    .collect()
            })
            .unwrap_or_default();
        for (member, dist) in known {
            let mut body = Vec::with_capacity(16 + dist.len());
            body.extend_from_slice(&member.0);
            body.extend_from_slice(&dist);
            let _ = self.core.send_app(peer, apptype::SENDER_KEY, &body);
        }
        Ok(())
    }

    /// Remove a member: epoch rotates, everyone re-keys, the removed member
    /// reads nothing from the new epoch (doc 08 §6.3).
    pub fn group_remove(&mut self, gid: &Id16, member: &NodeId) -> Result<u16, SendError> {
        let (epoch, members) = {
            let gs = self.core.groups.get_mut(gid).ok_or(SendError::UnknownPeer)?;
            if !gs.is_admin {
                return Err(SendError::UnknownPeer);
            }
            let epoch = gs.remove_member(&mut self.core.rng, member);
            (epoch, gs.members.members())
        };
        let me = self.core.node_id;
        for m in members {
            if m != me && m != *member {
                let _ = self.core.send_own_dist(gid, &m);
            }
        }
        self.core.events.push_back(Event::GroupEpochChanged { gid: *gid, epoch });
        Ok(epoch)
    }

    /// Send a text into the group under our sender key (doc 08 §6.2).
    pub fn group_send(&mut self, gid: &Id16, text: &str) -> Result<Id16, SendError> {
        self.core.send_group(gid, text.as_bytes())
    }

    pub fn group_members(&self, gid: &Id16) -> Vec<NodeId> {
        self.core.groups.get(gid).map(|g| g.members.members()).unwrap_or_default()
    }

    // -------------------------------------------------------------- beacon --

    /// Current 24-byte beacon payload (doc 03 §5.1). Fragment content is the
    /// re-salted summary schedule; the MVP broadcasts a zero fragment (beacon
    /// summaries are a ≤100-node optimization anyway, AUDIT F8b).
    pub fn beacon(&self, now_ms: u64) -> [u8; 24] {
        let slot = now_ms / EPHEM_SLOT_MS;
        Beacon {
            ver: 1,
            tier: self.core.tier as u8 & 0x03,
            battery: 0,
            ephem_id: ephem_id(&self.core.public, slot, &self.beacon_salt),
            busy: !self.sessions.is_empty(),
            epoch: (slot & 0x7f) as u8,
            frag_structure: 0,
            frag_offset: 0,
            fragment: [0; 12],
        }
        .encode()
    }

    // --------------------------------------------------------- TAL driving --

    pub fn on_tal_event(&mut self, ev: TalEvent) {
        match ev {
            TalEvent::Beacon { payload, .. } => {
                if let Ok(b) = Beacon::decode(&payload) {
                    self.core.utility.note_ephem(b.ephem_id, self.core.now);
                }
            }
            TalEvent::LinkUp { link, initiator, .. } => {
                let engine = if initiator {
                    SessionEngine::initiator(&self.identity, &self.hello, self.core.now)
                } else {
                    SessionEngine::responder(&self.identity, &self.hello, self.core.now)
                };
                if let Ok(e) = engine {
                    self.sessions.insert(link, e);
                    self.pump_link(link);
                }
            }
            TalEvent::LinkData { link, bytes } => {
                if let Some(engine) = self.sessions.get_mut(&link) {
                    engine.on_wire(&bytes, self.core.now, &self.identity, &mut self.core);
                }
                self.pump_link(link);
            }
            TalEvent::LinkDown { link } => {
                self.sessions.remove(&link);
            }
        }
    }

    fn pump_link(&mut self, link: LinkId) {
        let Some(engine) = self.sessions.get_mut(&link) else { return };
        while let Some(bytes) = engine.poll_out() {
            self.outbox.push_back((link, bytes));
        }
        let mut closed = false;
        while let Some(ev) = engine.poll_event() {
            match ev {
                SessionEvent::Established { peer, .. } => {
                    self.core.utility.note_contact(&peer, 1000, self.core.now);
                    self.core.events.push_back(Event::Established { peer });
                    self.core.events.push_back(Event::ContactSeen { peer });
                }
                SessionEvent::Closed => closed = true,
            }
        }
        if closed {
            self.sessions.remove(&link);
        }
    }

    /// Wire bytes to write to a link; drain fully each turn.
    pub fn poll_send(&mut self) -> Option<(LinkId, Vec<u8>)> {
        self.outbox.pop_front()
    }

    pub fn poll_event(&mut self) -> Option<Event> {
        self.core.events.pop_front()
    }

    /// Advance the monotonic clock (doc 11 §7): expiry, filter rotation,
    /// session timers, quota eviction.
    pub fn tick(&mut self, now_ms: u64) {
        self.core.now = now_ms;
        let expired = self.core.store.expire(now_ms);
        for id in &expired {
            self.core.diag.log(*id, JourneyEvent::Expired, now_ms);
            if let Some(m) = self.core.sent.get_mut(id) {
                if m.state != DeliveryState::Delivered {
                    m.state = DeliveryState::Expired;
                    self.core
                        .events
                        .push_back(Event::DeliveryProgress { id: *id, state: DeliveryState::Expired });
                }
            }
        }
        // Handbook §4.5: `store.expire` already deleted the bytes, so unpin
        // any friend-buffered ids that just expired before recomputing quota.
        self.core.unpin_expired(&expired);
        // Decision D2: same treatment for the blob symbol index.
        self.core.prune_expired_symbols(&expired);
        self.core.seen.rotate(now_ms);
        self.core.receipts.rotate(now_ms);
        self.core.claims.expire(now_ms);
        self.core.prune_expired_friendships(now_ms);
        // Decision D3: assemblies past BLOB_INTEREST_TTL_MS → Abandoned.
        self.core.sweep_blob_ttl(now_ms);
        self.core.enforce_quota();

        let links: Vec<LinkId> = self.sessions.keys().copied().collect();
        for link in links {
            if let Some(engine) = self.sessions.get_mut(&link) {
                engine.tick(now_ms);
            }
            self.pump_link(link);
        }
    }
}

// ------------------------------------------------------------- NodeCore -----

impl NodeCore {
    fn learn_claim(&mut self, claim: Claim, pinned: bool) {
        let owner = node_id_of(&claim.owner);
        let field_id = claim.field;
        let claim_owner = claim.owner;
        let claim_body = claim.body.clone();
        match self.claims.merge(claim, self.now, pinned) {
            mmp_core::ClaimMerge::Adopted => {
                self.events.push_back(Event::ClaimUpdated { owner, field: field_id });
                // Doc 18 §2.1/§6: AuthCert and REVOCATION_LIST ride ordinary
                // claim gossip at fields 0x07/0x08. Only acted on once
                // adopted — a Stale claim means we already hold the winner,
                // which was processed the moment IT was adopted.
                match field_id {
                    field::AUTHCERT => {
                        if claim_body.is_empty() {
                            // Doc 18 §6 subject-signed tombstone: empty body.
                            self.authority.drop_subject_certs(&claim_owner);
                        } else if let Ok(cert) = AuthCert::decode(&claim_body) {
                            // The claim asserts "I hold this grant" (doc 18
                            // §2.1) — owner must equal the cert's own subject.
                            if cert.subject == claim_owner {
                                let _ = self.authority.consider_cert(&cert, self.wall_unix_s);
                            }
                        }
                    }
                    field::REVOCATION_LIST => {
                        if let Ok(list) = RevocationList::decode(&claim_body) {
                            self.authority.merge_revocation_list(&claim_owner, &list);
                        }
                    }
                    field::REVOCATION => {
                        // Handbook §4.6: `successor` is only `Some` once the
                        // counter-signature (by the SUCCESSOR's own key) has
                        // verified — a hook for explicit user confirmation,
                        // never an automatic petname migration.
                        let successor = self.claims.successor(&owner);
                        self.events.push_back(Event::PeerRevoked { peer: owner, successor });
                    }
                    _ => {}
                }
            }
            mmp_core::ClaimMerge::Conflict { .. } => {
                self.events.push_back(Event::CompromiseWarning { owner });
            }
            mmp_core::ClaimMerge::Stale | mmp_core::ClaimMerge::Rejected => {}
        }
    }

    /// Sign, learn, and gossip a claim — [`Node::publish_claim`] delegates
    /// here, and it's also used directly from inside the `SessionHost` seam
    /// (`on_friend_ack`, handbook §4.5), which only has `&mut NodeCore`, not
    /// the outer `Node`.
    fn publish_claim_now(&mut self, field_id: u8, seq: u32, body: Vec<u8>) -> Vec<u8> {
        let mut claim = Claim { owner: self.public, field: field_id, seq, body, sig: Sig64::ZERO };
        claim.sig = self.identity.sign(SigDomain::Claim, &claim.signed_object());
        let encoded = claim.encode();
        self.learn_claim(claim.clone(), true);
        self.gossip_claim(&claim);
        encoded
    }

    /// Wrap a claim in a class-3 bundle so it gossips through sync (D0).
    fn gossip_claim(&mut self, claim: &Claim) {
        let mut bundle = Bundle {
            class: class::CLAIM,
            flags: 0,
            origin: self.public,
            dest: CLAIM_SCOPE,
            lifetime_ms: mmp_types::consts::LIFETIME_CLAIM_MS,
            priority: 2,
            exts: Vec::new(),
            payload: claim.encode(),
            signature: Sig64::ZERO,
        };
        bundle.signature = self.identity.sign(SigDomain::Bundle, &bundle.canonical_zeroed());
        self.store_own(bundle, 8);
    }

    /// Store one of our own bundles (already signed) with a spray budget.
    fn store_own(&mut self, bundle: Bundle, budget: u8) -> Id16 {
        let bytes = bundle.encode();
        let id = bundle.id();
        self.seen.insert(&id, bundle.class, self.now);
        let meta = BundleMeta {
            id,
            class: bundle.class,
            origin: self.node_id,
            dest: bundle.dest,
            priority: bundle.priority,
            // Own originals: a node trusts itself, no receive-time clamp
            // applies (doc 18 §5.2 is about what's adopted from *others*).
            effective_priority: bundle.priority,
            size: bytes.len() as u32,
            stored_at: self.now,
            expires_at: self.now + bundle.lifetime_ms,
            budget,
            own_origin: true,
            slot: SlotState::Stored,
        };
        self.store.put(meta, &bytes);
        self.diag.log(id, JourneyEvent::Created, self.now);
        id
    }

    /// Doc 18 §3: build, inner-sign (as the authority SUBJECT), and store a
    /// class-5 announcement bundle. See `Node::publish_announcement`.
    fn publish_announcement(
        &mut self,
        scope: Id16,
        ann_type: u8,
        priority: u8,
        text: &[u8],
        cert: Option<&AuthCert>,
    ) -> Result<Id16, SendError> {
        let mut body = AnnouncementBody {
            suite: 1,
            ann_type,
            scope,
            not_after: 0, // ⚠ no display-validity policy yet; caller-set validity is future work
            text: text.to_vec(),
            sig: Sig64::ZERO,
        };
        body.sig = self.identity.sign(SigDomain::Announce, &body.signed_object());

        let mut exts = Vec::new();
        if let Some(c) = cert {
            exts.push(Ext::Optional { typ: TLV_AUTHCERT_ATTACH, val: c.encode() });
        }

        let mut bundle = Bundle {
            class: class::ANNOUNCEMENT,
            flags: 0,
            origin: self.public,
            dest: scope,
            lifetime_ms: LIFETIME_MESSAGE_MS,
            priority,
            exts,
            payload: body.encode(),
            signature: Sig64::ZERO,
        };
        bundle.signature = self.identity.sign(SigDomain::Bundle, &bundle.canonical_zeroed());
        if bundle.encode().len() > MAX_BUNDLE {
            return Err(SendError::TooLarge);
        }
        let budget = origin_budget(self.utility.n_hat(self.now));
        Ok(self.store_own(bundle, budget))
    }

    /// Doc 18 §5.2: verify, elevate-if-authorized, and surface a class-5
    /// bundle. Every failure here degrades to "relay-only, clamped, never
    /// rendered official" — NEVER a drop; the bundle is already stored by the
    /// time this runs (ingest's admission gate is priority-blind, doc 18
    /// §5.2.2: "priority buys order, never volume").
    fn handle_announcement(&mut self, bundle: &Bundle) {
        let Ok(body) = AnnouncementBody::decode(&bundle.payload) else { return };
        if mmp_crypto::verify(SigDomain::Announce, &bundle.origin, &body.signed_object(), &body.sig).is_err() {
            return;
        }
        // Doc 18 §3: scope is redundant with dest, explicit for standalone
        // re-verification — a mismatch is fail-closed (no render, relay only).
        if body.scope != bundle.dest {
            return;
        }

        // TLV 0x81: a self-contained attached cert is verified and adopted
        // exactly as a gossiped one would be (doc 18 §5.2.1 item 1).
        for ext in &bundle.exts {
            if let Ext::Optional { typ: TLV_AUTHCERT_ATTACH, val } = ext {
                if let Ok(cert) = AuthCert::decode(val) {
                    let _ = self.authority.consider_cert(&cert, self.wall_unix_s);
                }
            }
        }
        // TLV 0x82: display-only hint, never trusted for admission (doc 18 §3).
        let scope_name_hint = bundle.exts.iter().find_map(|ext| match ext {
            Ext::Optional { typ, val } if *typ == TLV_SCOPE_INFO => Some(val.clone()),
            _ => None,
        });

        let subscribed = self.authority.is_subscribed(&bundle.dest);
        let origin_node = bundle.origin_node();

        // Doc 18 §5.2 elevation, one-shot: only evaluated for priorities
        // below NORMAL (declared ≥ NORMAL passes through unclamped already,
        // per `effective_class5_priority`'s own early return).
        let effective = if bundle.priority < prio::NORMAL {
            let grant = self.authority.grant_for(&bundle.origin, &bundle.dest, bundle.priority, self.wall_unix_s).copied();
            let now = self.now;
            let auth_buckets = &mut self.auth_buckets;
            effective_class5_priority(bundle.priority, subscribed, grant.as_ref(), |c| {
                auth_buckets.admit(&c.subject, c.priority_budget, now)
            })
        } else {
            bundle.priority
        };
        // Only elevation (declared kept) needs a meta update — ingest already
        // defaulted effective_priority to NORMAL for declared < NORMAL, which
        // is exactly the non-elevated outcome.
        if effective == bundle.priority && bundle.priority < prio::NORMAL {
            if let Some(mut meta) = self.store.meta(&bundle.id()) {
                meta.effective_priority = effective;
                self.store.update_meta(meta);
            }
        }

        // Doc 18 §5.2.1 item 2: rendering is a pure function of
        // (announcement, cert) with no anti-abuse state — independent of the
        // bucket and of declared priority.
        let official = self.authority.official(&bundle.origin, &bundle.dest, self.wall_unix_s);

        if subscribed {
            self.events.push_back(Event::Announcement {
                id: bundle.id(),
                scope: bundle.dest,
                ann_type: body.ann_type,
                not_after: body.not_after,
                text: body.text,
                from: origin_node,
                official,
                effective_priority: effective,
                scope_name_hint,
            });
        }
    }

    fn resolve_peer_key(&self, peer: &NodeId) -> Option<Key32> {
        for f in [field::EXCHANGE_KEY, field::PREKEY_BATCH, field::NICKNAME] {
            if let Some(c) = self.claims.get(peer, f) {
                return Some(c.owner);
            }
        }
        None
    }

    /// Build, sign and store a direct message; the mesh does the rest.
    fn send_direct(&mut self, peer: &NodeId, body: &[u8]) -> Result<Id16, SendError> {
        let id = self.send_app(peer, body[0], &body[1..])?;
        self.sent.insert(
            id,
            SentMessage { conv: conversation_id(&self.public, &self.resolve_peer_key(peer).expect("resolved in send_app")), state: DeliveryState::Queued },
        );
        self.events.push_back(Event::DeliveryProgress { id, state: DeliveryState::Queued });
        Ok(id)
    }

    /// The shared class-1 sending machinery (also used for sender-key dists).
    fn send_app(&mut self, peer: &NodeId, app: u8, payload: &[u8]) -> Result<Id16, SendError> {
        if self.claims.revocation(peer).is_some() {
            // Handbook §4.6: never open a new conversation, nor use a
            // prekey batch already on file, for a revoked identity — checked
            // up front so it also gates the X3DH first-contact path below.
            return Err(SendError::PeerRevoked);
        }
        let peer_pub = self.resolve_peer_key(peer).ok_or(SendError::UnknownPeer)?;
        let conv_id = conversation_id(&self.public, &peer_pub);

        if !self.convs.contains_key(&conv_id) {
            // First contact: X3DH against the peer's prekey batch (doc 08 §4.2).
            let batch_claim = self.claims.get(peer, field::PREKEY_BATCH).ok_or(SendError::UnknownPeer)?;
            let batch = PrekeyBatch::decode(&batch_claim.body).map_err(|_| SendError::UnknownPeer)?;
            let agreement = sender_agree(&mut self.rng, &self.identity, &peer_pub, &batch, Some(0))
                .map_err(|_| SendError::UnknownPeer)?;
            self.convs.insert(conv_id, Conv::new(conv_id, &self.public, &peer_pub, *agreement.ck0));
            self.pending_async.insert(conv_id, (agreement.eph_pub, agreement.prekey_id));
        }

        let conv = self.convs.get_mut(&conv_id).expect("just ensured");
        let (msg_key, epoch, index) = conv.send.next();
        let parents = conv.dag.heads();

        let mut body = Vec::with_capacity(1 + payload.len());
        body.push(app);
        body.extend_from_slice(payload);

        let dest = Id16(peer.0);
        let key = aead::committed_key(&msg_key, &msg_context(&self.public, &dest, &conv_id, epoch, index));
        let ct = aead::seal(&key, &conv_id.0, &body);

        let mut flags = FLAG_WANTS_RECEIPT;
        let wire_payload = if let Some((eph, prekey_id)) = self.pending_async.remove(&conv_id) {
            flags |= FLAG_ASYNC_KEYS;
            AsyncPayload::Prekey { prekey_id, sender_xpk: self.exchange, eph, ct }.encode()
        } else {
            DirectPayload { conv: conv_id, epoch, index, ct }.encode()
        };

        let mut exts = Vec::new();
        if !parents.is_empty() {
            flags |= FLAG_HAS_PARENTS;
            exts.push(Ext::Parents(parents));
        }

        let mut bundle = Bundle {
            class: class::DIRECT,
            flags,
            origin: self.public,
            dest,
            lifetime_ms: LIFETIME_MESSAGE_MS,
            priority: 1,
            exts,
            payload: wire_payload,
            signature: Sig64::ZERO,
        };
        bundle.signature = self.identity.sign(SigDomain::Bundle, &bundle.canonical_zeroed());
        if bundle.encode().len() > MAX_BUNDLE {
            return Err(SendError::TooLarge);
        }
        let parents_copy: Vec<Id16> = bundle.parents().to_vec();
        let budget = origin_budget(self.utility.n_hat(self.now));
        let id = self.store_own(bundle, budget);
        if let Some(c) = self.convs.get_mut(&conv_id) {
            c.dag.insert(id, &parents_copy);
        }
        Ok(id)
    }

    fn send_own_dist(&mut self, gid: &Id16, peer: &NodeId) -> Result<(), SendError> {
        let (dist_bytes, epoch) = {
            let gs = self.groups.get(gid).ok_or(SendError::UnknownPeer)?;
            (gs.local.dist().encode(), gs.epoch)
        };
        if !self.sent_dists.insert((*gid, *peer, epoch)) {
            return Ok(()); // already shipped for this epoch
        }
        let mut body = Vec::with_capacity(16 + dist_bytes.len());
        body.extend_from_slice(&self.node_id.0);
        body.extend_from_slice(&dist_bytes);
        self.send_app(peer, apptype::SENDER_KEY, &body).map(|_| ())
    }

    fn send_group(&mut self, gid: &Id16, text: &[u8]) -> Result<Id16, SendError> {
        let me = self.node_id;
        let gs = self.groups.get_mut(gid).ok_or(SendError::UnknownPeer)?;
        let epoch = gs.epoch;
        let mut body = Vec::with_capacity(1 + text.len());
        body.push(apptype::TEXT);
        body.extend_from_slice(text);
        let gp = gs.local.encrypt(me, &gid.0, &body);
        let mut bundle = Bundle {
            class: class::GROUP,
            flags: 0,
            origin: self.public,
            dest: *gid,
            lifetime_ms: LIFETIME_MESSAGE_MS,
            priority: 1,
            exts: vec![Ext::GroupEpoch(epoch)],
            payload: gp.encode(),
            signature: Sig64::ZERO,
        };
        bundle.signature = self.identity.sign(SigDomain::Bundle, &bundle.canonical_zeroed());
        if bundle.encode().len() > MAX_BUNDLE {
            return Err(SendError::TooLarge);
        }
        let budget = origin_budget(self.utility.n_hat(self.now));
        Ok(self.store_own(bundle, budget))
    }

    // ----------------------------------------------------------- delivery --

    fn deliver_direct(&mut self, bundle: &Bundle) {
        let origin = bundle.origin;
        let conv_id = conversation_id(&self.public, &origin);
        let plaintext = if bundle.flags & FLAG_ASYNC_KEYS != 0 {
            self.open_async(bundle, &conv_id, &origin)
        } else {
            self.open_direct(bundle, &conv_id)
        };
        let Some(body) = plaintext else { return };

        let missing = self
            .convs
            .get_mut(&conv_id)
            .map(|c| c.dag.insert(bundle.id(), bundle.parents()))
            .unwrap_or_default();
        if !missing.is_empty() {
            self.events.push_back(Event::MessageGap { conv: conv_id, parents: missing });
        }

        let (app, msg) =
            body.split_first().map(|(a, r)| (*a, r.to_vec())).unwrap_or((apptype::TEXT, Vec::new()));
        let from = node_id_of(&origin);

        if app == apptype::SENDER_KEY {
            self.handle_sender_key(&from, &msg);
        } else {
            // Doc 16 §3.1 / decision D8: manifest parsing happens IN
            // ADDITION to the existing MessageReceived — never in place of
            // it, so old and new consumers alike see the raw tagged body.
            if app == apptype::FILE_MANIFEST {
                self.try_offer_file(&conv_id, &from, &msg);
            }
            self.events.push_back(Event::MessageReceived {
                conv: conv_id,
                from,
                id: bundle.id(),
                body: msg,
                apptype: app,
            });
        }

        if bundle.flags & FLAG_WANTS_RECEIPT != 0 {
            self.emit_receipt(&conv_id, &bundle.id(), &origin);
        }
    }

    fn open_async(&mut self, bundle: &Bundle, conv_id: &Id16, origin: &Key32) -> Option<Vec<u8>> {
        let AsyncPayload::Prekey { prekey_id, sender_xpk, eph, ct } =
            AsyncPayload::decode(&bundle.payload).ok()?
        else {
            return None; // sealed-box form: not used by this node's sender path
        };
        if !self.convs.contains_key(conv_id) {
            let ck0 =
                recipient_agree(&self.prekeys, &self.identity, origin, &sender_xpk, &eph, prekey_id)
                    .ok()?;
            self.convs.insert(*conv_id, Conv::new(*conv_id, &self.public, origin, *ck0));
        } else {
            // Concurrent-initiation conflict: adopt the peer's root only if its
            // key is canonically smaller — both sides pick the same winner
            // (doc 08 §4.3's first-session rederive covers the rest).
            if origin.0 < self.public.0 {
                let ck0 = recipient_agree(
                    &self.prekeys,
                    &self.identity,
                    origin,
                    &sender_xpk,
                    &eph,
                    prekey_id,
                )
                .ok()?;
                self.convs.insert(*conv_id, Conv::new(*conv_id, &self.public, origin, *ck0));
            } else {
                return None;
            }
        }
        let conv = self.convs.get_mut(conv_id)?;
        let key = conv.recv.key_for(0, 0).ok()?;
        let bound = aead::committed_key(&key, &msg_context(origin, &self.node_id, conv_id, 0, 0));
        aead::open(&bound, &conv_id.0, &ct).ok()
    }

    fn open_direct(&mut self, bundle: &Bundle, conv_id: &Id16) -> Option<Vec<u8>> {
        let dp = DirectPayload::decode(&bundle.payload).ok()?;
        let conv = self.convs.get_mut(conv_id)?;
        let key = conv.recv.key_for(dp.epoch, dp.index).ok()?;
        let bound =
            aead::committed_key(&key, &msg_context(&bundle.origin, &self.node_id, conv_id, dp.epoch, dp.index));
        aead::open(&bound, &conv_id.0, &dp.ct).ok()
    }

    /// apptype 0x02: `member NodeID(16) ‖ SenderKeyDist`.
    fn handle_sender_key(&mut self, from: &NodeId, body: &[u8]) {
        if body.len() < 16 {
            return;
        }
        let mut member = [0u8; 16];
        member.copy_from_slice(&body[..16]);
        let member = Id16(member);
        let Ok(dist) = SenderKeyDist::decode(&body[16..]) else { return };
        let gid = dist.gid;
        let created = !self.groups.contains_key(&gid);
        let me = self.node_id;
        let gs = self.groups.entry(gid).or_insert_with(|| {
            // Invited: adopt the group; the inviter is admin (v1 immutable set).
            GroupState::create(&mut self.rng, gid, me, false)
        });
        gs.add_member(member);
        gs.learn_remote(member, &dist);
        if created {
            self.events.push_back(Event::GroupInvite { gid, from: *from });
        }
        // Reciprocate our own sender key so the sender can read us.
        let _ = self.send_own_dist(&gid, from);
        // Admin duty: forward the new member's dist to everyone else.
        let (is_admin, others) = {
            let gs = self.groups.get(&gid).expect("just inserted");
            (gs.is_admin, gs.members.members())
        };
        if is_admin {
            let mut fwd = Vec::with_capacity(16 + body.len() - 16);
            fwd.extend_from_slice(&member.0);
            fwd.extend_from_slice(&body[16..]);
            for m in others {
                if m != me && m != member && m != *from {
                    let _ = self.send_app(&m, apptype::SENDER_KEY, &fwd);
                }
            }
        }
    }

    fn deliver_group(&mut self, bundle: &Bundle) {
        let Ok(gp) = GroupPayload::decode(&bundle.payload) else { return };
        let Some(gs) = self.groups.get_mut(&gp.gid) else { return };
        let Some(remote) = gs.remotes.get_mut(&(gp.member, gp.epoch)) else { return };
        let Ok(body) = remote.decrypt(&gp, &gp.gid.0) else { return };
        let (_, msg) =
            body.split_first().map(|(a, r)| (*a, r.to_vec())).unwrap_or((apptype::TEXT, Vec::new()));
        self.events.push_back(Event::GroupMessage {
            gid: gp.gid,
            from: gp.member,
            id: bundle.id(),
            body: msg,
        });
    }

    fn emit_receipt(&mut self, conv_id: &Id16, delivered: &Id16, origin: &Key32) {
        let Some(conv) = self.convs.get(conv_id) else { return };
        let rkey = receipt::receipt_key(&conv.ck0);
        let mac = receipt::receipt_mac(&rkey, delivered);
        let payload = ReceiptPayload { delivered: *delivered, mac }.encode();
        let mut bundle = Bundle {
            class: class::CONTROL,
            flags: 0,
            origin: self.public,
            dest: node_id_of(origin),
            lifetime_ms: LIFETIME_CONTROL_MS,
            priority: 0,
            exts: Vec::new(),
            payload,
            signature: Sig64::ZERO,
        };
        bundle.signature = self.identity.sign(SigDomain::Bundle, &bundle.canonical_zeroed());
        self.store_own(bundle, L_RECEIPT);
    }

    /// Class-0 receipt handling for carriers AND the origin (doc 05 §7).
    fn handle_receipt(&mut self, bundle: &Bundle, to_self: bool) {
        let Ok(rp) = ReceiptPayload::decode(&bundle.payload) else { return };
        // Exact-receipt purge at any carrier — but only when the receipt's
        // origin IS the delivered bundle's destination (forgery gate).
        if let Some(m) = self.store.meta(&rp.delivered) {
            if m.dest == bundle.origin_node() {
                self.store.remove(&rp.delivered);
                // Handbook §4.5 unpin/GC: end-to-end receipt purge is the
                // normal way a friend's buffered copy stops being pinned.
                self.unpin_friend_buffer(&m.dest, &rp.delivered, m.size as u64);
                self.diag.log(rp.delivered, JourneyEvent::Purged, self.now);
            }
        }
        self.receipts.note_verified_receipt(&rp.delivered, self.now);
        self.diag.log(rp.delivered, JourneyEvent::ReceiptSeen, self.now);

        if !to_self {
            return;
        }
        // We are the origin: check the conversation MAC (proof of delivery).
        let Some(sent) = self.sent.get(&rp.delivered).copied() else { return };
        let Some(conv) = self.convs.get(&sent.conv) else { return };
        let rkey = receipt::receipt_key(&conv.ck0);
        if !receipt::mac_eq(&rp.mac, &receipt::receipt_mac(&rkey, &rp.delivered)) {
            return;
        }
        if let Some(m) = self.sent.get_mut(&rp.delivered) {
            m.state = DeliveryState::Delivered;
        }
        self.events.push_back(Event::Delivered { id: rp.delivered });
        self.events
            .push_back(Event::DeliveryProgress { id: rp.delivered, state: DeliveryState::Delivered });
    }

    /// Quota eviction (doc 04 §2). Bundles pinned by a live friendship
    /// (handbook §4.5 — buffered for a dependent, up to `FRIEND_QUOTA`) are
    /// excluded from eviction candidacy entirely; if that leaves nothing
    /// left to evict, this breaks rather than looping forever (a node can
    /// legitimately sit above `quota` while `friends.len() * FRIEND_QUOTA`
    /// bytes are pinned — an accepted MVP bound, handbook §4.5).
    fn enforce_quota(&mut self) {
        while self.store.total_bytes() > self.quota {
            let pinned: HashSet<Id16> =
                self.friends.values().flat_map(|s| s.buffered.iter().copied()).collect();
            let metas: Vec<BundleMeta> =
                self.store.all_meta().into_iter().filter(|m| !pinned.contains(&m.id)).collect();
            let order = eviction_order(&metas, |id| self.receipts.signal(id), self.now);
            let Some(victim) = order.first().copied() else { break };
            if !self.store.remove(&victim) {
                break;
            }
            self.deindex_symbol(&victim);
            self.diag.log(victim, JourneyEvent::Evicted, self.now);
        }
    }

    // ------------------------------------------------- friendship (§4.5) ----

    /// Drop `id` from `dependent`'s buffered set and refund its bytes from
    /// `quota_used` — the receipt-purge GC path (handbook §4.5 unpin/GC), a
    /// no-op if `dependent` isn't a live friend or never had `id` pinned.
    fn unpin_friend_buffer(&mut self, dependent: &NodeId, id: &Id16, size: u64) {
        if let Some(state) = self.friends.get_mut(dependent) {
            if state.buffered.remove(id) {
                state.quota_used = state.quota_used.saturating_sub(size);
            }
        }
    }

    /// Recompute `quota_used` for `dependent` from the bytes still on file
    /// for its buffered set — used wherever store-driven removal (expiry)
    /// hands us only an id, not the size that was removed with it.
    fn resync_friend_quota(&mut self, dependent: &NodeId) {
        let Some(state) = self.friends.get(dependent) else { return };
        let used: u64 =
            state.buffered.iter().filter_map(|id| self.store.meta(id)).map(|m| m.size as u64).sum();
        if let Some(state) = self.friends.get_mut(dependent) {
            state.quota_used = used;
        }
    }

    /// `store.expire()` already deleted `expired`'s bytes before handing us
    /// the ids — drop them from every friend's buffered set and resync that
    /// friend's `quota_used` from what's actually still on file.
    fn unpin_expired(&mut self, expired: &[Id16]) {
        if expired.is_empty() {
            return;
        }
        let affected: Vec<NodeId> = self
            .friends
            .iter()
            .filter(|(_, s)| expired.iter().any(|id| s.buffered.contains(id)))
            .map(|(n, _)| *n)
            .collect();
        for dependent in affected {
            if let Some(state) = self.friends.get_mut(&dependent) {
                for id in expired {
                    state.buffered.remove(id);
                }
            }
            self.resync_friend_quota(&dependent);
        }
    }

    /// TTL expiry for friendship relationships (doc 04 §4.4, handbook §4.5).
    /// A lapsed entry is simply dropped: on the patron side, its buffered ids
    /// fall out of `enforce_quota`'s pinned set immediately and revert to
    /// ordinary eviction candidacy — no separate unpin bookkeeping needed. On
    /// the dependent side, it just means we no longer believe we have that
    /// patron; renewal is a fresh offer/ack (handshake flow step 5).
    fn prune_expired_friendships(&mut self, now_ms: u64) {
        self.friends.retain(|_, s| now_ms.saturating_sub(s.since) < FRIENDSHIP_TTL_MS);
        self.patrons.retain(|_, since| now_ms.saturating_sub(*since) < FRIENDSHIP_TTL_MS);
    }

    /// Advisory spray-suppression signal (doc 06 §7 MVP): does `peer` look
    /// like it already buffers for `dest` — either a verified FRIENDSHIP
    /// claim (owner = dest, friend = peer) or a PROXY_SUMMARY hint we
    /// received from `peer` naming `dest`? Never authoritative — only nudges
    /// `load_for_handoff`'s utility (a false positive here wastes a spray
    /// copy, never causes a drop — the AUDIT F11 Hint/Purge shape).
    fn peer_buffers_for(&self, peer: &NodeId, dest: &NodeId) -> bool {
        if let Some(claim) = self.claims.get(dest, field::FRIENDSHIP) {
            if let Ok(f) = Friendship::decode(&claim.body) {
                if node_id_of(&f.friend) == *peer {
                    return true;
                }
            }
        }
        self.proxy_hints.get(peer).is_some_and(|deps| deps.contains(dest))
    }

    // ---------------------------------------------------- blob plane (§4.4) --

    /// Startup rebuild of `blob_index`/`symbol_of` from whatever class-4
    /// bundles the store already holds (decision D2) — mirrors how claims
    /// and groups restore from a persisted snapshot.
    fn rebuild_blob_index(&mut self) {
        for meta in self.store.all_meta() {
            if meta.class != class::SYMBOL {
                continue;
            }
            let Some((_, bytes)) = self.store.get(&meta.id) else { continue };
            let Ok(bundle) = Bundle::decode_canonical(&bytes) else { continue };
            self.index_symbol_bundle(&bundle, meta.id);
        }
    }

    /// Insert `id` into `blob_index`/`symbol_of` if its SYMBOL_INFO is
    /// well-formed and in-bounds (decision D1: `k/n/sym/block` sanity —
    /// out-of-bounds symbols are still stored/forwarded opaquely by the
    /// ordinary bundle pipeline, just not indexed here). Returns the derived
    /// `(root, sym_no)` on success.
    fn index_symbol_bundle(&mut self, bundle: &Bundle, id: Id16) -> Option<(Id16, u16)> {
        let (root, block, sym, k, n) = bundle.exts.iter().find_map(|e| match *e {
            Ext::SymbolInfo { root, block, sym, k, n } => Some((root, block, sym, k, n)),
            _ => None,
        })?;
        if k as usize != RS_DATA_K
            || n as usize != RS_TOTAL_N
            || sym as usize >= RS_TOTAL_N
            || block as usize >= FILE_MAX_STRIPES
        {
            return None;
        }
        let sym_no = block * RS_TOTAL_N as u16 + sym;
        self.blob_index.entry(root).or_default().insert(sym_no, id);
        self.symbol_of.insert(id, (root, sym_no));
        Some((root, sym_no))
    }

    /// Remove `id` from both blob indices — used on Merkle-verification
    /// failure and index prune (expiry/eviction).
    fn deindex_symbol(&mut self, id: &Id16) {
        if let Some((root, sym_no)) = self.symbol_of.remove(id) {
            if let Some(idx) = self.blob_index.get_mut(&root) {
                idx.remove(&sym_no);
                if idx.is_empty() {
                    self.blob_index.remove(&root);
                    // Decision D3 residual 2 GC rule: once an ABANDONED
                    // assembly's last servable symbol is gone, drop the
                    // assembly entry too.
                    if matches!(self.assemblies.get(&root).map(|a| a.state), Some(BlobState::Abandoned)) {
                        self.assemblies.remove(&root);
                    }
                }
            }
        }
    }

    /// `store.expire()`-driven prune (called from `Node::tick`, mirrors
    /// `unpin_expired`'s shape).
    fn prune_expired_symbols(&mut self, expired: &[Id16]) {
        for id in expired {
            self.deindex_symbol(id);
        }
    }

    /// TTL sweep for blob assemblies (doc 11 §6 "lifetime of interest",
    /// decision D3) — `Wanted`/`Fetching` past `BLOB_INTEREST_TTL_MS` become
    /// `Abandoned`; partial symbols stay servable via `blob_index`.
    fn sweep_blob_ttl(&mut self, now_ms: u64) {
        let mut failed = Vec::new();
        for (root, a) in self.assemblies.iter_mut() {
            if matches!(a.state, BlobState::Wanted | BlobState::Fetching)
                && now_ms.saturating_sub(a.started_at) > BLOB_INTEREST_TTL_MS
            {
                a.state = BlobState::Abandoned;
                failed.push(*root);
            }
        }
        for root in failed {
            self.events.push_back(Event::FileFailed { root, reason: FileFailReason::Abandoned });
        }
    }

    /// `Node::send_file`: RS-encode, Merkle-root, mint class-4 symbol
    /// bundles (dedup by content address, decision D6), then send the signed
    /// `FileManifest` as an apptype-0x01 class-1 message (decision D8).
    fn send_file(&mut self, peer: &NodeId, name: &str, mime: &str, data: &[u8]) -> Result<FileSend, SendError> {
        if data.len() as u64 > FILE_MAX {
            return Err(SendError::TooLarge);
        }
        if name.len() > NAME_MAX || mime.len() > MIME_MAX {
            return Err(SendError::TooLarge);
        }
        let stripes = blob::stripes_for_size(data.len() as u32);
        let mut padded = data.to_vec();
        padded.resize(stripes as usize * STRIPE_DATA_BYTES, 0);
        let full_hash = *blake3::hash(data).as_bytes();

        // Per-stripe systematic RS encode: symbols 0..k are the data itself.
        let mut all_symbols: Vec<Vec<u8>> = Vec::with_capacity(stripes as usize * RS_TOTAL_N);
        for stripe_bytes in padded.chunks(STRIPE_DATA_BYTES) {
            let data_syms: Vec<Vec<u8>> = stripe_bytes.chunks(mmp_types::consts::SYMBOL_SIZE).map(<[u8]>::to_vec).collect();
            all_symbols.extend(mmp_rs_code::encode(&data_syms, RS_TOTAL_N));
        }
        // Leaves = hash_block(symbol) for every symbol (data then parity
        // within each stripe), in global sym_no order (decision D1).
        let leaves: Vec<[u8; 32]> = all_symbols.iter().map(|s| hash_block(s)).collect();
        let root32 = merkle_root(&leaves);
        let mut root_bytes = [0u8; 16];
        root_bytes.copy_from_slice(&root32[..16]);
        let file_root = Id16(root_bytes);

        // Dedup by content address (decision D6): mint nothing if every
        // symbol for this root is already in our own index.
        let total_symbols = stripes as usize * RS_TOTAL_N;
        let already_complete =
            self.blob_index.get(&file_root).is_some_and(|m| m.len() >= total_symbols);
        if !already_complete {
            for (sym_no, sym) in all_symbols.iter().enumerate() {
                let block = (sym_no / RS_TOTAL_N) as u16;
                let sym_in_stripe = (sym_no % RS_TOTAL_N) as u16;
                let path = merkle_path(&leaves, sym_no).encode();
                let payload = SymbolPayload { path, symbol: sym.clone() }.encode();
                let mut bundle = Bundle {
                    class: class::SYMBOL,
                    flags: 0,
                    origin: self.public,
                    dest: Id16(peer.0),
                    lifetime_ms: LIFETIME_MESSAGE_MS,
                    priority: prio::BULK,
                    exts: vec![Ext::SymbolInfo {
                        root: file_root,
                        block,
                        sym: sym_in_stripe,
                        k: RS_DATA_K as u8,
                        n: RS_TOTAL_N as u8,
                    }],
                    payload,
                    signature: Sig64::ZERO,
                };
                bundle.signature = self.identity.sign(SigDomain::Bundle, &bundle.canonical_zeroed());
                debug_assert!(bundle.encode().len() <= MAX_BUNDLE, "symbol bundle sizing invariant");
                let budget = origin_budget(self.utility.n_hat(self.now));
                let id = self.store_own(bundle, budget);
                self.blob_index.entry(file_root).or_default().insert(sym_no as u16, id);
                self.symbol_of.insert(id, (file_root, sym_no as u16));
            }
        }

        let entry = FileEntry {
            name: name.as_bytes().to_vec(),
            mime: mime.as_bytes().to_vec(),
            size: data.len() as u32,
            file_root,
            rs_k: RS_DATA_K as u8,
            rs_n: RS_TOTAL_N as u8,
            full_hash,
            thumb_root: None,
        };
        let mut manifest =
            FileManifest { suite: 1, sharer: self.public, entries: vec![entry], sig: Sig64::ZERO };
        manifest.sig = self.identity.sign(SigDomain::Manifest, &manifest.signed_object());
        let message_id = self.send_app(peer, apptype::FILE_MANIFEST, &manifest.encode())?;
        Ok(FileSend { message_id, root: file_root })
    }

    /// Manifest hook (decision D8), called from `deliver_direct` on
    /// `apptype::FILE_MANIFEST`: parse + verify + cache; silent no-op on any
    /// failure (doc 16 §3.1 fail-safe — `MessageReceived` still fires from
    /// the caller either way).
    fn try_offer_file(&mut self, conv: &Id16, from: &NodeId, msg: &[u8]) {
        let Ok(manifest) = FileManifest::decode(msg) else { return };
        if mmp_crypto::verify(
            SigDomain::Manifest,
            &manifest.sharer,
            &manifest.signed_object(),
            &manifest.sig,
        )
        .is_err()
        {
            return;
        }
        let mut offers = Vec::with_capacity(manifest.entries.len());
        for e in &manifest.entries {
            if !self.offers.contains_key(&e.file_root) {
                if self.offer_order.len() >= FILE_OFFER_CACHE {
                    if let Some(evict) = self.offer_order.pop_front() {
                        self.offers.remove(&evict);
                    }
                }
                self.offer_order.push_back(e.file_root);
            }
            self.offers.insert(
                e.file_root,
                FileOfferInfo { size: e.size, full_hash: e.full_hash, sharer: manifest.sharer },
            );
            offers.push(FileOffer {
                name: e.name.clone(),
                mime: e.mime.clone(),
                size: e.size,
                root: e.file_root,
                thumb_root: e.thumb_root,
            });
        }
        self.events.push_back(Event::FileOffered {
            conv: *conv,
            from: *from,
            sharer: manifest.sharer,
            entries: offers,
        });
    }

    /// `Node::fetch_file`: register a `Wanted` assembly from a cached
    /// verified offer, absorb whatever symbols are already in our own
    /// `blob_index` (already-verified — reproven from the store, cheap at
    /// MVP scale), and try any stripe that's already complete. Repair beyond
    /// that rides `blob_frames`' BLOB_HAVE on the next session (decision D4).
    fn fetch_file(&mut self, root: &Id16, dest: &Path) -> Result<(), FetchError> {
        if let Some(a) = self.assemblies.get(root) {
            if matches!(a.state, BlobState::Wanted | BlobState::Fetching) {
                return Ok(()); // idempotent
            }
        }
        let Some(offer) = self.offers.get(root) else { return Err(FetchError::UnknownRoot) };
        let mut assembly =
            BlobAssembly::new(*root, offer.size, offer.full_hash, offer.sharer, dest.to_path_buf(), self.now);

        if let Some(idx) = self.blob_index.get(root).cloned() {
            for (sym_no, id) in idx {
                if self.symbol_verifies_for(root, sym_no, &id) {
                    assembly.set_have_bit(sym_no);
                }
            }
        }
        let stripes = assembly.stripes;
        self.assemblies.insert(*root, assembly);
        for s in 0..stripes {
            self.maybe_decode_stripe(root, s);
        }
        Ok(())
    }

    /// `Node::cancel_fetch`: → Abandoned; partials stay servable (decision D3/D8).
    fn cancel_fetch(&mut self, root: &Id16) {
        if let Some(a) = self.assemblies.get_mut(root) {
            if !matches!(a.state, BlobState::Assembled) {
                a.state = BlobState::Abandoned;
                self.events.push_back(Event::FileFailed { root: *root, reason: FileFailReason::Abandoned });
            }
        }
    }

    /// Re-verify a stored symbol bundle's Merkle path against `root`/`sym_no`
    /// (decision D1 checks a+b) — used both for absorbing already-stored
    /// symbols on `fetch_file` and for the live receive path.
    fn symbol_verifies_for(&self, root: &Id16, sym_no: u16, id: &Id16) -> bool {
        let Some((_, bytes)) = self.store.get(id) else { return false };
        let Ok(bundle) = Bundle::decode_canonical(&bytes) else { return false };
        symbol_payload_verifies(&bundle, root, sym_no)
    }

    /// Class-4 receive hook (`on_bundle`'s dispatch, decision D8): index (all
    /// nodes) then, if an active assembly matches, D1-verify, count, and try
    /// decoding the stripe. A Merkle-failing symbol is removed from the store
    /// and pruned from both indices (doc 11 §8 uniform verify-before-serve).
    fn handle_symbol(&mut self, bundle: &Bundle) {
        let id = bundle.id();
        let Some((root, sym_no)) = self.index_symbol_bundle(bundle, id) else { return };

        let Some(assembly) = self.assemblies.get(&root) else { return };
        if !matches!(assembly.state, BlobState::Wanted | BlobState::Fetching) || assembly.have_bit(sym_no) {
            return;
        }

        if !symbol_payload_verifies(bundle, &root, sym_no) {
            self.store.remove(&id);
            self.deindex_symbol(&id);
            if let Some(a) = self.assemblies.get_mut(&root) {
                a.bad_symbols += 1;
            }
            return;
        }

        let block = sym_no / RS_TOTAL_N as u16;
        if let Some(a) = self.assemblies.get_mut(&root) {
            a.set_have_bit(sym_no);
            if a.state == BlobState::Wanted {
                a.state = BlobState::Fetching;
            }
        }
        self.maybe_decode_stripe(&root, block);
    }

    /// Once a stripe has ≥`RS_DATA_K` verified symbols, pull them from the
    /// store, RS-decode, and write the stripe to `part` (decision D2 memory
    /// bound: one stripe, ≤64 KiB, ever held at once). Finalizes the
    /// assembly once every stripe is done.
    fn maybe_decode_stripe(&mut self, root: &Id16, stripe: u16) {
        let Some(assembly) = self.assemblies.get(root) else { return };
        if assembly.stripe_done_bit(stripe) || assembly.stripe_have_count(stripe) < RS_DATA_K {
            return;
        }
        let base = stripe as u32 * RS_TOTAL_N as u32;
        let mut pairs: Vec<(u8, Vec<u8>)> = Vec::with_capacity(RS_DATA_K);
        if let Some(idx) = self.blob_index.get(root) {
            for j in 0..RS_TOTAL_N as u32 {
                if pairs.len() >= RS_DATA_K {
                    break;
                }
                let sym_no = (base + j) as u16;
                if !assembly.have_bit(sym_no) {
                    continue;
                }
                let Some(bid) = idx.get(&sym_no) else { continue };
                let Some((_, bytes)) = self.store.get(bid) else { continue };
                let Ok(b) = Bundle::decode_canonical(&bytes) else { continue };
                let Ok(sp) = SymbolPayload::decode(&b.payload) else { continue };
                pairs.push((j as u8, sp.symbol));
            }
        }
        if pairs.len() < RS_DATA_K {
            return; // defensive: have-count said K but the store didn't yield K
        }
        let Some(data_syms) = mmp_rs_code::decode(&pairs, RS_DATA_K) else { return };
        let mut concat = Vec::with_capacity(STRIPE_DATA_BYTES);
        for s in &data_syms {
            concat.extend_from_slice(s);
        }
        let Some(assembly) = self.assemblies.get_mut(root) else { return };
        if assembly.write_stripe(stripe, &concat).is_err() {
            return; // disk error: try again next time a symbol nudges this stripe
        }
        assembly.set_stripe_done(stripe);
        if assembly.all_stripes_done() {
            self.finalize_assembly(root);
        }
    }

    /// Truncate/hash/compare, then rename-or-remove the `.part` file
    /// (decision D2 finalize) and emit the terminal event.
    fn finalize_assembly(&mut self, root: &Id16) {
        let Some(assembly) = self.assemblies.get_mut(root) else { return };
        let dest_path = assembly.dest_path.clone();
        let part_path = assembly.part_path();
        match assembly.finalize() {
            Ok(true) => {
                let renamed = std::fs::rename(&part_path, &dest_path).is_ok();
                if let Some(a) = self.assemblies.get_mut(root) {
                    a.state = if renamed { BlobState::Assembled } else { BlobState::Abandoned };
                }
                if renamed {
                    self.events.push_back(Event::FileReady { root: *root, path: dest_path });
                } else {
                    self.events
                        .push_back(Event::FileFailed { root: *root, reason: FileFailReason::Abandoned });
                }
            }
            Ok(false) => {
                let _ = std::fs::remove_file(&part_path);
                if let Some(a) = self.assemblies.get_mut(root) {
                    a.state = BlobState::Abandoned;
                }
                self.events
                    .push_back(Event::FileFailed { root: *root, reason: FileFailReason::HashMismatch });
            }
            Err(_) => {
                if let Some(a) = self.assemblies.get_mut(root) {
                    a.state = BlobState::Abandoned;
                }
                self.events
                    .push_back(Event::FileFailed { root: *root, reason: FileFailReason::Abandoned });
            }
        }
    }

    /// `Node::file_progress` snapshot.
    fn file_progress(&self, root: &Id16) -> Option<FileProgress> {
        self.assemblies.get(root).map(|a| FileProgress {
            state: a.state,
            have_symbols: (0..a.stripes).map(|s| a.stripe_have_count(s) as u32).sum(),
            total_symbols: a.stripes as u32 * RS_TOTAL_N as u32,
            stripes_done: a.done_count,
            stripes: a.stripes,
        })
    }

    /// Decision D4 responder logic, shared by `on_blob_have`/`on_blob_req`:
    /// `serve = { sym ∈ blob_index[root] : wanted(sym) }`, ordered by
    /// ascending `sym_no`, capped at the remaining per-contact serve budget.
    fn blob_serve_candidates(
        &mut self,
        peer: &NodeId,
        root: &Id16,
        block: u16,
        bitmap: &[u8],
        wants_set_bits: bool,
    ) -> Vec<Id16> {
        if !bitmap.len().is_multiple_of(5) {
            return Vec::new(); // decision D4: malformed bitmap length, ignore
        }
        let served = self.blob_served.get(peer).copied().unwrap_or(0);
        let remaining = BLOB_SERVE_MAX_PER_CONTACT.saturating_sub(served);
        if remaining == 0 {
            return Vec::new();
        }
        let base = u32::from(block) * RS_TOTAL_N as u32;
        let covered = (bitmap.len() / 5) as u32 * RS_TOTAL_N as u32;
        let lo = u16::try_from(base).unwrap_or(u16::MAX);
        let mut out = Vec::new();
        if let Some(idx) = self.blob_index.get(root) {
            for (&sym_no, id) in idx.range(lo..) {
                let sym_no_u32 = u32::from(sym_no);
                if sym_no_u32 < base {
                    continue;
                }
                let rel = sym_no_u32 - base;
                if rel >= covered {
                    break; // sorted by sym_no; nothing further is covered
                }
                let byte = (rel / 8) as usize;
                let bit_set = bitmap.get(byte).is_some_and(|b| b & (1 << (rel % 8)) != 0);
                if bit_set == wants_set_bits {
                    out.push(*id);
                    if out.len() as u32 >= remaining {
                        break;
                    }
                }
            }
        }
        self.blob_served.insert(*peer, served + out.len() as u32);
        out
    }
}

/// D1 symbol verification (checks a+b): the Merkle path must connect the
/// symbol's hash to the SYMBOL_INFO root's 16-byte prefix, AND the path's own
/// proven leaf index must equal the SYMBOL_INFO-derived `sym_no` — closing the
/// forged-position replay hole a membership-only check would leave open.
fn symbol_payload_verifies(bundle: &Bundle, root: &Id16, sym_no: u16) -> bool {
    let Ok(sp) = SymbolPayload::decode(&bundle.payload) else { return false };
    let Ok(path) = MerklePath::decode(&sp.path) else { return false };
    let leaf = hash_block(&sp.symbol);
    match compute_root(&leaf, &path) {
        Ok(computed) => computed[..16] == root.0 && path.leaf_index() == u64::from(sym_no),
        Err(_) => false,
    }
}

// ------------------------------------------------------- SessionHost --------

impl SessionHost for NodeCore {
    type Ids = SyncView;

    fn ids(&mut self, peer: &NodeId) -> &mut SyncView {
        let mut entries: Vec<(Id16, u8)> = self
            .store
            .all_meta()
            .into_iter()
            .filter(|m| m.remaining_ms(self.now) >= OFFER_MIN_LIFETIME_MS)
            .map(|m| {
                // Domain partition (doc 06 §2) — exactly one domain per bundle.
                let domain = match m.class {
                    0 | 3 => 0,
                    1 if m.dest == *peer || m.dest == self.node_id => 1,
                    2 => 2,
                    _ => 3,
                };
                (m.id, domain)
            })
            .collect();
        entries.sort();
        self.sync_view = SyncView { entries };
        &mut self.sync_view
    }

    fn summary(&mut self, peer: &NodeId) -> Summary {
        let me = self.node_id;
        let digest: Vec<ClaimDigestEntry> = self.claims.digest_for(&[me, *peer]);
        Summary {
            receipt_buckets: self.receipts.summary_buckets(2),
            claim_digest: digest,
            group_hints: Vec::new(),
            want_heads: Vec::new(),
        }
    }

    fn on_summary(&mut self, _peer: &NodeId, summary: Summary) {
        // Session is authenticated by construction here (doc 06 §6.3): fold
        // the peer's receipt buckets as suppression hints, never purge.
        for b in summary.receipt_buckets {
            self.receipts.union_from_peer(b.bucket_epoch, &b.bloom, self.now);
        }
    }

    fn load_for_handoff(&mut self, peer: &NodeId, id: &Id16) -> Option<(Vec<u8>, u8)> {
        let (mut meta, mut bytes) = self.store.get(id)?;
        let peer_is_dest = meta.dest == *peer;
        let u_self = self.utility.utility(&meta.dest, self.now);
        let mut u_peer: f32 = if peer_is_dest { 1.0 } else { 0.0 };
        // Doc 06 §7 MVP proxy-summary hint: prefer a peer that already
        // buffers for this bundle's destination (advisory only, AUDIT F11
        // Hint/Purge shape — never causes a drop, only a routing nudge).
        if !peer_is_dest && self.peer_buffers_for(peer, &meta.dest) {
            u_peer = u_peer.max(0.9);
        }
        let decision = handoff_decision(
            &meta,
            peer,
            u_self,
            u_peer,
            peer_is_dest,
            meta.origin == *peer,
            self.receipts.signal(id),
            self.now,
        );
        let grant = match decision {
            Handoff::Deliver => meta.budget.max(1),
            Handoff::Spray { grant, keep } => {
                meta.budget = keep;
                grant.max(1)
            }
            Handoff::Focus => {
                meta.budget = 0;
                1
            }
            Handoff::Keep => return None,
        };
        // Decrement lifetime by holding duration before hand-off (doc 03 §4.3)
        // and persist intent BEFORE the frame leaves (I3: under-spray on crash).
        let remaining = meta.remaining_ms(self.now);
        bytes[LIFETIME_OFFSET..LIFETIME_OFFSET + 5].copy_from_slice(&remaining.to_le_bytes()[..5]);
        meta.slot = SlotState::HandoffPending;
        self.store.update_meta(meta);
        self.diag.log(*id, JourneyEvent::HandedOff { peer: *peer, budget: grant }, self.now);
        if meta.own_origin && self.sprayed.insert(*id) {
            if let Some(m) = self.sent.get_mut(id) {
                if m.state == DeliveryState::Queued {
                    m.state = DeliveryState::InMesh;
                    self.events
                        .push_back(Event::DeliveryProgress { id: *id, state: DeliveryState::InMesh });
                }
            }
        }
        Some((bytes, grant))
    }

    fn on_bundle(&mut self, peer: &NodeId, bytes: &[u8]) {
        let now = self.now;
        let node_id = self.node_id;
        let size = bytes.len() as u64;
        let (disposition, bundle) = {
            let buckets = &mut self.buckets;
            let blocklist = &self.blocklist;
            let utility = &self.utility;
            let friends = &self.friends;
            let assemblies = &self.assemblies;
            let mut admission = |b: &Bundle, origin: &NodeId| {
                if blocklist.contains(origin) {
                    return false;
                }
                // Friend-buffering bypass (handbook §4.5 item 2): a live
                // patron relationship admits class-1 bundles for its
                // dependent without charging the origin bucket at all,
                // bounded by FRIEND_QUOTA instead. Over quota (or expired)
                // falls through to the ordinary charged path below.
                if b.class == class::DIRECT {
                    if let Some(state) = friends.get(&b.dest) {
                        if state.has_room(now, size) {
                            return true;
                        }
                    }
                }
                // Requester-side solicited bypass (decision D5): a class-4
                // symbol we're actively fetching (Wanted|Fetching) and don't
                // already hold is admitted without charging the origin
                // bucket — bounded, solicited traffic; duplicates die at
                // SeenIndex, junk dies at Merkle verify (`handle_symbol`) and
                // is removed. Relays get NO such exemption (D5): this bypass
                // only ever fires on the requester's own node, where an
                // assembly actually exists.
                if b.class == class::SYMBOL {
                    let info = b.exts.iter().find_map(|e| match *e {
                        Ext::SymbolInfo { root, block, sym, k, n } => Some((root, block, sym, k, n)),
                        _ => None,
                    });
                    if let Some((root, block, sym, k, n)) = info {
                        if k as usize == RS_DATA_K
                            && n as usize == RS_TOTAL_N
                            && (sym as usize) < RS_TOTAL_N
                            && (block as usize) < FILE_MAX_STRIPES
                        {
                            let sym_no = block * RS_TOTAL_N as u16 + sym;
                            if let Some(a) = assemblies.get(&root) {
                                if matches!(a.state, BlobState::Wanted | BlobState::Fetching)
                                    && !a.have_bit(sym_no)
                                {
                                    return true;
                                }
                            }
                        }
                    }
                }
                let wot = utility.contact(origin).map(|s| s.verified).unwrap_or(false);
                buckets.admit(origin, (b.payload.len() + 124) as u32, now, wot)
            };
            ingest(bytes, now, &node_id, &mut self.seen, &mut *self.store, &mut admission)
        };
        let _ = peer;
        let (to_self, bundle) = match (disposition, bundle) {
            (Disposition::Stored { to_self }, Some(b)) => (to_self, b),
            (Disposition::Dropped(reason), _) => {
                self.diag.drops += 1;
                let _ = reason;
                return;
            }
            _ => return,
        };
        self.diag.log(bundle.id(), JourneyEvent::Stored, now);
        // Friend-buffer pin bookkeeping (handbook §4.5 item 2), post-store:
        // re-checks the SAME predicate the admission bypass used above, so
        // only bundles that actually rode the friend-quota path get pinned —
        // one admitted via the ordinary charged path (quota exhausted or
        // friendship expired) is left fully eviction-eligible.
        if bundle.class == class::DIRECT {
            if let Some(state) = self.friends.get_mut(&bundle.dest) {
                if state.has_room(now, size) {
                    state.buffered.insert(bundle.id());
                    state.quota_used += size;
                }
            }
        }
        match bundle.class {
            class::CONTROL => self.handle_receipt(&bundle, to_self),
            class::DIRECT if to_self => self.deliver_direct(&bundle),
            class::GROUP => self.deliver_group(&bundle),
            class::CLAIM => {
                if let Ok(claim) = Claim::decode(&bundle.payload) {
                    if mmp_crypto::verify(
                        SigDomain::Claim,
                        &claim.owner,
                        &claim.signed_object(),
                        &claim.sig,
                    )
                    .is_ok()
                    {
                        self.learn_claim(claim, false);
                    }
                }
            }
            class::ANNOUNCEMENT => self.handle_announcement(&bundle),
            class::SYMBOL => self.handle_symbol(&bundle),
            _ => {} // unknown classes: store-and-forward only (doc 03 §8)
        }
    }

    fn on_budget(&mut self, _peer: &NodeId, id: &Id16, granted: u8) {
        // Engine already clamped to L_MAX (AUDIT F3); adopt for our copy.
        if let Some(mut meta) = self.store.meta(id) {
            if !meta.own_origin {
                meta.budget = granted;
                self.store.update_meta(meta);
            }
        }
    }

    fn wants(&mut self, _peer: &NodeId) -> Vec<Id16> {
        let mut out: Vec<Id16> = Vec::new();
        for conv in self.convs.values() {
            out.extend(conv.dag.gaps().copied());
            if out.len() >= 64 {
                break;
            }
        }
        out.truncate(64);
        out
    }

    fn accept_peer(&mut self, identity: &Key32, node: &NodeId) -> bool {
        if self.blocklist.contains(node) {
            return false; // silent deny (doc 07 §6.5)
        }
        if self.claims.revocation(node).is_some() {
            // Handbook §4.6: refuse new sessions from (or to) a revoked
            // identity — same silent-deny shape as the blocklist above.
            // `accept_peer` runs symmetrically on both sides of a session
            // (doc 11 §3's Binding phase, both ID_PROOFs), so this covers
            // inbound and outbound alike.
            return false;
        }
        // Handbook §4.5: the host's own NodeId → identity map, learned here
        // so the friendship handshake (keyed on NodeId at the SessionHost
        // seam) can resolve the peer's Ed25519 key when it needs to sign or
        // verify a countersig.
        self.known_identities.insert(*node, *identity);
        // Decision D4/D5: per-contact blob serve budget resets on every new
        // contact with this peer.
        self.blob_served.remove(node);
        true
    }

    fn on_friend_offer(&mut self, peer: &NodeId, body: &[u8]) -> Option<Vec<u8>> {
        // No auto-accept (doc 04 §4 security note): silently ignore an offer
        // from anyone not explicitly allowlisted.
        if !self.friend_allowlist.contains(peer) {
            return None;
        }
        let offer = FriendOfferBody::decode(body).ok()?;
        let dependent_key = *self.known_identities.get(peer)?;
        let preimage = friendship_signed_object(&dependent_key, &self.public, offer.since);
        let countersig = self.identity.sign(SigDomain::Friend, &preimage);
        let state = self.friends.entry(*peer).or_insert_with(|| FriendState {
            since: offer.since,
            quota_used: 0,
            buffered: BTreeSet::new(),
        });
        state.since = offer.since; // renewal: refresh TTL, keep existing pins
        self.events.push_back(Event::FriendshipEstablished { peer: *peer, since: offer.since });
        Some(FriendAckBody { countersig }.encode())
    }

    fn on_friend_ack(&mut self, peer: &NodeId, body: &[u8]) {
        // Only meaningful if we actually have an outstanding offer to this
        // peer — `offered_since` also supplies the `since` value the ack's
        // countersig was made over (the ack body itself only carries the
        // signature, doc 04 §4.1 wire shape).
        let Some(&since) = self.offered_since.get(peer) else { return };
        let Some(&patron_key) = self.known_identities.get(peer) else { return };
        let Ok(ack) = FriendAckBody::decode(body) else { return };
        let preimage = friendship_signed_object(&self.public, &patron_key, since);
        if mmp_crypto::verify(SigDomain::Friend, &patron_key, &preimage, &ack.countersig).is_err() {
            return; // invalid countersig → ignore (handbook §4.5 step 4)
        }
        self.patrons.insert(*peer, since);
        self.pending_offers.remove(peer);
        self.offered_since.remove(peer);
        let seq = self.next_friendship_seq;
        self.next_friendship_seq += 1;
        let body = Friendship { friend: patron_key, since, countersig: ack.countersig }.encode();
        self.publish_claim_now(field::FRIENDSHIP, seq, body);
        self.events.push_back(Event::FriendshipEstablished { peer: *peer, since });
    }

    fn on_proxy_summary(&mut self, peer: &NodeId, body: &[u8]) {
        let Ok(summary) = ProxySummaryBody::decode(body) else { return };
        self.proxy_hints.entry(*peer).or_default().insert(summary.dependent);
    }

    fn friend_frames(&mut self, peer: &NodeId) -> Vec<Frame> {
        let mut out = Vec::new();
        // Dependent side: solicit a patron we've asked for, once per contact.
        if self.pending_offers.contains(peer) && !self.patrons.contains_key(peer) {
            let since = self.now;
            self.offered_since.insert(*peer, since);
            out.push(Frame::FriendOffer(FriendOfferBody { since }.encode()));
        }
        // Patron side: advertise buffered dependents to every peer we sync
        // with (spray-suppression hint, doc 06 §7 MVP — a full poll-schedule
        // is deferred, see the handbook §4.5 shipped note).
        for (dependent, state) in &self.friends {
            if state.buffered.is_empty() {
                continue;
            }
            let mut bloom = Bloom::with_capacity(state.buffered.len());
            for id in &state.buffered {
                bloom.insert(id, &INTERNAL_SALT);
            }
            let wire_body =
                ProxySummaryBody { dependent: *dependent, bloom: bloom.folded(BLOOM_SUMMARY_MAX) }
                    .encode();
            out.push(Frame::ProxySummary(wire_body));
        }
        out
    }

    fn on_blob_have(&mut self, peer: &NodeId, root: &Id16, block: u16, bitmap: &[u8]) -> Vec<Id16> {
        self.blob_serve_candidates(peer, root, block, bitmap, false)
    }

    fn on_blob_req(&mut self, peer: &NodeId, root: &Id16, block: u16, bitmap: &[u8]) -> Vec<Id16> {
        self.blob_serve_candidates(peer, root, block, bitmap, true)
    }

    /// MVP requesters emit only BLOB_HAVE, one per active (`Wanted`/`Fetching`)
    /// assembly — the requester's in-RAM `have` vec is byte-identical to the
    /// wire bitmap form (decision D4), so this is a straight copy. BLOB_REQ
    /// send-side is receive-only in this MVP (decision D4 residual).
    fn blob_frames(&mut self, peer: &NodeId) -> Vec<Frame> {
        let _ = peer;
        self.assemblies
            .values()
            .filter(|a| matches!(a.state, BlobState::Wanted | BlobState::Fetching))
            .map(|a| Frame::BlobHave { root: a.root, block: 0, bitmap: a.have.clone() })
            .collect()
    }
}

#[cfg(test)]
mod tests;
