# Kotlin MeshDrive coordinator

This directory contains the Android-SDK-free orchestration core derived from
`implementation/mesh-milk/MOBILE_INTEGRATION.md` §4. It is deliberately a
source module, not an Android application or a replacement for the UniFFI
generated `MmpNode` bindings.

`MeshDrive` owns only deterministic drive-loop policy:

- a `DriveLoopOwner` guard that binds callbacks to one serialized owner (the
  default `ThreadConfinedDriveLoopOwner` is suitable for a Handler/Looper
  thread; hosts with another queue identity can inject their own check);
- scan-filter installation for the MMP service UUID, case-insensitive UUID
  validation at the callback boundary (missing/mismatched UUIDs are rejected),
  and defensive copying of service data;
- monotonic `pump()`/`pumpAt()`; backwards timestamps fail before `MeshNode.tick`;
- explicit `refreshBeacon(nowMs)` hook so long-lived hosts replace the rotating
  ephemeral advertisement instead of leaving the startup payload stale;
- transport-local `pollClose()` actions are drained before `pollSend()`;
- pull-only `pollSend()` and `pollEvent()` draining;
- link-up/data/down forwarding with stale-link input ignored;
- notification fragmentation at `negotiated ATT MTU - 3`, with adapter
  back-pressure retained for the next pump;
- close/event saturation diagnostics are observed after every drain; any
  counter increase latches fail-closed mode, stops discovery, closes all
  tracked links, and prevents the same drain from sending queued frames;
- native links awaiting a disconnect callback are capped at 64, and late
  callbacks are idempotently ignored after logical teardown;
- `LinkTokenAllocator` gives every native connection a positive, monotonic
  process-local token and fails closed before exhaustion; adapters must capture
  that token in every callback instead of reusing a platform connection handle
  while old callbacks may still be pending;
- `ConnectionBindingRegistry` binds those tokens to fresh per-connection
  `ConnectionIdentity` objects, makes duplicate opens idempotent, rejects
  stale callback contexts after close, and bounds live bindings before native
  callbacks reach `MeshDrive`;
- foreground relay capability demotion to `MeshTier.LEAF` when unavailable;
- atomic startup rollback: a failed clock/radio/initial drain leaves
  `isRunning == false`, stops any advertiser/scanner that started, and
  releases callbacks that admitted a link during startup;
- `stop()` releases protocol sessions and requests native GATT teardown for
  every active link before dropping coordinator ownership;
- scanner/advertiser/GATT/clock dependencies injected through interfaces.

`GattLinkBridge` is the small native callback adapter. The Android GATT layer
creates one `ConnectionIdentity` per physical connection, passes the returned
binding to every callback, and calls `onDisconnected` only after native
teardown. The bridge translates that identity into the coordinator's
non-reused link token and fences delayed callbacks from a retired connection.
If `onConnected` returns `null`, the native adapter must reject/close that
physical link; no protocol session was admitted.

`Scanner.start(serviceUuid, callback)` is the controller-filter seam. A real
Android adapter should override it with a `ScanFilter` for
`MmpBleService.SERVICE_UUID`; the default implementation exists only for
plain-JVM consumers and delegates to the legacy callback-only method.

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
Once `closeActionsDropped()` or `eventQueueDropped()` increases, the
coordinator stops admitting links and requires a recreated node for recovery;
the adapter should still deliver any eventual disconnect callbacks, which are
safe to repeat.

## Evidence boundary

No Android SDK, Kotlin compiler, BLE controller, or physical device is
required by this module. The repository's Python source-level test checks the
contract and rejects accidental Android imports. A consuming Android project
must compile this source and run JVM tests with its own Kotlin toolchain before
claiming an Android integration result. A passing source-level check is not
evidence of GATT, background execution, or radio interoperability. It is not
evidence of GATT behavior on a device.
