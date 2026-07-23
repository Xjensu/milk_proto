//! Core exit tests (HANDOFF §2.3): ingest invariants, SeenIndex resurrection
//! resistance, RBSR convergence, LWW/OR-Set properties, ReceiptFilter
//! hint-vs-purge asymmetry, and a full two-engine session sync.

use alloc::collections::BTreeSet;
use alloc::vec;
use alloc::vec::Vec;

use mmp_crypto::Identity;
use mmp_types::bundle::{class, zero_lifetime, Bundle, LIFETIME_OFFSET};
use mmp_types::claim::Claim;
use mmp_types::consts::{class_max_lifetime_ms, L_MAX, LIFETIME_MESSAGE_MS};
use mmp_types::domain::SigDomain;
use mmp_types::frame::{Hello, Summary};
use mmp_types::ids::{Id16, Key32, NodeId, Sig64};

use crate::claims::{ClaimMerge, ClaimStore};
use crate::ingest::{ingest, Disposition, DropReason};
use crate::orset::{MembershipOp, OrSet};
use crate::rbsr::{fingerprint, IdSource, Reconciler};
use crate::receiptfilter::{ReceiptFilter, SuppressSignal};
use crate::seen::SeenIndex;
use crate::session::{SessionEngine, SessionEvent, SessionHost};
use crate::store::{BundleMeta, BundleStore, MemStore, SlotState};

fn identity(seed: u8) -> Identity {
    Identity::from_seed(&[seed; 32])
}

/// A signed test bundle from `origin` to `dest`.
fn make_bundle(origin: &Identity, dest: Id16, payload: &[u8]) -> Bundle {
    let mut b = Bundle {
        class: class::DIRECT,
        flags: 0,
        origin: origin.public(),
        dest,
        lifetime_ms: LIFETIME_MESSAGE_MS,
        priority: 1,
        exts: vec![],
        payload: payload.to_vec(),
        signature: Sig64::ZERO,
    };
    b.signature = origin.sign(SigDomain::Bundle, &b.canonical_zeroed());
    b
}

fn accept_all(_b: &Bundle, _n: &NodeId) -> bool {
    true
}

#[test]
fn ingest_stores_verified_and_drops_bad_signature() {
    let alice = identity(1);
    let me = Id16([9; 16]);
    let mut seen = SeenIndex::new();
    let mut store = MemStore::new();
    let b = make_bundle(&alice, me, b"hi");
    let bytes = b.encode();

    let (d, decoded) = ingest(&bytes, 1000, &me, &mut seen, &mut store, &mut accept_all);
    assert_eq!(d, Disposition::Stored { to_self: true });
    assert_eq!(decoded.unwrap().payload, b"hi");
    assert_eq!(store.len(), 1);

    // Corrupt one payload byte → signature fails → dropped, never stored (I1).
    let mut bad = make_bundle(&alice, me, b"hi2").encode();
    let plen = bad.len();
    bad[plen - 70] ^= 1;
    let (d, _) = ingest(&bad, 1000, &me, &mut seen, &mut store, &mut accept_all);
    assert!(matches!(
        d,
        Disposition::Dropped(DropReason::BadSignature) | Disposition::Dropped(DropReason::NonCanonical)
    ));
    assert_eq!(store.len(), 1);
}

#[test]
fn seen_index_defeats_lifetime_resurrection() {
    // AUDIT F1 end-to-end: reset lifetime to class max, re-inject → Duplicate.
    let alice = identity(1);
    let me = Id16([9; 16]);
    let mut seen = SeenIndex::new();
    let mut store = MemStore::new();
    let b = make_bundle(&alice, Id16([7; 16]), b"replay-me");
    let bytes = b.encode();
    let (d, _) = ingest(&bytes, 1000, &me, &mut seen, &mut store, &mut accept_all);
    assert!(matches!(d, Disposition::Stored { .. }));

    // Attacker "resurrects": lifetime bytes reset to max, same signature.
    let mut resurrected = bytes.clone();
    resurrected[LIFETIME_OFFSET..LIFETIME_OFFSET + 5]
        .copy_from_slice(&class_max_lifetime_ms(class::DIRECT).to_le_bytes()[..5]);
    let (d, _) = ingest(&resurrected, 2000, &me, &mut seen, &mut store, &mut accept_all);
    assert_eq!(d, Disposition::Duplicate);

    // And the SeenIndex entry persists from FIRST sight even after the store
    // dropped the bundle.
    store.remove(&b.id());
    let (d, _) = ingest(&resurrected, 3000, &me, &mut seen, &mut store, &mut accept_all);
    assert_eq!(d, Disposition::Duplicate);
}

#[test]
fn lifetime_clamped_on_receive() {
    let alice = identity(1);
    let me = Id16([9; 16]);
    let mut seen = SeenIndex::new();
    let mut store = MemStore::new();
    let mut b = make_bundle(&alice, Id16([7; 16]), b"zombie");
    b.lifetime_ms = (1u64 << 40) - 1; // absurd claimed lifetime
    b.signature = alice.sign(SigDomain::Bundle, &b.canonical_zeroed());
    let (d, _) = ingest(&b.encode(), 1000, &me, &mut seen, &mut store, &mut accept_all);
    assert!(matches!(d, Disposition::Stored { .. }));
    let meta = store.meta(&b.id()).unwrap();
    assert_eq!(meta.expires_at, 1000 + class_max_lifetime_ms(class::DIRECT));
}

#[test]
fn receipt_filter_hint_never_purges() {
    // AUDIT F11: Bloom = hint; exact receipt = purge.
    let mut rf = ReceiptFilter::new();
    let id = Id16([1; 16]);
    assert_eq!(rf.signal(&id), SuppressSignal::None);
    // A peer bucket containing the id (simulated by our own insert to another
    // filter, then union).
    let mut other = ReceiptFilter::new();
    other.note_verified_receipt(&id, 5000);
    let buckets = other.summary_buckets(1);
    rf.union_from_peer(buckets[0].bucket_epoch, &buckets[0].bloom, 5000);
    assert_eq!(rf.signal(&id), SuppressSignal::Hint); // deprioritize only
    rf.note_verified_receipt(&id, 6000);
    assert_eq!(rf.signal(&id), SuppressSignal::Purge); // exact D0 receipt
}

/// E4-S4 persistence completion (handbook §4.3 step 3): a `SeenIndex`
/// surviving restart must still recognize IDs it saw before — the whole
/// point of persisting it, per doc 04 §7's "prevent post-restart duplicate
/// delivery" acceptance.
#[test]
fn seen_index_survives_encode_decode_roundtrip() {
    let mut seen = SeenIndex::new();
    let hot = Id16([1; 16]); // stays in the exact ring
    let bucketed = Id16([2; 16]);
    seen.insert(&hot, class::DIRECT, 1000);
    seen.insert(&bucketed, class::CLAIM, 1000); // a different class-max bucket

    let bytes = seen.encode();
    let restored = SeenIndex::decode(&bytes).expect("well-formed encoding must decode");

    assert!(restored.contains(&hot));
    assert!(restored.contains(&bucketed));
    assert!(!restored.contains(&Id16([9; 16])));
}

#[test]
fn seen_index_decode_rejects_truncated_and_trailing_bytes() {
    let mut seen = SeenIndex::new();
    seen.insert(&Id16([1; 16]), class::DIRECT, 1000);
    let bytes = seen.encode();

    assert!(SeenIndex::decode(&bytes[..bytes.len() - 1]).is_none(), "truncated tail must not decode");
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(SeenIndex::decode(&trailing).is_none(), "trailing bytes must not decode");
}

/// Mirrors `seen_index_survives_encode_decode_roundtrip` for `ReceiptFilter`:
/// both the Bloom-hint buckets and the exact-purge map must survive restart,
/// preserving the hint-vs-purge asymmetry `receipt_filter_hint_never_purges`
/// establishes.
#[test]
fn receipt_filter_survives_encode_decode_roundtrip() {
    let mut rf = ReceiptFilter::new();
    let hinted = Id16([1; 16]);
    let purged = Id16([2; 16]);

    let mut other = ReceiptFilter::new();
    other.note_verified_receipt(&hinted, 5000);
    let buckets = other.summary_buckets(1);
    rf.union_from_peer(buckets[0].bucket_epoch, &buckets[0].bloom, 5000);
    rf.note_verified_receipt(&purged, 6000);

    let bytes = rf.encode();
    let restored = ReceiptFilter::decode(&bytes).expect("well-formed encoding must decode");

    assert_eq!(restored.signal(&hinted), SuppressSignal::Hint);
    assert_eq!(restored.signal(&purged), SuppressSignal::Purge);
    assert_eq!(restored.signal(&Id16([9; 16])), SuppressSignal::None);
}

#[test]
fn receipt_filter_decode_rejects_truncated_and_trailing_bytes() {
    let mut rf = ReceiptFilter::new();
    rf.note_verified_receipt(&Id16([1; 16]), 5000);
    let bytes = rf.encode();

    assert!(ReceiptFilter::decode(&bytes[..bytes.len() - 1]).is_none(), "truncated tail must not decode");
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(ReceiptFilter::decode(&trailing).is_none(), "trailing bytes must not decode");
}

#[test]
fn claims_lww_merge_is_deterministic_and_flags_conflicts() {
    let alice = identity(1);
    let mk = |seq: u32, body: &[u8]| {
        let mut c = Claim {
            owner: alice.public(),
            field: 0x01,
            seq,
            body: body.to_vec(),
            sig: Sig64::ZERO,
        };
        c.sig = alice.sign(SigDomain::Claim, &c.signed_object());
        c
    };
    let mut s1 = ClaimStore::new();
    let mut s2 = ClaimStore::new();
    let a = mk(1, b"alice");
    let b = mk(2, b"alicia");
    let c = mk(2, b"evil-alicia"); // equal-seq conflict

    // Order-independence (commutativity).
    assert_eq!(s1.merge(a.clone(), 0, false), ClaimMerge::Adopted);
    assert_eq!(s1.merge(b.clone(), 0, false), ClaimMerge::Adopted);
    let r1 = s1.merge(c.clone(), 0, false);
    s2.merge(c.clone(), 0, false);
    s2.merge(a.clone(), 0, false);
    let _ = s2.merge(b.clone(), 0, false);
    let n = mmp_types::node_id_of(&alice.public());
    assert_eq!(s1.get(&n, 0x01).unwrap().body, s2.get(&n, 0x01).unwrap().body);
    assert!(matches!(r1, ClaimMerge::Conflict { .. }));
    // Idempotence: re-merging the currently adopted claim is Stale.
    let adopted = s1.get(&n, 0x01).unwrap().clone();
    assert_eq!(s1.merge(adopted, 0, false), ClaimMerge::Stale);
}

#[test]
fn orset_converges_and_compacts() {
    let m1 = Id16([1; 16]);
    let m2 = Id16([2; 16]);
    let ops = [
        MembershipOp::Add { member: m1, tag: 10 },
        MembershipOp::Add { member: m2, tag: 20 },
        MembershipOp::Remove { tags: vec![10] },
        MembershipOp::Add { member: m1, tag: 11 }, // concurrent re-add: add-wins
    ];
    // Any permutation converges.
    let mut sets: Vec<OrSet> = Vec::new();
    for perm in [[0usize, 1, 2, 3], [3, 2, 1, 0], [1, 3, 0, 2], [2, 0, 3, 1]] {
        let mut s = OrSet::new();
        for i in perm {
            s.apply(&ops[i]);
        }
        sets.push(s);
    }
    for s in &sets {
        assert_eq!(s.members(), vec![m1, m2]);
        assert_eq!(s.member_root(), sets[0].member_root());
    }
    // Anchor GC bounds state.
    let mut s = sets.pop().unwrap();
    let before = s.state_size();
    let root = s.member_root();
    s.compact_at_anchor(&root);
    assert!(s.state_size() <= before);
    assert_eq!(s.member_root(), root);
    assert_eq!(s.members(), vec![m1, m2]);
}

// ---------------------------------------------------------------- RBSR ------

struct VecSource {
    ids: Vec<Id16>, // sorted
}

impl IdSource for VecSource {
    fn ids_in(&mut self, _domain: u8, lo: &Id16, hi: &Id16) -> Vec<Id16> {
        self.ids.iter().filter(|id| **id >= *lo && **id <= *hi).copied().collect()
    }
    fn has(&mut self, id: &Id16) -> bool {
        self.ids.binary_search(id).is_ok()
    }
}

/// Drive two reconcilers to completion; returns (a_missing, b_missing, rounds).
fn reconcile(a_ids: Vec<Id16>, b_ids: Vec<Id16>) -> (BTreeSet<Id16>, BTreeSet<Id16>, usize) {
    use mmp_types::frame::Frame;
    let mut a_src = VecSource { ids: a_ids };
    let mut b_src = VecSource { ids: b_ids };
    let mut a = Reconciler::new(3);
    let mut b = Reconciler::new(3);
    let mut to_b = vec![a.init_frame(&mut a_src)];
    let mut to_a: Vec<Frame> = Vec::new();
    let mut rounds = 0;
    while !to_b.is_empty() || !to_a.is_empty() {
        rounds += 1;
        assert!(rounds < 64, "reconciliation must terminate");
        let mut next_a = Vec::new();
        for f in to_b.drain(..) {
            let replies = match f {
                Frame::ReconInit { range, .. } => b.on_range(&range, false, &mut b_src),
                Frame::ReconRange { domain, ranges } => {
                    let is_reply = domain & 0x80 != 0;
                    ranges.iter().flat_map(|r| b.on_range(r, is_reply, &mut b_src)).collect()
                }
                Frame::ReconIds { lo, hi, ids, .. } => b.on_ids(&lo, &hi, &ids, &mut b_src),
                _ => unreachable!(),
            };
            next_a.extend(replies);
        }
        let mut next_b = Vec::new();
        for f in to_a.drain(..) {
            let replies = match f {
                Frame::ReconInit { range, .. } => a.on_range(&range, false, &mut a_src),
                Frame::ReconRange { domain, ranges } => {
                    let is_reply = domain & 0x80 != 0;
                    ranges.iter().flat_map(|r| a.on_range(r, is_reply, &mut a_src)).collect()
                }
                Frame::ReconIds { lo, hi, ids, .. } => a.on_ids(&lo, &hi, &ids, &mut a_src),
                _ => unreachable!(),
            };
            next_b.extend(replies);
        }
        to_a = next_a;
        to_b = next_b;
    }
    (a.missing.clone(), b.missing.clone(), rounds)
}

fn rand_ids(n: usize, seed: u64) -> Vec<Id16> {
    // Deterministic pseudo-IDs via BLAKE3 (uniform like real BundleIDs).
    let mut out: Vec<Id16> = (0..n)
        .map(|i| {
            let mut h = blake3::Hasher::new();
            h.update(&seed.to_le_bytes());
            h.update(&(i as u64).to_le_bytes());
            let mut id = [0u8; 16];
            id.copy_from_slice(&h.finalize().as_bytes()[..16]);
            Id16(id)
        })
        .collect();
    out.sort();
    out
}

#[test]
fn rbsr_converges_on_disjoint_and_overlapping_sets() {
    // Identical sets: no traffic beyond the echo.
    let ids = rand_ids(500, 1);
    let (ma, mb, _) = reconcile(ids.clone(), ids.clone());
    assert!(ma.is_empty() && mb.is_empty());

    // Overlapping sets.
    let a: Vec<Id16> = rand_ids(800, 2);
    let b: Vec<Id16> = rand_ids(800, 3);
    let mut union: Vec<Id16> = a.iter().chain(b.iter()).copied().collect();
    union.sort();
    union.dedup();
    let a_set: BTreeSet<Id16> = a.iter().copied().collect();
    let b_set: BTreeSet<Id16> = b.iter().copied().collect();
    let (a_missing, b_missing, rounds) = reconcile(a.clone(), b.clone());
    // After exchange, A's missing = B \ A exactly; symmetric for B.
    let expect_a: BTreeSet<Id16> = b_set.difference(&a_set).copied().collect();
    let expect_b: BTreeSet<Id16> = a_set.difference(&b_set).copied().collect();
    assert_eq!(a_missing, expect_a);
    assert_eq!(b_missing, expect_b);
    assert!(rounds <= 10, "took {rounds} rounds");

    // Small-vs-empty edge.
    let (ma, mb, _) = reconcile(vec![], rand_ids(10, 4));
    assert_eq!(ma.len(), 10);
    assert!(mb.is_empty());
}

#[test]
fn fingerprint_is_order_and_content_sensitive() {
    let ids = rand_ids(50, 7);
    let mut minus_one = ids.clone();
    minus_one.pop();
    assert_ne!(fingerprint(&ids), fingerprint(&minus_one));
    assert_ne!(fingerprint(&[]), fingerprint(&ids));
}

// -------------------------------------------------------- session e2e ------

/// Minimal host: MemStore + SeenIndex + spray-budget bookkeeping.
struct TestHost {
    node: NodeId,
    store: MemStore,
    seen: SeenIndex,
    src: HostSource,
    received: Vec<Id16>,
    budgets: Vec<(Id16, u8)>,
    now: u64,
}

struct HostSource {
    ids: Vec<Id16>,
}

impl IdSource for HostSource {
    fn ids_in(&mut self, _domain: u8, lo: &Id16, hi: &Id16) -> Vec<Id16> {
        self.ids.iter().filter(|id| **id >= *lo && **id <= *hi).copied().collect()
    }
    fn has(&mut self, id: &Id16) -> bool {
        self.ids.binary_search(id).is_ok()
    }
}

impl TestHost {
    fn new(node: NodeId) -> TestHost {
        TestHost {
            node,
            store: MemStore::new(),
            seen: SeenIndex::new(),
            src: HostSource { ids: Vec::new() },
            received: Vec::new(),
            budgets: Vec::new(),
            now: 1000,
        }
    }

    fn add_bundle(&mut self, b: &Bundle) {
        let bytes = b.encode();
        let meta = BundleMeta {
            id: b.id(),
            class: b.class,
            origin: b.origin_node(),
            dest: b.dest,
            priority: b.priority,
            effective_priority: b.priority,
            size: bytes.len() as u32,
            stored_at: self.now,
            expires_at: self.now + b.lifetime_ms,
            budget: 8,
            own_origin: false,
            slot: SlotState::Stored,
        };
        self.store.put(meta, &bytes);
        self.refresh();
    }

    fn refresh(&mut self) {
        self.src.ids = self.store.ids_range(&Id16::MIN, &Id16::MAX);
    }
}

impl SessionHost for TestHost {
    type Ids = HostSource;

    fn ids(&mut self, _peer: &NodeId) -> &mut HostSource {
        &mut self.src
    }
    fn summary(&mut self, _peer: &NodeId) -> Summary {
        Summary::default()
    }
    fn on_summary(&mut self, _peer: &NodeId, _s: Summary) {}
    fn load_for_handoff(&mut self, _peer: &NodeId, id: &Id16) -> Option<(Vec<u8>, u8)> {
        let (mut meta, mut bytes) = self.store.get(id)?;
        // Decrement lifetime by holding duration (I3) and persist hand-off intent.
        let remaining = meta.remaining_ms(self.now);
        zero_lifetime(&mut bytes);
        bytes[LIFETIME_OFFSET..LIFETIME_OFFSET + 5].copy_from_slice(&remaining.to_le_bytes()[..5]);
        let granted = meta.budget / 2;
        meta.budget -= granted;
        meta.slot = SlotState::HandoffPending;
        self.store.update_meta(meta);
        Some((bytes, granted.max(1)))
    }
    fn on_bundle(&mut self, _peer: &NodeId, bytes: &[u8]) {
        let node = self.node;
        let (d, b) =
            ingest(bytes, self.now, &node, &mut self.seen, &mut self.store, &mut accept_all);
        if let (Disposition::Stored { .. }, Some(b)) = (d, b) {
            self.received.push(b.id());
            self.refresh();
        }
    }
    fn on_budget(&mut self, _peer: &NodeId, id: &Id16, granted: u8) {
        assert!(granted <= L_MAX, "engine must clamp before the host sees it");
        self.budgets.push((*id, granted));
        if let Some(mut meta) = self.store.meta(id) {
            meta.budget = granted.min(L_MAX);
            self.store.update_meta(meta);
        }
    }
    fn wants(&mut self, _peer: &NodeId) -> Vec<Id16> {
        Vec::new()
    }
    fn accept_peer(&mut self, _identity: &Key32, _node: &NodeId) -> bool {
        true
    }
}

fn hello() -> Hello {
    Hello { ver: 1, suites: vec![0x01], tier: 2, flags: 0, credits: 8, psm: 0 }
}

fn hello_with_suites(suites: Vec<u8>) -> Hello {
    Hello { ver: 1, suites, tier: 2, flags: 0, credits: 8, psm: 0 }
}

/// Pump both engines until quiescent; returns pumped byte count.
fn pump(
    a: &mut SessionEngine,
    b: &mut SessionEngine,
    ia: &Identity,
    ib: &Identity,
    ha: &mut TestHost,
    hb: &mut TestHost,
) -> usize {
    let mut moved = 0;
    for _ in 0..200 {
        let mut quiet = true;
        while let Some(bytes) = a.poll_out() {
            moved += bytes.len();
            quiet = false;
            b.on_wire(&bytes, 1000, ib, hb);
        }
        while let Some(bytes) = b.poll_out() {
            moved += bytes.len();
            quiet = false;
            a.on_wire(&bytes, 1000, ia, ha);
        }
        if quiet {
            break;
        }
    }
    moved
}

#[test]
fn full_session_syncs_two_stores() {
    let ia = identity(1);
    let ib = identity(2);
    let origin = identity(3);
    let mut ha = TestHost::new(ia.node_id());
    let mut hb = TestHost::new(ib.node_id());

    // A holds 5 bundles, B holds 3 (one shared).
    let shared = make_bundle(&origin, Id16([50; 16]), b"shared");
    for i in 0..5u8 {
        ha.add_bundle(&make_bundle(&origin, Id16([50; 16]), &[b'a', i]));
    }
    ha.add_bundle(&shared);
    for i in 0..2u8 {
        hb.add_bundle(&make_bundle(&origin, Id16([50; 16]), &[b'b', i]));
    }
    hb.add_bundle(&shared);

    let mut a = SessionEngine::initiator(&ia, &hello(), 1000).unwrap();
    let mut b = SessionEngine::responder(&ib, &hello(), 1000).unwrap();
    pump(&mut a, &mut b, &ia, &ib, &mut ha, &mut hb);

    // Both established with proven identities.
    let ev_a: Vec<_> = core::iter::from_fn(|| a.poll_event()).collect();
    let ev_b: Vec<_> = core::iter::from_fn(|| b.poll_event()).collect();
    assert!(ev_a.iter().any(
        |e| matches!(e, SessionEvent::Established { identity, .. } if *identity == ib.public())
    ));
    assert!(ev_b.iter().any(
        |e| matches!(e, SessionEvent::Established { identity, .. } if *identity == ia.public())
    ));

    // Stores converged: 8 distinct bundles each.
    assert_eq!(ha.store.len(), 8, "A store");
    assert_eq!(hb.store.len(), 8, "B store");
    assert_eq!(hb.received.len(), 5);
    assert_eq!(ha.received.len(), 2);
    // Budget grants arrived and were clamped.
    assert!(!hb.budgets.is_empty());
    assert!(hb.budgets.iter().all(|(_, g)| *g <= L_MAX));
}

#[test]
fn handshake_timeout_closes_session() {
    let ia = identity(1);
    let mut a = SessionEngine::initiator(&ia, &hello(), 1000).unwrap();
    a.tick(1000 + 4_999);
    assert_ne!(a.phase(), crate::session::Phase::Closed);
    a.tick(1000 + 5_001);
    assert_eq!(a.phase(), crate::session::Phase::Closed);
}

#[test]
fn handshake_refuses_peer_with_no_common_cipher_suite() {
    // E6-S2 cipher-agility exercise (doc 03 §8): a build advertising only a
    // suite id this side does not implement must be refused end-to-end at
    // the session layer, with no Established event and no store access,
    // rather than silently proceeding under whatever primitives happen to
    // be compiled in.
    let ia = identity(1);
    let ib = identity(2);
    let mut ha = TestHost::new(ia.node_id());
    let mut hb = TestHost::new(ib.node_id());

    let mut a = SessionEngine::initiator(&ia, &hello_with_suites(vec![0x02]), 1000).unwrap();
    let mut b = SessionEngine::responder(&ib, &hello(), 1000).unwrap();
    pump(&mut a, &mut b, &ia, &ib, &mut ha, &mut hb);

    assert_eq!(b.phase(), crate::session::Phase::Closed);
    let ev_b: Vec<_> = core::iter::from_fn(|| b.poll_event()).collect();
    assert!(!ev_b.iter().any(|e| matches!(e, SessionEvent::Established { .. })));
}
