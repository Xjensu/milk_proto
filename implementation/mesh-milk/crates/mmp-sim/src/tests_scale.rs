//! E6-S1: 1k-node scale validation (doc 13 §3 network load vs scale; §5
//! latency, the OQ-4 simulation deliverable).
//!
//! **Real finding, not a tuning bug (recorded in doc 13 §5/§6, not silently
//! worked around):** doc 13 §5 sets an explicit pass/fail band — "fail =
//! p50 same-district > 60 min at N=1k" — phrased for SLAW-like (clustered)
//! human mobility at a REALISTIC per-contact cadence (the 5-20 s gaps the
//! 30/100-node scenarios use, approximating real device-discovery timing).
//! `Sim::run_random_meetings` only has uniform-random pairwise contact, no
//! community structure. Two tuning attempts made the actual tradeoff
//! concrete: at a realistic gap (seconds), the meeting count needed for
//! N=1000's ~500k possible pairs to mix at all overruns
//! `LIFETIME_MESSAGE_MS` (72 h) by more than an order of magnitude before
//! most tracked pairs ever get a chance to connect via any relay chain.
//! Compressing the gap to ~250 ms (this file's actual constants) buys
//! enough meetings to fit the TTL, but at that point the simulated clock no
//! longer represents realistic elapsed time, so its p50/p90 output is not
//! comparable to doc 13 §5's real-minutes band at all — asserting it
//! against the 60-min line would be comparing two different units, not a
//! genuine validation. Implementing SLAW (or any clustered mobility model)
//! is a simulation-model project of its own, out of scope here.
//!
//! **What this scenario DOES validate, honestly, at the achieved density**
//! (delivery_ratio ≈ 0.45, p50 ≈ 434 min / p90 ≈ 683 min of *compressed*
//! simulated time, store high-water ≈ 208 KiB, 34.6 simulated hours, seed
//! `0x5CA1E_1000_0000_001`, 500k meetings — measured, not modeled): quota
//! safety holds at 1000-node scale (`quota_ok()`), the TTL budget is
//! respected, and a uniform-random contact process — with no geographic or
//! social clustering to concentrate contacts the way a real same-district
//! population would — measurably dilutes delivery probability as N grows
//! from 100 (ratio 1.0 in the CI/stress scenarios) toward 1000 (ratio
//! ~0.45 here even at inflated meeting density). That dilution is itself
//! the simulation-level evidence for doc 13 §5's own premise: SLAW-like
//! clustering isn't a decorative mobility detail, it's load-bearing for
//! delivery at scale — recorded as a partial answer to OQ-4 (uniform-random
//! mixing measured; the SLAW-specific latency claim remains open pending a
//! clustered-mobility model), not a full close.
//!
//! Manual-only (`--ignored`), same reasoning as `hundred_node_mesh_stress`:
//! wall time (~19 min under `--release`) is far outside a CI-friendly
//! budget at this scale.

use crate::Sim;

/// Manual clustered-mobility measurement. This prints delivery and latency
/// samples for a balanced-community contact process; it is intentionally
/// ignored until trace-derived parameters are available.
#[test]
#[ignore]
fn clustered_mobility_measurement_is_reproducible() {
    const N: usize = 100;
    const COMMUNITIES: usize = 10;
    const MEETINGS: usize = 5_000;
    const BRIDGE_EVERY: usize = 6;

    let mut sim = Sim::new(N, 0x00C1_A57E_D00D);
    let pairs: Vec<(usize, usize)> = (0..10).map(|k| (k, N - 1 - k)).collect();
    for &(from, to) in &pairs {
        sim.introduce(from, to);
        sim.send_text(from, to, "clustered mobility measurement");
    }

    assert!(sim.run_clustered_meetings(MEETINGS, 1_000, 5_000, COMMUNITIES, BRIDGE_EVERY));
    assert!(sim.quota_ok(), "clustered schedule must stay within store quotas");
    eprintln!(
        "clustered_mobility_measurement: delivery_ratio={:.3} p50_ms={:?} p90_ms={:?} meetings={} simulated_hours={:.2}",
        sim.delivery_ratio(),
        sim.latency_percentile_ms(0.50),
        sim.latency_percentile_ms(0.90),
        sim.counters().meetings_run,
        sim.now_ms() as f64 / 3_600_000.0,
    );
}

/// Manual social-cluster measurement. The generator's realized bridge rate is
/// printed alongside protocol outcomes so later trace calibration can compare
/// runs without treating this toy model as SLAW evidence.
#[test]
#[ignore]
fn social_clustered_mobility_measurement_is_reproducible() {
    const N: usize = 100;
    const COMMUNITIES: usize = 10;
    const MEETINGS: usize = 5_000;
    const BRIDGE_PROBABILITY_PER_MILLE: u16 = 120;

    let mut sim = Sim::new(N, 0x50C1_AA57_000D);
    let pairs: Vec<(usize, usize)> = (0..10).map(|k| (k, N - 1 - k)).collect();
    for &(from, to) in &pairs {
        sim.introduce(from, to);
        sim.send_text(from, to, "social clustered mobility measurement");
    }

    let metrics = sim
        .run_social_clustered_meetings(
            MEETINGS,
            1_000,
            5_000,
            COMMUNITIES,
            BRIDGE_PROBABILITY_PER_MILLE,
        )
        .expect("valid social clustered profile");
    assert!(sim.quota_ok(), "social clustered schedule must stay within store quotas");
    eprintln!(
        "social_clustered_mobility_measurement: delivery_ratio={:.3} p50_ms={:?} p90_ms={:?} \
         meetings={} bridges={} bridge_rate={:.3} simulated_hours={:.2}",
        sim.delivery_ratio(),
        sim.latency_percentile_ms(0.50),
        sim.latency_percentile_ms(0.90),
        metrics.meetings,
        metrics.bridge_meetings,
        metrics.bridge_meetings as f64 / metrics.meetings.max(1) as f64,
        sim.now_ms() as f64 / 3_600_000.0,
    );
}

#[test]
#[ignore]
fn thousand_node_mesh_scale_validation() {
    const N: usize = 1000;
    const MEETINGS: usize = 500_000;
    const MIN_GAP_MS: u64 = 100;
    const MAX_GAP_MS: u64 = 400;
    // 20 disjoint pairs spread across the population, same "far corners"
    // shape as the 10/30/100-node scenarios' PAIRS tables.
    const PAIR_COUNT: usize = 20;

    let mut sim = Sim::new(N, 0x5CA1_E100_0000_0001);

    let pairs: Vec<(usize, usize)> = (0..PAIR_COUNT).map(|k| (k, N - 1 - k)).collect();
    for &(a, b) in &pairs {
        sim.introduce(a, b);
    }
    for &(a, b) in &pairs {
        sim.send_text(a, b, "hello across the mesh");
    }

    let start = std::time::Instant::now();
    sim.run_random_meetings(MEETINGS, MIN_GAP_MS, MAX_GAP_MS);
    let elapsed = start.elapsed();

    let ratio = sim.delivery_ratio();
    let p50_ms = sim.latency_percentile_ms(0.50);
    let p90_ms = sim.latency_percentile_ms(0.90);
    let store_high_water = sim.store_high_water_bytes();
    let simulated_hours = sim.now_ms() as f64 / 3_600_000.0;

    eprintln!(
        "thousand_node_mesh_scale_validation: delivery_ratio={ratio:.3} \
         p50_latency_min={:?} p90_latency_min={:?} store_high_water_kib={:.1} \
         simulated_hours={simulated_hours:.1} wall_time={elapsed:?} \
         (gap compressed to fit the 72h TTL at this pair-space size — NOT \
         comparable in real minutes to doc 13 §5's band; see module doc)",
        p50_ms.map(|ms| ms as f64 / 60_000.0),
        p90_ms.map(|ms| ms as f64 / 60_000.0),
        store_high_water as f64 / 1024.0,
    );

    // Stay well inside LIFETIME_MESSAGE_MS (72 h) — a run that eats its own
    // TTL budget invalidates every conclusion below (the recorded tuning
    // lesson from the 100-node scenario's development).
    assert!(simulated_hours < 72.0, "simulated time {simulated_hours:.1}h ate its own message TTL budget");

    // 1000-node store growth stays within quota even under this scenario's
    // inflated meeting density (doc 13 §3's "ample inside quotas" claim).
    assert!(sim.quota_ok(), "no node's store may exceed its quota");

    // Regression guard, not a doc 13 §5 pass/fail gate (see module doc for
    // why that band doesn't apply to this model/timescale): a future change
    // that degrades mixing much further than the measured ~0.45 is worth
    // noticing, with headroom for legitimate PRNG-path variance.
    assert!(ratio >= 0.40, "delivery ratio {ratio:.3} regressed below the measured 0.45 baseline");
}
