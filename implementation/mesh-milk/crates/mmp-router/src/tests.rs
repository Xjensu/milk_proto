//! Router exit tests (HANDOFF §2.4): budget conservation, quota exhaustion,
//! eviction-order property, transfer ordering, focus threshold.

use alloc::vec;

use mmp_core::receiptfilter::SuppressSignal;
use mmp_core::store::{BundleMeta, SlotState};
use mmp_types::consts::{CONTACT_GRAPH_CAP, L_MAX};
use mmp_types::ids::Id16;

use crate::quota::OriginBuckets;
use crate::utility::UtilityState;
use crate::{eviction_order, handoff_decision, origin_budget, spray_split, transfer_order, Handoff};

fn meta(id: u8, class: u8, priority: u8, size: u32, budget: u8, own: bool) -> BundleMeta {
    BundleMeta {
        id: Id16([id; 16]),
        class,
        origin: Id16([0xEE; 16]),
        dest: Id16([0xDD; 16]),
        priority,
        effective_priority: priority,
        size,
        stored_at: 0,
        expires_at: 1_000_000,
        budget,
        own_origin: own,
        slot: SlotState::Stored,
    }
}

#[test]
fn origin_budget_tracks_population() {
    assert_eq!(origin_budget(0.0), 4);
    assert_eq!(origin_budget(100.0), 14);
    assert_eq!(origin_budget(1000.0), 20);
    assert_eq!(origin_budget(1_000_000.0), L_MAX); // cap binds
}

#[test]
fn spray_budget_is_conserved_over_arbitrary_trees() {
    // Σ budgets over all copies ≤ L after any spray tree (HANDOFF §2.4).
    fn spray_tree(budget: u8, depth: u8) -> u32 {
        if budget <= 1 || depth == 0 {
            return u32::from(budget);
        }
        let (give, keep) = spray_split(budget);
        assert_eq!(give + keep, budget);
        spray_tree(give, depth - 1) + spray_tree(keep, depth - 1)
    }
    for l in 1..=L_MAX {
        assert_eq!(spray_tree(l, 8), u32::from(l));
    }
}

#[test]
fn handoff_rules() {
    let peer = Id16([7; 16]);
    let now = 0;
    let m = meta(1, 1, 1, 100, 8, false);
    // Direct delivery always wins.
    assert_eq!(
        handoff_decision(&m, &peer, 0.9, 0.0, true, false, SuppressSignal::None, now),
        Handoff::Deliver
    );
    // Spray splits binary.
    assert_eq!(
        handoff_decision(&m, &peer, 0.2, 0.1, false, false, SuppressSignal::None, now),
        Handoff::Spray { grant: 4, keep: 4 }
    );
    // Strictly worse carrier (beyond θ_spray) is skipped.
    assert_eq!(
        handoff_decision(&m, &peer, 0.9, 0.1, false, false, SuppressSignal::None, now),
        Handoff::Keep
    );
    // Focus at budget 1 needs a decisively better peer.
    let m1 = meta(2, 1, 1, 100, 1, false);
    assert_eq!(
        handoff_decision(&m1, &peer, 0.5, 0.7, false, false, SuppressSignal::None, now),
        Handoff::Focus
    );
    assert_eq!(
        handoff_decision(&m1, &peer, 0.5, 0.6, false, false, SuppressSignal::None, now),
        Handoff::Keep
    );
    // Receipt hints suppress offers (never purge — that's the store's job).
    assert_eq!(
        handoff_decision(&m, &peer, 0.2, 0.2, false, false, SuppressSignal::Hint, now),
        Handoff::Keep
    );
}

#[test]
fn zero_budget_non_destination_is_not_offered_again() {
    let peer = Id16([7; 16]);
    let zero = meta(3, 1, 1, 100, 0, false);

    assert_eq!(
        handoff_decision(&zero, &peer, 0.1, 0.9, false, false, SuppressSignal::None, 0),
        Handoff::Keep,
        "a focused zero-budget copy must not mint another grant"
    );
}

#[test]
fn zero_budget_still_delivers_to_destination() {
    let destination = Id16([0xDD; 16]);
    let zero = meta(4, 1, 1, 100, 0, false);

    assert_eq!(
        handoff_decision(&zero, &destination, 0.1, 0.0, true, false, SuppressSignal::None, 0),
        Handoff::Deliver,
        "direct delivery must remain unconditional even after focus"
    );
}

#[test]
fn transfer_order_is_value_dense() {
    let peer = Id16([0xDD; 16]); // matches dest of meta()
    let metas = vec![
        meta(1, 1, 1, 500, 4, false),  // spray, mid
        meta(2, 0, 0, 50, 4, false),   // control → first
        meta(3, 1, 0, 100, 4, false),  // dest == peer → second
        meta(4, 1, 3, 100, 4, false),  // spray, low priority → last-ish
        meta(5, 1, 1, 100, 4, false),  // spray, smaller than #1
    ];
    let order = transfer_order(&metas, &peer, 0);
    assert_eq!(order[0], 1); // class 0
    assert_eq!(order[1], 2); // destination-here
    // Among spray: priority asc then size asc → #5 (100B, p1) before #1 (500B, p1), then #4 (p3).
    assert_eq!(&order[2..], &[4, 0, 3]);
}

#[test]
fn eviction_protects_own_originals_and_prefers_confirmed() {
    let confirmed = Id16([9; 16]);
    let metas = vec![
        meta(1, 1, 1, 100, 4, true),  // own original — never in the list
        meta(2, 1, 3, 900, 4, false), // big, low priority
        {
            let mut m = meta(9, 1, 0, 10, 4, false);
            m.id = confirmed;
            m
        },
    ];
    let order = eviction_order(
        &metas,
        |id| if *id == confirmed { SuppressSignal::Purge } else { SuppressSignal::None },
        0,
    );
    assert_eq!(order[0], confirmed); // receipt-confirmed evicts first
    assert!(!order.contains(&Id16([1; 16]))); // I4
}

#[test]
fn transfer_order_uses_declared_priority_eviction_order_uses_effective() {
    // Doc 18 §5.2.2: a class-5 bundle locally clamped to NORMAL (effective=2)
    // but declared OPERATIONAL (priority=1) must still sort by its declared
    // byte in transfer_order (wire-phase ordering, frozen), while
    // eviction_order (local store policy) sees the clamp.
    let peer = Id16([0xDD; 16]);
    let mut clamped = meta(1, 5, 1, 100, 4, false);
    clamped.effective_priority = 2; // clamped to NORMAL
    let elevated = meta(2, 5, 1, 100, 4, false); // priority == effective_priority == 1
    let metas = vec![clamped, elevated];

    // transfer_order: both declare priority 1 → tie broken by size/lifetime,
    // never by effective_priority (they'd differ if it were consulted).
    let t_order = transfer_order(&metas, &peer, 0);
    assert_eq!(metas[t_order[0]].priority, metas[t_order[1]].priority);

    // eviction_order: the clamped bundle (effective=2, worse) evicts before
    // the elevated one (effective=1) under pressure.
    let e_order = eviction_order(&metas, |_| SuppressSignal::None, 0);
    assert_eq!(e_order[0], Id16([1; 16]), "clamped (effective NORMAL) evicts first");
    assert_eq!(e_order[1], Id16([2; 16]));
}

#[test]
fn token_bucket_bounds_an_origin() {
    let mut q = OriginBuckets::new();
    let origin = Id16([1; 16]);
    // Burst = 128 KiB: admit until exhausted.
    let mut admitted = 0u64;
    while q.admit(&origin, 4096, 0, false) {
        admitted += 4096;
        assert!(admitted <= 128 * 1024);
    }
    assert_eq!(admitted, 128 * 1024);
    // Refill after an hour: 32 KiB more.
    let mut hour_admitted = 0u64;
    while q.admit(&origin, 4096, 3_600_000, false) {
        hour_admitted += 4096;
    }
    assert_eq!(hour_admitted, 32 * 1024);
    // WoT multiplier ×4.
    let friend = Id16([2; 16]);
    let mut wot_admitted = 0u64;
    while q.admit(&friend, 4096, 0, true) {
        wot_admitted += 4096;
    }
    assert_eq!(wot_admitted, 4 * 128 * 1024);
}

#[test]
fn utility_grows_with_contacts_and_ages() {
    let mut u = UtilityState::new();
    let peer = Id16([3; 16]);
    assert_eq!(u.utility(&peer, 0), 0.0);
    u.note_contact(&peer, 10_000, 0);
    let u0 = u.utility(&peer, 0);
    assert!(u0 >= 0.5, "first contact sets P ≈ 0.5, got {u0}");
    u.note_contact(&peer, 10_000, 1000);
    assert!(u.utility(&peer, 1000) > u0);
    // Aging decays over days.
    let later = 14 * 24 * 3_600_000;
    assert!(u.utility(&peer, later) < u.utility(&peer, 2000));
    // N̂ counts distinct peers + discounted ephems.
    u.note_ephem([1; 8], 0);
    u.note_ephem([1; 8], 10);
    u.note_ephem([2; 8], 20);
    let n = u.n_hat(30);
    assert!(n > 1.0 && n < 4.0, "n_hat = {n}");
}

#[test]
fn utility_ephem_window_is_bounded_under_beacon_flood() {
    let mut u = UtilityState::new();
    // Beacon ephem_ids are unauthenticated input.  Keep all events in one
    // horizon so time-based trimming alone cannot make the allocation finite.
    for i in 0u16..5000 {
        let mut ephem = [0u8; 8];
        ephem[..2].copy_from_slice(&i.to_be_bytes());
        u.note_ephem(ephem, 1);
    }

    // The implementation cap is the bounded N-hat window, discounted by the
    // unresolved-ephemeral factor.  In particular, this must not grow with
    // the number of hostile advertisements.
    let n = u.n_hat(1);
    assert!(n <= 0.75 * CONTACT_GRAPH_CAP as f32, "n_hat grew beyond bounded window: {n}");
}

#[test]
fn verified_contact_graph_stays_bounded_when_all_slots_are_trusted() {
    let mut u = UtilityState::new();
    for i in 0..CONTACT_GRAPH_CAP {
        let mut bytes = [0u8; 16];
        bytes[..8].copy_from_slice(&(i as u64).to_le_bytes());
        u.mark_verified(&Id16(bytes));
    }
    let retained = (0..CONTACT_GRAPH_CAP)
        .filter(|i| {
            let mut bytes = [0u8; 16];
            bytes[..8].copy_from_slice(&(*i as u64).to_le_bytes());
            u.contact(&Id16(bytes)).is_some()
        })
        .count();
    assert_eq!(retained, CONTACT_GRAPH_CAP);

    let overflow = Id16([0xf3; 16]);
    u.mark_verified(&overflow);
    let retained_after_overflow = (0..CONTACT_GRAPH_CAP)
        .filter(|i| {
            let mut bytes = [0u8; 16];
            bytes[..8].copy_from_slice(&(*i as u64).to_le_bytes());
            u.contact(&Id16(bytes)).is_some()
        })
        .count();
    assert_eq!(retained_after_overflow, CONTACT_GRAPH_CAP);
    assert!(u.contact(&overflow).is_none());
}

#[test]
fn contact_duration_accumulation_saturates() {
    let mut u = UtilityState::new();
    let peer = Id16([0x71; 16]);
    u.note_contact(&peer, u64::MAX, 0);
    u.note_contact(&peer, u64::MAX, 1);
    assert_eq!(u.contact(&peer).unwrap().total_duration_ms, u64::MAX);
}
