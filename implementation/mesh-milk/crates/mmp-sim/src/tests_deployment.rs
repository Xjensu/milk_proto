//! E6-S3: deployment-profile presets (doc 17), drills D-8 (train Profile A)
//! and D-9 (campus/building), doc 17 §8's pass bands.
//!
//! **Model scope, stated honestly:** doc 17 §9's own Performance Engineer
//! checkpoint says these bands are "[S]-tagged... OQ-2's phone-matrix
//! measurement and the doc 15 R1 simulator are what turn these into
//! confirmed numbers, not arithmetic on this page" — these drills are the
//! first such confirmation attempt, not a closing one. Announcement/telemetry
//! propagation is modeled with ordinary class-1 direct bundles rather than
//! the full AuthCert/class-5 apparatus (doc 18): the conductor-round
//! propagation latency this drill measures is a property of the *session
//! mechanism* (SUMMARY→RECONCILE→TRANSFER, doc 03 §5.2), identical
//! regardless of which bundle class rides it — wiring up real AuthCerts
//! would add setup complexity without changing what D-8 actually measures.

use mmp_types::ids::Id16;

use crate::{Event, Sim};

/// D-8 measures **one-way arrival at the conductor** ("telemetry reaches the
/// conductor within <=2 conductor rounds", doc 17 §8) — `Event::MessageReceived`
/// at the conductor's own node, not a round-trip delivery receipt back to the
/// sensor. An earlier version of this drill used [`Sim::is_delivered`]
/// (receipt round-trip) and got stuck at ratio 0.000 even after 10 rounds:
/// tracing individual contacts showed the conductor DID receive every
/// carriage's telemetry within round 0, but minting 8 receipts back-to-back
/// inside one round hits the conductor's own per-origin admission bucket —
/// a real, separate rate-limiting detail, not this drill's subject. Fixed by
/// measuring the one-way arrival doc 17 §8 actually asks for.
#[test]
#[ignore]
fn train_profile_a_misdeployment_drill() {
    const CARRIAGES: usize = 8;
    const ROUNDS: usize = 2; // doc 17 §8's own D-8 band: "<=2 conductor rounds"
    const COPRESENCE_GAP_MS: u64 = 1_000;
    const TRAVEL_MS: u64 = 30_000; // doc 17 §2.4-order dwell between carriages
    // Misdeployed far-sensor<->conductor contact: a brief walking-past pass,
    // not persistent same-carriage co-presence — modeled as a lossy contact
    // rather than a full one. Stated assumption ⚠, not a measured figure.
    const MISDEPLOYED_DROP_PCT: u64 = 60;

    // Layout: node 0 = conductor; 1..=CARRIAGES = one anchor per carriage
    // (conforming case only); CARRIAGES+1..=2*CARRIAGES = one sensor per
    // carriage (both cases; the misdeployed case just never gives sensors
    // 1..7 an anchor to talk to).
    let conductor = 0usize;
    let anchor = |i: usize| 1 + i;
    let sensor = |i: usize| 1 + CARRIAGES + i;
    let n = 1 + 2 * CARRIAGES;

    // Runs the scenario and returns, per carriage, whether the conductor saw
    // that carriage's telemetry `MessageReceived` within `ROUNDS` rounds.
    let run = |conforming: bool| -> Vec<bool> {
        let mut sim = Sim::new(n, if conforming { 0x7A1E_0001 } else { 0x7A1E_0002 });
        let mut ids = Vec::with_capacity(CARRIAGES);
        for i in 0..CARRIAGES {
            // Sensor identities are operator-provisioned (doc 17 §3.3's
            // AuthCert-subject framing) — known to the conductor's device
            // in advance, same bootstrap shortcut the other scale scenarios
            // use for their tracked pairs.
            sim.introduce(sensor(i), conductor);
            ids.push(sim.send_text(sensor(i), conductor, "telemetry"));
        }

        let mut reached = vec![false; CARRIAGES];
        let note_arrivals = |events: &[(usize, Event)], ids: &[Id16], reached: &mut [bool]| {
            for (idx, ev) in events {
                if *idx != conductor {
                    continue;
                }
                if let Event::MessageReceived { id, .. } = ev {
                    if let Some(pos) = ids.iter().position(|x| x == id) {
                        reached[pos] = true;
                    }
                }
            }
        };

        for _ in 0..ROUNDS {
            if conforming {
                // Long same-carriage co-presence (doc 17 §2.1): every
                // sensor's telemetry is already staged at its own carriage
                // anchor before the conductor ever arrives.
                for i in 0..CARRIAGES {
                    sim.meet(sensor(i), anchor(i), COPRESENCE_GAP_MS);
                }
                for i in 0..CARRIAGES {
                    let ev = sim.meet(conductor, anchor(i), TRAVEL_MS);
                    note_arrivals(&ev, &ids, &mut reached);
                }
            } else {
                // Only carriage 0 is provisioned. The conductor still walks
                // the whole train (duties unchanged), but carriages 1..7
                // have no persistent relay — telemetry can only move during
                // the conductor's own brief, lossy pass.
                sim.meet(sensor(0), anchor(0), COPRESENCE_GAP_MS);
                let ev = sim.meet(conductor, anchor(0), TRAVEL_MS);
                note_arrivals(&ev, &ids, &mut reached);
                for i in 1..CARRIAGES {
                    let ev = sim.meet_with_bundle_loss(conductor, sensor(i), TRAVEL_MS, MISDEPLOYED_DROP_PCT);
                    note_arrivals(&ev, &ids, &mut reached);
                }
            }
        }
        reached
    };

    let reached_conforming = run(true);
    let reached_misdeployed = run(false);

    let ratio = |r: &[bool]| r.iter().filter(|&&x| x).count() as f64 / r.len() as f64;
    // "positions >= 4 from the anchor" (doc 17 §8 D-8): carriages 4..7.
    let far_ratio = |r: &[bool]| ratio(&r[4..]);

    let conforming_ratio = ratio(&reached_conforming);
    let far_ratio_conforming = far_ratio(&reached_conforming);
    let far_ratio_misdeployed = far_ratio(&reached_misdeployed);

    eprintln!(
        "train_profile_a_misdeployment_drill: conforming_ratio={conforming_ratio:.3} \
         far_ratio_conforming={far_ratio_conforming:.3} far_ratio_misdeployed={far_ratio_misdeployed:.3}"
    );

    assert!(conforming_ratio >= 0.9, "conforming ratio {conforming_ratio:.3} below 0.9");
    assert!(
        far_ratio_misdeployed <= far_ratio_conforming * 0.5,
        "far-carriage misdeployment degradation insufficient: conforming={far_ratio_conforming:.3} \
         misdeployed={far_ratio_misdeployed:.3} (want misdeployed <= 50% of conforming, doc 17 §8 D-8)"
    );
}

/// D-9: campus/building, ratio >= 0.85 within 15 simulated minutes — the
/// tightest time budget of any scenario in this crate (vs. hours for the
/// 30/100/1000-node scenarios). Modeled with the existing uniform-random
/// contact primitive at a dense, hallway-cadence gap (doc 17 §2.6: a
/// building's density sits between the doc 03/09 baseline and the doc 14 W2
/// stadium extreme) — no new topology mechanism needed, unlike the train's
/// genuinely scheduled conductor-round structure.
#[test]
#[ignore]
fn campus_profile_delivery_drill() {
    const N: usize = 40;
    const MEETINGS: usize = 420;
    const MIN_GAP_MS: u64 = 1_000;
    const MAX_GAP_MS: u64 = 3_000;
    const PAIRS: [(usize, usize); 4] = [(0, 39), (10, 29), (5, 34), (15, 24)];

    let mut sim = Sim::new(N, 0xCA_57_00_01);
    for &(a, b) in &PAIRS {
        sim.introduce(a, b);
    }
    for &(a, b) in &PAIRS {
        sim.send_text(a, b, "hello across the building");
    }

    sim.run_random_meetings(MEETINGS, MIN_GAP_MS, MAX_GAP_MS);

    let ratio = sim.delivery_ratio();
    let minutes = sim.now_ms() as f64 / 60_000.0;
    eprintln!("campus_profile_delivery_drill: delivery_ratio={ratio:.3} simulated_minutes={minutes:.1}");

    assert!(minutes <= 15.0, "simulated time {minutes:.1} min exceeded the 15 min drill budget");
    assert!(sim.quota_ok(), "no node's store may exceed its quota");
    assert!(ratio >= 0.85, "delivery ratio {ratio:.3} below doc 17 §8 D-9's 0.85 band");
}
