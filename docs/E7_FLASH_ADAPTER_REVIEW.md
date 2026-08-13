# E7 flash-backed provider adapter review

Date: 2026-08-13

Status: **NO-CHANGE-JUSTIFIED / DEFERRED-WITH-REASON**

## Decision

Do not add a generic flash-backed `BundleStore` adapter yet. The current
`mmp-flash-journal` crate intentionally provides only allocation-free record
framing, recovery scanning, and the provider-neutral `JournalBackend` I/O
boundary. The additive `mmp-core::EmbeddedBundleStore` contract currently
provides bounded reads, visits, and expiry; it does not define snapshot
serialization, mutation, reservation, quarantine, generation-floor, or
`Committed`/`NotCommitted`/`Indeterminate` recovery results.

Adding an adapter before those decisions would invent board geometry,
allocation limits, durability semantics, and reset behavior. That would be
inconsistent with `docs/E7_NRF52_PROVIDER_CONTRACT.md` and could silently map
an indeterminate flash write to an ordinary capacity or abort result.

## Bounded review evidence

- Parser bounds use checked lengths and the fixed `MAX_PAYLOAD_LEN`; malformed
  and truncated candidates cannot replace a valid generation.
- Scanning selects the highest valid generation and resolves equal-generation
  ties deterministically.
- `write_record` keeps commit-marker bytes out of the body call and places
  them in the final aligned backend call.
- `BodyWrite` and `CommitWrite` are documented as requiring a recovery scan;
  `CommitWrite` remains explicitly indeterminate.
- The test NOR backend enforces range/alignment checks, one-way programming,
  erase-to-`0xff`, and partial-write fault behavior.
- No violation was found in the `EmbeddedBundleStore` boundary or the journal
  recovery contract. Board-level atomicity and generation-floor policy remain
  provider obligations.

## Verification

The focused checks passed on the clean working branch:

```text
cargo test -p mmp-flash-journal --all-targets --locked --offline  # 17 passed
cargo test -p mmp-core --all-targets --locked --offline            # 126 + 5 passed
cargo clippy -p mmp-core -p mmp-flash-journal --all-targets \
  --locked --offline -- -D warnings                                # passed
cargo check -p mmp-flash-journal --target thumbv7em-none-eabihf \
  --no-default-features --locked --offline                         # passed
rustfmt --check crates/mmp-flash-journal/src/lib.rs \
  crates/mmp-flash-journal/src/backend.rs \
  crates/mmp-core/src/embedded_store.rs                            # passed
git diff --check                                                     # passed
git diff --name-only -- specification                                  # empty
```

Generated `implementation/mesh-milk/target` artifacts were removed after
verification. No protocol specification file was changed.

## Next gate

Select and record one exact board/HAL profile, then define its flash geometry,
snapshot schema, reservation/quarantine policy, generation-floor handling, and
power-loss recovery mapping. Only after those inputs are reviewed should a
board-specific provider transaction adapter be implemented.

## Configuration validation slice

The `mmp-board-nrf52840-dk` profile now exposes a `const fn`-compatible
`StorageConfig` gate. It validates caller-supplied linker/provider values for
zero capacities, program/erase alignment, checked region bounds, slot/index
consistency, bundle-slot fit, complete snapshot payload limits, and encoded
journal-frame plus program-unit alignment against the region size. It does not
select a flash map, allocator, HAL geometry, executor, radio backend, or
durability policy. The slice is therefore **FIXED** as a configuration guard,
while the board provider itself remains deferred.

## Recovery classification slice

`mmp-flash-journal` now exposes an allocation-free `classify_recovery` helper
and `RecoveryClassification` result. Given the generation selected before an
append, the attempted generation, and a post-write `ScanResult`, it returns
`Committed` only for an exact attempted-generation match, `NotCommitted` only
for the known previous generation (or an empty first journal), and
`Indeterminate` for every other state. It also rejects reused or rolled-back
attempt generations (`attempted <= previous`) as indeterminate, because the
scanner's deterministic duplicate-generation rule cannot prove which append
was visible. This is a framed-record recovery aid, not snapshot authenticity,
generation-floor enforcement, or hardware atomicity.

The journal crate also exposes an in-memory `GenerationFloor` guard. It rejects
generation zero, equal/older candidates, and `u64` overflow while leaving its
state unchanged on rejection. The guard is only a local monotonic precondition:
the provider must persist and recover the floor with its own crash-safe policy;
the helper does not provide durable anti-rollback or hardware atomicity.

## Transaction lifecycle slice

`JournalTransaction` now composes the classifier and floor into a bounded
in-memory lifecycle: `Ready → InFlight → Ready` for proven commit or
not-committed recovery, and `InFlight → Quarantined` after an unknown write.
Reservations are blocked while a transaction is quarantined; only an explicit
scan resolution can release it. The state machine does not serialize the
snapshot, persist the floor, validate bundle authenticity, or prove board
flash atomicity. After proven `NotCommitted` recovery the floor is unchanged,
so a provider may retry the same generation only after safely erasing or
reusing the failed slot. Those remain requirements of the eventual named
provider, as do durable floor persistence, complete-snapshot validation, and
reboot rollback resistance.

Focused verification for this slice:

```text
cargo test -p mmp-flash-journal --all-targets --locked --offline  # 33 passed
cargo clippy -p mmp-flash-journal --all-targets --locked --offline -- -D warnings
cargo check -p mmp-flash-journal --target thumbv7em-none-eabihf \
  --no-default-features --locked --offline
cargo fmt --package mmp-flash-journal -- --check
```

## nRF52840 NVMC boundary slice

The board profile now exposes an ARM-only `NvmcJournalBackend` that reuses
Embassy's pinned `embedded-storage` implementation and translates a
caller-selected, page-aligned internal-flash subregion into the existing
`JournalBackend` interface. The region validator rejects zero bases,
unaligned/empty regions, checked-arithmetic overflow, and ranges outside the
documented board flash capacity. The adapter uses no unsafe code and reports
re-entrant access as a bounded error instead of panicking.

This board adapter does not choose a linker map or journal offset, connect the
`mmp-core` fixed-store snapshot codec to flash, prove power-loss atomicity,
persist the generation floor, or validate recovered bundle authenticity. The
complete board provider remains deferred until those board and recovery inputs
exist.
