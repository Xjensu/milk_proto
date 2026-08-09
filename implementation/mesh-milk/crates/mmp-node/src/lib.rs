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

#![warn(missing_docs)]

pub mod authority_bucket;
mod blob;
mod conv;
pub mod diag;
mod group;
mod secret_state;
mod vault;

pub use blob::BlobState;

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use rand::rngs::StdRng;
use rand::SeedableRng;
use zeroize::Zeroizing;

use mmp_core::authority::{effective_class5_priority, AuthorityStore};
use mmp_core::bloom::{Bloom, INTERNAL_SALT};
use mmp_core::ingest::{ingest, Disposition};
use mmp_core::store::{BundleMeta, BundleStore, SlotState};
use mmp_core::{
    ClaimStore, IdSource, ReceiptFilter, SeenIndex, SessionEngine, SessionEvent, SessionHost,
};
use mmp_crypto::group::SenderKeyDist;
use mmp_crypto::merkle::{compute_root, hash_block, merkle_path, merkle_root, MerklePath};
use mmp_crypto::prekey::{recipient_agree, sealed_ck0, sender_agree, PrekeyStore, PREKEY_NONE};
use mmp_crypto::{aead, conversation_id, ephem_id, receipt, Identity};
use mmp_crypto::sealed::{open_box, seal_box_with_root};
use mmp_router::{
    eviction_order, handoff_decision, origin_budget, Handoff, OriginBuckets, UtilityState,
};
use mmp_types::authcert::{scope_id_of, AnnouncementBody, AuthCert, RevocationList};
use mmp_types::bundle::{
    class, prio, Bundle, FLAG_ASYNC_KEYS, FLAG_HAS_PARENTS, FLAG_WANTS_RECEIPT, LIFETIME_OFFSET,
    SIG_LEN,
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
    FT_CLOSE, FT_ERROR, MAX_ENVELOPE_PAYLOAD,
};
use mmp_types::ids::{node_id_of, Id16, Key32, NodeId, Sig64};
use mmp_types::payload::{apptype, AsyncPayload, DirectPayload, GroupPayload, ReceiptPayload, SymbolPayload};
use mmp_types::tlv::{TLV_AUTHCERT_ATTACH, TLV_SCOPE_INFO};
use mmp_types::{Beacon, Ext, FileEntry, FileManifest};

use authority_bucket::AuthorityBuckets;
use blob::{BlobAssembly, FileOfferInfo};
use conv::{msg_context, Conv};
use diag::{DiagLog, JourneyEvent};
use group::{GroupState, MAX_GROUPS};
use mmp_tal::{LinkId, TalEvent};

/// Well-known scope for claim gossip bundles (dest is routing-opaque).
const CLAIM_SCOPE: Id16 = Id16([0xff; 16]);

/// Compute the complete encoded bundle length without calling `Bundle::encode`.
/// The latter has a debug assertion at the frozen MAX_BUNDLE boundary, so
/// admission checks must be able to reject an oversized candidate first.
fn bundle_wire_len(bundle: &Bundle) -> usize {
    bundle.canonical_zeroed().len() + SIG_LEN
}

/// The core grants eight data-frame credits; reserve four control frames for
/// handshake/credit/close traffic when sizing the host-side queue.
const OUTBOX_DATA_CREDIT_WINDOW: usize = 8;
const OUTBOX_CONTROL_RESERVE: usize = 4;
const OUTBOX_FRAME_BYTES: usize = MAX_ENVELOPE_PAYLOAD + 3;
/// Bound transport-local close actions so denied links cannot grow node RAM.
const CLOSE_ACTION_CAPACITY: usize = 64;
/// Bound the application callback FIFO so a peer cannot retain an attacker-
/// sized backlog of ciphertext-free `Event` payloads when the host stops
/// polling.  This is a local backpressure policy, not a wire limit.
const EVENT_QUEUE_CAPACITY: usize = 2048;
/// Bound advisory proxy-hint dependents retained per authenticated peer.
/// Hints only suppress an optional spray copy, so dropping a new hint under
/// local pressure is wire-neutral and cannot affect correctness.
const MAX_PROXY_HINTS_PER_PEER: usize = 256;
/// Bound the number of authenticated peers whose advisory proxy hints are
/// retained. Eviction is intentionally arbitrary because hints only affect
/// spray ordering, never admission or delivery correctness.
const MAX_PROXY_HINT_PEERS: usize = 256;
/// Bound per-contact blob-serving counters retained for distinct peers. A
/// peer beyond this local policy cap is served nothing until an existing
/// counter entry is released by policy.
const MAX_BLOB_SERVE_PEERS: usize = 256;
/// Local persistence cap for explicit user blocklist entries. This is a host
/// policy bound, not a wire limit.
const BLOCKLIST_SNAPSHOT_VERSION: u8 = 1;
const MAX_BLOCKLIST_ENTRIES: usize = 1024;
/// Bound receiver-side pairwise conversation state admitted from authenticated
/// contacts. A Sybil can otherwise send one valid async first-contact bundle
/// per identity and leave a permanent `Conv` plus ratchet material in RAM;
/// there is no protocol TTL for a conversation. This is a local admission
/// policy only and does not alter any wire object or ratchet format.
const MAX_CONVERSATION_STATE: usize = 256;
/// Runtime bound for local delivery-history entries. This matches the
/// secret-state snapshot limit; terminal history is evictable, while queued
/// and in-mesh messages remain protected until they expire or are delivered.
const MAX_SENT_RUNTIME: usize = 4096;

/// Node runtime tier (doc 04 §3), simplified ladder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    /// Lowest tier: a leaf/sensor node, battery- and storage-constrained.
    Leaf = 1,
    /// Middle tier: relays traffic for others in addition to its own.
    Relay = 2,
    /// Top tier: quota-privileged (`QUOTA_ANCHOR`) — mains-powered or
    /// otherwise storage-rich.
    Anchor = 3,
}

/// Delivery status, mapped 1:1 to protocol facts (HANDOFF_MOBILE §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryState {
    /// Sent locally, not yet handed off to any peer.
    Queued,
    /// Handed off to at least one peer during a session (`load_for_handoff`).
    InMesh,
    /// A verified delivery receipt was seen for it (`handle_receipt`).
    Delivered,
    /// Its `lifetime_ms` lapsed before a receipt was seen.
    Expired,
}

/// The event stream — the spine of any app built on this node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// A direct or async-first-contact message decrypted and verified
    /// successfully (`NodeCore::deliver_direct`).
    MessageReceived {
        /// The conversation it arrived on.
        conv: Id16,
        /// The sender's `NodeId`.
        from: NodeId,
        /// The message's BundleID.
        id: Id16,
        /// The decrypted, apptype-tagged body's payload (tag stripped).
        body: Vec<u8>,
        /// The apptype tag (`mmp_types::payload::apptype`) that prefixed `body`.
        apptype: u8,
    },
    /// Unknown parents → gap markers; repair flows via WANT (doc 06 §5.3).
    MessageGap {
        /// The conversation with the gap.
        conv: Id16,
        /// The missing parent BundleIDs (`wants`/repair candidates).
        parents: Vec<Id16>,
    },
    /// A verified delivery receipt proved the message with this id reached
    /// its destination (`NodeCore::handle_receipt`).
    Delivered {
        /// The delivered message's BundleID.
        id: Id16,
    },
    /// A message's `DeliveryState` changed (queued → in-mesh → delivered, or
    /// → expired).
    DeliveryProgress {
        /// The message whose state changed.
        id: Id16,
        /// The state it moved to.
        state: DeliveryState,
    },
    /// A peer's claim (profile field) was adopted (`NodeCore::learn_claim`).
    ClaimUpdated {
        /// The claim's owner.
        owner: NodeId,
        /// The claim field id that changed (`mmp_types::claim::field`).
        field: u8,
    },
    /// Equal-seq conflicting claims — normative UX duty (doc 07 §6.4).
    CompromiseWarning {
        /// The owner whose claims are in conflict.
        owner: NodeId,
    },
    /// A valid identity revocation (claim field `0x09`, handbook §4.6)
    /// arrived for `peer`. `successor` is `Some` only when the succession
    /// counter-signature verified (`ClaimStore::successor`) — this is the
    /// hook for the app to OFFER "trust successor," never a trigger to
    /// migrate a petname automatically (petname model: user confirmation
    /// only). Sessions with `peer` are already refused and its prekey
    /// claims already unusable by the time this fires — see `accept_peer`
    /// and `send_app`.
    PeerRevoked {
        /// The revoked identity's `NodeId`.
        peer: NodeId,
        /// The successor key, once its counter-signature has verified — see
        /// the variant doc above for what `Some`/`None` mean here.
        successor: Option<Key32>,
    },
    /// A friendship (handbook §4.5, claim field `0x05`) was established or
    /// renewed with `peer` — fires on BOTH sides: the patron (after
    /// countersigning an offer) and the dependent (after verifying the
    /// resulting FRIEND_ACK). `since` is the relationship's start time, used
    /// for the `FRIENDSHIP_TTL_MS` expiry check.
    FriendshipEstablished {
        /// The patron/dependent on the other side of the relationship.
        peer: NodeId,
        /// The relationship's start/renewal timestamp.
        since: u64,
    },
    /// A session with `peer` was established (also implies contact, doc 04's
    /// utility-tracking sense) — fired alongside `Event::Established`.
    ContactSeen {
        /// The peer just contacted.
        peer: NodeId,
    },
    /// A session with `peer` reached the Established phase (`pump_link`).
    Established {
        /// The peer the session was established with.
        peer: NodeId,
    },
    /// We were invited into a group and adopted it (`NodeCore::handle_sender_key`).
    GroupInvite {
        /// The group's id.
        gid: Id16,
        /// Who sent the sender-key dist that triggered the invite.
        from: NodeId,
    },
    /// A group message decrypted successfully (`NodeCore::deliver_group`).
    GroupMessage {
        /// The group's id.
        gid: Id16,
        /// The sending member.
        from: NodeId,
        /// The message's BundleID.
        id: Id16,
        /// The decrypted body (apptype tag stripped).
        body: Vec<u8>,
    },
    /// Removal takes effect as the network reconverges (doc 06 §8.6).
    GroupEpochChanged {
        /// The group whose epoch changed.
        gid: Id16,
        /// The new epoch number.
        epoch: u16,
    },
    /// `Node::set_tier` moved the node to a new runtime tier.
    TierChanged {
        /// The tier now in effect.
        tier: Tier,
    },
    /// Doc 18: a class-5 bundle for a subscribed scope. `official` is the
    /// rendering predicate (doc 18 §5.2.1 item 2, independent of priority);
    /// `effective_priority` is the receiver-local admission clamp (doc 18
    /// §5.2) — apps should use it for notification prominence, never
    /// `official` alone (an unauthenticated sender can still be `official ==
    /// false` yet still get through at declared priority if never clamped,
    /// i.e. declared ≥ NORMAL already).
    Announcement {
        /// The announcement bundle's id.
        id: Id16,
        /// The scope it was published to.
        scope: Id16,
        /// Application-defined announcement type.
        ann_type: u8,
        /// Display-validity cutoff (unix seconds); no policy enforces this
        /// yet, see the `⚠` note at its construction in `publish_announcement`.
        not_after: u32,
        /// The announcement's plaintext body.
        text: Vec<u8>,
        /// The publishing node.
        from: NodeId,
        /// The rendering predicate — see the variant doc above; independent
        /// of `effective_priority`.
        official: bool,
        /// The receiver-local admission-clamped priority — use this for
        /// notification prominence, never `official` alone.
        effective_priority: u8,
        /// TLV 0x82 SCOPE_INFO, display-only, never trusted (doc 18 §3).
        scope_name_hint: Option<Vec<u8>>,
    },
    /// A `FileManifest` (doc 16 §2, apptype 0x01) decrypted from a class 1/2
    /// body parsed and verified successfully — fired IN ADDITION to the
    /// ordinary `MessageReceived` (existing consumers unaffected). Any parse
    /// or verification failure is a silent no-op for this event path (doc 16
    /// §3.1 fail-safe: `MessageReceived` alone still fires, nothing errors).
    FileOffered {
        /// The conversation the manifest arrived on.
        conv: Id16,
        /// Who sent it.
        from: NodeId,
        /// The manifest's signing/sharer identity.
        sharer: Key32,
        /// One entry per file offered in the manifest.
        entries: Vec<FileOffer>,
    },
    /// A pull-fetched file (`Node::fetch_file`) finished assembling and
    /// passed its whole-file BLAKE3 check; `path` is where it was written.
    FileReady {
        /// The file's content-addressed Merkle root.
        root: Id16,
        /// Where the assembled file was written.
        path: PathBuf,
    },
    /// A blob assembly moved to ABANDONED (doc 11 §6): either the whole-file
    /// hash check failed after all stripes decoded, or the assembly's
    /// `BLOB_INTEREST_TTL_MS` lapsed / `Node::cancel_fetch` was called.
    FileFailed {
        /// The file's content-addressed Merkle root.
        root: Id16,
        /// Why the assembly failed.
        reason: FileFailReason,
    },
}

/// One file offer inside a verified `FileManifest` (doc 16 §2 `FileEntry`,
/// trimmed to what an app needs to render an offer / call `fetch_file`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileOffer {
    /// The file's declared name.
    pub name: Vec<u8>,
    /// The file's declared MIME type.
    pub mime: Vec<u8>,
    /// The true plaintext length in bytes.
    pub size: u32,
    /// The file's content-addressed Merkle root — pass to `Node::fetch_file`.
    pub root: Id16,
    /// The root of an accompanying thumbnail, if the manifest entry declared one.
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
    /// The assembly's current FSM state.
    pub state: BlobState,
    /// Total Merkle-verified symbols across every stripe so far.
    pub have_symbols: u32,
    /// Total symbol slots across every stripe (`stripes * RS_TOTAL_N`).
    pub total_symbols: u32,
    /// Number of stripes fully RS-decoded and written to `.part`.
    pub stripes_done: u16,
    /// Total stripe count for this file.
    pub stripes: u16,
}

#[derive(Debug, Clone, Copy)]
struct SentMessage {
    conv: Id16,
    state: DeliveryState,
}

/// Why a send (`Node::send_text`/`send_body`/`send_file`/`group_send`/…) failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendError {
    /// We lack the peer's exchange-key / prekey-batch claim (or group key).
    UnknownPeer,
    /// The first-contact async bundle is still awaiting an authenticated
    /// receipt; emitting a direct bundle now could arrive before the peer has
    /// established its conversation root under DTN reordering.
    ConversationPending,
    /// Body exceeds MAX_BUNDLE after framing.
    TooLarge,
    /// Announcement priority is outside the wire-defined range `0..=3`.
    InvalidPriority,
    /// `peer` has published a valid identity revocation (handbook §4.6) —
    /// refuse to open a new conversation or use their prekey batch, even if
    /// one is still on file from before the revocation.
    PeerRevoked,
    /// The conversation or group sender-key ratchet cannot advance any
    /// further. No bundle is emitted; the caller must re-establish or
    /// re-key the conversation/group before retrying.
    RatchetExhausted,
    /// A new conversation would exceed the receiver-local RAM admission cap.
    /// Existing conversations remain usable; callers should retry after local
    /// state recovery rather than changing any wire parameters.
    ResourceLimit,
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
    /// Doc 18: verification, sticky revocation, scope subscription. This is
    /// runtime state; hosts may persist its validated non-secret snapshot via
    /// [`Node::authority_snapshot`] and restore it before accepting traffic.
    authority: AuthorityStore,
    auth_buckets: AuthorityBuckets,
    /// Wall clock (unix seconds), for `not_after` checks only (doc 18 §6.3).
    /// Everything else in this crate runs on the monotonic `now`. Fail-closed
    /// default 0: every cert reads as expired until the host calls
    /// `Node::set_wall_clock`, so an un-clocked node clamps everything to
    /// NORMAL and renders nothing official, rather than trusting anything.
    wall_unix_s: Option<u32>,
    convs: HashMap<Id16, Conv>,
    /// Conv IDs for which a live session has completed at least once before
    /// (E4-S4 bump_epoch wiring). RAM-only bookkeeping, not the conversation
    /// itself: distinguishes "we've met before" (bump the ratchet epoch, doc
    /// 08 §5.2) from "this conv was just created moments ago by this very
    /// session's own async first-contact send/receive" (bumping here would
    /// desync the two sides — a fresh conv is already fresh, nothing to
    /// heal). Once ratchet state gains real disk persistence, restoring a
    /// `Conv` with prior history should also insert its id here so the next
    /// contact still bumps it correctly.
    established_convs: HashSet<Id16>,
    groups: HashMap<Id16, GroupState>,
    prekeys: PrekeyStore,
    /// (conv → X3DH material) consumed by the first outgoing message.
    pending_async: HashMap<Id16, (Key32, u16)>,
    /// First-contact bundle IDs awaiting an authenticated receipt. While
    /// present, subsequent application sends are rejected locally rather than
    /// emitting direct bundles that could outrun root establishment.
    pending_first_contact: HashMap<Id16, Id16>,
    /// Sender-key dists already sent, keyed (gid, member, epoch).
    sent_dists: HashSet<(Id16, NodeId, u16)>,
    events: VecDeque<Event>,
    event_queue_dropped: u64,
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
    /// `SessionHost` seam only carries `NodeId` for (handbook §4.5). Entries
    /// are session-scoped and released after the peer's last live link closes.
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

/// The driveable node: session lifecycle, claims gossip, direct messaging,
/// groups, friendship/proxy buffering, the blob plane, receipts, beacon
/// rotation, and the tier ladder. See the crate doc for the drive model
/// (`tick`/`poll_send`/`poll_event`) and the ownership-split rationale.
pub struct Node {
    identity: Identity,
    core: NodeCore,
    sessions: HashMap<LinkId, SessionEngine>,
    outbox: VecDeque<(LinkId, Vec<u8>)>,
    close_actions: VecDeque<LinkId>,
    close_actions_dropped: u64,
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
            wall_unix_s: None,
            convs: HashMap::new(),
            established_convs: HashSet::new(),
            groups: HashMap::new(),
            prekeys,
            pending_async: HashMap::new(),
            pending_first_contact: HashMap::new(),
            sent_dists: HashSet::new(),
            events: VecDeque::with_capacity(EVENT_QUEUE_CAPACITY),
            event_queue_dropped: 0,
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
            close_actions: VecDeque::new(),
            close_actions_dropped: 0,
            hello: Hello { ver: 1, suites: vec![0x01], tier: tier as u8, flags: 0, credits: 8, psm: 0 },
            beacon_salt,
        };
        // Identity claims (doc 03 §3.3): exchange key + prekey batch, seq 1.
        node.publish_claim(field::EXCHANGE_KEY, 1, node.core.exchange.0.to_vec());
        let batch = node.core.prekeys.batch(&node.identity).encode();
        node.publish_claim(field::PREKEY_BATCH, 1, batch);
        node
    }

    /// Turn on the opt-in, local-only journey log (`DiagLog`) — off by
    /// default (`Node::new`).
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

    /// `(send_epoch, recv_epoch)` of our conversation with `peer`, if one
    /// exists — test/diagnostic seam for the E4-S4 bump_epoch mechanism (doc
    /// 08 §5.2/§5.3). Not part of the protocol surface — never called from
    /// the wire or FFI paths, same convention as [`Node::set_store_quota`].
    pub fn conv_epoch(&self, peer: &NodeId) -> Option<(u16, u16)> {
        let peer_pub = self.core.resolve_peer_key(peer)?;
        let conv_id = conversation_id(&self.core.public, &peer_pub);
        self.core.convs.get(&conv_id).map(|c| (c.send.epoch, c.recv.epoch()))
    }

    /// This node's identity, as a `NodeId` (derived from the Ed25519 public key).
    pub fn node_id(&self) -> NodeId {
        self.core.node_id
    }

    /// This node's raw Ed25519 identity public key.
    pub fn public_key(&self) -> Key32 {
        self.core.public
    }

    /// The current runtime tier.
    pub fn tier(&self) -> Tier {
        self.core.tier
    }

    /// Number of bundles currently held in the store.
    pub fn store_len(&self) -> usize {
        self.core.store.len()
    }

    /// The opt-in journey log, for `mmp-cli inspect`-style consumers.
    pub fn diagnostics(&self) -> &DiagLog {
        &self.core.diag
    }

    /// `peer`'s nickname claim, if we hold one and it decodes as UTF-8.
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
            self.core.enqueue_event(Event::TierChanged { tier });
        }
    }

    /// Block `peer`: silently deny future sessions and admission from them
    /// (doc 07 §6.5) — no frame is sent, no error is raised.
    pub fn block(&mut self, peer: NodeId) {
        // Silent (doc 07 §6.5): no frame, no error — just admission denial.
        if self.core.blocklist.contains(&peer) || self.core.blocklist.len() >= MAX_BLOCKLIST_ENTRIES {
            return;
        }
        self.core.blocklist.insert(peer);
    }

    /// Dependent side (handbook §4.5): solicit `peer_identity` as a patron —
    /// a FRIEND_OFFER is sent on the next Established session with them.
    /// Enforces `MAX_PATRONS` (doc 04 §4.4): a call once already at the cap
    /// across live patrons + outstanding offers is a silent no-op, the same
    /// fire-and-forget shape as [`Node::block`].
    /// Encode the explicit local blocklist for a host snapshot. Entries are
    /// sorted so the record is deterministic and bounded.
    pub fn blocklist_snapshot(&self) -> Vec<u8> {
        let mut peers: Vec<NodeId> = self.core.blocklist.iter().copied().collect();
        peers.sort_unstable();
        let mut out = Vec::with_capacity(3 + 16 * peers.len());
        out.push(BLOCKLIST_SNAPSHOT_VERSION);
        out.extend_from_slice(&(peers.len() as u16).to_le_bytes());
        for peer in peers {
            out.extend_from_slice(&peer.0);
        }
        out
    }

    /// Restore a blocklist snapshot before traffic is accepted. Malformed,
    /// duplicate, unsorted, or over-capacity records leave the current list
    /// untouched and return `false`.
    pub fn restore_blocklist(&mut self, bytes: &[u8]) -> bool {
        if bytes.len() < 3 || bytes[0] != BLOCKLIST_SNAPSHOT_VERSION {
            return false;
        }
        let count = u16::from_le_bytes([bytes[1], bytes[2]]) as usize;
        let Some(expected) = 3usize.checked_add(count.saturating_mul(16)) else {
            return false;
        };
        if count > MAX_BLOCKLIST_ENTRIES || bytes.len() != expected {
            return false;
        }
        let mut parsed = HashSet::with_capacity(count);
        let mut previous: Option<NodeId> = None;
        for chunk in bytes[3..].chunks_exact(16) {
            let mut id = [0u8; 16];
            id.copy_from_slice(chunk);
            let peer = Id16(id);
            if previous.is_some_and(|old| old >= peer) || !parsed.insert(peer) {
                return false;
            }
            previous = Some(peer);
        }
        self.core.blocklist = parsed;
        true
    }

    /// Encode the complete local secret-state aggregate for a future
    /// keystore-backed provider. This Rust-internal primitive is not a
    /// persistence mechanism and is intentionally absent from UniFFI.
    #[allow(dead_code)]
    pub(crate) fn secret_state_snapshot(&self) -> Result<Zeroizing<Vec<u8>>, secret_state::SecretStateError> {
        secret_state::encode(&self.core)
    }

    /// Restore a previously authenticated secret-state aggregate before any
    /// live session or wire output exists. Failed decoding is atomic and never
    /// mutates the current node.
    #[allow(dead_code)]
    pub(crate) fn restore_secret_state(&mut self, bytes: &[u8]) -> Result<(), secret_state::SecretStateError> {
        if !self.sessions.is_empty()
            || !self.outbox.is_empty()
            || !self.core.convs.is_empty()
            || !self.core.established_convs.is_empty()
            || !self.core.pending_async.is_empty()
            || !self.core.pending_first_contact.is_empty()
            || !self.core.sent.is_empty()
            || !self.core.groups.is_empty()
            || !self.core.sent_dists.is_empty()
        {
            return Err(secret_state::SecretStateError::Busy);
        }
        let state = secret_state::decode(bytes)?;
        // The unsealed aggregate seam is Rust-internal, but keep the same
        // identity-to-conversation binding enforced by the provider vault.
        // Without this check, an authenticated-but-misbound state could place
        // a different peer's ratchet under a valid map key and cause later
        // sends or receives to fail closed in confusing ways. Reject before
        // moving any decoded secrets into the live node.
        if state
            .convs
            .iter()
            .any(|(id, conv)| conversation_id(&self.core.public, &conv.peer) != *id)
        {
            return Err(secret_state::SecretStateError::Invalid);
        }
        self.core.convs = state.convs;
        self.core.established_convs = state.established_convs;
        self.core.prekeys = state.prekeys;
        self.core.pending_async = state.pending_async;
        self.core.pending_first_contact = state.pending_first_contact;
        self.core.sent = state.sent;
        Ok(())
    }

    /// Restore one provider-authenticated secret generation before any live
    /// session or secret runtime state exists. This remains Rust-internal; the
    /// provider and its DEK never cross UniFFI or React Native.
    #[allow(dead_code)]
    pub(crate) fn restore_secret_state_from_vault(
        &mut self,
        vault: &mut impl vault::SnapshotVault,
    ) -> Result<u64, vault::VaultError> {
        if !self.sessions.is_empty()
            || !self.outbox.is_empty()
            || !self.core.convs.is_empty()
            || !self.core.established_convs.is_empty()
            || !self.core.pending_async.is_empty()
            || !self.core.pending_first_contact.is_empty()
            || !self.core.sent.is_empty()
            || !self.core.groups.is_empty()
            || !self.core.sent_dists.is_empty()
        {
            return Err(vault::VaultError::Busy);
        }
        let (generation, state) = vault::restore_from_vault(vault, &self.core.public, &self.core.node_id)?;
        self.core.convs = state.convs;
        self.core.established_convs = state.established_convs;
        self.core.prekeys = state.prekeys;
        self.core.pending_async = state.pending_async;
        self.core.pending_first_contact = state.pending_first_contact;
        self.core.sent = state.sent;
        Ok(generation)
    }

    /// Request a peer to act as a patron for this node. The offer is sent on
    /// the next authenticated session and is silently capped by policy.
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
    /// monotonic `now_ms`. The first reading, including `0`, marks the clock
    /// ready. Later readings that move backwards are ignored so an NTP/user
    /// rollback cannot make an expired authority certificate valid again.
    pub fn set_wall_clock(&mut self, unix_s: u32) {
        if self.core.wall_unix_s.is_none_or(|previous| unix_s >= previous) {
            self.core.wall_unix_s = Some(unix_s);
        }
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

    /// Subscribe to a scope already known by its derived `Id16` (doc 18 §4) —
    /// same effect as [`Node::subscribe_scope`] without re-hashing the name.
    pub fn subscribe_scope_id(&mut self, scope: Id16) {
        self.core.authority.subscribe(scope);
    }

    /// Unsubscribe from a scope — its announcements stop surfacing as
    /// `Event::Announcement` (doc 18 §4).
    pub fn unsubscribe_scope(&mut self, scope: &Id16) {
        self.core.authority.unsubscribe(scope);
    }

    /// Publish a signed profile claim about ourselves (doc 06 §4) and gossip
    /// it as a class-3 bundle. Returns the claim's encoded bytes (e.g. for
    /// [`Node::revoke_identity`], which needs them to hand back to the caller).
    pub fn publish_claim(&mut self, field_id: u8, seq: u32, body: Vec<u8>) -> Vec<u8> {
        self.core.publish_claim_now(field_id, seq, body)
    }

    /// Publish our nickname as a profile claim (doc 06 §4) — `seq` must
    /// increase on each call for the LWW claim-merge rule to adopt it.
    pub fn set_nickname(&mut self, name: &str, seq: u32) {
        self.publish_claim(field::NICKNAME, seq, name.as_bytes().to_vec());
    }

    /// Publish a self-revocation claim (field `0x09`, handbook §4.6) —
    /// tombstones every other claim we own, in every node that adopts it:
    /// they refuse new sessions with us (`accept_peer`) and stop using our
    /// prekey batch for X3DH (`send_app`), via `ClaimStore::merge`'s
    /// tombstone rule. Irreversible once gossiped and adopted — there is no
    /// "un-revoke." No successor: this is a pure "this identity is dead"
    /// tombstone. Returns the claim's encoded bytes.
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

    /// Publish a self-revocation claim that names a proposed successor. The
    /// `countersig` must already be the successor's signature over
    /// [`mmp_types::claim::succession_signed_object`]; this method packages it
    /// into the revocation claim and returns the encoded claim bytes. Peers
    /// verify that countersignature before surfacing the successor, while the
    /// revocation itself remains effective even when it is invalid.
    pub fn revoke_identity_with_successor(
        &mut self,
        reason: u8,
        successor: Key32,
        countersig: Sig64,
    ) -> Vec<u8> {
        let body = Revocation {
            revoked_key: self.core.public,
            reason,
            successor: Some(successor),
            successor_countersig: Some(countersig),
        }
        .encode();
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
        if !self.core.groups.contains_key(&gs.gid) && self.core.groups.len() >= MAX_GROUPS {
            return false;
        }
        self.core.groups.insert(gs.gid, gs);
        true
    }

    /// A snapshot of the current `SeenIndex`, for persistence (`mmp-store`,
    /// handbook §4.3 step 3) — the resurrection-replay defense (AUDIT F1)
    /// this exists to protect only works across restart if it survives one.
    pub fn seen_index(&self) -> SeenIndex {
        self.core.seen.clone()
    }

    /// Restore a previously-persisted `SeenIndex`, typically once per
    /// [`Node::new`] right after opening the snapshot, before any traffic is
    /// processed — mirrors [`Node::restore_group`]'s call-site convention.
    pub fn restore_seen_index(&mut self, seen: SeenIndex) {
        self.core.seen = seen;
    }

    /// A snapshot of the current `ReceiptFilter` — mirrors [`Node::seen_index`].
    pub fn receipt_filter(&self) -> ReceiptFilter {
        self.core.receipts.clone()
    }

    /// Restore a previously-persisted `ReceiptFilter` — mirrors
    /// [`Node::restore_seen_index`].
    pub fn restore_receipt_filter(&mut self, receipts: ReceiptFilter) {
        self.core.receipts = receipts;
    }

    /// Encode receiver-local authority admission state for persistence. This
    /// contains only pinned roots, verified public certificates, subscriptions,
    /// and sticky revocations; it never contains identity or ratchet secrets.
    pub fn authority_snapshot(&self) -> Vec<u8> {
        self.core.authority.encode()
    }

    /// Restore a previously validated authority snapshot before processing
    /// traffic. Returns `false` for malformed, tampered, or cross-scope state
    /// and leaves the current authority store unchanged in that case.
    pub fn restore_authority(&mut self, bytes: &[u8]) -> bool {
        let Some(authority) = AuthorityStore::decode(bytes) else { return false };
        self.core.authority = authority;
        true
    }

    // ------------------------------------------------------------- sending --

    /// Send a text message to `peer`; returns the BundleID to track.
    pub fn send_text(&mut self, peer: &NodeId, text: &str) -> Result<Id16, SendError> {
        self.send_body(peer, apptype::TEXT, text.as_bytes())
    }

    /// Send a raw application body (e.g. a FileManifest as apptype 0x01).
    pub fn send_body(&mut self, peer: &NodeId, app: u8, payload: &[u8]) -> Result<Id16, SendError> {
        // This is an application/FFI boundary: reject a payload that cannot
        // fit in any v1 bundle before copying attacker-sized host input into a
        // temporary body buffer. `send_app` performs the exact wire-size
        // preflight after framing; this coarse bound only caps the allocation
        // shape and rejects no valid bundle.
        if payload.len() >= MAX_BUNDLE {
            return Err(SendError::TooLarge);
        }
        let mut body = Vec::with_capacity(1 + payload.len());
        body.push(app);
        body.extend_from_slice(payload);
        self.core.send_direct(peer, &body)
    }

    /// The last-known `DeliveryState` of a message we sent, or `None` if `id`
    /// wasn't sent from this node (or was sent before restart — sent-message
    /// tracking is RAM-only).
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

    /// Invite `peer`: add it to the member set and ship our sender key through
    /// the pairwise channel. Each member distributes only its own key; a
    /// recipient must never trust a relay to assert another member's origin.
    /// Only the immutable v1 group administrator may mutate membership;
    /// non-admin members fail closed just like [`Self::group_remove`].
    pub fn group_invite(&mut self, gid: &Id16, peer: &NodeId) -> Result<(), SendError> {
        {
            let gs = self.core.groups.get_mut(gid).ok_or(SendError::UnknownPeer)?;
            if !gs.is_admin {
                return Err(SendError::UnknownPeer);
            }
        }
        // Ship the authenticated sender-key distribution before mutating the
        // OR-Set. Unknown peers and a pending first-contact conversation must
        // leave local membership untouched when the application send is
        // rejected; the key distribution itself does not require membership
        // to have been added yet.
        self.core.send_own_dist(gid, peer)?;
        self.core.groups.get_mut(gid).ok_or(SendError::UnknownPeer)?.add_member(*peer);
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
            let epoch = gs
                .remove_member(&mut self.core.rng, member)
                .ok_or(SendError::RatchetExhausted)?;
            (epoch, gs.members.members())
        };
        // A new local sender-key epoch supersedes every distribution marker
        // from older epochs for this group.  Keeping those markers forever
        // lets repeated membership rotations grow RAM without bound and can
        // suppress no useful send: `send_own_dist` always includes the epoch
        // in its idempotence key.  Drop only this group's stale epochs; other
        // groups and the freshly rotated epoch remain untouched.  If a
        // re-key send below fails, the missing current-epoch marker is still
        // absent and will be retried on the next contact.
        self.core.sent_dists.retain(|(sent_gid, _, sent_epoch)| {
            *sent_gid != *gid || *sent_epoch == epoch
        });
        let me = self.core.node_id;
        for m in members {
            if m != me && m != *member {
                let _ = self.core.send_own_dist(gid, &m);
            }
        }
        self.core.enqueue_event(Event::GroupEpochChanged { gid: *gid, epoch });
        Ok(epoch)
    }

    /// Send a text into the group under our sender key (doc 08 §6.2).
    pub fn group_send(&mut self, gid: &Id16, text: &str) -> Result<Id16, SendError> {
        // This is an application/FFI boundary: reject a payload that cannot
        // fit in any v1 bundle before copying attacker-sized host input into
        // a temporary body buffer. The exact placeholder preflight below
        // still handles the fixed group-envelope overhead for smaller bodies.
        if text.len() >= MAX_BUNDLE {
            return Err(SendError::TooLarge);
        }
        self.core.preflight_group_size(gid, text.len())?;
        let me = self.core.node_id;
        let needs_distribution = self.core.groups.get(gid).is_some_and(GroupState::needs_distribution);
        if needs_distribution {
            let members = self.core.groups.get(gid).map(|gs| gs.members.members()).unwrap_or_default();
            for member in members {
                if member != me {
                    self.core.send_own_dist(gid, &member)?;
                }
            }
            if let Some(gs) = self.core.groups.get_mut(gid) {
                gs.mark_distribution_attempted();
            }
        }
        self.core.send_group(gid, text.as_bytes())
    }

    /// Current membership of `gid`, or empty if we don't know that group.
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

    /// Return the concurrent-session admission cap for this public node API.
    ///
    /// This is the implementation mapping of specification/11 §3's
    /// normative T2/T3/Core caps: the standard `Node` exposes `Leaf` as the
    /// constrained T1 profile (one session), `Relay` as T2 (two sessions),
    /// and `Anchor` as T3 (eight sessions).  The T0/Core profile is an
    /// embedded API and is not represented by this `std` `Node` type.
    fn max_sessions_for_tier(tier: Tier) -> usize {
        match tier {
            Tier::Leaf => 1,
            Tier::Relay => 2,
            Tier::Anchor => 8,
        }
    }

    fn outbox_frame_cap(tier: Tier) -> usize {
        Self::max_sessions_for_tier(tier) * (OUTBOX_DATA_CREDIT_WINDOW + OUTBOX_CONTROL_RESERVE)
    }

    fn outbox_over_budget(
        outbox: &VecDeque<(LinkId, Vec<u8>)>,
        tier: Tier,
        link: LinkId,
        additional_bytes: usize,
    ) -> bool {
        let mut total_frames = 0usize;
        let mut total_bytes = 0usize;
        let mut link_frames = 0usize;
        let mut link_bytes = 0usize;
        for (queued_link, bytes) in outbox {
            total_frames = total_frames.saturating_add(1);
            total_bytes = total_bytes.saturating_add(bytes.len());
            if *queued_link == link {
                link_frames = link_frames.saturating_add(1);
                link_bytes = link_bytes.saturating_add(bytes.len());
            }
        }
        let frame_cap = Self::outbox_frame_cap(tier);
        let byte_cap = frame_cap.saturating_mul(OUTBOX_FRAME_BYTES);
        total_frames >= frame_cap
            || total_bytes.saturating_add(additional_bytes) > byte_cap
            || link_frames >= OUTBOX_DATA_CREDIT_WINDOW + OUTBOX_CONTROL_RESERVE
            || link_bytes.saturating_add(additional_bytes)
                > (OUTBOX_DATA_CREDIT_WINDOW + OUTBOX_CONTROL_RESERVE).saturating_mul(OUTBOX_FRAME_BYTES)
    }

    /// Drop the peer identity cache entry once its last authenticated session
    /// is removed. `known_identities` exists only because the friendship
    /// callbacks receive a `NodeId`; they run while an established
    /// `SessionEngine` is alive and do not need a process-lifetime identity
    /// cache. Keeping entries while another link for the same peer remains
    /// established preserves interleaved FRIEND_ACK handling.
    fn release_known_identity_for_link(&mut self, link: LinkId) {
        let Some(peer) = self.sessions.get(&link).and_then(SessionEngine::peer) else { return };
        let still_active = self
            .sessions
            .iter()
            .any(|(other, engine)| *other != link && engine.peer() == Some(peer));
        if !still_active {
            self.core.known_identities.remove(&peer);
        }
    }

    fn clear_link_outbox(&mut self, link: LinkId) {
        self.outbox.retain(|(queued_link, _)| *queued_link != link);
        self.release_known_identity_for_link(link);
        self.sessions.remove(&link);
    }

    fn is_close_frame(bytes: &[u8]) -> bool {
        matches!(bytes.get(2), Some(typ) if *typ == FT_ERROR || *typ == FT_CLOSE)
    }

    fn has_close_frame(&self, link: LinkId) -> bool {
        self.outbox.iter().any(|(queued_link, bytes)| *queued_link == link && bytes.get(2) == Some(&FT_CLOSE))
    }

    fn clear_link_outbox_preserving_close(&mut self, link: LinkId) {
        // SessionEngine emits ERROR+CLOSE before publishing Closed. Data
        // queued by an earlier pump is no longer valid once the session has
        // entered CLOSING, but those terminal control frames must still reach
        // the transport before the host tears the physical link down.
        self.outbox.retain(|(queued_link, bytes)| *queued_link != link || Self::is_close_frame(bytes));
        self.release_known_identity_for_link(link);
        self.sessions.remove(&link);
    }

    fn request_close(&mut self, link: LinkId) {
        if self.close_actions.contains(&link) {
            return;
        }
        if self.close_actions.len() >= CLOSE_ACTION_CAPACITY {
            // Keep queue saturation observable; the host must drain
            // poll_close promptly to preserve transport cleanup.
            self.close_actions_dropped = self.close_actions_dropped.saturating_add(1);
            return;
        }
        self.close_actions.push_back(link);
    }

    fn cancel_close(&mut self, link: LinkId) {
        self.close_actions.retain(|queued| *queued != link);
    }

    /// Feed one transport-layer event into the node: beacon overheard, a link
    /// coming up/down, or bytes arriving on a link. Drives the per-link
    /// `SessionEngine`s and the E4-S4 opportunistic ratchet epoch bump on
    /// link teardown.
    pub fn on_tal_event(&mut self, ev: TalEvent) {
        match ev {
            TalEvent::Beacon { payload, .. } => {
                if let Ok(b) = Beacon::decode(&payload) {
                    self.core.utility.note_ephem(b.ephem_id, self.core.now);
                }
            }
            TalEvent::LinkUp { link, initiator, .. } => {
                // Admission denial is intentionally silent (specification/11
                // §3): retain existing links and emit no handshake output.
                if self.sessions.contains_key(&link) {
                    return;
                }
                if self.sessions.len() >= Self::max_sessions_for_tier(self.core.tier) {
                    self.request_close(link);
                    return;
                }
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
                // Opportunistic ratchet epoch bump (doc 08 §5.2/§5.3, E4-S4
                // bump_epoch wiring): applied here, at contact TEARDOWN, not
                // at `Established` — a message queued moments before this
                // contact (e.g. `group_remove`'s re-key send) was encrypted
                // under whatever epoch the chain was at BEFORE this contact
                // began, and must still be transmitted and decrypted THIS
                // SAME contact before the epoch moves on for next time.
                // Bumping at session start would orphan it: the receiver's
                // recv chain would already be past the epoch that message
                // was encrypted under. `chan` (unlike `hs`/`recon`) survives
                // all the way from `Established` to here, so the material
                // and peer identity are both still available.
                if let Some(engine) = self.sessions.get(&link) {
                    if let (Some(material), Some(identity)) =
                        (engine.session_material(), engine.peer_identity())
                    {
                        let conv_id = conversation_id(&self.core.public, &identity);
                        let met_before = self.core.established_convs.contains(&conv_id);
                        if !met_before && self.core.established_convs.len() >= MAX_CONVERSATION_STATE {
                            // Multiple links may have been admitted while the
                            // marker table still had room and tear down after
                            // another link fills it. Do not let that race
                            // exceed the bound; retire the unmarked Conv so a
                            // later contact is denied rather than reusing an
                            // untracked ratchet without an epoch bump.
                            self.core.convs.remove(&conv_id);
                            self.core.pending_async.remove(&conv_id);
                            self.core.pending_first_contact.remove(&conv_id);
                        } else {
                            self.core.established_convs.insert(conv_id);
                        }
                        if met_before {
                            // Epoch rollover is intentionally fail-closed.  A
                            // u16 epoch cannot be incremented at its terminal
                            // value; retaining a conversation whose two
                            // chains cannot move together would permit an
                            // accidental nonce/key reuse or a permanent
                            // send/receive desynchronization.  Remove the
                            // local state so a later send must establish a
                            // fresh conversation instead of panicking.
                            let retire = if let Some(conv) = self.core.convs.get_mut(&conv_id) {
                                if conv.send.epoch == u16::MAX || conv.recv.epoch() == u16::MAX {
                                    true
                                } else {
                                    let send_ok = conv.send.try_bump_epoch(&material).is_ok();
                                    let recv_ok = conv.recv.try_bump_epoch(&material).is_ok();
                                    !send_ok || !recv_ok
                                }
                            } else {
                                false
                            };
                            if retire {
                                self.core.convs.remove(&conv_id);
                                self.core.established_convs.remove(&conv_id);
                                self.core.pending_async.remove(&conv_id);
                                self.core.pending_first_contact.remove(&conv_id);
                            }
                        }
                    }
                }
                self.cancel_close(link);
                self.clear_link_outbox(link);
            }
        }
    }

    fn pump_link(&mut self, link: LinkId) {
        let mut overflowed = false;
        let mut closed = false;
        {
            let Some(engine) = self.sessions.get_mut(&link) else { return };
            while let Some(bytes) = engine.poll_out() {
                if Self::outbox_over_budget(&self.outbox, self.core.tier, link, bytes.len()) {
                    overflowed = true;
                    break;
                }
                self.outbox.push_back((link, bytes));
            }
            if !overflowed {
                while let Some(ev) = engine.poll_event() {
                    match ev {
                        SessionEvent::Established { peer, .. } => {
                            self.core.utility.note_contact(&peer, 1000, self.core.now);
                            self.core.enqueue_event(Event::Established { peer });
                            self.core.enqueue_event(Event::ContactSeen { peer });
                        }
                        SessionEvent::Closed => closed = true,
                    }
                }
            }
        }
        if overflowed {
            // Never drop one encrypted frame and continue: that would leave
            // the Noise nonce stream ambiguous. Drop the affected session and
            // all of its queued frames, preserving unrelated links.
            self.clear_link_outbox(link);
            self.request_close(link);
        } else if closed {
            self.clear_link_outbox_preserving_close(link);
            self.request_close(link);
        }
    }

    /// Wire bytes to write to a link; drain fully each turn.
    pub fn poll_send(&mut self) -> Option<(LinkId, Vec<u8>)> {
        self.outbox.pop_front()
    }

    /// Return the next transport-local close action once terminal wire output
    /// has drained. The host must close the physical link and then feed
    /// [`TalEvent::LinkDown`] back to the node; this action is not a protocol
    /// frame and does not change the wire specification. `None` can therefore
    /// mean "send the pending CLOSE first", not only "no close is needed".
    pub fn poll_close(&mut self) -> Option<LinkId> {
        // A graceful/protocol-error close is a two-step wire operation. Do
        // not hand the host the physical teardown action until its CLOSE
        // envelope has been drained through poll_send; otherwise a host that
        // follows the normal close-then-send polling order would drop it.
        if let Some(&link) = self.close_actions.front() {
            if self.has_close_frame(link) {
                return None;
            }
        }
        self.close_actions.pop_front()
    }

    /// Number of close actions dropped because the bounded host queue was
    /// full. A non-zero value means the host did not drain [`Node::poll_close`]
    /// promptly and should treat this as an operational error.
    pub fn close_actions_dropped(&self) -> u64 {
        self.close_actions_dropped
    }

    /// The next queued [`Event`] for the app to consume; drain fully each turn.
    pub fn poll_event(&mut self) -> Option<Event> {
        self.core.events.pop_front()
    }

    /// Number of application events discarded because the host did not drain
    /// [`Node::poll_event`] before the bounded callback FIFO filled.  A
    /// non-zero value is an operational backpressure signal; it is never
    /// caused by a remote peer directly and does not alter protocol state.
    pub fn event_queue_dropped(&self) -> u64 {
        self.core.event_queue_dropped
    }

    /// Advance the monotonic clock (doc 11 §7): expiry, filter rotation,
    /// session timers, quota eviction. A sample earlier than the last
    /// accepted value is ignored (the public, no-return API fails closed) so
    /// a native host that bypasses a higher-level drive-loop guard cannot
    /// rewind TTL or session state.
    pub fn tick(&mut self, now_ms: u64) {
        if now_ms < self.core.now {
            return;
        }
        self.core.now = now_ms;
        let expired = self.core.store.expire(now_ms);
        for id in &expired {
            self.core.diag.log(*id, JourneyEvent::Expired, now_ms);
            self.core.pending_first_contact.retain(|_, pending| pending != id);
            if let Some(m) = self.core.sent.get_mut(id) {
                if m.state != DeliveryState::Delivered {
                    m.state = DeliveryState::Expired;
                    self.core.enqueue_event(Event::DeliveryProgress {
                        id: *id,
                        state: DeliveryState::Expired,
                    });
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
        // Admission buckets are RAM-only accounting state. Compact idle
        // origins/authority subjects so a Sybil stream cannot leave one map
        // entry per pseudonymous key forever. The idle horizon matches the
        // longest ordinary message/symbol lifetime; after that delay a full
        // token burst is already the conservative refill state.
        self.core.buckets.compact(now_ms, LIFETIME_MESSAGE_MS);
        self.core.auth_buckets.compact(now_ms, LIFETIME_MESSAGE_MS);
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
    /// Whether a pairwise conversation may be created without exceeding the
    /// receiver-local state bound. Existing conversations are always allowed
    /// to continue; callers receive the explicit `ResourceLimit` error for a
    /// new conversation rejected solely by this local resource policy.
    fn conversation_capacity_available(&self, conv_id: &Id16) -> bool {
        self.convs.contains_key(conv_id)
            || (self.convs.len() < MAX_CONVERSATION_STATE
                && self.established_convs.len() < MAX_CONVERSATION_STATE)
    }

    /// Whether one more locally-originated delivery record can be admitted.
    /// Terminal history is reclaimable; active messages are never evicted just
    /// to satisfy a later send.
    fn sent_capacity_available(&self) -> bool {
        self.sent.len() < MAX_SENT_RUNTIME
            || self
                .sent
                .values()
                .any(|message| matches!(message.state, DeliveryState::Delivered | DeliveryState::Expired))
    }

    /// Insert one delivery record, evicting only terminal history if the
    /// bounded table is full. The caller must preflight with
    /// `sent_capacity_available` before creating a bundle.
    fn record_sent(&mut self, id: Id16, conv: Id16) -> Result<(), SendError> {
        if self.sent.len() >= MAX_SENT_RUNTIME {
            let terminal = self.sent.iter().find_map(|(id, message)| {
                matches!(message.state, DeliveryState::Delivered | DeliveryState::Expired).then_some(*id)
            });
            let Some(terminal) = terminal else { return Err(SendError::ResourceLimit) };
            self.sent.remove(&terminal);
        }
        self.sent.insert(id, SentMessage { conv, state: DeliveryState::Queued });
        Ok(())
    }

    /// Enqueue one ciphertext-free application callback without allowing the
    /// host-facing queue to grow with authenticated peer traffic.  Keep the
    /// existing FIFO when saturated (drop newest) so already queued lifecycle
    /// events retain ordering; expose the loss through a sticky counter.
    fn enqueue_event(&mut self, event: Event) {
        if self.events.len() >= EVENT_QUEUE_CAPACITY {
            self.event_queue_dropped = self.event_queue_dropped.saturating_add(1);
            return;
        }
        self.events.push_back(event);
    }

    fn learn_claim(&mut self, claim: Claim, pinned: bool) {
        let owner = node_id_of(&claim.owner);
        let field_id = claim.field;
        let claim_owner = claim.owner;
        let claim_body = claim.body.clone();
        match self.claims.merge(claim, self.now, pinned) {
            mmp_core::ClaimMerge::Adopted => {
                self.enqueue_event(Event::ClaimUpdated { owner, field: field_id });
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
                                if let Some(now_unix_s) = self.wall_unix_s {
                                    let _ = self.authority.consider_cert(&cert, now_unix_s);
                                }
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
                        self.enqueue_event(Event::PeerRevoked { peer: owner, successor });
                    }
                    _ => {}
                }
            }
            mmp_core::ClaimMerge::Conflict { .. } => {
                self.enqueue_event(Event::CompromiseWarning { owner });
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
        // `Bundle::decode_canonical` rejects priorities above BULK. Validate
        // local API input before signing or storing so release builds cannot
        // mint a bundle that this node (or any peer) will reject.
        if priority > prio::BULK {
            return Err(SendError::InvalidPriority);
        }
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
        if bundle_wire_len(&bundle) > MAX_BUNDLE {
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
        // Unknown announcement suites are opaque future-version payloads:
        // relay/storage may retain them, but v1 must not render or act on an
        // unimplemented body grammar.
        if body.suite != 0x01 {
            return;
        }
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
                    if let Some(now_unix_s) = self.wall_unix_s {
                        let _ = self.authority.consider_cert(&cert, now_unix_s);
                    }
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
            let grant = self
                .wall_unix_s
                .and_then(|now_unix_s| self.authority.grant_for(&bundle.origin, &bundle.dest, bundle.priority, now_unix_s))
                .copied();
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
        let official = self
            .wall_unix_s
            .is_some_and(|now_unix_s| self.authority.official(&bundle.origin, &bundle.dest, now_unix_s));

        if subscribed {
            self.enqueue_event(Event::Announcement {
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

    /// Resolve the peer's current X25519 exchange-key claim for the sealed-box
    /// baseline. The identity key alone is not sufficient for asynchronous
    /// encryption; malformed or missing exchange claims fail closed.
    fn resolve_peer_exchange(&self, peer: &NodeId) -> Option<Key32> {
        let claim = self.claims.get(peer, field::EXCHANGE_KEY)?;
        if claim.body.len() != 32 {
            return None;
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(&claim.body);
        Some(Key32(key))
    }

    /// Build the always-available sealed-box first-contact form (doc 08
    /// §4.1). This path is used only when the peer's exchange claim exists but
    /// no usable prekey batch is available. The sealed conversation root is
    /// retained locally so the normal receipt and subsequent ratchet paths can
    /// use the same conversation state after the message is delivered.
    fn send_sealed(
        &mut self,
        peer: &NodeId,
        peer_pub: &Key32,
        conv_id: Id16,
        app: u8,
        payload: &[u8],
    ) -> Result<Id16, SendError> {
        let peer_exchange = self.resolve_peer_exchange(peer).ok_or(SendError::UnknownPeer)?;
        if !self.conversation_capacity_available(&conv_id) {
            return Err(SendError::ResourceLimit);
        }
        let mut body = Vec::with_capacity(1 + payload.len());
        body.push(app);
        body.extend_from_slice(payload);

        // The ephemeral key is part of the async payload, which is excluded
        // from the outer AD. A zero placeholder therefore has the same header
        // length and lets us compute AD before sealing.
        let placeholder = AsyncPayload::Sealed { eph: Key32([0; 32]), ct: vec![0u8; body.len() + 16] }.encode();
        let mut bundle = Bundle {
            class: class::DIRECT,
            flags: FLAG_ASYNC_KEYS | FLAG_WANTS_RECEIPT,
            origin: self.public,
            dest: Id16(peer.0),
            lifetime_ms: LIFETIME_MESSAGE_MS,
            priority: 1,
            exts: Vec::new(),
            payload: placeholder,
            signature: Sig64::ZERO,
        };
        let (eph, ct, ck0) = seal_box_with_root(
            &mut self.rng,
            &peer_exchange,
            &self.public,
            peer_pub,
            &bundle.canonical_aead_ad(),
            &body,
        );
        bundle.payload = AsyncPayload::Sealed { eph, ct }.encode();
        bundle.signature = self.identity.sign(SigDomain::Bundle, &bundle.canonical_zeroed());
        if bundle_wire_len(&bundle) > MAX_BUNDLE {
            return Err(SendError::TooLarge);
        }

        let budget = origin_budget(self.utility.n_hat(self.now));
        let id = self.store_own(bundle, budget);
        let mut conv = Conv::new(conv_id, &self.public, peer_pub, *ck0);
        conv.dag.insert(id, &[]);
        self.convs.insert(conv_id, conv);
        self.pending_first_contact.insert(conv_id, id);
        Ok(id)
    }

    /// Build, sign and store a direct message; the mesh does the rest.
    fn send_direct(&mut self, peer: &NodeId, body: &[u8]) -> Result<Id16, SendError> {
        if !self.sent_capacity_available() {
            // Preserve the same cheap admission precedence as `send_app`, but
            // fail before any first-contact X3DH/prekey or ratchet work can
            // emit a bundle while the local delivery table is saturated.
            if self.claims.revocation(peer).is_some() {
                return Err(SendError::PeerRevoked);
            }
            let peer_pub = self.resolve_peer_key(peer).ok_or(SendError::UnknownPeer)?;
            let conv_id = conversation_id(&self.public, &peer_pub);
            if self.pending_first_contact.contains_key(&conv_id) {
                return Err(SendError::ConversationPending);
            }
            return Err(SendError::ResourceLimit);
        }
        let id = self.send_app(peer, body[0], &body[1..])?;
        let peer_pub = self.resolve_peer_key(peer).expect("resolved in send_app");
        self.record_sent(id, conversation_id(&self.public, &peer_pub))?;
        self.enqueue_event(Event::DeliveryProgress { id, state: DeliveryState::Queued });
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
        if self.pending_first_contact.contains_key(&conv_id) {
            return Err(SendError::ConversationPending);
        }
        if !self.conversation_capacity_available(&conv_id) {
            return Err(SendError::ResourceLimit);
        }
        let mut body = Vec::with_capacity(1 + payload.len());
        body.push(app);
        body.extend_from_slice(payload);

        if !self.convs.contains_key(&conv_id) {
            // Preflight the largest first-contact encoding (the prekey form)
            // before generating ephemeral material or creating a Conv. The
            // sealed fallback is smaller, so this is a safe upper bound for
            // both normative async modes.
            let first_placeholder = AsyncPayload::Prekey {
                prekey_id: 0,
                sender_xpk: self.exchange,
                eph: Key32([0; 32]),
                ct: vec![0u8; body.len() + 16],
            }
            .encode();
            let first_bundle = Bundle {
                class: class::DIRECT,
                flags: FLAG_ASYNC_KEYS | FLAG_WANTS_RECEIPT,
                origin: self.public,
                dest: Id16(peer.0),
                lifetime_ms: LIFETIME_MESSAGE_MS,
                priority: 1,
                exts: Vec::new(),
                payload: first_placeholder,
                signature: Sig64::ZERO,
            };
            if bundle_wire_len(&first_bundle) > MAX_BUNDLE {
                return Err(SendError::TooLarge);
            }
        }

        if !self.convs.contains_key(&conv_id) {
            // Prefer X3DH against the peer's prekey batch (doc 08 §4.2), but
            // retain the always-available sealed-box baseline when a valid
            // batch is absent, malformed, or suppressed (§4.1).
            let agreement = self
                .claims
                .get(peer, field::PREKEY_BATCH)
                .and_then(|claim| PrekeyBatch::decode(&claim.body).ok())
                .and_then(|batch| sender_agree(&mut self.rng, &self.identity, &peer_pub, &batch, Some(0)).ok());
            let Some(agreement) = agreement else {
                return self.send_sealed(peer, &peer_pub, conv_id, app, payload);
            };
            self.convs.insert(conv_id, Conv::new(conv_id, &self.public, &peer_pub, *agreement.ck0));
            self.pending_async.insert(conv_id, (agreement.eph_pub, agreement.prekey_id));
        }

        let dest = Id16(peer.0);
        let pending = self.pending_async.get(&conv_id).copied();
        let parents = self.convs.get(&conv_id).map(|conv| conv.dag.heads()).unwrap_or_default();
        let mut flags = FLAG_WANTS_RECEIPT;
        if pending.is_some() {
            flags |= FLAG_ASYNC_KEYS;
        }

        let mut exts = Vec::new();
        if !parents.is_empty() {
            flags |= FLAG_HAS_PARENTS;
            exts.push(Ext::Parents(parents));
        }

        // ChaCha20-Poly1305 appends a fixed 16-byte tag. The placeholder
        // lets us construct the exact bundle header (including payload_len)
        // before sealing, while the live lifetime is normalized by
        // `canonical_aead_ad` so relays may decrement it safely.
        let placeholder_ct = vec![0u8; body.len() + 16];
        let placeholder_payload = if let Some((eph, prekey_id)) = pending {
            AsyncPayload::Prekey {
                prekey_id,
                sender_xpk: self.exchange,
                eph,
                ct: placeholder_ct,
            }
            .encode()
        } else {
            // Epoch and index are fixed-width fields; zero is a sizing
            // placeholder and does not consume the sender ratchet.
            DirectPayload { conv: conv_id, epoch: 0, index: 0, ct: placeholder_ct }.encode()
        };
        let mut bundle = Bundle {
            class: class::DIRECT,
            flags,
            origin: self.public,
            dest,
            lifetime_ms: LIFETIME_MESSAGE_MS,
            priority: 1,
            exts,
            payload: placeholder_payload,
            signature: Sig64::ZERO,
        };
        // Validate the exact wire size before advancing the sender chain.
        // This keeps a rejected oversized send from poisoning the next
        // first-contact async index or a later direct message.
        if bundle_wire_len(&bundle) > MAX_BUNDLE {
            return Err(SendError::TooLarge);
        }

        let (msg_key, epoch, index) = {
            let conv = self.convs.get_mut(&conv_id).expect("just ensured");
            conv.send.try_next().map_err(|_| SendError::RatchetExhausted)?
        };
        let key = aead::committed_key(&msg_key, &msg_context(&self.public, &dest, &conv_id, epoch, index));
        let ct = aead::seal(&key, &bundle.canonical_aead_ad(), &body);
        bundle.payload = if let Some((eph, prekey_id)) = pending {
            AsyncPayload::Prekey { prekey_id, sender_xpk: self.exchange, eph, ct }.encode()
        } else {
            DirectPayload { conv: conv_id, epoch, index, ct }.encode()
        };
        bundle.signature = self.identity.sign(SigDomain::Bundle, &bundle.canonical_zeroed());
        if bundle_wire_len(&bundle) > MAX_BUNDLE {
            return Err(SendError::TooLarge);
        }
        if pending.is_some() {
            self.pending_async.remove(&conv_id);
        }
        let parents_copy: Vec<Id16> = bundle.parents().to_vec();
        let budget = origin_budget(self.utility.n_hat(self.now));
        let id = self.store_own(bundle, budget);
        if let Some(c) = self.convs.get_mut(&conv_id) {
            c.dag.insert(id, &parents_copy);
        }
        if pending.is_some() {
            self.pending_first_contact.insert(conv_id, id);
        }
        Ok(id)
    }

    fn send_own_dist(&mut self, gid: &Id16, peer: &NodeId) -> Result<(), SendError> {
        let (dist_bytes, epoch) = {
            let gs = self.groups.get(gid).ok_or(SendError::UnknownPeer)?;
            (gs.local.dist().encode(), gs.epoch)
        };
        if self.sent_dists.contains(&(*gid, *peer, epoch)) {
            return Ok(()); // already shipped for this epoch
        }
        let mut body = Vec::with_capacity(16 + dist_bytes.len());
        body.extend_from_slice(&self.node_id.0);
        body.extend_from_slice(&dist_bytes);
        self.send_app(peer, apptype::SENDER_KEY, &body).map(|_| {
            self.sent_dists.insert((*gid, *peer, epoch));
        })
    }

    fn preflight_group_size(&self, gid: &Id16, text_len: usize) -> Result<(), SendError> {
        let epoch = self.groups.get(gid).ok_or(SendError::UnknownPeer)?.epoch;
        let body_len = text_len.checked_add(1).ok_or(SendError::TooLarge)?;
        let ct_len = body_len.checked_add(16).ok_or(SendError::TooLarge)?;
        if ct_len > u16::MAX as usize {
            return Err(SendError::TooLarge);
        }
        // Group encryption advances the local sender chain. Construct an
        // exact-size placeholder first so an oversized application body is
        // rejected before consuming a chain index or signing key operation.
        // The ciphertext and member signature lengths are fixed, so the
        // placeholder has the same canonical wire width as the live payload.
        let placeholder = GroupPayload {
            gid: *gid,
            epoch,
            member: self.node_id,
            chain_index: 0,
            ct: vec![0u8; ct_len],
            member_sig: Sig64::ZERO,
        };
        let candidate = Bundle {
            class: class::GROUP,
            flags: 0,
            origin: self.public,
            dest: *gid,
            lifetime_ms: LIFETIME_MESSAGE_MS,
            priority: 1,
            exts: vec![Ext::GroupEpoch(epoch)],
            payload: placeholder.encode(),
            signature: Sig64::ZERO,
        };
        if bundle_wire_len(&candidate) > MAX_BUNDLE {
            return Err(SendError::TooLarge);
        }
        Ok(())
    }

    fn send_group(&mut self, gid: &Id16, text: &[u8]) -> Result<Id16, SendError> {
        let me = self.node_id;
        let origin = self.public;
        let dest = *gid;
        let mut body = Vec::with_capacity(1 + text.len());
        body.push(apptype::TEXT);
        body.extend_from_slice(text);
        self.preflight_group_size(gid, text.len())?;
        let epoch = self.groups.get(gid).ok_or(SendError::UnknownPeer)?.epoch;
        let (epoch, gp) = {
            let gs = self.groups.get_mut(gid).ok_or(SendError::UnknownPeer)?;
            let gp = gs
                .local
                .try_encrypt_with_ad(me, &body, |placeholder| {
                    Bundle {
                        class: class::GROUP,
                        flags: 0,
                        origin,
                        dest,
                        lifetime_ms: LIFETIME_MESSAGE_MS,
                        priority: 1,
                        exts: vec![Ext::GroupEpoch(epoch)],
                        payload: placeholder.encode(),
                        signature: Sig64::ZERO,
                    }
                    .canonical_aead_ad()
                })
                .map_err(|_| SendError::RatchetExhausted)?;
            (epoch, gp)
        };
        let mut bundle = Bundle {
            class: class::GROUP,
            flags: 0,
            origin,
            dest,
            lifetime_ms: LIFETIME_MESSAGE_MS,
            priority: 1,
            exts: vec![Ext::GroupEpoch(epoch)],
            payload: gp.encode(),
            signature: Sig64::ZERO,
        };
        bundle.signature = self.identity.sign(SigDomain::Bundle, &bundle.canonical_zeroed());
        if bundle_wire_len(&bundle) > MAX_BUNDLE {
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
            self.enqueue_event(Event::MessageGap { conv: conv_id, parents: missing });
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
            self.enqueue_event(Event::MessageReceived {
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
        // A valid signature and ciphertext are necessary but not sufficient
        // to allocate another permanent conversation. Check the local bound
        // before doing X3DH/sealed-box work or consuming a one-time prekey;
        // an over-cap first contact is rejected without mutating state.
        if !self.conversation_capacity_available(conv_id) {
            return None;
        }
        let payload = AsyncPayload::decode(&bundle.payload).ok()?;
        match payload {
            AsyncPayload::Sealed { eph, ct } => {
                // Sealed fallback is a first-contact form. Never replace an
                // established conversation with a new ephemeral root: doing
                // so would make queued direct messages undecryptable and
                // invalidate receipt MACs. A sender that lost local state must
                // use the live-session recovery path instead of resetting the
                // peer's established state.
                if self.convs.contains_key(conv_id) {
                    return None;
                }
                let plaintext = open_box(&self.identity, &eph, &bundle.canonical_aead_ad(), &ct).ok()?;
                let dh = self.identity.exchange_dh(&eph);
                let ck0 = sealed_ck0(&dh, origin, &self.public);
                // Commit only after the sealed ciphertext authenticates. A
                // forged, signature-valid bundle must not create conversation
                // state or consume receiver resources.
                self.convs.insert(*conv_id, Conv::new(*conv_id, &self.public, origin, *ck0));
                Some(plaintext)
            }
            AsyncPayload::Prekey { prekey_id, sender_xpk, eph, ct } => {
                // Concurrent-initiation conflict: adopt the peer's root only
                // if its identity is canonically smaller; both sides then
                // choose the same winner. Build candidate state and verify
                // its first ciphertext before committing it. A valid bundle
                // signature alone must not authorize ratchet-state mutation.
                if self.convs.contains_key(conv_id)
                    && (self.established_convs.contains(conv_id) || origin.0 >= self.public.0)
                {
                    return None;
                }
                let ck0 =
                    recipient_agree(&self.prekeys, &self.identity, origin, &sender_xpk, &eph, prekey_id)
                        .ok()?;
                let mut candidate = Conv::new(*conv_id, &self.public, origin, *ck0);
                let key = candidate.recv.key_for(0, 0).ok()?;
                let bound = aead::committed_key(&key, &msg_context(origin, &self.node_id, conv_id, 0, 0));
                let plaintext = aead::open(&bound, &bundle.canonical_aead_ad(), &ct).ok()?;
                if prekey_id != PREKEY_NONE {
                    self.prekeys.mark_otk_consumed(prekey_id).ok()?;
                }
                self.convs.insert(*conv_id, candidate);
                Some(plaintext)
            }
        }
    }

    fn open_direct(&mut self, bundle: &Bundle, conv_id: &Id16) -> Option<Vec<u8>> {
        let dp = DirectPayload::decode(&bundle.payload).ok()?;
        let conv = self.convs.get_mut(conv_id)?;
        let key = conv.recv.key_for(dp.epoch, dp.index).ok()?;
        let bound =
            aead::committed_key(&key, &msg_context(&bundle.origin, &self.node_id, conv_id, dp.epoch, dp.index));
        aead::open(&bound, &bundle.canonical_aead_ad(), &dp.ct).ok()
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
        // The outer direct bundle authenticates `from`; the body member is
        // merely an assertion and must be exactly that authenticated sender.
        // Without this binding, a peer could inject or relay a different
        // member's symmetric chain seed and reset receiver state.
        if member != *from || dist.epoch == 0 {
            return;
        }
        let me = self.node_id;
        let created = !self.groups.contains_key(&gid);
        if created {
            if self.groups.len() >= MAX_GROUPS {
                return;
            }
            // Bootstrap is TOFU in the current sender-key wire format: only
            // epoch one may create a local group, and the authenticated
            // inviter becomes the initial remote member. A future protocol
            // revision must carry the admin-signed membership proof required
            // by doc 07 before this admission can be made stronger.
            if dist.epoch != 1 {
                return;
            }
            let mut gs = GroupState::create(&mut self.rng, gid, me, false);
            gs.add_member(*from);
            if !gs.learn_remote(*from, &dist) {
                return;
            }
            self.groups.insert(gid, gs);
            self.enqueue_event(Event::GroupInvite { gid, from: *from });
        } else {
            let Some(gs) = self.groups.get_mut(&gid) else { return };
            // A removed/non-member peer cannot reintroduce sender-key state.
            if !gs.members.contains(from) || dist.epoch < gs.epoch {
                return;
            }
            if !gs.learn_remote(*from, &dist) {
                return;
            }
        }
        // Reciprocate our own sender key so the sender can read us.
        let _ = self.send_own_dist(&gid, from);
    }

    fn deliver_group(&mut self, bundle: &Bundle) {
        let Ok(gp) = GroupPayload::decode(&bundle.payload) else { return };
        let Some(gs) = self.groups.get_mut(&gp.gid) else { return };
        let Some(remote) = gs.remotes.get_mut(&(gp.member, gp.epoch)) else { return };
        let Ok(body) = remote.decrypt(&gp, &bundle.canonical_aead_ad()) else { return };
        let (_, msg) =
            body.split_first().map(|(a, r)| (*a, r.to_vec())).unwrap_or((apptype::TEXT, Vec::new()));
        self.enqueue_event(Event::GroupMessage {
            gid: gp.gid,
            from: gp.member,
            id: bundle.id(),
            body: msg,
        });
    }

    fn emit_receipt(&mut self, conv_id: &Id16, delivered: &Id16, origin: &Key32) {
        let Some(conv) = self.convs.get(conv_id) else { return };
        let mac = receipt::receipt_mac(&conv.receipt_key, delivered);
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
                if self.store.remove(&rp.delivered) {
                    self.deindex_symbol(&rp.delivered);
                }
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
        let sent = self.sent.get(&rp.delivered).copied();
        let pending_conv = self
            .pending_first_contact
            .iter()
            .find_map(|(conv, first)| (*first == rp.delivered).then_some(*conv));
        let Some(conv_id) = sent.map(|message| message.conv).or(pending_conv) else { return };
        let Some(conv) = self.convs.get(&conv_id) else { return };
        if !receipt::mac_eq(&rp.mac, &receipt::receipt_mac(&conv.receipt_key, &rp.delivered)) {
            return;
        }
        if self.pending_first_contact.get(&conv_id) == Some(&rp.delivered) {
            self.pending_first_contact.remove(&conv_id);
        }
        if let Some(m) = self.sent.get_mut(&rp.delivered) {
            m.state = DeliveryState::Delivered;
        }
        if sent.is_none() {
            return;
        }
        self.enqueue_event(Event::Delivered { id: rp.delivered });
        self.enqueue_event(Event::DeliveryProgress {
            id: rp.delivered,
            state: DeliveryState::Delivered,
        });
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
            // Snapshot checksums detect accidental corruption, not an
            // attacker who can rewrite the local image.  Re-apply the same
            // canonical/signature/ID and metadata binding used on wire before
            // any restored bundle can enter a routing or blob index.
            if !stored_bundle_is_valid(&meta, &bytes) {
                self.store.remove(&meta.id);
                self.deindex_symbol(&meta.id);
                continue;
            }
            let Ok(bundle) = Bundle::decode_canonical(&bytes) else { continue };
            let Some((root, sym_no)) = self.index_symbol_bundle(&bundle, meta.id) else { continue };
            // A valid bundle signature does not prove that a class-4 payload
            // is a symbol for the advertised Merkle root.  Do this check while
            // rebuilding the holder index so a tampered-but-re-signed local
            // record cannot become servable after restart.
            if !symbol_payload_verifies(&bundle, &root, sym_no) {
                self.store.remove(&meta.id);
                self.deindex_symbol(&meta.id);
            }
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
        if let Some(previous) = self.blob_index.get(&root).and_then(|idx| idx.get(&sym_no)).copied() {
            if previous != id && self.symbol_of.get(&previous) == Some(&(root, sym_no)) {
                self.symbol_of.remove(&previous);
            }
        }
        self.blob_index.entry(root).or_default().insert(sym_no, id);
        self.symbol_of.insert(id, (root, sym_no));
        Some((root, sym_no))
    }

    /// Remove `id` from both blob indices — used on Merkle-verification
    /// failure and index prune (expiry/eviction).
    fn deindex_symbol(&mut self, id: &Id16) {
        if let Some((root, sym_no)) = self.symbol_of.remove(id) {
            if let Some(idx) = self.blob_index.get_mut(&root) {
                if idx.get(&sym_no) == Some(id) {
                    idx.remove(&sym_no);
                }
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
    /// `Abandoned`; terminal progress is retained for one further interest
    /// window, then only the in-memory assembly is reclaimed. Partial symbols
    /// stay servable via `blob_index`.
    fn sweep_blob_ttl(&mut self, now_ms: u64) {
        let mut failed = Vec::new();
        let mut collect_terminal = Vec::new();
        for (root, a) in self.assemblies.iter_mut() {
            if matches!(a.state, BlobState::Wanted | BlobState::Fetching)
                && now_ms.saturating_sub(a.started_at) > BLOB_INTEREST_TTL_MS
            {
                a.state = BlobState::Abandoned;
                a.terminal_at = Some(now_ms);
                failed.push(*root);
            } else if matches!(a.state, BlobState::Assembled | BlobState::Abandoned) {
                // Assemblies are runtime UI/progress state, not durable blob
                // inventory. Keep terminal progress visible for one more
                // interest window, then reclaim only this entry. Partial
                // symbols remain in `blob_index`/the store and therefore keep
                // their normal holder-side serving behavior.
                match a.terminal_at {
                    None => {
                        // Defensive repair for a terminal state created by an
                        // older in-memory layout or a future transition that
                        // did not stamp its timestamp: start retention now
                        // rather than deleting progress immediately.
                        a.terminal_at = Some(now_ms);
                    }
                    Some(terminal_at)
                        if now_ms.saturating_sub(terminal_at) >= BLOB_INTEREST_TTL_MS =>
                    {
                        collect_terminal.push(*root);
                    }
                    Some(_) => {}
                }
            }
        }
        for root in failed {
            self.enqueue_event(Event::FileFailed { root, reason: FileFailReason::Abandoned });
        }
        for root in collect_terminal {
            self.assemblies.remove(&root);
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
        // Match `send_app`'s revocation gate before minting any class-4
        // symbols. A manifest send to a revoked identity must not leave
        // durable blob bytes or holder-index entries behind after the final
        // application-send preflight rejects it.
        if self.claims.revocation(peer).is_some() {
            return Err(SendError::PeerRevoked);
        }
        // Check the application-send admission boundary before minting any
        // content-addressed symbol bundles. A rejected first-contact send
        // must be side-effect free: otherwise ConversationPending or an
        // unknown peer would still consume local storage and quota.
        let peer_pub = self.resolve_peer_key(peer).ok_or(SendError::UnknownPeer)?;
        let _peer_exchange = self.resolve_peer_exchange(peer).ok_or(SendError::UnknownPeer)?;
        let conv_id = conversation_id(&self.public, &peer_pub);
        if self.pending_first_contact.contains_key(&conv_id) {
            return Err(SendError::ConversationPending);
        }
        if !self.conversation_capacity_available(&conv_id) {
            // Do this before RS/Merkle expansion or minting any symbol
            // bundles: local admission pressure must leave the send fully
            // side-effect-free, just like ConversationPending.
            return Err(SendError::ResourceLimit);
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
                let id = bundle.id();
                self.index_symbol_bundle(&bundle, id);
                self.store_own(bundle, budget);
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
        self.enqueue_event(Event::FileOffered {
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
                a.terminal_at = Some(self.now);
                self.enqueue_event(Event::FileFailed { root: *root, reason: FileFailReason::Abandoned });
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

        // Verify every symbol before it becomes servable, even when no local
        // assembly currently requests it.  `index_symbol_bundle` runs for all
        // class-4 traffic so relays can retain opaque symbols, but an
        // unverified shape-valid payload must not enter `blob_index`: a later
        // BLOB_HAVE/BLOB_REQ would otherwise hand the bytes to another peer
        // through `load_for_handoff` without a second Merkle check.
        if !symbol_payload_verifies(bundle, &root, sym_no) {
            self.store.remove(&id);
            self.deindex_symbol(&id);
            if let Some(a) = self.assemblies.get_mut(&root) {
                if matches!(a.state, BlobState::Wanted | BlobState::Fetching) {
                    a.bad_symbols = a.bad_symbols.saturating_add(1);
                }
            }
            return;
        }

        let Some(assembly) = self.assemblies.get(&root) else { return };
        if !matches!(assembly.state, BlobState::Wanted | BlobState::Fetching) || assembly.have_bit(sym_no) {
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
                    a.terminal_at = Some(self.now);
                }
                if renamed {
                    self.enqueue_event(Event::FileReady { root: *root, path: dest_path });
                } else {
                    self.enqueue_event(Event::FileFailed {
                        root: *root,
                        reason: FileFailReason::Abandoned,
                    });
                }
            }
            Ok(false) => {
                let _ = std::fs::remove_file(&part_path);
                if let Some(a) = self.assemblies.get_mut(root) {
                    a.state = BlobState::Abandoned;
                    a.terminal_at = Some(self.now);
                }
                self.enqueue_event(Event::FileFailed {
                    root: *root,
                    reason: FileFailReason::HashMismatch,
                });
            }
            Err(_) => {
                if let Some(a) = self.assemblies.get_mut(root) {
                    a.state = BlobState::Abandoned;
                    a.terminal_at = Some(self.now);
                }
                self.enqueue_event(Event::FileFailed {
                    root: *root,
                    reason: FileFailReason::Abandoned,
                });
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
        let served = match self.blob_served.get(peer).copied() {
            Some(value) => value,
            None if self.blob_served.len() >= MAX_BLOB_SERVE_PEERS => return Vec::new(),
            None => 0,
        };
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
        // A request that yields no symbols consumes no per-contact serving
        // budget and must not reserve one of the bounded peer-counter slots.
        // Otherwise a Sybil stream can fill the table with zero-valued entries
        // and deny later peers service even though no blob work was done.
        if !out.is_empty() {
            self.blob_served.insert(*peer, served + out.len() as u32);
        }
        out
    }
}

/// D1 symbol verification (checks a+b): the Merkle path must connect the
/// symbol's hash to the SYMBOL_INFO root's 16-byte prefix, AND the path's own
/// proven leaf index must equal the SYMBOL_INFO-derived `sym_no` — closing the
/// forged-position replay hole a membership-only check would leave open.
fn stored_bundle_is_valid(meta: &BundleMeta, bytes: &[u8]) -> bool {
    if meta.size as usize != bytes.len() {
        return false;
    }
    let Ok(bundle) = Bundle::decode_canonical(bytes) else { return false };
    if bundle.id() != meta.id
        || bundle.class != meta.class
        || bundle.origin_node() != meta.origin
        || bundle.dest != meta.dest
        || bundle.priority != meta.priority
    {
        return false;
    }
    mmp_crypto::verify(SigDomain::Bundle, &bundle.origin, &bundle.canonical_zeroed(), &bundle.signature).is_ok()
}

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
        // The ordinary ingest path verifies before storing, but a checksum-
        // only local snapshot can be tampered with between process starts.
        // Revalidate before touching the lifetime bytes or emitting anything;
        // besides preventing unauthenticated forwarding this guards the fixed
        // offset slice below against truncated provider records.
        if !stored_bundle_is_valid(&meta, &bytes) {
            self.store.remove(id);
            self.deindex_symbol(id);
            return None;
        }
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
                    self.enqueue_event(Event::DeliveryProgress {
                        id: *id,
                        state: DeliveryState::InMesh,
                    });
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
        let conv_id = conversation_id(&self.public, identity);
        // Conversation and "met before" marker state has no wire lifetime.
        // Refuse a new authenticated contact once either local table is full;
        // this keeps a Sybil stream from creating one permanent entry per
        // identity. Existing conversation state remains usable, while the
        // host can recover capacity through its normal state/restart policy.
        if !self.convs.contains_key(&conv_id)
            && (!self.established_convs.contains(&conv_id)
                && (self.convs.len() >= MAX_CONVERSATION_STATE
                    || self.established_convs.len() >= MAX_CONVERSATION_STATE))
        {
            return false;
        }
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
        self.enqueue_event(Event::FriendshipEstablished { peer: *peer, since: offer.since });
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
        self.enqueue_event(Event::FriendshipEstablished { peer: *peer, since });
    }

    fn on_proxy_summary(&mut self, peer: &NodeId, body: &[u8]) {
        let Ok(summary) = ProxySummaryBody::decode(body) else { return };
        if !self.proxy_hints.contains_key(peer) && self.proxy_hints.len() >= MAX_PROXY_HINT_PEERS {
            if let Some(evict) = self.proxy_hints.keys().next().copied() {
                self.proxy_hints.remove(&evict);
            }
        }
        let dependents = self.proxy_hints.entry(*peer).or_default();
        if dependents.len() < MAX_PROXY_HINTS_PER_PEER {
            dependents.insert(summary.dependent);
        }
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
