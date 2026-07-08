# 04 — Architecture Document

**mesh_milk system architecture.** Status: Phase 2 draft; **revised 2026-06-11 post-audit** (`AUDIT.md`, `CHANGELOG.md`). Complements doc 03 (normative wire behavior) with system structure: components, storage, node tiers, the friendship mechanism, and power management. Maps 1:1 onto the Rust crate plan (doc 12).

> **Audit revision summary (this doc):** §5 gates **burst-scan to FG/foreground tiers** and documents the **Android FG-service survival** deployment risk (AUDIT F7, F13); §6 notes **beacon-summary contact-selection is FP-saturated at scale** (AUDIT F8b).

---

## 1. Component architecture

```
┌─────────────────────────────────────────────────────────┐
│ Applications (chat UI, sensor firmware logic)            │
├─────────────────────────────────────────────────────────┤
│ Service layer                                            │
│  Messaging ─ Profile/Claims ─ Groups ─ Blob ─ Receipts   │
├─────────────────────────────────────────────────────────┤
│ Sync engine          │ Router                            │
│  RBSR, claim LWW,    │  spray budget, utility scores,    │
│  summary builder     │  queues, quotas, eviction         │
├──────────────────────┴──────────────────────────────────┤
│ Bundle core:  verify → dedupe → store → age → expire     │
├─────────────────────────────────────────────────────────┤
│ Contact manager: peer cache, session scheduler,          │
│                  friendship agent                        │
├─────────────────────────────────────────────────────────┤
│ Session layer: Noise handshake, AEAD channel             │
├─────────────────────────────────────────────────────────┤
│ TAL: BLE (beacon engine + GATT streams)   [WiFi-D, UWB]  │
├─────────────────────────────────────────────────────────┤
│ Platform: radio HAL, monotonic clock, RNG, storage HAL   │
└─────────────────────────────────────────────────────────┘
```

Control flow on a typical contact: `beacon_scan` hit → Contact Manager scores the peer (novelty, last-sync age, battery state) → decides dial/skip → Session layer authenticates → Sync engine runs SUMMARY/RECONCILE → Router selects TRANSFER set and spray budgets → Bundle core ingests inbound (verify-before-store) → Service layer surfaces app events.

**Inversion of dependencies:** Router and Sync never touch radios; TAL never touches bundles. Everything between them speaks two narrow traits (`Stream`, `BeaconPort`) — this is what makes PC/phone/`no_std` sensor builds share one core (doc 12).

## 2. Data stores

| Store | Contents | Bounds (Std/Core ⚠) | Persistence |
|---|---|---|---|
| BundleStore | bundles by ID, indexed by (class, dest, expiry) | 128 MiB / 32 KiB | durable |
| SeenIndex | BundleIDs seen, time-bucketed | covers max lifetime / 4 KiB ring | durable |
| ReceiptFilter | rotating Blooms of delivered IDs (doc 05 §7) | 64 KiB / 1 KiB | durable |
| ProfileStore | claims by (NodeID, field) → (seq, value, sig) | 16 MiB / own keys + friends only | durable |
| ContactGraph | per-peer: encounter stats, tie strength, predictability | ≤4096 peers × 64 B / ≤16 peers | durable |
| PeerCache | ephem_id resolutions, session resumption tickets | RAM | volatile |
| ReasmBuf | transient partial-frame reassembly (AUDIT F2/F10; replaces the old key-unknown Quarantine — origin pubkey is now inline, so bundles verify on sight) | ≤16 KiB / 4 KiB | volatile |

Eviction (BundleStore, in order): expired → receipt-confirmed delivered → over-quota by (priority desc, size desc, remaining-lifetime asc). A node MUST NOT evict its own undelivered originals before expiry.

## 3. Node tiers and roles

| Tier | Class | Radio duty | Typical |
|---|---|---|---|
| T0 Sensor | MMP-Core | beacon + poll friend on its own schedule | nRF52/ESP32 |
| T1 Leaf | MMP-Standard, constrained | beacon-RX/poll only; no relay duty | iOS background [C-B2]; any phone <20% battery |
| T2 Relay | MMP-Standard | full duty cycle, store-carry-forward | Android w/ foreground service [C-B4]; iOS foreground |
| T3 Anchor | MMP-Anchor | continuous scan + listen; storage anchor | PC/Linux box, plugged-in phone |

Tier is **self-declared in the beacon (2 bits) but treated as a hint**; actual behavior is what peers measure (a "T3" that never relays is just a bad peer — ContactGraph tracks observed usefulness, not claims). Tiers shift dynamically: battery, charger state, OS mode.

## 4. Friendship (T0/T1 ↔ T2/T3)

Adapted from Bluetooth Mesh Friend/LPN [F-A6], decentralized — no provisioner:

1. A T0/T1 node solicits friendship from an encountered T2/T3 via a signed FRIEND_OFFER exchange inside a session; both store the relationship (a mutual Claim).
2. The friend **buffers** bundles destined to (or wanted by) the dependent: it answers RECONCILE on the dependent's behalf using the dependent's last summary, and resolves ephem_ids for it (doc 03 §11 sensor cap).
3. The dependent polls its friend on its own schedule (sensor: e.g. every 5 min ⚠; iOS leaf: on app foreground) — a poll is an ordinary, cheap session limited to SUMMARY+TRANSFER.
4. Friendships expire (24 h ⚠) unless renewed; a dependent MAY hold up to 3 friends; bundles for a dependent are NOT considered delivered (no receipt) until the dependent itself acknowledges — friends are caches, not custody transfer [F-A5 lesson: no custody timers].

**Security note:** a friend sees the dependent's summary (what it holds/wants) — a deliberate, consented metadata disclosure to a chosen peer; strangers learn nothing (doc 07 §metadata).

## 5. Power management

Battery budget <5%/day on phones drives the duty-cycle scheduler:

- **Beacon TX:** adaptive 2–10 s interval ⚠ — densest observed neighborhood → slowest beacons (collision + energy); empty neighborhood → slow + scan-biased (nothing to announce to).
- **Scan:** slotted, target duty ≤2.5% ⚠ baseline; burst to continuous for 30 s after any beacon hit (contact exploitation — contacts are rare and precious; energy goes where information is). **Burst is foreground / Android-FG-service only (AUDIT F7):** iOS background and Android-without-FG cannot sustain a 30 s continuous scan, so those tiers fall back to opportunistic-on-app-open. Burst-scan is the first lever to cut (before beacon interval, before baseline scan) if the whole-phone energy multiplier `k_sys` measures hot (doc 13 §1.2, OQ-13).
- **Session budget:** relay airtime token bucket per hour (doc 05 §8) so a hostile neighborhood cannot make a phone burn its battery relaying garbage (DoS-by-charity).
- **Battery ladder:** >50%: declared tier honored; 20–50%: relay budget halved; <20%: drop to T1; charging+WiFi-class link: T3-like behavior allowed.
- Android: relay duty requires the foreground service; without it the node honestly declares T1 [C-B4]. iOS: background = T1 always; the app schedules opportunistic foreground work (user opens app → burst sync) [C-B2].

**Deployment risk — FG-service survival (AUDIT F13).** The T2 relay tier *assumes* an Android foreground service stays alive, but 2025-era OEM battery managers (notably aggressive on some vendors) kill background/FG services unpredictably; users also swipe them away. The protocol degrades correctly (a vanished T2 honestly becomes T1, and the mesh leans on remaining T2/T3 + mobility), but **relay availability is OEM-policy-dependent and may silently thin out** — this is a deployment precondition to measure, not a protocol bug. The build SHOULD surface relay-active state to the user and SHOULD request battery-optimization exemption where the platform allows. Field validation belongs in an extended doc 14 drill (D-7 OEM background-kill matrix).

Quantitative model (idle/discovery/sync/relay mA·h breakdown) is doc 13's deliverable; the scheduler knobs above are its inputs.

## 6. Scheduling policy (Contact Manager)

Contacts are the scarce resource; the scheduler maximizes *information value per joule*:

`dial_score(peer) = novelty (new ephem_id or stale last-sync) × tier hint × link quality (RSSI) × own budget state`

At 1k+ nodes the beacon `summary` fragment is false-positive-saturated and contributes little to dial scoring (AUDIT F8b): contact selection rests on novelty + RSSI + tier, and "what the peer holds/delivered" is learned exactly only in-session (RBSR + ReceiptFilter union, doc 06). Beacon-summary signal is a ≤~100-node convenience, not a scale mechanism.

- Suppress re-dialing the same peer within T_redial (60 s ⚠) unless it advertises a new epoch (summary changed).
- Prefer one good contact over three marginal ones (BLE connection setup cost amortization, doc 02 Track B).
- T3 anchors accept all inbound dials; T2 caps concurrent sessions (2 ⚠); T0 never accepts inbound except its friends.

## 7. Failure containment

- Each layer fails closed: TAL loss → Contact Manager retries/backs off; session abort → idempotent phases resume next contact (doc 03 §5.2); store corruption → per-record checksums, drop-and-redownload (bundles are replicated by design — the network is the backup).
- Watchdog on `no_std` targets: protocol task overruns → radio off + clean restart (doc 11 state machine).
- Clock anomalies: only monotonic clocks are trusted for aging; wall-clock jumps never delete data (doc 03 §4.3).

## 8. Persona review checkpoints

- **Battery Specialist — Potential flaw:** "Burst-scan-on-beacon-hit invites a battery-drain attack: beacon spam forces everyone into continuous scan." *Resolution:* burst trigger rate-limited (≤6 bursts/min ⚠) and ephem_id-novelty-gated; repeated unknown ephem_ids without successful handshakes mark the neighborhood hostile → fixed low duty. Accepted; attack documented in doc 07.
- **Skeptical Professor — Potential flaw:** "Self-declared tiers are gameable." *Resolution:* §3 — tiers are scheduling hints; ContactGraph records *observed* behavior (sessions completed, bytes usefully exchanged) and that, not the claim, feeds dial_score and relay quota. Accepted.
- **Distributed Systems Reviewer — Potential flaw:** "Friend-answered RECONCILE risks serving stale summaries → wasted transfers." *Resolution:* friend marks proxy answers with the summary's age; peers discount accordingly; dependent's own poll reconciles truth. Bounded waste, accepted.
- **Performance Engineer:** SeenIndex covering 72 h at city scale is the hidden memory hog — at 10k nodes × 50 msg/day ≈ 500k IDs × 16 B = 8 MB raw; time-bucketed Blooms (1% FP) cut it to ~600 KB on Standard, and Core tiers only track what they can hold anyway. Sized in §2; exact structure in doc 12.
- **Security Auditor:** friendship metadata disclosure (§4) is consented and peer-scoped — approved, with the requirement that FRIEND_OFFER UX must say what the friend learns (carried to doc 07 §UX).

## 9. Cross-references

Wire behavior → doc 03. Routing math/quotas → doc 05. Energy model → doc 13. State machines for tier transitions, friendship lifecycle, session phases → doc 11. Crate boundaries mirroring §1 → doc 12.
