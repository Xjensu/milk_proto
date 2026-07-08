# 05 — Routing Specification

**mesh_milk routing layer.** Status: Phase 2 draft; **revised 2026-06-11 post-audit** (`AUDIT.md`, `CHANGELOG.md`). Implements R-C1 (doc 02 §3): bounded replication via Binary Spray-and-Wait, steered by locally-observed utility, with receipt back-propagation. No flooding data plane, no DHT, no geographic routing. Constants ⚠ are provisional until doc 13 (OQ-4 simulation).

> **Audit revision summary (this doc):** §2.3 adds the **receive-side copy-budget clamp** to `L_MAX=32` (AUDIT F3); §7.2 notes that **beacon-summary-driven offer-suppression is a ≤~100-node optimization** (AUDIT F8b); §11 corrects the copy-inflation defense row.

---

## 1. Overview

Routing answers three questions at each contact: *what to offer* (reconciliation set), *what to hand off and at what copy budget* (spray), and *what to keep or evict* (buffer policy). There is no route computation; there are only per-contact decisions from local state.

A bundle's journey: origin creates it with copy budget `L` → spray phase distributes copies along the utility gradient → wait/focus phase: carriers hold until they meet the destination (or a strictly better carrier) → delivery → a signed receipt back-propagates and purges residual copies.

## 2. Copy budget (Binary Spray-and-Wait with Focus)

2.1. Each *carried copy* of a bundle has a local integer `budget ≥ 1` (per doc 03 §4.5: session-channel metadata, not part of the signed bundle).

2.2. **Origin budget:** `L = clamp(4, 2·⌈log₂(N̂+1)⌉, 32)` ⚠, where N̂ is the estimated population (§3). Direct messages to a node with strong predictability (§4) MAY start lower (L=4); group bundles use L per *scope* (§9).

2.3. **Spray (budget > 1):** on contact with an eligible peer (§5), transfer the bundle with `peer_budget = ⌊budget/2⌋`, retain `budget − peer_budget` (binary split — provably near-optimal expected delay among spray strategies [C3]).

2.3.1. **Receive-side budget clamp (normative, AUDIT F3):** a node receiving a BUDGET grant (doc 10 §2.5) MUST clamp it to `L_MAX = 32` ⚠ before adopting it, exactly as lifetime is clamped on receive (doc 03 §4.3). Rationale: copy budget is session-channel metadata a *relaying* (non-originating) adversary could inflate to make **honest** downstream peers over-spray on their own airtime — the clamp caps any single grant at the protocol ceiling, and per-origin token buckets (§8) cap aggregate amplification per origin. A grant exceeding `L_MAX` is clamped silently (not an error — the giver may be an old or buggy peer, not necessarily hostile).

2.4. **Focus (budget = 1):** hand the single copy (and stop carrying) only if `U_peer(dest) > U_self(dest) + θ_focus` (θ_focus = 0.15 ⚠) — Spray-and-Focus single-copy utility forwarding; otherwise carry until expiry/delivery.

2.5. **Direct delivery** always wins: if the contacted peer *is* the destination (or its friend, doc 04 §4), transfer regardless of budget and emit/collect the receipt.

## 3. Population estimate N̂

`N̂` = count of distinct resolved peers + distinct unresolved ephem_ids (collision-discounted ×0.75 ⚠) observed in a 24 h sliding window, EWMA-smoothed (α=0.25/day ⚠). Sensors (MMP-Core) skip estimation and use fixed L=8 ⚠. N̂ is never exchanged — population claims from peers are a poisoning vector (Security review, doc 02 §3.3).

## 4. Utility model (locally observed only)

For a contact peer `c` and destination `d`:

```
U_c(d) = max( P_c(d), σ·tie_c(d) )        σ = 0.5 ⚠
```

**Encounter predictability** P_c(d) — PRoPHET-style, direct observations only:
- On each contact with d:  P ← P + (1−P)·P_init   (P_init = 0.5 ⚠)
- Aging per hour:          P ← P·γ                (γ = 0.98/h ⚠)
- **No transitivity.** PRoPHET's transitive update imports third-party claims — our route-poisoning surface — and is removed (doc 02 §3.3). Cost: slower convergence for never-met destinations; mitigated by tie term and by spray's blindness being adequate at small N.

**Social tie** tie_c(d) ∈ [0,1] — from the local ContactGraph: normalized function of (contact frequency, cumulative contact duration, web-of-trust edge if d is a verified contact of c *as attested by c's own signed claims that we chose to trust*). Tie data is only what the node itself measured or explicitly trusts (moderation web, fixed requirement) — BUBBLE Rap's insight without its global centrality estimation [C5].

**Bounded state:** P-table only for destinations in active conversations (≤256 ⚠ entries, LRU); tie state bounded by ContactGraph (≤4096 peers, doc 04 §2). Everything else: U = 0 → pure spray. This is the answer to the 10k-destination state explosion (doc 02 §3.3 review).

## 5. Eligibility and admission

A peer is **spray-eligible** if: session authenticated; not the bundle's origin; not already in the seen/receipt set for that bundle; its relay admission not exhausted (§8); and (budget>1 spray) `U_peer ≥ U_self − θ_spray` (θ_spray = 0.3 ⚠ — mostly-blind spray, mild bias against strictly worse carriers) or peer tier ∈ {T2,T3} with fresher mobility (heuristic: more novel ephem_ids seen per hour than self ⚠).

Inbound admission (am I willing to carry this?): verify signature (doc 03), within quota, origin not rate-limited (§8), class allowed for my tier (T1 leaves accept only own-destined + friend traffic).

## 6. Transfer ordering within a contact

Contacts die unpredictably; order by expected value density:
1. Class 0 control (receipts, claims-of-contact) — tiny, high network value.
2. Destination-here bundles (direct delivery).
3. Spray transfers: priority class asc → size asc → remaining-lifetime asc (urgent small first — Battery review, doc 02 §3.3).
4. Blob symbols (requested only).
Within (3), per-bundle preemption: never start a transfer that doesn't fit the contact-duration estimate when something smaller is queued (estimate from TAL link-quality hints + historical contact length for this peer; default pessimistic 8 s ⚠).

## 7. Receipts and purge

7.1. On delivery, the recipient creates a **receipt**: class 0 bundle, payload = delivered BundleID + BLAKE3 MAC under the conversation key (proof-of-delivery to origin; doc 08), lifetime 6 h, budget L_receipt = 8 ⚠, sprayed normally with a bias along the *reverse* utility gradient (toward origin).

7.2. Every node folds verified receipts into its **ReceiptFilter** (rotating time-bucketed Blooms, 1% FP ⚠). Effects: refuse re-admission; advertise filter fragments in beacons (doc 03 §5.1) so neighbors *deprioritize* offering — the "implicit ACK" mechanism (doc 02 §4.3 Inventive-Scientist note), recovering MaxProp's ACK benefit at Bloom cost [C2]. **Purge authority is exact, not probabilistic:** a stored bundle is *purged* (eviction class 2, doc 04 §2) only on an **exact receipt match** reconciled in session domain D0 (doc 06 §6.3, AUDIT F11) — never on a Bloom hit alone, since a 1 % Bloom false positive would otherwise delete an *undelivered* copy. The Bloom is a suppression hint everywhere; the exact D0 receipt set is the purge trigger.

7.2.1. **Beacon-summary suppression is neighborhood-scale only (AUDIT F8b).** The 12-byte beacon summary fragment (doc 09 §3.3) is a ≈96-bit Bloom; at 1k+ live BundleIDs it is false-positive-saturated and carries little discriminative "what I hold/delivered" signal. Implementations MUST NOT rely on beacon summaries for offer-suppression or purge at scale; treat them as a useful optimization at ≤~100 nodes and fall back to in-session ReceiptFilter union (§7.2) + RBSR (doc 06 §3) elsewhere. Contact *selection* at scale rests on novelty + RSSI + tier (doc 04 §6), not summary content.

7.3. Bloom FPs can suppress an undelivered bundle (1% of offers); harmless because suppression is per-pair and other carriers still spray; origin re-sends on receipt timeout (app policy). Filters are *hints from authenticated peers*, never from beacons alone (beacons only deprioritize offers, never purge — doc 03 §5.1.2).

## 8. Fairness, quotas, anti-abuse

- **Per-origin token bucket** at every relay: default 32 KiB/h ⚠ per origin key, burst 128 KiB. Sybil cost: each new key starts at default; keys in the node's web-of-trust (direct or friend-of-friend) get multiplier ×4 ⚠ — socially-close traffic wins under contention (fixed moderation requirement), strangers still pass at base rate (openness preserved).
- **Relay airtime budget** per hour per node (doc 04 §5) — caps total charity.
- **Per-conversation in-flight cap:** an origin SHOULD NOT have more than 64 ⚠ undelivered bundles per destination (backpressure to apps).
- Blocked identities (local moderation): drop at admission, no carry, no error revealed.

## 9. Group dissemination

Group bundles (class 2) destinate a GroupID (random 128-bit, not derivable from membership). Members carry with elevated priority; non-member relays treat them as ordinary spray traffic toward *scope*, with budget L_group = clamp(8, member_count_estimate, 48) ⚠ managed per-scope rather than per-member (one spray tree serves all members; receipt = per-member, purge only when the *carrier's known members* are covered or lifetime ends). Detailed member-set semantics → doc 06; sender-key crypto → doc 08.

## 10. Scalability estimates (preliminary; full model in doc 13)

Per-message airtime cost ≈ L transfers (spray) + receipt overhead ≈ L+8 bundle transmissions ≈ (L+8)·size. At 1 KiB messages:

| N | L | per-msg network cost | routing state/node | notes |
|---|---|---|---|---|
| 100 | 14 | ~22 KiB | ContactGraph ≤100×64 B + P-table ≈ 10 KB | comfortably within budgets |
| 1,000 | 20 | ~28 KiB | ≤4096×64 B ≈ 256 KB + Blooms ~600 KB | fine on phones |
| 10,000 | 28 | ~36 KiB | capped — same as 1k (graceful: caps bind) | delivery ratio depends on mobility mixing; OQ-4 |

Contrast epidemic: per-message cost ∝ N (≈10 MiB at N=10k for the same message) — the cliff we avoid [C1][C2]. Key honest caveat: spray delivery *delay* grows when mobility mixing is poor; minutes-tier latency is the requirement and simulation must confirm it at 10k (OQ-4).

## 11. Security considerations (summary; full treatment doc 07)

| Attack | Mechanism | Defense |
|---|---|---|
| Sinkhole | advertise high utility to attract copies, then drop | utility is *locally observed only* (§4) — an attacker can't claim utility, it must actually exhibit contact behavior, which costs presence; spray keeps ≥⌈L/2⌉ copies on other paths |
| Blackhole relay | accept and delete | same as sinkhole + observed-usefulness scoring (doc 04 §3) deprioritizes silent peers |
| Copy inflation | clone bundles / inflate granted budget for *relayed* traffic | **receive-side budget clamp to L_MAX (§2.3.1) + per-origin token buckets (§8)** — corrected bound (AUDIT F3): for non-originated bundles the damage is *honest downstream* airtime, not the attacker's, so the real cap is per-origin quotas at each honest relay, not "cooperative field" reasoning; the clamp caps any single grant |
| Spam flood | many bundles from fresh keys | per-origin buckets + WoT multiplier (§8); verify-on-sight via inline origin key, no store-before-verify (doc 03 §4.1.2, AUDIT F2); store quota + eviction order |
| Receipt forgery | fake receipts purge real traffic | receipts MAC'd under conversation key — only true recipient can produce (§7.1); relays verify origin-signature on the receipt bundle itself |
| Replay | re-inject old bundles | SeenIndex + lifetime clamp (doc 03 §4.2, §11) |
| Beacon spam | fake summaries / drain via burst-scan | beacons are hints only; burst rate-limit (doc 04 §8) |

## 12. Persona review checkpoints

- **Skeptical Professor — Potential flaw:** "Removing PRoPHET transitivity guts its value; what remains is barely better than blind spray for far destinations." *Resolution:* accepted as true and intended — for far/never-met destinations we *rely on spray*, which published comparisons show is near-epidemic in delivery [C1][C3]; utility only accelerates the common case (recurrent social contact). The design degrades to its provably-OK floor, not to a poisoned gradient.
- **Distributed Systems Reviewer — Potential flaw:** "Per-scope group budget (§9) under-replicates when members are geographically split: one cluster absorbs the budget." *Resolution:* real; mitigated by member-carriers re-spraying at elevated priority (members are origins-by-proxy for their cluster), and group receipt semantics not purging until local members covered. Flagged for OQ-4 simulation scenarios with partitioned groups.
- **Battery Specialist:** receipt spray (8 copies per delivery) doubles control traffic per message at small N. *Resolution:* receipts piggyback in SUMMARY phase (they're filter entries, not separate transfers, between filter-synced peers); standalone receipt bundles only spray toward origin-gradient peers. Accepted.
- **Security Auditor:** WoT multiplier leaks social-graph hints to relays observing quota behavior (timing side channel). *Resolution:* acknowledged as residual metadata leakage within threat tier (local adversary learns coarse "this relay favors that origin"); documented in doc 07 §metadata budget; full mitigation (uniform quotas) available as a paranoid build flag at fairness cost.
- **Performance Engineer:** §6 pessimistic 8 s default contact estimate wastes long contacts' tails when history is absent. *Resolution:* estimate doubles on each completed transfer within the same contact (slow-start-like). Accepted.

## 13. Cross-references

Copy-budget wire encoding → doc 10. Session phase ordering → doc 03 §5.2. ReceiptFilter structure → doc 06. Quota/WoT interaction with identity → docs 07–08. Simulation validation plan → doc 13 (OQ-4, OQ-5).
