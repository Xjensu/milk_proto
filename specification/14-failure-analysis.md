# 14 — Failure Analysis

**mesh_milk edge cases and recovery.** Status: Phase 5 draft; **revised 2026-06-11 post-audit** (`AUDIT.md`, `CHANGELOG.md`). FMEA-style register (§1) + deep walks of the five highest-risk scenarios (§2) + implementation drills (§3). Severity: how bad if unhandled; all rows list the *specified* handling and the residual.

> **Audit revision summary (this doc):** F4/F14 reference verify-on-sight + canonical-S/reassembly bounds; F15 now bounded by the copy-budget receive-clamp (AUDIT F3); new rows F21 (lifetime-resurrection, AUDIT F1) and F22 (prekey suppression, AUDIT F6b); W1/W3 note exact-purge and budget-clamp; drills D-2/D-7 extended (canonical-S/reassembly fuzz; OEM background-kill matrix, AUDIT F13).

---

## 1. Failure register

| # | Failure | Sev | Specified handling | Residual |
|---|---|---|---|---|
| F1 | Wall-clock jump (user/NTP) | M | ordering & aging use monotonic clocks only (03 §4.3, 11 §7); claims use seq not time (06 §4) | none — wall clock is never load-bearing |
| F2 | Monotonic clock loss across reboot (embedded) | M | lifetime decrement uses persisted boot-session anchor + elapsed; unknown gap → decrement by conservative max (gap treated as full holding time) | early expiry of carried bundles after sensor reboots — safe direction |
| F3 | Store corruption | M | per-record checksums; corrupt record → drop & re-fetch from mesh (04 §7) | loss of own undelivered originals if not yet sprayed — app retry |
| F4 | Quota exhaustion under spam | H | eviction order + per-origin buckets (04 §2, 05 §8) | churn of low-priority spray traffic; own/WoT traffic protected |
| F5 | Partition → merge (festival ends, two halves reunite) | M | RBSR is exactly an anti-entropy merge; exact D0 receipts purge duplicates (06 §6.3, AUDIT F11); claims LWW converge; OR-Set membership converges and **compacts below the mutual epoch anchor** (06 §8.5, AUDIT F5); group epochs: max wins, missed sender keys requested (08 §6.3) | burst of sync traffic at merge boundary — bounded by session airtime caps + receive-side budget clamp; transient duplicate spray copies |
| F6 | Sustained Sybil flood at quota limits | H | bounded: attacker airtime = origins_present × bucket rate; relay budget caps total (13 §7) | fairness degradation; honest traffic keeps WoT multiplier lanes |
| F7 | Dense crowd (1000 nodes, one hall) | H | anti-storm rules: busy flag backoff, adaptive beacon slowdown, connection caps (09 §4, 04 §5) | discovery latency rises (by design); delivery via mobility still works — **the failure mode is slowness, not collapse** (vs flooding cliff, F-A1) |
| F8 | All-T1 neighborhood (every phone backgrounded/iOS) | H | beacons still TX (Android bg) / connectable (iOS); any single T2/T3 entering range drains the neighborhood's queued traffic (store-carry) | latency until a relay passes by; honest limitation of OS policy (C-B2/B4) |
| F9 | Identity key loss | M | identity replacement (03 §3.3); new key, claims re-issued; contacts re-verify (07 §6.1) | history orphaned (old conversations unreadable by design); social recovery only |
| F10 | Identity key theft | H | thief = identity (threat tier); compromise indicator on conflicting claims (06 §4.2, 07 §6.4) warns both owner and observers | no cryptographic revocation without infrastructure — documented honestly |
| F11 | Friend disappears with buffered traffic | L | friends are caches not custody (04 §4.4); origin copies still spray; dependent re-friends | delay for the dependent's inbound; none for the network |
| F12 | Group admin(s) offline forever | M | messaging continues (sender chains independent); membership frozen (no epoch advance) (08 §6) | stale membership until admin returns; OQ-8 (v2 admin succession) |
| F13 | Version skew (v1 ↔ v2 nodes) | M | wire version fields everywhere; unknown classes relayed-if-signed, never processed (03 §8); critical TLVs gate processing | v1 relays still useful to v2 traffic; feature islands |
| F14 | Malformed input torture | H | canonical-reject rules + zero-alloc pre-auth parsers + **canonical-S verify (F9)** + **bounded reassembly (F10)** + fuzz mandate (10 §6, 12 §7) | implementation quality risk — mitigated by process, not protocol |
| F15 | Battery dies mid-handoff | L | phases idempotent; BUDGET follows BUNDLE (10 §2.5); **granted budget clamped to L_MAX on receive (03 §4.5, 05 §2.3.1, AUDIT F3)** + write-ahead intent under-sprays on crash (11 I3) | over-replication now double-bounded (clamp + under-spray direction); SeenIndex dedupes |
| F16 | BLE stack pathologies (Android vendor bugs, bonding prompts) | M | MMP never bonds/pairs (Noise above plain GATT — no SMP dependency); stack resets → TAL error → backoff (11 §3) | per-device quirks; field matrix testing (OQ-2) |
| F17 | RBSR pathological splits (crafted ID clusters) | L | IDs are BLAKE3 outputs — adversary cannot cluster the space without mining valid signed bundles (06 §3.5); depth capped by T_list | negligible |
| F18 | Blob never completes (publisher gone, < k symbols circulating) | L | partial caches serve (11 §6); interest expiry 7 d → ABANDONED; UI shows partial state | unavoidable — content with insufficient replication dies; RS makes it strictly rarer than uncoded (02 §5.2) |
| F19 | Eviction race (bundle evicted while session offers it) | L | offer re-validated at TRANSFER read; gone → skip, peer's WANT unmet this contact | one wasted offer entry |
| F20 | Beacon ephem_id collision (8-byte) | L | birthday at city scale: ~10⁴ concurrent IDs → P(collision) ≈ 10⁸/2⁶⁵ ≈ negligible; resolution failures just skip dial-score boost | none meaningful |
| F21 | Lifetime-resurrection replay (AUDIT F1) | M | captor resets a stored bundle's `lifetime_ms` to class-max and re-injects: BundleID excludes lifetime → identical ID → caught by **SeenIndex persisted from first sight for class-max** (03 §4.2); past that horizon, **per-class application dedup** (ratchet index / claim seq / receipt MAC / symbol coords) discards it | ≤ one quota-bounded relay store/offer cycle per resurrection; **no duplicate user-visible delivery** — content is authentic, just stale |
| F22 | Prekey suppression → FS downgrade (AUDIT F6b) | M | active relay-adversary withholds a victim's fresh prekey-batch claims, forcing senders onto a stale signed-prekey or the sealed-box baseline (08 §4.2); cannot forge a higher-seq claim, only eclipse newer ones | first-message forward secrecy degrades to the sealed-box floor (no-FS) under attack; **confidentiality never drops below baseline**; eclipse-class, detected by the receipt-starvation alarm (07 §6.3) |
| F23 | Stolen operator ROOT key (pointer only — rev. B, full analysis doc 18 §7 A6) | H | scope containment (a stolen root grants nothing outside its scope namespace); dual-control minting; recovery = new root + physical re-pin | **unbounded within the scope until re-pin** — the one entry in this register that is not merely bounded but genuinely open-ended pending an out-of-band recovery ceremony |
| F24 | Declared-priority queue-jump (pointer only — rev. B, full analysis doc 18 §7 A7) | L | none at wire-phase ordering in v1, stated plainly; priority buys order, never volume — per-origin buckets/quotas stay priority-blind | earlier-in-session service for junk within unenlarged budgets — a scheduling nuisance, not a safety breach; structural fix filed as a `CHANGELOG.md` errata candidate |

## 2. Deep walks

### W1 — Partition merge (F5)
Two halves of a 1k-node mesh, 48 h apart, reunite. Each side holds ~2 MiB live bundles (13 §3), largely disjoint; receipt filters disjoint. First contacts: SUMMARY exchanges show massive filter deltas → RECONCILE D0 first, where **exact authenticated receipts** (not Bloom hits — doc 06 §6.3, AUDIT F11) purge cross-partition duplicates *before* D3 re-spray (phase order, doc 03 §5.2, prevents the dumbest waste); spray budgets on re-offered bundles restart at residual values, each **clamped to L_MAX on receive** (AUDIT F3) so no merge-boundary peer can inflate re-spray; merge traffic spreads over many independent pair contacts (no coordinator, no thundering herd beyond per-session caps). Model: each boundary contact moves ≤ contact capacity; merge completes organically over hours. **Verdict: design handles it by construction; sim scenario S-5 (12 §8) must confirm no receipt-purge/respray livelock [S].**

### W2 — Stadium scenario (F7)
10k nodes, one venue. Beacon channel load: 10k × beacon/5 s = 2000 beacons/s across 3 adv channels → severe CRC-collision regime; discovery degrades to nearest-neighbors (highest RSSI survive capture). Adaptive response (04 §5): density estimate → beacon intervals stretch toward 10 s, burst scans rate-limited, session caps bind. Result: each node syncs with a handful of nearest peers; messages still diffuse — store-carry + crowd churn does the long-range work. Delivery latency p50 rises to multi-minute even same-venue [S]. **The protocol's promise under extreme density is degraded-but-alive — exactly the F-A1 lesson; sim scenario S-7.**

### W3 — Hostile neighborhood (F6 + battery-drain attack)
Adversary deploys 50 Sybil beacons + connect-and-stall sessions, and (where it relays others' bundles) hands out inflated copy budgets. Defenses fire in order: burst rate-limit (≤6/min) caps scan cost; handshake budget (5 s) bounds stall sessions; **inflated budgets are clamped to L_MAX on receive and aggregate amplification per origin is capped by per-origin token buckets** (AUDIT F3, doc 05 §2.3.1/§8); reassembly buffers are pre-bounded so a dribble-stream cannot pin memory (AUDIT F10); failed/useless sessions mark peers down in ContactGraph (observed usefulness, 04 §3) → dial_score → 0; hostile-neighborhood fallback fixes duty cycle at baseline. Steady-state cost ≈ baseline scan + occasional probe ≈ +1 mAh/day [A]. Honest traffic continues through any non-hostile contact. **Verdict: bounded annoyance; drill D-6 quantifies in sim with the doc 07 adversary plugins.**

### W4 — Sensor fleet through reboots (F2)
Watchdog-rebooting nRF52 (04 §7) loses monotonic continuity. F2 rule: persisted anchor → conservative decrement → carried bundles age out early (a rebooting sensor is a bad carrier; correct). Own telemetry bundles re-created fresh. Friendship survives (claims durable); poll schedule resumes. **Verdict: safe-direction failure; invariant fuzz: lifetime must never *increase* across persistence round-trips (test in 12 §7.3).**

### W5 — Two devices, one identity (user error: key copied to tablet+phone)
Both emit claims with independent seq counters → equal-seq conflicts → deterministic LWW + **compromise indicator fires on the owner's own devices** (06 §4.2) — the user discovers the misuse mechanism within one gossip cycle. Conversations: ratchet state forks (skipped-key windows absorb briefly, then MAC failures). Specified guidance: multi-device is **unsupported in v1** (doc 15 lists it); the failure is loud, early, and non-silent by design. **Verdict: acceptable v1 posture; the loud failure is the feature.**

## 3. Implementation drills (must-pass before v1.0, references 12 §7)

| Drill | Scenario | Pass criterion |
|---|---|---|
| D-1 | 10k-bundle store, power-cut loop ×1000 | zero corrupt records accepted post-recovery |
| D-2 | Frame fuzz 72 h CI cumulative (incl. non-canonical-S sigs, cross-tag sigs, over-long reassembly prefixes, lifetime-reset bundles) | zero panics/OOM/hangs; pre-auth zero-alloc proof holds; all AUDIT F1/F4/F9/F10 negatives reject |
| D-3 | Partition-merge sim (W1, S-5) | no livelock; merge p95 < 6 h at 1k nodes |
| D-4 | Stadium sim (W2, S-7) | delivery ratio ≥ 0.9 same-venue within 30 min ⚠ |
| D-5 | Clock torture (jumps, reboots, skew) | F1/F2 invariants hold; no lifetime inflation |
| D-6 | Sybil/drain adversary suite | honest-traffic delivery drop ≤ 10%; battery cost bounded per W3 |
| D-7 | iOS/Android background matrix **+ OEM background-kill matrix** (AUDIT F13) | leaf reachability per 09 §6 table on ≥3 device classes; T2 FG-service survival measured across ≥3 OEM battery-manager policies; honest T2→T1 degradation on kill |
| D-8 | Pointer only — rev. B, full definition doc 17 §8 (S-8, train profile) | train Profile A misdeployment check: a conforming one-anchor-per-carriage topology vs. a single-lead-anchor deployment must show ≥50% delivery-ratio degradation ⚠ in far carriages, confirming §3.1's minimum cardinality is load-bearing |
| D-9 | Pointer only — rev. B, full definition doc 17 §8 (S-9, campus profile) | delivery ratio ≥0.85 ⚠ same-building within 15 min ⚠ — tighter time, lower density than W2/D-4's stadium band |

## 4. Persona review checkpoints

- **Skeptical Professor — Potential flaw:** "W2's 'degraded-but-alive' claim has no number behind the capture-effect discovery assumption." *Resolution:* marked [S] with sim scenario S-7 and a falsifiable pass band (D-4); the qualitative mechanism (RSSI capture → nearest-neighbor graph remains connected under churn) is standard radio behavior, but the quantitative claim is explicitly deferred. Accepted.
- **Security Auditor:** F10's "no revocation" must be visible in user docs, not buried here — carried to doc 07 §6 UX duties (compromise indicator copy includes "create new identity" guidance). Accepted.
- **Distributed Systems Reviewer:** F15's "conservative-summed budgets" can inflate copies across repeated crashes (crash-loop amplification). *Resolution:* per-bundle hand-off log entry persisted *before* BUNDLE send (write-ahead intent); recovery treats unconfirmed hand-offs as completed (under-spray, not over-spray — same safe direction as F2). Register updated. **This review changed the spec** (doc 11 §4 I3 note extended).
- **Battery Specialist:** W3 steady-state (+1 mAh/day) verified against §13 ledger composition. Accepted.
- **Performance Engineer:** D-3's 6 h merge p95 is achievable per W1 arithmetic only if boundary contact density ≥ ~50 contacts/h; rural splits may exceed — band annotated as urban-calibrated ⚠. Accepted.

- **Security Auditor — Potential flaw (post-audit, AUDIT F1):** "F21 says resurrection is caught by SeenIndex-from-first-sight, but what bounds SeenIndex memory?" *Resolution:* SeenIndex is time-bucketed Blooms sized in doc 04 §8 / doc 13 §3 (~700 KB at 10k for the 72 h window); persisting from first sight does not change its footprint (the window length is the same, only the anchor moves from current-lifetime to first-sight). The per-class application dedup backstop (F21) means even a Bloom FP or an aged-out ID cannot produce a duplicate user-visible delivery. Accepted.
- **Distributed Systems Reviewer — Potential flaw (post-audit, AUDIT F3):** "Does the receive-side budget clamp interact badly with the write-ahead under-spray (F15/11 I3)?" *Resolution:* no — they compose in the safe direction: the clamp caps any single adopted grant at L_MAX, and crash recovery treats unconfirmed hand-offs as completed (under-spray). Worst case is fewer copies, never an amplification. Accepted.

## 5. Cross-references

Mechanisms invoked: docs 03–11 as cited per row. Sim scenarios S-5/S-7 + adversary plugins → doc 12 §8. Unfixable classes (jamming, full eclipse) remain in doc 07 §7 residual register. Multi-device, revocation, admin succession → doc 15 research directions. Audit findings driving this revision → `AUDIT.md`; edit map → `CHANGELOG.md`.
