# E6-S1 social-cluster simulator handoff

Status: controlled research instrumentation only. The frozen `specification/`
tree, wire format, routing implementation, and platform schedulers are
unchanged. This handoff does not close the SLAW gap or support a performance
claim.

## Gap and model decision

The existing `mmp-sim::run_random_meetings` process is uniform over node pairs,
so it dilutes contact opportunities as the population grows. The earlier
`ClusteredMobility` helper adds balanced communities and a periodic adjacent
bridge, but its fixed cadence can hide temporal variability. Neither model is
SLAW-calibrated and neither should be used as a human-mobility claim.

`SocialClusteredMobility` is an additive, deterministic control model for the
next measurement pass. It has an explicit latent active community:

1. Communities are balanced by round-robin node assignment; every community
   has at least two nodes.
2. Each contact takes a bridge transition with a seeded integer probability in
   per-mille units. A bridge chooses a non-current community and moves the
   latent active state there.
3. A non-bridge contact samples two distinct nodes from the active community.
4. The existing simulator `Rng` drives every choice, so a seed reproduces the
   contact generator exactly.

This is deliberately a small social-cluster control, not a SLAW implementation:
there are no fitted traces, Levy flights, pause-time distributions, waypoints,
geographic coordinates, radio PHY, or calibrated contact durations. The
`bridge_probability_per_mille` value is an experiment input, not a protocol
constant.

## API and metrics

`Sim::run_social_clustered_meetings` returns
`SocialClusteredRunMetrics` without exposing or changing routing state:

```text
meetings
bridge_meetings
bridge_probability_per_mille
```

The ignored measurement test additionally records the protocol observables
needed for later comparison:

```text
seed, node_count, community_count, requested bridge probability,
minimum/maximum inter-meeting gap, meetings, realized bridge count/rate,
simulated elapsed hours, delivery ratio, p50/p90 delivery latency,
store high-water bytes, quota_ok
```

Run the deterministic unit checks with:

```text
cargo test -p mmp-sim social_clustered -- --test-threads=1
```

Run the long, manual metric capture only when a measurement window is
available:

```text
cargo test -p mmp-sim social_clustered_mobility_measurement_is_reproducible -- --ignored --nocapture --test-threads=1
```

The output is a reproducible toy-model observation, not a target or a
production result. Compare it against the uniform-random baseline and the
periodic-cluster control using the same node/workload/gap settings before
making any interpretation.

## Required calibration before using the model for OQ-4

The following inputs are still missing and must come from trace evidence or a
documented synthetic-study decision:

- community count and membership distribution;
- inter-community transition probability and whether transitions are adjacent,
  weighted, or long-range;
- inter-contact and dwell-time distributions, including heavy tails;
- pair-contact duration, loss, and usable BLE capacity distributions;
- trace sampling interval, anonymization, and reproducible seed/provenance;
- confidence intervals for delivery, p50/p90 latency, airtime, and storage.

The next safe experiment is a paired replay: hold the `mmp-node` workload,
bundle size, TTL, gap range, seed policy, and measurement counters constant;
run uniform-random, periodic-cluster, and social-cluster generators; then
report the complete metric block above. Do not tune routing or copy budgets to
make one generator pass the doc 13 latency band. The SLAW-specific claim
remains open until the synthetic model is calibrated or a trace-backed model
is available.

## Adversarial review

### Skeptical Professor

**Potential flaw:** a latent community state is called social mobility without
observing a real social process.

**Alternative:** label it a clustered control and compare its distributions to
trace-derived statistics before using SLAW terminology.

**Resolution:** this handoff explicitly calls the model non-SLAW-calibrated and
requires trace calibration.

### Security Auditor

**Potential flaw:** synthetic clustering could be mistaken for trusted social
claims and feed a routing change.

**Alternative:** keep the generator entirely inside simulation and prohibit
its output from altering wire or admission semantics.

**Resolution:** the API returns contact-generator metrics only; no claims or
router state are produced.

### Performance Engineer

**Potential flaw:** bridge probability alone says nothing about contact
duration, loss, controller scheduling, or transfer capacity.

**Alternative:** add measured duration/loss traces as separate inputs and
report airtime and goodput independently.

**Resolution:** those fields are explicit calibration gates, not hidden model
assumptions.

### Distributed Systems Reviewer

**Potential flaw:** persistent active-community state can create artificial
mixing or partition behavior that dominates delivery results.

**Alternative:** replay multiple seeds and compare against both controls;
report realized bridge and dwell distributions.

**Resolution:** the bridge count/rate is returned and deterministic; no single
run is sufficient evidence.

### Battery Specialist

**Potential flaw:** a clustered contact model may improve delivery while using
more radio sessions, falsely implying an energy win.

**Alternative:** keep radio/host/whole-device energy as separate external
measurements and do not infer battery from delivery ratio.

**Resolution:** this model has no energy claim; energy remains the EDR harness's
separate measurement input.

### Inventive Scientist

**Potential flaw:** adding more synthetic knobs can produce a fitted story
without predictive value.

**Alternative:** pre-register the parameter sweep, retain raw generator logs,
and test held-out traces or distributions.

**Resolution:** current parameters are a bounded control experiment only; no
new knob should be promoted without trace evidence and reproducible metrics.

## Files and boundaries

- `implementation/mesh-milk/crates/mmp-sim/src/lib.rs`: additive social-cluster
  generator and metrics return type.
- `implementation/mesh-milk/crates/mmp-sim/src/tests.rs`: deterministic and
  validation tests.
- `implementation/mesh-milk/crates/mmp-sim/src/tests_scale.rs`: ignored manual
  metric capture.
- `specification/`: unchanged.
