//! Adversarial simulation suite (E5-S7, handbook §4.8; doc 14 §1 failure
//! register). Six independent scenarios, each driving an [`adversary::Adversary`]
//! (raw frames/bundles, never `mmp_node::Node`) against one or more honest,
//! unmodified `Node`s obtained from `Sim::node_mut`/`Sim::node`.
//!
//! Every assertion below is a PROTOCOL OBSERVABLE (an `Event`, a `store_len`
//! delta, a `diag` record, or a public getter) — never an internal counter
//! that isn't already exposed, per the story's design constraint.
//!
//! Timing discipline (recorded workspace lesson, see `tests.rs` and handbook
//! §5's bug ledger): total simulated time must stay well under
//! `LIFETIME_MESSAGE_MS` (72 h) or expiry is correct behavior, not a failure;
//! nothing here ever asserts on wall-clock time.

use std::collections::HashSet;

use mmp_crypto::Identity;
use mmp_node::diag::JourneyEvent;
use mmp_types::bundle::class;
use mmp_types::claim::field;
use mmp_types::consts::{FILTER_BUCKET_MS, L_MAX, LIFETIME_MESSAGE_MS, MAX_FRAME_PAYLOAD, ORIGIN_BUCKET_BURST};
use mmp_types::frame::{Frame, Summary, SummaryBucket};
use mmp_types::ids::Id16;
use mmp_types::payload::ReceiptPayload;

use crate::adversary::{adversary_contact, signed_bundle, Adversary};
use crate::Sim;

/// Well-known claim-gossip routing scope (mirrors `mmp_node`'s private
/// `CLAIM_SCOPE` constant — not exported, so the adversary uses the same
/// literal value; `class::CLAIM` handling in `on_bundle` never inspects
/// `dest` to decide whether to process a claim, only routing does, so any
/// value would in fact work, but this keeps the wire traffic realistic).
const CLAIM_SCOPE: Id16 = Id16([0xff; 16]);

/// ---------------------------------------------------------------------
/// Scenario 1: Replay storm — doc 14 §1 **closest row F21** ("Lifetime-resurrection
/// replay"). F21's literal scenario is a captor resetting `lifetime_ms` and
/// re-injecting; this test drives the more general case of the SAME
/// mechanism (SeenIndex-from-first-sight dedup, doc 03 §4.2, AUDIT F1) with
/// byte-identical replay and no lifetime tampering at all — a strict subset
/// of F21's threat, so it is the closest row rather than an exact citation.
/// F21's residual promise is exactly what this test checks: "no duplicate
/// user-visible delivery."
/// ---------------------------------------------------------------------
#[test]
fn replay_storm_delivers_exactly_once() {
    let mut sim = Sim::new(2, 0xA11C_E000_0000_0001);
    const SENDER: usize = 0;
    const VICTIM: usize = 1;
    sim.introduce(SENDER, VICTIM);
    let msg_id = sim.send_text(SENDER, VICTIM, "replay me if you can");

    // The adversary is not the destination — it fetches the bundle honestly
    // (a direct WANT, which `load_for_handoff` grants as an ordinary spray
    // copy to any contact, doc 05 §2) from the ORIGIN, capturing the exact
    // wire bytes it will then replay.
    let adv = Adversary::new([0xAA; 32]);
    let now = sim.now_ms();
    let mut fetch = adv.dial(now);
    adversary_contact(sim.node_mut(SENDER), &mut fetch, 1, now, |sess| {
        sess.send(Frame::Want(vec![msg_id]));
    });
    let bundle_bytes = fetch
        .drain_inbox()
        .into_iter()
        .find_map(|f| match f {
            Frame::Bundle(b) => Some(b),
            _ => None,
        })
        .expect("adversary captured the origin's spray copy");

    let before = sim.store_len(VICTIM);

    // Now flood the VICTIM with the same, byte-identical bundle 100 times in
    // one session — unsolicited, no WANT preceded any of these (`on_bundle`
    // processes any `Frame::Bundle` regardless of whether it was requested;
    // that is what makes this an actual attack surface, not a hypothetical).
    let now = sim.now_ms();
    let mut flood = adv.dial(now);
    adversary_contact(sim.node_mut(VICTIM), &mut flood, 2, now, |sess| {
        for _ in 0..100 {
            sess.send(Frame::Bundle(bundle_bytes.clone()));
        }
    });

    let events: Vec<_> = std::iter::from_fn(|| sim.node_mut(VICTIM).poll_event()).collect();
    let received = events
        .iter()
        .filter(|e| matches!(e, mmp_node::Event::MessageReceived { id, .. } if *id == msg_id))
        .count();
    assert_eq!(received, 1, "100x replay must deliver exactly once: {events:?}");

    // The store observable confirms it too: only the first copy (plus the
    // one receipt bundle the victim emits in response) landed — the other 99
    // took the `Disposition::Duplicate` path (SeenIndex hit, dropped before
    // `store.put`), never touching the store at all.
    let after = sim.store_len(VICTIM);
    assert!(after - before <= 2, "replay storm must not multiply store entries: {before} -> {after}");
}

/// ---------------------------------------------------------------------
/// Scenario 2: Flood — doc 14 §1 **F6** ("Sustained Sybil flood at quota limits"):
/// "bounded: attacker airtime = origins_present × bucket rate; relay budget
/// caps total"; residual "fairness degradation; honest traffic keeps WoT
/// multiplier lanes." The bound below is computed from the real constants in
/// `mmp-router/src/quota.rs` (`ORIGIN_BUCKET_BURST` / admission cost, as
/// actually used by `OriginBuckets::admit` and the `mmp-node` admission
/// closure) — not guessed.
/// ---------------------------------------------------------------------
#[test]
fn flood_origin_is_throttled_but_others_deliver() {
    const N: usize = 6;
    const PAIRS: [(usize, usize); 3] = [(0, 1), (2, 3), (4, 5)];
    let mut sim = Sim::new(N, 0x0F10_0D00_0000_0002);

    for &(a, b) in &PAIRS {
        sim.introduce(a, b);
    }
    for &(a, b) in &PAIRS {
        sim.send_text(a, b, "unrelated honest traffic");
    }
    sim.run_random_meetings(100, 60_000, 300_000);
    // A few extra direct contacts give every pair's receipt every chance to
    // round-trip even if the random schedule was unlucky (mirrors
    // `tests.rs`'s existing pattern).
    for k in 0..5 {
        for &(a, b) in &PAIRS {
            sim.meet(a, b, 60_000 + k * 1_000);
        }
    }

    // Node 5 (an endpoint of pair (4,5), so it still has "other traffic" of
    // its own) is the flood target. `mmp-node/src/lib.rs`'s admission
    // closure costs a bundle at `payload.len() + 124` bytes against the
    // per-origin bucket (`OriginBuckets::admit`, mmp-router/src/quota.rs);
    // the bucket starts at `ORIGIN_BUCKET_BURST` (128 KiB) with no WoT
    // multiplier for an unverified stranger, and this flood happens at a
    // single instant (no elapsed time => no refill), so the bound is exactly
    // one burst's worth of admits. Near-MAX_BUNDLE payloads (rather than a
    // smaller size) keep the bound — and so the bundle count needed to prove
    // "sent >> admitted" at the story's ~50x ratio — small enough to stay a
    // CI-fast scenario (every bundle costs a real Ed25519 sign + verify).
    // The Bundle itself may be 4096 bytes, but the encrypted frame has a
    // 16-byte Noise tag. Keep the adversarial payload at the largest value
    // that still fits the frozen 4096-byte envelope cap.
    const PAYLOAD_LEN: usize = MAX_FRAME_PAYLOAD - 16 - 124;
    let cost_per_bundle = (PAYLOAD_LEN + 124) as u64;
    let bound = ORIGIN_BUCKET_BURST / cost_per_bundle;
    // 12x (not the story's illustrative "~50x") keeps this a CI-fast
    // scenario — every sent bundle is a real Ed25519 sign + verify pair, and
    // the throttling proof (`sent >> admitted`) is just as convincing at 12x
    // as at 50x: what matters is the gap between "sent" and "admitted", not
    // the absolute multiplier.
    let sent = bound * 12;

    let victim = 5usize;
    let victim_id = sim.node_id(victim);
    let before = sim.store_len(victim);

    let adv = Adversary::new([0xF1; 32]);
    let now = sim.now_ms();
    let mut s = adv.dial(now);
    adversary_contact(sim.node_mut(victim), &mut s, 999, now, |sess| {
        for i in 0..sent {
            let mut payload = vec![0u8; PAYLOAD_LEN];
            payload[..8].copy_from_slice(&i.to_le_bytes());
            let b = adv.bundle(class::DIRECT, victim_id, 1, LIFETIME_MESSAGE_MS, 0, payload);
            sess.send(Frame::Bundle(b.encode()));
        }
    });

    let admitted = sim.store_len(victim).saturating_sub(before) as u64;
    assert!(admitted <= bound + 1, "flood admitted {admitted} bundles, bucket bound is {bound}");
    assert!(sent >= admitted * 10, "flood must send far more than the bucket ever admits");

    assert!(
        sim.delivery_ratio() >= 0.9,
        "the flood target's OWN honest conversation (and everyone else's) must still deliver: {}",
        sim.delivery_ratio()
    );
}

/// Scenario 3: a Byzantine relay reuses one inflated BUDGET grant in two
/// independent sessions.  Origin admission is receiver-local: each honest
/// receiver must stop admitting this origin at its own bucket burst, while
/// the aggregate across receivers is intentionally *not* treated as a
/// globally conserved capability.
#[test]
fn byzantine_double_grant_is_bounded_per_receiver_not_globally() {
    const LEFT: usize = 0;
    const RIGHT: usize = 1;
    const PAYLOAD_LEN: usize = MAX_FRAME_PAYLOAD - 16 - 124;
    const GRANT: u8 = u8::MAX;
    const DEST: Id16 = Id16([0xD1; 16]);

    let mut sim = Sim::new(2, 0xDB1E_0000_0000_0003);
    let relay = Adversary::new([0xBA; 32]);
    // Keep the bundle origin distinct from the Byzantine relay.  Otherwise an
    // honest receiver's normal "never offer back to origin" rule would hide
    // the grant adoption we are exercising here.
    let origin = Identity::from_seed(&[0xC1; 32]);

    let cost_per_admit = (PAYLOAD_LEN + 124) as u64;
    let local_bound = (ORIGIN_BUCKET_BURST / cost_per_admit) as usize;
    let mut wire = Vec::with_capacity(local_bound + 1);
    for i in 0..=local_bound {
        let mut payload = vec![0u8; PAYLOAD_LEN];
        payload[..8].copy_from_slice(&(i as u64).to_le_bytes());
        let bundle = signed_bundle(&origin, class::DIRECT, DEST, 1, LIFETIME_MESSAGE_MS, 0, payload);
        wire.push((bundle.id(), bundle.encode()));
    }
    let baseline = [sim.store_len(LEFT), sim.store_len(RIGHT)];

    // The relay presents the same oversized grant value to each receiver for
    // the same set of origin-signed bundles.  BUDGET is channel-authenticated
    // but relay-mutable, so the receiver-local clamp is the relevant defense.
    for (receiver, link) in [(LEFT, 700), (RIGHT, 701)] {
        sim.node_mut(receiver).enable_diagnostics();
        let before = sim.store_len(receiver);
        let now = sim.now_ms();
        let mut session = relay.dial(now);
        adversary_contact(sim.node_mut(receiver), &mut session, link, now, |sess| {
            for (id, bytes) in &wire {
                sess.send(Frame::Bundle(bytes.clone()));
                sess.send(Frame::Budget { id: *id, granted: GRANT });
            }
        });
        let admitted = sim.store_len(receiver).saturating_sub(before);
        assert!(
            admitted <= local_bound,
            "receiver {receiver} admitted {admitted} bundles; local origin bucket bound is {local_bound}"
        );
        assert_eq!(
            admitted, local_bound,
            "receiver {receiver} should consume its own burst before the relay's extra copy"
        );
    }

    // Verify that the oversized grant was clamped independently at both
    // receivers.  A subsequent WANT exposes the receiver's adopted budget as
    // a normal outgoing grant; it must be no larger than L_MAX.
    for (receiver, link) in [(LEFT, 702), (RIGHT, 703)] {
        let now = sim.now_ms();
        let mut session = relay.dial(now);
        adversary_contact(sim.node_mut(receiver), &mut session, link, now, |sess| {
            sess.send(Frame::Want(vec![wire[0].0]));
        });
        let grant = session
            .drain_inbox()
            .into_iter()
            .find_map(|frame| match frame {
                Frame::Budget { id, granted } if id == wire[0].0 => Some(granted),
                _ => None,
            })
            .expect("receiver must answer WANT for the admitted bundle");
        assert!(grant > 0 && grant <= L_MAX, "receiver {receiver} leaked an invalid grant: {grant}");
    }

    let aggregate = sim
        .store_len(LEFT)
        .saturating_sub(baseline[0])
        + sim.store_len(RIGHT).saturating_sub(baseline[1]);
    assert!(
        aggregate > local_bound,
        "independent receiver quotas may sum above one local bound; this test must not claim global conservation"
    );
}

/// ---------------------------------------------------------------------
/// Scenario 4: Quota abuse — doc 14 §1 **F4** ("Quota exhaustion under spam"):
/// "eviction order + per-origin buckets"; residual "churn of low-priority
/// spray traffic; own/WoT traffic protected." `eviction_order`
/// (`mmp-router/src/lib.rs`) sorts receipt-confirmed bundles first, then by
/// (lowest effective priority, LARGEST size, least remaining life) — so at
/// equal priority, oversized junk is preferentially sacrificed before a
/// smaller honest bundle. Verified here via `Node::diagnostics()`'s
/// `JourneyEvent::Evicted` records (the sim-exposed "inspect diag" path the
/// story calls for), not an internal counter.
///
/// Runs at CI scale via the `Node::set_store_quota` test seam. A tier-sized
/// quota (128 MiB, `QUOTA_STANDARD`) is unreachable cheaply: a single origin's
/// token bucket caps its lifetime admission near a couple of MB, so 2× real
/// pressure would need a ~2000-identity Sybil swarm and tens of thousands of
/// Ed25519 sign+verify pairs. Shrinking only the *victim's* quota to
/// `QUOTA_SMALL` reproduces the exact same eviction mechanism with ~8 Sybil
/// origins — the defense under test (`eviction_order`) is quota-relative, so
/// the observable is identical, only the byte magnitudes shrink.
/// ---------------------------------------------------------------------
#[test]
fn quota_abuse_degrades_gracefully() {
    let mut sim = Sim::new(3, 0xB0B0_0000_0000_0003);
    const SENDER: usize = 0;
    const RECEIVER: usize = 1;
    const RELAY: usize = 2; // the node under quota pressure
    const QUOTA_SMALL: u64 = 512 * 1024; // test-seam quota, not the tier default

    sim.introduce(SENDER, RECEIVER);
    let msg_id = sim.send_text(SENDER, RECEIVER, "small honest message");

    // Give the relay an unconfirmed (not-yet-receipted) copy to protect.
    sim.meet(SENDER, RELAY, 60_000);
    assert!(sim.store_len(RELAY) > 0, "relay must be carrying the honest bundle");

    sim.node_mut(RELAY).enable_diagnostics();

    // "Max-size junk": as close to MAX_BUNDLE (4096 B) as the fixed 58-byte
    // header + 2-byte payload-length prefix + 64-byte signature allow, at the
    // SAME declared priority (1) as the honest message — so the only thing
    // that can protect the honest bundle at eviction time is `eviction_order`'s
    // size tie-break, not a priority shortcut.
    const PAYLOAD_LEN: usize = MAX_FRAME_PAYLOAD - 16 - 124;
    const WIRE_LEN: u64 = (58 + 2 + PAYLOAD_LEN + 64) as u64;
    let cost_per_admit = (PAYLOAD_LEN + 124) as u64;
    // Upper bound on admits from a single fresh origin's burst (no elapsed
    // time in this flood ⇒ no refill). Real admissions land at or below this;
    // the outer loop below keeps recruiting Sybils until the store actually
    // crosses 2x quota rather than trusting this figure to be exact.
    let sends_per_identity = ORIGIN_BUCKET_BURST / cost_per_admit;
    let target_bytes = 2 * QUOTA_SMALL; // "2x quota pressure"

    let mut link = 10_000u64;
    let mut sybil_i = 0u64;
    while (sim.store_len(RELAY) as u64) * WIRE_LEN <= target_bytes {
        assert!(sybil_i < 10_000, "admission appears broken — store never reached 2x quota");
        let mut seed = [0u8; 32];
        seed[..8].copy_from_slice(&sybil_i.to_le_bytes());
        seed[8] = 0x51; // "sybil" tag, avoids colliding with other seeds in this suite
        let sybil = Adversary::new(seed);
        let now = sim.now_ms();
        let mut s = sybil.dial(now);
        link += 1;
        adversary_contact(sim.node_mut(RELAY), &mut s, link, now, |sess| {
            for j in 0..sends_per_identity {
                let mut payload = vec![0xA5u8; PAYLOAD_LEN];
                payload[..8].copy_from_slice(&j.to_le_bytes());
                let b = sybil.bundle(class::DIRECT, Id16([0x99; 16]), 1, LIFETIME_MESSAGE_MS, 0, payload);
                sess.send(Frame::Bundle(b.encode()));
            }
        });
        sybil_i += 1;
    }

    // Precondition guaranteed by the loop above; asserted explicitly so a
    // silent admission/storage regression can't masquerade as a passing
    // "eviction happened" test below.
    assert!(
        (sim.store_len(RELAY) as u64) * WIRE_LEN > target_bytes,
        "flood must drive the store past 2x quota before eviction runs: {} bundles",
        sim.store_len(RELAY)
    );

    // Now shrink the victim's quota and force one `tick()` — quota enforcement
    // only runs there. Deliberately AFTER the flood: `adversary_contact` ticks
    // the victim on every contact (adversary.rs), so a small quota set earlier
    // would evict mid-flood and the store could never build past 2x pressure.
    // Under the tier default (128 MiB) the flood accumulates freely; the small
    // quota then makes this single tick do the eviction the scenario is about.
    sim.node_mut(RELAY).set_store_quota(QUOTA_SMALL);
    let tick_at = sim.now_ms() + 1;
    sim.node_mut(RELAY).tick(tick_at);

    let evicted: HashSet<Id16> = sim
        .node(RELAY)
        .diagnostics()
        .records()
        .iter()
        .filter(|r| r.event == JourneyEvent::Evicted)
        .map(|r| r.bundle)
        .collect();
    assert!(!evicted.is_empty(), "2x quota pressure must have triggered eviction");
    assert!(
        !evicted.contains(&msg_id),
        "the honest, unreceipted, small bundle must not be among the evicted"
    );

    // And it must still actually be deliverable: continue the honest mesh.
    sim.meet(RELAY, RECEIVER, 60_000);
    for k in 0..3 {
        sim.meet(SENDER, RECEIVER, 120_000 + k * 1_000);
    }
    assert!(sim.delivery_ratio() >= 0.7, "honest delivery ratio degraded below 0.7: {}", sim.delivery_ratio());
}

/// ---------------------------------------------------------------------
/// Scenario 5: Eclipse — doc 14 §1 **closest row F22** ("Prekey suppression → FS
/// downgrade"), the one row this doc's own prose labels "eclipse-class,
/// detected by the receipt-starvation alarm (07 §6.3)". F22's exact scenario
/// (prekey withholding forcing an FS downgrade) is narrower than the three
/// properties this test checks (LWW non-regression, the receipt purge gate,
/// and bounded spray) — those draw on doc 06 §4 (claims) and doc 05 §7/§2
/// (receipt gate, spray budget) more directly than F22 itself — so this is
/// the closest row, honestly, not an exact citation.
/// ---------------------------------------------------------------------
#[test]
fn eclipse_gains_nothing() {
    // A single node, isolated: for the whole test its ONLY possible contact
    // is the adversary (there is no other `Node` in the sim to reach).
    let mut sim = Sim::new(1, 0xEC71_5E00_0000_0004);
    const VICTIM: usize = 0;
    let adv = Adversary::new([0xEC; 32]);

    // --- (a) LWW: a stale claim cannot regress an already-adopted one. ---
    let now = sim.now_ms();
    let mut s1 = adv.dial(now);
    adversary_contact(sim.node_mut(VICTIM), &mut s1, 1, now, |sess| {
        let claim = adv.sign_claim(field::NICKNAME, 5, b"legit".to_vec());
        let bundle = signed_bundle(
            &Identity::from_seed(&[0xEC; 32]),
            class::CLAIM,
            CLAIM_SCOPE,
            2,
            mmp_types::consts::LIFETIME_CLAIM_MS,
            0,
            claim.encode(),
        );
        sess.send(Frame::Bundle(bundle.encode()));
    });
    assert_eq!(
        sim.node(VICTIM).nickname_of(&adv.node_id()).as_deref(),
        Some("legit"),
        "the seq-5 claim must be adopted"
    );

    let now = sim.now_ms();
    let mut s2 = adv.dial(now);
    adversary_contact(sim.node_mut(VICTIM), &mut s2, 2, now, |sess| {
        let stale = adv.sign_claim(field::NICKNAME, 1, b"rollback".to_vec());
        let bundle = signed_bundle(
            &Identity::from_seed(&[0xEC; 32]),
            class::CLAIM,
            CLAIM_SCOPE,
            2,
            mmp_types::consts::LIFETIME_CLAIM_MS,
            0,
            stale.encode(),
        );
        sess.send(Frame::Bundle(bundle.encode()));
    });
    assert_eq!(
        sim.node(VICTIM).nickname_of(&adv.node_id()).as_deref(),
        Some("legit"),
        "a lower-seq claim must never regress the adopted one (LWW, doc 06 §4)"
    );

    // --- (b) receipt gate: a forged receipt cannot purge a bundle whose
    // destination is not the forger. The carried bundle itself is honestly
    // signed by a SYNTHETIC third party ("Carol") the victim is merely
    // relaying toward a fourth party ("Dave") — the adversary is not
    // impersonating Carol, it is trying to fake being Dave's receipt.
    let carol = Identity::from_seed(&[0xCA; 32]);
    let dave = Id16([0xDA; 16]); // a NodeId no honest node in this test owns
    let relayed = signed_bundle(&carol, class::DIRECT, dave, 1, LIFETIME_MESSAGE_MS, 0, b"for dave".to_vec());
    let relayed_id = relayed.id();

    let before = sim.store_len(VICTIM);
    let now = sim.now_ms();
    let mut s3 = adv.dial(now);
    adversary_contact(sim.node_mut(VICTIM), &mut s3, 3, now, |sess| {
        sess.send(Frame::Bundle(relayed.encode()));
    });
    assert_eq!(sim.store_len(VICTIM), before + 1, "the relayed bundle must be stored");

    let now = sim.now_ms();
    let mut s4 = adv.dial(now);
    adversary_contact(sim.node_mut(VICTIM), &mut s4, 4, now, |sess| {
        // Forged: origin = adversary (not Dave), claiming Carol's bundle was
        // delivered. `handle_receipt`'s purge gate requires
        // `stored.dest == receipt_bundle.origin_node()` — that only holds if
        // the receipt truly comes from Dave.
        let payload = ReceiptPayload { delivered: relayed_id, mac: [0x42; 16] }.encode();
        let forged = adv.bundle(class::CONTROL, adv.node_id(), 0, mmp_types::consts::LIFETIME_CONTROL_MS, 0, payload);
        sess.send(Frame::Bundle(forged.encode()));
    });
    // +2, not +1: the CONTROL bundle carrying the forged receipt is itself a
    // validly-signed (by the adversary, over its own bytes) bundle and gets
    // stored regardless of what its payload claims — ingest's admission gate
    // never looks inside a receipt payload. What matters is that the
    // RELAYED bundle (`relayed_id`) is not among the casualties: if the
    // forged purge had succeeded, the count would have stayed at `before + 1`
    // (forged bundle in, relayed bundle purged back out) instead of growing.
    assert_eq!(
        sim.store_len(VICTIM),
        before + 2,
        "a forged receipt from a non-destination must not purge the relayed bundle"
    );

    // --- (c) sessions stay functional, and spray stays bounded: the victim
    // has its own undelivered message toward a THIRD honest node it knows
    // out-of-band (met before the eclipse started) but can no longer reach —
    // the adversary, as its only remaining contact, gets only what ordinary
    // spray policy legitimately hands off (grant clamped to `L_MAX`), not an
    // unbounded dump.
    let mut sim2 = Sim::new(2, 0xEC71_5E00_0000_0005);
    sim2.introduce(0, 1); // out-of-band, "before" the eclipse
    let vid = sim2.send_text(0, 1, "reach the far node");
    // Node 1 ("Bob") is never met again — 0 only ever talks to the adversary.
    let adv2 = Adversary::new([0xE2; 32]);
    let now = sim2.now_ms();
    let mut s5 = adv2.dial(now);
    let mut granted: Option<u8> = None;
    adversary_contact(sim2.node_mut(0), &mut s5, 5, now, |sess| {
        sess.send(Frame::Want(vec![vid]));
    });
    for f in s5.drain_inbox() {
        if let Frame::Budget { id, granted: g } = f {
            if id == vid {
                granted = Some(g);
            }
        }
    }
    assert!(s5.established(), "the session must still establish normally under eclipse");
    let granted = granted.expect("adversary's WANT for the victim's own message must be answered");
    assert!(
        granted <= L_MAX,
        "the eclipsing adversary must only ever receive a policy-bounded spray grant: {granted} > {L_MAX}"
    );
}

/// ---------------------------------------------------------------------
/// Scenario 6: Bloom poison — doc 14 §1 **closest row F5** ("Partition → merge"),
/// whose specified handling explicitly names the exact mechanism this test
/// exercises: "exact D0 receipts purge duplicates ... AUDIT F11" (the
/// hint-vs-purge asymmetry lives in `mmp-core/src/receiptfilter.rs`). No row
/// in doc 14 §1 targets an adversarial poisoned-bloom SUMMARY specifically;
/// F5 is the closest because it is the row that actually discusses the
/// exact/hint split this attack tries to abuse.
/// ---------------------------------------------------------------------
#[test]
fn bloom_poison_never_deletes() {
    let mut sim = Sim::new(3, 0xB100_A000_0000_0006);
    const SENDER: usize = 0;
    const RELAY: usize = 1;
    const RECEIVER: usize = 2;
    sim.introduce(SENDER, RECEIVER);
    let msg_id = sim.send_text(SENDER, RECEIVER, "hi via relay");

    sim.meet(SENDER, RELAY, 60_000); // relay carries an unconfirmed spray copy
    let before = sim.store_len(RELAY);
    assert!(before > 0, "relay must be carrying the honest bundle");

    // Poison: an all-ones SUMMARY receipt bloom, claiming (falsely) that
    // every bundle imaginable has already been receipted.
    let adv = Adversary::new([0xB1; 32]);
    let now = sim.now_ms();
    let mut s = adv.dial(now);
    adversary_contact(sim.node_mut(RELAY), &mut s, 7, now, |sess| {
        let bucket_epoch = (now / FILTER_BUCKET_MS) as u16;
        sess.send(Frame::Summary(Summary {
            receipt_buckets: vec![SummaryBucket {
                bucket_epoch,
                bloom: vec![0xffu8; mmp_types::consts::BLOOM_SUMMARY_MAX],
            }],
            claim_digest: Vec::new(),
            group_hints: Vec::new(),
            want_heads: Vec::new(),
        }));
    });

    // Hints only deprioritize offers — they can never delete. Nothing was
    // purged by the poisoned SUMMARY alone.
    assert_eq!(sim.store_len(RELAY), before, "a poisoned summary bloom must delete nothing");

    // Delivery still completes: `handoff_decision` checks `peer_is_dest`
    // BEFORE it ever consults the (poisoned) receipt hint, so the real
    // recipient's own WANT still succeeds regardless of the relay's
    // globally-poisoned filter.
    sim.meet(RELAY, RECEIVER, 60_000);
    for k in 0..3 {
        sim.meet(SENDER, RECEIVER, 120_000 + k * 1_000);
    }
    assert_eq!(sim.counters().messages_received, 1, "delivery must still complete via the WANT path");
    let _ = msg_id;
}
