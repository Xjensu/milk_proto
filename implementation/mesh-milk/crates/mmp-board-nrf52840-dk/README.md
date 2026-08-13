# `mmp-board-nrf52840-dk`

This crate is the first compile-time board profile for the Nordic nRF52840 DK
(PCA10056).  It is intentionally a small `no_std` configuration API and a
target compile probe for `embassy-nrf` 0.11.0 with its `nrf52840` chip feature.

The dependency is target-only and uses `default-features = false`.  Therefore
the probe does not silently enable Embassy's `rt` feature, an executor, a
radio backend, or a host runtime.  The crate exposes only the board identifier,
target triple, documented total SoC capacities, and the exact target-only HAL
selection. `nrf52840_dk_config()` returns the named
`Nrf52840DkConfig` record (`nrf52840-dk-pca10056`, `nrf52840`,
`thumbv7em-none-eabihf`, `embassy-nrf` 0.11.0, and the `nrf52840` chip
feature). Total capacity is not application space: boot/vector reservations,
linker sections, radio firmware, journal geometry, and wear policy remain
unselected. No board-revision or fitted-component value is asserted because
the repository does not yet contain an authoritative hardware inventory.

It also exposes `StorageConfig::validate`, a `const fn` gate for a future
board-specific profile. The caller supplies the linker-derived region,
program/erase units, fixed-capacity accounting, and snapshot limits. The gate
rejects zero capacities, unaligned regions, checked-arithmetic overflow,
out-of-range regions, slot/index mismatches, and two-slot snapshots that do
not fit the journal payload or aligned region. On ARM, `NvmcJournalBackend` adapts the
already-pinned Embassy nRF52840 internal-NVMC driver to the provider-neutral
`mmp-flash-journal::JournalBackend` contract. It accepts only a caller-validated
page-aligned subregion; it does not choose a flash map or provide durability,
power-loss atomicity, or snapshot authenticity.

`StorageConfig::validate_with_floor_region` additionally checks a caller-selected
`DurableFloorConfig`: the floor region is disjoint from the snapshot journal,
fits two independently erase-aligned GFL1 record slots, and passes checked
program/erase alignment, bounds, and arithmetic validation. This remains a
geometry gate only; it selects no linker address and provides no authentication
or cross-region atomicity.

On ARM, `recover_nvmc_fixed_store` composes that adapter with the generic
`mmp-embedded-provider::JournaledFixedStore`. It derives a two-slot layout
relative to the supplied region and reuses FBS1 snapshot validation and
quarantine. The additive `recover_nvmc_fixed_store_with_durable_floor`
variant also passes a caller-owned persisted generation floor into recovery,
rejecting an older checksum-valid image. The
`recover_nvmc_fixed_store_with_exact_durable_floor` variant requires exact
generation equality and rejects both cross-region ordering mismatches. Neither
helper persists or authenticates the floor or selects a linker geometry; the
caller still supplies the region, slot size, payload bound, recovery image,
and the independently durable floor. The exact `0/0` match is only an explicit
first-install or uninitialized case.

After a board linker map and provider contract are approved, a board-specific
crate can place `StorageConfig::validate()` in a `const` assertion. No such
assertion is enabled here because this profile intentionally has no selected
flash region or provider geometry.

## Verification

From `implementation/mesh-milk`:

```text
cargo test -p mmp-board-nrf52840-dk --all-targets --locked
```

The host tests cover the caller-region validator and fixed geometry. The
ARM-only adapter is compile-checked below because the host cannot construct an
Embassy peripheral:

```text
cargo check -p mmp-board-nrf52840-dk \
  --target thumbv7em-none-eabihf --no-default-features --locked
cargo check -p mmp-board-nrf52840-dk --locked
cargo clippy -p mmp-board-nrf52840-dk --target thumbv7em-none-eabihf \
  --no-default-features --locked -- -D warnings
cargo test -p mmp-embedded-provider --all-targets --locked --offline
```

The host check validates the allocation-free constants only; it does not
compile Embassy because the HAL dependency is intentionally ARM-target-only.
The ARM check is the compatibility gate for the selected chip feature.  A
passing check is not evidence of a linked image, working J-Link session, BLE
radio behavior, flash geometry, or production readiness.

## Deliberate non-goals

This profile does not choose an executor, clock setup, SoftDevice/bootloader
reservation, linker script, allocator, journal offset, radio/TAL
implementation, or power-measurement result. The NVMC adapter is only an
I/O translation boundary; provider geometry, journal policy, fault-injection
tests, and hardware transcripts are still required before production use.
