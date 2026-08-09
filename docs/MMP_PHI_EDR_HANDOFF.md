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
