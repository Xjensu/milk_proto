# E7 journal writer handoff

Date: 2026-08-12

Status: **FIXED** for the provider-neutral writer slice.

## Scope

`mmp-flash-journal::write_record` is an allocation-free helper for a
`JournalBackend`. It keeps the frozen protocol and all files under
`specification/` unchanged. It does not select a board, define flash
atomicity, or implement `BundleStore`.

The caller provides the absolute record offset and a scratch buffer. The
helper validates alignment, length overflow, capacity, and scratch size. It
encodes the bounded record, fills alignment padding with erased bytes, writes
the aligned body prefix without any commit-marker byte, and then makes one
final aligned backend call whose span contains the marker. This also handles a
marker that crosses a physical write-unit boundary.

## Error and recovery contract

`BodyWrite` means the prefix operation failed before the marker-containing
call was requested; the backend may still have partial body bytes, so the
caller must recover by scanning before reusing the slot.

`CommitWrite` is explicitly indeterminate: the final operation may have
programmed none, some, or all of its bytes before returning an error. The
caller must scan the journal and must not reuse the generation or treat this
as an ordinary capacity failure. A successful call only establishes that the
backend returned success; board-specific power-loss and write-order evidence
remain required.

The helper therefore enforces marker ordering at the backend-call boundary,
but does not claim that a backend call is power-loss atomic or that its bytes
are programmed in a particular internal order.

## Verification

The focused slice passed:

```text
cargo test -p mmp-flash-journal --all-targets --locked --offline  16 passed
cargo clippy -p mmp-flash-journal --all-targets --locked --offline -- -D warnings  passed
cargo check -p mmp-flash-journal --target thumbv7em-none-eabihf \
  --no-default-features --locked --offline  passed
rustfmt --check crates/mmp-flash-journal/src/lib.rs  passed
git diff --check  passed
```

Tests cover marker exclusion from the prefix, aligned padding, byte- and
multi-byte write units, offset/capacity/scratch rejection, prefix failure,
indeterminate final failure, and recovery of the previous generation.

## Next bounded step

Choose one named board geometry and implement its `JournalBackend` adapter.
That adapter must document erase/program ordering, partial-write behavior,
power-loss tests, and the `Committed`/`NotCommitted`/`Indeterminate` recovery
mapping before it is connected to the finite provider or producer state.

## Validator-gated recovery scan (2026-08-13)

**Task status: FIXED.** `mmp-flash-journal::scan_validated` now wraps the
existing allocation-free framing scan with a caller-supplied bounded validator
for the selected highest-generation record. The validator runs before the
`ScanResult` is returned to a provider; if it rejects the payload, recovery
returns the caller's error and never silently falls back to an older
generation. This prevents a checksum-valid but semantically corrupt snapshot
from being published as provider state. Empty journals remain successful and
do not invoke the validator.

The function does not authenticate snapshots itself, persist a generation
floor, or establish flash atomicity. A board provider must pass a validator
that performs canonical snapshot decoding, bundle signature/metadata checks,
and any profile-specific integrity check, then apply its durable rollback and
indeterminate-write policy.

The initial implementation slice had four focused provider tests. The current
provider matrix has fourteen tests, including equal-generation ambiguity,
torn-newer-slot recovery, attempted-generation fencing, reserved-generation
rejection, and durable-floor rollback checks; the current command is recorded
in `docs/E7_EMBEDDED_HANDOFF.md`.

Historical focused evidence for the initial slice:

```text
cargo test -p mmp-flash-journal --all-targets --locked --offline
cargo clippy -p mmp-flash-journal --all-targets --locked --offline -- -D warnings
cargo check -p mmp-flash-journal --target thumbv7em-none-eabihf \
  --no-default-features --locked --offline
cargo fmt --package mmp-flash-journal -- --check
```

## Host two-slot provider slice (2026-08-13)

**Task status: FIXED.** `mmp-embedded-provider::JournaledFixedStore` now
composes the fixed-store FBS1 snapshot codec with two erase-aligned journal
slots. Recovery reads both slots, uses strict head-record selection, validates
the selected snapshot before publication, and restores the generation floor
and active slot. Commit writes only the inactive slot, verifies the post-write
selection, and replaces memory with the recovered durable state when an
attempt is proven absent. Backend write/erase failures quarantine the
transaction; `recover_in_place` must prove `NotCommitted` or `Committed`.

The provider is host-testable and `no_std` with caller-owned image, payload,
and record-scratch buffers. It does not claim board flash atomicity, durable
anti-rollback, coordinated producer checkpoints, or snapshot-level
authentication for local metadata. Those remain explicit nRF52 integration
gates.

Focused evidence:

```text
cargo test -p mmp-embedded-provider --all-targets --offline  # 4 passed at initial slice
cargo clippy -p mmp-embedded-provider --all-targets --offline -- -D warnings
cargo check -p mmp-embedded-provider \
  --target thumbv7em-none-eabihf --no-default-features --offline
cargo run -p layer-check --offline                              # passed
```
