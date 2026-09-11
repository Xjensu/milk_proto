# MMP testing lifecycle

This policy keeps the protocol's correctness evidence intact while shortening
the developer feedback loop. It changes test scheduling and fixture reuse; it
does not relax a security, storage, wire, cryptographic, or interoperability
invariant.

## Mandatory evidence classes

The following remain required before a release claim:

| Class | Evidence | Current location |
| --- | --- | --- |
| Wire/golden vectors | Canonical bytes, strict negative vectors, independent readers | `implementation/mesh-milk/vectors/`, `mmp-cli` vector tests |
| Cryptography | Domain separation, strict signatures, Noise/prekey/ratchet/group tests | `mmp-crypto` and `mmp-cli` conformance tests |
| Security/regression | Replay, admission, lifecycle, revocation, Byzantine-local resource tests | `mmp-core`, `mmp-node`, `mmp-sim` |
| Property/fuzzing | Proptest mutation checks, deterministic seed corpus, libFuzzer targets | `mmp-types`, `fuzz/`, CI fuzz jobs |
| Storage invariants | Bounded admission, replacement deltas, snapshot recovery, transactional acceptance double | `mmp-store` tests |
| Key end-to-end flows | Direct delivery/receipt, sealed first contact, groups, files, restart/FFI paths | `mmp-node`, `mmp-ffi`, `mmp-cli` |

Removing a test is allowed only when another test proves the same invariant by
the same or a stronger mechanism. A shorter test list by itself is not an
acceptance criterion.

## Local fast tier (target: 10-30 seconds after compilation for small or parallel scopes)

From `implementation/mesh-milk`, run the changed package and its Cargo
dependencies:

```powershell
python scripts/fast_check.py --package mmp-node
python scripts/fast_check.py --package mmp-crypto --package mmp-types
python scripts/fast_check.py --python
```

For a Rust scope, the helper runs rustfmt only on changed `.rs` files inside the
selected package(s), then package-scoped all-target tests with four bounded
libtest workers and warnings-as-errors Clippy. `mmp-store` recovery fixtures
use unique RAII temporary directories, so storage scopes can use the same
parallel local loop without cross-test state leakage. This avoids unrelated
pre-existing formatting drift elsewhere in the workspace while shortening the
local loop; if no Rust file changed in the selected scope, the formatting step
is skipped.
`--all` deliberately retains the full-workspace `cargo fmt --all -- --check`
gate and serial test execution. Cargo builds the selected package's dependencies
automatically. If the root manifest, lockfile, or toolchain changes, the helper
refuses to guess and requires an explicit `--package` or `--all`.

The 10-30 second target is a budget for small changed crates and already-built
dependency scopes. Package checks, including `mmp-store`, use four workers.
The full workspace command remains serial for reproducibility while the other
workspace test families are reviewed independently; this is no longer required
by the storage fixture implementation.

The repository still has pre-existing rustfmt drift in several tracked Rust
files. Package-scoped checks no longer report that unrelated drift; the `--all`
gate continues to expose it until the baseline is repaired or a project
formatter version is pinned. This is reported as baseline debt, not treated as
permission to reformat unrelated files in a protocol change.

For a script-only edit, `--python` runs the Python gate. Fuzz recipe changes
run the deterministic seed check; the sanitizer campaign is never hidden in a
developer's 30-second loop.

Edits under `implementation/mesh-milk/vectors/` automatically select the
`mmp-cli` package in automatic mode. That package owns the independent reader,
generator drift checks, and negative-vector conformance tests, so changing a
golden file cannot accidentally look like a documentation-only edit.

## Normal CI tier (target: 2-5 minutes per host job)

The host matrix runs the full Rust workspace test matrix and Clippy. Separate
jobs keep Android/iOS cross-checks, mobile artifact verification, no-std
dependency closure, layer checks, and security/dependency checks visible. The
normal CI tier includes all mandatory fast tests and all active non-ignored
tests, but does not run long ignored simulator drills or a multi-hour fuzzing
campaign.

All host-matrix Cargo build, test, Clippy, and named storage-gate commands use
the committed lockfile (`--locked`). This makes the 2-5 minute CI tier
reproducible instead of allowing an unrelated dependency-resolution change to
look like a protocol regression.

Similar mobile source fixtures are shared where practical. Kotlin and Swift
surface tests use the same manifest-driven fixture shape, while CLI/FFI tests
retain separate adapter assertions because they exercise different mutex,
restart, and artifact boundaries. These are not interchangeable invariants.

## Nightly/release tier

The scheduled/manual extended tier runs:

- ignored `mmp-sim` deployment and scale tests;
- the deterministic fuzz corpus on every workflow and bounded libFuzzer smoke
  runs on the weekly schedule or an explicit manual dispatch;
- release manifest/signature checks and platform artifact jobs; and
- named-device Linux/BlueZ, Android, iOS, and embedded hardware gates when
  those runners and devices are available.

The full workspace matrix remains a release prerequisite even if a developer
only ran the fast tier locally. The six ignored simulator tests include a
roughly 19-minute 1,000-node release-scale run in the current evidence model;
the extended job is therefore a full scheduled/manual tier, not a 2-5 minute
normal-CI job.

## What is intentionally not merged or deleted

No security, storage, wire, crypto, or end-to-end test is removed in this
change. Similar tests are consolidated only at the fixture/parameter level;
negative cases remain separate when they protect different failure boundaries.
Long-running simulator and fuzz work is scheduled in CI rather than silently
skipped. Hardware tests remain external evidence and cannot be replaced by a
host fixture.
