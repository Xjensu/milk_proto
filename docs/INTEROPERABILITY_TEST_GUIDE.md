# MMP/1 interoperability test guide

This runbook describes the tests that can be performed against the current
reference implementation. It is intentionally separate from the protected
protocol specification and from the existing manual gate. Commands and
expected observations below are based on the checked-in CLI, BlueZ transport,
UniFFI bindings, and evidence validators.

## 1. What this guide can prove

There are three different levels of evidence. Do not report a lower level as a
higher one.

| Level | What is exercised | Current status |
| --- | --- | --- |
| Deterministic | Two in-memory `MmpNode` instances and the reference state machine | Available on every supported Rust host |
| PC BLE | A real Linux/BlueZ peripheral and a real Linux/Windows/macOS central | Available with a Linux peripheral, a supported central adapter, and two hosts |
| Mobile BLE | A consuming Android or iOS app, its OS BLE callbacks, and the Rust node | Bindings/build scripts are available; a turnkey app and device result are not included |

The deterministic test is not radio interoperability. A successful artifact
build is not a mobile-device result. A manifest accepted by the evidence
validator is structurally complete, but the validator cannot authenticate the
text logs or prove that a radio was present.

## 2. Prerequisites and clean checkout

Use a checked-out revision of this repository. Record the revision before the
run and do not copy private identity seeds between hosts.

From the repository root:

```bash
cd implementation/mesh-milk
cargo build --workspace --all-features
cargo test --workspace --all-targets -- --test-threads=1
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p layer-check
cargo run -p mmp-cli -- demo
```

The demo should print two deterministic node IDs, a receiver
`MessageReceived`, a sender `Delivered`, and a final line equivalent to
`demo assertions: receiver_message=true, sender_delivered=true`. It does not
exercise Bluetooth, BlueZ, Android, or iOS.

For a release-quality run also record:

```bash
rustc --version
cargo --version
git rev-parse HEAD
git status --short
```

The release evidence gate requires the recorded commit to exist and the
`specification/` subtree to be clean. Unrelated local work may remain dirty,
but it must not be confused with the tested revision.

## 3. Linux/BlueZ two-PC test

### Hardware and operating-system requirements

Use two PCs, each with a working BLE adapter. Confirm that the adapter is
powered and visible to the user running the process. One machine must be Linux
with BlueZ and runs the peripheral (`listen`); the other machine is the central
(`connect` and `send`) and may run Linux, Windows, or macOS through the
`mmp-tal-btleplug` backend. The current peripheral implementation is not
available on Windows or macOS, so a Windows/macOS pair cannot satisfy this
real-BLE gate.

Build the release CLI on both machines:

```bash
cd implementation/mesh-milk
cargo build --release -p mmp-cli
```

Create one seed on each host. Keep the files private and use different seeds:

```bash
target/release/mmp-cli keygen /secure/path/alice.seed
target/release/mmp-cli keygen /secure/path/bob.seed
```

Record a canonical, non-secret fingerprint for each seed; never put the seed
bytes in an evidence package. The helper hashes the decoded 32-byte seed, so
line endings and hexadecimal case do not change the value:

```bash
python scripts/fingerprint_seed.py /secure/path/alice.seed
python scripts/fingerprint_seed.py /secure/path/bob.seed
```

Copy only the printed `sha256:<64 lowercase hex>` values into
`hosts[].seed_fingerprint`. This is an evidence label, not an MMP node ID and
does not make the seed safe to disclose.

### Host A: peripheral

Choose a snapshot **file** path whose parent directory already exists and
capture the complete stdout/stderr stream. `--store` is not a directory
argument: `SnapshotStore` atomically replaces the file and its sibling `.tmp`
recovery candidate. Passing a pre-created directory makes persistence fail
closed at shutdown.

```bash
target/release/mmp-cli listen /secure/path/alice.seed \
  --store /tmp/mmp-alice.snapshot 2>&1 | tee /tmp/mmp-alice-listen.log
```

Leave this process running. The log must show at least `advertising`, then
`Established`, and finally `MessageReceived` after the central sends a
message. The beacon is only a discovery hint; the handshake and identity proof
are the gates that make the peer usable.

### Host B: central bootstrap (Linux, Windows, or macOS)

Use a different snapshot file. Capture the bootstrap connection separately:

```bash
target/release/mmp-cli connect /secure/path/bob.seed \
  --store /tmp/mmp-bob.snapshot 2>&1 | tee /tmp/mmp-bob-connect.log
```

Wait for the connection to establish and for claims/state to be exchanged.
Stop it with `Ctrl-C` and wait until the process reports that the store was
persisted. Do not start `send` concurrently against the same store.

### Host B: message delivery

Start the delivery command as a new process, using the node ID printed by the
peripheral or otherwise recorded during the bootstrap:

```bash
target/release/mmp-cli send /secure/path/bob.seed <alice-node-id> \
  "BLE interoperability smoke test" \
  --store /tmp/mmp-bob.snapshot 2>&1 | tee /tmp/mmp-bob-send.log
```

For the release manifest, concatenate the two central logs in chronological
order so that `hosts[1].log` contains `dialed`, `Established`, and `Delivered`:

```bash
cat /tmp/mmp-bob-connect.log /tmp/mmp-bob-send.log > /tmp/mmp-bob.log
```

Record the `send` command as the central host's `command`; retain the
bootstrap `connect` command in the run notes. This reflects the CLI's current
two-process flow and avoids presenting a bootstrap-only log as delivery proof.

### Disconnect/reconnect and store-carry-forward

After the first delivery, stop both processes with `Ctrl-C`, wait for store
persistence, and repeat the connection. At least one reconnect cycle is
required. A stronger test moves the hosts out of range before sending, then
brings them back into range and verifies that the stored bundle is delivered
after reconnection. Keep the resulting logs and both `inspect` outputs:

```bash
target/release/mmp-cli inspect /tmp/mmp-alice.snapshot > /tmp/mmp-alice.inspect
target/release/mmp-cli inspect /tmp/mmp-bob.snapshot   > /tmp/mmp-bob.inspect
```

The minimum physical gates are:

1. advertisement observed;
2. GATT connection established;
3. Noise/session identity proof completed;
4. bundle accepted by the peripheral;
5. receipt returned and the central reports `Delivered`;
6. a later reconnect does not lose the durable state.

If any gate is missing, classify the run as failed or incomplete; do not infer
success from a process that merely stayed alive.

## 4. Evidence package and validator

Keep a small directory containing the manifest, the two distinct host logs,
and the two distinct store-inspection files. Paths in the manifest are
relative to the manifest, may not escape that directory, and must not use
symbolic links (including symlinked parent directories). The schema is
`mmp-ble-evidence-v1`.

The manifest must contain exactly two hosts (one `peripheral`, one `central`),
distinct seed fingerprints, exact commands, OS/kernel/adapter descriptions,
the recorded commit and toolchain, all five gates, message delivery with a
32-character hexadecimal BundleID, a returned receipt, and reconnect evidence
with `cycles >= 1`. Required role-specific log markers are:

* peripheral: `advertising`, `Established`, `MessageReceived`;
* central: `dialed`, `Established`, `Delivered`.

If MTU or RSSI is unavailable, state that explicitly with
`{"available": false, "reason": "..."}` rather than inventing a value.

Validate from `implementation/mesh-milk`:

```bash
python scripts/validate_ble_evidence.py /absolute/path/to/evidence.json
python scripts/validate_release_preflight.py /absolute/path/to/evidence.json \
  --repo ../..
```

The first command checks package shape, bounded file sizes, paths, markers,
and internal consistency. The second additionally resolves the recorded
commit and requires it to equal the checkout's current `HEAD`, then checks
that `git status -- specification` is clean. Running the preflight from a
different revision therefore fails instead of accidentally certifying a
nearby implementation. Neither command is a substitute for observing the
radio or reviewing the raw logs.

## 5. Android procedure

The repository currently supplies a UniFFI library and a reference drive-loop
shape, not a finished Android application. A consuming app must own the BLE
scanner, advertiser, GATT server/client, permissions, background execution,
and its monotonic clock. Use the service UUID and characteristic definitions
from `implementation/mesh-milk/MOBILE_INTEGRATION.md` and `mmp-tal`; do not
copy UUIDs into a second source of truth.

### Build the native artifact

On a machine with the Android NDK:

```bash
cd implementation/mesh-milk
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
cargo install cargo-ndk
# Set ANDROID_NDK_HOME to the installed NDK directory.
scripts/build-android-aar.sh mobile-artifacts/android
```

The build script runs the Kotlin source-level API/secret gate against its
generated source directory before packaging. The script requires
ANDROID_NDK_HOME and zip. The resulting AAR is
native-only; the app still has to package generated Kotlin bindings and
implement the BLE bridge. The SDK-free Kotlin drive-loop tests are source/JVM
tests, not proof of behavior on a phone.

### Drive one phone against the Linux peripheral

Install the consuming app on an Android device with Bluetooth enabled and all
runtime permissions granted. Keep one `MmpNode` on a single native
`HandlerThread` (never the React Native JS thread). Feed scanner service data
to `onBeacon(payload, rssi)`, call `linkUp` when GATT is ready, feed received
characteristic bytes through `linkData`, and call `linkDown` exactly once on a
physical disconnect. Call `tick(SystemClock.elapsedRealtime())` from the
native drive loop, and drain `pollSend()` into ordered notifications or writes.

The host must drain the transport-local lifecycle seam after every event or
tick: drain `pollClose()` before `pollSend()`, close the native BLE/GATT link,
and then call `linkDown` once. Inspect the sticky `closeActionsDropped()` and
`eventQueueDropped()` counters; any increase is a fail-closed result requiring
investigation.

Capture, at minimum, permission and radio state, scan visibility, connect time,
RSSI, callback ordering, peer identity proof, inbound/outbound byte counts,
message and receipt events, disconnect/reconnect, app suspension/resume, and
the counter values. Repeat once in foreground and once with the app
backgrounded/locked if the product tier claims background operation. Android
Doze and vendor power management can change scan cadence; report the device,
OS version, battery mode, and exact foreground-service configuration.

## 6. iOS procedure

The iOS surface has the same Rust core but requires a native CoreBluetooth
consumer. A macOS host with Xcode and Apple Rust targets is required to build
the framework:

```bash
cd implementation/mesh-milk
rustup target add aarch64-apple-ios aarch64-apple-ios-sim
scripts/build-ios-xcframework.sh mobile-artifacts/ios
python scripts/verify_ios_artifacts.py \
  mobile-artifacts/ios/MmpCore.xcframework \
  mobile-artifacts/ios/swift-bindings
```

The verifier checks framework and binding shape only. Add the framework and
Swift bindings to a CoreBluetooth app, then map discovery to `onBeacon`, GATT
updates to `linkData`, disconnect to `linkDown`, and a monotonic uptime source
to `tick`. Drain `pollSend()` in order and honor CoreBluetooth's
`canSendWriteWithoutResponse` flow control.

Run the same message, receipt, disconnect, and reconnect gates as the Linux
test. Test foreground first, then the declared background mode. In background
operation scan explicitly for the MMP service UUID; local advertisement names
are not reliable there, callbacks may be coalesced, and iOS may suspend the
process between BLE events. State restoration and durable store reopening are
therefore part of the test. Never claim indefinite iOS relaying from this
guide.

## 7. Measurements and pass/fail record

For every run record a timestamp, revision, host/device model, OS/kernel,
Bluetooth adapter or CoreBluetooth details, configured MTU (or why it is not
available), RSSI samples (or why unavailable), handshake time, delivery time,
payload size, reconnect count, and store paths. Preserve raw logs before
editing or concatenating them.

Pass only when all applicable gates are observed and the relevant counters do
not increase. A run is **artifact/API verified** when only native packaging,
binding checks, or source-level drive-loop tests were run. It is
**end-to-end interoperable** only when a real device exchanged a bundle and a
receipt with the opposite real endpoint. Report unsupported combinations
(Windows/macOS BlueZ peripheral, missing Android/iOS consumer app, OS
background restrictions, unavailable MTU/RSSI) explicitly rather than treating
them as protocol failures or silently passing them.

This guide does not authorize or require any change to `specification/`.
