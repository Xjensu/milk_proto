# MMP/1 React Native Integration Guide

**How to embed the mesh_milk Rust core in a React Native application.**
Audience: RN/mobile engineers wiring `mmp-ffi` into a JS/TS app on Android and iOS.
Companion documents: `MOBILE_INTEGRATION.md` (the general mobile guide — read it first; this
document assumes its §1–§2 architectural rules and does not repeat the Android/iOS
radio-duty checklists from its §4–§5, only cross-references them), `specification/12-rust-blueprint.md`
§5–§6 (normative FFI surface), `specification/09-transport-ble.md` (BLE mapping, MTU/credit numbers).

---

## 1. Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│  JS/TS app code                                                   │
│    MmpClient (promise API + EventEmitter)                         │
├─────────────────────────────────────────────────────────────────┤
│  RN bridge / JSI                       ← app-level events + calls │
│    NativeModules.MmpMesh  (TurboModule spec, or classic NativeModule) │
├─────────────────────────────────────────────────────────────────┤
│  Native module (Kotlin / Swift)                                   │
│    • owns the BLE radio (BluetoothGattServer/Scanner, CBCentral/  │
│      PeripheralManager)                                            │
│    • owns the drive loop (HandlerThread / DispatchQueue)          │
│    • holds the single MmpNode instance                            │
├─────────────────────────────────────────────────────────────────┤
│  UniFFI-generated bindings (Kotlin / Swift, from mmp-ffi)          │
├─────────────────────────────────────────────────────────────────┤
│  Rust core (mmp-ffi → mmp-node → mmp-core/router/crypto/store)    │
└─────────────────────────────────────────────────────────────────┘
```

### The golden rule

**The BLE radio is implemented natively (Kotlin/Swift). Link bytes go native → Rust
directly, through the UniFFI binding, and never through the JS bridge.**

`BluetoothGattServer.onCharacteristicWriteRequest` and
`CBPeripheralManagerDelegate.didReceiveWrite` fire many times per second per active link
during a sync burst, each with up to (MTU−3) bytes (§09 §5.1: negotiated MTU 247+, or a
23-byte floor if negotiation fails or the peer doesn't support it). Round-tripping each of
those chunks through the RN bridge — JSON-serialize a byte array (classic bridge) or even a
JSI call with base64 encode/decode (new architecture) on every single GATT write — adds
scheduler latency (the JS thread is shared with rendering) and burns CPU/radio wakeups for
no protocol benefit, since none of that traffic is ever rendered. It also defeats the
sans-IO design: `MmpNode` already reassembles fragments internally (`mmp-node/src/lib.rs`
`Node::on_tal_event` / `pump_link`) — routing raw link bytes through JS would just add a
second, slower reassembly hop in front of the one that already exists in Rust.

JS receives only **app-level events** — `MessageReceived`, `Delivered`, `ContactSeen`, etc.
(the `Event` enum in `mmp-node/src/lib.rs`) — emitted at a rate of maybe a few per second
even under load, which the bridge handles trivially.

Concretely: the Kotlin/Swift module holds `BluetoothGattServer`/`CBPeripheralManager` and
the `MmpNode` object in the same process and calls `node.linkData(link, bytes)`
straight from the characteristic-write callback — the clock is advanced separately, on
the drive thread's own timer, via `node.tick(now_ms)`; it is never threaded through the
link-data call itself. JS never sees a raw BLE byte.

## 2. The TypeScript API (`MmpClient`)

Ship a thin, promise-based wrapper over the native module rather than exposing
`NativeModules.MmpMesh` directly to app code — it gives you a stable seam to swap the
native module implementation (hand-written vs. `uniffi-bindgen-react-native`-generated)
without touching call sites, and a place to centralize the id-encoding convention below.

### ID and byte encoding conventions

| MMP type | Size | JS representation | Rationale |
|---|---|---|---|
| `NodeId` / `Id16` (peer id, message id, group id, conv id) | 16 bytes | lowercase hex string (32 chars) | fixed-length, human-diffable in logs, safe as a JS `Map`/object key |
| `Key32` / `Sig64` / claim bodies / beacon payload / `OutFrame.bytes` / diagnostic snapshots | variable | base64 string | binary-safe across the bridge on every RN version; see the ArrayBuffer note below |
| `seed: bytes32` | 32 bytes | **never appears in JS.** Generated and stored natively (Keychain/Keystore); JS only ever triggers "create if absent" | secret boundary, §5 |

**On ArrayBuffer vs. base64**: with the New Architecture (JSI/TurboModules, default since
RN 0.76), it is possible to pass `ArrayBuffer`/typed-array arguments and return values in a
TurboModule spec without going through JSON serialization, and this is meaningfully faster
for larger payloads. Support and ergonomics for `ArrayBuffer` in Codegen-generated specs
have been in flux across 0.74–0.76 releases and differ between iOS and Android codegen —
**verify against the exact RN version pinned in your project before relying on it**. Base64
strings are the safe, version-independent baseline used throughout this guide; treat
ArrayBuffer passing as an opt-in perf upgrade once you've confirmed your RN version's
codegen supports it end-to-end (see the Hermes pitfall in §9).

### `src/mmp/MmpClient.ts`

```ts
import { NativeModules, NativeEventEmitter, EmitterSubscription } from 'react-native';

// ---- wire types (mirror the FFI surface; keep in sync with mmp-ffi) --------

export type NodeIdHex = string;   // 32 lowercase hex chars (16 bytes)
export type MsgIdHex = string;    // 32 lowercase hex chars (16 bytes)
export type GroupIdHex = string;  // 32 lowercase hex chars (16 bytes)

export type Tier = 'leaf' | 'relay' | 'anchor';

export type DeliveryState = 'queued' | 'inMesh' | 'delivered' | 'expired';

export type MmpEvent =
  | { type: 'messageReceived'; conv: MsgIdHex; from: NodeIdHex; id: MsgIdHex; body: string /* base64 */; apptype: number }
  | { type: 'messageGap'; conv: MsgIdHex; parents: MsgIdHex[] }
  | { type: 'delivered'; id: MsgIdHex }
  | { type: 'deliveryProgress'; id: MsgIdHex; state: DeliveryState }
  | { type: 'claimUpdated'; owner: NodeIdHex; field: number }
  | { type: 'compromiseWarning'; owner: NodeIdHex }
  | { type: 'contactSeen'; peer: NodeIdHex }
  | { type: 'established'; peer: NodeIdHex }
  | { type: 'groupInvite'; gid: GroupIdHex; from: NodeIdHex }
  | { type: 'groupMessage'; gid: GroupIdHex; from: NodeIdHex; id: MsgIdHex; body: string /* base64 */ }
  | { type: 'groupEpochChanged'; gid: GroupIdHex; epoch: number }
  | { type: 'tierChanged'; tier: Tier };

interface MmpMeshNative {
  // Lifecycle. Idempotent: if a node already exists for this seed/store, returns its id.
  start(tier: Tier): Promise<NodeIdHex>;
  stop(): Promise<void>;
  setTier(tier: Tier): Promise<void>;

  // Identity / claims.
  nodeId(): Promise<NodeIdHex>;
  publicKey(): Promise<string>;                 // base64, 32-byte Ed25519 identity public key
  ownClaims(): Promise<string[]>;               // base64-encoded Claim records
  ingestClaim(claimB64: string, verified: boolean): Promise<void>;
  setNickname(name: string, seq: number): Promise<void>;
  block(peer: NodeIdHex): Promise<void>;

  // Messaging.
  sendText(peer: NodeIdHex, text: string): Promise<MsgIdHex>;
  messageState(id: MsgIdHex): Promise<DeliveryState | null>;

  // Groups.
  createGroup(): Promise<GroupIdHex>;
  groupInvite(gid: GroupIdHex, peer: NodeIdHex): Promise<void>;
  groupSend(gid: GroupIdHex, text: string): Promise<MsgIdHex>;
  groupMembers(gid: GroupIdHex): Promise<NodeIdHex[]>;
  groupRemove(gid: GroupIdHex, member: NodeIdHex): Promise<number>;  // returns the new epoch

  // Diagnostics.
  enableDiagnostics(): Promise<void>;
  diagDump(): Promise<string>;

  // Storage / persistence — only meaningful when the native module started the node with a
  // store path (see §5 "Store persistence"); a no-op / zero on an in-memory-only node.
  storeLen(): Promise<number>;
  flush(): Promise<void>;
  lastPersistError(): Promise<string | null>;

  // NativeEventEmitter requires these two no-ops on iOS; Android ignores them.
  addListener(eventName: string): void;
  removeListeners(count: number): void;
}

const native = NativeModules.MmpMesh as MmpMeshNative;
const emitter = new NativeEventEmitter(NativeModules.MmpMesh);

export class MmpClient {
  private sub: EmitterSubscription | null = null;

  /** Starts (or attaches to) the native mesh service. Call once, e.g. from App root. */
  async start(tier: Tier = 'leaf'): Promise<NodeIdHex> {
    return native.start(tier);
  }

  async stop(): Promise<void> {
    return native.stop();
  }

  setTier(tier: Tier): Promise<void> {
    return native.setTier(tier);
  }

  nodeId(): Promise<NodeIdHex> {
    return native.nodeId();
  }

  publicKey(): Promise<string> {
    return native.publicKey();
  }

  sendText(peer: NodeIdHex, text: string): Promise<MsgIdHex> {
    return native.sendText(peer, text);
  }

  messageState(id: MsgIdHex): Promise<DeliveryState | null> {
    return native.messageState(id);
  }

  createGroup(): Promise<GroupIdHex> {
    return native.createGroup();
  }

  groupInvite(gid: GroupIdHex, peer: NodeIdHex): Promise<void> {
    return native.groupInvite(gid, peer);
  }

  groupSend(gid: GroupIdHex, text: string): Promise<MsgIdHex> {
    return native.groupSend(gid, text);
  }

  groupMembers(gid: GroupIdHex): Promise<NodeIdHex[]> {
    return native.groupMembers(gid);
  }

  groupRemove(gid: GroupIdHex, member: NodeIdHex): Promise<number> {
    return native.groupRemove(gid, member);
  }

  setNickname(name: string, seq: number): Promise<void> {
    return native.setNickname(name, seq);
  }

  ownClaims(): Promise<string[]> {
    return native.ownClaims();
  }

  ingestClaim(claimB64: string, verified: boolean): Promise<void> {
    return native.ingestClaim(claimB64, verified);
  }

  block(peer: NodeIdHex): Promise<void> {
    return native.block(peer);
  }

  enableDiagnostics(): Promise<void> {
    return native.enableDiagnostics();
  }

  diagDump(): Promise<string> {
    return native.diagDump();
  }

  storeLen(): Promise<number> {
    return native.storeLen();
  }

  /** Force an immediate persist; also called automatically from the native on-background
   * lifecycle hook (Android onStop / iOS applicationDidEnterBackground), so app code
   * normally does not need to call this itself — see §5 "Store persistence". */
  flush(): Promise<void> {
    return native.flush();
  }

  lastPersistError(): Promise<string | null> {
    return native.lastPersistError();
  }

  /** Subscribe to the app-level event stream (one native → JS emitter, many listeners). */
  onEvent(handler: (ev: MmpEvent) => void): () => void {
    const sub = emitter.addListener('mmp', handler);
    return () => sub.remove();
  }
}

export const mmp = new MmpClient();
```

### TurboModule spec (New Architecture codegen)

If you generate a TurboModule (recommended — see §3), add the Codegen spec next to the
client, e.g. `src/mmp/NativeMmpMesh.ts`:

```ts
import type { TurboModule } from 'react-native';
import { TurboModuleRegistry } from 'react-native';

export interface Spec extends TurboModule {
  start(tier: string): Promise<string>;
  stop(): Promise<void>;
  setTier(tier: string): Promise<void>;
  nodeId(): Promise<string>;
  publicKey(): Promise<string>;
  sendText(peer: string, text: string): Promise<string>;
  messageState(id: string): Promise<string | null>;
  createGroup(): Promise<string>;
  groupInvite(gid: string, peer: string): Promise<void>;
  groupSend(gid: string, text: string): Promise<string>;
  groupMembers(gid: string): Promise<string[]>;
  groupRemove(gid: string, member: string): Promise<number>;
  setNickname(name: string, seq: number): Promise<void>;
  ownClaims(): Promise<string[]>;
  ingestClaim(claimB64: string, verified: boolean): Promise<void>;
  block(peer: string): Promise<void>;
  enableDiagnostics(): Promise<void>;
  diagDump(): Promise<string>;
  storeLen(): Promise<number>;
  flush(): Promise<void>;
  lastPersistError(): Promise<string | null>;
  addListener(eventName: string): void;
  removeListeners(count: number): void;
}

export default TurboModuleRegistry.getEnforcing<Spec>('MmpMesh');
```

Codegen (`react-native.config.js` → `codegenConfig`) turns this into generated C++/ObjC++/
Java glue at build time; the Kotlin/Swift classes in §3–§4 implement the generated
interface instead of extending `ReactContextBaseJavaModule` directly. TurboModule enum
support (mapping the `Tier`/`DeliveryState` string unions above to proper native enums) is
codegen-version-dependent — plain strings with native-side validation, as sketched here, are
the portable choice; tighten to generated enum types once you've pinned an RN version and
verified codegen handles them the way you expect.

## 3. Android (Kotlin) native module

The Android side is the same foreground-service-owned `MeshService` from
`MOBILE_INTEGRATION.md` §4 — RN does not replace it, it only adds a thin module that talks
to it. Do **not** duplicate the `MmpNode` inside the RN module; the FG service must keep
running (and relaying) even if the RN JS context is torn down (app backgrounded /
low-memory JS reload), so the service is the single owner of the node and the module is a
client of the service.

### 3.1 Module responsibilities

1. Bind to `MeshService` (already running, or start it).
2. Run the drive loop on **one** background `HandlerThread` — never the JS thread, never
   the main/UI thread. Tick every ~1 s, plus an out-of-band pump immediately after any
   `linkData`/`linkUp`/`linkDown` injection, so sync bursts drain promptly instead of
   waiting for the next tick.
3. Drain `pollSend()` into the BLE layer (already inside `MeshService`, per
   `MOBILE_INTEGRATION.md` §4.2) and `pollEvent()` into a `DeviceEventEmitter`/RCTEventEmitter
   emission to JS.
4. Expose the promise-based methods from §2 by calling straight through to the UniFFI
   Kotlin bindings, hopping onto the drive thread for anything that mutates `MmpNode`
   state (single-owner rule, §5).

### 3.2 TurboModule implementation

```kotlin
// android/app/src/main/java/com/yourapp/mmp/MmpMeshModule.kt
package com.yourapp.mmp

import android.os.Handler
import android.os.HandlerThread
import com.facebook.react.bridge.LifecycleEventListener
import com.facebook.react.bridge.Promise
import com.facebook.react.bridge.ReactApplicationContext
import com.facebook.react.module.annotations.ReactModule
import com.facebook.react.turbomodule.core.interfaces.TurboModule
import uniffi.mmp_ffi.MmpNode
import uniffi.mmp_ffi.FfiEvent
import android.util.Base64
import java.util.concurrent.ConcurrentHashMap

@ReactModule(name = MmpMeshModule.NAME)
class MmpMeshModule(reactContext: ReactApplicationContext) :
    NativeMmpMeshSpec(reactContext), TurboModule, LifecycleEventListener {

    companion object {
        const val NAME = "MmpMesh"
    }

    // The drive thread: the ONLY thread that ever touches `node` (§5 single-owner rule).
    private val driveThread = HandlerThread("mmp-drive").apply { start() }
    private val driveHandler = Handler(driveThread.looper)

    private var node: MmpNode? = null
    private var hasListeners = false

    private val tickRunnable = object : Runnable {
        override fun run() {
            node?.let { n ->
                n.tick(android.os.SystemClock.elapsedRealtime().toULong())
                drain(n)
            }
            // 1 s cadence; MeshService's own alarm-driven pump handles Doze wakeups
            // (MOBILE_INTEGRATION.md §4) — this handler only covers the foreground case.
            driveHandler.postDelayed(this, 1_000)
        }
    }

    override fun start(tier: String, promise: Promise) {
        driveHandler.post {
            try {
                if (node == null) {
                    val seed = Keystore.loadOrCreateSeed(reactApplicationContext)
                    // Bundles (queued/in-mesh, including our own outgoing sends) survive
                    // process death via this path; SeenIndex/receipts/ratchet state
                    // intentionally do not (doc 04 §7 drop-and-redownload) — see MmpNode's
                    // own constructor doc comment in mmp-ffi/src/lib.rs.
                    val storePath = reactApplicationContext.filesDir.resolve("mmp").path
                    node = MmpNode(seed, tierOf(tier), storePath)
                    reactApplicationContext.addLifecycleEventListener(this)
                    driveHandler.post(tickRunnable)
                }
                promise.resolve(hex(node!!.nodeId()))
            } catch (e: Exception) {
                promise.reject("MMP_START_FAILED", e)
            }
        }
    }

    override fun stop(promise: Promise) {
        driveHandler.post {
            driveHandler.removeCallbacks(tickRunnable)
            try { node?.flush() } catch (e: Exception) { /* best-effort; see flush() doc comment */ }
            node = null
            reactApplicationContext.removeLifecycleEventListener(this)
            promise.resolve(null)
        }
    }

    // Natural persistence point (mmp-ffi's `flush()` doc comment): a host `onPause`/
    // `applicationWillResignActive`-equivalent hook, rather than waiting for the next
    // periodic tick()-driven persist. `ReactContext`'s host-pause callback fires when the
    // RN activity backgrounds, which is the closest RN-level analogue to Android `onStop`.
    override fun onHostResume() {}
    override fun onHostPause() {
        driveHandler.post {
            try { node?.flush() } catch (e: Exception) { /* best-effort; see flush() doc comment */ }
        }
    }
    override fun onHostDestroy() {}

    override fun sendText(peer: String, text: String, promise: Promise) {
        driveHandler.post {
            val n = node ?: return@post promise.reject("MMP_NOT_STARTED", "call start() first")
            try {
                val id = n.sendText(unhex16(peer), text)
                drain(n)
                promise.resolve(hex(id))
            } catch (e: Exception) {
                promise.reject("MMP_SEND_FAILED", e)
            }
        }
    }

    override fun messageState(id: String, promise: Promise) {
        driveHandler.post {
            val n = node ?: return@post promise.reject("MMP_NOT_STARTED", "call start() first")
            promise.resolve(n.messageState(unhex16(id))?.name?.lowercase())
        }
    }

    // createGroup / groupInvite / groupSend / groupMembers / groupRemove / setNickname /
    // ownClaims / ingestClaim / block / enableDiagnostics / diagDump / setTier / publicKey /
    // storeLen / flush / lastPersistError follow the identical "post to driveHandler, call
    // the UniFFI binding, resolve/reject" shape and are omitted here for brevity — see
    // MmpMeshModuleFull.kt in the reference app.

    /** Feed a BLE write straight from MeshService's GATT callback — bytes never touch JS.
     * No `now`/timestamp param: the clock only ever advances via `tickRunnable`'s
     * `node.tick(now_ms)` call above, never as a side effect of feeding link bytes in. */
    fun onLinkData(link: Long, bytes: ByteArray) {
        driveHandler.post {
            node?.let { n ->
                n.linkData(link.toULong(), bytes)
                drain(n)
            }
        }
    }

    fun onLinkUp(link: Long, initiator: Boolean) {
        driveHandler.post {
            node?.linkUp(link.toULong(), initiator)
        }
    }

    fun onLinkDown(link: Long) {
        driveHandler.post { node?.linkDown(link.toULong()) }
    }

    /** Feed a scanned advertisement straight from MeshService's scan callback — payload/rssi
     * never touch JS, same as link bytes above. No `now`/timestamp param, same reason as
     * `onLinkData`: the core folds it into the N̂ population estimate (contact radar / dial
     * gating) and never emits an event on its own. */
    fun onBeacon(payload: ByteArray, rssi: Byte) {
        driveHandler.post { node?.onBeacon(payload, rssi) }
    }

    private fun drain(n: MmpNode) {
        while (true) {
            val out = n.pollSend() ?: break
            MeshService.instance?.notifyFragmented(out.link.toLong(), out.bytes) // MOBILE_INTEGRATION.md §4.2
        }
        while (true) {
            val ev = n.pollEvent() ?: break
            emit(ev)
        }
    }

    private fun emit(ev: FfiEvent) {
        if (!hasListeners) return
        val map = eventToWritableMap(ev) // maps FfiEvent variants to the MmpEvent JSON shape in §2
        emitOnMmp(map) // codegen-generated typed emitter, or emitDeviceEvent("mmp", map) on classic bridge
    }

    override fun addListener(eventName: String) { hasListeners = true }
    override fun removeListeners(count: Int) { hasListeners = count == 0 }

    private fun hex(id: ByteArray): String = id.joinToString("") { "%02x".format(it) }
    private fun unhex16(s: String): ByteArray =
        ByteArray(16) { i -> s.substring(i * 2, i * 2 + 2).toInt(16).toByte() }
}
```

```kotlin
// android/app/src/main/java/com/yourapp/mmp/MmpMeshPackage.kt
class MmpMeshPackage : TurboReactPackage() {
    override fun getModule(name: String, reactContext: ReactApplicationContext): NativeModule? =
        if (name == MmpMeshModule.NAME) MmpMeshModule(reactContext) else null

    override fun getReactModuleInfoProvider() = ReactModuleInfoProvider {
        mapOf(MmpMeshModule.NAME to ReactModuleInfo(
            MmpMeshModule.NAME, MmpMeshModule.NAME,
            canOverrideExistingModule = false, needsEagerInit = false,
            isCxxModule = false, isTurboModule = true,
        ))
    }
}
```

**Classic `NativeModule` fallback** (pre-New-Architecture apps, or while you're migrating):
drop the `NativeMmpMeshSpec` base class and `extends ReactContextBaseJavaModule` +
`ReactMethod`-annotate each method instead; emit events with
`reactApplicationContext.getJSModule(DeviceEventEmitterModule::class.java).emit("mmp", map)`
in place of the generated `emitOnMmp`. Logic is otherwise identical — the drive-thread
discipline is what matters, not which bridge generation carries the calls.

### 3.3 Foreground service note

The RN module does not own the notification or the `BluetoothGattServer` — `MeshService`
does (`MOBILE_INTEGRATION.md` §4.1). Start `MeshService` from `Application.onCreate()` (or
lazily from `MmpMeshModule.start()`, as above) so relay behavior does not depend on the RN
JS context being alive; bind the module to the running service instance rather than
constructing a second `MmpNode`. If you skip the FG service, call `setTier(LEAF)` — the
same honesty rule applies whether or not RN is in the picture.

## 4. iOS (Swift) native module

```swift
// ios/MmpMesh/MmpMesh.swift
import Foundation
import CoreBluetooth
import React
import UIKit

@objc(MmpMesh)
class MmpMesh: RCTEventEmitter, CBCentralManagerDelegate, CBPeripheralManagerDelegate {

    private var node: MmpNode?
    private let driveQueue = DispatchQueue(label: "mmp.drive")   // single owner, §5
    private var central: CBCentralManager!
    private var peripheral: CBPeripheralManager!
    private var hasListeners = false
    private var driveTimer: DispatchSourceTimer?

    override static func requiresMainQueueSetup() -> Bool { false }
    override func supportedEvents() -> [String] { ["mmp"] }

    override func startObserving() { hasListeners = true }
    override func stopObserving() { hasListeners = false }

    @objc func start(_ tier: String, resolver resolve: @escaping RCTPromiseResolveBlock,
                      rejecter reject: @escaping RCTPromiseRejectBlock) {
        driveQueue.async {
            if self.node == nil {
                let seed = Keychain.loadOrCreateSeed()
                // Bundles (queued/in-mesh, including our own outgoing sends) survive process
                // death via this path; SeenIndex/receipts/ratchet state intentionally do not
                // (doc 04 §7 drop-and-redownload) — see MmpNode's constructor doc comment in
                // mmp-ffi/src/lib.rs.
                let storePath = FileManager.default
                    .urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
                    .appendingPathComponent("mmp").path
                self.node = MmpNode(seed: seed, tier: Self.tierOf(tier), storePath: storePath)
                self.central = CBCentralManager(delegate: self, queue: self.driveQueue,
                    options: [CBCentralManagerOptionRestoreIdentifierKey: "mmp.central"])
                self.peripheral = CBPeripheralManager(delegate: self, queue: self.driveQueue,
                    options: [CBPeripheralManagerOptionRestoreIdentifierKey: "mmp.peripheral"])
                self.startDriveTimer()
                NotificationCenter.default.addObserver(self, selector: #selector(self.appDidEnterBackground),
                    name: UIApplication.didEnterBackgroundNotification, object: nil)
            }
            resolve(self.node!.nodeId().hexString)
        }
    }

    // Natural persistence point (mmp-ffi's `flush()` doc comment): rather than waiting for
    // the next periodic tick()-driven persist.
    @objc private func appDidEnterBackground() {
        driveQueue.async { try? self.node?.flush() }
    }

    @objc func sendText(_ peer: String, text: String,
                         resolver resolve: @escaping RCTPromiseResolveBlock,
                         rejecter reject: @escaping RCTPromiseRejectBlock) {
        driveQueue.async {
            guard let n = self.node else {
                reject("MMP_NOT_STARTED", "call start() first", nil); return
            }
            do {
                let id = try n.sendText(peer: Data(hex: peer), text: text)
                self.drain(n)
                resolve(id.hexString)
            } catch {
                reject("MMP_SEND_FAILED", error.localizedDescription, error)
            }
        }
    }

    // messageState / createGroup / groupInvite / groupSend / groupMembers / groupRemove /
    // setNickname / ownClaims / ingestClaim / block / enableDiagnostics / diagDump /
    // setTier / publicKey / storeLen / flush / lastPersistError: identical
    // "driveQueue.async { ... resolve/reject }" shape, omitted for brevity.

    private func startDriveTimer() {
        let t = DispatchSource.makeTimerSource(queue: driveQueue)
        t.schedule(deadline: .now(), repeating: 1.0)
        t.setEventHandler { [weak self] in
            guard let self, let n = self.node else { return }
            n.tick(nowMs: UInt64(ProcessInfo.processInfo.systemUptime * 1000))
            self.drain(n)
        }
        t.resume()
        driveTimer = t
    }

    private func drain(_ n: MmpNode) {
        while let out = n.pollSend() {
            write(link: out.link, bytes: out.bytes)   // respects canSendWriteWithoutResponse, per MOBILE_INTEGRATION.md §5
        }
        while let ev = n.pollEvent() {
            if hasListeners { sendEvent(withName: "mmp", body: eventToDict(ev)) }
        }
    }

    // CBCentralManagerDelegate / CBPeripheralManagerDelegate callbacks all dispatch on
    // driveQueue (per the `queue:` param above) and call straight into `node`:
    //   didDiscover         → node.onBeacon(serviceData, rssi) — no drain(); beacon ingestion
    //                          only folds into N̂ (contact radar / dial gating), never emits
    //                          an event
    //   didConnect          → node.linkUp(handle(for: peripheral), initiator: true)
    //   didReceiveWrite     → node.linkData(link, request.value); drain(node)
    //   didDisconnect       → node.linkDown(handle); drain(node)
    // See MOBILE_INTEGRATION.md §5 for the full callback-to-call mapping; identical here.
}
```

```objc
// ios/MmpMesh/MmpMesh.m — classic bridge registration (also required alongside the
// TurboModule spec until you've fully migrated codegen for this module).
#import <React/RCTBridgeModule.h>
@interface RCT_EXTERN_MODULE(MmpMesh, RCTEventEmitter)
RCT_EXTERN_METHOD(start:(NSString *)tier
                  resolver:(RCTPromiseResolveBlock)resolve
                  rejecter:(RCTPromiseRejectBlock)reject)
RCT_EXTERN_METHOD(sendText:(NSString *)peer text:(NSString *)text
                  resolver:(RCTPromiseResolveBlock)resolve
                  rejecter:(RCTPromiseRejectBlock)reject)
// ... remaining methods, same pattern
@end
```

### iOS background-BLE caveats (additive to `MOBILE_INTEGRATION.md` §5)

These apply identically whether or not RN is in the stack — they are `CoreBluetooth`
constraints, not bridge constraints — but are easy to rediscover the hard way inside an RN
app, so they're repeated here:

- **Service UUID filter is mandatory in the background.** `CBCentralManager.scanForPeripherals`
  called with `nil`/no `serviceUUIDs` while backgrounded returns nothing; you must scan for
  the MMP service UUID explicitly for background discovery to work at all.
- **Local name is unavailable in background advertisements.** `CBAdvertisementDataLocalNameKey`
  is dropped from adverts made while backgrounded; do not build any UX or peer-labeling logic
  that depends on it being present — use the beacon payload's `ephem_id` (service data)
  instead, exactly as the foreground path does.
- **Background adverts are coalesced.** iOS reduces advertisement frequency and may merge
  multiple adverts before delivering a `didDiscover` callback while backgrounded; do not
  assume a 1:1 timing relationship between the peripheral's advertising interval and your
  central's `didDiscover` rate. The beacon's slot/epoch fields (not wall-clock arrival time)
  are the source of truth for freshness, per doc 03 §5.1.
- **State restoration (`CBCentralManagerOptionRestoreIdentifierKey` /
  `CBPeripheralManagerOptionRestoreIdentifierKey`) is what survives app termination by the
  system.** Without it, an iOS app killed for memory pressure will not be relaunched into
  the background to service an incoming connection; implement
  `centralManager(_:willRestoreState:)` / `peripheralManager(_:willRestoreState:)` to
  re-attach `node` (constructed fresh from the Keychain seed) to the restored
  peripherals/centrals — do not assume `self.node` survives process restart, only the
  Keychain-backed seed and the on-disk store do.

## 5. Threading & lifecycle

**Single-owner rule**: every call into `MmpNode` — from JS-triggered methods, from BLE
delegate callbacks, and from the tick timer — happens on exactly one thread/queue
(`driveHandler`/`HandlerThread` on Android, `driveQueue`/`DispatchQueue` on iOS). The
UniFFI object is documented as internally mutex-guarded (`MOBILE_INTEGRATION.md` §2), which
makes concurrent calls *safe*, but not *correct*: interleaving, say, a `sendText` from a UI
tap with a `linkData` ingest from a GATT callback on two different threads can still produce
event ordering your app doesn't expect (a `Delivered` observed before the `DeliveryProgress:
queued` that logically precedes it, races between `pollSend`/`pollEvent` drains). Route
everything through one queue and this class of bug does not exist.

**Foreground/background transitions**: RN exposes `AppState` — use its `change` event only
to adjust *tick cadence and UI expectations* (e.g. slow the foreground 1 s timer down when
backgrounded on iOS, since you have at most ~30 s foreground execution and then ~10 minutes
of extended background execution before suspension if you requested it — see
`MOBILE_INTEGRATION.md` §5 and the pitfalls table below), never to tear down or recreate
`node`. Reconstructing `MmpNode` on every foreground/background flip discards in-memory
session state for no reason; the store on disk is what makes resume-after-death safe
(`MOBILE_INTEGRATION.md` §4's "resume ≡ reboot" note), not app-state churn. The
`background` transition *is*, however, the right moment to call `node.flush()` — the native
module already does this from its own on-background lifecycle hook (Android
`onHostPause`/`LifecycleEventListener`, iOS `applicationDidEnterBackground` via
`UIApplication.didEnterBackgroundNotification`, §3.2/§4 above) rather than waiting for the
next periodic `tick()`-driven persist, so app code normally does not need to call
`MmpClient.flush()` itself — it exists for the rare case (e.g. "sync now" UI affordance)
where you want a persisted state sooner than either of those.

**Seed storage — Keychain/Keystore, seed IN only**: the 32-byte identity seed is generated
once natively (`Keystore.loadOrCreateSeed()` / `Keychain.loadOrCreateSeed()`, both referenced
above) and handed to `MmpNode`'s constructor. It never flows back out across the FFI — not
in a claim, not in a diagnostic dump, not in an event — and it never crosses the RN bridge in
either direction; JS code has no API that could observe it. This mirrors the secret-boundary
rule in `MOBILE_INTEGRATION.md` §1's ownership table ("Identity secrets at rest... You store
the 32-byte seed; the FFI carries only public keys, ciphertexts, events") and §8's export
rule (exportable only via explicit user action with a strong warning) — RN adds nothing new
here except one more reason to keep the boundary native: an RN JS-layer bug (a stray
`console.log`, a crash reporter that serializes function arguments, a dev-mode remote
debugger) is a more plausible leak path than a Kotlin/Swift one.

**Store persistence**: passing a `storePath` at construction (§3.2/§4 above; `None`/omitted
gives an in-memory-only node, same as before persistence was wired up) backs the node with a
file-backed store under the app's private data directory (`filesDir` on Android, Application
Support on iOS), created and owned by the native module — same as `MOBILE_INTEGRATION.md`
§4/§5's `Node.new(..., storePath, ...)` calls. This persists queued/in-mesh bundles
(including our own outgoing sends) across restart; `SeenIndex`, receipts, and
session/ratchet state intentionally do not survive — the network is the backup, and a
restarted node re-establishes sessions the same way a freshly-met peer would (doc 04 §7).
RN's JS layer never touches the path directly; if you need bundle-count-style diagnostics
(e.g. showing storage usage in a settings screen), use `MmpClient.storeLen()` rather than
exposing a raw path string.

## 6. Expo

**Bare workflow or a custom dev client is required.** MMP needs two things Expo Go cannot
provide: a compiled native module (the Kotlin/Swift code in §3–§4, wrapping the UniFFI
bindings generated from `mmp-ffi`) and unrestricted BLE central+peripheral access
(`react-native-ble-plx`-class permissions are not the blocker here — the *custom native
module* is). Expo Go ships a fixed set of precompiled native modules and cannot load your
`.so`/`.xcframework`; you must either eject to bare workflow or build a custom
[development client](https://docs.expo.dev/develop/development-builds/introduction/)
(`expo-dev-client`) via EAS Build or a local prebuild, so `npx expo run:android` / `run:ios`
produce a binary containing `mmp-ffi`.

Config-plugin sketch (`plugins/withMmpMesh.js`), assuming a bare/prebuild project — this
plugin's job is to make `npx expo prebuild` regenerate the native projects with the
permissions and Podfile/Gradle hooks MMP needs, so you don't hand-edit `AndroidManifest.xml`
/ `Info.plist` after every prebuild:

```js
const { withAndroidManifest, withInfoPlist, withPodfile } = require('@expo/config-plugins');

function withMmpMeshAndroid(config) {
  return withAndroidManifest(config, (config) => {
    const manifest = config.modResults.manifest;
    const perms = [
      'android.permission.BLUETOOTH_SCAN',
      'android.permission.BLUETOOTH_ADVERTISE',
      'android.permission.BLUETOOTH_CONNECT',
      'android.permission.FOREGROUND_SERVICE',
      'android.permission.FOREGROUND_SERVICE_CONNECTED_DEVICE',
    ];
    // ... add <uses-permission> entries + <service android:name=".mmp.MeshService"
    //     android:foregroundServiceType="connectedDevice"/> to the manifest
    return config;
  });
}

function withMmpMeshIOS(config) {
  config = withInfoPlist(config, (config) => {
    config.modResults.NSBluetoothAlwaysUsageDescription =
      'Used to relay messages with nearby devices over Bluetooth mesh, with no internet required.';
    config.modResults.UIBackgroundModes = [
      ...(config.modResults.UIBackgroundModes || []),
      'bluetooth-central',
      'bluetooth-peripheral',
    ];
    return config;
  });
  return withPodfile(config, (config) => {
    // Ensure the Podfile links MmpCore.xcframework (built per MOBILE_INTEGRATION.md §3)
    // — typically via a `pod 'MmpCore', :path => '../MmpCore'` podspec you author once.
    return config;
  });
}

module.exports = (config) => withMmpMeshIOS(withMmpMeshAndroid(config));
```

`app.json`/`app.config.js`:

```json
{ "expo": { "plugins": ["./plugins/withMmpMesh"] } }
```

This gets you native-project regeneration on every `expo prebuild`; it does not, by itself,
produce the compiled `mmp-ffi` artifacts — those are still built via the `cargo
ndk`/`cargo build --target aarch64-apple-ios` steps in `MOBILE_INTEGRATION.md` §3 and wired
into the Gradle/CocoaPods build as a pre-build step (an `eas-build-pre-install` hook, or a
Gradle task / Xcode Run Script phase, depending on your CI).

## 7. Worked example: send a message to a nearby peer

App code — everything a product engineer needs to write:

```ts
import { mmp } from '../mmp/MmpClient';

async function greetNearbyPeer(peerHex: string) {
  const unsub = mmp.onEvent((ev) => {
    if (ev.type === 'delivered') {
      console.log('delivered:', ev.id);
    }
  });

  await mmp.start('leaf');
  const msgId = await mmp.sendText(peerHex, 'hey — found you on the mesh');
  console.log('queued as', msgId);

  // later, e.g. from a status pill:
  const state = await mmp.messageState(msgId); // 'queued' | 'inMesh' | 'delivered' | 'expired'

  unsub();
}
```

What happens underneath (native side, all off the JS thread):

```
JS: mmp.sendText(peer, "hey…")
  │
  ▼ (bridge call → driveHandler/driveQueue)
Kotlin/Swift: node.sendText(peer, text)
  │  Rust (mmp-node): resolves peer's exchange/prekey claim, runs X3DH if this is the
  │  first contact (send_app → sender_agree), encrypts, signs, stores the bundle
  │  locally with a spray budget. Emits Event::DeliveryProgress{Queued}. Returns msg_id.
  ▼
[wait for radio contact — could be milliseconds if already linked, or minutes/hours
 in DTN store-and-forward if the peer isn't nearby yet]
  │
  ▼ BLE (native only, never JS):
  scan → beacon match → novelty×RSSI dial gate → GATT connect
    → node.linkUp(link, initiator) → session handshake frames via node.pollSend()/
      node.linkData() → SessionEvent::Established → Event::Established/ContactSeen
    → sync round (Hello/Summary/offers) exchanges the stored bundle
    → peer's node ingests it (on_bundle → deliver_direct → decrypts, DAG-inserts)
      → peer emits Event::MessageReceived, and — because FLAG_WANTS_RECEIPT was set —
        emits a signed receipt bundle back into the mesh
    → receipt bundle eventually reaches us (possibly relayed, possibly direct)
      → handle_receipt verifies the conversation MAC → Event::Delivered{id} +
        Event::DeliveryProgress{Delivered}
  │
  ▼ (drain loop: pollEvent → emit("mmp", …) → NativeEventEmitter)
JS: mmp.onEvent listener fires with { type: 'delivered', id: msgId }
```

Note the gap between "app code returns" and "Delivered fires" can be anywhere from
sub-second (peer already linked) to the message's TTL (peer reachable only via relay,
`LIFETIME_MESSAGE_MS`) — build UI around `DeliveryProgress` states, not around
`sendText`'s promise resolving, which only means "accepted into the local store."

## 8. Testing

**Rust core**: already covered by `cargo test` across `mmp-core`, `mmp-crypto`, `mmp-router`,
`mmp-node` (protocol logic, crypto correctness, DAG ordering, quota eviction, etc.). RN-side
tests should not re-verify protocol behavior — they verify the *bridge*: that the TS API
calls the right native method with correctly-encoded arguments, and that native events
decode into the right `MmpEvent` shapes.

**Jest — mock the native module**, don't try to load the real `.so`/`.xcframework` in a
Node test environment:

```ts
// __mocks__/react-native.ts (or a per-test jest.mock)
jest.mock('react-native', () => {
  const actual = jest.requireActual('react-native');
  const listeners: Record<string, ((ev: any) => void)[]> = {};
  return {
    ...actual,
    NativeModules: {
      ...actual.NativeModules,
      MmpMesh: {
        start: jest.fn().mockResolvedValue('a'.repeat(32)),
        sendText: jest.fn().mockResolvedValue('b'.repeat(32)),
        messageState: jest.fn().mockResolvedValue('queued'),
        addListener: jest.fn(),
        removeListeners: jest.fn(),
      },
    },
    NativeEventEmitter: jest.fn().mockImplementation(() => ({
      addListener: (name: string, cb: (ev: any) => void) => {
        (listeners[name] ??= []).push(cb);
        return { remove: jest.fn() };
      },
    })),
  };
});

test('sendText resolves a hex msg id', async () => {
  const { mmp } = require('../src/mmp/MmpClient');
  const id = await mmp.sendText('c'.repeat(32), 'hi');
  expect(id).toMatch(/^[0-9a-f]{32}$/);
});
```

Drive fake `MmpEvent`s through the mocked emitter to test reducers/store logic
(`ev.type === 'delivered'` → flip a message to ✓✓, etc.) without any native code involved.

**Detox / device-farm**: reserve for what Jest fundamentally cannot exercise — real BLE
discovery and pairing between two physical (or two emulator/simulator, where BLE is
unsupported on most simulators/emulators and this tier is often skipped in favor of physical
device farms) devices. Detox drives the *app*, not the mesh directly; a realistic e2e is "two
devices, app A sends via the UI, assert app B's UI shows the message" — treat it as an
integration smoke test layered on top of the protocol-level integration matrix already in
`MOBILE_INTEGRATION.md` §9 (phone↔PC anchor, phone↔phone via relay, Doze soak, OEM kill
matrix, iOS background dial-in), not a replacement for it.

## 9. Pitfalls

| Pitfall | Why it bites | Mitigation |
|---|---|---|
| Shuttling BLE bytes through the JS bridge | Per-chunk bridge round-trips (JSON serialize on classic bridge, or JSI marshaling even on New Architecture) at GATT-write frequency destroy latency and burn battery; the JS thread also contends with rendering | **Forbidden by design (§1).** Bytes go native GATT callback → `node.linkData()` directly; JS only sees `pollEvent()` output |
| Android Doze vs. scan intervals | Doze/App Standby throttle scan results and background execution windows; a naive `setInterval`-style JS timer for ticking stops firing entirely once JS is suspended | Tick from `AlarmManager.setAndAllowWhileIdle` inside `MeshService` (native, already covered in `MOBILE_INTEGRATION.md` §4), *not* from any JS-side timer; RN's own JS timers are unreliable once the app is backgrounded regardless of Doze |
| iOS ~10-minute background execution budget | Even with `bluetooth-central`/`bluetooth-peripheral` background modes and state restoration, iOS gives limited background execution time (typically capped around 3 minutes for a plain background task, extendable via BLE-specific background modes, but not indefinite); iOS can also suspend the process between BLE events | Design the leaf role (`MOBILE_INTEGRATION.md` §5) to tolerate suspension: rely on state restoration to reattach `node` rather than assuming the timer/queue survives; do not promise "always relaying" in iOS background UI copy |
| MTU negotiation failures | Some peripherals/centrals silently keep the 23-byte ATT MTU floor despite a 247 request (doc 09 §5.1); code that assumes a fixed post-negotiation chunk size will corrupt fragmentation | The core doesn't care — it reassembles by length prefix, not fragment count (doc 09 §5.2) — but your native chunking code must always chunk to the *negotiated* MTU−3, read from the platform API after negotiation completes, never a hardcoded 244 |
| Hermes / ArrayBuffer conversions | Base64 string ⇄ `Uint8Array`/`ArrayBuffer` conversion has real CPU cost on Hermes, and TurboModule codegen support for passing raw `ArrayBuffer` varies by RN version (§2) — assuming it "just works" on an unverified RN version can silently fall back to slow paths or fail codegen | Benchmark base64 codec cost for your actual payload sizes (claims, diagnostic dumps) before optimizing; treat direct `ArrayBuffer` passing as a version-gated optimization, not a baseline assumption |
| Recreating `MmpNode` on `AppState` transitions | Discards in-session state for no protocol benefit and risks racing the drive thread against a half-torn-down node | Only adjust tick cadence on foreground/background; never reconstruct `node` outside of process restart (§5) |
| Calling `MmpNode` methods from more than one thread/queue | The UniFFI object being mutex-guarded makes this *safe*, not *correct* — event ordering guarantees assume single-threaded access | Route every call (JS-triggered, BLE-callback-triggered, timer-triggered) through the one drive thread/queue (§5) |
