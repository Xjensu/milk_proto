# MMP/1 manual beta-testing runbook

Status: operator guide, 2026-08-12. This runbook describes how to install the
currently available reference artifacts, execute a PC or mobile beta run, and
collect evidence that can be reviewed by another engineer. It is an
implementation document; `docs/spec/` remains the wire authority.

## What can be tested now

| Level | Available artifact | Evidence it can provide | Not proven |
| --- | --- | --- | --- |
| Host smoke | `mmp-cli demo` | deterministic bundle, receipt, and delivery loop | Bluetooth, mobile callbacks, background execution |
| PC BLE | Linux/BlueZ peripheral + Linux/Windows/macOS central | real advertisement, GATT session, bundle, receipt, reconnect | mobile behavior and unsupported peripheral roles |
| Android | SDK-free Kotlin shell, generated bindings, native-only AAR | source/API and packaging checks | a runnable Android app or physical radio exchange |
| iOS | SDK-free Swift shell, generated bindings, XCFramework | source/API and packaging checks | a runnable iOS app or CoreBluetooth exchange |
| React Native | `@mesh-milk/react-native-mesh` source package | TypeScript bridge contract and local beta report | native module registration, BLE, and device behavior |

There is no turnkey Android or iOS application in this repository. A mobile
artifact or source-contract pass must be reported as **artifact/API verified**,
not as mobile interoperability. A physical result requires a bundle and a
receipt observed at the opposite endpoint.

Detailed protocol/adapter contracts are in
[`MANUAL_INTEROP_TESTING.md`](MANUAL_INTEROP_TESTING.md),
[`PC_MOBILE_MANUAL_TEST_GUIDE.md`](PC_MOBILE_MANUAL_TEST_GUIDE.md),
[`implementation/mesh-milk/MOBILE_INTEGRATION.md`](../../../implementation/mesh-milk/MOBILE_INTEGRATION.md),
and [`implementation/mesh-milk/REACT_NATIVE_INTEGRATION.md`](../../../implementation/mesh-milk/REACT_NATIVE_INTEGRATION.md).

## 1. Prepare a test revision

Use a committed revision for release evidence and record the revision before
running anything:

```text
git rev-parse HEAD
git status --short
git status -- specification
```

Run from `implementation/mesh-milk` with the Rust toolchain selected by
`rust-toolchain.toml`:

```text
cargo build --workspace --all-features --locked
cargo run --locked -p layer-check
cargo run --locked -p mmp-cli -- demo
```

The demo must exit successfully and report
`demo assertions: receiver_message=true, sender_delivered=true`. This is only
a host-stack smoke test. For a changed-scope development check, use
`python scripts/fast_check.py --package <changed-package>` and
`python scripts/fast_check.py --python`; the full workspace matrix belongs in
CI, not every local run.

Create a new evidence directory outside the repository where practical. Never
put identity seeds, private keys, Keychain/Keystore exports, raw snapshots, or
session bytes in it.

## 2. PC installation and BLE run

### Prerequisites

Build the release CLI on both PCs:

```text
cargo build --release -p mmp-cli --locked
rustc --version
cargo --version
```

The peripheral role is Linux/BlueZ only. Install representative Debian/Ubuntu
dependencies and verify the adapter:

```bash
sudo apt-get update
sudo apt-get install bluez pkg-config libdbus-1-dev
sudo systemctl enable --now bluetooth
bluetoothctl list
```

The central role uses `btleplug` on Linux, Windows, or macOS. Grant Bluetooth
permission to the terminal/application and record OS, adapter model, and
power-saving mode. Windows and macOS are not current peripheral hosts.

### Identities and peripheral

Create one private seed on each host. Do not copy or reuse a seed:

```bash
target/release/mmp-cli keygen /secure/path/alice.seed
target/release/mmp-cli keygen /secure/path/bob.seed
python scripts/fingerprint_seed.py /secure/path/alice.seed
python scripts/fingerprint_seed.py /secure/path/bob.seed
```

Record node IDs and only the printed SHA-256 fingerprints. On the Linux
peripheral, use a snapshot *file* and keep the log:

```bash
target/release/mmp-cli listen /secure/path/alice.seed \
  --store /tmp/mmp-alice.snapshot 2>&1 | tee /tmp/mmp-alice-listen.log
```

### Central bootstrap and delivery

On the central host, bootstrap claims/session state, stop cleanly, then start a
new process for delivery using the same snapshot:

```bash
target/release/mmp-cli connect /secure/path/bob.seed \
  --store /tmp/mmp-bob.snapshot 2>&1 | tee /tmp/mmp-bob-connect.log

target/release/mmp-cli send /secure/path/bob.seed <alice-node-id> \
  "BLE interoperability smoke test" \
  --store /tmp/mmp-bob.snapshot 2>&1 | tee /tmp/mmp-bob-send.log
```

On Windows PowerShell, use `target\release\mmp-cli.exe`, private Windows
paths, and `Tee-Object`; do not run `send` concurrently with `connect`.
Concatenate the central logs if an evidence manifest requires one file:

```bash
cat /tmp/mmp-bob-connect.log /tmp/mmp-bob-send.log > /tmp/mmp-bob.log
```

Stop both processes with `Ctrl-C`, wait for persistence, and inspect both
stores:

```bash
target/release/mmp-cli inspect /tmp/mmp-alice.snapshot
target/release/mmp-cli inspect /tmp/mmp-bob.snapshot
```

Pass only when the central sees the advertisement, GATT and identity proof
complete (`Established`), the peripheral reports `MessageReceived`, the
central reports `Delivered`, and a stop/restart/reconnect with the same stores
does not lose durable state. Repeat once out of range to exercise
store-carry-forward. The CLI makes one dial attempt per process; restart it to
retry.

The development BLE identifiers are:

```text
service  6d6d7031-0000-4d4d-5031-6d6573686d6c
TX       6d6d7031-0001-4d4d-5031-6d6573686d6c  (client writes, no response)
RX       6d6d7031-0002-4d4d-5031-6d6573686d6c  (server notifies)
```

These are implementation identifiers, not a SIG-assigned profile. A visible
advertisement alone is not success. Classify failures as no advertisement, no
GATT, no identity proof, no bundle, no receipt, or lost state after restart.

## 3. Mobile artifact and beta-app setup

### Android

On Linux or macOS with Android NDK, `cargo-ndk`, `zip`, and Rust targets:

```bash
cd implementation/mesh-milk
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
cargo install cargo-ndk
export ANDROID_NDK_HOME=/path/to/android-ndk
scripts/build-android-aar.sh mobile-artifacts/android
```

This produces a native-only AAR and Kotlin source artifact and runs the
repository API/secret checks. It does not contain a complete application or
prove Bluetooth behavior. The consuming app must compile/register the generated
Kotlin bindings, own the `MmpNode`, and provide a serialized native drive queue.
It must request `BLUETOOTH_SCAN`, `BLUETOOTH_ADVERTISE`, and
`BLUETOOTH_CONNECT` where applicable, plus the foreground-service permissions
required by its target SDK. Use a filtered scan for the service UUID above;
write TX in negotiated `ATT_MTU - 3` chunks and notify RX in order.

Use `SystemClock.elapsedRealtime()` for `tick`, call it once immediately after
constructing/restoring the node and before any send, and drain `poll_close`
before `poll_send`. A foreground service is required for a relay claim; without
one, report the node as leaf. The SDK-free Kotlin source check is not an
Android SDK, GATT, or device test.

### iOS

On macOS with Xcode, `xcrun`, and Apple Rust targets:

```bash
cd implementation/mesh-milk
rustup target add aarch64-apple-ios aarch64-apple-ios-sim
scripts/build-ios-xcframework.sh mobile-artifacts/ios
python scripts/verify_ios_artifacts.py \
  mobile-artifacts/ios/MmpCore.xcframework \
  mobile-artifacts/ios/swift-bindings
```

The output is a framework plus Swift bindings, not an iOS app. The consuming
app owns CoreBluetooth, `Info.plist` Bluetooth usage/background declarations,
state restoration, GATT characteristics, and a serialized drive queue. Use
`ProcessInfo.processInfo.systemUptime` (or another monotonic source) for
`tick`; use the service UUID explicitly when scanning; respect
`canSendWriteWithoutResponse`; and call `linkDown` only after physical
disconnect. Foreground success does not prove background restoration or
indefinite relay; iOS is currently an honest leaf/foreground-burst role.

### React Native

The package is an application seam, not a BLE implementation. In a consuming
app with native module registration:

```bash
npm install @mesh-milk/react-native-mesh
```

For the checked-out package, run `npm install` and
`npm run typecheck` from `implementation/mesh-milk/platform/react-native-mesh`.
Use `createMmpClient()` only after the native module is registered as
`MmpMesh`; the package deliberately does not expose constructors, `tick`, raw
BLE bytes, or transport-close queues. Use `BetaTestSession` for a bounded,
local, secret-free event history and deterministic report. Its report is not
telemetry and has no uploader.

### Mobile pass criteria

For a real device run, reproduce the PC gates and also record permission and
radio transitions, callback order, foreground/background state, process
restart, reconnect, monotonic tick ordering, negotiated MTU, RSSI, inbound and
outbound bytes, and delivery latency. Record `closeActionsDropped()` and
`eventQueueDropped()` at start and end; any increase is a fail-closed
host-overload result, even if a message appears later.

## 4. Evidence, triage, and cleanup

Record for every run:

- commit, toolchain, OS, device/adapter, and app build identifier;
- exact commands or app build/install steps;
- seed fingerprints only (never seeds or private material);
- timestamp, payload size, BundleID, handshake and delivery latency;
- advertisement/RSSI/MTU observations and byte counters, or an explicit
  unavailable reason;
- ordered logs, store inspection output, event sequence, and dropped counters.

For PC BLE evidence, validate the manifest from `implementation/mesh-milk`:

```bash
python scripts/validate_ble_evidence.py /absolute/path/to/evidence.json
python scripts/validate_release_preflight.py \
  /absolute/path/to/evidence.json --repo ../..
```

These validators check manifest structure, markers, paths, revision, and
specification cleanliness; they cannot authenticate logs or prove a radio
exchange. Use result labels **host loopback passed**, **PC BLE interoperable**,
**mobile artifact/API verified**, **Android/iOS end-to-end interoperable**, or
**incomplete / environment blocked**, with the exact missing gate.

On stop, cancel/close native links, drain pending close actions, call the
platform persistence flush hook, wait for the persistence confirmation, and
then terminate the process. Remove only temporary stores/logs after copying a
sanitized report; securely delete test seeds when their identities are no
longer needed. Never upload seeds, private keys, raw ratchet snapshots, raw
BLE/session bytes, or unredacted crash dumps.

## 5. Current blockers

The following are intentionally not claimed by this runbook: native Android
Keystore/iOS Keychain ratchet persistence, a complete Android/iOS application,
CoreBluetooth/Android SDK compilation on this Windows checkout, physical
mobile hardware interoperability, Windows/macOS peripheral support, and
background battery performance. These remain hardware/toolchain or native-app
gates in the roadmap. A failed gate is recorded as evidence for its owner; it
is not fixed by changing the frozen protocol specification.
