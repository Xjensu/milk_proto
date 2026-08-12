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
