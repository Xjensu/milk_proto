# E7 nRF52840 DK (PCA10056) board profile proposal

Status: **PROPOSAL - implementation profile only; not hardware validated**

This document selects a first board profile for the embedded implementation
ticket. It does not amend `specification/`, add a wire field, change a MMP/1
limit, or claim production readiness. Values marked **TBD** are intentionally
unselected until the board adapter, linker map, and power-loss tests provide
evidence.

## 1. Scope and decision boundary

The proposed first physical target is the Nordic nRF52840 DK, board identifier
PCA10056, with the nRF52840 SoC populated. The board is useful as a development
and measurement target because Nordic documents an on-board SEGGER J-Link,
current-measurement pins, and an external QSPI memory device. Those facts make
it a practical bring-up board; they do **not** establish the layout of an MMP
flash journal or the behavior of a production sensor enclosure. [L2] [L3]

The current repository already uses `thumbv7em-none-eabihf` in
`implementation/mesh-milk/rust-toolchain.toml`. That target is a project
toolchain policy for this profile, not a new claim about the MMP wire protocol.
The profile remains a compile and measurement target until a linked image and
hardware exchange are captured.

## 2. Evidence table

| Item | Source fact | Evidence | Confidence | Boundary |
|---|---|---|---|---|
| SoC identity and CPU | nRF52840 has a 64 MHz Arm Cortex-M4 with FPU | [L1] | High (Nordic product specification) | Does not select clock setup or interrupt policy |
| SoC memory | The product specification lists 1 MB flash and 256 kB RAM | [L1] | High (Nordic product specification) | Total silicon capacity is not application-available capacity |
| Board identity | Nordic's hardware guide names the nRF52840 DK as PCA10056 | [L3] | High (Nordic hardware guide) | Board revision and fitted components still need an inventory record |
| Board debug | Nordic documents an on-board SEGGER J-Link debugger/programmer | [L2] | High (Nordic DK page) | Probe-rs support and runner configuration remain to be tested |
| Board measurement | Nordic documents dedicated current-measurement pins | [L2] | High (Nordic DK page) | No MMP energy result is inferred without a trace and workload |
| Board external memory | Nordic documents external memory connected to the SoC QSPI peripheral | [L2] | High (Nordic DK page) | Capacity, part number, and driver configuration are not selected here |
| HAL candidate | Embassy nRF 0.11.0 documents an `nrf52840` chip-selection feature and both blocking and async APIs | [L4] | High (versioned official Embassy documentation) | Feature availability is not proof of MMP radio or flash integration |

No claim in this table is a claim that the existing embedded crates can yet
run a complete MMP session on the board.

## 3. Proposed configuration record

The following record is the smallest useful starting point. It is a proposal
for the implementation workspace; it is not a linker script and must not be
copied into a release image without filling every **TBD** field.

| Parameter | Proposed value | Status and required evidence |
|---|---|---|
| `BOARD_ID` | `nrf52840-dk-pca10056` | Proposed name; verify board revision and SoC population |
| `MCU` | `nrf52840` | Source fact from [L1] and [L3] |
| `CPU` | Cortex-M4F, 64 MHz | Source fact from [L1]; clock-tree configuration is TBD |
| `TARGET` | `thumbv7em-none-eabihf` | Existing repository policy; target check must pass |
| `FLASH_CAPACITY` | `1_048_576` bytes | SoC capacity from [L1]; usable application range is TBD |
| `RAM_CAPACITY` | `262_144` bytes | SoC capacity from [L1]; reserved/runtime regions are TBD |
| `HAL` | `embassy-nrf` 0.11.0, `nrf52840` feature | Pinned in the board compile-probe crate; target closure passed with the locked workspace dependency graph. Hardware APIs remain unselected |
| `HAL_DEFAULT_FEATURES` | `false` unless an explicit feature is justified | Project policy to protect the no-std closure; verify against selected HAL version |
| `RUNTIME` | `no_std`; executor **TBD** | Embassy supports either blocking or async use [L4]; no executor is selected |
| `RADIO_BACKEND` | **TBD** | Must be selected only after the BLE/TAL contract and board test exist |
| `FLASH_BACKEND` | **TBD** | Internal NVMC versus board QSPI must be decided from geometry and wear evidence |
| `SOFTDEVICE_OR_BOOTLOADER_RESERVATION` | **TBD** | No bytes are reserved by assumption; obtain the exact image/linker contract |
| `STORE_REGION_OFFSET` | **TBD** | Must follow reservation, linker, and journal-generation decisions |
| `STORE_REGION_BYTES` | **TBD** | Must be proved against fixed-capacity metadata and snapshot bounds |
| `PROGRAM_UNIT_BYTES` | **TBD** | Obtain from the selected flash provider/HAL contract; do not infer from RAM/flash size |
| `ERASE_PAGE_BYTES` | **TBD** | Obtain from the selected provider and verify on a real device |
| `LINKER_SCRIPT` | **TBD** | Board-specific script and map file are required before size claims |
| `ALLOCATOR` | **TBD**; default design is fixed-capacity/no heap | Must be measured and reviewed against the embedded boundary |
| `RUNNER` | On-board J-Link; probe-rs is a host-tool candidate | J-Link presence is documented by [L2]; probe-rs compatibility is unverified |
| `POWER_MEASUREMENT` | DK current pins; instrument and sampling plan **TBD** | The measurement path is documented by [L2], not the expected result |
| `MAX_LIVE_BUNDLES` and related limits | **TBD** | Derive from fixed-capacity store accounting, not from total flash capacity |

The two capacity values above are deliberately decimal byte counts written as
their exact binary quantities (1 MiB and 256 KiB). They describe the SoC's
documented capacity, not the amount available to MMP after vectors, boot code,
radio firmware, linker sections, journal metadata, and wear policy.

## 4. Why this board and HAL are the first candidates

### Board rationale

The nRF52840 DK is a better first measurement target than an unspecified
"nRF52-class" board because it gives the implementation team a stable board
identifier, a documented debug path, and an explicit current-measurement path.
The nRF52840's larger documented memory than the minimum nRF52-class profile
also leaves room for a bounded host-reference-to-target comparison. This is a
bring-up rationale, not an assertion that the eventual sensor BOM will use the
DK.

### HAL rationale

The repository already contains Embassy-oriented embedded shells. Embassy's
versioned documentation exposes an nRF52840 feature and states that the HAL can
be used with blocking or async APIs and with a chosen executor. That is a good
fit for keeping MMP's sans-IO core independent of runtime policy. The
compile-probe dependency has been evaluated for the repository MSRV and
`no_std` target closure, and is now pinned in the workspace manifest. Flash API
semantics, radio/TAL coverage, and hardware supply-chain evidence remain
outside this profile. [L4]

### Tooling rationale

The DK's on-board SEGGER J-Link is the authoritative hardware fact. A
probe-rs/J-Link workflow is therefore a reasonable host-tool candidate, but the
repository must test the exact probe, driver mode, reset behavior, and target
description before recording it as the canonical runner. The official J-Link
toolchain remains the fallback during bring-up. No tooling choice changes the
MMP wire protocol. [L2]

## 5. Alternatives considered

| Decision | Alternative | Reason for current proposal | What would overturn it |
|---|---|---|---|
| First board | Unspecified nRF52 module or nRF52832 DK | PCA10056 gives a named board, J-Link, and measurement pins | A lower-cost target with a reproducible MMP radio and flash evidence |
| HAL | A different nRF HAL or direct PAC access | Existing Embassy shell and documented nRF52840 feature reduce integration surface | A measured no-std, radio, or flash incompatibility; direct PAC remains a narrow fallback |
| Runtime | Immediate full Embassy executor integration | Keep the current sans-IO boundary and prove target closure first | A reviewed executor design with stack/wake-up evidence |
| Storage | Use external QSPI immediately | Board exposes QSPI, but internal/external geometry and wear policy are not yet selected | A measured provider contract and power-loss test matrix |
| Debug runner | Probe-rs as canonical runner | It may streamline Rust workflows while J-Link is present | Failed reset/flash/recovery behavior or unsupported probe path; use Nordic/SEGGER tooling |

## 6. Implementation gates

The next implementation ticket should be split into independently reviewable
gates:

1. Record board revision, SoC identification, and the exact HAL version in a
   checked configuration record. Keep the `specification/` tree unchanged.
2. Add a board-specific target check for the selected Embassy feature and the
   existing fixed-capacity crates. The check must not silently enable `std`.
   The `mmp-board-nrf52840-dk` compile probe now passes for
   `thumbv7em-none-eabihf` with `--no-default-features --locked`; this proves
   dependency/target closure only, not a linked image or hardware behavior.
3. Produce a minimal linked image with a linker map. Record code, static RAM,
   stack reserve, vector table, and every reserved flash range.
4. Select internal NVMC or external QSPI only after recording program/erase
   geometry, power-loss behavior, and wear assumptions. Fill the journal region
   and capacity fields only then.
5. Flash a diagnostic image through the J-Link path, reset it, and capture a
   deterministic UART/RTT or equivalent health record. Probe-rs may be used for
   comparison, not as an unverified production dependency.
6. Run the host fault matrix and target reset/power-cycle matrix before wiring
   own-origin durable producers. A receive-only profile is safer until replay
   and ratchet checkpoint state shares the same commit boundary.
7. Use the DK current pins to measure idle, discovery, authenticated exchange,
   and flash-write workloads. Report traces and setup, not a single battery
   percentage.

### Required acceptance artifacts

The profile is not complete until the repository contains, or links to, all of
the following implementation-owned artifacts:

- exact board and HAL versions plus a reproducible locked build;
- linker script and map with flash/RAM accounting;
- provider geometry and journal-region decision;
- J-Link/probe-rs flash, reset, and recovery transcript;
- fault-injected journal recovery results on the target;
- authenticated phone-to-DK bundle exchange and restart result; and
- a DK current trace with workload and instrument configuration.

Until then, the profile is a planning input and should not be advertised as a
production port or as evidence for the MMP battery target.

## 7. Adversarial review

### Skeptical Professor

**Potential flaw:** A named DK and complete-looking table can be mistaken for
a working port.

**Alternative:** Require a linked image, reset transcript, and authenticated
exchange before calling the target functional.

**Resolution:** This document labels all unmeasured values as TBD and keeps
the acceptance artifacts explicit.

### Security Auditor

**Potential flaw:** Treating the full 1 MB flash as an application store could
overwrite a bootloader, SoftDevice, vector table, or generation floor.

**Alternative:** Derive reservations from the exact image/linker contract and
fail closed when the region is unknown.

**Resolution:** No reservation or journal offset is selected here; own-origin
durability remains blocked until coordinated checkpoint state is designed.

### Performance Engineer

**Potential flaw:** A 64 MHz CPU and large memory numbers say nothing about
BLE goodput, stack use, erase latency, or Reed-Solomon cost.

**Alternative:** Capture map/stack data, radio transfer traces, and a named
target benchmark under fixed compiler settings.

**Resolution:** These are listed as gates; no throughput or size claim is made.

### Distributed Systems Reviewer

**Potential flaw:** A durable bundle journal without durable SeenIndex,
receipts, or ratchet state can replay data after reset.

**Alternative:** Use a coordinated checkpoint or explicitly run receive-only
until the checkpoint is implemented.

**Resolution:** The profile selects neither producer policy nor a journal
region and preserves the existing conservative contract.

### Battery Specialist

**Potential flaw:** The DK's current pins can produce a misleading number if
the debugger, USB supply, external QSPI, or instrument is left in the path.

**Alternative:** Document power topology, sampling rate, calibration, and
debugger state for each workload.

**Resolution:** Measurement setup is an acceptance artifact, not an inferred
battery result.

### Inventive Scientist

**Potential flaw:** Selecting a spacious board may hide constraints that will
appear on a small production sensor.

**Alternative:** Use the DK for repeatable baseline evidence, then repeat the
same gates on the intended low-power hardware.

**Resolution:** The DK is explicitly a first profile and measurement fixture,
not the final product assumption; MMP/Phi research remains separate from MMP/1.

## 8. Decision and handoff

**Decision:** adopt `nrf52840-dk-pca10056` with `thumbv7em-none-eabihf` and an
`embassy-nrf` `nrf52840` feature as the implementation compile-probe profile.
The locked target build is verified; keep all storage geometry, reservations,
linker details, runtime, and production tooling unselected until evidence
exists.

**State:** `FIXED` as an implementation profile with a verified compile probe;
the hardware path is not implemented or production-ready.

The implementation now contains a const-evaluable `Nrf52840DkConfig` record
covering only the repository-backed board identifier, SoC name, target triple,
total SoC capacities, and the pinned target-only `embassy-nrf` package/version/
chip feature. No board revision is asserted because an authoritative fitted-
hardware inventory is still absent. This record does not select linker or
runtime policy.

**Next owner action:** produce the linked image and exact board inventory before
adding executor, radio, or storage providers. Any further manifest change must
be reviewed for MSRV, `no_std`, license, allocation behavior, and frozen-wire
compatibility.
