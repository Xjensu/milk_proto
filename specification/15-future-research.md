# 15 — Future Research Directions

**mesh_milk open problems and v2+ directions.** Status: Phase 5; §6 appended in the extension phase (rev. B, 2026-07-03). Ordered by leverage (impact ÷ research risk) within each section. Each entry: problem, why v1 excluded it, research path. OQ-* cross-references → research/open-questions.md.

---

## 1. High leverage, near-term

**R1 — BLE-parameterized DTN simulation as a published artifact (OQ-4/OQ-5).** The routing literature's evaluations assume WiFi-class contacts; mmp-sim (doc 12 §8) with doc 02 Track B parameters would be a *contribution in itself* — publishable as "DTN routing under real BLE constraints," and it de-risks every ⚠ constant. Path: implement, validate against published ONE-simulator baselines, release scenarios + data. *Rev. B addendum:* the simulator now also hosts the extension-phase scenarios S-8 (train: line-graph carriages, anchor rail, conductor-ferry trace) and S-9 (campus per-floor cells) with drills D-8/D-9 (doc 17 §8, `ROADMAP.md` Stage 4).

**R2 — Whole-phone energy ground truth (OQ-13).** No literature measures *system-level* cost of duty-cycled BLE mesh participation on modern Android/iOS. A measurement paper (Battery Historian / Instruments methodology, ≥3 device classes, k_sys decomposition) fills a real gap and decides our lever ordering.

**R3 — External crypto review + formal modeling (OQ-10).** Tamarin/ProVerif models of: serverless prekeys (08 §4.2), channel binding (08 §3.2), opportunistic DH ratchet (08 §5.2). The Noise Explorer methodology [F3] extends naturally; the opportunistic-ratchet security definition ("PCS upon contact") may itself be a novel formalization worth publishing.

**R4 — Adaptive spray parameter learning.** L = f(N̂) is crude. Research: online estimation of mobility mixing time from local contact statistics (inter-contact distribution tails) → per-bundle L tuned to observed regime. Constraint discovered in this project: estimator inputs must be *locally observable only* (the doc 05 §4 poisoning lesson applies to any learned input). *Rev. B addendum:* the council's CR-A4 finding reframes the input as an **encounter-rate EWMA** (rate of *distinct-peer* contacts per unit time) rather than a population estimate N̂ — same locality constraint, better-behaved under the relay-scarcity regime CR-A3 identified (`COUNCIL-REVIEW.md`).

## 2. Protocol extensions (designed-for, not designed-yet)

**R5 — WiFi Direct / WiFi Aware TAL.** The TAL seam (03 §7) takes a second transport unchanged; the research questions are platform-political (Android Aware availability, iOS Multipeer constraints) and scheduling (when to burn WiFi power for a 250× throughput contact). Blob plane benefits most. *Rev. B addendum:* doc 16 caps file transfer at 8 MiB precisely because BLE's goodput floor makes larger transfers dishonest to promise (doc 16 §1/§8); files above that cap are this R-item's first concrete workload.

**R6 — UWB TAL + ranging-secured contacts.** UWB gives distance-bounded sessions (relay attacks physically excluded) — interesting for verification ceremonies (07 §6.1) more than throughput.

**R7 — Extended-advertising opportunistic upgrade.** Where both peers support BLE 5 extended adv (Track B: Android-modern only), beacons could carry full summaries (254 B) — cutting SUMMARY costs to zero for capable pairs. Design constraint: must remain *purely additive* (legacy 31-byte beacons stay the universal floor, C-B2).

**R8 — Privacy profiles (OQ-9).** Sealed-origin bundles, per-conversation ephemeral destination tags, and the quantified routing-utility cost of each. Bridges toward (but does not reach) the [A22] anonymity tier.

**R9 — Hybrid PQ suite 0x02 (OQ-11).** Track hybrid-Noise standardization; the open question is handshake size vs BLE contact budgets (ML-KEM-768 adds ~2.3 KB to a ~1 KB handshake — tolerable [A]-level arithmetic, but prekey claims inflate 6×, pressuring claim gossip).

## 3. Harder research problems

**R10 — Decentralized group key agreement under partition (OQ-8 + 08 §6.5).** Sender keys concede PCS; MLS concedes partition-tolerance. The DCGKA literature (Weidner et al. lineage) targets exactly this gap but assumes authenticated causal broadcast — which our conversation DAG approximates. Mapping DCGKA onto MMP's DAG + OR-Set membership is a publishable protocol-design problem, and admin succession (OQ-8) folds into it.

**R11 — In-network coding with verifiable recoding (doc 02 F-E2).** RLNC recoding was rejected because relays can't sign recoded symbols. Homomorphic authenticators (lattice/pairing-based) exist but cost orders of magnitude too much for nRF52-class nodes today. Re-evaluate when (a) hardware floors rise or (b) batched verification schemes mature. The §5.2 coupon-collector win is already captured at the source; recoding's residual upside is multi-source *generation mixing* — quantify before investing.

**R12 — Incentives without currency.** Relaying is charity bounded by quotas. Research: does observed-usefulness reciprocity (04 §3, already specified as scheduling input) suffice at city scale, or does free-riding dominate? Game-theoretic DTN literature is rich but assumes rational-agent models that crowd behavior contradicts; an empirical study on pilot deployments would beat more theory. *Rev. B addendum:* doc 17 §6 defers any urban-scale deployment profile until this item and R15 are answered — the pointer runs both ways.

**R13 — Multi-device identity (doc 14 W5).** v1 fails loudly on key sharing. Proper multi-device needs per-device subkeys under an identity root (certificate-chain claims) + ratchet-state synchronization between owner devices — essentially a private mini-mesh inside the mesh. Significant spec surface; defer until v1 field data shows demand.

**R14 — Internet bridging.** Optional gateways tunneling MMP bundles over the Internet between mesh islands (Reticulum does this transport-agnostically [A10]). Trivial technically (a TCP TAL), heavy *politically*: gateways become metadata concentrators and DoS targets — requires R8-level privacy work first. Explicitly out of v1 scope ("no Internet infrastructure" is the product).

**R15 — Scaling past 10⁴: hierarchical scopes.** Beyond city scale, flat BundleID reconciliation domains saturate. Direction: geographic/temporal scope tags (coarse, self-assigned, privacy-screened) partitioning reconciliation — the research risk is re-importing location leakage through the back door (the doc 02 geographic-routing rejection must keep holding). *Rev. B addendum:* gates a future urban profile together with R12 (doc 17 §6).

## 4. Standardization & ecosystem

- **SIG 16-bit UUID** (OQ-12) — production blocker, administrative.
- **RFC-track write-up:** docs 03+10 are deliberately RFC-shaped; an independent-stream Internet-Draft ("MMP: store-carry-forward messaging over BLE") would force the spec-ambiguity discipline further (the doc 10 §8 lifetime catch shows the value).
- **Test-vector conformance suite** as the ecosystem's interop anchor (12 §7.1).
- **RaptorQ IPR re-check** (OQ-6) before any v2 coding-scheme expansion.

## 5. Honest self-assessment (what this project would do differently)

1. The **SUMMARY cost** (20 KiB Standard) was discovered late (doc 13 §2 made it load-bearing); a v2 design should treat per-contact fixed costs as the primary budget from day one.
2. **Receipt semantics for groups** (05 §9) are the least-validated mechanism — per-member receipts over per-scope spray has subtle purge interactions; sim scenario priority.
3. The persona-review process caught real flaws (RPA linkage 09 §8, MLS mischaracterization 08 §6.5, crash-loop spray inflation 14 §4) — but **all by internal review**; nothing substitutes for external adversarial eyes (R3) and field pilots.

## 6. Extension-phase additions (rev. B, 2026-07-03)

R-items arising from the docs 16–18 extension work and the external multi-model council review (`COUNCIL-REVIEW.md`). Same entry discipline as §1–§3; novelty grades follow the council's re-graded (harsher) verdicts, not the original plan's.

**R16 — Contact-window-aware session admission (OQ-15).** Estimate *residual contact time* during a live session — RSSI slope as the v1 fallback (weak in metal-body multipath environments, doc 17 §2.3); BLE 6.0 Channel Sounding via Android 16 `RangingManager` [I11], measured at ±1 m to 20 m on Pixel 10 [I13], where hardware permits — and re-plan the in-session sync-phase order against the estimated deadline (which RBSR domains, whether to open BLOB phase, doc 09 §4 dial_score). Closest prior art: **Ott & Kutscher, "Drive-thru Internet," IEEE INFOCOM 2004 [H13]** — phased transfer across a contact window's entry/production/exit zones for vehicular WiFi. The honest claim is therefore a *fresh BLE composition* (radio-signal residual-time estimation driving sync-phase reordering under DTN semantics), not an unprecedented idea (COUNCIL-REVIEW N1 disposition). Research path: deadline-constrained scheduler formulation + trace-driven evaluation against an oracle that knows true window length. Gated by OQ-15 (Channel Sounding update rate unpublished, iOS support unknown). Feeds doc 17 §2.7 and doc 09 §4.

**R17 — Corridor (linear-topology) spray.** On a detected 1-D topology, split copy budgets *directionally toward unvisited ends* instead of the isotropic binary split (doc 05 §2.2) — doc 17 §7 carries this as an advisory profile knob for the train's anchor rail, where line position is provisioned rather than inferred. The open research part is **detection**: inferring "I occupy a corridor position" from locally-observable contact sequences alone (COUNCIL-REVIEW N2's weak point). Strengthening required before any novelty claim: a **line-graph optimality argument** — on a line graph, the isotropic binary split provably wastes copies on the already-visited direction; formalize the expected-delivery-delay gap. Prior art to differentiate against: MoVe (motion-vector forwarding), GeOpps, and ferry-based linear sensor networks (Jawhar et al., GLOBECOM 2013 [H2]).

**R18 — Profile calculus (minor pointer only).** Deployment profiles as versioned constant-vectors with drill pass-bands (doc 17 §1) are a spec-engineering discipline, not protocol science — the council downgraded this claim (N4) citing FODA feature models and 3GPP/Thread conformance-class prior art, and doc 17 §1.3 concurs. Recorded here only so the cross-reference resolves; no research program is proposed.

**R19 — Decentralized time / clock-skew resilience (CR-A6, unanimous HIGH).** TTL expiry, SeenIndex pruning, epoch rotation, and AuthCert `not_after` all lean on wall-clock time a partitioned mesh cannot establish or agree on. Research direction: prefer *elapsed-local-time* decrement (the doc 03 §4.3 lifetime clamp already has this shape) and hop-count-style TTL over absolute timestamps; bound SeenIndex by count rather than age; make group epochs event-ordered (causal) rather than time-ordered. Docs 17 §3.4 and 18 §6.3 already apply the pattern locally (receiver-local clocks, stated skew tolerance, expiry-first revocation); this item is the systematic sweep of the frozen suite the council asked for.

**R20 — Self-certifying key revocation over the DTN (CR-A7).** A gossiped, self-certifying "this key is retired" claim — signed by the key itself or by a pre-designated recovery key — with LWW-tombstone merge semantics, generalizing what doc 18 §6 does for AuthCerts to identity keys, group-admin keys, and exchange keys. Constraints: no CRL endpoint, no shared clock (R19), gossip-speed propagation as the only distribution channel; the ACPC activation-code pattern [J9] fails here because it assumes periodic infrastructure connectivity. Doc 18 §6 is deliberately a *scoped instance* of this general problem.

**R21 — Provable non-amplification and gossip-free misbehavior bounds (CR-N6 — the extension phase's headline theoretical direction; proposed independently by both novelty-seat models).** Two facets. **(a) Hint-plane non-amplification ⇒ bounded energy-DoS:** beacons/hints never create durable state (doc 09 §3, doc 05 §4), so an active local adversary's ability to drain victims' energy is bounded by the *authenticated-session admission rate*, not by beacon volume — a short theorem requiring no wire change. **Stated boundary, honestly:** the bound holds for the beacon plane; CR-A1 (per-origin token buckets are Sybil-defeated at session/verify admission) is where energy still leaks, so the theorem's value is precisely to isolate *where* the residual lives, not to claim the system DoS-proof. **(b) Gossip-free misbehavior visibility:** a receiver can flag anomalous per-hop dwell using only cryptographically-bound bundle fields (hop count) plus its own locally-observed contact-window statistics — no third-party accusations, consistent with the E1 reject-third-party-claims posture (Burgess et al., MobiCom 2007 lineage). Research path: state and prove (a) against the doc 05 §8 / doc 03 §4.1.2 admission pipeline; define (b)'s anomaly statistic and its false-positive behavior under honest-but-bursty mobility.

## 7. Cross-references

All OQ-1…18 live in research/open-questions.md with resolution paths (OQ-14–18 added in the extension phase). Implementation roadmap gates → doc 12 §10. Extension-phase R-item provenance: R16/R17 ← docs 17 + COUNCIL-REVIEW N1/N2; R19/R20 ← CR-A6/CR-A7; R21 ← CR-N6; consumers: doc 16 (R5), doc 17 (R1, R16, R17, R19), doc 18 (R19, R20). The five-phase document suite is complete with this document; doc 01 (executive summary) is finalized last, reflecting the whole; extension docs 16–18 (rev. B) build on, and never modify, the frozen v1 wire format.
