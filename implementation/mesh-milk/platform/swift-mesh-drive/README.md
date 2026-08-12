# MMP/1 Swift drive coordinator

This directory contains the SDK-free Swift counterpart of the Kotlin
`MeshDrive` coordinator. It deliberately imports no CoreBluetooth symbols: an
iOS application supplies `CBCentralManager`/`CBPeripheralManager` adapters,
uses a serial dispatch queue, and calls the coordinator's callbacks from that
queue.

The coordinator owns only deterministic host policy:

- monotonic `pump`/`pumpAt` and rotating-beacon refresh;
- pull-based send/event draining;
- ATT MTU minus three fragmentation with adapter backpressure;
- optional `MeshLinkQualityHint` values (MTU `23...4096`, goodput
  `1...10,000,000` bit/s) validated before admission and forwarded to
  quality-aware FFI adapters; the legacy callback uses the Rust default;
- bounded active-link and pending-frame state;
- transport-close ordering and stale callback rejection;
- fail-closed handling for sticky close/event queue saturation;
- startup rollback and physical teardown requests.

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

The native adapter must call `onLinkDown` only after CoreBluetooth reports the
physical disconnect. `ProcessInfo.processInfo.systemUptime` or `DispatchTime`
should implement `MonotonicClock`; wall-clock `Date` values are not valid for
protocol ticks. This source module is not evidence of CoreBluetooth behavior,
background state restoration, battery use, or device interoperability.

`MeshDutyCycleRequest` is an immutable, transport-local request for the native
adapter. `MeshDrive.start()` validates it and invokes the injected
`DutyCycleSink` exactly once before advertiser or scanner startup. Defaults are
the MMP/1 baseline (5 s advertisement, 10% jitter, 30 ms / 1.28 s scan,
30-second burst with one permitted burst per minute, 15–30 ms connection
interval, and 4 s supervision timeout). The shell does not start timers and
the request is never serialized into protocol frames; native scheduling and
hardware validation remain platform gates.
