# E4-S5 — Battery and duty-cycle measurement record

Status: **measurement template; hardware gate incomplete until a physical run is
attached**.

This form records a paired MMP/1 baseline and candidate duty-cycle run. It does
not change the frozen wire protocol, select a production scheduler, or turn a
modelled energy value into a battery claim. Leave a field as `unavailable` and
give the reason when the platform does not expose it; do not estimate a missing
measurement from configured timings.

The fields below are grounded in:

- [`specification/09-transport-ble.md`](../specification/09-transport-ble.md)
  §§3.2, 4, 5.2–5.3, and 7 (advertising/scan timing, session parameters,
  negotiated MTU, flow control, and airtime limits);
- [`specification/13-performance-model.md`](../specification/13-performance-model.md)
  §§1–3 and 6 (radio-only ledger, host-overhead uncertainty, workload, and
  validation status);
- [`PC_MOBILE_MANUAL_TEST_GUIDE.md`](PC_MOBILE_MANUAL_TEST_GUIDE.md) §§1–6
  (revision, monotonic callbacks, radio context, counters, logs, and result
  labels); and
- [`MMP_PHI_EDR_HANDOFF.md`](MMP_PHI_EDR_HANDOFF.md) (paired traces, actual
  event counters, controller/host/whole-device energy, wakeups, calibration,
  and confidence reporting).

## 1. Run identity and provenance

Complete one record for each run. A baseline and candidate that are intended to
be compared MUST share `trace_id`, workload, device pair, and seed where the
platform permits deterministic replay. Record the pair in separate sections or
duplicate this file; never combine readings from unrelated runs.

| Field | Value | Required / notes |
| --- | --- | --- |
| `record_id` | `TODO` | Stable local identifier for this record |
| `pair_id` | `TODO` | Same value for baseline and candidate |
| `strategy` | `MMP/1 baseline` / `EDR candidate` / other isolated research strategy | Candidate remains research-only |
| `role` | `central` / `peripheral` / `relay` / `sender` / `receiver` | Record one row per measured endpoint when possible |
| `trace_id` | `TODO` | Required for a paired comparison; must match on both sides |
| `random_seed` | `TODO` / `not applicable` | Record every seed used by simulator or workload generator |
| `revision` | `TODO` | `git rev-parse HEAD` of the tested checkout |
| `working_tree` | `clean` / `dirty (explain)` | Release evidence requires a clean specification tree |
| `specification_status` | `unchanged` / `proposal explicitly approved` | This experiment must normally be `unchanged` |
| `operator` | `TODO` | Non-secret operator label |
| `run_start_utc` | `TODO` | Human-readable provenance only; timing uses monotonic time below |
| `evidence_directory` | `TODO` | Absolute path outside the repository when possible |
| `command_or_app_build` | `TODO` | Exact command, app build, or harness invocation |
| `toolchain` | `TODO` | Rust/OS/device tooling versions used |

Do not put identity seeds, private keys, keychain exports, or other secrets in
the evidence directory. Store only non-secret BundleIDs and fingerprints where
the manual guide permits them.

## 2. Device, operating-system, and radio context

| Field | Baseline | Candidate | Notes |
| --- | --- | --- | --- |
| endpoint label / model | `TODO` | `TODO` | E.g. phone, PC, nRF52, or ESP32 role |
| platform tier | `T0` / `T1` / `T2` / `T3` | same / `TODO` | Use the project tier actually exercised |
| OS and version/build | `TODO` | `TODO` | Include Android/iOS/Linux/Windows/macOS version |
| app or CLI version | `TODO` | `TODO` | Include native consumer build identifier |
| BLE adapter/controller/chipset | `TODO` | `TODO` | Record the model; `unavailable` needs a reason |
| adapter firmware/driver | `TODO` / `unavailable` | `TODO` / `unavailable` | Optional when the platform exposes it |
| negotiated MTU | `TODO` | `TODO` | Record value or explicit unavailable reason |
| PHY / L2CAP CoC | `TODO` / `not used` | `TODO` / `not used` | Do not infer PHY from a request |
| screen / process state | `foreground/unlocked` etc. | `foreground/unlocked` etc. | Record background, suspended, or service state |
| battery saver / Doze / low-power mode | `TODO` | `TODO` | Include exact state, not just “on/off” |
| unrelated radios and workload | `TODO` | `TODO` | Hold constant between paired runs or explain deviation |
| ambient temperature | `TODO` / `unavailable` | `TODO` / `unavailable` | Optional; record instrument and units if used |
| energy instrument and calibration | `TODO` / `not instrumented` | `TODO` / `not instrumented` | Required to interpret energy values |

For Android and iOS, document whether the run is foreground or background and
whether state restoration was enabled. An artifact/API test is not a physical
radio result.

## 3. Workload and controlled conditions

| Field | Value | Notes |
| --- | --- | --- |
| node count and roles | `TODO` | Include the measured pair and any relay population |
| topology / mobility model | `TODO` | Physical trace, replay, uniform, clustered, or other |
| contact distribution | `TODO` | Include contact cadence or trace identifier; do not call it realistic without evidence |
| loss / disconnect model | `TODO` | Physical observations or configured loss model |
| TTL / lifetime | `TODO` | Record units and the configured value |
| payload classes | `TODO` | Text, control, claims, group, blob, or other |
| payload size(s) | `TODO` | Bytes before and after any framing if both are known |
| bundle count | `TODO` | Number offered in this run |
| bundle identifiers | `TODO` | Non-secret IDs or a separate manifest reference |
| retry / reconnect policy | `TODO` | Include any operator-triggered restart |
| repeated-run count | `TODO` | Number of paired repetitions, not just samples within one run |
| randomization order | `TODO` | Baseline-first, candidate-first, or randomized order |

Baseline and candidate MUST use the same workload and contact trace for a
paired claim. If that is impossible, classify the comparison as
`inconclusive` rather than attributing the difference to duty cycle.

## 4. Configured duty-cycle and session parameters

Record configured values *and* observed counters. Configuration is not evidence
that a controller or operating system honoured the request.

| Parameter | Baseline | Candidate | Units / allowed context |
| --- | ---: | ---: | --- |
| advertisement interval target | `TODO` | `TODO` | MMP/1 reference range is adaptive 2–10 s |
| advertisement jitter | `TODO` | `TODO` | MMP/1 specifies ±10% randomization |
| scan window | `TODO` | `TODO` | MMP/1 baseline is 30 ms |
| scan interval | `TODO` | `TODO` | MMP/1 baseline is 1.28 s |
| baseline scan duty target | `TODO` | `TODO` | MMP/1 reference is approximately 2.3% |
| burst-scan enabled/duration | `TODO` | `TODO` | Record rate limit and foreground/background state |
| requested connection interval | `TODO` | `TODO` | MMP/1 active-session guidance is 15–30 ms |
| requested supervision timeout | `TODO` | `TODO` | MMP/1 active-session guidance is 4 s |
| blob-stream connection interval | `TODO` | `TODO` | Record `not applicable` when no blob traffic |
| per-session airtime cap | `TODO` | `TODO` | MMP/1 hard cap is 60 s |
| relay airtime token-bucket policy | `TODO` | `TODO` | Include budget or `not applicable` |
| frame credits | `TODO` | `TODO` | MMP/1 HELLO default is N=8 |
| controller filtering/monitoring | `TODO` | `TODO` | Capability and actual use, not a marketing label |

For a candidate research strategy, record the exact candidate label and all
parameters needed to reproduce it. Do not edit MMP/1 defaults in this record.

## 5. Monotonic test timeline

Use one monotonic clock per endpoint (for example Android elapsed realtime or
iOS system uptime). Wall-clock timestamps may accompany events for provenance,
but MUST NOT be used to calculate session timers, delivery latency, or TTL
elapsed time.

| Event | Monotonic time (ms) | Endpoint | Evidence reference / note |
| --- | ---: | --- | --- |
| test process/app started | `TODO` | `TODO` | |
| node restored/created | `TODO` | `TODO` | |
| first advertisement emitted | `TODO` | `TODO` | |
| first scan window opened | `TODO` | `TODO` | |
| beacon observed | `TODO` | `TODO` | Include RSSI sample reference |
| dial requested | `TODO` | `TODO` | |
| GATT ready / `link_up` | `TODO` | `TODO` | |
| Noise/session established | `TODO` | `TODO` | Handshake completion |
| first useful payload sent | `TODO` | `TODO` | |
| bundle accepted / received | `TODO` | `TODO` | |
| receipt observed / delivery confirmed | `TODO` | `TODO` | |
| disconnect / `link_down` | `TODO` | `TODO` | |
| reconnect | `TODO` | `TODO` | Repeat rows as needed |
| snapshot persisted | `TODO` | `TODO` | |
| process/device stopped | `TODO` | `TODO` | |

Add rows for every reconnect or contact. Calculate durations from monotonic
values and preserve the raw event log so a reviewer can reconstruct the order.

## 6. Radio, discovery, and session observations

| Metric | Baseline | Candidate | Unit / definition |
| --- | ---: | ---: | --- |
| advertisement events emitted | `TODO` | `TODO` | Actual controller/app counter |
| scan windows opened | `TODO` | `TODO` | Actual counter |
| valid beacons observed | `TODO` | `TODO` | Parsed MMP service beacons |
| missed/expected beacon opportunities | `TODO` / `unavailable` | `TODO` / `unavailable` | Explain denominator when unavailable |
| duplicate/collision observations | `TODO` / `unavailable` | `TODO` / `unavailable` | Record method or reason |
| discovery latency | `TODO` | `TODO` | Monotonic beacon-to-dial or beacon-to-session interval; report distribution |
| session attempts | `TODO` | `TODO` | |
| sessions established | `TODO` | `TODO` | |
| handshake duration | `TODO` | `TODO` | Monotonic interval |
| RSSI samples | `TODO` | `TODO` | Preserve raw samples and units |
| negotiated MTU | `TODO` | `TODO` | Value or unavailable reason |
| inbound bytes | `TODO` | `TODO` | Raw radio/application counter source |
| outbound bytes | `TODO` | `TODO` | Raw radio/application counter source |
| close/event drops | `TODO` | `TODO` | FFI/native counters; non-zero is operational failure |

## 7. Delivery, replication, airtime, and storage

| Metric | Baseline | Candidate | Unit / definition |
| --- | ---: | ---: | --- |
| bundles offered | `TODO` | `TODO` | Workload denominator |
| bundles delivered | `TODO` | `TODO` | Confirm at the opposite endpoint |
| delivery probability | `TODO` | `TODO` | `delivered / offered`, with denominator stated |
| delivery latency p50 / p90 | `TODO` | `TODO` | Monotonic send-to-confirmed-receipt; report sample count |
| copies per bundle | `TODO` | `TODO` | Total observed copies or exact unavailable reason |
| copy-count distribution | `TODO` | `TODO` | Not just the mean |
| useful payload bytes | `TODO` | `TODO` | Delivered application bytes |
| total transfer bytes | `TODO` | `TODO` | Include protocol overhead when available |
| airtime | `TODO` | `TODO` | Seconds; state whether controller or session estimate |
| airtime per delivered byte | `TODO` | `TODO` | Derived only when both inputs are measured |
| storage high-water | `TODO` | `TODO` | Bytes; include message/blob scope |
| storage after restart | `TODO` | `TODO` | Bytes and whether durable state was preserved |
| rejected/expired bundles | `TODO` | `TODO` | Count and reason |

Do not count a process that remained alive as delivery. A delivery result needs
the bundle and receipt (or equivalent opposite-endpoint confirmation) described
by the manual guide.

## 8. Battery, energy, CPU, and wakeup observations

The performance model separates radio/controller energy from uncertain host
overhead. Keep the channels separate; a single battery percentage is an energy
proxy, not a calibrated radio measurement.

| Metric | Baseline | Candidate | Unit / method / reason if unavailable |
| --- | ---: | ---: | --- |
| radio/controller energy | `TODO` / `unavailable` | `TODO` / `unavailable` | Instrument, calibration, and sampling period |
| host/SoC energy | `TODO` / `unavailable` | `TODO` / `unavailable` | Instrument or platform counter |
| whole-device energy | `TODO` / `unavailable` | `TODO` / `unavailable` | Instrument, calibration, and unrelated-load controls |
| battery level at start/end | `TODO` / `unavailable` | `TODO` / `unavailable` | Percentage is a proxy; record duration and source |
| battery/charge delta | `TODO` / `unavailable` | `TODO` / `unavailable` | Do not convert to mAh without a documented method |
| radio active time | `TODO` / `unavailable` | `TODO` / `unavailable` | Seconds or counter units |
| CPU time | `TODO` / `unavailable` | `TODO` / `unavailable` | Process/system scope and source |
| CPU utilization | `TODO` / `unavailable` | `TODO` / `unavailable` | Sampling method and interval |
| host wakeups | `TODO` / `unavailable` | `TODO` / `unavailable` | Count and instrumentation source |
| scan callbacks / app callbacks | `TODO` / `unavailable` | `TODO` / `unavailable` | Count actual callbacks |
| thermal/power throttling | `none` / `TODO` | `none` / `TODO` | Record any event or unavailable reason |

Record `start`, `end`, and elapsed monotonic duration for every energy sample.
If the platform exposes only battery percentage, report it as a proxy and do
not claim controller or whole-phone mAh. The model's nRF52 and phone current
values are assumptions for planning, not replacements for this section.

## 9. Paired comparison and analysis

| Result | Value | Evidence / calculation |
| --- | --- | --- |
| baseline/candidate same trace | `yes` / `no` | `trace_id` and workload references |
| baseline/candidate same device pair | `yes` / `no` | Device context references |
| discovery-rate delta | `TODO` | Counts and denominator |
| delivery-rate delta | `TODO` | Counts and denominator |
| latency delta (p50/p90) | `TODO` | Sample counts and monotonic source |
| copies delta | `TODO` | Distribution, not only average |
| airtime delta | `TODO` | Controller/session measurement scope |
| storage high-water delta | `TODO` | Same TTL and workload required |
| CPU/wakeup delta | `TODO` | Scope and counters |
| energy delta by channel | `TODO` | Radio, host, whole-device separately |
| confidence interval / distribution | `TODO` | Method, repetitions, and seed list |
| candidate meets pre-registered threshold | `yes` / `no` / `inconclusive` | State threshold before inspecting result |

Do not report an “idealized gain” from the analytical EDR model as measured
improvement. A claim of `2x` requires the same baseline, metric, workload, and
paired evidence; individual delivery, latency, airtime, storage, CPU/wakeup,
and energy results remain visible even when a composite metric is used.

## 10. Evidence inventory

| Artifact | Path / hash | Present? | Notes |
| --- | --- | --- | --- |
| raw operator log | `TODO` | `yes/no` | Preserve in chronological order |
| native/FFI event log | `TODO` | `yes/no` | Include dropped counters |
| radio/controller counter export | `TODO` | `yes/no` | |
| energy instrument export | `TODO` | `yes/no` | Include calibration metadata |
| snapshot / inspect output | `TODO` | `yes/no` | Never include secrets |
| workload/trace manifest | `TODO` | `yes/no` | Include seed and `trace_id` |
| analysis script and version | `TODO` | `yes/no` | Reproducibility reference |
| photo/device inventory (if allowed) | `TODO` | `yes/no` | Avoid personal data |

## 11. Result classification and hardware gate

Select exactly one primary label and explain every missing gate:

- [ ] **Host loopback passed** — deterministic Rust exchange only; no radio or
  battery evidence.
- [ ] **Mobile artifact/API verified** — packaging, bindings, and source checks
  passed; no physical mobile radio exchange was observed.
- [ ] **PC BLE interoperable** — real advertisement, GATT/session, bundle,
  receipt, and reconnect were observed on supported PC hardware.
- [ ] **Android/iOS end-to-end interoperable** — a real device exchanged a
  bundle and receipt with the opposite endpoint and lifecycle/drop counters
  remained clean.
- [ ] **Battery measurement complete for this device pair** — calibrated
  radio/host/whole-device channels or clearly bounded proxies are present,
  paired runs share the trace/workload, and all unavailable fields are
  explained.
- [ ] **Incomplete / environment blocked** — list the exact missing hardware,
  OS permission, toolchain, instrumentation, or background-execution gate.

This template is complete only as a recording form. A filled record with
`unavailable` measurements is evidence of a limitation, not evidence that the
MMP/1 battery target or MMP/Φ improvement was achieved.

## 12. Operator sign-off

| Check | Result / initials |
| --- | --- |
| raw logs retained and checksummed | `TODO` |
| no secrets copied into evidence | `TODO` |
| monotonic timing used for derived durations | `TODO` |
| baseline/candidate controls matched | `TODO` |
| missing measurements have explicit reasons | `TODO` |
| specification tree unchanged | `TODO` |
| final classification reviewed | `TODO` |
