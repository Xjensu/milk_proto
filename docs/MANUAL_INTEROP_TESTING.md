# Manual interoperability testing

This document describes the checks that can be performed by hand with the
current reference implementation. It is deliberately explicit about what is
and is not runnable today: a passing build or the in-memory demo is not proof
of a real radio exchange.

## Current capability boundary

| Path | Manual status | What is required |
| --- | --- | --- |
| PC-to-PC, in-memory | Available on any Rust host | One checkout; no Bluetooth hardware |
| PC-to-PC, real BLE | Linux/BlueZ peripheral + Linux/Windows/macOS central | One Linux machine running `listen`, plus a second PC with a supported central adapter |
| Windows/macOS PC as peripheral | Not available in the current implementation | `mmp-tal-peripheral` is a compile-time stub on these platforms; use a Linux peripheral |
| Android/iOS-to-PC | Integration-ready but not a turnkey app test | A consuming Android/iOS app must provide the platform BLE callbacks and compile the produced bindings; this repository does not contain a reference mobile app |
| Mobile artifact build | CI/toolchain gated | Android NDK + `cargo-ndk` for AAR; macOS + Xcode for XCFramework |

The protocol specification remains the wire authority. Do not "fix" a failed
interop test by changing `specification/`; record the observed bytes and file a
specification/implementation discrepancy instead.

## 1. Prepare a clean checkout

Use a stable Rust toolchain satisfying the workspace MSRV of 1.96. The
repository's `implementation/mesh-milk/rust-toolchain.toml` selects the stable
channel; it does not pin an exact compiler release.
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
* a line reports `demo assertions: receiver_message=true,
  sender_delivered=true`;
* the process exits successfully without a Bluetooth adapter.

The command now exits with status 1 if either the receiver-side
`MessageReceived` event or the sender-side `Delivered` state is missing. This
makes it a useful deterministic host-stack smoke gate rather than a log-only
demonstration. It still proves only the Rust protocol loopback: it does not
exercise a Bluetooth controller, GATT framing, mobile callbacks, background
execution, or physical radio delivery.

## 2. Generate two PC identities

Build the CLI on each test machine and create exactly one separate seed file per
machine. Keep seed files private; the node ID is safe to copy to the sending
machine. For the commands below, machine A owns `alice.seed` and machine B
owns `bob.seed`:

On Windows PowerShell:

```powershell
cargo build -p mmp-cli --release
.\target\release\mmp-cli.exe keygen alice.seed
```

On Linux or macOS, use the native executable path instead:

```bash
cargo build -p mmp-cli --release
./target/release/mmp-cli keygen alice.seed
```

On machine B, run the corresponding command for that host followed by
`bob.seed`. Do not copy either seed to the other machine.

Record the printed node IDs. They are 32 hexadecimal characters (16 bytes).
Do not reuse a seed between machines or delete a seed after a test unless the
identity is intentionally being revoked.

Before collecting the evidence manifest, record a canonical, non-secret
fingerprint for each seed. Hash the decoded seed rather than the text file so
line endings and hexadecimal case do not affect the value:

```bash
python scripts/fingerprint_seed.py /path/to/alice.seed
python scripts/fingerprint_seed.py /path/to/bob.seed
```

Copy only the printed `sha256:<64 lowercase hex>` values into
`hosts[].seed_fingerprint`; never publish the seed files. This fingerprint is
an evidence label, not an MMP node ID and not a replacement for seed secrecy.

## 3. PC-to-PC over real BLE (Linux peripheral + Linux/Windows/macOS central)

The current peripheral backend is Linux/BlueZ only. The central backend uses
`btleplug` on Linux, Windows, and macOS, so the second machine may be any of
those three platforms; it does not need to run `listen`. Select two machines
on the same test bench, grant the process Bluetooth permissions, and make sure
no other application is holding the adapter exclusively.

Install the host prerequisites before collecting evidence:

* **Linux peripheral:** install BlueZ, `pkg-config`, and the D-Bus development
  headers (for Debian/Ubuntu: `sudo apt-get update && sudo apt-get install
  bluez pkg-config libdbus-1-dev`), then ensure the daemon is running with
  `sudo systemctl enable --now bluetooth`.
* **Linux central:** use the same packages when the central is built locally;
  the adapter must be visible to BlueZ (`bluetoothctl list`).
* **Windows central:** install the MSVC Rust toolchain and Windows Bluetooth
  support, then grant Bluetooth access to the terminal/application if the OS
  prompts. No Windows peripheral path is claimed by this repository.
* **macOS central:** install Xcode Command Line Tools (`xcode-select
  --install`), enable Bluetooth for the terminal/application in System
  Settings, and use a macOS runner with a supported adapter. No macOS
  peripheral path is claimed by this repository.

Build the CLI on each host before the radio run and record the versions and
exact command lines:

```text
cargo build -p mmp-cli --release
rustc --version
cargo --version
```

The central host runs `connect`/`send`; the Linux host runs `listen` as shown
below. A Windows or macOS central connecting to a Linux peripheral is the
supported cross-OS PC path. If the central cannot see the Linux advertisement,
record that as an adapter/OS gate failure rather than substituting an
unsupported peripheral implementation.

On machine A (peripheral):

```bash
cargo run --release -p mmp-cli -- listen /path/to/alice.seed --store /tmp/mmp-alice.store
```

Leave this process running. It should advertise the development MMP service
and report incoming connections/events. Do not treat an advertisement alone as
success; a session must become established and the drive loop must continue.

On machine B (central), first discover A with `connect`.

On Linux or macOS:

```bash
cargo run --release -p mmp-cli -- connect /path/to/bob.seed --store /tmp/mmp-bob.store
```

On Windows PowerShell:

```powershell
cargo run --release -p mmp-cli -- connect .\bob.seed --store .\mmp-bob.store
```

Leave it running until the log shows an established link and the peer's
claims have exchanged. Then press `Ctrl-C` and wait for `store persisted to
disk`. `connect` is a foreground bootstrap process; it is not intended to run
concurrently with the later `send` command. The later `send` process must use
the same `--store` path so that the persisted peer claims are available.

Keep the bootstrap output as a separate log (for example,
`bob-connect.log`). It is not, by itself, the central-side delivery evidence:
the machine-checkable package requires one central log containing all three
reference-CLI markers `dialed`, `Established`, and `Delivered`. The later
`send` invocation starts a new central process, so capture its output as well
and concatenate the two files (in chronological order) into the single
`bob.log` used by `hosts[1].log` in the evidence manifest. Record the `send`
command as `hosts[1].command`; retain the bootstrap `connect` command in the
operator notes alongside the two raw logs.

For a message test, obtain machine A's node ID from its `keygen` output and
run on B:

On Linux or macOS:

```bash
cargo run --release -p mmp-cli -- send /path/to/bob.seed <alice-node-id> "ble smoke test" --store /tmp/mmp-bob.store
```

On Windows PowerShell, use paths valid on that machine:

```powershell
cargo run --release -p mmp-cli -- send .\bob.seed <alice-node-id> "ble smoke test" --store .\mmp-bob.store
```

`send` waits up to the CLI's configured delivery timeout. A successful test
needs all of the following:

1. machine B discovers and connects to A;
2. both sides complete the protocol handshake and identity proof;
3. the bundle is accepted by A and the receipt returns to B;
4. `send` reports delivery rather than timing out;
5. the central process is stopped and restarted with the same persisted store
   before a reconnect attempt; the current CLI makes one dial attempt per
   peripheral per process and does not retry in-process.

Stop both processes with `Ctrl-C`, then inspect each store on the host that
owns it. Use the exact path supplied to that host's `--store` option; the
following examples avoid accidentally asking a Windows host to read the
Linux `/tmp` path:

On Linux or macOS:

```bash
cargo run -p mmp-cli -- inspect /tmp/mmp-alice.store
cargo run -p mmp-cli -- inspect /tmp/mmp-bob.store
```

On Windows PowerShell (when machine B is the Windows central):

```powershell
cargo run -p mmp-cli -- inspect .\mmp-bob.store
```

If the peripheral store is on another host, run `inspect` there rather than
copying or renaming the snapshot before collecting evidence. The evidence
manifest requires distinct, existing inspection files for both roles.

Capture the exact CLI output, OS version, adapter model, and elapsed delivery
time. The reference CLI does not print negotiated MTU or RSSI; obtain those
from adapter/OS tooling when available, otherwise record them as unavailable
with a reason. A single successful run is only a smoke result; stop and restart
the central process with the same store after an out-of-range interval to
exercise store-carry-forward and reconnect behavior.

The peripheral advertises a 128-bit service UUID and 24-byte service data.
Controllers that expose only legacy advertisement capacity may not surface it;
classify that as an adapter/backend limitation, not a protocol failure.

### What to record when it fails

Record which gate failed rather than only "BLE failed":

* no advertisement visible;
* advertisement visible but no GATT connection;
* connection established but no `Established`/identity proof;
* handshake succeeds but no bundle transfer;
* bundle arrives but no receipt;
* delivery succeeds once but reconnect loses state.

Also preserve `mmp-cli inspect` output and the exact command lines. Do not
publish seed files or raw private keys in an issue.

## 4. Mobile-to-PC test (once a consuming app exists)

There is currently no runnable Android or iOS application in this repository.
The mobile surface is a UniFFI library: the app owns scanning, advertising,
GATT subscriptions, permissions, background execution, and clock values, then
passes received link bytes to `MmpNode` and writes bytes returned by the FFI
back to the platform BLE characteristic. The host must also drain the
transport-local lifecycle seam after every injected event or tick: drain
`pollClose()` before `pollSend()`, close the corresponding native BLE/GATT
link, and call `linkDown()` exactly once when the platform reports the physical
disconnect. Read the sticky `closeActionsDropped()` and
`eventQueueDropped()` counters; any increase is a fail-closed host-overload
signal, not a successful interoperability result. The current Kotlin
drive-loop source is deliberately not a complete implementation of this seam;
the consuming Android/iOS host must add it and test the native close callback.

Immediately after constructing or restoring `MmpNode`, on the same serialized
drive queue, call `tick` once with current same-boot monotonic uptime before
calling `sendText`, `sendFile`, or any group-send method. Android
should use `SystemClock.elapsedRealtime()`; iOS should use
`ProcessInfo.processInfo.systemUptime`. Assert that this first tick completes
before exposing send controls or injecting the first link event. A send before
the initial tick is a failed host-ordering run, not a protocol or radio
failure: persisted deadlines and expiry policy are intentionally anchored by
that first monotonic sample.

The repository also contains an Android-SDK-free Kotlin drive-loop reference at
`implementation/mesh-milk/platform/kotlin-mesh-drive/`. Its injected clock,
scanner, advertiser, GATT, and node adapters make callback ordering and
MTU-minus-three fragmentation testable in a JVM-capable consuming project. The
source-level checks in `scripts/tests/test_kotlin_mesh_drive.py` are not a
substitute for Kotlin compilation or a device test. Hosts that need a complete
on-disk snapshot quota can use the additive FFI constructor
`MmpNode.newWithStoreLimit(...)`; the quota includes framing and auxiliary
records, while the legacy `MmpNode.new(...)` behavior is unchanged.

### Android preparation

On a Linux/macOS host with Android SDK/NDK, `cargo-ndk`, `zip`, and the
required Rust targets installed, set `ANDROID_NDK_HOME` to the installed NDK
directory before running the packaging script:

```bash
cd implementation/mesh-milk
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
cargo install cargo-ndk
scripts/build-android-aar.sh mobile-artifacts/android
```

The script fails closed when `ANDROID_NDK_HOME` is unset or when `zip` is not
available; neither prerequisite is installed by the script.

The script produces a native-only `mmp-ffi.aar` plus a Kotlin bindings source
zip. Before packaging, it runs the repository's source-level API/secret gate
against the generated source directory. A pass means
the generated Kotlin `MmpNode` surface matches the Rust FFI and no internal
vault/ratchet identifiers leaked into the source; it does not compile Kotlin or
prove BLE behavior. In the consuming app, compile the Kotlin bindings, add the
AAR, implement the BLE callback bridge, and verify that the app uses the UUIDs
documented in `MOBILE_INTEGRATION.md` and exported by `mmp-tal`.

### iOS preparation

On macOS with Xcode and the Apple Rust targets:

```bash
cd implementation/mesh-milk
rustup target add aarch64-apple-ios aarch64-apple-ios-sim
scripts/build-ios-xcframework.sh mobile-artifacts/ios
```

The script produces `MmpCore.xcframework` and a Swift bindings directory, then
verifies that both the `ios-arm64` device slice and the
`ios-arm64-simulator` slice contain a non-empty static library and headers and
that the Swift source artifact is present. To rerun the structural check after
copying artifacts between hosts:

```bash
python scripts/verify_ios_artifacts.py \
  mobile-artifacts/ios/MmpCore.xcframework \
  mobile-artifacts/ios/swift-bindings
```

The verifier is an artifact-shape gate, not a Swift compiler or CoreBluetooth
test. It does not upgrade an artifact pass into mobile interoperability. The
build script also runs the corresponding source-level gate against the
generated Swift source directory before moving the Swift source into the output.
As with Android, this verifies generated API
coverage and the secret denylist only; it is not a Swift compiler or hardware
result. Add the generated framework and Swift bindings to a test app, implement
the CoreBluetooth central/peripheral callbacks, and exercise the same
handshake/message/receipt gates as the PC test. Test both foreground and
background transitions; background BLE scheduling is platform-controlled and
must not be inferred from a foreground success.

### Mobile observations

At minimum, capture:

* permission and Bluetooth state transitions;
* scan/advertise visibility and connection time;
* callback ordering (`onBeacon`, link established, inbound bytes, outbound
  bytes, `pollClose`/native close callback, disconnect);
* handshake completion and peer node ID;
* message and receipt delivery;
* behavior after app suspension, process restart, and radio loss.

Also record `closeActionsDropped()` and `eventQueueDropped()` at startup and
after each test run. They must not increase. If either counter increases,
stop admitting links, close active native connections, and report the run as a
host-overload failure requiring restart/reconciliation rather than as a wire
or radio failure.

Until a consuming app and physical devices are available, the mobile result
must be reported as **artifact/build verified** or **API-bridge verified**, not
as end-to-end mobile interoperability.

## 5. Evidence checklist

Attach the following to a test report:

* commit ID and Rust/toolchain versions;
* platform, kernel/OS, adapter and phone models;
* exact command lines for each host (record them as `hosts[].command` in the
  evidence manifest; for the central role this is the delivery-producing
  `send` command, while any earlier `connect` bootstrap command belongs in
  the operator notes) and seed-file fingerprints (never the seeds themselves);
* CLI/app logs with timestamps;
* `inspect` output for both stores;
* whether the result was in-memory, PC BLE, Android, or iOS;
* failures classified by the gate list above;
* any observed wire/spec discrepancy, with the relevant specification section.

This checklist is the Phase A hardware exit-gate evidence. It does not replace
the deterministic vectors, fuzzing, or adversarial simulation required by the
later production phases.

### Machine-checking a two-host evidence package

When the Linux/BlueZ run is complete, place the two CLI logs and the two
`inspect` outputs beside a JSON manifest and validate the package from
`implementation/mesh-milk`:

```bash
python scripts/validate_ble_evidence.py /path/to/evidence.json
```

The manifest format is `mmp-ble-evidence-v1`. It requires one peripheral and
one central host with distinct seed fingerprints, a clean commit/toolchain
record, the exact command line used by each host in `hosts[].command`, message
delivery and receipt flags, reconnect evidence, and explicit MTU/RSSI
measurements. If an adapter does not expose MTU or RSSI, the manifest
must record `available: false` and a reason; an omitted measurement is not
silently treated as a pass. Each host's declared log markers and each
store-inspection path are checked, and paths may not escape the manifest
directory. The two host log paths and two store-inspection paths must be
distinct (canonicalized path aliases are rejected), so one file cannot stand
in for both hosts. The reference CLI role markers are mandatory: the peripheral log
must declare and contain `advertising`, `Established`, and `MessageReceived`;
the central log must declare and contain `dialed`, `Established`, and
`Delivered`. These checks make the package structurally complete but cannot
authenticate text logs or prove that a real adapter produced them.

The validator checks evidence completeness and internal consistency only. It
cannot establish that a claimed log came from a real adapter, authenticate a
seed fingerprint, or close the separate RBSR/D3 conformance gate.

For the release preflight, run the repository-side wrapper as well:

```bash
python scripts/validate_release_preflight.py \
  /path/to/evidence.json --repo ../..
```

In addition to the manifest checks, this resolves the recorded Git commit and
requires it to equal the checkout's current `HEAD`, then verifies that
`git status -- specification` is empty, including staged, unstaged, and
untracked specification files. Running the preflight from another revision
fails instead of certifying a nearby implementation. Changes elsewhere in the
worktree are intentionally ignored so the check does not adjudicate unrelated
development work. A preflight pass is an evidence-integrity result, not proof
that the physical radio exchange actually occurred.
