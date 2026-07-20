# Traceability discrepancies

## DISC-001 — Development UUID conflicts with production legacy beacon layout
- Specification: `specification/09-transport-ble.md` §2 and §3 require a SIG-assigned 16-bit service UUID for the production 31-byte legacy Service Data layout; `specification/research/open-questions.md` OQ-12 names assignment as a release blocker.
- Implementation: `implementation/mesh-milk/crates/mmp-tal-peripheral/src/lib.rs` defines a 128-bit `DEV_SERVICE_UUID`; `src/linux.rs` documents that its UUID plus 24-byte payload cannot fit the specified layout.
- Evidence: `implementation/mesh-milk/crates/mmp-tal-peripheral/src/lib.rs`; `implementation/mesh-milk/crates/mmp-tal-peripheral/src/linux.rs`; `specification/research/open-questions.md` OQ-12.
- Impact: BLE development behavior must not be presented as production-beacon conformance; the blocker affects discovery and iOS-targeted scan assumptions.
- Course treatment: block
- Confidence: High
- Resolution owner: protocol governance and BLE transport lead

## DISC-002 — Whole-phone energy multiplier ranges disagree
- Specification: `specification/13-performance-model.md` §1.2 uses `k_sys ∈ [4,12]`; `specification/research/open-questions.md` OQ-13 still records `[3,10]`.
- Implementation: none in audited baseline.
- Evidence: `specification/13-performance-model.md` §1.2; `specification/research/open-questions.md` OQ-13.
- Impact: the <5%/day planning claim and mitigation threshold cannot be taught as a single controlled model until the ledger is reconciled and measured.
- Course treatment: block
- Confidence: High
- Resolution owner: performance-model maintainer

## DISC-003 — F-A6 is not directly resolvable through the source register
- Specification: `specification/13-performance-model.md` §1.3 invokes `[F-A6]` for the Bluetooth-Mesh-LPN lifetime analogy; `specification/02-research-report.md` defines F-A6 as an internal finding sourced by `[A23][A24]`.
- Implementation: none in audited baseline.
- Evidence: `specification/13-performance-model.md` §1.3; `specification/02-research-report.md` §1.1; `course/references/source-register.md`.
- Impact: a mechanical course-source join cannot resolve `F-A6` as a register key, so students could misread a finding label as independently cited evidence.
- Course treatment: teach-as-gap
- Confidence: High
- Resolution owner: research bibliography maintainer

## DISC-004 — Required golden vectors are not committed
- Specification: `specification/10-message-format.md` §6 and `specification/12-rust-blueprint.md` §7.1 require fixed-key vectors, specified negative cases, and an independent decoder cross-check.
- Implementation: `implementation/mesh-milk/vectors/README.md` states that the directory intentionally contains no vectors at Stage 0.
- Evidence: `implementation/mesh-milk/vectors/README.md`; `course/audit/implementation.md` Test and vector inventory.
- Impact: codec unit tests do not provide portable interoperability evidence or pin canonical bytes across independent implementations.
- Course treatment: block
- Confidence: High
- Resolution owner: wire-format and test-infrastructure lead

## DISC-005 — Fuzz harnesses are present but no fuzz execution evidence is committed
- Specification: `specification/10-message-format.md` §6 and `specification/12-rust-blueprint.md` §7.2 require parser fuzzing and pre-auth allocation checks.
- Implementation: four `cargo-fuzz` targets exist, while `implementation/mesh-milk/fuzz/README.md` records no curated corpus and describes runs as external/manual CI work.
- Evidence: `implementation/mesh-milk/fuzz/Cargo.toml`; `implementation/mesh-milk/fuzz/README.md`; `implementation/mesh-milk/fuzz/fuzz_targets/`.
- Impact: harness presence is not evidence of panic, OOM, hang, allocation, or parser-resilience properties under generated input.
- Course treatment: block
- Confidence: High
- Resolution owner: security test-infrastructure lead

## DISC-006 — Platform and cross-target conformance evidence is absent
- Specification: `specification/09-transport-ble.md` §6 and `specification/12-rust-blueprint.md` §7.2 require platform mappings and a desktop, mobile, and embedded target matrix.
- Implementation: `mmp-tal-mobile` exposes shared types only; `mmp-tal-embedded` is a no_std placeholder; the peripheral backend is Linux-gated and the audited host selected unsupported paths.
- Evidence: `implementation/mesh-milk/crates/mmp-tal-mobile/src/lib.rs`; `implementation/mesh-milk/crates/mmp-tal-embedded/src/lib.rs`; `implementation/mesh-milk/crates/mmp-tal-peripheral/src/lib.rs`; `course/audit/implementation.md` Mobile and embedded boundaries.
- Impact: no audited result establishes Android, iOS, Linux/Windows, or nRF52/ESP32 operational conformance; hardware BLE and cross-target claims remain unsupported.
- Course treatment: block
- Confidence: High
- Resolution owner: platform integration lead
