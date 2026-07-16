# 09 — BLE Transport Specification

**mesh_milk TAL for Bluetooth Low Energy.** Status: Phase 4 draft; **revised 2026-06-11 post-audit** (`AUDIT.md`, `CHANGELOG.md`). Implements the TAL contract (doc 03 §7) within the constraints quantified in doc 02 Track B. Constants ⚠ provisional until doc 13.

> **Audit revision summary (this doc):** §3.2 gates **burst-scan to foreground / Android-FG only** (AUDIT F7); §3.3 mandates **per-epoch summary re-salt** (AUDIT F8a); §5.1 specifies the **reassembly bound** (AUDIT F10).

---

## 1. Roles and primitives

BLE maps onto the TAL as: *beacon plane* = legacy advertising + passive/active scanning; *bulk plane* = GATT connection (primary) or L2CAP CoC (optional fast path, §5.4). Every Standard/Anchor node alternates Peripheral (advertise + accept) and Central (scan + dial) roles; Core sensors are Peripheral-mostly; T1 leaves are Central-mostly (iOS background cannot advertise usefully [C-B2]).

## 2. Service identity

- **MMP 16-bit Service Data UUID:** one well-known UUID from the SIG-assigned range is required for iOS targeted background scans [C-B2]. Until SIG assignment, development uses 128-bit UUID `mmp1` namespace ⚠; production assignment is a release blocker (recorded OQ-12).
- All MMP advertisements carry Service Data (AD type 0x16) under this UUID; all GATT sessions target the MMP primary service.

## 3. Beacon plane

### 3.1 Advertisement layout (31-byte legacy budget)

```
AD1: Flags (3 B: len, type 0x01, value)            — required by spec
AD2: Service Data 16-bit (4 B header: len, 0x16, UUID16)
     + Beacon payload 24 B (doc 03 §5.1):
       ver_role(1) ephem_id(8) epoch(1) summary(14)
Total: 31 B exactly.
```

No device name, no TX-power AD (saves bytes; RSSI suffices for proximity ranking). Scan-response: empty v1 ⚠ (a future 31-byte extension surface).

### 3.2 Timing

- **Advertising interval:** adaptive 2–10 s ⚠ (doc 04 §5); ±10% randomization (spec-mandated delay plus our own jitter) to prevent persistent collisions.
- **Scan:** slotted — window 30 ms / interval 1.28 s baseline ⚠ (≈2.3% duty); burst mode (continuous 30 s) on novel beacon hit, rate-limited (doc 04 §8). **Burst-scan is foreground / Android-foreground-service only (AUDIT F7):** iOS background cannot sustain a 30 s continuous scan (and Doze defers it on Android-without-FG), so on those tiers the burst degrades to opportunistic-on-app-open; the energy model (doc 13 §1.2) treats burst-scan as the first lever to cut if `k_sys` measures hot.
- **ephem_id rotation:** coarse_time_slot = 15 min ⚠ epochs; receivers accept ±1 slot (doc 03 §5.1.1). Rotation MUST coincide with a randomized private-address (RPA) rotation — otherwise the controller's address links consecutive ephem_ids, defeating §5.1.1 (Security Auditor catch, §8).

### 3.3 Summary field schedule

The 14-byte `summary` rotates contents by `epoch mod 4`: 0 → ReceiptFilter fragment, 1 → held-bundle Bloom fragment, 2 → claim-digest hash, 3 → WANT hint. Fragments are (offset, 12-byte slice) of the current structure with a 2-byte index header. Receivers accumulate fragments opportunistically; completeness is never assumed (beacons are hints, doc 03 §5.1.2).

**Per-epoch re-salt (normative, AUDIT F8a).** Each fragment's bytes MUST be salted with `slot_salt = BLAKE3("mmp1 beacon-salt" ‖ coarse_time_slot ‖ beacon_salt)` (doc 03 §5.1.1.1) so the slowly-changing underlying structure does not produce correlatable fragments across an `ephem_id` rotation; without this, a continuous co-present scanner bridges two `ephem_id`s by matching fragments, defeating the rotation. **Scale caveat (AUDIT F8b):** at 1k+ live IDs the 12-byte Bloom fragment is FP-saturated and near-useless for "what I hold"; per doc 05 §7.2.1 it is a ≤~100-node optimization — not a purge/suppression authority at scale.

## 4. Discovery → dial decision

1. Scan hit with MMP service data → parse beacon (fixed-size, no allocation — doc 07 §8 parser rule).
2. PeerCache lookup by ephem_id (resolve against known contacts via BLAKE3 trials, doc 03 §11).
3. dial_score (doc 04 §6) over (novelty, role bits, RSSI > −85 dBm ⚠ gate, own budget).
4. Dial: central connects, MTU negotiation → 247+ requested, PHY 2M requested, DLE on (doc 02 Track B: throughput levers [B1][B2]); proceed to session handshake (doc 08 §3) regardless of negotiated values (graceful on 23-byte MTU floors).

Anti-storm rule: if ≥3 ⚠ MMP centrals are connected or connecting to the same peripheral (observable via its busy flag in `ver_role` bit 7 ⚠), back off with randomized delay 1–8 s — dense-room connection storms are the BLE-mesh failure mode [A3].

## 5. Bulk plane

### 5.1 GATT service (primary path)

```
MMP Primary Service
 ├─ TX characteristic: Write Without Response (client→server frames)
 └─ RX characteristic: Notify (server→client frames)
```

Both directions carry the **session frame stream** (doc 10 §2): length-prefixed frames fragmented into ATT payloads of (MTU−3) bytes; reassembly by length prefix only — no per-fragment headers (stream semantics over ordered, reliable ATT writes/notifications). Write-without-response + notifications avoid per-PDU round trips, the documented throughput technique [B1].

**UUIDs (implementation-pinned, OQ-12 development placeholders pending SIG assignment):** this
spec deliberately did not pin characteristic UUIDs, leaving it to the first implementation —
`implementation/mesh-milk` has since done so and both radio crates (`mmp-tal-btleplug`, central;
`mmp-tal-peripheral`, peripheral) are cross-verified byte-equal against each other in CI, so any
second implementation MUST match them for interop:

```
Service:               6d6d7031-0000-4d4d-5031-6d6573686d6c
TX (client writes):    6d6d7031-0001-4d4d-5031-6d6573686d6c   (Write Without Response)
RX (server notifies):  6d6d7031-0002-4d4d-5031-6d6573686d6c   (Notify)
```

"Client"/"TX" and "server"/"RX" are named from the **central's** perspective throughout this
document and the implementation — a peripheral's GATT server exposes TX as writable-into (its
inbound direction) and RX as its outbound notify channel; do not re-derive the naming per role,
it inverts and causes exactly the kind of bug the peripheral implementation had to guard against
explicitly. These 128-bit UUIDs remain a placeholder pending Bluetooth SIG 16-bit assignment
(OQ-12) — swapping to an assigned UUID is a coordinated, versioned change across every
implementation, not a local edit.

**Reassembly bound (normative, AUDIT F10):** the reassembly buffer is pre-allocated to `MAX_BUNDLE = 4096` and is never grown by an attacker-supplied length prefix; a prefix > 4096 → immediate `ERROR + CLOSE` (doc 10 §6.4, doc 11 §3). A frame whose bytes stall past the session idle timer (doc 11 §7) is discarded — no cross-contact reassembly, no slow-dribble buffer pin (also bounded by the 60 s airtime cap, §7).

### 5.2 Flow control

Credit-based at the frame layer: each side grants N=8 ⚠ frame credits in HELLO and replenishes via CREDIT frames (doc 10) — prevents notification flooding of slow stacks (Android binder queues are the classic victim). TAL link-quality hints (negotiated MTU, PHY, observed goodput EWMA) feed the contact-duration estimator (doc 05 §6).

### 5.3 Connection parameters

Request: interval 15–30 ms ⚠, latency 0, supervision timeout 4 s ⚠ during active session phases; renegotiate to interval 50 ms ⚠ during BLOB streaming (energy/throughput balance); disconnect cleanly (CLOSE frame, doc 10) when phases complete — idle connections burn radio time both sides.

### 5.4 L2CAP CoC (optional fast path)

Where both peers support it (iOS 11+/Android 10+ per [B18]; flag bit in HELLO), the session MAY migrate to an L2CAP CoC (PSM advertised in HELLO) for the TRANSFER/BLOB phases. Evidence: throughput gain over well-tuned GATT is modest [B18][B1], so CoC is OPTIONAL — the win is stream semantics and fewer stack quirks, not raw speed. GATT remains the conformance baseline; CoC failures fall back transparently (idempotent phases, doc 03 §5.2).

## 6. Platform mappings

| Platform | Beacon TX | Scan | Peripheral GATT | Notes |
|---|---|---|---|---|
| Android (T2 w/ FG service) | ✔ legacy adv | filtered scan (MMP UUID) — survives bg throttling rules [B9][B10] | ✔ | batch scan callbacks; avoid >5 scan start/stop per 30 s [B9]; request `neverForLocation` assertion where applicable |
| Android (no FG service) | ✔ low rate | ~30 s windows, deferred by Doze | ✔ while process lives | honest T1-like behavior; doc 04 §5 |
| iOS foreground | ✔ (28 B usable) | ✔ targeted | ✔ | near-full participation |
| iOS background | ✘ effective (overflow area only) | targeted-UUID scans only; screen-state caveats [B4][B11] | ✔ (connectable) | T1 leaf: remains *connectable* — Androids/PCs can still dial it; CoreBluetooth state restoration mandatory |
| Linux/Windows PC | ✔ | continuous | ✔ | BlueZ/WinRT; T3 anchor defaults |
| nRF52/ESP32 | ✔ | windowed | ✔ | SoftDevice/NimBLE; Peripheral-mostly, friend-polling Central bursts |

**iOS leaf note:** because iOS-background devices stay *connectable peripherals* advertising the MMP service in the overflow area, dedicated relays (Android/PC) SHOULD include overflow-area-targeted scans in their cycle [B11] — recovering iOS participation that pure service-data scans would miss. This is the single most deployment-critical platform behavior (doc 02 C-B2).

## 7. Coexistence and airtime fairness

- Advertising duty ≤0.5% ⚠, scan duty ≤2.5% baseline ⚠ (doc 04 §5): MMP must be a good 2.4 GHz citizen (WiFi/BT-classic coexistence).
- Per-session airtime cap: 60 s ⚠ hard, then yield (re-dial allowed after T_redial) — prevents one pair monopolizing a peripheral's connection slot in dense scenes.
- Relay airtime token bucket (doc 04 §5) enforced at the TAL: budget exhausted → no new dials, accept-only.

## 8. Persona review checkpoints

- **Security Auditor — Potential flaw (caught in drafting):** "ephem_id rotation without RPA rotation is cosmetic — the BLE address links slots." *Resolution:* §3.2 now mandates synchronized RPA+ephem_id rotation; on stacks where RPA timing isn't controllable (some Android vendors), the implementation MUST detect address persistence and degrade honestly (report linkability in doc 07 #14 terms). Accepted.
- **Performance Engineer — Potential flaw:** "Credit-based flow control on top of ATT's own ordering adds RTT stalls if credits are too small." *Resolution:* credits sized to cover bandwidth-delay product at worst connection interval (8 × 244 B ≈ 2 KB in flight); CREDIT piggybacks on any frame. Modeled in doc 13. Accepted.
- **Battery Specialist:** 15–30 ms connection interval during handshake is energy-hot. *Resolution:* handshake is ≤3 round trips (~6 PDUs); the hot interval lasts <500 ms typical, then SUMMARY/RECONCILE proceed at the same interval only while frames are queued; renegotiate-up on idle >2 s ⚠. Accepted.
- **Skeptical Professor:** "Overflow-area scanning is reverse-engineered behavior [B11]; building a normative requirement on it is fragile." *Resolution:* downgraded to SHOULD with explicit OQ-3 dependency; conformance does not require it; system works (degraded for iOS-bg) without it. Accepted.
- **Distributed Systems Reviewer:** busy-flag anti-storm (bit 7) is an unauthenticated beacon bit an attacker can set to deter dials to a victim... but the attacker would need to forge the victim's ephem_id in its own advertisements, which only suppresses dials to *the attacker's* broadcasts, not the victim's. No vulnerability; noted in doc 07 #15 scope. Accepted.

## 9. Cross-references

Frame grammar carried over this transport → doc 10. Session/connection state machines incl. resumption → doc 11. Energy arithmetic for §3.2/§5.3 duty cycles → doc 13. OQ-12 (SIG UUID assignment) appended to research/open-questions.md. WiFi Direct / UWB future TALs → doc 15.
