# Mobile throwing-contract handoff

Status: implementation handoff; no wire or specification change.

## Completed in this continuation

Commit `7501013` closes a release-gate mismatch between the combined mobile
API check and the single-language packaging checks.

`scripts/check_mobile_binding.py` now validates, in addition to the generated
method names and secret denylist:

- Kotlin `@Throws(FfiException::class)` for every method listed in
  `EXPECTED_MMP_NODE_THROWING_METHODS`;
- Swift `throws` for the same method set.

The Android AAR and iOS XCFramework jobs use this one-language scanner through
their artifact verification paths. Before this change, stale generated output
could pass those independent jobs while failing the stronger dual-language
`check_mobile_api_surface.py` job.

The focused tests include complete generated fixtures and one negative test for
each host language. Android and iOS artifact fixtures were updated to represent
the throwing ABI emitted by the current Rust FFI manifest.

## Evidence

Run from `implementation/mesh-milk`:

```powershell
python -m unittest scripts.tests.test_mobile_binding `
  scripts.tests.test_mobile_api_surface -v
python -m unittest discover -s scripts/tests -v
```

Observed result: 153 Python tests passed, with 10 platform-specific symbolic
link tests skipped on the Windows host. `git diff --check` also passed.

The prior Rust verification baseline remains green from this continuation:

```powershell
cargo test --workspace --all-targets -- --test-threads=1
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p layer-check
python scripts/check_spec_status.py --repo ../..
```

## Boundaries

- `specification/` was not modified; the frozen wire contract remains the
  authority.
- No generated Android/iOS binding was claimed to compile on a native SDK or
  to interoperate with physical BLE hardware. This is a deterministic
  source/artifact gate only.
- Existing user-owned dirty files and untracked audit/candidate files were not
  staged or committed.

## Required next implementation order

1. **E7 producer-capacity contract (highest correctness risk).** Do not change
   `store_own` from infallible `put` to `try_put` as a shortcut. The audit found
   producer state (ratchets, group sender keys, file indexes, pending records,
   and SeenIndex entries) can advance before storage succeeds. First add a
   private, test-only rejecting provider/harness and red tests that require
   rejection to leave all producer state, diagnostics, and delivery records
   unchanged. Then design reservation/prepare/commit semantics and only after
   that expose a production capability.
2. **Embedded path.** `mmp-node-embassy` and `mmp-tal-embedded` currently prove
   only no-std shell compilation. A fixed-capacity store, executor integration,
   named nRF52/ESP32 backend, linked image, memory measurements, and HIL phone
   exchange are still required. See `docs/E7_EMBEDDED_HANDOFF.md`.
3. **Normative schema decisions.** Friendship and `avatar_root` layouts remain
   implementation-provisional relative to the frozen documents. Do not add
   strict decoders, dual-publish behavior, or migration bytes until an erratum
   or explicit migration decision fixes the exact wire encoding. See the
   delegated schema audit and the existing conformance handoffs.
4. **Physical mobile gates.** Run the documented Android/iOS build and BLE
   evidence procedures on real devices. Source-level throwing checks do not
   establish UniFFI generation, OS background behavior, GATT behavior, or
   radio interoperability.

## Adversarial review

### Security auditor

**Potential flaw:** a regex can accept an annotation or `throws` token in an
unrelated declaration.

**Resolution:** the check is intentionally a source-level packaging gate, not a
language parser. It binds the token to the exact generated method signature and
is paired with the existing manifest and secret-surface checks. Native compiler
and artifact tests remain separate gates.

### Distributed-systems reviewer

**Potential flaw:** fixing the mobile ABI surface does not make producer storage
transactional.

**Resolution:** capacity/rejection semantics remain explicitly deferred to the
E7 reservation/commit design; no false production guarantee is made here.

### Performance engineer

**Potential flaw:** source scanning adds no evidence about mobile battery or
BLE throughput.

**Resolution:** the change protects release compatibility only. Battery,
contact duration, and throughput still require device measurements.

### Skeptical professor

**Potential flaw:** green fixture tests could drift from real UniFFI output.

**Resolution:** the throwing method manifest is checked against Rust `Result`
exports by `test_mobile_api_surface.py`; generated artifact snapshots and real
SDK builds remain future acceptance evidence.

## Stop conditions for the next model

Stop and write/update a handoff if context or execution budget becomes limited.
Do not modify frozen specifications to resolve the Friendship/avatar ambiguity,
do not stage unrelated user changes, and do not claim E7 or mobile physical
interoperability complete without the evidence listed above.
