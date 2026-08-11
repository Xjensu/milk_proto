# `@mesh-milk/react-native-mesh`

This package is the TypeScript application seam for the MMP/1 React Native bridge.
It mirrors §2 of `REACT_NATIVE_INTEGRATION.md`: promise-based lifecycle, identity,
messaging, group, diagnostics, and persistence methods plus an app-level event stream.

The native module owns the BLE radio, the drive queue, the monotonic clock, and the
`MmpNode` instance. BLE advertisements, GATT frames, and other link bytes are never
represented by this package and must never be sent through the React Native JS bridge.
Only base64-encoded opaque values documented by the bridge and ciphertext-free app
events cross into JavaScript.

## Beta status and metrics

The client exposes two secret-free snapshots for beta-test screens and local
diagnostics:

```ts
const status = await client.status();
// { tier, persistentStore, diagnosticsEnabled, lastTickMs }
const metrics = await client.metrics();
// { storeLen, closeActionsDropped, eventQueueDropped,
//   diagnosticRecords, diagnosticClamps, diagnosticDrops }
```

The counters are local observations and are not telemetry. They contain no
peer/session identifiers, provider handles, wire bytes, or key material. The
native implementation remains the authority for their values and numeric
encoding.

## Installation and native registration

The package has no runtime dependencies other than the host application's `react` and
`react-native` peer dependencies. The Kotlin/Swift module must be built separately and
registered under the native name `MmpMesh`; Expo Go cannot load that module.

```sh
npm install @mesh-milk/react-native-mesh
```

The package's `build` script uses the consuming project's TypeScript toolchain. Pin and
verify the React Native version and TurboModule codegen support in that project before
using the optional `NativeMmpMesh` codegen surface.

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
