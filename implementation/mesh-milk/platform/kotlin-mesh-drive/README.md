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
- transport-local close actions are drained before outbound frames;
- bounded native batch polling (`pollCloseBatch`, `pollSendBatch`, and
  `pollEventBatch`, capped at 64) reduces bridge calls while retaining FIFO
  order; legacy one-item methods remain the source-compatible fallback;
- link-up/data/down forwarding with stale-link input ignored;
- notification fragmentation at `negotiated ATT MTU - 3`, with adapter
  back-pressure retained for the next pump;
- optional `LinkQualityHint` values (MTU `23..=4096`, goodput
  `1..=10,000,000` bit/s) are validated before link admission and forwarded to
  quality-aware FFI adapters; the legacy callback uses the Rust default;
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
- transactional link admission: a node-callback, MTU, or initial-drain
  failure removes the logical link, drops pending frames, and requests
  physical GATT closure before the error is propagated; duplicate callbacks
  cannot tear down an already-admitted link;
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

`DutyCycleRequest` is an immutable, transport-local request for the native
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
`mmp-ffi` snapshot store. A native Android host should wrap its node's
`MmpNode.flush()` call and pass that wrapper to `MeshDrive`; the host then calls
`flushPersistence()` from its background/termination hook. The method is valid
while the drive is running or stopped, propagates a flush error, and is never
invoked automatically by `start()` or `stop()`. The default sink is an
in-memory no-op. Keystore durability, snapshot I/O, retry policy, and observed
success/error reporting remain native application gates; this SDK-free shell
does not receive ratchet/prekey material.

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

When the native adapter has a negotiated MTU and a conservative goodput
estimate, pass them to `GattLinkBridge.onConnected(..., quality =
LinkQualityHint(...))`. The hint is host-local scheduling metadata and is not
wire data. Values outside the documented range close the physical link and
are rejected before `MeshNode.linkUp`; derive goodput conservatively rather
than copying an optimistic application-level rate.
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
