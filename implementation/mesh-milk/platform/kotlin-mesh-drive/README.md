# Kotlin MeshDrive coordinator

This directory contains the Android-SDK-free orchestration core derived from
`implementation/mesh-milk/MOBILE_INTEGRATION.md` §4. It is deliberately a
source module, not an Android application or a replacement for the UniFFI
generated `MmpNode` bindings.

`MeshDrive` owns only deterministic drive-loop policy:

- monotonic `pump()`/`pumpAt()`; backwards timestamps fail before `MeshNode.tick`;
- explicit `refreshBeacon(nowMs)` hook so long-lived hosts replace the rotating
  ephemeral advertisement instead of leaving the startup payload stale;
- transport-local `pollClose()` actions are drained before `pollSend()`;
- pull-only `pollSend()` and `pollEvent()` draining;
- link-up/data/down forwarding with stale-link input ignored;
- notification fragmentation at `negotiated ATT MTU - 3`, with adapter
  back-pressure retained for the next pump;
- close/event saturation diagnostics are exposed so a host can fail closed;
- foreground relay capability demotion to `MeshTier.LEAF` when unavailable;
- atomic startup rollback: a failed clock/radio/initial drain leaves
  `isRunning == false` and stops any advertiser/scanner that started;
- `stop()` releases protocol sessions and requests native GATT teardown for
  every active link before dropping coordinator ownership;
- scanner/advertiser/GATT/clock dependencies injected through interfaces.

The Android foreground service should adapt these interfaces to
`SystemClock.elapsedRealtime()`, `BluetoothLeScanner`, the advertiser, and
`BluetoothGattServer`. `GattAdapter.close(link)` requests native teardown; the
adapter must call `MeshDrive.onLinkDown(link)` after the platform reports the
physical disconnect. The adapters remain responsible for permissions,
background policy, connection establishment, platform callbacks, and treating
`Advertiser.start(serviceData)` as a replace/update operation when
`refreshBeacon(nowMs)` is called. The coordinator suppresses a redundant
restart when the payload has not changed. Refresh the beacon on the platform's
advertising cadence (or at least once per protocol beacon slot) using the same
monotonic timestamp supplied to the drive loop; never substitute a wall-clock
value.

## Evidence boundary

No Android SDK, Kotlin compiler, BLE controller, or physical device is
required by this module. The repository's Python source-level test checks the
contract and rejects accidental Android imports. A consuming Android project
must compile this source and run JVM tests with its own Kotlin toolchain before
claiming an Android integration result. A passing source-level check is not
evidence of GATT, background execution, or radio interoperability. It is not
evidence of GATT behavior on a device.
