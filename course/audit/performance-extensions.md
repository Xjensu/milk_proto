# Audit: performance, failures, extensions, and research provenance

This audit is a course-facing traceability record, not a performance claim or an implementation result. `MUST`, `SHOULD`, and `MAY` retain the force assigned by the source specifications. `validated` means an arithmetic or protocol property is checked in the cited source; it does not mean field-tested. `simulated` is reserved for a result from the required simulator; none of the pass bands below has that status. Every `⚠` item remains unresolved.

## 13 — Performance Model

### Purpose

Quantify whether the DTN design can meet the battery, contact-capacity, load, sync, and latency goals, while labelling arithmetic `[A]`, simulation work `[S]`, and measurement work `[E]`.

### Normative invariants

The `<5%/day` phone target is a design target, not an achieved result. For contacts under 15 s, mini SUMMARY is REQUIRED; quotas, relay token buckets, and receive clamps remain the load bounds. Any changed input requires rerunning the model rather than reusing its conclusion.

### Physical or distributed-systems model

The model assumes intermittent BLE contacts, 50–150 kbps goodput, 20 messages/node/day, and spray cost growing with `L`, not directly with N. A phone's radio cost is only a component: OS wakeups are represented by `k_sys`; human mobility, not a closed form, controls delivery delay.

### Design choices and alternatives

It chooses spray-and-wait rather than epidemic forwarding, mini filters before expensive reconciliation, time-bucketed Bloom state, and burst-scan reduction as the first battery lever. Epidemic has lower expected delay but the documented 10k-node load is about 280× higher; full inventories were rejected because short contacts cannot afford them.

### Provisional or uncertain claims

`k_sys`, delivery latency, adversarial RBSR behavior, short-contact discovery sufficiency, connection parameters, and RS decode cost are unmeasured or simulated. The source document's `[4,12]` planning band conflicts with OQ-13's stale `[3,10]`; neither value is selected here. Same-district p50 ≤60 min is a simulator fail band, not a delivery guarantee.

### Dependencies and cross-references

Depends on docs 02, 04–06, 08–09, and 11–12; its operational failures feed doc 14. Evidence aliases include [B12]–[B14], [C1], and [C3]; the literal `[F-A6]` reference has no matching bibliography entry and requires provenance repair.

### Teaching implications

Students should recompute dimensional arithmetic before accepting a headline: controller mAh/day is not whole-phone percent/day, bytes/contact are not delivered bytes, and an arithmetic pass cannot validate mobility. Treat the model as a falsifiable worksheet whose assumptions are part of its result.

### Potential flaw / Alternative / Resolution

**Battery Specialist — Potential flaw:** controller-current proxies can understate phone cost. **Alternative:** declare the target met from radio arithmetic. **Resolution:** retain the multiplier, require device measurements, and use the specified lever order; do not strengthen “plausible” to “achieved.”

## 14 — Failure Analysis

### Purpose

Record specified recovery paths, residual risks, deep scenario walks, and mandatory pre-v1 drills for failures F1–F22.

### Normative invariants

Monotonic time governs ordering and lifetime aging; malformed input is canonical-rejected; verified data is bounded before storage; received copy budgets are clamped; crash recovery under-sprays rather than amplifies. Exact authenticated D0 receipts, not Bloom hits, authorize purge.

### Physical or distributed-systems model

The system may reboot, partition, merge, congest in a 10k-node venue, face Sybils and background-killed phones, or lose carriers. There is no coordinator, global clock, custody guarantee, or way to cryptographically revoke a stolen identity key without infrastructure.

### Design choices and alternatives

The design favors safe-direction degradation: conservative lifetime decrement, write-ahead handoff intent, quota eviction, idempotent phases, and partial blob caches. It rejects quarantine of unverifiable data and treats a conductor/key compromise, jamming, and full eclipse as bounded or explicit residuals rather than solved protocol states.

### Provisional or uncertain claims

W1 merge completion, W2 crowd delivery, W3 +1 mAh/day, and D-3/D-4/D-6/D-7 criteria remain model or measurement work. The claim that the F20 collision probability is negligible lacks a cited derivation in this document; it is a design assessment, not validated evidence.

### Dependencies and cross-references

Consumes docs 03–13 and sends drills to doc 12. Docs 16–18 add manifest parser, profile, and authority failure cases; their traceability remains required even though the base failure register predates them.

### Teaching implications

Use each row as a failure drill: identify state before the event, bounded resource, recovery direction, and residual. A “handled” row is not a zero-risk row; W2, W3, and F8 are useful exercises in separating survivability from latency.

### Potential flaw / Alternative / Resolution

**Distributed Systems Reviewer — Potential flaw:** a single conductor round can become a single point of slowness. **Alternative:** claim anchor rails give instant train-wide delivery. **Resolution:** retain store-carry secondary mobility and test a loose round-based band; no synchronous guarantee is implied.

## 15 — Future Research Directions

### Purpose

Maintain the v2+ research agenda and distinguish designed-for seams from designed-and-validated mechanisms.

### Normative invariants

Nothing in R1–R21 changes the frozen v1 wire format. External crypto review remains a prerequisite before a production security claim; urban deployment remains deferred until incentive and hierarchical-scope research is resolved.

### Physical or distributed-systems model

Research targets include BLE-constrained DTN contacts, partitioned groups, no shared time, local-only learning inputs, and optional infrastructure. The model never assumes a reliable Internet, shared clock, or globally trusted PKI.

### Design choices and alternatives

The roadmap prioritizes simulation and whole-phone measurement before WiFi/UWB transports, privacy, hybrid PQ, DCGKA, coding, incentives, multi-device, and Internet bridges. It explicitly distinguishes a fresh BLE composition from novelty claims already anticipated by message-ferry, drive-thru, and delegation work.

### Provisional or uncertain claims

R16 contact-duration estimation, R17 corridor detection/optimality, R19 clock resilience, R20 key revocation, and R21 bounds are research directions, not protocol guarantees. The “~2.3 KB” ML-KEM handshake increment and “~1 KB” baseline are planning arithmetic only; no source-derived derivation is supplied here.

### Dependencies and cross-references

All OQ-1–OQ-18 reside in the open-question file. R1 consumes docs 12–14 and 17; R5 consumes doc 16; R16–R21 arise from docs 17–18 and council findings. Sources [F3], [H2], [H13], [I11], [I13], and [J9]–[J16] are registered separately.

### Teaching implications

Have learners label every proposal as “seam,” “hypothesis,” “measurement,” or “standardization task.” A future-work list is responsible only when its rejected alternative, validation method, and blocking uncertainty are visible.

### Potential flaw / Alternative / Resolution

**Skeptical Professor — Potential flaw:** a broad roadmap can read as evidence. **Alternative:** silently promote extensions after implementation. **Resolution:** retain gates, prior-art qualifications, and OQ links; no item becomes normative merely by appearing here.

## 16 — File Transfer Application Layer

**Course placement: elective.** It is traceable because it exercises opaque application payloads, blob integrity, quotas, and abuse controls, but it is not required to understand the base message protocol.

### Purpose

Specify signed photo/document manifests over the existing blob plane with no new relay-visible frame, TLV, bundle class, or blob FSM allocation.

### Normative invariants

`FileManifest` is signed in `mmp1/manifest` by its embedded `sharer`; relays do not parse AEAD-interior app data. Receivers MUST preserve legacy bodies on parse failure, MUST cap fields before allocation, MUST never auto-fetch a full file, and MUST verify blob symbols through the existing Merkle path.

### Physical or distributed-systems model

Files move over intermittent 50–150 kbps contacts through a shared quota and class-4 TTL. Thumbnails are recipient-only eager fetches; full files are explicit-user-action pull requests. Partial verified caches can serve but cannot guarantee an absent publisher's file completes.

### Design choices and alternatives

The chosen body tag avoids a network-wide flag day; separate thumbnail and file roots preserve eager-preview/lazy-payload policy. Rejected alternatives are a critical TLV, a new bundle class/frame, a codec-dependent progressive preview, and treating metadata as protocol-interpreted MIME.

### Provisional or uncertain claims

All extension constants marked ⚠ remain provisional, including the 8 MiB scope, 16-item album, 64 B MIME, 32 KiB thumbnail, 40% BLOB sub-budget, and k=64 planning example. The size cap is a BLE-honesty policy, not a measured usability threshold; OQ-18 must determine thumbnail quality.

### Dependencies and cross-references

Depends on docs 03, 05, 08, 10–11, 15 R5, and OQ-2/OQ-7/OQ-18. Its abuse residual inherits Sybil-defeated per-origin buckets; authority gating in doc 18 is intentionally not applied to ordinary peer files. Evidence aliases [K1]–[K18] cover precedent, landscape, coding, and parser hazards.

### Teaching implications

Teach application-layer extension placement: data inside AEAD may evolve without relay parsing, but it still needs canonical parsing, attribution, quota accounting, and a UI truthfulness rule for thumbnails. Contrast content integrity with semantic correspondence.

### Potential flaw / Alternative / Resolution

**Security Auditor — Potential flaw:** a signed thumbnail does not prove it depicts the file. **Alternative:** call the preview verified content. **Resolution:** describe attribution/accountability only, show sharer identity, and require first-open friction; semantic mismatch remains a residual.

## 17 — Deployment Profiles

**Course placement: case study.** Profiles demonstrate configuration governance and contact-model reasoning; they do not alter the base wire protocol.

### Purpose

Define named tuning vectors and drill bands for assisted/ad-hoc train and campus deployments without changing wire bytes.

### Normative invariants

Anything called a profile MUST use declared local tunables and composition only; it MUST NOT add a class, TLV, signing domain, or wire field. Profile A requires one T3 anchor per carriage; Profile B has no authority root and MUST render all announcements unverified.

### Physical or distributed-systems model

Carriages and floors are treated as RF-bounded cells. Intra-carriage contacts are long; platform dwell is useful; opposing-train contacts are shorter than setup and are not a transfer channel. A train's inter-cell path is a conductor ferry plus incidental mobility, not assumed 2.4 GHz bridging.

### Design choices and alternatives

Profile A uses an anchor rail and provisioned conductor; B honestly accepts leaf-heavy latency; campus repeats per-cell logic. The document rejects one-anchor-for-an-entire-train reasoning and defers urban scale. Corridor spray is advisory only, with provisioned position rather than general topology detection.

### Provisional or uncertain claims

OQ-14 leaves 2.4 GHz gangway loss unmeasured; OQ-15 leaves residual contact-time prediction unproven. D-8/D-9 and their bands are `[S]` targets, not results. The private class-1 telemetry priority extension is explicitly a profile-local broadening of doc 18, not a guarantee supplied by doc 18 itself.

### Dependencies and cross-references

Depends on docs 03–05, 09, 11, 13–16, and 18; it reuses F8 and extends doc 14 drills. Sources [H1]–[H13] and [I1]–[I14] support contact, RF, throughput, and ranging claims. OQ-14–OQ-18 remain active.

### Teaching implications

This is a case study in making deployment assumptions testable: name the cells, relay capacity, authority boundary, power source, and a drill that deliberately violates one topology assumption.

### Potential flaw / Alternative / Resolution

**Performance Engineer — Potential flaw:** dwell-capacity arithmetic treats a successful BLOB phase as given. **Alternative:** equate contact duration with transferable payload. **Resolution:** retain setup amortization as a simulator dependency; use D-8/D-9 only as provisional pass bands.

## 18 — Authority, Announcements, and Priority Admission

**Course placement: elective.** It adds an opt-in scoped trust anchor and must be taught after base identity, signed bundles, and local scheduling semantics.

### Purpose

Add scoped AuthCerts, public class-5 announcements, scoped destination labels, and receiver-local priority admission for infrastructure-assisted deployments.

### Normative invariants

An announcement may render official or receive elevated local admission only after a verifying, in-scope, unexpired AuthCert and available local budget. Profile B MUST render unverified. RBSR and transfer ordering MUST use the declared signed priority, never a receiver's effective clamp; priority bytes are never mutated on wire. Revocation-list receivers retain sticky verified entries until their `not_after`.

### Physical or distributed-systems model

Receivers are partitioned, have independent clocks and bucket state, and may learn a cert after an announcement. There is no online CRL or shared accounting. A compromised root can mint fresh certs within its scope until physical re-pinning; a compromised subject is bounded only per honest receiver and cert lifetime.

### Design choices and alternatives

The document chooses class 5 over pseudo-groups, D3 reuse over a new capability-negotiated domain, attached certs for latency, expiry-first revocation, and issuer whole-state lists over per-subject singleton claims. It rejects treating local accounting as cryptographic prevention, and it records the declared-priority queue-jump residual.

### Provisional or uncertain claims

`priority_budget`, 15 min skew tolerance, ≤12 h cert target, and `REVLIST_MAX=48` are provisional. The lemma is validated as an algebraic per-honest-receiver bound under its stated assumptions, not a global or adversary-enforced budget. OQ-16 and OQ-17 still govern provisioning, compromise detection, and revocation latency.

### Dependencies and cross-references

Depends on docs 03, 05–07, 10, 13, 15, and 17. It is a required traceability consumer for the course despite elective placement. Evidence aliases [J1]–[J16] cover alert threats, TESLA/CRL constraints, delegation, revocation, and offline-budget alternatives.

### Teaching implications

Teach the split between authenticity, authority, admission, display, and wire ordering. A valid signature is not a trustworthy root; a local clamp is not a global protocol fact; and a proved accounting bound can coexist with a root-compromise catastrophe.

### Potential flaw / Alternative / Resolution

**Security Auditor — Potential flaw:** a receiver can ignore its bucket. **Alternative:** claim Sybil-resistant priority is cryptographically enforced. **Resolution:** scope the lemma to honest receivers, retain the root and queue-jump residuals, and defer cryptographic rate credentials to research.

## Open-question ledger

| Item | Carried status and required resolution |
|---|---|
| OQ-2, OQ-4, OQ-5, OQ-7, OQ-13 | Phone throughput, BLE simulation, RBSR contacts, RS decode, and whole-phone energy require bench/simulator work. OQ-13's `[3,10]` value conflicts with doc 13's `[4,12]`; repair the question record before a gate. |
| OQ-6, OQ-8–OQ-12 | RaptorQ IPR, group succession, privacy, crypto review, hybrid PQ, and SIG UUID remain unresolved; none is closed by docs 13–18. |
| OQ-14–OQ-15 | Measure inter-carriage 2.4 GHz propagation and contact-duration estimation; current train/campus claims are provisional profiles. |
| OQ-16–OQ-17 | Validate QR/gossip provisioning, root-compromise detection, and issuer kill-list latency against cert expiry. |
| OQ-18 | Benchmark thumbnail codec/quality and eager dissemination cost; do not infer usefulness from the 32 KiB target. |

## Constant-validation ledger

Every value below states units, basis, dependencies, and status. “Design target” is not validation; “simulated” is intentionally absent until a simulator produces a result.

| Constant or calculation | Units and basis | Dependencies | Status |
|---|---|---|---|
| Phone battery 4,000 mAh; beacon 5 s; scan 30 ms/1.28 s; bursts 100×30 s/day; 50 sessions×15 s/day | mAh/day arithmetic from B12–B14 controller proxies | phone OS, doc 11 duty policy | validated arithmetic `[A]`; proxy inputs provisional |
| Radio subtotal 13.8 mAh/day = 0.35%/day | mAh/day and percent of 4,000 mAh; source calculation | listed radio assumptions | validated arithmetic `[A]` |
| `k_sys ∈ [4,12]`; total 1.4–4.2%/day | dimensionless multiplier; range is a post-audit planning band, no bibliography derivation | wakeups/foreground service | provisional `[E]`; OQ record conflict noted |
| T0 0.9 mAh/day; ~3 years | mAh/day and years; source arithmetic | CR2477 1,000 mAh, 2 s beacons, 96 polls/day | validated arithmetic `[A]`; battery premise design input |
| Goodput 50–150 kbps; contacts 5/15/60 s | kbps, seconds, KiB/MiB capacity arithmetic | OQ-2, handshake/SUMMARY | validated arithmetic `[A]`; goodput provisional |
| Handshake ~1 KiB; Standard SUMMARY ~20 KiB; mini ~0.7 KiB | KiB; document planning sizes | docs 06, 10–11 | design inputs; mini requirement normative for short contacts |
| M=20 messages/node/day; 1.32 KiB/bundle; L=14/20/28 for N=100/1k/10k | messages/day, KiB, node count | doc 05 L formula, TTL | validated arithmetic `[A]`; workload/design inputs |
| 572/728/936 KiB/day; 47/60/77 s at 100 kbps; 1.7/2.1/2.8 MiB store | KiB/day, s/day, MiB for 3-day TTL | previous load inputs, 128 MiB quota | validated arithmetic `[A]` |
| Epidemic at N=10k: ~260 MiB/node/day, ~280× spray | MiB/node/day and dimensionless comparison; doc 13 arithmetic | M, bundle size, forwarding policy | validated arithmetic `[A]`; not a field measurement |
| 600k IDs/72 h; ~700 KB SeenIndex | IDs, hours, KB | N=10k/load/retention | validated arithmetic `[A]`; sizing assumption |
| RBSR `n=100k`, `d=100`, `B_f=16`, `T_list=24`, 1–3 rounds, 2–20 KiB, ~300 ms | IDs, branches, bytes, ms | doc 06, RTT distribution | arithmetic `[A]`; adversarial behavior provisional `[S]` |
| Same-district p50 ≤60 min; D-3 p95 <6 h; D-4 ≥0.9/30 min; D-6 drop ≤10% | minutes/hours/ratio | mobility, contact graph, adversary | simulated pass bands/design targets `[S]` |
| D-1 10k bundles ×1000 cuts; D-2 72 h; D-7 ≥3 device/OEM classes | count, duration, classes | implementation harness | required drills; unexecuted |
| File ≤8 MiB; album ≤16; name ≤255 B; MIME ≤64 B | bytes/files; scope and parser bounds, no formal derivation | doc 16 parsing/quota | provisional design targets |
| Thumbnail ≤32 KiB; generic auto-fetch ≤1 MiB; class-4 TTL 72 h; quota 128 MiB | KiB/MiB/hours; thumbnail is OQ-18 target, other values reused | docs 03, 08, 16 | thumbnail provisional; reused constants retain source status |
| Symbol ~3,947 B; block ~246.7 KiB (`k=64`); 8 MiB ≈34 blocks | bytes/KiB/blocks; `4096−124−25`, then ceiling division | bundle overhead, RS parameters | derived planning arithmetic; k and redundancy provisional |
| File transfer 7.5–22.4 min; 19–56 contacts at 24 s; 1.6–4.5 min at 250–680 kbps | minutes, contacts, kbps | OQ-2, setup clearance, I14 phone→MCU limitation | arithmetic valid; planning/provisional |
| BLOB sub-budget ≤40% of 60 s = 24 s | percent and seconds; policy mirrors D3 | doc 11 queue state | provisional admission policy |
| Train poll 60 s vs 5 min; validity ≤12 h | seconds/minutes/hours; profile target for train power | power source, doc 18 | provisional |
| Dwell 30–120 s; 180–560 KiB then 0.7–2 MiB | seconds/KiB/MiB from goodput and 60 s cap | OQ-2/setup cost | provisional arithmetic `[S]` |
| Opposing contact 0.24–1.08 s (`2R/v`) | seconds from 10–30 m and 200–300 km/h | H2/H6, range/velocity assumptions | derived/provisional; ruled unusable by setup model |
| D-8: 8 cars, ≤2 rounds p95, ≥95%, ≥50% degradation; D-9 ≥0.85/15 min | cars, rounds, percentages, minutes | mmp-sim, profile topology | simulated design targets `[S]` |
| Campus floor loss `15+4(n−1)` dB; N≈30 | dB/model coefficient; secondary I9 | actual building/material/frequency | low-confidence provisional model |
| Class 5; fields 0x07/0x08; TLVs 0x81/0x82; D3 | identifiers, not physical quantities; allocated protocol constants | docs 03, 06, 10 | normative extension allocation; compatibility-dependent |
| `priority_budget` admissions/hour; `B≤β`; bound `B+βT≤β(T+1)` | admissions/hour and hours; documented lemma | honest receiver, distinct IDs, local clock | lemma arithmetic validated under assumptions; β provisional |
| `REVLIST_MAX=48`, ~2.5 KiB; skew 15 min | entries/KiB/minutes; document estimate/policy | claim overhead, clock, OQ-17 | provisional design targets |

## Extension placement in the course

| Extension | Placement | Why it remains traceable |
|---|---|---|
| Doc 16 file transfer | Elective | Tests application-payload evolution, blob integrity, quota, parser, and preview-trust boundaries. |
| Doc 17 deployment profiles | Case study | Turns environmental assumptions into composition and simulator-drill checks without changing the wire. |
| Doc 18 authority announcements | Elective | Introduces scoped infrastructure trust, local priority semantics, revocation, and explicit root-compromise residuals. |

## Research gaps requiring fresh web verification

Fresh verification is required before implementation or claims based on: current phone-to-phone BLE throughput and background behavior; whole-device energy; BLE RBSR/contact traces; nRF52 RS decoding; RaptorQ IPR; hybrid Noise/PQ maturity; Bluetooth UUID assignment; current inter-carriage 2.4 GHz measurements; Channel Sounding update rate and iOS support; file-transfer landscape/version claims; thumbnail codec quality; QR/root-compromise UX; revocation propagation; and all authority/alert or offline-delegation literature that may have changed since the bibliography access dates. This audit intentionally performs no web verification.
