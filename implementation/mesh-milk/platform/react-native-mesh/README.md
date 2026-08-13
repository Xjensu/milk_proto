# `@mesh-milk/react-native-mesh`

This package is the TypeScript application seam for the MMP/1 React Native bridge.
It mirrors §2 of `REACT_NATIVE_INTEGRATION.md`: promise-based lifecycle, identity,
messaging, group, diagnostics, and persistence methods plus an app-level event stream.

The native module owns the BLE radio, the drive queue, the monotonic clock, and the
`MmpNode` instance. BLE advertisements, GATT input, and link lifecycle callbacks are
never represented by this package. The optional bounded output poll used by a beta
transport harness exposes only base64-encoded `OutFrame.bytes`; no binary array or
secret crosses the React Native JS bridge. Ciphertext-free app events are normalized
before they reach JavaScript.

## Authority, scope, and friendship API

The client also mirrors the safe, app-facing authority and relationship controls
already exported by `mmp-ffi`:

```ts
await client.setWallClock(Math.floor(Date.now() / 1000));
const scope = await client.subscribeScope('train/nationalrail/1234');
await client.pinAuthorityRoot(scope, issuerPublicKeyB64);
const cert = await client.mintAuthCert(1, subjectPublicKeyB64, scope, 1, 10, expiryUnixS);
await client.publishAnnouncement(scope, 1, 1, 'service delayed', cert);
await client.requestFriendship(peerPublicKeyB64);
await client.allowFriendshipFrom(peerPublicKeyB64);
```

`scope` is a content-derived 16-byte identifier represented as lowercase hex;
public keys, signatures, claims, and certificates are base64 strings. The
native FFI validates lengths, authority policy, and expiry. `setWallClock` is
only a local input for certificate validation; it does not establish trusted
network time. `requestFriendship` and `allowFriendshipFrom` take identity public
keys, not 16-byte NodeIDs.

The package intentionally does not expose the FFI constructors (`new` and
`new_with_store_limit`), `tick`, beacon generation, or link injection. Construction
and storage limits are native configuration; `start`/`stop` remain the lifecycle
seam. A beta transport harness may use the bounded `pollSendBatch`,
`pollEventBatch`, and `pollCloseBatch` methods to drain host-owned queues; each
call preserves FIFO order, treats zero as a no-op, and relies on the native fixed
limit of 64. `pollSendBatch` returns `OutFrame.bytes` as base64 and does not grant
the JS app ownership of the radio or session lifecycle.

## Beta-test session helper

For manual or beta builds, `BetaTestSession` provides a bounded local event
history and a deterministic JSON-safe report. It has no uploader and never
changes the protocol or native lifecycle policy:

```ts
import { BetaTestSession } from '@mesh-milk/react-native-mesh';

const session = new BetaTestSession(client, { maxEvents: 128 });
await session.start('leaf');
// Exercise the app, then persist explicitly at a test checkpoint.
await session.flush();
const reportJson = await session.reportJson();
await session.stop();
```

The report contains only status, local counters, capabilities, app-level event
payloads, and an event-drop count. Its `schemaVersion` is the local report
compatibility fence (currently `1`, independent of the MMP/1 wire version); a
consumer must reject unknown values before decoding the remaining fields. Event
history keeps the newest entries after
the configured cap; dropped events are counted for test diagnostics.
Call `session.clearHistory()` between manual scenarios to reset only this local
report window and its drop counter; it does not stop the mesh, flush storage, or
acknowledge protocol events.

For deterministic automated steps, `await session.waitForEvent('delivered',
10_000)` resolves with the next matching app event. The wait requires a started
session, accepts a timeout from 1 ms through 60 s, and rejects with
`BetaTestSessionError` when it expires. Its temporary listener and timer are
removed on both success and timeout; it never exposes raw transport data.

When a test already has a message ID, use `await session.waitForDelivery(id,
10_000)` instead. It samples the native delivery state before and after
listener registration, then waits only for the matching message's terminal
`delivered` or `expired` state. This avoids confusing an unrelated delivery
event with the scenario under test and closes the state/listener hand-off
race. The helper is local and deterministic; it does not acknowledge or alter
the protocol message.

`BetaTestSession` is also the application-facing composition facade for the
existing client surface. Once started, the same session exposes direct text and
application-typed direct bodies (`sendBody`), group (including application-typed
bodies via `groupSendBody`), explicit-pull file, authority/scope, friendship, identity/claim,
diagnostics, persistence, and runtime tier methods (`setTier`, `sendText`,
`createGroup`, `sendFile`,
`fetchFile`, `subscribeScope`, `pinAuthorityRoot`, `requestFriendship`, and the
corresponding operations already present on `MmpClient`). It adds no native
methods and does not alter wire behavior. Operations that can mutate or
exercise the running mesh fail with `BetaTestSessionError` until `start()` has
completed; `start()` and `stop()` are idempotent at the facade boundary.
Concurrent `start()` calls join the first native start operation (the first
tier argument wins), and a `stop()` issued while startup is pending waits for
that operation before stopping it. A failed startup removes the listener and
leaves the session stopped.

For a host-owned beta transport harness, a started session also exposes
`pollSendBatch(maxItems = 64)`, `pollEventBatch(maxItems = 64)`, and
`pollCloseBatch(maxItems = 64)`. Each method delegates to the corresponding
`MmpClient` bounded poll and preserves native FIFO order and clamping semantics;
the default matches the native batch bound of 64. `pollEventBatch` drains the
native event queue independently of `onEvent` listeners and the session's
bounded report history, so applications must not assume polled events will also
appear in either stream. These methods return only normalized `OutFrame`,
`MmpEvent`, and numeric link values; they do not expose raw BLE input or alter
session ownership.

The facade delegates validation of IDs, base64 values, authority policy, file
limits, and enum spellings to `MmpClient`/native. Unknown capabilities and
delivery states therefore fail closed rather than being coerced. `nodeId()` and
`publicKey()` expose public identity values only; seeds, private keys, provider
handles, raw BLE input, and automatic uploads remain unavailable. `flush()` is
always explicit, and `report()`/`reportJson()` are local snapshots only.

Group application bodies use base64 at the bridge boundary. Decrypted
`groupMessage` events include the numeric `apptype` supplied to
`groupSendBody`, so a beta app can dispatch registered application payloads
without guessing from body bytes.

## Beta status and metrics

`client.capabilities()` returns the stable, secret-free protocol/API capability
list and whether this node uses a file-backed store. Values describe core
support only; native permissions, background policy, and BLE hardware remain
separate platform-health evidence. Unknown native capability values fail closed.

The client exposes two secret-free snapshots for beta-test screens and local
diagnostics:

```ts
const status = await client.status();
// { tier, persistentStore, diagnosticsEnabled, lastTickMs }
const metrics = await client.metrics();
// { storeLen, closeActionsDropped, eventQueueDropped,
//   diagnosticRecords, diagnosticClamps, diagnosticDrops }
```

Diagnostics recording is opt-in. Call `client.enableDiagnostics()` at the
start of a scenario and `client.disableDiagnostics()` at its boundary to stop
new journey records without discarding the current local dump.

The counters are local observations and are not telemetry. They contain no
peer/session identifiers, provider handles, wire bytes, or key material. The
native implementation remains the authority for their values and numeric
encoding.

Friendship completion is delivered through `client.onEvent` as
`{ type: 'friendshipEstablished', peer, since }`. The native adapter maps
`FfiEvent::FriendshipEstablished` to this camel-case event, where `peer` is a
lowercase 16-byte NodeID hex string and `since` is the host monotonic timestamp
in milliseconds. It is emitted for both the dependent and patron when the
friendship is established or renewed; the request/allow promises only enqueue
the operation and are not completion acknowledgements.

Announcements arrive through `client.onEvent` as the secret-free
`announcement` event. Its fields are `id`, `scope`, `annType`, `notAfter`,
`text`, `from`, `official`, `effectivePriority`, and `scopeNameHint`. The
variable byte fields `text` and `scopeNameHint` are base64 strings at this
boundary; decode them explicitly as UTF-8 only when the application has chosen
that presentation. Use `effectivePriority` for notification prominence and
treat `scopeNameHint` as a display-only hint, never as an authority or
admission decision.

## Installation and native registration

The package has no runtime dependencies other than the host application's `react` and
`react-native` peer dependencies. The Kotlin/Swift module must be built separately and
registered under the native name `MmpMesh`; Expo Go cannot load that module.

```sh
npm install @mesh-milk/react-native-mesh
```

The package's `build` script uses the consuming project's TypeScript toolchain. Pin and
verify the React Native version and TurboModule codegen support in that project before
using the `NativeMmpMesh` codegen surface. The package declares
`codegenConfig` (`NativeMmpMeshSpec`, `modules`, `src`) so a New Architecture
consumer can discover the spec during its normal React Native Codegen pass; this
metadata does not compile or register the Kotlin/Swift implementation by itself.

## Inject the native module

Injection makes the bridge seam testable and lets an application choose a hand-written
or generated native implementation. An explicitly injected module is checked at
construction. The exported default client is lazy, so importing the package before native
registration is safe; its first operation rejects with `MmpNativeModuleError` if the
module is missing or incomplete.

```ts
import { createMmpClient } from '@mesh-milk/react-native-mesh';

const client = createMmpClient(); // resolves NativeModules.MmpMesh
await client.start('leaf');
const id = await client.sendText(peerIdHex, 'hello over the mesh');
const unsubscribe = client.onEvent((event) => {
  if (event.type === 'delivered' && event.id === id) {
    console.log('delivered');
  }
});
unsubscribe();
```

## File transfer

The beta app can share a file with a known peer and explicitly pull a verified
offer. Content-addressed IDs (`messageId`, `root`, and event IDs) are lowercase
hex strings. File bytes and variable-length manifest bytes use base64 strings;
the identity seed, private keys, provider handles, and BLE/session bytes never
cross this API.

```ts
const sent = await client.sendFile(peerIdHex, 'photo.jpg', 'image/jpeg', dataB64);
await client.fetchFile(sent.root, appCachePath);
const progress = await client.fileProgress(sent.root);
if (progress?.state === 'assembled') {
  console.log(`received ${progress.haveSymbols}/${progress.totalSymbols} symbols`);
}
```

`sendFile` is bounded by the native `FILE_MAX` policy and may require the whole
payload in host memory. A `fileOffered` event presents verified entries; the app
must call `fetchFile` for an entry rather than downloading a full file
automatically. `fileReady` reports the native destination path, while
`fileFailed` reports a typed abandonment reason. `cancelFetch` stops an active
assembly without deleting ordinary partial symbols from the mesh store.

Native UniFFI file-failure enums use `HashMismatch` and `Abandoned`; native event
adapters MUST map them explicitly to the JavaScript values `hashMismatch` and
`abandoned` (the exported `mapNativeFileFailReason` helper is the reference
mapping). Generic lower-casing is not valid because `HashMismatch` would become
`hashmismatch`.

Delivery states are normalized just as explicitly. `MmpClient.messageState()` and
`deliveryProgress` events accept the native spellings `Queued`, `InMesh`,
`Delivered`, and `Expired`, the generated Kotlin spellings `QUEUED`, `IN_MESH`,
`DELIVERED`, and `EXPIRED`, and the historical lower-case spellings `inmesh` and
`in_mesh` for compatibility. They become the public values `queued`, `inMesh`,
`delivered`, and `expired`. Unknown native values throw `MmpNativeModuleError`
instead of being silently coerced. Use the exported `mapNativeDeliveryState`
helper when writing a custom native event adapter; generic lower-casing is not
valid because `InMesh` would become `inmesh` and Kotlin's `IN_MESH` would become
`in_mesh`.

For tests, pass a method-complete fake `MmpMeshNative` and an event emitter implementing
`addListener`. The identity seed remains native-only; there is no JavaScript API that can
read or write it.

## Build evidence boundary

`npm run build` requires a TypeScript compiler and the React Native type declarations from
the consuming app. Repository CI additionally runs `npm run typecheck` with the pinned
TypeScript dev dependency and a minimal declaration fixture for the imported React Native
symbols. That fixture checks this wrapper's TypeScript API and does not model TurboModule
code generation, Kotlin/Swift compilation, background execution, GATT interoperability,
or behavior on a physical device. The repository source-level test checks the API inventory,
encoding conventions, module rejection, and the no-raw-BLE rule.
