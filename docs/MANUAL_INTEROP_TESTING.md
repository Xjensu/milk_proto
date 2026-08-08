# Manual interoperability testing

This document describes the checks that can be performed by hand with the
current reference implementation. It is deliberately explicit about what is
and is not runnable today: a passing build or the in-memory demo is not proof
of a real radio exchange.

## Current capability boundary

| Path | Manual status | What is required |
| --- | --- | --- |
| PC ↔ PC, in-memory | Available on any Rust host | One checkout; no Bluetooth hardware |
| PC ↔ PC, real BLE | Available on Linux with BlueZ | Two Linux machines, Bluetooth adapters, one `listen` peripheral and one `connect`/`send` central |
| Windows/macOS PC as peripheral | Not available in the current implementation | `mmp-tal-peripheral` is a compile-time stub on these platforms; use a Linux peripheral |
| Android/iOS ↔ PC | Integration-ready but not a turnkey app test | A consuming Android/iOS app must provide the platform BLE callbacks and compile the produced bindings; this repository does not contain a reference mobile app |
| Mobile artifact build | CI/toolchain gated | Android NDK + `cargo-ndk` for AAR; macOS + Xcode for XCFramework |

The protocol specification remains the wire authority. Do not “fix” a failed
interop test by changing `specification/`; record the observed bytes and file a
specification/implementation discrepancy instead.

## 1. Prepare a clean checkout

Use Rust 1.96 (the version pinned by `implementation/mesh-milk/rust-toolchain.toml`).
From the repository root:

```powershell
Set-Location implementation/mesh-milk
cargo build --workspace --all-features
cargo test --workspace --all-targets -- --test-threads=1
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p layer-check
```

The serial workspace test invocation is intentional on Windows: the snapshot
store tests use temporary files and parallel processes can collide. If the
non-serial command is used and `mmp-store` reports `NotFound` during a rename,
rerun the serial command before diagnosing a protocol failure.

Before any radio test, also run the deterministic local smoke test:

```powershell
cargo run -p mmp-cli -- demo
```

Expected observations:

* two deterministic node IDs are printed;
* a message is queued, transferred over the in-memory link, and acknowledged;
* the process exits successfully without a Bluetooth adapter.

## 2. Generate two PC identities

On each test machine, build the CLI and create a separate seed file. Keep seed
files private; the node ID is safe to copy to the sending machine.

```powershell
cargo build -p mmp-cli --release
target\release\mmp-cli.exe keygen alice.seed
target\release\mmp-cli.exe keygen bob.seed
```

Record the printed node IDs. They are 32 hexadecimal characters (16 bytes).
Do not reuse a seed between machines or delete a seed after a test unless the
identity is intentionally being revoked.

## 3. PC ↔ PC over real BLE (Linux peripheral + central)

The current peripheral backend is Linux/BlueZ only. Select two machines on the
same test bench, grant the process Bluetooth permissions, and make sure no
other application is holding the adapter exclusively.

On machine A (peripheral):

```bash
cargo run --release -p mmp-cli -- listen /path/to/alice.seed --store /tmp/mmp-alice.store
```

Leave this process running. It should advertise the development MMP service
and report incoming connections/events. Do not treat an advertisement alone as
success; a session must become established and the drive loop must continue.

On machine B (central), first discover A with `connect`:

```bash
cargo run --release -p mmp-cli -- connect /path/to/bob.seed --store /tmp/mmp-bob.store
```

Leave it running until the log shows an established link and the peer's
claims have exchanged. Then press `Ctrl-C` and wait for `store persisted to
disk`. `connect` is a foreground bootstrap process; it is not intended to run
concurrently with the later `send` command using the same store file.

For a message test, obtain machine A's node ID from its `keygen` output and
run on B:

```bash
cargo run --release -p mmp-cli -- send /path/to/bob.seed <alice-node-id> "ble smoke test" --store /tmp/mmp-bob.store
```

`send` waits up to the CLI's configured delivery timeout. A successful test
needs all of the following:

1. machine B discovers and connects to A;
2. both sides complete the protocol handshake and identity proof;
3. the bundle is accepted by A and the receipt returns to B;
4. `send` reports delivery rather than timing out;
5. each side remains stable after disconnect/reconnect.

Stop both processes with `Ctrl-C`, then inspect the stores if needed:

```bash
cargo run -p mmp-cli -- inspect /tmp/mmp-alice.store
cargo run -p mmp-cli -- inspect /tmp/mmp-bob.store
```

Capture the exact CLI output, OS version, adapter model, negotiated MTU (if
the platform exposes it), RSSI, and elapsed delivery time. A single successful
run is only a smoke result; repeat with the machines moving out of range and
back into range to exercise store-carry-forward and reconnect behavior.

### What to record when it fails

Record which gate failed rather than only “BLE failed”:

* no advertisement visible;
* advertisement visible but no GATT connection;
* connection established but no `Established`/identity proof;
* handshake succeeds but no bundle transfer;
* bundle arrives but no receipt;
* delivery succeeds once but reconnect loses state.

Also preserve `mmp-cli inspect` output and the exact command lines. Do not
publish seed files or raw private keys in an issue.

## 4. Mobile ↔ PC test (once a consuming app exists)

There is currently no runnable Android or iOS application in this repository.
The mobile surface is a UniFFI library: the app owns scanning, advertising,
GATT subscriptions, permissions, background execution, and clock values, then
passes received link bytes to `MmpNode` and writes bytes returned by the FFI
back to the platform BLE characteristic.

### Android preparation

On a Linux/macOS host with Android SDK/NDK, `cargo-ndk`, and the required
Rust targets installed:

```bash
cd implementation/mesh-milk
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
cargo install cargo-ndk
scripts/build-android-aar.sh mobile-artifacts/android
```

The script produces a native-only `mmp-ffi.aar` plus a Kotlin bindings source
zip. In the consuming app, compile the Kotlin bindings, add the AAR, implement
the BLE callback bridge, and verify that the app uses the UUIDs documented in
`MOBILE_INTEGRATION.md` and exported by `mmp-tal`.

### iOS preparation

On macOS with Xcode and the Apple Rust targets:

```bash
cd implementation/mesh-milk
rustup target add aarch64-apple-ios aarch64-apple-ios-sim
scripts/build-ios-xcframework.sh mobile-artifacts/ios
```

Add the generated `MmpCore.xcframework` and Swift bindings to a test app,
implement the CoreBluetooth central/peripheral callbacks, and exercise the
same handshake/message/receipt gates as the PC test. Test both foreground and
background transitions; background BLE scheduling is platform-controlled and
must not be inferred from a foreground success.

### Mobile observations

At minimum, capture:

* permission and Bluetooth state transitions;
* scan/advertise visibility and connection time;
* callback ordering (`onBeacon`, link established, inbound bytes, outbound
  bytes, disconnect);
* handshake completion and peer node ID;
* message and receipt delivery;
* behavior after app suspension, process restart, and radio loss.

Until a consuming app and physical devices are available, the mobile result
must be reported as **artifact/build verified** or **API-bridge verified**, not
as end-to-end mobile interoperability.

## 5. Evidence checklist

Attach the following to a test report:

* commit ID and Rust/toolchain versions;
* platform, kernel/OS, adapter and phone models;
* exact commands and seed-file fingerprints (never the seeds themselves);
* CLI/app logs with timestamps;
* `inspect` output for both stores;
* whether the result was in-memory, PC BLE, Android, or iOS;
* failures classified by the gate list above;
* any observed wire/spec discrepancy, with the relevant specification section.

This checklist is the Phase A hardware exit-gate evidence. It does not replace
the deterministic vectors, fuzzing, or adversarial simulation required by the
later production phases.
