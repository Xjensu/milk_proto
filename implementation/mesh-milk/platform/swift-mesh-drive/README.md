# MMP/1 Swift drive coordinator

This directory contains the SDK-free Swift counterpart of the Kotlin
`MeshDrive` coordinator. It deliberately imports no CoreBluetooth symbols: an
iOS application supplies `CBCentralManager`/`CBPeripheralManager` adapters,
uses a serial dispatch queue, and calls the coordinator's callbacks from that
queue.

The coordinator owns only deterministic host policy:

- monotonic `pump`/`pumpAt` and rotating-beacon refresh;
- bounded native batch polling for close actions, outbound frames, and events
  (capped at 64 items, with legacy one-item fallbacks) while preserving FIFO
  order and pull-based ownership; close retries and throwing event sinks retain
  the current item until the host accepts it;
- ATT MTU minus three fragmentation with adapter backpressure;
- optional `MeshLinkQualityHint` values (MTU `23...4096`, goodput
  `1...10,000,000` bit/s) validated before admission and forwarded to
  quality-aware FFI adapters; the legacy callback uses the Rust default;
- bounded active-link and pending-frame state;
- transport-close ordering and stale callback rejection;
- fail-closed handling for sticky close/event queue saturation;
- startup rollback and physical teardown requests.
- transactional link admission: node-callback, MTU, and initial-drain
  failures roll back logical state, pending frames, and the physical GATT
  link before the error is propagated; duplicate callbacks are idempotent;
- service-data admission only for the exact observed MMP service UUID;
  missing or mismatched UUID metadata is dropped before `MeshNode.onBeacon`.

`MeshRadioShell` is the protocol-neutral boundary for a CoreBluetooth adapter.
It adds the iOS lifecycle rules that a raw `MeshDrive` call site must not
reimplement:

- create a fresh `MeshConnectionIdentity` for every central `didConnect` or
  peripheral subscription, then capture its `MeshConnectionBinding` in every
  callback closure;
- allocate a positive, never-reused `UInt64` link token and reject callbacks
  for a retired identity (including a reused native handle or peripheral ID);
- forward central/peripheral callbacks in the order `link up → data/ready →
  physical disconnect`, with data before admission ignored;
- accept only the state-restoration identifiers `mmp.central` and
  `mmp.peripheral`, while keeping restoration bookkeeping separate from wire
  state;
- invoke `pump` only from the host's serialized drive queue, including
  `onReadyToSend` when CoreBluetooth reports write-without-response capacity;
- keep the node at `.leaf` unless the host explicitly reports foreground relay
  capability.

Example adapter shape (CoreBluetooth types stay in the application target):

```swift
let shell = MeshRadioShell(drive: drive)
try shell.start()

// CBCentralManagerDelegate.didConnect / CBPeripheralManager subscription:
let identity = MeshConnectionIdentity.create()
guard let binding = try shell.onLinkUp(identity, role: .central) else {
    // Capacity was refused; cancel the native connection.
    return
}

// If the adapter has measured transport quality, pass it as local scheduling
// metadata. It is never encoded in an MMP/1 frame.
let quality = MeshLinkQualityHint(mtu: 247, goodputBps: 125_000)
guard let measuredBinding = try shell.onLinkUp(
    MeshConnectionIdentity.create(), role: .central, quality: quality
) else {
    return
}

// Capture `binding` in every callback for this physical connection.
try shell.onLinkData(binding, bytes: value)
try shell.onReadyToSend(binding)
try shell.onLinkDown(binding) // after CoreBluetooth reports disconnect
try shell.onLinkDown(measuredBinding) // after CoreBluetooth reports disconnect
```

`MeshRadioShell` and `MeshDrive` are intentionally SDK-free.  The exact
CoreBluetooth delegate ordering, restoration callbacks, background behavior,
and radio interoperability are verified only by the macOS/Xcode CI job (and,
for the final gate, on physical iOS hardware); this Linux/Windows workspace
cannot claim those results.

Native CoreBluetooth adapters SHOULD install a targeted scan filter for
`MmpBleService.serviceUUID` and populate `ScanResult.serviceUuid` from the
UUID actually observed.  The SDK-free shell does not implement or claim
CoreBluetooth filtering; it remains fail-closed when that metadata is absent
or does not match.

The native adapter must call `onLinkDown` only after CoreBluetooth reports the
physical disconnect. `ProcessInfo.processInfo.systemUptime` or `DispatchTime`
should implement `MonotonicClock`; wall-clock `Date` values are not valid for
protocol ticks. This source module is not evidence of CoreBluetooth behavior,
background state restoration, battery use, or device interoperability.

`MeshDutyCycleRequest` is an immutable, transport-local request for the native
adapter. `MeshDrive.start()` validates it and invokes the injected
`DutyCycleSink` exactly once before advertiser or scanner startup. Defaults are
the MMP/1 baseline (5 s advertisement, exactly 10% jitter, 30 ms / 1.28 s
scan at no more than 2.5% duty, 30-second burst with one permitted burst per
minute, 15–30 ms connection interval, 4 s supervision timeout, and the 60 s
per-session airtime cap). The shell does not start timers and the request is
never serialized into protocol frames; values are requests to the native
adapter, not proof of controller compliance. Record observed radio counters
and hardware timing separately when measuring a device. The shell only
validates and forwards `perSessionAirtimeCapMs`; the native adapter/core must
enforce the 60 s cap, and no airtime timer is implemented here.

`PersistenceSink` is a separate, explicit lifecycle seam for the existing
`mmp-ffi` snapshot store. A native iOS host should wrap its node's
`MmpNode.flush()` call and pass that wrapper to `MeshDrive`; the host then calls
`MeshRadioShell.flushPersistence()` (or `MeshDrive.flushPersistence()`) from a
background/termination hook. The method is valid while the radio is running or
stopped, propagates a flush error, and is never invoked automatically by
`start()` or `stop()`. The default sink is an in-memory no-op. Keychain
durability, snapshot I/O, retry policy, and observed success/error reporting
remain native application gates; this SDK-free shell does not receive
ratchet/prekey material.
