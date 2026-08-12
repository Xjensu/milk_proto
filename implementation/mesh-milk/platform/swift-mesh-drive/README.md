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

// Capture `binding` in every callback for this physical connection.
try shell.onLinkData(binding, bytes: value)
try shell.onReadyToSend(binding)
try shell.onLinkDown(binding) // after CoreBluetooth reports disconnect
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
