# MMP/1 Mobile Integration Guide

**How to embed the mesh_milk protocol as a transport in mobile (and desktop) applications.**
Audience: app engineers integrating the Rust core on Android, iOS, React Native, Flutter, Kotlin Multiplatform, and PC.
Companion documents: `other/HANDOFF_MOBILE.md` (product/UX duties), `specification/12-rust-blueprint.md` §5–§6 (normative API surface), `specification/09-transport-ble.md` (BLE mapping).

---

## 1. The one architectural rule

The Rust core is **sans-IO**. It owns *protocol truth* — identity, crypto, bundles, routing, sync, storage decisions — and nothing else. Your platform shell owns three things the core never touches:

| The platform owns | Because | Core-side seam |
|---|---|---|
| **The radio** | BLE APIs are irreducibly platform-specific | You feed `TalEvent`s in, write link bytes out |
| **The clock & wakeups** | Doze/App Refresh quantize timers; the core must not pretend otherwise | Every call takes monotonic `now_ms`; the core never sleeps |
| **Identity secrets at rest** | Android Keystore / iOS Keychain are the only honest vaults | You store the 32-byte seed; the FFI carries only public keys, ciphertexts, events |

If you keep these three inversions intact, the same core binary runs identically under tokio on a PC, inside an Android foreground service, in an iOS background session, and in the deterministic simulator that CI uses. If you break any of them (e.g. spawn a thread in Rust that sleeps), Doze will break you back.

## 2. The drive loop (identical on every platform)

The whole integration is four calls in a loop. Pseudocode:

```
node = Node.new(seed from keystore, store path, tier)

loop {
    // 1. Feed radio reality in
    for event in radio_events:            // scan hit / link up / bytes / link down
        node.on_tal_event(event)

    // 2. Flush protocol output to the radio
    while (link, bytes) = node.poll_send():
        ble.write(link, bytes)             // fragment to MTU; order-preserving

    // 3. Surface protocol events to the UI
    while event = node.poll_event():
        event_router.dispatch(event)       // MessageReceived, Delivered, …

    // 4. Advance time (on wakeup / alarm / foreground pulse)
    node.tick(monotonic_now_ms)
}
```

There is no callback re-entrancy, no threading contract beyond "one caller at a time" (the FFI object is internally mutex-guarded), and no hidden timers. **Every** behavior — session timeouts, TTL expiry, filter rotation, quota eviction — happens inside `tick(now_ms)` with the time *you* provide. Timer quantization to OS alarm granularity is tolerated by design (doc 11 §7); session timers only matter while a link is up, i.e. while you're awake anyway.

### The event stream is the app

Native bindings expose the coarse `FfiError` enum, including `ConversationPending`: after a
first-contact asynchronous send, wait for the authenticated receipt (or a corresponding
`DeliveryProgress` event) before retrying another direct body for that peer. This is a normal
conversation-establishment state, not a malformed argument or a reason to create duplicate
bundles.

Every screen is a projection of `poll_event()` (see HANDOFF_MOBILE §2 for the full UI-duty table):

| Event | Meaning | Minimum UI duty |
|---|---|---|
| `MessageReceived{conv, from, id, body, apptype}` | decrypted, DAG-ordered inbound message | insert by DAG order, notify |
| `MessageGap{conv, parents}` | a causal parent is still in transit | gap marker; auto-repairs via WANT — never an error |
| `Delivered{id}` | **cryptographic** delivery proof (receipt MAC verified) | flip to ✓✓ |
| `DeliveryProgress{id, state}` | Queued → InMesh → Delivered / Expired | status glyphs (HANDOFF_MOBILE §3) |
| `ClaimUpdated{owner, field}` | nickname/avatar/key claim adopted | refresh profile |
| `CompromiseWarning{owner}` | equal-seq conflicting claims — possible key compromise | **normative**: non-dismissable banner (doc 07 §6.4) |
| `PeerRevoked{peer, successor}` | identity tombstone adopted; successor is an optional 32-byte public key | refuse new sessions and offer explicit trust-successor UX |
| `ContactSeen / Established{peer}` | live mesh activity | "nearby" indicators, mesh screen |
| `GroupInvite / GroupMessage / GroupEpochChanged` | group plane (`GroupMessage` includes `apptype`) | show "removal applies as the network reconverges" (doc 06 §8.6) |
| `TierChanged{tier}` | relay ladder moved | mesh status chip |
| `Announcement{id, scope, annType, notAfter, text, from, official, effectivePriority, scopeNameHint}` | subscribed class-5 announcement | render only after applying `effectivePriority`; `scopeNameHint` is display-only |
| `FileOffered / FileReady / FileFailed` | verified file offer, completed pull, or abandoned/hash-failed assembly | offer explicit pull; never auto-fetch a full file |

## 3. Building the core

Rust targets (all tier-1/tier-2, stable toolchain):

```sh
# Android (via cargo-ndk; produces .so per ABI)
cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 -o app/src/main/jniLibs build -p mmp-ffi --release

# iOS (device + simulator, then wrap as XCFramework)
cargo build -p mmp-ffi --release --target aarch64-apple-ios
cargo build -p mmp-ffi --release --target aarch64-apple-ios-sim
xcodebuild -create-xcframework -library ... -output MmpCore.xcframework

# Desktop (Linux/macOS/Windows)
cargo build -p mmp-ffi --release
```

**docs/AGILE_BACKLOG.md E2-S5** turns the Android and iOS steps above into real,
one-command-reproducible artifacts, and CI (`.github/workflows/ci.yml`'s
`android-aar` / `ios-xcframework` jobs) produces them on every push:

```sh
# Android: mmp-ffi.aar (native-only shape — jni/<abi>/*.so + a minimal
# manifest, no compiled classes.jar; see the script's header comment for why)
# plus the Kotlin bindings as a source zip alongside it. Needs cargo-ndk and
# an installed NDK (ANDROID_NDK_HOME) — not installed by the script itself.
scripts/build-android-aar.sh

# iOS: MmpCore.xcframework (device + Apple-Silicon-simulator) plus the Swift
# bindings source. macOS + Xcode only.
scripts/build-ios-xcframework.sh
```

Neither script has been run end-to-end outside CI as of this writing — this
development box has neither an Android NDK nor Xcode (categorically
unavailable outside macOS) — so the first real execution in CI is the actual
verification, same as this repo's other toolchain/hardware-gated stories
(e.g. E3-S2's real BLE exchange). The scripts were written directly against
the exact commands already documented above, so drift risk is limited to the
new packaging steps (AAR zip layout, XCFramework header/modulemap assembly).

Bindings are generated by **UniFFI** from the `mmp-ffi` crate — one definition produces Kotlin and Swift with matching types:

```sh
cargo run -p mmp-ffi --features cli --release --locked --bin uniffi-bindgen -- generate --library target/release/libmmp_ffi.so \
    --language kotlin --out-dir bindings/kotlin
cargo run -p mmp-ffi --features cli --release --locked --bin uniffi-bindgen -- generate --library target/release/libmmp_ffi.dylib \
    --language swift --out-dir bindings/swift
```

Before packaging, the source-level binding gates must pass for both generated
trees:

```sh
python scripts/check_mobile_api_surface.py bindings/kotlin bindings/swift
python scripts/check_mobile_binding.py kotlin bindings/kotlin
python scripts/check_mobile_binding.py swift bindings/swift
```

These checks cover the complete exported `MmpNode` method/throws surface and
all public FFI records/enums (including the Kotlin `FfiException` spelling for
the Rust `FfiError`), plus the secret-state denylist. They deliberately do not
claim Kotlin/Swift compilation, native radio behavior, or hardware
interoperability; those remain CI, platform-toolchain, and device gates.

The exported surface (stable across languages):

```
MmpNode(seed: bytes32, tier: Tier, storePath: string?)
  // storePath: null => in-memory MemStore, nothing survives process death.
  // Some(path) => file-backed SnapshotStore. Bundles, claims, group membership,
  // SeenIndex, ReceiptFilter, authority admission state, and the explicit
  // blocklist survive restart. Secret session/ratchet/prekey state (including
  // sender-key material) still requires a platform-keystore provider and is
  // not persisted by this mobile FFI path; a restarted node re-establishes
  // sessions and re-syncs that state through normal protocol traffic.
  // Persisted periodically by tick() and on demand by flush().
  .nodeId() -> bytes16              // BLAKE3(identity_pk)[0..16]
  .publicKey() -> bytes32
  .ownClaims() -> List<bytes>       // for QR verification / bootstrap exchange
  .ingestClaim(claim: bytes, verified: bool)
  .setNickname(name: string, seq: u32)
  .nicknameOf(peer: bytes16) -> string?            // locally verified display nickname
  .revokeIdentity(reason: u8) -> bytes
  .tryRevokeIdentity(reason: u8) -> bytes                    // throws FfiError; preferred checked form
  .revokeIdentityWithSuccessor(reason: u8, successor: bytes32, countersig: bytes64) -> bytes
                                                               // throws FfiError
  .sendText(peer: bytes16, text: string) -> bytes16          // BundleID to track
  .sendBody(peer: bytes16, apptype: u8, payload: bytes) -> bytes16
  .messageState(id: bytes16) -> FfiDeliveryState?
  .createGroup() -> bytes16
  .groupInvite(gid: bytes16, peer: bytes16)                     // throws FfiError; no return
  .groupSend(gid: bytes16, text: string) -> bytes16             // throws FfiError; BundleID
  .groupRemove(gid: bytes16, member: bytes16) -> u16            // throws FfiError; new epoch
  .groupSendBody(gid, apptype: u8, payload: bytes) -> bytes16
  .groupMembers(gid) -> List<bytes16>
  .requestFriendship(peerPublicKey: bytes32) / .allowFriendshipFrom(peerPublicKey: bytes32)
  .setWallClock(unixS: u32)
  .pinAuthorityRoot(scope: bytes16, issuer: bytes32)
  .subscribeScope(scopeName: string) -> bytes16
  .unsubscribeScope(scope: bytes16)
  .mintAuthCert(suite, subject, scope, caps, priorityBudget, notAfter) -> bytes
  .publishAnnouncement(scope, annType, priority, text, cert?) -> bytes16
  .beacon(nowMs: u64) -> bytes24    // advertise this payload
  .onBeacon(payload: bytes, rssi: i8) // scanned advertisement in; folds into
                                     // the N̂ population estimate (contact
                                     // radar / dial gating) — no nowMs, time
                                     // is driven by tick()
  .linkUp(link: u64, initiator: bool)
  .linkUpWithQuality(link: u64, initiator: bool, quality: FfiLinkQuality)
  .linkData(link: u64, bytes: bytes)
  .linkDown(link: u64)
  .pollSend() -> Option<(u64, bytes)>
  .pollSendBatch(maxItems: u32) -> List<OutFrame>       // FIFO, hard cap 64
  .pollClose() -> Option<u64>                           // host closes link, then linkDown
  .pollCloseBatch(maxItems: u32) -> List<u64>
  .pollEvent() -> Option<Event>     // tagged enum, per §2 table
  .pollEventBatch(maxItems: u32) -> List<Event>          // FIFO, hard cap 64
  .closeActionsDropped() -> u64
  .eventQueueDropped() -> u64
  .tick(nowMs: u64)                 // also drives the periodic persist when storePath is set
  .setTier(tier)
  .enableDiagnostics() / .disableDiagnostics()
  .status() -> FfiNodeStatus / .capabilities() -> FfiCapabilities
  .metrics() -> FfiMetrics / .betaSnapshot() -> FfiBetaSnapshot
  .diagDump() -> string
  .block(peer: bytes16)
  .sendFile(peer, name, mime, data: bytes) -> FfiFileSend
  .fetchFile(root: bytes16, dest: string)
  .cancelFetch(root: bytes16)
  .fileProgress(root: bytes16) -> FfiFileProgress?
  .storeLen() -> u64                // bundles currently held in the store
  .flush()                          // force an immediate persist; call from the platform's
                                     // on-background lifecycle hook (Android onStop / iOS
                                     // applicationDidEnterBackground) rather than waiting for
                                     // the next periodic tick()-driven persist
  .lastPersistError() -> string?    // Display of the most recent persist failure, if any

MmpNode.newWithStoreLimit(seed: bytes32, tier: Tier, storePath: string, maxStoreBytes: u64)
  -> MmpNode                         // throws FfiError; maxStoreBytes must be nonzero
```

## 4. Android (Kotlin) — the relay workhorse

### 4.1 Process model

A **foreground service** with a persistent notification is the only honest way to be a T2 relay (doc 04 §5). The service owns: the `BluetoothLeScanner` (filtered on the MMP service UUID — filtered scans survive background throttling), the `BluetoothGattServer` (peripheral role), the advertiser, and the node drive loop. Without the FG service, call `node.setTier(FfiTier.LEAF)` — never pretend.

### 4.2 GATT mapping (doc 09 §5.1)

* One primary service (128-bit dev UUID until SIG assignment — OQ-12). **Single source of truth
  (E3-S3):** `mmp_tal::DEV_SERVICE_UUID` (`implementation/mesh-milk/crates/mmp-tal/src/lib.rs`) —
  both the central (`mmp-tal-btleplug`) and peripheral (`mmp-tal-peripheral`) Rust TAL crates
  re-export it rather than each carrying their own literal, so this doc quotes the same values they
  do, not an independently-hand-copied one:
  * Service: `6d6d7031-0000-4d4d-5031-6d6573686d6c`
  * TX (client→server, Write Without Response): `6d6d7031-0001-4d4d-5031-6d6573686d6c`
  * RX (server→client, Notify): `6d6d7031-0002-4d4d-5031-6d6573686d6c`
* **TX characteristic**: client writes with *Write Without Response*; feed each write into `node.linkData(link, bytes)`. The core reassembles frames internally with a hard 4 KiB bound — you never buffer.
* **RX characteristic**: *Notify*; drain `node.pollSend()` into ≤(MTU−3)-byte notifications, in order.
* Request MTU 247+, 2M PHY, DLE; proceed gracefully on 23-byte floors — the core doesn't care, only throughput does.
* `link` id is any u64 you mint per connection (e.g. connection handle); call `linkDown` on disconnect — no other cleanup exists or is needed.

```kotlin
class MeshService : Service() {
    private lateinit var node: MmpNode
    private val main = Handler(Looper.getMainLooper())

    override fun onCreate() {
        startForeground(NOTIF_ID, meshNotification())
        node = MmpNode(Keystore.loadOrCreateSeed(), FfiTier.RELAY, filesDir.resolve("mmp").path)
        startAdvertising(node.beacon(now()))
        startFilteredScan()          // ScanFilter on MMP_SERVICE_UUID
        pump()
    }

    private fun pump() {
        node.tick(SystemClock.elapsedRealtime())          // monotonic, Doze-honest
        while (true) {
            val out = node.pollSend() ?: break
            gatt.notifyFragmented(out.link, out.bytes)    // chunk to MTU-3
        }
        while (true) {
            val ev = node.pollEvent() ?: break
            EventRouter.dispatch(ev)                       // one router, many stores
        }
        main.postDelayed(::pump, if (hasLiveLinks) 30 else 2_000)
    }

    fun onScanResult(result: ScanResult) {
        result.scanRecord?.serviceData?.get(MMP_UUID)?.let {
            node.onBeacon(it, result.rssi.toByte())
        }
        maybeDial(result)                                  // novelty × RSSI gate (doc 04 §6)
    }
}
```

Platform duties checklist:
- `SystemClock.elapsedRealtime()` for **all** `nowMs` (monotonic; wall clock never deletes data — doc 03 §4.3).
- Scan-throttle compliance: ≤5 scan start/stops per 30 s; use batched callbacks.
- `AlarmManager.setAndAllowWhileIdle` for the background heartbeat tick (~15 min in Doze is fine; TTLs are hours).
- Battery-optimization exemption with an honest explanation screen; surface relay state in the notification (OEM kill risk is real — AUDIT F13).
- Identity seed in **Android Keystore**-wrapped storage; the seed never crosses the FFI after construction.
- On service death: re-create `Node` from the same seed + store path — resume ≡ reboot by design (crash-consistent store).

## 5. iOS (Swift) — the honest leaf

Background iOS cannot relay; design for **leaf + foreground bursts** (doc 04, HANDOFF_MOBILE §1).

* **State restoration is mandatory** (`CBCentralManagerOptionRestoreIdentifierKey`): it is what lets an Android/PC anchor dial a pocketed iPhone.
* Background: scan for the MMP service UUID only (works via overflow area), stay a connectable peripheral publishing the TX/RX characteristics. Advertise the beacon in service data; accept that backgrounded advertising moves the UUID to the overflow area — Android anchors compensate (doc 09 §6).
* Foreground: full participation — burst scan, dial, advertise. Make app-open trigger a visible sync pulse ("open app = pump the mesh").

```swift
final class MeshController: NSObject, CBPeripheralManagerDelegate, CBCentralManagerDelegate {
    private var node: MmpNode!

    func start() {
        node = MmpNode(seed: Keychain.loadOrCreateSeed(),
                       tier: .leaf, storePath: appSupportURL.path)
        central = CBCentralManager(delegate: self, queue: .main,
            options: [CBCentralManagerOptionRestoreIdentifierKey: "mmp.central"])
        peripheral = CBPeripheralManager(delegate: self, queue: .main,
            options: [CBPeripheralManagerOptionRestoreIdentifierKey: "mmp.peripheral"])
    }

    // Central: didDiscover → node.onBeacon(serviceData, rssi)
    // didConnect → node.linkUp(handle(for: peripheral), initiator: true)
    // characteristic value updates → node.linkData(...) ; disconnect → node.linkDown(...)

    func pump() {
        node.tick(UInt64(ProcessInfo.processInfo.systemUptime * 1000))   // monotonic
        while let out = node.pollSend() { write(out.link, out.bytes) }    // respects canSendWriteWithoutResponse
        while let ev = node.pollEvent() { EventRouter.shared.dispatch(ev) }
    }
}
```

Platform duties checklist:
- `ProcessInfo.systemUptime` (or `DispatchTime`) for `nowMs` — never `Date()`.
- Identity seed in **Keychain** (`kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly`).
- Respect `canSendWriteWithoutResponse` / `updateValue == false` back-pressure: park the outbox and resume on the ready callback — the core's queue is pull-based precisely for this.
- No relay promises in UI copy on iOS. It is a leaf; pretending otherwise breaks user trust.

## 6. Cross-platform frameworks

The pattern is always the same: **the Rust core + the native BLE layer are shared infrastructure; the framework only consumes the event stream.**

### React Native
Use `uniffi-bindgen-react-native` to generate a TS-typed TurboModule directly from `mmp-ffi`, or hand-wrap the Kotlin/Swift layers above in a native module. Keep the BLE plumbing **in the native layer** (the Android FG service must exist regardless of RN); expose only `sendText/…` + an event emitter to JS:

```ts
const mesh = NativeModules.MmpMesh;
const emitter = new NativeEventEmitter(mesh);
emitter.addListener('mmp', (ev: MmpEvent) => store.dispatch(route(ev)));
await mesh.sendText(peerIdHex, 'hello');
```
Do **not** attempt BLE from JS (react-native-ble-plx etc.) for the mesh path — background execution and scan filtering will not survive.

### Flutter
Same shape with `flutter_rust_bridge` (bind Rust directly) or a `MethodChannel`/`EventChannel` over the native shells. The Dart side sees a typed event stream; BLE stays native. If you bind Rust directly, you still need the Kotlin FG service / Swift restoration shells for the radio — FRB replaces UniFFI, not the platform duties.

### Kotlin Multiplatform
`mmp-ffi`'s Kotlin bindings work as the `androidMain` implementation; `iosMain` consumes the Swift/ObjC XCFramework via cinterop. The common module defines the `MeshTransport` interface mirroring §3's surface; view-models live in commonMain and only ever see events.

### Desktop / PC anchor
`mmp-cli` (this workspace) is the reference host: same `Node`, `btleplug`-based TAL (Linux/macOS/Windows). PCs should run `FfiTier.ANCHOR` — always-on relays with a 2 GiB quota that dramatically improve neighborhood delivery.

## 7. Using MMP as a *transport* (not just a chat app)

Class-1 bodies are `apptype(u8) ‖ payload` inside the AEAD (doc 16 §3.1). `0x00` = text, `0x01` = FileManifest, `0x02` = reserved (sender keys). **`0x10`–`0xFF` are yours**: put CBOR/protobuf app messages behind your own apptype and you get a store-carry-forward, end-to-end-encrypted, delivery-receipted datagram service:

```kotlin
node.sendBody(peer, apptype = 0x42, payload = cbor(gameMove))   // ≤ ~3.8 KiB per bundle
```

Rules of the road: bundles are ≤4096 B (larger content → blob plane); latency is minutes-tier by design — build request/response as async state machines, not RPC; `Delivered` is cryptographic proof the *device* got it, not that a human saw it; per-origin quotas (32 KiB/h through strangers) bound your throughput — this is a messaging fabric, not a pipe.

## 8. Security duties you cannot skip (doc 07 §6 — normative)

1. **Verification ceremony**: QR-exchange of identity keys (`ownClaims()` → `ingestClaim(verified = true)`); verified state must be visually unmistakable vs TOFU.
2. **CompromiseWarning** must surface non-dismissably for the user's own identity.
3. **Blocking is silent** — no UI copy may suggest the blocked party learns anything.
4. Seed handling: generated once, stored in the platform vault, exportable only with explicit user action and strong warnings ("the key *is* the identity; there is no recovery").
5. Strip EXIF/GPS from media *before* it enters a manifest; generate thumbnails yourself, deterministically (doc 16 §5.2).

## 9. Integration test matrix (before you ship)

| Test | Pass criterion |
|---|---|
| Phone ↔ PC anchor first contact | handshake < 5 s, message + ✓✓ receipt round-trip |
| Phone ↔ phone via anchor relay | delivery with both phones never co-present |
| Android Doze soak (24 h) | node resumes on alarm ticks; no data loss (resume ≡ reboot) |
| OEM kill matrix (doc 14 D-7) | FG service death → honest Leaf tier + user-visible state |
| iOS background dial-in | pocketed iPhone reachable via state restoration |
| Battery (T2, synthetic neighborhood) | < 5 %/day background |
| Store corruption injection | corrupt records dropped, node keeps running (network is the backup) |
