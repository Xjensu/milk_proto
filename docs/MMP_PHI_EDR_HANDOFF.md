# MMP/Φ EDR audit and measurement handoff

Status: research-only follow-up. This document records an implementation audit
and an experiment plan; it does not change the frozen `specification/` tree,
wire behavior, or any platform scheduler.

## Audit result (2026-08-09)

The current `mmp_sim::energy::RendezvousEnergyModel` implements exactly the
controlled algebra described in `docs/MMP_PHI_SIMULATION.md`:

```text
h ∝ λρ
E = c_tx λ + c_rx ρ
```

It keeps `λρ` equal to the baseline product and computes the square-root
stationary point. The three unit tests now cover (a) the fixed-hazard and
equal-energy-contribution invariants, (b) the illustrative 0.12/4.5 model
point, and (c) rejection of non-representable floating-point inputs. The last
guard is important because finite input fields can still overflow their
product, baseline energy, or optimum terms; accepting those values would make
`idealized_gain()` return an unusable result or make `optimum()` panic.

Confidence: High for the implementation facts (source and tests); Medium for
the algebra because it is conditional on the stated fixed-hazard and linear
cost assumptions.

## What the supplied MMP/Φ proposal establishes — and what it does not

The proposal's 3.14× number is reproducible as an idealized ratio for the
illustrative model: `4.62 / 1.4697 ≈ 3.14`. It is not a measured BLE result,
and it cannot be promoted to the requested 2× production claim. In
particular:

1. `c_tx` and `c_rx` are aggregate baseline costs in the current model, not
   independently measured marginal costs over a range of duty cycles.
2. Holding `q` constant is a useful first-order experiment, but increasing
   advertisement activity can change collisions, channel overlap, controller
   scheduling, and host wakeups. Therefore preserving `λρ` does not prove that
   discovery hazard is preserved on any platform.
3. Mapping a 5-second advertisement interval to approximately 0.82 seconds
   and a 30 ms scan window to approximately 7.8-second spacing is a candidate
   sweep point only. The simulator uses normalized activities and has no BLE
   timing, controller, or OS model, so these values are not implementation
   parameters.
4. Bluetooth 6 controller features and platform background policies are
   capability and OS questions. They are not evidence that Android, iOS,
   desktop, or IoT hosts will expose the required filtering/monitoring behavior
   to this project.
5. The proposal's target table (6.90 versus 13.82 mAh/day) is a planning
   budget, not an observation. A whole-device result must include host wakeups,
   radio/controller energy, and foreground/background state in addition to
   packet airtime.

Confidence: High that these limitations follow from the current model and
docs; Low for any claimed hardware multiplier until a controlled measurement
exists.

## Required experiment before an EDR scheduler decision

The next implementation should be a measurement harness, not a production
policy. For each platform and device pair, compare the existing duty-cycle
policy with a pre-registered EDR candidate on the same contact trace and in
randomized order. Record at least:

- configured advertisement interval, scan interval/window, and controller
  capabilities;
- actual advertisement and scan events, received beacons, missed contacts,
  collision/duplicate counts, and discovery latency;
- authenticated-session success, useful payload bytes, airtime, and delivery
  latency under the same bundle workload;
- radio/controller energy, host wakeups/CPU time, and whole-device energy with
  screen and unrelated radios held constant;
- device model, OS version, power state, temperature, background policy, and
  measurement instrument/calibration metadata.

Use the same contact trace for both policies, repeat each point across multiple
independent runs and device pairs, and report confidence intervals rather than
only a mean. The first sweep should include the baseline and several bounded
activity multipliers around it; it must not assume that the analytical optimum
is schedulable. A candidate is informative only when discovery probability and
missed-contact rate are statistically comparable to baseline while energy,
host wakeups, airtime, and delivery latency are reported separately.

Minimum release gate for any opt-in scheduler experiment:

```text
same-trace paired comparison recorded for Linux/Windows, Android, and iOS;
actual event counts and whole-device energy captured;
discovery and delivery non-inferiority bound agreed in advance;
no platform exceeds its background-execution or battery budget;
results are reproducible from raw logs and the model's input schema.
```

Until that gate is met, EDR remains simulator instrumentation only. No value
from this handoff may alter MMP/1 wire fields, copy budgets, BLE state
machines, or default platform behavior.

## Adversarial review

### Skeptical Professor

**Potential flaw:** the square-root solution is presented as an "optimum".

**Alternative:** call it the constrained optimum of the toy model and test
whether the measured hazard surface is separable or even monotone.

**Resolution:** retain the analytical test, but require trace-derived hazard
and confidence intervals before using the word optimum for a platform.

### Security Auditor

**Potential flaw:** a more aggressive beacon schedule could expose more
metadata or create a larger unauthenticated hint stream.

**Alternative:** evaluate privacy leakage, admission pressure, and malformed
beacon handling independently of energy.

**Resolution:** EDR is not allowed to change authentication or admission
semantics; any platform trial must use the existing verify-before-store/forward
path and collect resource-abuse metrics.

### Performance Engineer

**Potential flaw:** the linear energy model omits fixed wakeup/setup costs and
collision losses, which can reverse the analytical ranking.

**Alternative:** fit piecewise or measured cost curves and include event
success, not only configured activity.

**Resolution:** capture actual event counters, host wakeups, and radio energy;
keep the model as a comparison baseline rather than a scheduler oracle.

### Distributed Systems Reviewer

**Potential flaw:** preserving a local discovery hazard does not establish the
same end-to-end DTN delivery probability under partitions and clustered
mobility.

**Alternative:** replay identical contact traces through the existing
`mmp-node`/`mmp-tal` simulator and report delivery and latency separately.

**Resolution:** no routing or copy-budget change is admitted from EDR alone.

### Battery Specialist

**Potential flaw:** radio-only mAh can improve while whole-phone battery life
gets worse because the host wakes more often.

**Alternative:** measure controller, host, and whole-device energy in separate
channels, with background policy documented.

**Resolution:** the whole-device and host-wakeup fields above are mandatory
acceptance data.

### Inventive Scientist

**Potential flaw:** the proposal's EDR/PCM/contact-survival ideas could be
combined into an untestable stack of assumptions.

**Alternative:** isolate one hypothesis at a time: first EDR discovery,
then any mobility evidence, then contact-survival scheduling.

**Resolution:** this handoff limits the next step to a paired EDR measurement;
PCM, EWS, Channel Sounding, and DoF Spray remain separate research tracks.

## Files and boundaries

- `implementation/mesh-milk/crates/mmp-sim/src/energy.rs`: rejects derived
  non-finite values while preserving the normalized model.
- `docs/MMP_PHI_SIMULATION.md`: existing model contract; unchanged.
- `specification/`: unchanged by this audit.

The next agent should add only a deterministic harness/input parser after the
measurement schema and raw-log provenance are agreed. Do not add a production
EDR scheduler or infer a 2× result from the analytical tests.

## 2026-08-09 aggregate input schema

`mmp_sim::energy` now provides the wire-neutral
`RendezvousMeasurement`/`PairedRendezvousMeasurement` input seam and a
`RendezvousComparison` summary. A host measurement harness supplies the
baseline and candidate records; the crate does not invent platform readings,
derive BLE timings, or alter `mmp-node` behavior. The records carry controller
event counters, contact/discovery and delivery counts, latency sums plus sample
counts, useful payload bytes, airtime, radio/host/whole-device energy, and host
wakeups. Baseline and candidate must share a `trace_id`.

Validation rejects non-finite or negative measurements, inconsistent success
and sample counts, and mismatched traces. The summary deterministically reports
discovery and delivery rates/deltas, absolute energy channels, candidate versus
baseline whole-device energy when the baseline is non-zero, and optional mean
latencies when samples exist. A zero baseline energy is represented as an
undefined ratio rather than an infinity; an overflowing derived ratio is
rejected. The focused tests cover valid paired output, count/trace rejection,
and non-finite input rejection. This schema is an experimental data contract,
not a production acceptance claim.

## 2026-08-09 deterministic EDR experiment matrix

`RendezvousEnergyModel::default_experiment_matrix()` now supplies a fixed,
wire-neutral sweep for the next trace-replay step. It contains normalized
transmit-activity multipliers:

```text
0.25, 0.50, 0.75, 1.00, 2.00, 4.00, 8.00
```

The `1.00` point is labelled `BinarySprayBaseline` only to identify the
current MMP/1 workload in a comparison. The other six points are labelled
`EdrDutyCycle`; each derives receive activity as `K / tx_activity`, preserving
the toy model's fixed hazard product. The matrix is deterministic and returns
`None` if a derived point is not representable as a finite positive value.

This helper does not invoke `mmp-router`, mutate copy budgets, add wire fields,
or select a platform scheduler. It supplies candidate normalized points to a
future harness that must replay the same contact trace and workload for every
point, then populate `RendezvousMeasurement` records. The multipliers are a
bounded sweep for measurement planning, not evidence that any point is
schedulable or superior; collision probability, controller behavior, host
wakeups, and whole-device energy remain external measurements.

## 2026-08-09 checked gain boundary

`RendezvousEnergyModel::checked_idealized_gain()` is the safe reporting path
for callers that accept a wide floating-point input range. The constructor
rejects non-finite intermediate model terms, but a finite baseline energy and a
finite optimum energy can still have an unrepresentable quotient. The checked
method returns `None` for that case. The original `idealized_gain() -> f64`
method remains available for compatibility with the existing research API;
its result must be checked with `is_finite()` (or replaced by the checked
method) before it is serialized into an experiment report.

This guard does not add a measurement parser. A parser still requires an
agreed raw-log format, provenance/calibration metadata, and confidence
interval policy; inventing those fields in `mmp-sim` would make aggregate
records look like platform evidence. Until that external contract exists, the
crate accepts only caller-constructed `RendezvousMeasurement` values and
performs validation/comparison without inventing readings.

## 2026-08-12 bounded CLI reporting seam

`mmp-cli edr-summary <pair.json> [--json]` now consumes the existing strict
`PairedRendezvousMeasurement` JSON contract and prints the validated comparison
without touching the production router or any wire field. The command checks
the file size before and after reading, rejects option-looking paths, and
preserves the simulator's fail-closed schema/count/trace validation. Human
output is intended for local inspection; `--json` emits a deterministic,
versioned report suitable for a beta harness. The output is explicitly marked
research-only and must not be read as evidence of a 2× improvement.

The CLI is an adapter over the mature `mmp-sim` parser rather than a second
JSON implementation. This keeps the input contract and bounds in one place;
the remaining production gate is still measured platform data with raw-log
provenance and a pre-registered comparison matrix.

## 2026-08-14 baseline replay provenance seam

`mmp-sim::ResearchExperiment` now provides a baseline-only host harness for
the first reproducible comparison step. It accepts one validated
`ContactSchedule` and one bounded direct-text workload, creates fresh MMP/1
simulator nodes, replays the schedule, and returns `Mmp1Replay` with
`ResearchProvenance` containing the node count, seed, schedule configuration
ID, full trace ID, generator label, and a deterministic workload ID. Invalid
sender/recipient pairs and clearly oversized text are rejected before
simulation state is created; final wire-framing failures are returned as a
typed send error rather than panicking. Focused tests prove repeated runs are
equality-identical, retain the schedule trace ID, account for one sent
workload, and preserve quota safety.

This is a baseline/provenance slice, not a candidate strategy or an EDR
measurement. The EWS, contact-survival, and DoF modules still do not alter
`Sim::meet`, routing, wire frames, or physical energy; they remain sidecar
research evaluators until a separate candidate adapter and aggregation policy
are agreed. No 2× claim follows from this seam.

## 2026-08-14 EWS replay provenance adapter

`mmp-sim::ews::evaluate_replay` now binds a caller-supplied EWS evidence set
to an exact `Mmp1Replay` provenance record. It rejects mismatched seed,
schedule, trace, workload, or generator identifiers, and independently
rejects a replay whose metrics disagree with its trace or generator. The
validated evidence is evaluated by the existing bounded deterministic EWS
planner and receives an order-independent BLAKE3 fingerprint for audit
reports.

This remains a research sidecar: probabilities and costs are caller
assumptions, the independence product is only a heuristic, and the result is
not candidate transport evidence or a Byzantine/global copy-budget guarantee.
The adapter does not mutate `Sim`, `mmp-router`, storage, wire frames, or the
frozen specification. Candidate-vs-baseline delivery and energy claims still
require paired replay aggregation with identical seeds and workloads.
