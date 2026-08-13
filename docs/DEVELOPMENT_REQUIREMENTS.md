# Development Requirements for the Next MMP/1 Gates

**Status:** environment checklist, 2026-08-14
**Purpose:** record the hardware and software needed to close the existing
roadmap gates. This is an inventory, not a new product requirement.

## Current Windows workstation

The current repository work has been performed on Windows. The local inventory
has Rust/Cargo/rustup, Python, Node/npm, Java, `cargo-fuzz`, and
`cargo-audit`. It does not provide the complete radio/mobile/embedded toolchain:

- no Android SDK/ADB/Gradle/NDK environment;
- no Xcode or `xcodebuild` (macOS-only);
- no Linux `pkg-config`, BlueZ utilities, `dbus-daemon`, or `libdbus-1-dev`;
- no local `clang` sanitizer setup suitable for the planned fuzz campaign;
- no observed `cosign`/remote OIDC environment for keyless release signing.

The Windows host remains suitable for Rust unit/property tests, CLI boundary
tests, Python contract checks, documentation checks, and the PC CLI's graceful
no-adapter behavior. It is not evidence for a real BLE exchange, mobile build,
or whole-phone energy result.

## Required acquisition matrix

| Gate | Hardware | Software/toolchain | Evidence required |
|---|---|---|---|
| PC BLE (E3-S1/E3-S2) | Linux host plus two independent BLE adapters/hosts | Linux, BlueZ, `bluetoothctl`, `dbus-daemon`, `pkg-config`, `libdbus-1-dev`, Rust stable | peripheral build, central/peripheral lifecycle, message + receipt over real BLE |
| Android (E4-S1/E4-S4/E4-S5) | Supported physical Android devices; at least three device classes and the planned five-device field run | Android SDK/platform tools (`adb`), Gradle, NDK r26d used by CI, `cargo-ndk`, Kotlin toolchain, Battery Historian | foreground service/GATT, permissions/Doze, keystore-wrapped ratchet restart, battery/wakeup report |
| iOS (E4-S2/E4-S4/E4-S5) | macOS host and physical iPhone/iOS devices | Xcode, iOS SDK, `xcodebuild`, Swift toolchain, CoreBluetooth test app, Instruments | XCFramework build, restoration/background boundary, Keychain restart, Instruments report |
| React Native (E4-S3) | A consumer fixture device/emulator for each mobile target | Node/npm, pinned TypeScript fixture, React Native TurboModule/codegen, Android/iOS native toolchains | generated declarations, consumer compile, event/error surface exercise |
| Embedded durability (E7) | nRF52840 DK or the board profile in `E7_NRF52840_DK_PROFILE.md`; ESP32 only if its existing profile is selected; debugger/probe and power-loss rig | `thumbv7em-none-eabihf`, nRF SoftDevice/nrf HAL, Embassy, `esp-hal` where applicable, linker/map tools, serial/log capture | linked image geometry, crash/power-loss recovery, flash provider and RS benchmark |
| Fuzz/security (E5-S6/E6-S4) | Linux CI runner with enough CPU-hours and retained artifact storage | `cargo-fuzz`, sanitizer-capable `clang`, locked Rust dependencies, corpus upload | zero unexplained crashes/timeouts, retained fuzz corpus artifact from the completed campaign, regression cases, external review package |
| Remote release (E0/E2/E8) | GitHub repository permissions and a push-capable branch | GitHub Actions runners (Linux/macOS/Windows), artifact retention, OIDC-enabled `cosign`, Rust/Android/iOS matrices | successful workflow run, downloadable bindings/AAR/XCFramework, verifiable signatures |

## Software checks before a gate starts

Run these from `implementation/mesh-milk/` on the matching host; do not claim a
gate from a tool merely being installed:

```text
rustc --version
cargo --version
python --version
cargo test -p <changed-crate> --locked
cargo clippy -p <changed-crate> --all-targets --locked -- -D warnings
```

The normal CI matrix remains the source of truth for workspace/platform checks;
nightly/release runs carry long simulations, fuzz campaigns, packaging, and
hardware gates. Local development should use the fast changed-scope helper in
`implementation/mesh-milk/README.md`.

## Evidence and ownership boundaries

- A simulator, SDK-free shell, generated binding, or YAML parse proves only
  that bounded artifact; it does not prove native radio, background execution,
  persistence, or battery behavior.
- Hardware and external-review dependencies cannot be closed by editing Rust
  or by changing the frozen specification.
- If a listed tool is unavailable, record the exact command and failure in the
  relevant handoff and leave the story `DEFERRED-WITH-REASON`; do not create
  substitute files outside the repository or desktop workspace.
