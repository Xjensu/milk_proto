# E7 embedded/no_std readiness handoff

Status: audit-only. The frozen `specification/` tree, wire format, lower-crate
APIs, and platform schedulers are unchanged. This handoff records the first
reproducible E7 boundary rather than claiming that an MCU port exists.

## Audit result (2026-08-09)

The two E7 crates have different readiness levels:

| Crate | Host/cross check | What is actually present | Blocking gap |
| --- | --- | --- | --- |
| `mmp-tal-embedded` | `cargo check -p mmp-tal-embedded --target thumbv7em-none-eabihf --no-default-features` passes | `#![no_std]`; bounded generic `EmbeddedEventQueue<const CAPACITY>` over the shared `TalEvent` vocabulary | no nRF52/ESP32 radio backend, GATT lifecycle, IRQ/event bridge, or target binary |
| `mmp-node-embassy` | host check and the same target check pass after `750c807` | `#![no_std]` marker and no executor code | no fixed-capacity flash store, Embassy executor, TAL backend, or target binary |

The following failure is retained as historical evidence from before
`750c807`; it is not the current result:

```text
cargo check -p mmp-node-embassy --target thumbv7em-none-eabihf --no-default-features
error[E0463]: can't find crate for `std`
.../getrandom-0.2.17/src/error_impls.rs
```

Before `750c807`, `--no-default-features` on the root package could not remove dependency defaults
when the dependency manifests did not expose a corresponding feature. The
pre-fence feature tree selected `mmp-core` `default`/`std` for
`mmp-node-embassy`, while `mmp-store` selected `mmp-core` and `mmp-crypto`
defaults.
This is an E7-S1/E7-S3 dependency-closure gap, not an Embassy compiler bug.

The existing root CI `no-std-size-gate` intentionally excludes both E7 crates
and measures only per-crate `.rlib` archives for the lower protocol crates. It
does not prove a linked nRF52 image, static RAM, executor stack, or radio
backend footprint. The documented `<=256 KiB` flash / `<=48 KiB` RAM numbers
therefore remain target acceptance criteria, not current measurements.

## Reproducible host checks

Run from `implementation/mesh-milk` with the repository's stable toolchain and
the installed `thumbv7em-none-eabihf` target:

```powershell
# TAL placeholder: this is the only E7 crate currently cross-checkable.
cargo check -p mmp-tal-embedded --target thumbv7em-none-eabihf --no-default-features

# Lower no_std proof used by the existing size gate.
cargo check --target thumbv7em-none-eabihf --no-default-features `
  -p mmp-types -p mmp-crypto -p mmp-core -p mmp-router -p mmp-rs-code -p mmp-tal -p mmp-tal-embedded

# Current Embassy shell closure (passes after `750c807`; no executor or store yet).
cargo check -p mmp-node-embassy --target thumbv7em-none-eabihf --no-default-features
```

The successful checks demonstrate only compilation for a target without a
standard library. They do not demonstrate that an Embassy executor can be
linked or that BLE packets can be exchanged.

## Required implementation sequence

1. **E7-S1 dependency closure.** Introduce an explicit embedded/no-alloc path
   in every lower dependency used by the node shell. `mmp-store` needs a
   fixed-capacity/flash-log implementation and must not pull its std snapshot;
   `mmp-core` and `mmp-crypto` must be selected with `default-features = false`.
   Keep the existing std implementation as a separate feature. Add a target
   check for the complete closure only after this split compiles.
2. **E7-S3 executor shell.** Add Embassy dependencies behind target features,
   inject the existing sans-IO engines and a monotonic clock, and keep all
   protocol state transitions in the shared core. The shell must not add a
   second protocol implementation.
3. **E7-S2 TAL backend.** Add one target feature per backend (nRF52
   `nrf-softdevice`; ESP32 `esp-hal`/the selected NimBLE integration), with a
   board-specific binary/example that owns interrupts, GATT events, and radio
   scheduling. No backend dependency is currently declared in this workspace.
4. **Linked-image gate.** Replace the rlib proxy with a reproducible release
   binary for a named board, `size`/map-file extraction, and a static-RAM
   accounting method. Record optimization level, LTO, panic strategy, linker
   script, board revision, and feature set alongside each measurement.
5. **OQ-7 benchmark and HIL.** Benchmark Reed-Solomon decode at `k <= 64` on
   an nRF52-class board, then run a phone-to-sensor friendship-buffer exchange
   with packet loss, reset, and power-cycle cases. Host tests cannot close
   either gate.

## Toolchain and hardware prerequisites

The repository currently pins only the Rust stable channel and
`thumbv7em-none-eabihf`. Before implementation begins, record exact versions
for each target rather than relying on floating dependency resolution.

### nRF52

- a named nRF52832 or nRF52840 board and its memory map/linker script;
- the selected `nrf-softdevice` and Embassy-compatible HAL versions;
- an ARM embedded linker (`rust-lld` or an explicitly selected
  `arm-none-eabi` toolchain), panic handler, and defmt/logging policy;
- a debug probe/runner (for example, a supported CMSIS-DAP or J-Link path) and
  a repeatable flash/reset command;
- a power monitor and a BLE phone peer for the friendship-buffer HIL test.

### ESP32

- the exact chip family (ESP32 Xtensa versus an RISC-V variant), because its
  Rust target and linker differ;
- the selected `esp-hal`/NimBLE integration and matching Rust toolchain;
- chip linker script, flash runner, reset control, and serial/log capture;
- a power monitor and a phone peer for the same bundle exchange matrix.

Neither backend, board target, linker script, runner, or power fixture is
present in the current repository. Their absence is why this audit does not
add a fake target feature or claim a measured memory/battery result.

## Acceptance and non-claims

E7 can be marked complete only when all of the following are evidenced:

- `cargo check` of the full embedded dependency closure for each named target;
- a linked and stripped board image within `<=256 KiB` flash and `<=48 KiB`
  static RAM, with the measurement artifact retained;
- an nRF52-class RS decode benchmark with input sizes, cycles, stack/RAM, and
  compiler settings;
- a phone-to-sensor authenticated bundle exchange, including reset and loss
  cases, reproduced from a documented test matrix.

This audit does **not** claim MCU performance, BLE interoperability, battery
life, executor correctness, or production readiness. It identifies the exact
first failing seam so the next implementation can address it without changing
the frozen protocol specification.

## Adversarial review

### Skeptical Professor

**Potential flaw:** a `no_std` crate attribute can be mistaken for a complete
embedded port.

**Alternative:** require target compilation, linked-image evidence, and HIL
traffic before calling the port functional.

**Resolution:** this handoff separates placeholder compilation from every
functional acceptance gate.

### Security Auditor

**Potential flaw:** casually enabling a heapless path could silently change
capacity limits or rejection behavior.

**Alternative:** derive fixed capacities from existing protocol bounds, add
negative tests for exhaustion, and keep wire behavior unchanged.

**Resolution:** no feature or capacity change is introduced in this audit;
E7-S1 must specify those bounds before code lands.

### Performance Engineer

**Potential flaw:** an rlib size proxy does not predict link-time dead-code
elimination, stack use, or radio-driver overhead.

**Alternative:** measure a named board image and capture map/stack data.

**Resolution:** the existing proxy remains a regression signal only; the
linked-image gate is explicitly outstanding.

### Distributed Systems Reviewer

**Potential flaw:** an executor shell could duplicate protocol state and drift
from the host implementation.

**Alternative:** drive the same sans-IO engines through injected clock/events.

**Resolution:** the implementation sequence makes shared-engine reuse an
acceptance condition.

### Battery Specialist

**Potential flaw:** a successful bundle exchange says nothing about idle power
or background radio duty cycle.

**Alternative:** add a board power trace and phone-side energy record to HIL.

**Resolution:** battery evidence is a separate prerequisite, not inferred from
compilation or throughput.

### Inventive Scientist

**Potential flaw:** backend-specific optimizations introduced before a stable
no-alloc boundary can make comparisons irreproducible.

**Alternative:** establish the fixed-capacity core and deterministic benchmark
first, then compare backend implementations under the same workload.

**Resolution:** this handoff freezes the order and rejects unmeasured
optimizations as production claims.

## Files and boundaries

- `implementation/mesh-milk/crates/mmp-tal-embedded/`: bounded ingress-seam cross-check
  and queue implementation.
- `implementation/mesh-milk/crates/mmp-node-embassy/`: dependency closure
  audited; unchanged.
- `.github/workflows/ci.yml`: existing lower-crate rlib gate audited; unchanged.
- `specification/`: unchanged.
- `implementation/mesh-milk/crates/mmp-types/`: untouched (including existing
  user changes).

## Dependency-fence correction and CI regression (2026-08-09)

The first audit above captured the pre-fence state and is retained as an
historical record. Commit `750c807` then changed only the empty Embassy shell's
path dependency declarations to use `default-features = false`; it did not add
an executor, a store backend, a TAL implementation, or a protocol transition.

The current closure was rechecked after that change:

```text
cargo check -p mmp-node-embassy --target thumbv7em-none-eabihf --no-default-features
Finished `dev` profile ...

cargo check -p mmp-node-embassy -p mmp-tal-embedded \
  --target thumbv7em-none-eabihf --no-default-features
Finished `dev` profile ...
```

Both commands pass with the installed target. `mmp-node-embassy` therefore
belongs in the no_std *compile-closure* regression, even though it remains a
seven-line `#![no_std]` shell. The root CI `no-std-size-gate` now runs the
combined check before the lower-crate rlib size proxy. A future reintroduction
of a default `std` edge will fail this target check rather than waiting for a
hardware build.

This regression proves only that the declared dependency graph is target
checkable. It does not close E7-S1's fixed-capacity flash store, E7-S2's radio
backend, E7-S3's Embassy executor wiring, the linked-image flash/RAM budget,
the OQ-7 benchmark, or phone-to-sensor HIL. Those acceptance gates and the
historical failure explanation above remain valid as implementation work still
outstanding; the historical failure block above is not the current result.

## Source-boundary regression (2026-08-09)

The follow-up audit deliberately added no provider API or allocator policy. It
added one host-side CI regression in
`implementation/mesh-milk/scripts/tests/test_ci_workflow.py`:
`test_embedded_shell_sources_keep_the_no_std_boundary`. The test checks both E7
shell entry points for an unconditional `#![no_std]` crate attribute and rejects
direct `std` imports or `std::` references. The existing manifest-fence test and
the target dependency-closure job remain the authoritative checks for the
dependency graph; this source check is an early-review guard against turning an
empty shell into a host-only crate by accident.

Evidence from this audit:

```text
python -m unittest scripts.tests.test_ci_workflow -v
Ran 15 tests ... OK

cargo check -p mmp-node-embassy -p mmp-tal-embedded -p mmp-store \
  --target thumbv7em-none-eabihf --no-default-features
Finished `dev` profile
```

This is a regression improvement, not an embedded implementation. The shells
still contain no Embassy executor, radio backend, flash provider, linked board
image, static-RAM measurement, or phone-to-sensor HIL path. The next E7 change
must first name the allocator/flash policy and producer transaction boundary;
adding a generic `BundleStore` implementation before those decisions would
make capacity and crash-atomicity claims unverifiable.
