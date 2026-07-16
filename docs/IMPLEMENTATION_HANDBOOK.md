# MMP/1 Implementation Handbook

**Audience:** the next implementer — a junior engineer or an LLM (Sonnet-class) continuing this
codebase without the original architect in the loop. This document assumes you can write Rust that
compiles but does **not** assume you understand DTN routing, ratchets, or BLE. Every algorithm is
given step-by-step; every structure is given with its exact memory cost; every remaining task is
given as a buildable spec with acceptance criteria.

**Companion documents:** `docs/ROADMAP_TO_PRODUCTION.md` (what to build, in what order),
`docs/USER_FEATURES.md` (why), `implementation/mesh-milk/MOBILE_INTEGRATION.md` and
`REACT_NATIVE_INTEGRATION.md` (platform glue), `specification/01–18` (normative truth — when this
handbook and a spec disagree, the spec wins and this handbook has a bug; report it).

---

## 0. Golden rules (read before touching anything)

1. **Never add `serde` or any codec derive.** All wire encoding is hand-written in
   `mmp-types` (`src/wire.rs` `Reader`/`BufExt`). One valid encoding per object, byte-exact.
2. **Never call `verify` — only `verify_strict`** (Ed25519 canonical-S; AUDIT F9). The only call
   site is `mmp_crypto::verify()`; route all verification through it.
3. **The lower stack is sans-IO.** `mmp-types ← mmp-crypto ← mmp-core ← mmp-router` are
   `#![no_std]`-capable, own no sockets, no threads, no clocks. Time enters exactly one way:
   a `now_ms: u64` monotonic parameter. If your change needs `tokio`, `std::time`, or a radio,
   it belongs in `mmp-node` or above. `cargo run -p layer-check` enforces this; CI must stay green.
4. **Verify-before-forward is unconditional.** A bundle that fails signature verification is
   dropped *before* the dedup insert (see §3.1). Never reorder the ingest pipeline.
5. **Receipt Bloom = hint, exact receipt = purge** (AUDIT F11). The type system enforces this via
   `SuppressSignal::{None, Hint, Purge}` — never convert a `Hint` into a deletion.
6. **No wall-clock assertions in tests.** Assert protocol observables (delivery ratio, counters,
   store contents). Print wall time with `eprintln!` for information only.
7. **All constants live in `mmp-types/src/consts.rs`** — a single file so the doc-13 calibration
   pass is a one-file change. Never inline a protocol number elsewhere.
8. **Secrets never cross the FFI outward.** The 32-byte seed goes in at construction; only public
   keys, IDs, plaintext-for-the-owner, and wire bytes come out.
9. **No panic reachable from wire or disk bytes.** Any parser fed attacker- or
   corruption-controlled input (frame/bundle decoders in `mmp-types`, `mmp-store`'s snapshot
   loader) must fail by returning `None`/`Err`/skipping the record — never `.expect()`,
   `.unwrap()`, or direct slice indexing on a length derived from that input. `.unwrap()` is
   only acceptable where the value came from a bound already checked via `.get()` in the same
   expression (so the conversion itself cannot fail, e.g. a length-4 slice into `[u8; 4]`), never
   as a substitute for the bounds check. (E2-S6, `mmp-store/src/lib.rs::load`/`decode_record`.)

---

## 1. System map and memory model

### 1.1 Crate layering (arrow = "may depend on")

```
mmp-types  ←  mmp-crypto  ←  mmp-core  ←  mmp-router
   (no_std, alloc-only; zero IO; the protocol's truth)
                                  ↑
                        mmp-node (std; orchestration)
                       ↑        ↑         ↑
                 mmp-ffi    mmp-cli    mmp-sim
                     ↑          ↑
             (mobile apps)  mmp-tal-btleplug (tokio lives HERE and only here)
```

`mmp-tal` defines the transport vocabulary (`TalEvent`, `LinkId`, `LinkQuality`) plus `MemLink`
for tests. Radio crates are **byte shuttles** — they never parse protocol bytes.

### 1.2 RAM budget per subsystem (Standard tier — a phone)

All figures derived from `consts.rs`; formulas given so you can recompute after retuning.

| Subsystem | Structure | Formula | Standard-tier bytes |
|---|---|---|---|
| SeenIndex blooms | 12 buckets × Bloom | `FILTER_BUCKETS × ⌈(BLOOM_CAPACITY×10)/8⌉` | 12 × 10 240 = **120 KiB** cap |
| ReceiptFilter blooms | 12 buckets × Bloom | same | **120 KiB** cap |
| ReceiptFilter exact set | `BTreeMap<Id16,u64>` | entries × ~56 B (16 key + 8 expiry + node overhead) | 8 192 receipts ≈ **448 KiB** worst; typically ≪ |
| Ratchet skipped keys | per conversation, per direction | `RATCHET_WINDOW × ~40 B` (4 B idx + 32 B key + map node) | ≤ **20 KiB**/conv worst; ~0 typical |
| PRoPHET P-table | `BTreeMap<NodeId,Predict>` LRU-capped | `P_TABLE_CAP × ~60 B` | ≤ **15 KiB** |
| ContactGraph | `BTreeMap<NodeId,ContactStats>` capped | `CONTACT_GRAPH_CAP × ~70 B` | ≤ **280 KiB** |
| Session RX buffer | preallocated `Vec<u8>` | `MAX_ENVELOPE_PAYLOAD + 3` | **4 131 B** per live session |
| Bundle store | disk (SnapshotStore) or RAM (MemStore) | `QUOTA_*` | Core 32 KiB / Standard 128 MiB / Anchor 2 GiB |

Rule of thumb: the resident protocol state of a Standard node is **< 1 MiB** plus the bundle
store. The Core (embedded) tier shrinks blooms to 2 buckets × 256 entries (320 B each) and
`RATCHET_WINDOW`→32; see §4.9.

### 1.3 Wire-size invariants

- Bundle ≤ `MAX_BUNDLE` = 4 096 B; fixed header overhead 124 B (includes the inline 32-byte
  origin public key — AUDIT F2 — so verification never depends on claim propagation).
- Session frame payload ≤ `MAX_FRAME_PAYLOAD` = 4 096 B; envelope = `[len:u16 LE][type:u8][payload]`,
  encrypted payloads get +16 B AEAD tag headroom inside `MAX_ENVELOPE_PAYLOAD` = 4 128 B.
- **Anything that can exceed a frame must be folded or split at the sender.** Precedent: the
  SUMMARY receipt bloom is OR-folded to `BLOOM_SUMMARY_MAX` = 1 280 B (§3.6). A `debug_assert`
  in `write_envelope` is the canary; if you hit it, your frame construction is wrong — do not
  raise the cap.

---

## 2. Core data structures (what exists — precise)

### 2.1 Bundle (`mmp-types/src/bundle.rs`)

The unit of store-carry-forward. Wire layout (doc 10 §3): fixed header (class, priority, flags,
lifetime, dest `Id16`, origin pubkey `Key32`, conv/scope fields), ascending-order TLV extensions,
payload, Ed25519 signature. Two derived identities, both computed over **canonical zeroed-lifetime
bytes** (AUDIT F1 — lifetime is mutable in flight, so it is zeroed for hashing/signing):

- `BundleID = BLAKE3(canonical_zeroed)[0..16]` — the content address, dedup key, store key.
- Signature = Ed25519 over `signed_preimage(SigDomain::Bundle, canonical_zeroed)` where the
  domain prefix is `"mmp1/bundle"` (AUDIT F4 — every signed object type has its own domain tag
  in `mmp-types/src/domain.rs`; **never sign raw bytes without a domain**).

Classes: 0 control (receipts), 1 direct, 2 group, 3 claim, 4 blob symbol (§4.4). Unknown classes
are stored and forwarded blind (doc 03 §8) — never rejected.

### 2.2 SeenIndex (`mmp-core/src/seen.rs`)

Purpose: resurrection-resistant duplicate suppression. Structure: 12 time-bucketed Blooms
(same dimensioning as §1.2) plus an exact recent-ring. Key property: a bundle is inserted
**from first sight** and the insert happens *after* verification but *before* storage decisions —
so an evicted-then-reoffered bundle is still recognized (no "resurrection" of purged data).

### 2.3 ReceiptFilter (`mmp-core/src/receiptfilter.rs`)

12 × 6 h Bloom buckets (rotation covers the 72 h max lifetime) + `BTreeMap<Id16,u64>` of exact
verified receipts. `signal(id)` returns `Purge` only on exact hit; Bloom hit → `Hint`
(deprioritize offers, never delete). Peer buckets are folded in via `union_from_peer` —
authenticated sessions only (doc 06 §6.3). Wire form is the folded mini-filter (§3.6).

### 2.4 Ratchet (`mmp-crypto/src/ratchet.rs`)

Symmetric double-ratchet-lite: per conversation, two direction chains derived from CK₀
(`direction_chain`), each a hash chain producing per-message keys. DTN reordering is absorbed by
a skipped-key window of `RATCHET_WINDOW` = 512 message keys (evicting oldest beyond the window).
Memory: §1.2. Epoch bumps happen opportunistically at live contact (PCS-upon-contact; there is
deliberately **no** PCS between meetings — doc 08 §5 rationale).

### 2.5 Routing state (`mmp-router/src/utility.rs`)

- `P-table`: PRoPHET-style predictability `P ← P + (1−P)·P_INIT` on contact, aged by
  `P·γ^hours` (γ = 0.98/h). **Locally observed only — no transitivity**, which is the routing
  poisoning defense: a peer can never inflate your opinion of a third party.
- `ContactGraph`: encounters, duration, last_seen, verified flag.
- `tie(dest)`: `0.5·freq + 0.5·recency (+0.25 verified, capped 1.0)`; **zero-encounter contacts
  contribute freq = recency = 0** — see §5 bug ledger, this was a delivered-message-blocking bug.
- `utility(dest) = max(P(dest), 0.5·tie(dest))`.
- `N̂` population estimate: distinct resolved peers + 0.75 × distinct ephem IDs over 24 h, EWMA.
  Never exchanged with peers (poisoning vector).

### 2.6 Session engine (`mmp-core/src/session.rs`)

A pure state machine: `Handshake → Binding → Established → Closed`. Inputs: `on_wire(bytes)`,
`tick(now_ms)`. Outputs: `poll_out()` (wire bytes), `poll_event()`. All storage callbacks go
through the `SessionHost` trait (implemented by `NodeCore` in `mmp-node`). Timers: handshake
budget 5 s, idle 10 s, airtime cap 60 s. Denials are **silent** — indistinguishable from radio
loss (doc 07 §6.5).

---

## 3. Core algorithms (step-by-step, as implemented)

### 3.1 Ingest pipeline (`mmp-core/src/ingest.rs`) — order is NORMATIVE (doc 03 §4.1.2)

```
bytes → 1. decode_canonical (reject trailing bytes, bad TLV order, oversize)
      → 2. verify signature (domain mmp1/bundle, verify_strict, inline origin key)
      → 3. SeenIndex check  — duplicate? drop (count it)
      → 4. SeenIndex insert — from FIRST sight (before any storage decision)
      → 5. lifetime clamp   — min(received, class_max_lifetime_ms(class))
      → 6. admission        — blocklist, per-origin token bucket (WoT ×4)
      → 7. store            — BundleStore::put
```
Reordering any two steps creates a vulnerability (e.g., dedup-before-verify lets an attacker
poison the dedup index with garbage IDs; store-before-admission bypasses quotas). If you touch
this function, re-run the negative-vector tests in `mmp-core/src/tests.rs`.

### 3.2 Session establishment

1. Initiator (chosen by dial direction) sends Noise **XX** message 1 (`snow`,
   `Noise_XX_25519_ChaChaPoly_BLAKE2s`).
2. msg2 ← responder; msg3 → responder; both sides now hold an AEAD channel.
3. Both sides immediately send `ID_PROOF`: an Ed25519 signature (domain `mmp1/chanbind`) over a
   channel-binding object that includes **both** identity public keys — this kills
   unknown-key-share attacks (doc 08 §3.2).
4. On verified proof: `accept_peer()` (blocklist ⇒ silent close), phase → Established, send
   SUMMARY first (short contacts deliver value early — doc 03 §5.2), then the initiator starts
   reconciliation domain 0.

### 3.3 RBSR set reconciliation (`mmp-core/src/rbsr.rs`, doc 06 §3)

Purpose: two nodes discover which BundleIDs each is missing, in O(log n) rounds, statelessly.

- Fingerprint of an ID set: `BLAKE3(xor_fold_16(ids) ‖ count_u32_le)[0..8]`.
- Initiator opens a domain with the full-range `[MIN,MAX]` fingerprint.
- Receiver compares against its own set for that range:
  - equal fp+count → echo (reply flag set) — range resolved;
  - mismatch and either side ≤ `T_LIST` = 24 items → send the full ID list;
  - both big → split into ≤ `B_f` = 16 count-balanced sub-ranges, send those fingerprints.
- An ID list is terminal: receiver WANTs what it lacks and answers with its own list at most once
  per range.
- **Domains** partition the ID space so priority traffic reconciles first (doc 06 §2):
  0 = control+claims, 1 = direct-for-{me,you}, 2 = group, 3 = foreign direct (relay inventory).
  Driven strictly in order 0→3 by the initiator.
- Transfer is **pull-based**: WANTed bundles are sent as `BUNDLE` immediately followed by
  `BUDGET` (the spray grant, §3.4).

### 3.4 Spray-and-wait routing decision (`mmp-router/src/lib.rs`, doc 05 §2)

Every bundle carries a copy budget `L`. Origin sets
`L = clamp(2·⌈log₂(N̂+1)⌉, 4, 32)` (integer math: `32 − leading_zeros(x−1)`).
On contact, per WANTed bundle, `handoff_decision`:

```
peer is destination        → Deliver (budget irrelevant)
peer is origin             → Keep  (never offer back)
receipt signal ≠ None      → Keep  (delivered already — hint or purge)
remaining life < 10 min    → Keep  (not worth the airtime)
budget > 1:
    u_peer ≥ u_self − 0.3  → Spray: give ⌊b/2⌋, keep ⌈b/2⌉   (binary split)
    else                   → Keep
budget == 1:
    u_peer > u_self + 0.15 → Focus: hand over the last copy, stop carrying
    else                   → Keep
```
Σ budgets across all copies never exceeds L (property-tested). Receive side clamps any grant to
`L_MAX` = 32 (AUDIT F3) — never trust the peer's arithmetic.

### 3.5 Receipt lifecycle (doc 05 §7)

1. Destination ingests a class-1 bundle addressed to it → decrypts → emits a class-0 receipt
   bundle: MAC keyed by the conversation receipt key (only the true recipient can mint it),
   spray budget `L_RECEIPT` = 8, lifetime 6 h.
2. Any carrier ingesting a verified receipt: inserts into ReceiptFilter (exact + Bloom bucket).
3. **Purge gate**: a node deletes its stored copy only if the receipt's exact ID matches AND
   the deletion target's destination equals the receipt's claimed origin — never on Bloom hit.
4. Origin receiving the receipt: `DeliveryState → Delivered`, UI gets the cryptographic ✓✓.

### 3.6 SUMMARY mini-filter fold (`mmp-core/src/bloom.rs::folded`)

A Standard receipt bucket is 10 240 B > 4 096 B frame cap. Fold: OR the top half onto the bottom
half repeatedly until ≤ `BLOOM_SUMMARY_MAX` = 1 280 B. Correctness: each halving keeps `m′ | m`,
so `(h mod m) mod m′ = h mod m′` — a member can never be lost, only extra false positives appear,
and the receiving side treats the result as `Hint`-only, so extra FPs merely deprioritize offers.
Receive side (`union`) tiles a smaller filter across its full array (same argument in reverse).
**This pattern — lossy-but-never-false-negative compression gated on hint-only semantics — is the
approved way to fit any oversized advisory structure into a frame.**

### 3.7 Claims LWW (`mmp-core/src/claims.rs`, doc 06 §4)

`Claim { owner: Key32, field: u8, seq: u32, body, sig }`, domain `mmp1/claim`. Merge: higher seq
wins; equal seq + different body → lexicographically smaller `BLAKE3(body)` wins (deterministic
everywhere, no clocks) AND the conflict is surfaced as a compromise indicator. Fields: 0x01
nickname, 0x02 avatar root, 0x03 exchange key, 0x04 prekey batch, 0x05 friendship, 0x06 group
epoch anchor (0x07 reserved — see §4.6 revocation).

### 3.8 Groups (doc 08 §6)

Sender-keys: each member distributes a per-epoch symmetric sender key (`SenderKeyDist`) to every
other member over the pairwise-encrypted channel. Membership is an admin-signed OR-Set
(add-wins); **every effective change bumps the epoch**, admins sign an epoch-anchor claim
(membership BLAKE3 root), and tag/tombstone GC compacts at mutually-anchored epochs (AUDIT F5).
Removal takes effect as the network reconverges — this is a physics bound, not a bug (doc 06
§8.6); the UI must say so.

---

## 4. Remaining work — buildable specs

Each spec: context → structures → algorithm → memory → acceptance. Sizes refer to
`docs/ROADMAP_TO_PRODUCTION.md` phases.

### 4.1 M5b: PC peripheral role (Phase A)

**Context.** `mmp-tal-btleplug` is central-only (the library cannot advertise/serve GATT). For
PC↔PC radio contact one side must be a peripheral. Platform APIs: BlueZ (Linux) via the `bluer`
crate supports GATT server + advertising; Windows via `windows` crate WinRT
(`GattServiceProvider`, `BluetoothLEAdvertisementPublisher`); macOS peripheral is possible but
lowest priority.

**Structure.** New crate `mmp-tal-peripheral` (do not graft onto the btleplug crate — different
dependency trees per OS, feature-gated: `bluez`, `winrt`):

```rust
pub struct PeripheralRole { /* platform handle, link table */ }
impl PeripheralRole {
    pub async fn new() -> Result<Self, PeriphError>;
    /// Start advertising: service UUID + the 24-byte beacon payload as service data.
    /// Re-call every EPHEM_SLOT_MS (15 min) with the freshly rotated beacon from Node::beacon().
    pub async fn advertise(&self, beacon: Vec<u8>) -> Result<(), PeriphError>;
    pub fn events(&self) -> UnboundedReceiver<TalEvent>;      // LinkUp{initiator:false}/LinkData/LinkDown
    pub async fn notify(&self, link: LinkId, bytes: &[u8]) -> Result<(), PeriphError>; // TX via notifications
}
```

GATT layout (must byte-match `mmp-tal-btleplug`'s dev constants): service `DEV_SERVICE_UUID`;
RX characteristic (`...-0001-...`) write-without-response — central writes, you emit
`TalEvent::LinkData`; TX characteristic (`...-0002-...`) notify — you send via `notify` in chunks
of (negotiated MTU − 3), floor 20 B. Mint `LinkId` from BLAKE3 of the central's connection
identifier (same recipe as the central crate). One connected central = one link; on disconnect
emit `LinkDown` and forget.

**Memory.** Negligible: link table ≤ 8 entries × ~64 B; no protocol state (byte shuttle).

**Acceptance.** `mmp-cli listen` (see §4.2) on machine A + `mmp-cli` dial on machine B exchange a
text and its receipt over real radio. Unit tests: chunk arithmetic, LinkId determinism (no
hardware). Hardware path behind an example binary, never in CI.

### 4.2 mmp-cli radio wiring (Phase A)

Add subcommands. The drive loop is THE reference implementation of "how to host a node" — keep it
under ~100 lines and heavily commented:

```
mmp-cli listen <seedfile> [--store <path>]   # peripheral role
mmp-cli connect <seedfile> [--store <path>]  # central role: scan, dial best RSSI, sync
mmp-cli send <seedfile> <hex-nodeid> <text>  # queue a message into the store, then run connect
```

Drive loop algorithm (identical for both roles — only the TAL differs):

```
node = Node::new(seed, SnapshotStore::open(path), Tier::Anchor)
every 1 s:                       tick(now_ms)
on TAL event:                    node.on_tal_event(ev)
after any event/tick:            while Some((link, bytes)) = node.poll_send(): tal.send/notify
                                 while Some(ev) = node.poll_event(): print(ev)
every EPHEM_SLOT_MS (peripheral): tal.advertise(node.beacon(now_ms))
on Ctrl-C:                       node flush/persist, exit 0
```

**Acceptance:** the Phase A exit gate (two machines, real BLE, message + receipt), plus
`--help` documents every subcommand.

### 4.3 Persistence completion (Phase B)

Today only the bundle store survives restart. Extend in **this order** (each step independently
shippable):

1. **Claims** (`ClaimStore`): serialize as the wire `Claim` encoding (already canonical, already
   signed — re-verify on load, dropping any record that fails). File format: reuse the
   `SnapshotStore` record framing (`MAGIC ‖ per-record BLAKE3 checksum ‖ skip-corrupt-on-load`).
   Memory: a claim ≤ ~600 B; ContactGraph cap 4 096 peers × ~3 claims → ≤ ~8 MiB disk worst case.
2. **Group state** (`GroupState` per group): members OR-Set ops since last anchor, epoch, local
   sender key, remote sender keys, dists. All content already has canonical encodings
   (`SenderKeyDist::encode`, membership ops) — persist those, never ad-hoc formats.
3. **SeenIndex / ReceiptFilter**: persist bucket epochs + raw bloom bit arrays + the exact-receipt
   map (~240 KiB + exact map). Cheap and prevents post-restart duplicate delivery.
4. **Ratchet state — DECIDED: persist, keystore-wrapped.** Normative decision + full rationale:
   **doc 08 §5.3 (amendment 2026-07-16)**. The earlier "(a) don't persist" default here was wrong
   because its assumed recovery path doesn't exist: a sender holding a `Conv` never re-runs X3DH,
   prekey secrets are memory-only too, and the `open_async` concurrent-initiation tiebreak means
   restarts of the canonically-larger party wedge the pair deterministically — losing all in-flight
   DTN traffic every restart, silently.
   What to build (phones/PC only): persist per-conversation `(send CK, epoch, next_index)`,
   `(recv CK, epoch, next_index)` and the skipped-key map, plus the `PrekeyStore` secrets, encrypted
   under a platform-keystore-wrapped key (Android Keystore / iOS Keychain / DPAPI) — never in the
   clear, and never CK₀ (erase it once `Conv::new` has derived both chains). Stamp the blob with
   written-at time; on load discard anything older than `RATCHET_PERSIST_TTL_MS =
   LIFETIME_MESSAGE_MS` (new local constant) and fall back to fresh X3DH. Prerequisite: wire
   `bump_epoch` into the live-session path (doc 08 §5.2) so the first contact after a restore heals
   the on-disk exposure. Where no keystore exists, and on Core/`no_std`: build nothing (RAM only).

**Acceptance:** kill -9 during a sync session; restart; node resumes with same claims/groups; a
previously-received bundle re-offered by a peer is deduped (proves SeenIndex survived); test in
`mmp-node` using a real `SnapshotStore` in a temp dir.

### 4.4 Blob plane — file transfer (Phase C; the largest remaining algorithm)

**Context.** `FileManifest`/`FileEntry` (mmp-types), the GF(256) Reed-Solomon codec
(`mmp-rs-code`), and BLAKE3 Merkle (arity 4, `mmp-crypto/src/merkle.rs`) all exist and are
tested. Missing: the orchestration that turns a file into class-4 bundles and back. Frames
`BlobReq`/`BlobHave` parse but are advisory. Normative spec: doc 16.

**Constants** (add to `consts.rs`, cite doc 16): `SYMBOL_SIZE = 2048` B (half the bundle payload
budget → symbol bundle ≈ 2 180 B wire, comfortably under `MAX_BUNDLE`); `RS_DATA_K = 32`,
`RS_PARITY_R = 8` per stripe (25 % overhead, tolerates any 8 losses per stripe; GF(256) bound
k+r=40 ≤ 255 ✓). `FILE_MAX` = 8 MiB → ≤ 128 stripes → ≤ 5 120 symbols per file.

**Sender algorithm:**
```
1. bytes = file; pad to a multiple of SYMBOL_SIZE·RS_DATA_K with zeros, record true length.
2. for each stripe s (RS_DATA_K consecutive symbols):
       parity[0..R] = mmp_rs_code::encode(data_symbols)      # systematic: data unchanged
3. all symbols (data+parity, in order) → leaves of the BLAKE3 Merkle tree (arity 4) → root.
4. FileManifest { root, true_len, symbol_size, k, r, stripes, name, mime, thumb? } —
   signed inside an ordinary class-1/2 message (the manifest IS the "file offer" the user sees).
5. each symbol → class-4 bundle: dest = conversation/group scope, payload =
   stripe_no u16 ‖ index_in_stripe u8 ‖ symbol bytes, plus a TLV carrying the Merkle root
   (so carriers can group symbols) and the Merkle path for self-verification.
   Sign like any bundle. Lifetime = LIFETIME_MESSAGE_MS. Spray budget: origin_budget(N̂) but
   cap total outstanding symbol bytes against the origin's own token bucket — a file must not
   starve the sender's text messages (send symbols at lower priority).
```

**Receiver algorithm:**
```
1. manifest arrives in a normal message → create BlobAssembly { root, have: bitvec, stripes }.
2. class-4 bundle ingested (ordinary pipeline §3.1 — symbols are self-verifying via Merkle path
   against the manifest root; verify BEFORE counting toward assembly): set have[symbol].
3. per stripe: ≥ K symbols present → mmp_rs_code::decode → stripe bytes; all stripes → truncate
   to true_len → BLAKE3 whole-file check → emit Event::FileReady { root, path }.
4. WANT-driven repair: on session, send BlobHave { root, bitmap } (bitmap = ⌈symbols/8⌉ B ≤ 640 B
   for 8 MiB — fits one frame); peer replies with symbols you lack that it holds, budget rules
   as usual. BlobReq { root, ranges } asks explicitly for stripes when you know the peer is the
   origin or a strong holder.
```

**Memory.** Assembly state per active file: bitvec 640 B + stripe buffer `K × SYMBOL_SIZE` =
64 KiB (decode one stripe at a time — never hold the whole file in RAM; write decoded stripes to
disk incrementally). Symbols themselves live in the bundle store under normal quota. **Forbid the
blob plane on the Core tier** (32 KiB quota can't hold a stripe).

**Acceptance:** sim test — A sends a 1 MiB file to B via relay R with 30 % random symbol loss per
contact; B reconstructs bit-exact; store quotas respected; a second send of the same file is a
no-op (dedup by content address). Unit tests: stripe padding edges (empty file, exactly one
symbol, non-multiple length), Merkle path verification failure drops the symbol.

### 4.5 Friendship / proxy buffering (Phase C; doc 04 §4, doc 06 §7)

**Context.** A "friend" node volunteers to buffer traffic for a mostly-offline "dependent"
(e.g., your home PC buffers for your phone). Frames `FRIEND_OFFER/FRIEND_ACK/PROXY_SUMMARY`
parse; the policy engine is stubbed to "no friendship".

**Structures** (in `mmp-node`):
```rust
struct FriendState { quota_used: u64, buffered: BTreeSet<Id16>, last_poll_ms: u64 }
friends:    HashMap<NodeId, FriendState>,   // dependents I buffer for
patrons:    HashSet<NodeId>,                // nodes buffering for me
FRIEND_QUOTA: u64 = 16 MiB per dependent (new const, doc 13 note).
```

**Algorithm.** (1) Friendship is a counter-signed claim (field 0x05 exists): both parties sign
`owner ‖ friend ‖ since`; gossip as normal claims. (2) A friend, when ingesting a class-1 bundle
whose dest is a dependent, exempts it from normal eviction order (pin) up to `FRIEND_QUOTA` and
does NOT count it against the dependent's origin bucket. (3) `PROXY_SUMMARY` (friend → anyone):
receipt-filter-style digest of buffered IDs per dependent, so third parties can route toward the
friend. (4) On dependent contact: ordinary RBSR (domain 1 already covers dest=you) delivers;
after the dependent's receipts appear, unpin and GC. Receipts stay end-to-end — the friend never
mints them.

**Acceptance:** 3-node sim: A sends to phone-B while B is offline; PC-F (B's friend) buffers past
its normal eviction pressure (fill the store with junk to force eviction — pinned bundles
survive); B comes online, syncs from F, receipt purges F's copy. Negative: non-friend node under
the same pressure evicts.

### 4.6 Revocation & succession claims (Phase C; doc 18 §6 generalized)

**Field number corrected (E5-S3, 2026-07-15):** this section originally assigned field `0x07` to
general identity revocation, which collides with `0x07 = AUTHCERT` (doc 18 §2.1, `mmp-types/src/claim.rs`,
now implemented — see E5-S5). The orchestrator decision in `docs/AGILE_BACKLOG.md` E5-S3 moved
general identity revocation/succession to the next free field, **`0x09`** — doc 06 §4.1 registers
it there once E5-S3 lands. This is a distinct mechanism from doc 18 §6's `REVOCATION_LIST`
(field `0x08`, AuthCert/authority-scoped, already implemented), which revokes a *grant*, not an
*identity*.

New claim field `0x09 revocation`: body = `revoked_key: Key32 ‖ reason: u8 ‖ successor: Key32?
‖ successor_countersig: Sig64?`. Signed by the revoked identity (domain `mmp1/claim` as usual).
Merge rule: a revocation is a **tombstone** — it wins over any other claim by that owner
regardless of seq (implement as: `ClaimStore::merge` short-circuits when a stored revocation
exists; a revocation itself is immutable once seen). Succession: if `successor` present and the
counter-signature (successor signs `revoked ‖ successor`, domain `mmp1/claim`) verifies, UI may
offer "trust successor" — petnames migrate only on explicit user confirmation (petname model,
never automatic). Nodes MUST refuse new sessions and drop prekey claims from a revoked key.
Memory: one claim per revoked identity, retained indefinitely for ContactGraph peers (same rule
as other claims).

**Acceptance:** unit — revocation beats higher-seq nickname claim; session attempt from revoked
key is silently denied; succession countersig failure → succession ignored, revocation stands.

### 4.7 Fuzzing (Phase C) — targets SHIPPED (E5-S6, 2026-07-16); test vectors still open

`cargo fuzz` workspace member `fuzz/` with targets, in priority order (pre-auth parsers first —
doc 07 §8 / doc 12 §7 mandate):
1. `fuzz_beacon` → `Beacon::decode` (24 B input — trivial but it's pre-auth).
2. `fuzz_envelope` → `read_envelope` + `Frame::decode_payload` over arbitrary type bytes.
3. `fuzz_bundle` → `Bundle::decode_canonical` (must never panic, only `Err`).
4. `fuzz_hs` → `NoiseHandshake::responder().advance(bytes)` (std-gated).
Invariant for all: no panic, no OOM (cap allocations by construction — the parsers already
pre-validate lengths; any allocation proportional to unvalidated input found by the fuzzer is a
bug of AUDIT F10 class).

**Shipped:** `implementation/mesh-milk/fuzz/` (own `[workspace]`, isolated from the main build —
verified via `cargo test --workspace`/`clippy`/`layer-check` all staying green with it present).
All four harnesses compile cleanly under `rustc`; full sanitizer-instrumented linking was verified
on CI's `ubuntu-latest` runner (`fuzz-build` job, every push) but could **not** be verified on the
Windows box this was implemented on — a documented toolchain gap (missing ASan runtime), not a code
defect; see `fuzz/README.md` for the exact linker errors and reasoning. A `fuzz-run` CI job
(manual `workflow_dispatch`, 2 min/target) exists as a smoke check, deliberately short of doc 12
§7's "N CPU-hours once a baseline is measured" real hardening run. **Deferred, not done:** a
curated seed corpus (bootstrapping from empty works for formats this size, but a hand-picked seed
set would likely find issues faster).

Test vectors: `specification/test-vectors/*.json` + a generator binary (`mmp-cli vectors`) and a
consuming test. Cover: bundle canonical bytes + BundleID + signature (fixed seed), beacon codec,
claim signing, receipt MAC, X3DH agreement (deterministic RNG), envelope framing, RBSR
fingerprint of a known ID set. These vectors are the anti-ossification tool — a second
implementation must pass them byte-exact.

### 4.8 Adversarial simulation suite (Phase C; doc 14)

Extend `mmp-sim` with an `Adversary` node wrapper that bypasses `Node` and speaks raw frames
(reuse `mmp-types` directly). Scenarios, each an independent test asserting a protocol
observable:

| Scenario | Attack | Must hold |
|---|---|---|
| replay | re-inject every observed bundle 100× | delivery events fire exactly once; SeenIndex drop counter ≥ replays |
| flood | 1 origin sends at 50× bucket rate | victim's other conversations still deliver (ratio ≥ 0.9); flood origin's admission drops ≥ expected |
| quota abuse | adversary fills relay stores with max-size bundles | honest traffic delivery ratio degrades gracefully (≥ 0.7 at 2× quota pressure), eviction order sacrifices adversary's unreceipted bundles first |
| eclipse | adversary is victim's ONLY contact | victim's claims cannot be regressed (LWW), no foreign purge (receipt gate), sessions stay up (adversary gains nothing but its own traffic) |
| bloom poison | adversary sends all-ones summary blooms | hints deprioritize but nothing is deleted; delivery still completes via WANT path |

### 4.9 Embedded tier (post-GA unless a customer forces it; OQ-7)

Budget to fit **48 KiB RAM / 256 KiB flash** (nRF52833 class): Core-tier constants — blooms
2 buckets × 256 entries = 320 B each; `RATCHET_WINDOW` 32 → 1.3 KiB/conv, max 2 conversations;
P-table cap 16 → ~1 KiB; store quota 32 KiB (it's the RAM store); session RX buffer 4 131 B;
one session at a time. Total protocol RAM ≈ 12 KiB + 32 KiB store ≈ 44 KiB — inside budget with
no heap fragmentation margin to spare, hence: **fixed-capacity collections (`heapless`), no
`alloc` growth after init.** The lower crates are already `no_std`; the work is (a) a
`heapless` feature replacing `BTreeMap/Vec` growth points, (b) `mmp-tal-embedded` over
`nrf-softdevice`/`esp-hal` GATT, (c) `mmp-node-embassy` executor shell (the engines are sans-IO —
this is wiring, not protocol). Gate: doc 12 §8's OQ-7 RS-decode benchmark on target silicon.

### 4.10 Authority announcements — core mechanism SHIPPED (E5-S5, 2026-07-16, doc 18)

**Design delegated to a Fable-class agent** (per `docs/AGILE_BACKLOG.md`'s own assignment table:
security-sensitive/cross-cutting design → Opus/Fable-class designs, Sonnet implements), because the
integration point — where the doc 18 §5.2 clamp sits relative to `ingest()`'s existing boolean
admission gate, and how it interacts with the doc 18 §5.2.2 requirement that RBSR partitioning key
on the *declared* priority, never the clamped one — was a genuine architectural fork, not a
mechanical translation of the spec.

**What shipped:**
- `mmp-types/src/authcert.rs`: `AuthCert` (152 B wire), `AnnouncementBody`, `RevocationList`/`RevEntry`
  codecs + `signed_object()`s under `SigDomain::AuthCert`/`SigDomain::Announce`; `scope_id_of` (doc 18 §4).
- `mmp-core/src/store.rs`: `BundleMeta` split into `priority` (declared, wire-signed, never mutated —
  feeds `transfer_order` and the future RBSR partition key) and `effective_priority` (receiver-local
  clamp target — feeds `eviction_order` only). `mmp-core/src/ingest.rs` sets the fail-safe default
  (class-5 ⇒ `effective_priority = declared.max(NORMAL)`) unconditionally; this can only be *raised*
  back toward the declared byte afterward, never lowered, and never causes a drop.
- `mmp-core/src/authority.rs` (`AuthorityStore`): cert verification (pinned-root only — gossiped-root
  trust is deferred), sticky revocation union (doc 18 §6 "revoked wins, sticky"), the `grant_for`/
  `official` predicates, and the pure `effective_class5_priority` combinator.
- `mmp-node/src/authority_bucket.rs` (`AuthorityBuckets`): per-AuthCert.subject token bucket (doc 18
  §5.3), structural twin of `mmp-router`'s per-origin `OriginBuckets` but on the node-policy plane.
  **RAM-only, not persisted** — its loss costs only one re-granted burst per subject on restart, the
  same "accounting not prevention" residual doc 18 §5.4(b) already treats as first-class; this is a
  different call than doc 08 §5.3's ratchet-persistence reversal because bucket state carries no
  secrets and its loss doesn't threaten availability, only bound over-admission.
- `mmp-node/src/lib.rs`: `handle_announcement` (class-5 dispatch arm), the `learn_claim` hooks for
  fields `0x07`/`0x08`, and the public API (`set_wall_clock`, `pin_authority_root`, `subscribe_scope`,
  `publish_announcement`, `Event::Announcement`).
- `mmp-ffi/src/lib.rs`: the same surface across UniFFI (`FfiEvent::Announcement`, `pinAuthorityRoot`,
  `subscribeScope`, `publishAnnouncement`, …) — see `docs/API_REFERENCE.md` §5.9.

**Explicitly deferred** (tracked as follow-on work under E5-S5, not silently dropped): RBSR
priority-class partitioning itself (doc 03 §5.2 phase 3 — still absent everywhere; this design only
guarantees the future partition key survives unmutated); D3 relay-spray admission weighting for
subscribed scopes (doc 18 §4 item b); gossiped-root trust (`RootTrust::Gossiped` variant reserved,
unused); rendering re-evaluation on late cert arrival (doc 18 §5.2.1 item 2 is MAY, not done);
durable persistence of `AuthorityStore` (`encode`/`decode` exist, no `mmp-store` record kind wired
in yet — pins/subscriptions/sticky-revocation-union are lost on restart, acceptable for sim/CLI,
**not shippable on mobile** without this); TLV `0x82 SCOPE_INFO` is parsed and forwarded but nothing
emits it on the publish side yet.

**Tests:** `mmp-core::authority` (7 unit tests — cert verification, cross-scope rejection, expiry,
sticky revocation, the pure clamp combinator); `mmp-router` (declared-vs-effective ordering split);
`mmp-node::tests` (5 integration tests over the real two-node harness — unauthenticated clamp,
unsubscribed-scope silence, valid-cert elevation + official rendering, per-authority budget
exhaustion across distinct announcements, revocation-gossip stickiness). All green in one verify
cycle (`cargo test --workspace`, `clippy -D warnings`, `layer-check`), no fix cycles needed.

**Known spec-precision gap, not silently "fixed":** doc 18 §3's `AnnouncementBody` payload-schema
listing omits `suite`, but its signed preimage requires it (the same pattern doc 18 §2 already
carries for `AuthCert`, with its own explicit cipher-agility rationale). The implementation carries
`suite` as a wire byte for both objects, consistently — recorded as a `CHANGELOG.md` errata
candidate for doc 18 rather than edited into the spec unilaterally.

---

## 5. Bug ledger (fixed — do not reintroduce)

| Bug | Root cause | Guard |
|---|---|---|
| Origin never sprayed own messages | `tie()` treated `last_seen = 0` sentinel as "just seen" → origin over-rated itself as carrier | zero-encounter contacts score 0 freq/recency; regression test `spray_budget_flows...` |
| SUMMARY frame panic after first delivery | 10 KiB receipt bloom > 4 KiB frame cap | mini-filter fold §3.6; canary `debug_assert` in `write_envelope` |
| WoT multiplier ignored on first admission | initial bucket fill computed before multiplier | fixed in `quota.rs`; test covers first-contact admission |
| LWW test asserted stale claim | test re-merged the wrong object after conflict adoption | deterministic-tie-break test in mmp-core |
| Flaky CI | wall-clock assert in sim | banned by rule §0.6 |

Debugging technique that found the first two: the envelope type byte (`bytes[2]`) is outside the
AEAD — a test pump can print the frame-type dialogue without decrypting. Reach for this first
when "sync doesn't deliver".

---

## 6. Definition of done — every task, no exceptions

```
cargo test --workspace                                   # all green, new behavior has a test
cargo clippy --workspace --all-targets -- -D warnings    # zero findings
cargo run -p layer-check                                 # layering intact
```
Plus: constants in `consts.rs` only; new signed objects get a new `SigDomain` tag; new frames get
canonical-encoding negative tests (trailing bytes, oversize, truncation); anything crossing the
FFI gets a length-checked boundary and appears in `MOBILE_INTEGRATION.md` §3 + the RN doc's
surface enumerations; docs updated in the same change, not "later".
