# 13 — Performance Model

**mesh_milk quantitative validation.** Status: Phase 5 draft; **revised 2026-06-11 post-audit** (`AUDIT.md`, `CHANGELOG.md`). Arithmetic from doc 02 Track B measurements [B12][B13][B14]; every result is tagged **[A]** confirmed by arithmetic, **[S]** needs mmp-sim (doc 12 §8), or **[E]** needs empirical bench. Assumptions are explicit; change an assumption, rerun the sheet.

> **Audit revision summary (this doc):** §1.2 widens the `k_sys` planning band to [4,12] and names burst-scan as the first lever (AUDIT F7); §3 absorbs the **+16 B/bundle** from the inline origin pubkey (AUDIT F2) — still far inside budget.

---

## 1. Phone energy budget (<5%/day target)

**Assumptions:** 4000 mAh battery (typical 2024–26 mid-range); radio currents from nRF52-class measurements [B12] as proxy for phone BLE controllers (phones embed similar radios; SoC overhead handled separately — §1.2): advertising event ≈ 6 mA × 4 ms; scan/RX ≈ 8 mA while window open; connected ≈ 10–15 mA during active exchange.

### 1.1 Radio-only ledger (T2 relay, defaults from doc 11 §7)

| Activity | Math | mAh/day |
|---|---|---|
| Beacon TX @ 5 s avg | 17,280 ev × 0.024 mA·s | 0.12 |
| Baseline scan 30 ms/1.28 s | 2.34% × 8 mA × 24 h | 4.5 |
| Burst scans (~100/day × 30 s × 8 mA) | 24,000 mA·s | 6.7 |
| Sessions (~50/day × 15 s × 12 mA) | 9,000 mA·s | 2.5 |
| Relay transfers (§3: ~60–150 s airtime in sessions above) | included | — |
| **Radio subtotal** | | **≈ 13.8 ≈ 0.35%/day [A]** |

### 1.2 The honest dominant term: system overhead

Radio arithmetic is *not* the battery story on phones. Each scan callback / connection wakes the SoC; a foreground service holds process memory; JVM/Swift callbacks burn CPU. Published BLE energy literature measures controllers, not whole-phone deltas — no trustworthy multiplier exists in our bibliography. **Model:** total = radio × k_sys, with **k_sys ∈ [4, 12]** plausible (band widened post-audit, AUDIT F7, reflecting frequent-wakeup apps on real hardware) → **1.4–4.2%/day**. The <5% target is *plausible but no longer comfortable* and k_sys MUST be measured on real devices (**OQ-13 [E]**). **Lever order if k_sys runs hot (AUDIT F7):** (1) cut burst-scan first — it is the largest controllable wakeup term (§1.1, 6.7 mAh radio-only, SoC-dominated under k_sys) and is already foreground/Android-FG-only (doc 04 §5, doc 09 §3.2); (2) lengthen beacon interval; (3) shrink the baseline scan slot (last — it gates discovery); plus batch scan callbacks (doc 09 §6).

### 1.3 Sensor (nRF52 on CR2477, 1000 mAh, T0)

Sleep 3 µA (0.07) + beacon @ 2 s (0.52) + friend polls 96/day × 1.5 s × 8 mA (0.32) + sensor-data bundles (negligible at telemetry rates) ≈ **0.9 mAh/day → ~3 years** [A] — comfortably in coin-cell territory; matches the Bluetooth-Mesh-LPN-class lifetimes the friendship design imitates [F-A6].

## 2. Contact capacity

Goodput 50–150 kbps (C-B1) ⇒ per contact: 5 s → 30–90 KiB; 15 s → 90–270 KiB; 60 s cap → 0.37–1.1 MiB.
Fixed costs: handshake+BINDING ≈ 1 KiB; Standard SUMMARY ≈ 20 KiB (mini: 0.7 KiB) ⇒ **a 5 s stranger contact at 50 kbps spends ~70% of its budget on SUMMARY** → doc 06 §9's delta/mini-filter requirement is *load-bearing*, not an optimization [A]. With minis: 5 s contact ≈ 25–80 KiB usable ≈ 20–60 message bundles. Adequate [A].

## 3. Network load vs scale (msgs; blobs scale separately)

**Assumptions:** M = 20 sent messages/node/day; avg bundle **1.32 KiB** incl. receipt overhead and the **+16 B/bundle inline origin pubkey** (AUDIT F2; was 1.3 KiB — a ≈1.2 % bump that the table below already absorbs within rounding); spray cost ≈ (L + 8) transfers/message (doc 05 §10).

| N | L | transfers/node/day = M(L+8) | KiB/day | airtime @100 kbps | store high-water (3-day TTL) |
|---|---|---|---|---|---|
| 100 | 14 | 440 | 572 | 47 s | ~1.7 MiB |
| 1,000 | 20 | 560 | 728 | 60 s | ~2.1 MiB |
| 10,000 | 28 | 720 | 936 | 77 s | ~2.8 MiB |

All three orders of magnitude sit far inside quotas (128 MiB) and airtime budgets (relay token bucket) **[A]** — confirming doc 05 §10 and the central claim: *cost per message grows with log N (via L), not N*. Epidemic at N=10k would be ~260 MiB/node/day — 280× worse [A].
SeenIndex at 10k: 600k IDs / 72 h → time-bucketed Blooms ≈ 700 KB [A] (doc 04 §8 confirmed).

## 4. Sync arithmetic

RBSR: domain of n=100k IDs, diff d=100: rounds ≈ ⌈log₁₆(n/24)⌉ = 3; bytes ≈ Σ branch fan-out ≈ 3 × 16 × 12 B × active branches ≈ 5–20 KiB worst case, ≪ inventory exchange (1.6 MB) [A]; typical post-Bloom diffs (d<20) terminate in 1–2 rounds, <2 KiB [A]. Round-trips: 3 rounds × ~100 ms ≈ 300 ms of a 5 s contact — OQ-5 answered affirmatively at the model level, **[S]** for adversarial/edge distributions.
ReceiptFilter: 8192 receipts/bucket capacity vs §3 load (≤720 deliveries/day/node neighborhood-wide visibility) — saturation implausible by ~10× margin except under receipt-spam, which quotas bound [A].

## 5. Latency (the [S]-dominated question)

Store-carry-forward delivery time ≈ hops × inter-contact time. With human mobility (pause-times minutes–hours) and L-way spray: published spray analyses [C1][C3] show near-epidemic delay at a fraction of cost, and epidemic ≈ optimal. **Model expectation:** same-gathering (≤2 radio hops): seconds–minute; same-district with mobility mixing: tens of minutes; cross-city multi-day carry chains: hours — consistent with the "minutes acceptable" requirement for the common cases. **No credible closed-form exists for our exact stack; this is THE simulation deliverable (OQ-4) [S]** — the model above sets pass/fail bands for the sim (fail = p50 same-district > 60 min at N=1k under SLAW-like mobility ⚠).

## 6. Constant validation status (the de-⚠ ledger)

| Constant (doc) | Status |
|---|---|
| Beacon 2–10 s, scan 30/1280 ms (09/11) | [A] within energy budget; [E] k_sys check |
| L formula (05 §2.2) | [A] cost side; [S] delivery side |
| SUMMARY sizes / mini-filter mandate (06) | [A] — mini REQUIRED for <15 s contacts |
| RBSR B_f=16, T_list=24 (06 §3.2) | [A] model; [S] adversarial |
| Burst ≤6/min, sessions 50/day class (04/09) | [A] energy; [S] discovery sufficiency |
| Quotas 128 MiB / 32 KiB (03 §9) | [A] ample for msgs; blob policy governs real fill |
| Skipped-key windows 512/32 (08 §5) | [A] RAM arithmetic trivial; [E] Core RAM gate |
| RS k≤64 on nRF52 (08 §8) | [E] OQ-7 bench |
| Connection params (09 §5.3) | [E] phone-matrix (OQ-2) |
| Contact-estimate 8 s default (05 §6) | [S] |

## 7. Persona review checkpoints

- **Skeptical Professor — Potential flaw:** "Using nRF52 controller currents as phone proxies (§1) then admitting k_sys ∈ [3,10] makes the headline '<5% plausible' soft." *Resolution:* correct — which is why §1.2 names it the dominant term, creates OQ-13 [E], and lists the pre-specified levers; the claim is downgraded to 'plausible with measured-risk plan', not 'achieved'. Accepted as the honest ceiling of paper analysis.
- **Performance Engineer:** §3 assumes uniform relay participation; real meshes have hub nodes (anchors) absorbing multiples of the average. *Resolution:* anchors are mains-powered by definition (T3) and quota-elevated; battery-relevant tiers are protected by their own token buckets regardless of topology skew. Noted in §3 assumptions. Accepted.
- **Distributed Systems Reviewer:** §5's pass/fail bands are arbitrary without a requirement trace. *Resolution:* bands derive from the discovery decision ("minutes acceptable") applied to the common case (same-district); cross-city has no hard requirement. Documented. Accepted.
- **Battery Specialist:** burst-scan line (6.7 mAh) is the largest controllable radio cost — flagged as first lever if OQ-13 measures hot, before touching baseline scan (which gates discovery). Accepted, ordering noted in §1.2.
- **Security Auditor:** model assumes honest load; sustained Sybil spam at quota limits adds bounded relay airtime (per-origin buckets × origins present) — worst-case arithmetic belongs to doc 14 drill D-6. Cross-referenced.

## 8. Cross-references

Sim harness → doc 12 §8. Spam-load worst cases → doc 14. OQ-13 (whole-phone energy measurement) appended to research/open-questions.md. Source numbers: [B12][B13][B14][C1][C3] in research/bibliography.md.
