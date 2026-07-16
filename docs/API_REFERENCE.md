# MMP/1 — Mobile Library API Reference

**Scope.** This document is the stable contract for `mmp-ffi` (crate at
`implementation/mesh-milk/crates/mmp-ffi`) — the UniFFI 0.29 surface that Kotlin, Swift, and
(through them) React Native consume. It is the single source of truth for the API shape; where
`MOBILE_INTEGRATION.md` or `REACT_NATIVE_INTEGRATION.md` show code samples, they must match this
document. If they ever disagree, this document and the crate's `src/lib.rs` are authoritative —
open a doc-fix task, don't trust the sample.

**Audience.** Mobile app engineers consuming the library, and binding-generation/CI maintainers.
Not a tutorial — see `MOBILE_INTEGRATION.md` (platform integration walkthrough) and
`REACT_NATIVE_INTEGRATION.md` (RN-specific bridge code) for that. This document is the reference
you check when you need the exact signature, the exact error semantics, or what a field actually
means.

**Versioning.** This surface has no version number yet (pre-1.0, single implementation). Once a
second consumer (a shipped mobile app) depends on it, adopt semver on the `mmp-ffi` crate
specifically and note breaking changes here under a "Changelog" section to be added at that point.

---

## 1. The one rule this crate exists to enforce

The mobile platform owns the BLE radio and the clock. `mmp-ffi` owns nothing but a thin,
mutex-guarded shim around the sans-IO protocol core. **There are no threads, no timers, no radio
code inside this crate.** Every side effect is driven by an explicit host call:

- inject a transport event in (`link_up` / `link_data` / `link_down` / `on_beacon`),
- poll wire bytes or an app event out (`poll_send` / `poll_event`),
- advance time explicitly (`tick(now_ms)`).

If you are integrating this library and find yourself wanting the library to "just handle" BLE or
timing internally, that is a sign you're fighting the design — the host's drive loop (§5) is not
optional glue, it *is* the integration.

## 2. Secret boundary

The 32-byte identity seed crosses the FFI **exactly once, inward, at `MmpNode.new()`**. Store it
in Android Keystore / iOS Keychain on the host side; the library never asks for it again and never
returns it, or anything derived from it besides the public identity key, back across the boundary.
Every other method returns public keys, NodeIDs, ciphertext-free app events, or wire bytes that
are already encrypted by the core before `poll_send()` returns them.

## 3. Threading contract

`MmpNode` is internally mutex-guarded (`Mutex<mmp_node::Node>`), so calls are safe to make from
any thread — but there is no *ordering* contract beyond mutual exclusion. **Convention: one
dedicated thread/queue owns all calls into a given `MmpNode` instance** (a single background
HandlerThread on Android, a single serial dispatch queue on iOS — see `MOBILE_INTEGRATION.md` §2
and `REACT_NATIVE_INTEGRATION.md` §5 for the platform-specific pattern). This isn't enforced by
the type system; it's a design invariant because interleaved calls from multiple threads make the
event ordering the host observes unpredictable, even though nothing will crash or corrupt state.

## 4. Type reference

### 4.1 ID encoding

Every `NodeId` / `Id16` (peer ID, message ID, group ID, conversation ID) crosses the FFI as a
`Vec<u8>` of **exactly 16 bytes**. A wrong length is rejected with `FfiError::BadArg` — never
silently truncated or padded. The public identity key (`public_key()`) is a `Vec<u8>` of exactly
32 bytes. Recommended host-side representation: lowercase hex string (stable `Map`/dictionary
key, human-diffable in logs) — see `REACT_NATIVE_INTEGRATION.md` §2 for the full encoding
convention table including variable-length fields.

### 4.2 `FfiError`

```
enum FfiError { BadArg, UnknownPeer, Internal }
```
Deliberately coarse — the host app cannot act differently on finer-grained core error variants,
and none of the failure reasons are secret, so a three-way enum keeps generated bindings simple.

| Variant | Meaning | Typical cause |
|---|---|---|
| `BadArg` | A byte slice had the wrong length, or a claim/body failed to decode | wrong-length ID, corrupt claim bytes, oversized send |
| `UnknownPeer` | No exchange key / prekey batch on file for this peer yet | tried to message someone before ingesting their claims |
| `Internal` | A core invariant was violated | should not happen; report as a bug |

### 4.3 `FfiTier`

```
enum FfiTier { Leaf, Relay, Anchor }
```
Mirrors the core `Tier` (routing/storage posture — see `docs/IMPLEMENTATION_HANDBOOK.md` §2.5 and
§4.9 for what each tier means operationally). Set at construction, changeable at runtime via
`set_tier()` — e.g. drop to `Leaf` on low battery, promote to `Anchor` while charging and on
Wi-Fi/mains power.

### 4.4 `FfiDeliveryState`

```
enum FfiDeliveryState { Queued, InMesh, Delivered, Expired }
```
Lifecycle of a message *you sent*, returned by `message_state()` and mirrored in
`FfiEvent::DeliveryProgress`. `Queued` → not yet handed to any carrier. `InMesh` → at least one
copy has left your device. `Delivered` → a cryptographic receipt came back (see handbook §3.7 for
the game-design explanation of what this represents; in the real protocol it's an MAC only the
true recipient's conversation key can produce). `Expired` → all copies' TTL ran out unread.

### 4.5 `OutFrame`

```
struct OutFrame { link: u64, bytes: Vec<u8> }
```
One outbound wire frame. Write `bytes` to the transport identified by `link` (GATT
write-without-response, chunked to your negotiated MTU on the host side — the core does not
chunk for you at this boundary). Frames are already in the order the core wants sent; write them
in `poll_send()` drain order, don't reorder or batch them.

### 4.6 `FfiEvent`

A faithful, ciphertext-free mirror of the core `Event` enum — every screen in the host app is a
projection of this stream.

| Variant | Fields | Fires when |
|---|---|---|
| `MessageReceived` | `conv, from, id, body, apptype` | a direct or async message decrypted successfully for you |
| `MessageGap` | `conv, parents` | a conversation DAG parent is missing — render a gap marker (doc 06 §5.3) |
| `Delivered` | `id` | your sent message's receipt came back |
| `DeliveryProgress` | `id, state: FfiDeliveryState` | any state transition in §4.4 |
| `ClaimUpdated` | `owner, field` | a newer claim (nickname, avatar, exchange key, …) arrived for `owner` |
| `CompromiseWarning` | `owner` | two conflicting equal-seq claims from `owner` — surface as a security notice, never silently pick one |
| `ContactSeen` | `peer` | a beacon/session activity resolved to a known contact (not necessarily a full session) |
| `Established` | `peer` | a session handshake + ID_PROOF completed with `peer` |
| `GroupInvite` | `gid, from` | you were added to a group |
| `GroupMessage` | `gid, from, id, body` | a group message decrypted successfully |
| `GroupEpochChanged` | `gid, epoch` | membership changed (add/remove) — re-render member list |
| `TierChanged` | `tier: FfiTier` | the core itself changed tier (rare; usually host-driven via `set_tier`) |
| `Announcement` | `id, scope, annType, notAfter, text, from, official, effectivePriority, scopeNameHint` | a class-5 announcement arrived for a scope you subscribed to (doc 18) |

**`Announcement` fields, precisely** (doc 18 §5.2): `effectivePriority` is the receiver-local
admission clamp — key notification prominence off *this*, not `official` alone. `official` is a
pure rendering predicate (a valid, unrevoked AuthCert with the announce capability) independent of
priority or any rate limit — an announcement can be `official == false` yet still arrive at full
declared priority if it was never clamped (declared priority already ≥ NORMAL). `scopeNameHint`
is a display-only hint (TLV `0x82`), never meaningful for admission — do not use it in any trust
decision.

`on_beacon()` deliberately produces **no** `FfiEvent` — it only feeds the local population
estimate (N̂) used for routing decisions. Do not expect a beacon sighting to surface as
`ContactSeen`; that only fires from actual session activity.

## 5. Method reference — the drive loop

Call shape, grouped by role in the host's drive loop (see `MOBILE_INTEGRATION.md` §2 for the full
loop diagram):

### 5.1 Construction

```
MmpNode.new(seed: bytes32, tier: FfiTier, storePath: string?) -> MmpNode   // throws FfiError
```
`storePath = null` → in-memory store, nothing survives process death. `storePath = <path>` →
file-backed `SnapshotStore`, opened/created immediately, periodically flushed by `tick()` every
30 000 ms of simulated time, or immediately via `flush()`. See §6 for exactly what persists.

### 5.2 Transport injection (host → core)

```
node.linkUp(link: u64, initiator: bool)
node.linkData(link: u64, bytes: bytes)
node.linkDown(link: u64)
node.onBeacon(payload: bytes, rssi: i8)
```
Call these from your native BLE callbacks *directly* — never route raw link bytes through the RN
JS bridge (see `REACT_NATIVE_INTEGRATION.md` §1, the "golden rule"). `linkUp`'s `quality`
parameter from the core `TalEvent` is not yet exposed at the FFI boundary — the core assumes the
pessimistic BLE-GATT floor. `onBeacon` is bookkeeping-only (§4.6) — call it for every scanned
advertisement whose service UUID matches ours, no need to pre-filter further.

### 5.3 Output polling (core → host)

```
node.pollSend() -> OutFrame?     // drain in a loop until null
node.pollEvent() -> FfiEvent?    // drain in a loop until null
```
Both are non-blocking, single-item pops. Drain each to exhaustion after every injected event and
after every `tick()` call — the core does not push, the host pulls.

### 5.4 Time

```
node.tick(nowMs: u64)
```
The only way time enters the core. Call on a steady cadence (≈1 s is the reference interval used
by `mmp-cli` and the test harnesses) *and* immediately after any transport injection that might
need timer-driven follow-up. Drives: message/claim/receipt expiry, filter bucket rotation,
session handshake/idle/airtime timers, quota eviction, and (if `storePath` was set) periodic
persistence.

### 5.5 Messaging

```
node.sendText(dest: bytes16, text: string) -> bytes16      // throws FfiError; returns BundleID
node.messageState(id: bytes16) -> FfiDeliveryState?         // throws FfiError
```
`sendText` requires the destination's exchange key/prekey batch to already be known (via
`ingestClaim`, from a prior QR exchange or mesh gossip) — otherwise `FfiError.UnknownPeer`.
Track delivery via the returned BundleID against `messageState()` or the `Delivered` /
`DeliveryProgress` events.

### 5.6 Groups

```
node.createGroup() -> bytes16
node.groupInvite(gid: bytes16, peer: bytes16)                // throws FfiError
node.groupSend(gid: bytes16, text: string) -> bytes16        // throws FfiError; returns BundleID
node.groupMembers(gid: bytes16) -> [bytes16]
node.groupRemove(gid: bytes16, member: bytes16) -> u16       // throws FfiError; returns new epoch
```
The creator is the (v1, immutable-set) admin. `groupRemove` is admin-only at the protocol level;
the FFI does not pre-check caller identity — the core enforces it and returns an error if you
call it as a non-admin. Removal converges as the network reconverges (doc 06 §8.6) — **do not**
tell the user removal is instant; see `docs/USER_FEATURES.md` for the honest UX framing.

### 5.7 Identity and claims

```
node.nodeId() -> bytes16
node.publicKey() -> bytes32
node.setNickname(name: string, seq: u32)
node.ownClaims() -> [bytes]                                  // encoded Claim objects, for QR/out-of-band exchange
node.ingestClaim(claim: bytes, verified: bool)                // throws FfiError
node.block(peer: bytes16)                                     // throws FfiError
```
`seq` in `setNickname` must be host-tracked and monotonically increasing per field per owner (LWW
merge rule, doc 06 §4.2) — reusing or decreasing it means your own newer claim can lose to a
stale one. `verified: true` in `ingestClaim` marks the owner non-TOFU (completed QR/tap ceremony)
— this feeds the routing utility's social-tie bonus, it is not merely cosmetic.

### 5.8 Tier and diagnostics

```
node.setTier(tier: FfiTier)
node.enableDiagnostics()
node.diagDump() -> string
node.storeLen() -> u64
```
Diagnostics are off by default (doc 12 §7.7) and, once enabled, are **local-only** — nothing here
is telemetry; `diagDump()` is for in-app "message journey" UI or bug reports the user explicitly
shares, never auto-uploaded (see `docs/ROADMAP_TO_PRODUCTION.md` Phase E, "operational
diagnostics policy").

### 5.9 Authority announcements (doc 18)

```
node.setWallClock(unixS: u32)
node.pinAuthorityRoot(scope: bytes16, issuer: bytes32)         // throws FfiError
node.subscribeScope(scopeName: string) -> bytes16
node.unsubscribeScope(scope: bytes16)                          // throws FfiError
node.publishAnnouncement(scope: bytes16, annType: u8, priority: u8, text: string, cert: bytes?) -> bytes16
                                                                 // throws FfiError; returns BundleID
```

`setWallClock` is the **only** place a wall clock enters this crate — everything else runs on
`tick`'s monotonic `nowMs`. It exists solely for AuthCert/revocation `not_after` checks (doc 18
§6.3, no shared clock in a DTN). Never calling it is fail-closed, not merely incomplete: every
AuthCert reads as expired, so every class-5 bundle clamps to NORMAL and nothing renders official —
call it once at startup and again on any host-detected clock jump, using the platform's own trusted
wall clock.

`pinAuthorityRoot` is the "pinned" trust level (doc 18 §2.2 QR-ceremony) — scan the operator's root
key at boarding/enrollment and pin it before subscribing, or every announcement for that scope will
clamp regardless of any attached cert. There is no "gossiped" root API yet (deferred, see
`docs/AGILE_BACKLOG.md` E5-S5).

`subscribeScope` derives and subscribes to `scope_id = BLAKE3("mmp1/scope" ‖ scopeName)[0..16]`
(doc 18 §4) — a **local filter only, never transmitted**; a stranger scanning your beacons or
sessions learns nothing about what you subscribe to. Unsubscribed scopes still relay (store-and-
forward, doc 03 §8) but never surface an `Announcement` event.

`publishAnnouncement`'s `cert` parameter is an encoded `AuthCert` (opaque bytes, same pattern as
`ownClaims()`/`ingestClaim()` — no dedicated UniFFI Record type yet). Attaching it (TLV `0x81`,
RECOMMENDED whenever `priority` ≤ 1/OPERATIONAL, doc 18 §5.2.1) lets a stranger receiver — one who
hasn't seen your AuthCert over claim gossip yet — verify and elevate on first contact. Minting an
`AuthCert` itself has no dedicated method yet; construct and sign one Rust-side (`mmp_types::authcert::AuthCert`)
until a mint helper is added.

### 5.10 Persistence control

```
node.flush() -> void            // throws FfiError; no-op if storePath was null
node.lastPersistError() -> string?
```
Call `flush()` from your platform's background-transition hook (Android `onPause`/
`onStop`, iOS `applicationDidEnterBackground`) to get a sooner-than-30s guarantee before the OS
might suspend or kill the process. Check `lastPersistError()` after `tick()`-driven persists if
you want to surface storage-write failures (e.g., disk full) in a settings/diagnostics screen —
`tick()` itself never throws on a persist failure, it only records it.

## 6. What persists across restart, precisely

| State | Survives `storePath` restart? | Why |
|---|---|---|
| Bundle store (queued/in-mesh messages, gossiped claims-as-bundles, your own outgoing messages) | **Yes** | it *is* the `SnapshotStore` contents |
| `ClaimStore` (every claim currently held, not just what re-arrives as class-3 gossip) | **Yes** | persisted alongside the bundle store, one claim per record, re-verified on load — a claim whose signature no longer checks out is dropped exactly like a corrupt record (handbook §4.3 step 1) |
| Group membership (which groups you're in, current epoch, admin flag) | **Yes** | persisted as an opaque per-group blob (handbook §4.3 step 2) |
| `SeenIndex` (dedup/replay window) | No | rebuilds from mesh traffic; a duplicate re-offer after restart is deduped again once re-seen |
| `ReceiptFilter` | No | same — receipts re-propagate |
| Session/ratchet state (X3DH state, conversation ratchets, group sender-key ratchets — including group members' sender-key distributions) | No — **intentional** | doc 04 §7 "the network is the backup": a restarted node re-establishes sessions and re-derives a fresh local sender key per group exactly like a freshly-met/freshly-invited peer would. Persisting a chain seed or ratchet position would weaken forward secrecy if the disk is captured and risks message-key reuse on the sending side. See `docs/IMPLEMENTATION_HANDBOOK.md` §4.3 for the open PCS-vs-persistence design question if you're considering changing this. |

## 7. What is NOT yet in this surface (as of this writing)

Cross-check against `docs/ROADMAP_TO_PRODUCTION.md` before assuming any of these are close:

- **Blob/file transfer** — `FileManifest` and the Reed-Solomon codec exist in the core but are
  not wired into `mmp-node`'s orchestration or exposed here yet (Phase C).
- **Friendship/proxy buffering** — frames parse, policy is stubbed to "no friendship" (Phase C).
- **Key revocation** — no claim field or FFI method yet (Phase C).
- **Link quality** on `linkUp` — parameter exists in the core `TalEvent` but isn't threaded
  through the FFI call; the core currently assumes a pessimistic default.
- **Radio-adjacent methods on PC** — `mmp-tal-btleplug` (central) and the peripheral-role crate
  are native Rust, consumed by `mmp-cli`, not by this FFI crate — mobile apps always implement
  their own native BLE stack and call the transport-injection methods in §5.2, they never link
  against the PC radio crates.

## 8. Minimal usage sketch

```ts
// TypeScript, illustrative only — see REACT_NATIVE_INTEGRATION.md for the real bridge code.
const node = await MmpMesh.create(seedHex, "RELAY", storePath);
await node.setNickname("alice", 1);

// native BLE callback →
onCharacteristicWrite((link, bytes) => node.linkData(link, bytes));

// drive loop, ~1 Hz, plus after every injected event:
setInterval(async () => {
  await node.tick(Date.now());
  let frame; while ((frame = await node.pollSend())) bleWrite(frame.link, frame.bytes);
  let ev;    while ((ev = await node.pollEvent()))    dispatch(ev);
}, 1000);

const msgId = await node.sendText(peerNodeIdHex, "hello over the mesh");
```

---

**See also:** `MOBILE_INTEGRATION.md` (platform-by-platform integration guide),
`REACT_NATIVE_INTEGRATION.md` (RN bridge implementation), `docs/IMPLEMENTATION_HANDBOOK.md`
(protocol algorithms, for anyone extending the core itself), `docs/ROADMAP_TO_PRODUCTION.md`
(what's shipped vs. planned).
