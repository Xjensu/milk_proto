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
mod conv;
pub mod diag;
mod group;

use std::collections::{HashMap, HashSet, VecDeque};

use rand::rngs::StdRng;
use rand::SeedableRng;

use mmp_core::authority::{effective_class5_priority, AuthorityStore};
use mmp_core::ingest::{ingest, Disposition};
use mmp_core::store::{BundleMeta, BundleStore, SlotState};
use mmp_core::{
    ClaimStore, IdSource, ReceiptFilter, SeenIndex, SessionEngine, SessionEvent, SessionHost,
};
use mmp_crypto::group::SenderKeyDist;
use mmp_crypto::prekey::{recipient_agree, sender_agree, PrekeyStore};
use mmp_crypto::{aead, conversation_id, ephem_id, receipt, Identity};
use mmp_router::{
    eviction_order, handoff_decision, origin_budget, Handoff, OriginBuckets, UtilityState,
};
use mmp_types::authcert::{scope_id_of, AnnouncementBody, AuthCert, RevocationList};
use mmp_types::bundle::{
    class, prio, Bundle, FLAG_ASYNC_KEYS, FLAG_HAS_PARENTS, FLAG_WANTS_RECEIPT, LIFETIME_OFFSET,
};
use mmp_types::claim::{field, Claim, PrekeyBatch};
use mmp_types::consts::{
    EPHEM_SLOT_MS, LIFETIME_CONTROL_MS, LIFETIME_MESSAGE_MS, L_RECEIPT, MAX_BUNDLE,
    OFFER_MIN_LIFETIME_MS, PREKEY_BATCH, QUOTA_ANCHOR, QUOTA_STANDARD,
};
use mmp_types::domain::SigDomain;
use mmp_types::frame::{ClaimDigestEntry, Hello, Summary};
use mmp_types::ids::{node_id_of, Id16, Key32, NodeId, Sig64};
use mmp_types::payload::{apptype, AsyncPayload, DirectPayload, GroupPayload, ReceiptPayload};
use mmp_types::tlv::{TLV_AUTHCERT_ATTACH, TLV_SCOPE_INFO};
use mmp_types::{Beacon, Ext};

use authority_bucket::AuthorityBuckets;
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
        let core = NodeCore {
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
        };
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
    /// it as a class-3 bundle.
    pub fn publish_claim(&mut self, field_id: u8, seq: u32, body: Vec<u8>) {
        let mut claim = Claim { owner: self.core.public, field: field_id, seq, body, sig: Sig64::ZERO };
        claim.sig = self.identity.sign(SigDomain::Claim, &claim.signed_object());
        self.core.learn_claim(claim.clone(), true);
        self.core.gossip_claim(&claim);
    }

    pub fn set_nickname(&mut self, name: &str, seq: u32) {
        self.publish_claim(field::NICKNAME, seq, name.as_bytes().to_vec());
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
        for id in self.core.store.expire(now_ms) {
            self.core.diag.log(id, JourneyEvent::Expired, now_ms);
            if let Some(m) = self.core.sent.get_mut(&id) {
                if m.state != DeliveryState::Delivered {
                    m.state = DeliveryState::Expired;
                    self.core
                        .events
                        .push_back(Event::DeliveryProgress { id, state: DeliveryState::Expired });
                }
            }
        }
        self.core.seen.rotate(now_ms);
        self.core.receipts.rotate(now_ms);
        self.core.claims.expire(now_ms);
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
                    _ => {}
                }
            }
            mmp_core::ClaimMerge::Conflict { .. } => {
                self.events.push_back(Event::CompromiseWarning { owner });
            }
            mmp_core::ClaimMerge::Stale => {}
        }
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

    /// Quota eviction (doc 04 §2).
    fn enforce_quota(&mut self) {
        while self.store.total_bytes() > self.quota {
            let metas = self.store.all_meta();
            let order = eviction_order(&metas, |id| self.receipts.signal(id), self.now);
            let Some(victim) = order.first().copied() else { break };
            if !self.store.remove(&victim) {
                break;
            }
            self.diag.log(victim, JourneyEvent::Evicted, self.now);
        }
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
        let u_peer = if peer_is_dest { 1.0 } else { 0.0 };
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
        let (disposition, bundle) = {
            let buckets = &mut self.buckets;
            let blocklist = &self.blocklist;
            let utility = &self.utility;
            let mut admission = |b: &Bundle, origin: &NodeId| {
                if blocklist.contains(origin) {
                    return false;
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
        let _ = identity;
        true
    }
}

#[cfg(test)]
mod tests;
