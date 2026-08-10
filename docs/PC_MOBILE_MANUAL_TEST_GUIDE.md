# MMP/1 PC and mobile manual test guide

This guide is the operator-facing procedure for checking the current reference
implementation on a PC, Android device, or iOS device. It is deliberately
separate from the protocol specification. A failing test is recorded as an
implementation, host, or hardware discrepancy; it is not fixed by silently
changing `specification/`.

## What is runnable today

| Test level | Endpoint(s) | What it proves | Status |
| --- | --- | --- | --- |
| Host loopback | Two Rust `MmpNode` instances in `mmp-cli demo` | Core state machine, bundle, receipt, and delivery path without a radio | Runnable on any supported Rust host |
| PC BLE | Linux/BlueZ peripheral plus Linux, Windows, or macOS central | Real advertisement, GATT, session, bundle, receipt, and reconnect | Runnable with two PCs and supported adapters |
| Android BLE | A consuming Android app plus a PC/Linux peer | Native Android BLE callbacks driving the Rust FFI and real device delivery | Requires an app; this repository ships no turnkey app |
| iOS BLE | A consuming CoreBluetooth app plus a PC/Linux peer | Native iOS BLE callbacks, state restoration, and real device delivery | Requires macOS/Xcode and an app; this repository ships no turnkey app |
| Mobile artifact | Android AAR/Kotlin sources or iOS XCFramework/Swift sources | Cross-compilation, generated binding shape, and secret/API boundary | Toolchain-gated; not radio evidence |

Do not report an artifact or source test as a successful phone-to-phone or
phone-to-PC exchange. A real interoperability result requires a bundle and a
receipt observed at the opposite physical endpoint.

## 1. Prepare the revision and evidence directory

Run from the repository root and record the revision before starting:

```text
git rev-parse HEAD
git status --short
```

For a release evidence package, the tested revision must be the committed
revision and `git status -- specification` must be empty. Keep all output in a
new evidence directory outside the repository if possible. Never copy identity
seeds, private keys, or raw keychain exports into that directory.

From `implementation/mesh-milk`, the fast deterministic preflight is:

```text
cargo build --workspace --all-features --locked
cargo run --locked -p layer-check
cargo run --locked -p mmp-cli -- demo
```

`mmp-cli demo` must exit successfully and print evidence equivalent to:

```text
demo assertions: receiver_message=true, sender_delivered=true
```

This is a host-stack smoke test only. It does not exercise a Bluetooth
controller, GATT, mobile callbacks, background execution, or a physical link.

For a broader local run, use the repository's test-tier helper rather than
running unrelated work in parallel:

```text
python scripts/fast_check.py --package mmp-node
python scripts/fast_check.py --python
```

The normal CI and release tiers are described in
[`TESTING_LIFECYCLE.md`](TESTING_LIFECYCLE.md); the manual procedure below is
the hardware gate that CI cannot replace.

## 2. PC-to-PC BLE smoke test

### 2.1 Required roles and prerequisites

The current reference implementation has these roles:

- `mmp-cli listen` uses the Linux/BlueZ peripheral backend.
- `mmp-cli connect` and `mmp-cli send` use the `btleplug` central backend on
  Linux, Windows, and macOS.
- Windows and macOS are not implemented as peripheral hosts. A Windows-to-
  Windows or macOS-to-macOS pair therefore does not satisfy the real-BLE gate.

Both PCs need a working BLE adapter. The Linux peripheral needs BlueZ, D-Bus
development headers, and `pkg-config`; on Debian/Ubuntu a representative
setup is:

```bash
sudo apt-get update
sudo apt-get install bluez pkg-config libdbus-1-dev
sudo systemctl enable --now bluetooth
bluetoothctl list
```

On Windows, use a Rust MSVC toolchain and an enabled Windows Bluetooth adapter.
On macOS, use a supported adapter and grant Bluetooth access to the terminal or
application when the OS asks. Record the exact OS version, adapter model, and
toolchain; these are part of the result, not incidental notes.

Build the release CLI on each PC:

```text
cargo build --release -p mmp-cli --locked
rustc --version
cargo --version
```

### 2.2 Create test identities safely

Create one seed per PC. Do not reuse or copy a seed between hosts:

```bash
target/release/mmp-cli keygen /secure/path/alice.seed
target/release/mmp-cli keygen /secure/path/bob.seed
```

In PowerShell, use `target\\release\\mmp-cli.exe` and a private path instead.
Record only the node IDs printed by `keygen`. If a non-secret evidence label is
needed, use the repository helper to fingerprint the decoded seed and record
only its `sha256:<64 lowercase hex>` result:

```text
python scripts/fingerprint_seed.py /secure/path/alice.seed
python scripts/fingerprint_seed.py /secure/path/bob.seed
```

### 2.3 Start the Linux peripheral

Use a snapshot **file**, not a pre-created directory. The store replaces this
file atomically and may create a sibling recovery candidate:

```bash
target/release/mmp-cli listen /secure/path/alice.seed \
  --store /tmp/mmp-alice.snapshot 2>&1 | tee /tmp/mmp-alice-listen.log
```

Keep the process running. The first useful observation is an advertisement;
the acceptance gate is a later `Established` event followed by
`MessageReceived`. An advertisement by itself is not a protocol success.

### 2.4 Bootstrap and send from the central PC

Run `connect` as a separate bootstrap process, wait for claims/session state to
exchange, then stop it cleanly so the snapshot is persisted:

```bash
target/release/mmp-cli connect /secure/path/bob.seed \
  --store /tmp/mmp-bob.snapshot 2>&1 | tee /tmp/mmp-bob-connect.log
```

On Windows PowerShell, the equivalent form is:

```powershell
target\release\mmp-cli.exe connect .\bob.seed `
  --store .\mmp-bob.snapshot 2>&1 | Tee-Object .\mmp-bob-connect.log
```

After `Ctrl-C`, wait for `store persisted to disk`. Do not run `send` at the
same time as `connect` against the same snapshot.

Start a new process for delivery, using the same store path and the peripheral
node ID:

```bash
target/release/mmp-cli send /secure/path/bob.seed <alice-node-id> \
  "BLE interoperability smoke test" \
  --store /tmp/mmp-bob.snapshot 2>&1 | tee /tmp/mmp-bob-send.log
```

The central evidence log must include the bootstrap and delivery markers in
chronological order. Preserve both raw logs and, when required by the evidence
validator, make a concatenated copy:

```bash
cat /tmp/mmp-bob-connect.log /tmp/mmp-bob-send.log > /tmp/mmp-bob.log
```

### 2.5 PC pass/fail gates

Pass only if all applicable gates are observed:

1. Central sees the MMP advertisement.
2. GATT connection is established.
3. Noise/session identity proof completes (`Established` on both sides).
4. The peripheral accepts the bundle (`MessageReceived`).
5. The central receives a receipt and reports `Delivered`.
6. After both processes stop and restart with the same snapshot files, a
   reconnect does not lose the durable state.

Classify a missing gate precisely: no advertisement, no GATT connection, no
identity proof, no bundle, no receipt, or lost state after restart. Do not
classify a process that merely stayed alive as delivery.

After the run, preserve store observations:

```text
target/release/mmp-cli inspect /tmp/mmp-alice.snapshot
target/release/mmp-cli inspect /tmp/mmp-bob.snapshot
```

Repeat once with the machines temporarily out of range before sending, then
bring them back into range. This distinguishes direct connected delivery from
store-carry-forward. The CLI makes one dial attempt per process; restart it
with the same store for another attempt.

## 3. Android device test

### 3.1 Build and verify the native artifact

The repository provides a native-only AAR and generated Kotlin source, not an
Android application. On a host with the Android NDK, `cargo-ndk`, `zip`, and
the listed Rust targets installed:

```bash
cd implementation/mesh-milk
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
cargo install cargo-ndk
export ANDROID_NDK_HOME=/path/to/android-ndk
scripts/build-android-aar.sh mobile-artifacts/android
```

The script runs the generated-binding API/secret checks and
`verify_android_artifacts.py`. A successful script proves packaging and
binding shape only; it does not prove that a phone can scan, advertise, or
exchange bytes.

### 3.2 Drive-loop setup in the consuming app

The consuming Android app owns permissions, scanning, advertising, GATT, the
background service, and the monotonic clock. Keep one FFI node on one native
serialized queue (for example, a dedicated `HandlerThread`), not on the
React Native or UI thread. Use a same-boot monotonic value such as
`SystemClock.elapsedRealtime()` for `tick`; call it once immediately after
constructing/restoring the node and before `sendText`, `sendFile`, or group
send.

The minimum callback mapping is:

| Android event | FFI operation | Observation |
| --- | --- | --- |
| MMP service advertisement | `on_beacon(payload, rssi)` | Discovery hint only; no peer identity is trusted here |
| GATT ready | `link_up(link, initiator)` | Outbound handshake frames become available |
| RX characteristic write/notification | `link_data(link, bytes)` | Feed bytes in arrival order, without JS serialization |
| Physical disconnect | drain `poll_close`, close native link, then `link_down(link)` once | No duplicate close/link-down calls |
| Native timer | `tick(now_ms)` | Session timers, expiry, and persistence advance only here |
| Outbound queue | drain `poll_close` before `poll_send` | Preserve frame order and close actions |

For each callback/tick, drain `poll_event` and retain the event sequence. A
non-zero `close_actions_dropped()` or `event_queue_dropped()` is a fail-closed
operational result, even if a text message eventually appears to arrive.

The RX/TX bridge must preserve ordering and fragment notifications/writes to
the platform's ATT payload limit. Do not buffer unbounded input and do not put
per-chunk BLE bytes through the JavaScript bridge.

### 3.3 Android foreground and background runs

First run with the app foregrounded and the screen unlocked. Exercise the same
PC gates as above: advertisement, GATT, `Established`, message, receipt,
disconnect, and reconnect.

If the product claims background operation, repeat with the screen locked and
the app backgrounded. Record Android version, device model, battery saver/
Doze state, foreground-service configuration, scan visibility, callback timing,
RSSI, byte counts, and all dropped-event counters. A vendor power manager that
stops the service is an environmental limitation, not evidence that the wire
protocol succeeded.

## 4. iOS device test

### 4.1 Build and verify the framework

Run on macOS with Xcode, `xcrun`, and the Apple Rust targets:

```bash
cd implementation/mesh-milk
rustup target add aarch64-apple-ios aarch64-apple-ios-sim
scripts/build-ios-xcframework.sh mobile-artifacts/ios
python scripts/verify_ios_artifacts.py \
  mobile-artifacts/ios/MmpCore.xcframework \
  mobile-artifacts/ios/swift-bindings
```

This checks framework slices, headers, generated Swift source, and the binding
surface. It is not a device test.

### 4.2 CoreBluetooth bridge and runs

The consuming app owns `CBCentralManager`, `CBPeripheralManager`, GATT
characteristics, permissions, background modes, and state restoration. Map
discovery to `on_beacon`, connection to `link_up`, characteristic updates to
`link_data`, and disconnect to one `link_down`. Drive `tick` from a monotonic
source such as `ProcessInfo.processInfo.systemUptime`; do not use wall-clock
time for session timers or TTL advancement.

Drain `poll_send` in order and respect
`canSendWriteWithoutResponse`; drain `poll_close` before closing the physical
link. Record `poll_event`, `close_actions_dropped`, and
`event_queue_dropped` just as on Android.

Run foreground/unlocked first. If the product claims background relaying, test
with the app suspended and state restoration enabled. Explicitly scan for the
MMP service UUID; do not rely on a local advertisement name. iOS may coalesce
callbacks or suspend the process, so a reconnect that restores the durable
snapshot is a required observation. Do not claim indefinite background relay
from an artifact or simulator-only run.

## 5. Measurements and evidence package

For every physical run record:

- revision, clean/dirty specification status, Rust/tool versions;
- OS version, device/PC model, adapter/chipset, and power mode;
- exact commands and app build identifier;
- timestamp, payload size, BundleID, handshake time, delivery time, and
  reconnect count;
- advertisement visibility, RSSI samples, negotiated MTU or an explicit
  unavailable reason, and inbound/outbound byte counts;
- ordered raw logs, `inspect` output, FFI event sequence, and dropped counters.

For PC BLE evidence, the checked-in validator can check the package shape:

```text
python scripts/validate_ble_evidence.py /absolute/path/to/evidence.json
python scripts/validate_release_preflight.py /absolute/path/to/evidence.json --repo ../..
```

The validator checks paths, markers, consistency, and revision metadata. It
cannot authenticate a text log or observe a radio; retain the raw operator
notes and hardware details alongside the manifest.

## 6. Result labels and known limitations

Use one of these labels in a report:

- **Host loopback passed** — deterministic Rust exchange only.
- **Mobile artifact/API verified** — packaging, generated bindings, and source
  checks passed; no physical mobile radio exchange was observed.
- **PC BLE interoperable** — all PC gates and at least one reconnect passed.
- **Android/iOS end-to-end interoperable** — a real device exchanged a bundle
  and receipt with the opposite endpoint, with no dropped lifecycle/event
  counters.
- **Incomplete / environment blocked** — identify the exact missing gate and
  the hardware, OS, permission, or toolchain limitation.

Current limitations are intentional: there is no turnkey Android/iOS app,
Windows/macOS peripheral backend is unsupported, portable MTU reporting is
not exposed by the central backend, and background execution depends on each
mobile OS. These limitations must remain visible in release reports.
