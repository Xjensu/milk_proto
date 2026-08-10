# E10 Blob and CI Test-Tier Handoff

**Status:** implemented and verified
**Date:** 2026-08-10
**Scope:** test coverage and CI command alignment; no protocol or specification change

## Implemented

### Multi-stripe blob regression

`mmp-node/src/tests.rs` now contains
`blob_multi_stripe_request_applies_block_relative_bitmap_offset`. It creates a real
two-stripe file, sends a `BLOB_REQ` for `block = 1` with bitmap bit zero set, and
asserts that the selected holder is global symbol `RS_TOTAL_N`. This protects the
stripe-major bitmap interpretation from regressions that accidentally serve stripe 0.

The test is intentionally test-only: the existing serving implementation already
applies the correct offset, so no production or wire change was justified.

### Host CI all-targets coverage

The root workflow host job now runs:

```text
cargo test --workspace --all-targets --all-features --locked -- --test-threads=1
```

The previous command omitted Cargo examples. The expanded command compiles and runs
the workspace's TAL examples (`scan.rs` and `serve.rs`) as well as the ordinary tests,
while retaining the serial store-test discipline.

The Python workflow contract test was updated to require this exact command, preventing
future drift back to a narrower host matrix.

## Verification

- Focused blob regression: 1 passed.
- Full `mmp-node` library suite: 140 passed, 0 failed.
- `mmp-node` all-target Clippy with `-D warnings`: passed.
- Full workspace all-targets test: exit 0; 282.9 seconds locally. The simulator's
  ordinary suite took approximately 231 seconds; all other workspace crates and
  examples also passed.
- CI workflow contract: 18 passed.
- Full Python script suite: 157 passed, 10 expected skips.
- `cargo fmt --all -- --check` remains red only because of pre-existing formatting
  drift in unrelated files; no formatter rewrite was applied.

## Test-tier interpretation

The user's reduced test policy is preserved:

- local fast loop: changed package/dependencies, Clippy, and formatting;
- normal CI: workspace all-targets test plus the existing build, Clippy, store,
  platform-check, and security jobs;
- nightly/manual release tier: simulator scale drills, fuzz smoke runs, packaging,
  and hardware/platform gates.

The all-targets correction does not move fuzzing or long ignored simulator drills into
the local loop. It only closes a normal-CI coverage hole.

## Adversarial review

- **Security Auditor — potential flaw:** a bitmap offset test may not cover malformed
  or forged symbols. **Resolution:** existing Merkle verification and malformed-symbol
  tests remain in the mandatory node suite; this regression targets the distinct
  cross-stripe indexing invariant.
- **Performance Engineer — potential flaw:** all-targets makes CI slower. **Resolution:**
  the measured run is 282.9 seconds, within the intended 2–5 minute normal-CI band;
  local development keeps the package-scoped fast tier.
- **Distributed Systems Reviewer — potential flaw:** examples could be mistaken for
  hardware evidence. **Resolution:** compiling/running zero-test examples proves only
  target coverage; Linux/BlueZ exchange remains an external gate.
- **Battery Specialist — potential flaw:** no battery behavior is validated here.
  **Resolution:** this change is explicitly a test/coverage improvement; device
  battery measurements remain pending on Android/iOS hardware.
- **Skeptical Professor — potential flaw:** one test cannot prove all blob semantics.
  **Resolution:** it is an additive invariant test, not a replacement for golden
  vectors, Merkle/property tests, or end-to-end file transfer.
- **Inventive Scientist — potential flaw:** changing CI could conceal a protocol
  change. **Resolution:** the workflow and test are wire-neutral; frozen
  `specification/` files remain untouched.

## Next steps

1. Reconcile the existing dirty API/mobile documentation with the committed typed FFI
   revocation surface before adding documentation-enforcement tests.
2. Keep origin-targeted `BLOB_REQ` scheduling, airtime accounting, and streaming
   symbols deferred until their normative and performance contracts are approved.
3. Obtain real Linux/BlueZ two-host, Android/iOS runtime, battery, and external crypto
   review evidence before claiming production readiness.
