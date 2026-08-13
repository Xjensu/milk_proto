# `mmp-board-nrf52840-dk`

This crate is the first compile-time board profile for the Nordic nRF52840 DK
(PCA10056).  It is intentionally a small `no_std` configuration API and a
target compile probe for `embassy-nrf` 0.11.0 with its `nrf52840` chip feature.

The dependency is target-only and uses `default-features = false`.  Therefore
the probe does not silently enable Embassy's `rt` feature, an executor, a
radio backend, or a host runtime.  The crate exposes only the board identifier,
target triple, and documented total SoC capacities.  Total capacity is not
application space: boot/vector reservations, linker sections, radio firmware,
journal geometry, and wear policy remain unselected.

## Verification

From `implementation/mesh-milk`:

```text
cargo check -p mmp-board-nrf52840-dk \
  --target thumbv7em-none-eabihf --no-default-features --locked
cargo check -p mmp-board-nrf52840-dk --locked
cargo clippy -p mmp-board-nrf52840-dk --target thumbv7em-none-eabihf \
  --no-default-features --locked -- -D warnings
```

The host check validates the allocation-free constants only; it does not
compile Embassy because the HAL dependency is intentionally ARM-target-only.
The ARM check is the compatibility gate for the selected chip feature.  A
passing check is not evidence of a linked image, working J-Link session, BLE
radio behavior, flash geometry, or production readiness.

## Deliberate non-goals

This profile does not choose an executor, clock setup, SoftDevice/bootloader
reservation, linker script, allocator, NVMC/QSPI provider, journal offset,
radio/TAL implementation, or power-measurement result.  Those require named
board revisions, provider geometry, fault-injection tests, and hardware
transcripts before they can be added to the implementation workspace.
