# 12 — Rust Implementation Blueprint

**mesh_milk reference implementation plan.** Status: Phase 5 draft; **revised 2026-06-11 post-audit** (`AUDIT.md`, `CHANGELOG.md`). Maps docs 03–11 onto a Rust workspace. Crate-version pinning happens at implementation start; the *shape* below is normative for the reference implementation.

> **Audit revision summary (this doc):** §4 pins `ed25519-dalek` **`verify_strict`** (canonical-S, AUDIT F9); §7 adds golden vectors for the signing-domain scheme + non-canonical-S rejection (AUDIT F4/F9); §7.7 adds the **diagnostics/trace schema** (AUDIT F14).

---

## 1. Workspace layout

```
mesh-milk/
├── crates/
│   ├── mmp-types        # no_std: ids, frames, bundles, canonical codec (doc 10)
│   ├── mmp-crypto       # no_std+alloc: suite 0x01, ratchets, sealed/prekey (doc 08)
│   ├── mmp-core         # no_std+alloc: bundle core, session FSM, sync engine (docs 03/06/11)
│   ├── mmp-router       # no_std+alloc: spray/utility/quotas/eviction (doc 05)
│   ├── mmp-store        # traits + impls: redb (std), flash-log (no_std)
│   ├── mmp-tal          # transport traits (doc 03 §7) + frame codec (doc 09 §5)
│   ├── mmp-tal-btleplug # PC BLE (Linux/Win/macOS) via btleplug
│   ├── mmp-tal-mobile   # Android/iOS BLE glue behind UniFFI callbacks (§6)
│   ├── mmp-tal-embedded # nRF52 (nrf-softdevice or Zephyr bindings), ESP32 (esp-idf/NimBLE)
│   ├── mmp-node         # std orchestration: scheduler, tier ladder, power mgmt (doc 04)
│   ├── mmp-node-embassy # embedded orchestration on Embassy executor
│   ├── mmp-rs-code      # no_std GF(256) Reed-Solomon (doc 08 §8; OQ-7 bench here)
│   ├── mmp-ffi          # UniFFI bindings: Kotlin/Swift API surface
│   ├── mmp-sim          # discrete-event simulator (OQ-4/OQ-5 — §8)
│   └── mmp-cli          # PC node + debugging/inspection tool
└── vectors/             # golden test vectors (doc 10 §6.4)
```

Dependency rule (enforced via CI `cargo-deny` + layer test): `mmp-types ← mmp-crypto ← mmp-core ← mmp-router` are `#![no_std]` (alloc-optional via feature `alloc`); nothing below `mmp-node` may depend on tokio, std time, or any TAL implementation. This is doc 04 §1's inversion made mechanical.

**Rev-B crate ownership (doc 18 authority/announcements; `docs/AGILE_BACKLOG.md` architecture decision A3.1):** `AuthCert` and announcement-body structs/codecs → `mmp-types` (new `authcert.rs`, extending `payload.rs`); AuthCert verification, revocation tombstones, and the receive-time priority clamp → `mmp-core` (`claims.rs` + ingest admission, doc 18 §5.2); per-subject announcement token bucket and UI events → `mmp-node` (doc 18 §5.3). Same layering rule as above — nothing rev-B adds may push `mmp-types`/`mmp-core` past their `no_std`+alloc-only budget. Tracked as `docs/AGILE_BACKLOG.md` E5-S5, still open.

## 2. Core trait surface (the two seams + platform ports)

```rust
// mmp-tal — the only radio-facing seams (doc 04 §1)
pub trait BeaconPort {
    fn set_beacon(&mut self, payload: &BeaconPayload) -> Result<(), TalError>;
    fn scan(&mut self, mode: ScanMode) -> impl Stream<Item = BeaconHit>;
}
pub trait ContactPort {
    type Link: FrameLink;
    async fn dial(&mut self, hint: &PeerHint) -> Result<Self::Link, TalError>;
    fn incoming(&mut self) -> impl Stream<Item = Self::Link>;
}
pub trait FrameLink {  // reliable ordered frames (doc 10 §2), quality hints (doc 05 §6)
    async fn send(&mut self, f: Frame<'_>) -> Result<(), LinkError>;
    async fn recv(&mut self) -> Result<OwnedFrame, LinkError>;
    fn quality(&self) -> LinkQuality;   // mtu, phy, goodput EWMA
    async fn close(self);
}

// platform ports (no_std-friendly)
pub trait MonotonicClock { fn now_ms(&self) -> u64; }            // doc 03 §4.3
pub trait CryptoRng: rand_core::CryptoRngCore {}
pub trait KeyVault { /* identity ops behind HSM/keystore boundary, doc 08 §10 */ }
pub trait BundleStore { /* get/put/evict/iter_domain(range) for RBSR, quota hooks */ }
```

`BundleStore::iter_domain(lo: Id16, hi: Id16)` returning ordered IDs is the contract RBSR needs (doc 06 §3) — redb range scans on std; on `no_std`, a sorted flash log with an in-RAM index (Core tier holds ≤ hundreds of bundles, doc 03 §9).

## 3. Session FSM as typestate (doc 11 §3)

```rust
pub struct Session<S: Phase> { /* … */ }
pub struct Handshake; pub struct Binding; pub struct Established<P: DomainPhase>;
impl Session<Handshake> { pub async fn complete(self, …) -> Result<Session<Binding>, Abort> }
impl Session<Binding>   { pub async fn prove(self, vault: &impl KeyVault) -> Result<Session<Established<D0>>, Abort> }
// frames acceptable in each phase are methods only on that phase's type —
// the doc 11 phase grammar is a compile-time property; ERROR/CLOSE paths consume self.
```

Bundle lifecycle (doc 11 §4) is *not* object-per-bundle (Performance review, doc 11 §8): state lives in store indices (`enum SlotState` in the record header); invariants I1–I4 are `#[test]`-asserted transition tables over that enum plus property tests (§7).

## 4. Crate selections (rationale, alternatives noted)

| Need | Choice | Rationale / alternative |
|---|---|---|
| Ed25519/X25519 | `ed25519-dalek` (**`verify_strict` only**, AUDIT F9) / `x25519-dalek` | audited lineage, `no_std`; `verify_strict` enforces canonical-S (RFC 8032 §5.1.7) — the permissive `verify` MUST NOT be used; alt: `ring` (no `no_std`) |
| AEAD | `chacha20poly1305` (RustCrypto) | `no_std`, constant-time aims |
| Hash | `blake3` | official, `no_std`, SIMD on std |
| Noise | `snow` | maintained, pattern-complete; alt: hand-rolled XX over dalek (smaller for `mmp-tal-embedded` if snow's footprint hurts — decide at OQ-7 benching) |
| Zeroization | `zeroize` (+`zeroize_derive`) | doc 08 §10 mandate |
| Async (std) | `tokio` | btleplug ecosystem |
| Async (embedded) | `embassy` | nRF52 first-class |
| Embedded BLE | `nrf-softdevice` (nRF52), `esp-idf-svc`/NimBLE (ESP32) | proven peripherals |
| PC BLE | `btleplug` | only maintained cross-OS option; Windows caveats tracked |
| Store (std) | `redb` | pure-Rust, ordered ranges (RBSR need); alt: sled (unmaintained risk) |
| Serialization | **hand-written codec in `mmp-types`** | canonical encoding (doc 10 §6) forbids serde-style permissiveness; serde allowed only for config/debug, never wire |
| RS coding | own `mmp-rs-code` | existing crates target large-block storage profiles; we need `no_std`, k≤64, table-driven (OQ-7); alt: `reed-solomon-erasure` behind a feature for std |
| FFI | `uniffi` | Kotlin+Swift from one IDL; callbacks let platform BLE stacks implement TAL traits (§6) |

## 5. Public app API (sketch, via mmp-node / mmp-ffi)

```rust
let node = Node::builder(vault, store, tal).tier_policy(Auto).build()?;
node.profile().set_nickname("alyosha")?;            // claim, doc 06 §4
let conv = node.conversations().open(peer_id);
conv.send_text("hi")?;                              // bundle, spray, receipt tracked
node.events() -> Stream<Event>;                     // MessageReceived, Delivered(receipt),
                                                    // ClaimUpdated, FriendRequest, CompromiseWarning…
let g = node.groups().create(admins, "name")?; g.invite(peer)?;  // doc 06 §8
node.blobs().publish(bytes)? -> RootId;             // RS-coded, doc 08 §8
```

Events map 1:1 to doc 07 §6 UX duties (verification states, compromise warnings, eclipse alarm) so app authors cannot miss them — they arrive in the same stream as messages.

## 6. Mobile integration strategy

Rust core compiles to `aarch64-linux-android` / `aarch64-apple-ios`; the **platform owns the radio**, implementing TAL via UniFFI callback interfaces: Kotlin (`BluetoothLeScanner`/`BluetoothGattServer` in a foreground service, doc 09 §6) and Swift (CoreBluetooth with state restoration; overflow-area scan list per doc 09 §6 note). Scheduling hooks (doc 04 §5 ladder) surface as platform callbacks (battery/charger/Doze state in; duty-cycle decisions out). The Rust side never sleeps on its own timers on mobile — it requests wakeups (platform alarms), keeping Doze-compatibility honest.

## 7. Testing mandates (normative for the reference impl)

1. **Golden vectors** (`vectors/`): every frame & bundle type, with fixed keys — cross-checked by an independent decoder written from doc 10 alone (spec-ambiguity detector, like the doc 10 §8 lifetime catch). MUST include the post-audit negatives: (a) a signature minted under one signing-domain tag (doc 10 §3.1) presented under another → reject (AUDIT F4); (b) a non-canonical/overlarge-`S` Ed25519 signature → reject via `verify_strict` (AUDIT F9); (c) a bundle with `lifetime_ms` reset → identical BundleID, caught by SeenIndex (AUDIT F1); (d) a budget grant > `L_MAX` → clamped (AUDIT F3); (e) **rev. B cross-tag negatives (doc 10 §3.1):** a signature under any of `mmp1/manifest` / `mmp1/authcert` / `mmp1/announce` MUST NOT verify under any other tag (and vice versa) — this requirement was previously stated only in doc 10 §3.1 and tracked implementation-side in `implementation/mesh-milk/vectors/README.md`; it is now also normative here.
2. **Fuzzing** (`cargo-fuzz`): targets per parser — beacon, HS1, ID_PROOF, every post-auth frame, bundle canonical decode, TLV walker, RBSR range handler. Pre-auth targets run under allocation-counting harness (doc 10 §6.2: zero-alloc proof).
3. **Property tests** (`proptest`): canonical-encoding round-trip + rejection (mutate any byte → reject or different ID); RBSR convergence (random set pairs → equal stores ≤ R_max rounds); LWW merge commutativity/idempotence (doc 06 §4.2); OR-Set convergence (doc 06 §8); ratchet out-of-order windows (doc 08 §5).
4. **FSM invariants** I1–I4 (doc 11 §4) as transition-table exhaustive tests.
5. **Loom** (or equivalent) on store/session concurrency primitives in `mmp-core`.
6. **CI matrix:** `x86_64-pc-windows-msvc`, `x86_64-unknown-linux-gnu`, `aarch64-apple-darwin`, `aarch64-linux-android`, `aarch64-apple-ios`, `thumbv7em-none-eabihf` (build + size budget gate: `mmp-types+crypto+core+router` ≤ 256 KiB flash, ≤ 48 KiB static RAM ⚠ on Core profile), plus `cargo-deny`, layer-dependency test, MIRI on `mmp-types`.

7. **Diagnostics / trace schema (AUDIT F14).** A store-carry-forward network's hardest question is "why didn't this deliver?", so the reference impl MUST expose (behind a `diag` build flag, off by default, never on Core) a structured trace independent of the user-facing event stream (§5): (a) **per-bundle journey log** — `(BundleID, event ∈ {created, received, verified, stored, offered, handed_off(peer,budget), receipt_seen, purged, expired, evicted, dropped(reason)}, peer_nodeid?, monotonic_ts)`; (b) **contact log** — `(peer ephem_id→nodeid?, dial/accept, handshake_ms, domains_synced, bytes_in/out, close_reason)`; (c) counters for quarantine/clamp/drop reasons. Emitted as line-delimited records consumable by `mmp-cli inspect`; the same records feed `mmp-sim` assertions (doc 14 drills reference them). No protocol-surface change.

## 8. mmp-sim (the OQ-4/OQ-5 instrument)

Discrete-event simulator embedding the *real* `mmp-core`/`mmp-router` crates (not a re-implementation — the point is validating the shipped logic): synthetic radios with doc 02 Track B parameters (contact capacity 50–150 kbps, setup latency, loss), mobility models (random waypoint + SLAW-like social mobility), N ∈ {100, 1k, 10k}, adversary plugins (sinkhole, Sybil flood, eclipse) per doc 07 matrix. Outputs: delivery ratio, latency distributions, airtime/node, store occupancy, receipt-purge effectiveness — the numbers doc 13 needs to de-⚠ constants. Runs headless in CI (small N) and as a research tool (large N).

## 9. Persona review checkpoints

- **Performance Engineer — Potential flaw:** "snow + dalek + blake3 + chacha on thumbv7em may bust the 256 KiB Core budget." *Resolution:* size gate in CI from day one (§7.6); snow is the first candidate for replacement by a minimal hand-rolled XX (pattern is small) if the gate trips — flagged, not assumed. Accepted.
- **Security Auditor — Potential flaw:** "UniFFI boundary passes key material?" *Resolution:* no — `KeyVault` keeps identity secrets behind the trait; mobile implements it over Android Keystore/iOS Keychain (doc 08 §10); UniFFI surface carries only public keys, ciphertexts, and events. Signing happens Rust-side with vault-released ephemeral use or platform-side signing where keystores require it (both paths specified; per-platform choice documented). Accepted.
- **Skeptical Professor:** "Independent-decoder golden tests (§7.1) are expensive ceremony." *Resolution:* they already paid for themselves once (doc 10 §8 lifetime ambiguity was found by exactly this think-as-a-stranger exercise during spec writing); kept. Accepted.
- **Distributed Systems Reviewer:** mmp-sim embedding real crates risks sim-only code paths leaking in (cfg(test) routing). *Resolution:* sim injects only via the public trait seams (§2) — the same surface real TALs use; no cfg leakage by construction. Accepted.
- **Battery Specialist:** platform-alarm-driven wakeups (§6) quantize timers (doc 11 §7) to OS alarm granularity (~minutes in Doze). *Resolution:* every timer in doc 11 §7 tolerates ±1 alarm quantum except session timers (which only run while connected/awake anyway); annotated in the timer table semantics. Accepted.

## 10. Build order (implementation roadmap)

1. `mmp-types` + vectors + fuzz harness (the codec is the spec's truth).
2. `mmp-crypto` (suite 0x01, sealed/prekey, ratchets) + property tests.
3. `mmp-core` session+sync against in-memory TAL; `mmp-router`.
4. `mmp-sim` → **de-⚠ pass on doc 13 constants** (gate for everything below).
5. `mmp-store` redb impl; `mmp-cli` PC node (btleplug) — first real-radio contact.
6. `mmp-ffi` + Android FG-service app skeleton; iOS skeleton.
7. `mmp-tal-embedded` nRF52 Core profile + OQ-7 RS bench + size gate.
8. External crypto review window (OQ-10) before any v1.0 tag.

## 11. Cross-references

Wire truth → doc 10. FSM semantics → doc 11. Constants the sim must validate → doc 13 (every ⚠). Failure drills the implementation must pass → doc 14 §5. Licensing/IPR checks (RaptorQ exclusion already decided) → doc 02 Track E, OQ-6.
