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

The native adapter must call `onLinkDown` only after CoreBluetooth reports the
physical disconnect. `ProcessInfo.processInfo.systemUptime` or `DispatchTime`
should implement `MonotonicClock`; wall-clock `Date` values are not valid for
protocol ticks. This source module is not evidence of CoreBluetooth behavior,
background state restoration, battery use, or device interoperability.
