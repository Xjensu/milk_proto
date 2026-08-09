# Mobile host backpressure handoff

Status: implementation handoff; no wire or specification change.

This note closes a host-side integration gap found in the platform-neutral
Kotlin coordinator (`implementation/mesh-milk/platform/kotlin-mesh-drive/.../MeshDrive.kt`)
and the equivalent Swift drive loop described by
`implementation/mesh-milk/MOBILE_INTEGRATION.md`.  The Rust FFI already exposes
three local-lifecycle signals:

* `pollClose() -> u64?` - one transport-local close action.  It is **not** a
  wire frame.  The host must disconnect the physical BLE link and then feed the
  resulting native disconnect callback to `linkDown`.
* `closeActionsDropped() -> u64` - sticky count of close actions that could not
  enter the bounded host-facing close queue.
* `eventQueueDropped() -> u64` - sticky count of application callbacks dropped
  because the bounded event FIFO was full.

The current `MeshDrive` source drains `pollSend()` and `pollEvent()` but does
not model these three methods.  A native host that copies the old loop can
therefore leave a link alive after a fail-closed session or miss a user-visible
event without an alarm.  This handoff is the required implementation contract
until a platform module is added.

## Required drain order

Each Android service tick and each iOS serial-queue pump must execute this order
on the single owner queue:

```text
1. Drain every pollClose() result.
   - remove any queued outbound fragments for that link;
   - call the platform disconnect/close primitive;
   - do not call linkDown here unless the platform callback is synchronous;
     otherwise call linkDown exactly once from the native disconnect callback.
2. Read closeActionsDropped() and eventQueueDropped().
3. If either counter is greater than the last observed value:
   - latch HostOverload (do not clear a sticky counter or retry blindly);
   - stop admitting new links and stop dialing;
   - disconnect all currently active links through the platform adapter;
   - surface the exact counter values to diagnostics/UI;
   - after callbacks have delivered linkDown for every link, restart/recreate
     the node or require an explicit operator recovery action.
4. Drain pollSend() until None, respecting BLE back-pressure and preserving
   frame order.
5. Drain pollEvent() until None.  Continue draining after the overload signal
   so already queued lifecycle events are not discarded by the host.
```

Counters are monotonic for the lifetime of the node.  A host may store a
baseline at node creation, but must never reset the Rust counters or interpret
an observed non-zero delta as a transient warning.  `eventQueueDropped` means
that a required callback such as `MessageReceived` may already be unavailable;
it is not equivalent to successful protocol delivery and requires an explicit
resync/restart policy.

## Adapter seam (platform-neutral)

The host-side coordinator should depend on an injected operation with this
shape rather than importing Android or CoreBluetooth types into the shared
source module:

```text
interface LinkCloser {
    close(link: UInt64)
}

data class HostOverload(
    closeActionsDropped: UInt64,
    eventQueueDropped: UInt64,
)
```

Kotlin maps `close` to `BluetoothGattServer.cancelConnection` or the central
disconnect operation.  Swift maps it to `cancelPeripheralConnection` or the
peripheral-side teardown.  The native callback remains responsible for the
single `linkDown` call; a synchronous fake may complete that callback inline.
The shared coordinator only owns ordering, pending-fragment removal, admission
latching, and diagnostics delivery.

## Required source-level acceptance tests

The Android/JVM and iOS host test targets should use a fake node and fake radio
adapter and assert all of the following without BLE hardware:

1. A queued `pollClose` causes exactly one `close(link)` call, removes pending
   fragments for that link, and does not call `linkDown` twice when the fake
   disconnect callback arrives.
2. An increment in `closeActionsDropped` latches `HostOverload`, prevents a new
   `onLinkUp`/dial, and closes every active link.  The latch remains set after
   the counter is read again.
3. An increment in `eventQueueDropped` is surfaced with both counter values and
   triggers the same explicit resync/restart path; it is never silently logged
   and ignored.
4. `pollClose` is drained before `pollSend`, and `pollEvent` continues draining
   after overload is reported.  This protects terminal cleanup and preserves
   already queued lifecycle notifications.

These tests are host-contract tests only.  They do not claim Kotlin/Swift
compilation, background scheduling, GATT behavior, or physical BLE
interoperability; those remain separate external gates.

## Evidence boundary

No Rust protocol field, bundle encoding, cryptographic primitive, or frozen
specification document is changed by this handoff.  The missing implementation
is deliberately left to the native Android/iOS integration story, where the
correct physical close primitive and lifecycle callback are available.
