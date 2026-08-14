# MMP/1 Remaining Work — Agile Execution Queue

**Status:** current queue, 2026-08-14
**Evidence rule:** every item below is derived from an open/deferred story,
roadmap gate, handoff boundary, or research question already present in this
repository. This file does not create a new protocol requirement.
**Specification rule:** `specification/` is frozen. Any proposed amendment
must be written and approved separately before implementation.

The detailed historical story record remains in `docs/AGILE_BACKLOG.md`.
This file removes completed stories from the execution view and groups the
remaining work by the project priority queue.

## Definition of done for every implementation story

- The change is narrowly scoped and preserves the frozen wire contract.
- A focused regression/property/security test covers the affected invariant;
  mandatory wire/golden, crypto, security, property/fuzzing, storage-invariant,
  and key end-to-end classes remain represented where applicable.
- The changed package passes its focused tests and strict Clippy; full matrix,
  long simulation, fuzz campaigns, and hardware checks stay in CI/nightly
  tiers according to `docs/TESTING_LIFECYCLE.md`.
- The implementation handoff/backlog evidence is updated and the commit
  contains only deliberate files.

## P0 — security and correctness

### AG-P0-01 — External crypto and formal review

- **Source/status:** E6-S4, OQ-10, `docs/SECURITY_REVIEW_DOSSIER.md`; `OPEN /
  EXTERNAL`.
- **Scope:** obtain independent review of the non-standard channel binding,
  serverless prekeys, and opportunistic ratchet compositions; optionally model
  them in Tamarin/ProVerif as the reviewer requires.
- **Dependencies:** the dossier, vectors, and fuzz recipe are already present;
  reviewer availability is external.
- **Acceptance:** signed review findings are archived, every finding is
  triaged with a regression test or an explicit accepted-risk decision, and no
  production cryptography claim is made before closure.

### AG-P0-02 — Mobile keystore/keychain ratchet persistence

- **Source/status:** E4-S1, E4-S2, E4-S4, `MOBILE_KEYSTORE_BRIDGE_DESIGN.md`;
  `OPEN / PLATFORM-GATED`.
- **Scope:** wire Android Keystore and iOS Keychain into the existing FFI
  secret-state seam; persist the full keystore-wrapped ratchet/prekey state and
  demonstrate process kill/restart recovery. The Windows provider and PC
  restart path are already verified.
- **Dependencies:** Android/iOS SDK builds and physical devices (AG-P1-04/05).
- **Acceptance:** native builds, secret-surface checks, crash/restart tests,
  and physical evidence show no plaintext secret persistence and correct epoch
  recovery.

### AG-P0-03 — RBSR and authority admission follow-ons

- **Source/status:** E5-S5, `RBSR_D3_CONFORMANCE_HANDOFF.md`,
  `RBSR_PRIORITY_PARTITION_PROPOSAL.md`; `DEFERRED-WITH-REASON`.
- **Scope:** resolve priority-class partitioning, D3 subscribed-scope
  weighting, and gossiped-root trust only after the required protocol meaning
  is approved. Do not add a selector or wire field locally.
- **Dependencies:** explicit erratum/amendment and interoperability vectors.
- **Acceptance:** approved semantics, updated vectors and negative cases, and
  differential tests against the reference implementation. Otherwise keep the
  current safe default and record `NO-CHANGE-JUSTIFIED`.

### AG-P0-04 — Measured fuzz campaign and corpus

- **Source/status:** E5-S6, `docs/SECURITY_REVIEW_DOSSIER.md`;
  `OPEN / CI-GATED`.
- **Scope:** measure a baseline, then set a CPU-hour budget derived from that
  throughput before running the cargo-fuzz targets on a Linux CI runner;
  retain the campaign corpus artifact and triage every crash or timeout.
  Deterministic seed generation and smoke coverage already exist; Windows'
  sanitizer/toolchain limitation is known.
- **Acceptance:** reproducible CI command, retained corpus artifact from the
  completed campaign, zero unexplained crashes/timeouts, and regression tests
  for every fixed finding.

### AG-P0-05 — Embedded durability and crash recovery

- **Source/status:** E7-S1, `E7_COORDINATED_CHECKPOINT_DECISION.md`,
  `E7_DURABLE_FLOOR_CODEC_HANDOFF.md`, `E7_FLASH_ADAPTER_REVIEW.md`;
  `OPEN / HARDWARE-GATED`.
- **Scope:** coordinated producer checkpoints, authenticated snapshot local
  metadata, exact linker geometry, linked image, and power-loss/HIL evidence.
  The bounded codec and provider-neutral seams are already implemented.
- **Acceptance:** crash-injection and power-loss tests prove no invalid floor,
  replay, or torn-record admission; the target linked image meets the existing
  embedded budget without changing the wire format.

## P1 — interoperability and implementation completeness

### AG-P1-01 — Linux/BlueZ peripheral build and runtime

- **Source/status:** E3-S1 host-testable implementation shipped; Linux/BlueZ
  build and runtime evidence remains `OPEN / HARDWARE-GATED`.
- **Scope:** build and run the peripheral role with BlueZ/libdbus on Linux.
- **Acceptance:** documented build, adapter discovery, listen/connect lifecycle,
  and focused tests pass on a real Linux host.

### AG-P1-02 — Real two-host BLE exchange

- **Source/status:** E3-S2 and Phase A exit gate; `OPEN / HARDWARE-GATED`.
- **Scope:** two independent PC hosts exchange a message and receipt over real
  BLE, including reconnect and failure evidence.
- **Dependencies:** AG-P1-01, two BLE adapters/hosts.
- **Acceptance:** captured reports prove handshake, authenticated session,
  bundle delivery, receipt, and clean teardown; simulator success is not used
  as a substitute.

### AG-P1-03 — Remote CI and release-artifact evidence

- **Source/status:** E0-S1, E2-S0/E2-S2/E2-S5, E8-S2; `OPEN / EXTERNAL-CI`.
- **Scope:** run the root Actions matrix on a pushed branch and verify mobile
  bindings, AAR/XCFramework jobs, signed artifacts, and downstream consumer
  checks. Local YAML validation is not sufficient.
- **Acceptance:** retained successful workflow run and downloadable artifacts;
  failures become scoped regression work rather than silently changing claims.

### AG-P1-04 — Android native radio module and device exchange

- **Source/status:** E4-S1; SDK-free shell shipped, native gate `OPEN`.
- **Scope:** implement the Android foreground-service/GATT adapter, compile it
  with the Android SDK, and perform the documented physical exchange.
- **Acceptance:** permission/Doze behavior, service-UUID filtering, lifecycle
  fencing, message/receipt, and restart evidence pass on supported devices.

### AG-P1-05 — iOS CoreBluetooth module and device exchange

- **Source/status:** E4-S2; SDK-free shell shipped, native gate `OPEN`.
- **Scope:** implement CoreBluetooth central/peripheral integration, Xcode
  build, background restoration, and physical exchange. Preserve the honest
  leaf/foreground-relay boundary.
- **Acceptance:** restoration identifiers, stale-binding fencing, backpressure,
  message/receipt, and documented background behavior pass on real iOS devices.

### AG-P1-06 — React Native native declarations and consumer validation

- **Source/status:** E4-S3 and
  `implementation/mesh-milk/REACT_NATIVE_INTEGRATION.md`; package Codegen
  metadata and the SDK-free TypeScript declaration are shipped; native consumer
  validation remains `OPEN / PLATFORM-GATED`.
- **Scope:** validate a consumer build/runtime against the existing FFI
  surface, including generated native declarations, Kotlin/Swift adapters, and
  a device or emulator smoke path.
- **Local slice completed:** the React Native facade now fail-closes unknown
  blob-progress states and inconsistent counters at the JavaScript boundary;
  the native consumer/device gate below remains open.
- **Acceptance:** generated declarations match Kotlin/Swift/Rust surfaces;
  the fixture app builds and exercises the bounded event/error contract.

### AG-P1-07 — Deferred blob and proxy policies

- **Source/status:** E5-S1/E5-S2, `E10_BLOB_CI_TEST_TIER_HANDOFF.md`;
  `DEFERRED-WITH-REASON`.
- **Scope:** approve and implement per-domain airtime accounting, streaming
  symbol production, non-origin partial-cache requests, offline patron
  reconciliation, signed proxy-summary validity, ephemeral-ID resolution, and
  the real poll schedule. Group file sharing and automatic thumbnails remain
  explicitly deferred until their contracts exist.
- **Acceptance:** written policy and vectors precede code; storage, Merkle,
  disconnect, and end-to-end tests cover each admitted request path.

### AG-P1-08 — Embedded TAL, executor, and RS benchmark

- **Source/status:** E7-S2/E7-S3/E7-S4; `OPEN / HARDWARE-GATED`.
- **Scope:** implement nRF SoftDevice/ESP-HAL transport, Embassy executor and
  flash/linked-image integration, then run the physical nRF52 Reed–Solomon
  benchmark.
- **Dependencies:** AG-P0-05 and the board profile in
  `E7_NRF52840_DK_PROFILE.md`.
- **Acceptance:** target binary, transport lifecycle, power-loss behavior, and
  measured benchmark are archived; Core remains bounded and blob-free.

### AG-P1-09 — Bluetooth SIG UUID decision

- **Source/status:** E8-S1/OQ-12; `OPEN / EXTERNAL`.
- **Scope:** obtain the 16-bit assignment or publish the quantified 128-bit
  fallback tradeoff and choose it through the existing release decision.
- **Acceptance:** external assignment or documented fallback measurement,
  mobile scan behavior, and release documentation are consistent.

### AG-P1-10 — Consumer beta/store evidence and v2 handoff review

- **Source/status:** E8-S3/E8-S5; `OPEN / EXTERNAL`.
- **Scope:** produce the consumer app/store and beta evidence requested by the
  roadmap, then obtain the human v2 handoff review. Do not infer approval from
  generated bindings alone.
- **Acceptance:** artifacts, test reports, and reviewer disposition are linked
  from the roadmap without changing frozen protocol text.

## P2 — performance and MMP/Φ research

### AG-P2-01 — Clustered mobility and 1k/10k simulation gate

- **Source/status:** E6-S1, Phase D, `E6_S1_SOCIAL_CLUSTER_HANDOFF.md`;
  `PARTIALLY DONE / RESEARCH-ONLY`.
- **Scope:** the bounded uniform, periodic, social-cluster, and heavy-tail
  clustered controls are implemented for deterministic paired replays. The
  remaining gate is trace calibration plus latency, storage, delivery, and
  survivability validation at the existing 1k tier; the 10k tier from the
  performance model remains deferred until a scalable clustered model and
  execution budget are available. Uniform quota/store measurements are not a
  substitute for clustered latency.
- **Acceptance:** fixed seeds, published distributions/confidence intervals,
  baseline comparison, and a reproducible CI/nightly command.

### AG-P2-02 — On-device battery and wake-up measurements

- **Source/status:** E4-S5/OQ-13; `OPEN / HARDWARE-GATED`.
- **Scope:** measure radio, SoC wakeups, FFI callbacks, and whole-phone energy
  on at least three device classes and the planned five-device field run.
- **Acceptance:** Android Battery Historian/iOS Instruments reports separately
  show whether the `<5%/day` target holds; no theoretical multiplier is called
  a measured optimization.

### AG-P2-03 — MMP/Φ baseline-and-candidate experiments

- **Source/status:** `MMP_PHI_EDR_HANDOFF.md`, `PROPOSAL_AUDIT_MMP_PHI.md`,
  `CONTACT_SURVIVAL_IMPLEMENTATION_PLAN.md`; `OPEN / RESEARCH-ONLY`.
- **Scope:** the simulator now has a baseline-only `ResearchExperiment` seam
  that replays one validated `ContactSchedule` and fixed text workload through
  unchanged MMP/1 nodes while retaining seed/config/trace/workload provenance.
  A provenance-bound, wire-neutral EWS replay sidecar now evaluates
  caller-supplied evidence without mutating the simulator or deriving
  probabilities from baseline metrics. The remaining work is to add selectable
  candidate adapters for Energy-Dual Rendezvous, contact-survival scheduling,
  and fountain/DoF transfer; keep the reference MMP/1 router selectable and
  unchanged. EWS, contact-survival, and DoF remain sidecar research models and
  must not be reported as candidate transport metrics until paired replay
  aggregation exists.
- **Acceptance:** identical seeds/workloads for baseline and candidate,
  separately reported delivery, latency, copies, bytes/airtime, storage, CPU,
  and energy-proxy distributions with confidence intervals. The “2×” claim is
  `CONFIRMED`, `PARTIALLY CONFIRMED`, `NOT CONFIRMED`, `REFUTED`, or
  `INCONCLUSIVE` only after these measurements.

### AG-P2-04 — Open measurement questions

- **Source/status:** `specification/research/open-questions.md`; `OPEN` items
  OQ-2, OQ-5, OQ-7, OQ-14, OQ-15, OQ-16, OQ-17, and OQ-18. OQ-1 remains
  recorded there as a verification question; its proposed RBSR-over-ID
  direction is implemented, but the question is not rewritten without a
  new explicit evidence entry.
- **Scope:** throughput, short-contact RBSR behavior, RS decode cost, 2.4 GHz
  attenuation, residual-contact prediction, authority provisioning/abuse,
  revocation propagation, and thumbnail codec/size measurements.
- **Acceptance:** each question receives measured evidence, a confidence tag,
  and an update to its existing owner document; no unsupported constant is
  promoted into the frozen specification.

### AG-P2-05 — Explicitly deferred v2 research

- **Source/status:** OQ-6, OQ-8, OQ-9, OQ-11 and the roadmap's v2 notes;
  `DEFERRED-WITH-REASON`.
- **Scope:** RaptorQ/IPR, group-admin rotation, privacy profiles, and hybrid
  post-quantum Noise maturity. Track external standards and revisit only when
  the v1 gates are closed.
- **Acceptance:** no v1 wire or dependency change; each item remains linked to
  its source question and is reopened only by new evidence or an explicit
  product decision.

## Dependency spine

`AG-P1-01 → AG-P1-02 → Phase A exit → AG-P1-04/05 → AG-P0-02`
`AG-P0-05 → AG-P1-08 → AG-P1-08 benchmark`
`E5-S4 vectors + AG-P0-04 → AG-P0-01 external review`
`AG-P2-01/02 baseline → AG-P2-03 MMP/Φ comparison`

No item in this queue authorizes a specification edit. A demonstrated
contradiction must first be recorded as a proposal with compatibility and
test-vector impact, then explicitly approved.
