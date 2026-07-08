# 17 — Deployment Profiles

**Named, versioned tuning vectors over the protocol's declared constants, validated by required drill pass-bands — never a wire-format change.** Status: extension-phase (rev. B) **FINAL**, 2026-07-03 (drafted 2026-07-02; Round 2-bis fix CR2bis-9 applied — §3.3 scoped to provisioned nodes; cross-references resolved in the rev. B consistency pass). Citations [H*]/[I*] in research/bibliography.md. Constants ⚠ provisional.

> A deployment profile is a named vector over doc 03 §9's ⚠ tunable constants (plus the handful of local-policy knobs doc 04/05/09/11/16 already leave to per-deployment choice) and a required composition of doc 03 §6 conformance classes across doc 04 §3 tiers, validated by a set of required doc 14-style drill pass-bands evaluated against the doc 15 R1 simulator — it never touches bundle layout, frame grammar, or a signing domain (zero-wire-change, §1.2). Three profiles are defined: **Train Profile A** (infrastructure-assisted, per-carriage anchor rail + operator authority, §3), **Train Profile B** (pure ad-hoc, no anchors/no authority, §4), and a **campus/building** profile (§5); urban scale is a pointer only (§6).

---

## 1. The profile concept

1.1. Formal shape:

```
Profile {
  name        : string        // e.g. "train-assisted-v1"
  version     : semver
  composition : set of (tier ∈ {T0..T3}, class ∈ {MMP-Core, MMP-Standard, MMP-Anchor})
                // doc 04 §3 × doc 03 §6, REQUIRED minimum cardinality per role
  tunables    : partial map over doc 03 §9's declared ⚠ constants, plus the
                local-policy knobs doc 04 §4/§5, doc 05 §2.1/§2.2, doc 09 §3.2/§7,
                doc 11 §7, and doc 16 §8 already leave to per-deployment choice
  drills      : set of {drill_id, pass_band}
                // required subset of doc 14 §3's drills + this doc's D-8/D-9 (§8),
                // evaluated against the doc 15 R1 simulator
}
```

1.2. **Zero-wire-change property (normative for anything calling itself a profile).** Every field in `tunables` is, by construction, a value doc 03 §4.5 already treats as local (lifetime/copy-budget on receive), doc 04 §5/§6 already treats as a scheduler input (beacon interval, scan duty, dial_score weighting), or doc 05 §2.1 already declares session-channel metadata never carried inside the signed bundle (spray strategy, copy budget). A profile therefore changes *only* what a node decides to do with contacts it already has — never what bytes cross the air. A proposed field that would require a new bundle class, TLV, or signing domain (doc 03 §8) is by definition not a profile field; it belongs in a doc 16/18-style extension instead. This is the same "extension-safe" test docs 16 and 18 apply to their own additions, one level stricter: those docs add nothing observable to legacy relays; a profile changes nothing observable at the wire at all, ever.

1.3. **Honesty about what this is (N4).** The council's verdict stands and is not contested here: a named, versioned constant-vector-plus-conformance-composition is methodology, not protocol science — prior art (FODA feature models, 3GPP/Thread conformance classes, TTCN-3 test suites) makes the pattern itself unremarkable, and networking reviewers will correctly read this doc as a well-organized appendix. What earns its keep is narrower and testable: a profile turns "retune some constants for your deployment" from prose an engineer might skim (or skip) into a declared, versioned object a drill can fail against. §1.4 gives the concrete case this doc uses to justify that claim rather than assert it.

1.4. **Worked misdeployment example.** An operator reading an informal "trains" appendix that says, in prose, "add BLE beacons through the train for coverage" could reasonably deploy **one** T3 anchor in the lead carriage of an eight-carriage train, reasoning that BLE's advertised range should span the consist. §2.2's evidence says otherwise — every inter-carriage bridge found in Track H research moves off 2.4 GHz specifically because it doesn't reliably cross a gangway [H4][H8] — but an informal appendix has no mechanism to force that fact into a go/no-go check; the deployment ships, and only field complaints ("carriage 6 never sees delay announcements") surface the gap. A **profile** makes the same fact a conformance requirement instead: Profile A's `composition` (§3.1) declares a minimum cardinality of one T3 anchor **per carriage**, and drill D-8 (§8) is defined precisely to fail — with a quantified delivery-ratio gap, not a shrug — a topology that under-provisions anchors relative to carriage count. This is N4's honest contribution: not a new mechanism, but a place for a fact like OQ-14's to become something a build pipeline checks instead of something a prose appendix hopes a deployer reads.

## 2. Contact models

2.1. **Intra-carriage co-presence (long).** Within one carriage, passenger and sensor devices are co-present for a journey segment — minutes to hours, far longer than any BLE session needs. This is not a scarce-contact regime; the ordinary duty-cycle scheduler (doc 04 §5) and adaptive beacon slowdown under density (doc 09 §3.2, the same mechanism as the doc 14 W2 stadium case) govern it without profile-level changes.

2.2. **Inter-carriage RF is unreliable at 2.4 GHz — treat each carriage as a separate BLE cell (load-bearing).** Every inter-carriage bridging system in Track H research uses a different band, never 2.4 GHz: TGV inter-car WiFi bridges at 5 GHz [H4], and commercial inter-carriage bridge products use 5.8/6.0 GHz directional links specifically to cross the gangway [H8]. Two on-point measurement papers on inter-car 2 GHz-band propagation exist but are paywalled — no directly quotable dB figure was obtainable [H11][H12]. **This is OQ-14, stated honestly: the qualitative conclusion (2.4 GHz does not reliably bridge a gangway) rests on converging indirect evidence and industry practice, not a measured attenuation figure for MMP's own band.** The profile-level consequence doesn't need the missing number — §3 treats each carriage as an independent BLE cell and routes inter-carriage delivery around RF rather than through it (§3.2).

2.3. **RSSI ranging is specifically weak inside a carriage.** [H3] measured BLE RSSI inside train carriages as **non-monotonic with distance** — metal-body multipath reflections mean a farther device can read stronger than a nearer one. This caveats the RSSI-slope contact-duration estimator (the v1 fallback for novelty N1, doc 09 §4 dial_score, doc 15 R16) specifically in this environment: a train interior is close to a worst case for that estimator, not a neutral one.

2.4. **Platform dwell (usable).** 30–120 s ⚠ at a typical stop. Against the doc 13 §2 contact-capacity model (50–150 kbps goodput), a 30 s dwell carries ≈180–560 KiB ⚠; a full 120 s dwell exceeds the 60 s hard session-airtime cap (doc 11 §7) and therefore spans up to two session windows, redial permitted immediately once the peer's beacon epoch changes (doc 04 §6), paying a second fixed SUMMARY/handshake cost (doc 13 §2: ≈1 KiB + ≈20 KiB) out of the second window — net ≈0.7–2 MiB ⚠ across a full dwell, not a linear doubling.

2.5. **Passing-at-speed (opposing train) — verdict: useless for transfer.** Contact-window duration at relative velocity `v_rel` and radio range `R` is `t ≈ 2R/v_rel`; two independent sources agree on this form [H2][H6]. At `v_rel ≈ 200–300 km/h` (55.6–83.3 m/s) and `R ≈ 10–30 m`: `t ≈ 0.24–1.08 s` ⚠. Compare against the doc 11 §7 handshake stall-kill budget (5 s) and doc 09 §8's own optimistic figure for the Noise XX handshake alone ("≤3 round trips… hot interval lasts <500 ms typical") — before BLE connection establishment (scan, connect, MTU/PHY negotiation) is even counted. **A sub-second contact cannot clear connection setup plus handshake, let alone reach SUMMARY.** This is CR-A5/CA4's short-contact finding — setup can exceed the whole contact, so ContactGraph logs a "contact" that delivered nothing — applied concretely: opposing-train contacts are not a degraded channel, they are not a channel. §3.4's station data-mule (a fixed T3 anchor met during a 30–120 s dwell) is the real train-to-operator egress path, not inter-train RF.

2.6. **Campus/building.** Density sits between the doc 03/09 baseline and the doc 14 W2 stadium extreme — a floor of a building, not a 10k-node hall. Interior propagation: ITU-R P.1238's office model gives floor loss `15 + 4(n−1)` dB and a path-loss exponent N≈30 for a 1.8–2.0 GHz office environment [I9] (secondary source, low confidence — no MMP-band-specific measurement). The practical consequence mirrors §2.2: wall/floor attenuation is large enough that a multi-floor building partitions into **per-floor cells**, the same "treat the RF-bounded unit as its own cell" pattern as a carriage, for the same underlying reason (§5).

2.7. **Channel Sounding is forward-looking, not v1-guaranteed (OQ-15).** BLE 6.0 Channel Sounding, exposed on Android 16 via a unified `RangingManager` [I11], has been measured at ±1 m accuracy to 20 m on a Pixel 10 against an nRF54L15 reference [I13] — but the update rate is unpublished [I3] and iPhone support is unknown. N1's channel-sounding contact-duration path (doc 09 §4, doc 15 R16) is a future strengthening, not something any profile here can assume; the RSSI-slope fallback is what v1 profiles actually get, weak as §2.3 shows it to be on trains specifically.

## 3. Train Profile A — infrastructure-assisted

### 3.1 Composition

Minimum required cardinality (conformance test, §1.4): **one MMP-Anchor (T3) per carriage** ("anchor rail") — this is the misdeployment check itself, not decoration. Plus: one MMP-Standard (T2) conductor/staff device per shift, holding an AuthCert subject key (doc 18 §2); MMP-Core (T0) sensors per carriage (door, HVAC, occupancy, tank levels) as friends (doc 04 §4) of that carriage's anchor; passenger phones participate as ordinary, un-provisioned T1/T2 nodes with no special role.

### 3.2 Inter-carriage delivery: the conductor as message-ferry, not RF

Because §2.2 rules out direct inter-carriage RF, Profile A's actual inter-carriage path is the conductor's own device physically walking the train during a shift — an ordinary DTN message-ferry [H1][H2] arising for free from an existing operational duty, not a mechanism this doc invents. Each carriage's anchor accumulates that carriage's telemetry and any announcements it has originated or reconciled; the conductor's device, on entering RF range of each carriage anchor in turn, runs an ordinary session (SUMMARY→RECONCILE→TRANSFER, doc 03 §5.2) and carries what it collects to the next carriage and, eventually, off the train (§3.4). **Honesty note:** full train-wide propagation of one carriage's announcement therefore takes on the order of one conductor round, not one radio hop — slower than a single-cell contact, faster than passenger mobility alone, and it degrades gracefully if the round is interrupted (idempotent phases, doc 03 §5.2; nothing here assumes the round completes).

### 3.3 Authority and telemetry classing

The operator root key (pinned via QR at boarding, gossip fallback — doc 18 §2.2) signs the conductor's AuthCert (`scope = train/<operator>/<service>`, doc 18 §4; `caps` bit0 ANNOUNCE set for delay/platform-change/safety announcements, doc 18 §3). This profile additionally issues a **second class of AuthCert**, one per carriage-sensor identity, `caps = 0` — no ANNOUNCE, no bit1 PRIORITY_EMERGENCY, which per doc 18 §2's own field semantics already caps such a subject at priority 1 (OPERATIONAL). Carriage telemetry — ordinary class 1 direct bundles (doc 10 §4.1) addressed to the conductor's identity, declared `priority = 1 OPERATIONAL` (doc 18 §5.1) — then clears an admission check instead of being silently dropped to `NORMAL` on receive.

**Scope note, stated plainly rather than assumed:** doc 18 §5.2's clamp and §4's scope-subscription machinery are written around class-5 public announcements; applying the same per-subject `caps`/clamp logic to gate a private class-1 telemetry bundle is a genuine broadening of doc 18's literal framing, not something its text guarantees. It is nonetheless within a profile's authority under §1.2's test: doc 18 §5.2 itself describes a **receiver-local** admission decision (never a wire-carried one), and this profile is exactly the mechanism by which every **provisioned** node in the deployment (anchors, the conductor device — the only nodes that receive or act on telemetry; passengers are un-provisioned by §3.1 and never apply this rule, consistent with §3.6's zero-reliance posture) agrees, in advance, to extend that local check to a second bundle class. §9's Security Auditor checkpoint records this explicitly so it is never mistaken for a silent doc 18 amendment. `AuthCert.priority_budget` for sensor subjects ⚠ sized to each carriage's telemetry rate (low: door/occupancy events, not a stream); `not_after` ≤ shift length (≤12 h ⚠, doc 18 §9/OQ-17), same as the conductor's own cert.

### 3.4 Station-egress data-mule

Telemetry's second consumer is the operator backend, reached only when a carriage anchor (or the conductor's device) is in RF range of a **station** T3 anchor during a platform dwell (§2.4) — a fixed-anchor contact, not a moving one, so it is the one train-related contact §2.5 did *not* rule out. What the station anchor does with delivered bundles beyond MMP's boundary (bridging to the operator's own backend network) is deployment plumbing outside this protocol's scope, the same posture doc 15 R14 takes toward Internet gateways generally. **No shared clock is assumed anywhere in this path (CR-A6):** telemetry lifetime uses the ordinary monotonic remaining-lifetime decrement (doc 03 §4.3), and AuthCert `not_after` is checked against each receiver's own local clock with the doc 18 §6 skew tolerance — the same doc 15 R19 no-shared-clock pattern docs 16/18 already apply to blob and manifest lifetime.

### 3.5 Friendship retune

T0 carriage sensors poll their carriage anchor at **`FRIEND_POLL_TRAIN = 60 s`** ⚠, down from the doc 04 §4 default (5 min ⚠), because door/HVAC state is operationally time-sensitive in a way the default's generic sensor assumption doesn't capture. **This retune is conditioned on the sensors being train-powered, not coin-cell** — unlike the doc 13 §1.3 CR2477 sensor the default is sized against; the Battery Specialist checkpoint (§9) makes this precondition explicit so it is never silently inherited by a battery-constrained, non-train deployment.

### 3.6 Leaf-fraction assumption (CR-A3)

Profile A states its assumption plainly, as CR-A3 requires of every profile: passenger phones are assumed **~100% T1-leaf-equivalent for relay purposes** — Profile A places zero reliance on passenger devices carrying telemetry or announcements. The anchor rail plus the conductor-as-ferry (§3.1–§3.2) *is* the "create relay capacity" answer CR-A3 asked for, independent of whatever fraction of passengers happen to run a foreground app or plug in. This is the profile's core structural claim, and §8's D-8 drill exists specifically to check it against an under-provisioned anchor rail (§1.4).

## 4. Train Profile B — pure ad-hoc

### 4.1 Composition and posture

No anchors, no operator root, no AuthCert chain. Every device is an ordinary, un-provisioned MMP-Standard/Core node; whatever T2 relay capacity exists is whichever passenger phones happen to be foregrounded/charging/Android-with-FG-service at a given moment — the CR-A3 baseline (~90% T1-leaf fleet, doc 04 §3/§5), unmitigated. No sensor telemetry class exists in this profile by construction (telemetry requires the operator infrastructure Profile A supplies).

### 4.2 Authority semantics

No trust root exists to pin or gossip against. Every announcement — self-issued or otherwise — carries doc 18's **"unverified" banner** (doc 18 §1, §7 row A4): priority 0/1 never admits under §5.2's clamp because no AuthCert chain roots anywhere trusted, so all class-5 traffic is effectively `NORMAL`. Any informal trust a rider extends to a sender ("this looks like a real conductor") is ordinary petname/web-of-trust context (doc 07 §6), never authority, and the UI MUST NOT blur that distinction (doc 18 §7 A4's residual: "depends entirely on the UX banner being non-bypassable").

### 4.3 Quantified degradation (honest)

Profile B inherits doc 14's **F8** failure row directly, unmodified: with a mostly-T1 carriage, "any single T2/T3 entering range drains the neighborhood's queued traffic (store-carry)" — delivery happens, but only in bursts gated by whichever passenger happens to be relay-capable at a given moment, with a residual of "latency until a relay passes by; honest limitation of OS policy" (doc 14 F8). §2.2's per-carriage-cell finding still holds without an anchor to exploit it, so cross-carriage delivery in Profile B rests on whatever passenger mobility (walking to the dining car, detraining, boarding) happens to provide — materially worse and much less predictable than Profile A's conductor round (§3.2), stated here rather than left implicit.

## 5. Campus/building profile

Composition mirrors Profile A's per-cell logic without requiring uniformity: an org MAY deploy one T3 anchor per floor (the anchor-rail pattern at §2.6's per-floor granularity) and MAY layer doc 18's AuthCert chain for building-wide alerts (fire, lockdown) exactly as Profile A layers it for delay/platform-change — both are the same mechanism at a different physical cell shape, not a new design. Where no anchor is deployed, the profile degrades to Profile B's posture per floor. Density sits between the doc 03/09 baseline and the doc 14 W2 extreme (§2.6); no train-specific mechanism (conductor-as-ferry, station data-mule) applies — a campus has no scheduled physical ferry analogous to a conductor's round, so cross-floor delivery, absent an anchor rail, rests on ordinary passenger-equivalent mobility (people taking the stairs) alone. Tuning vector: §7.

## 6. Urban-scale pointer (deferred)

City-scale deployment (10⁴+ nodes, no single operator, no bounded physical topology) is out of this doc's scope — it is not a profile this doc can responsibly define with current evidence. Doc 15 R12 (incentives without currency at city scale) and R15 (hierarchical scopes past 10⁴) are the open research questions a future urban profile would need answered first; this doc records the pointer and stops.

## 7. Tuning tables

Baseline = doc 03 §9 defaults (unmodified where a cell reads "—").

| Constant | Baseline | Profile A (train, infra) | Profile B (train, ad-hoc) | Campus/building |
|---|---|---|---|---|
| BEACON_INTERVAL (03 §9) | 2–10 s adaptive | — | — | — |
| SCAN_DUTY (03 §9) | ≤2.5% | — | — | — |
| SESSION_AIRTIME_CAP (11 §7) | 60 s | — | — | — |
| Spray L (05 §2.2) | clamp(4, 2⌈log₂(N̂+1)⌉, 32) | — (sensors: fixed L=8 ⚠, doc 05 §3) | — | — |
| Corridor-spray (N2, **advisory**) | isotropic split (default) | ADVISORY: directional split toward unvisited carriage-ends on the anchor rail's line graph — doc 15 R17, low–med novelty confidence, detection mechanism open | not applicable (no anchor rail) | not applicable (no declared line topology) |
| Blob sub-budget (16 §8) | ≤40% of session cap | — | — | — |
| Friendship poll, T0 (04 §4) | 5 min ⚠ | **60 s ⚠** (§3.5, train-powered sensors only) | not applicable (no sensors in-profile) | 5 min ⚠ unless building sensors, then per-deployment |
| SUMMARY tier (06 §9, 13 §2) | mini REQUIRED <15 s contact | full Standard SUMMARY default-affordable (dwell/co-presence ≥15 s typical, §2.1/§2.4); mini mandate still applies to any actual <15 s contact | mini more often load-bearing (contact length less predictable without anchor infra) | full SUMMARY default-affordable (floor co-presence) |
| AuthCert `priority_budget` (18 §2) | n/a | conductor ⚠ sized to announcement rate; sensor subjects ⚠ sized to telemetry rate (low) | not applicable (no AuthCert) | ⚠ per building policy |
| AuthCert `not_after` (18 §9) | n/a | ≤ shift length (≤12 h ⚠) | not applicable | ⚠ per building policy |

Corridor spray is **advisory and non-normative**: spray strategy is already session-channel-local, never signed (doc 05 §2.1), so retuning it is squarely inside a profile's authority (§1.2) — but its detection mechanism (how an anchor knows it occupies a line-graph position rather than a hub) is unresolved in the general case (COUNCIL N2 finding). Profile A sidesteps the open part: each carriage anchor's position on the line is a **provisioned fact** (which carriage it's bolted to), not something it must infer from contact statistics — making the train deployment a candidate first concrete instance of R17's optimality argument rather than a test of the general detection problem (Inventive Scientist checkpoint, §9).

## 8. New drills and sim scenarios

| Drill | Scenario | Pass criterion |
|---|---|---|
| D-8 | Train Profile A end-to-end: 8 carriages ⚠, one T3 anchor/carriage, one conductor-mule round, sensor telemetry + one train-wide announcement + one passenger-to-passenger file share (doc 16) | telemetry reaches the conductor within ≤2 conductor rounds ⚠ p95; announcement reaches ≥95% of carriage anchors within one round trip of the train ⚠; **misdeployment check:** re-run with a single lead-carriage anchor only (non-conforming to §3.1's minimum cardinality) — pass band requires this configuration to show ≥50% delivery-ratio degradation ⚠ in carriages ≥4 positions from the anchor, confirming the per-carriage requirement is load-bearing and not decorative |
| D-9 | Campus/building profile: N floors ⚠, per-floor anchor density interpolated between the doc 03/09 baseline and the doc 14 W2 extreme | delivery ratio ≥0.85 ⚠ same-building within 15 min ⚠ (tighter time, lower density than W2's stadium 0.9/30 min, doc 14 D-4) |

Sim scenarios S-8 (train: line-graph carriage topology, anchor rail, conductor-mule mobility trace, platform-dwell windows per §2.4, opposing-train pass-through events modeled at §2.5's ≈0.2–1.1 s useless-contact rate) and S-9 (campus: per-floor cells, [I9]-derived floor-loss attenuation feeding the contact graph) hook into doc 15 R1's mmp-sim artifact and `ROADMAP.md` Stage 4 gate (OQ-4/OQ-5), exactly as doc 14's S-5/S-7 do. **Both drills are [S]-tagged (doc 13 notation): the pass bands above are targets for the simulator to confirm or refute, not measured results.**

## 9. Persona review checkpoints

- **Skeptical Professor — Potential flaw:** "Calling profiles 'novel' oversells a constants table with a name." *Resolution:* not contested — §1.3 states the council's (b)-methodology verdict plainly and does not headline this as scientific novelty. The one thing defended is narrower: §1.4/D-8's misdeployment check is a concrete case where formalizing the informal-appendix tradition catches something prose alone reliably wouldn't. Accepted, scope kept honest.
- **Security Auditor — Potential flaw:** "§3.3 extends doc 18's AuthCert priority-admission clamp from public class-5 announcements to private class-1 telemetry — is that still doc 18's mechanism, or a quiet new one?" *Resolution:* it is doc 18's mechanism used beyond its originally-described class, and §3.3 says so explicitly rather than assuming compatibility. The admission check itself is receiver-local by doc 18's own design (never wire-carried, doc 18 §5.2), and a profile is exactly the agreed-in-advance context in which every *provisioned* participant (the only nodes that handle telemetry — §3.3, CR2bis-9) applies the same local rule to a second bundle class — which is within a profile's authority under §1.2's zero-wire-change test. If a future doc 18 maintenance revision narrows admission strictly to class 5, §3.3 would need re-designation; flagged here rather than discovered later. No new cryptography, no new clamp logic; the doc 18 §5.4(c) lemma bounds sensor-subject abuse exactly as it bounds conductor-subject abuse.
- **Performance Engineer — Potential flaw:** "§2.4's 0.7–2 MiB dwell-capacity figure and D-8's round-count pass band both inherit doc 13 §2's 50–150 kbps range and CR-A5's unmodeled setup-amortization gap — the same honest-range problem doc 16 §8.2 already flagged." *Resolution:* correct, and not resolved differently here; §2.4 and D-8 are explicitly [S]-tagged (§8) for the same reason — OQ-2's phone-matrix throughput measurement and the doc 15 R1 simulator are what turn these into confirmed numbers, not arithmetic on this page.
- **Distributed Systems Reviewer — Potential flaw:** "The conductor-as-sole-ferry (§3.2) is a single point of *slowness*: a conductor who lingers in one carriage stalls every other carriage's cross-carriage delivery." *Resolution:* correct and stated as a real property, not hidden; the conductor is the *reliable* ferry (scheduled to traverse the whole train), not the *only* one — ordinary passenger mobility (dining-car trips, boarding/alighting) provides a secondary, unscheduled path via the same store-carry mechanism already specified (doc 03 §5.2), with no profile-level change needed to use it. D-8's ≤2-round p95 band is deliberately loose enough to admit this variance rather than assume a punctual conductor.
- **Battery Specialist — Potential flaw:** "§3.5's 5×-faster friendship poll (60 s vs the 300 s default) is a real per-poll energy cost the doc 13 §1.3 coin-cell ledger doesn't budget for." *Resolution:* correct, which is exactly why §3.5 states the retune is conditioned on mains/train power, not coin-cell — this profile field MUST NOT be copied into a battery-constrained (non-train) sensor deployment without re-running the doc 13 §1.3 ledger at the new poll rate. Stated as a precondition, not a footnote.
- **Inventive Scientist — tasked with N2/N4:** On N2: the train's anchor rail is a natural, *provisioned* instance of a line-graph topology (§7) — sidestepping N2's open detection-mechanism problem (an anchor here is told its position, never infers it), which makes Profile A a plausible first concrete validation of doc 15 R17's line-graph optimality argument rather than a general solution to corridor detection; flagged as a strengthening path for R17, not claimed as one here. On N4: the misdeployment-catch pattern (§1.4/D-8) generalizes beyond trains — a disaster-response or temporary-event profile could use the same "declare a minimum topology, drill against a violation of it" shape; filed as a future profile candidate alongside §6's urban pointer, not adopted in this revision.

## 10. Constants (⚠ provisional)

| Constant | Value | Note |
|---|---|---|
| FRIEND_POLL_TRAIN | 60 s ⚠ | §3.5, Profile A only, train-powered sensors only |
| Platform dwell (usable) | 30–120 s ⚠ | §2.4 |
| Opposing-train contact window | ≈0.24–1.08 s ⚠ | §2.5, useless-contact verdict |
| AuthCert validity (train) | ≤ shift length (≤12 h ⚠) | §3.3, reuses doc 18 §9 |
| D-8 carriage count | 8 ⚠ | §8, drill sizing only |
| D-8 misdeployment degradation threshold | ≥50% ⚠ | §8 |
| D-9 delivery-ratio target | ≥0.85 ⚠ within 15 min ⚠ | §8 |
| Floor-loss model (secondary, campus) | 15+4(n−1) dB, N≈30 | §2.6, [I9], low confidence |

## 11. Cross-references

Profile formalism vs conformance classes/tiers → doc 03 §6, doc 04 §3. Tunable-constant surface → doc 03 §9; local-policy knobs extended in kind → doc 04 §4/§5, doc 05 §2.1/§2.2, doc 09 §3.2/§7, doc 11 §7, doc 16 §8. AuthCert, scope registry, priority admission, announcement flow, revocation → doc 18 §2–§7, §9 (claim field **0x07**, not 0x06). File-share manifest/blob mechanics reused unchanged for passenger transfers → doc 16 §2–§8 (D-8, §8). Contact-capacity arithmetic → doc 13 §2. Failure register row reused directly (F8 for Profile B) → doc 14 §1; drill/scenario format extended (D-8/D-9, S-8/S-9) → doc 14 §3, doc 15 R1, `ROADMAP.md` Stage 4. Council constraints folded in: CR-A3 (§3.6, §4.3), CR-A5/CA4 (§2.5), CR-A6 (§3.4), N1 caveat (§2.3, §2.7), N2 (§7), N4 (§1.3–§1.4) — full findings in `COUNCIL-REVIEW.md`. Open questions: OQ-14 (§2.2), OQ-15 (§2.7), OQ-16/OQ-17 (§3.3, reused from doc 18), OQ-18 (reused from doc 16 for D-8's file share). Doc 15 §6 R-items cited: R16, R17, R19 (appended 2026-07-03 per `COUNCIL-REVIEW.md`'s disposition list). Drive-thru Internet prior art (CR-A5/N1 disposition): Ott & Kutscher, "Drive-thru Internet," IEEE INFOCOM 2004 [H13] — phases transfer across a contact window's entry/production/exit zones; this doc's contact-window verdicts (§2.4–§2.5) are positioned as a fresh BLE-specific composition, not an unprecedented result, consistent with the council's N1 disposition.
