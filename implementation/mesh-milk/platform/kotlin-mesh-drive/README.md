# Kotlin MeshDrive coordinator

This directory contains the Android-SDK-free orchestration core derived from
`implementation/mesh-milk/MOBILE_INTEGRATION.md` §4. It is deliberately a
source module, not an Android application or a replacement for the UniFFI
generated `MmpNode` bindings.

`MeshDrive` owns only deterministic drive-loop policy:

- monotonic `pump()`/`pumpAt()`; backwards timestamps fail before `MeshNode.tick`;
- transport-local `pollClose()` actions are drained before `pollSend()`;
- pull-only `pollSend()` and `pollEvent()` draining;
- link-up/data/down forwarding with stale-link input ignored;
- notification fragmentation at `negotiated ATT MTU - 3`, with adapter
  back-pressure retained for the next pump;
- close/event saturation diagnostics are exposed so a host can fail closed;
- foreground relay capability demotion to `MeshTier.LEAF` when unavailable;
- scanner/advertiser/GATT/clock dependencies injected through interfaces.

The Android foreground service should adapt these interfaces to
`SystemClock.elapsedRealtime()`, `BluetoothLeScanner`, the advertiser, and
`BluetoothGattServer`. `GattAdapter.close(link)` requests native teardown; the
adapter must call `MeshDrive.onLinkDown(link)` after the platform reports the
physical disconnect. The adapters remain responsible for permissions,
background policy, connection establishment, and platform callbacks.

## Evidence boundary

No Android SDK, Kotlin compiler, BLE controller, or physical device is
required by this module. The repository's Python source-level test checks the
contract and rejects accidental Android imports. A consuming Android project
must compile this source and run JVM tests with its own Kotlin toolchain before
claiming an Android integration result. A passing source-level check is not
evidence of GATT, background execution, or radio interoperability. It is not
evidence of GATT behavior on a device.
