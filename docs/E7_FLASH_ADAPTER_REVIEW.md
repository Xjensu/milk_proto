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
