# 02 — Scientific Research Report

**mesh_milk** Phase 1 literature review. Date: 2026-06-10. All citations resolve in `research/bibliography.md` (cited as [A1], [B3], …). Confidence tags: **H/M/L** per the methodology in CLAUDE.md.

---

## 1. Track A — Existing Systems

Full per-system matrix: `research/systems-comparison.md`. Summary of design-relevant conclusions:

### 1.1 Findings

**F-A1 (H).** Pure and managed flooding have a *delivery cliff*, not graceful degradation. Bluetooth Mesh simulations show reliability collapse from broadcast storms as density grows [A3][A4][A24]; Meshtastic community data shows <10% delivery at ~1000 nodes regardless of hop limit [A13][A14]. *Consequence:* flooding may be used only as a bounded control-plane or last-resort mechanism, never as the primary data plane at scale.

**F-A2 (H).** Security must be designed in, not retrofitted. Bridgefy was broken twice — first its custom crypto (tracking, impersonation, one-packet network shutdown) [A1], then again after adopting libsignal, because transport metadata, routing, and DoS surfaces were never part of the security model [A2]. bitchat repeated the pattern in 2025: optional signatures covering only the payload, identity trusted from packet fields, caching before authentication → cache poisoning and replay [A15][A16][A17]. *Consequence:* every header byte that influences node behavior must be authenticated; identity must be a verification key, never a claim.

**F-A3 (H).** Single-writer append-only logs (SSB) make anti-entropy nearly free: replication state per peer is just (feed_id, latest_seq), and social-graph-weighted replication doubles as Sybil/spam resistance [A9]. But unbounded immutable logs are a mistake for our setting (storage quota, deletion) — we inherit the *sequencing* idea, not the immutability [A9].

**F-A4 (M).** Reticulum is the closest philosophical ancestor: address = hash(pubkey), transport-agnostic, no global topology knowledge, functions at 5 bps [A10]. But it has no published formal security analysis, no `no_std` implementation, and its announce-propagation cost at 10k nodes is unquantified. We treat it as design inspiration, not validated prior art.

**F-A5 (H).** DTN BPv7 [A7] is the canonical store-and-forward formalism (bundles, lifetimes, convergence-layer abstraction), proven over a decade of space deployments. Its weaknesses for us: CBOR/EID overhead is heavy against 31-byte BLE advertisements, custody transfer was dropped from the core spec because retransmission timers are hard to estimate in unpredictable-contact settings [A7][A8]. We adopt the bundle/lifetime *model* with a compact binary encoding.

**F-A6 (H).** Bluetooth Mesh's *friendship* mechanism (a mains-powered Friend node buffers traffic for a sleeping Low Power Node which polls at its own schedule) [A23][A24] maps directly onto our IoT-sensor + phone topology and is worth adapting peer-to-peer — minus the centralized provisioner, which contradicts our trust model.

**F-A7 (M).** Briar demonstrates contact-only sync with per-transport rotating keys (forward secrecy at the transport layer) [A11][A12] but deliberately does not relay between strangers — hence it forms a social DAG, not a mesh. Our protocol must relay through strangers, which is precisely why relay-layer DoS/Sybil economics (Track C) dominate our design risk.

**F-A8 (M).** DHT-based discovery (cjdns, libp2p Kademlia) fails on sparse, churning, battery-bound networks: cjdns retreated to supernodes [A18-adjacent history]; DHT random-walks are battery-hostile on mobile [A19 + DHT survey]. Nostr/Matrix confirm relay/federation models need infrastructure [A20][A21]. *Consequence:* no DHT in the core protocol; discovery must be local-broadcast + gossip.

### 1.2 Persona review (Track A conclusions)

- **Skeptical Professor — Potential flaw:** "The Meshtastic 1000-node collapse figure comes from community/blog sources, not a peer-reviewed study." *Resolution:* downgraded to (M); the *mechanism* (flooding broadcast storms) is independently peer-reviewed for Bluetooth Mesh [A3][A24], so the qualitative conclusion stands even if the exact 10% figure is soft.
- **Security Auditor:** Bridgefy/bitchat lessons elevated to hard requirements (entire-packet authentication, no behavior-affecting unauthenticated fields) — carried into doc 07.
- **Distributed Systems Reviewer:** SSB's (feed, seq) reconciliation assumes per-sender sequencing survives deletion/expiry. Flagged as open question OQ-1 (research/open-questions.md).

---

## 2. Track B — BLE Physical & OS Constraints

These numbers bound every later design decision.

### 2.1 Quantified constraints table

| Constraint | Value | Source | Conf |
|---|---|---|---|
| Legacy advertisement payload | 31 B (ADV_IND), +31 B scan response | BLE spec via [B6][B7] | H |
| Extended advertising payload | up to 255 B per AUX PDU (chained: ~1650 B theoretical) | [B6] | H |
| Extended adv on Android | API exists (8.0+); reliability depends on Android version × controller × vendor firmware; works on e.g. Pixel 7 | [B5][B8] | M |
| Extended adv on iOS | Cannot *transmit* extended advertisements; receive support partial; Apple recommends against in accessory guidelines | [B4] | M |
| **Design floor: legacy 31-byte advertisements are the only universally receivable beacon format across all v1 platforms** | — | synthesis of [B4][B5] | H |
| GATT practical throughput, 1M PHY | ~100–210 kbps measured phone-to-phone class (MTU/DLE dependent) | [B1][B2][B3] | H |
| GATT practical throughput, 2M PHY | ~400–780 kbps best case; 2M ≈ 1.8–2.1× of 1M | [B2][B3] | H |
| Realistic planning number (mixed fleet, interference) | **~50–150 kbps per connection** | conservative reading of [B1–B3] | M |
| Android background scan w/o filter | killed after 30 s (Android 7+); must register ScanFilter | [B9][B10] | M (vendor-variable) |
| Android scan start/stop quota | >5 starts per 30 s → silently throttled | [B9][B10] | M |
| Android background without foreground service | scans throttled to short windows; Doze defers further | [B9][B10] | M |
| iOS background advertising | local name dropped; service UUIDs moved to "overflow area" — only discoverable by iOS devices *explicitly scanning for that UUID* | [B4][B11] | H |
| iOS background↔background discovery | works only if the scanning device's **screen is on** (documented by reverse engineering) | [B11] | M |
| Smartphone BLE peripheral-role connections | typically ~4–10 concurrent (stack-dependent) | [B3][B6] | M |
| Advertising current (nRF52-class, 0 dBm) | ~5.6–6.3 mA *during event*; duty-cycled average orders lower | [B12][B13] | H |
| Energy drivers | adv interval, scan interval/window dominate idle budget; neighbor-discovery latency ∝ 1/(duty cycle) | [B13][B14] | H |
| Multi-hop reality | each extra relay adds collision probability; PER can degrade >100× with redundant relays [B15]; per-hop forwarding latency tens-to-hundreds of ms (relay queue + adv repetition) | [B15][B16][B17] | H |

### 2.2 Design consequences (synthesis)

**C-B1 (H).** **Two-plane architecture is forced by physics and OS policy.** A *beacon plane* over legacy 31-byte advertisements (universal, background-capable within limits) for presence, summaries, and wake-up signaling; a *bulk plane* over GATT connections (50–150 kbps) for actual message/blob sync. BLE advertisements alone cannot carry chat traffic at any useful scale; GATT alone cannot do low-power discovery.

**C-B2 (H).** **iOS is the binding constraint on discovery.** Background iOS nodes are second-class citizens: they cannot beacon normally and can only be found via service-UUID-targeted scans (overflow area), with screen-state dependencies. The protocol must (a) fix one well-known service UUID so all platforms can target-scan it, (b) treat iOS-background nodes as *poll-only leaf participants*, and (c) never assume symmetric discovery.

**C-B3 (H).** **100+ hops cannot mean 100 synchronous radio hops.** At tens-to-hundreds of ms per relay hop plus collision-driven loss compounding per hop [B15][B16], a 100-hop connected path is neither reliable nor timely. The requirement is met instead by *store-carry-forward*: physical movement of nodes provides most of the distance; radio hops provide local diffusion. This matches the chosen latency budget (minutes) and is the standard DTN result.

**C-B4 (M).** **Android foreground service is mandatory** for relay-class participation; the protocol must define degraded modes (beacon-only, poll-on-app-open) for nodes that can't hold one, rather than pretending all phones are equal relays.

**C-B5 (H).** Duty-cycled neighbor discovery is *the* battery lever: discovery energy scales with scan window/interval and adv interval [B13][B14]. The <5%/day budget is achievable only with slotted, low-duty advertising+scanning (quantified model deferred to doc 13).

### 2.3 Persona review (Track B)

- **Battery Specialist — Potential flaw:** "5.6 mA advertising figures are *during the event*, not average; naive readers will multiply wrongly." *Resolution:* table now states duty-cycled average is orders lower; doc 13 will carry the full energy model with explicit duty-cycle math.
- **Performance Engineer:** planning number deliberately set at 50–150 kbps (not the 780 kbps best case) to absorb mixed fleets, interference, and conservative connection parameters. Marked (M) until phone-matrix measurements exist (OQ-2).
- **Skeptical Professor:** iOS background↔background screen-on behavior is from reverse engineering [B11], not Apple documentation; tagged (M) and listed as OQ-3 (re-verify on current iOS).

---

## 3. Track C — DTN / Opportunistic Routing

### 3.1 Candidate algorithms and published evidence

| Algorithm | Mechanism | Delivery | Overhead | State needed | Conf |
|---|---|---|---|---|---|
| Epidemic (Vahdat & Becker) | Replicate to every contact | Highest possible (upper bound) | Unbounded — O(N) copies/message; floods buffers and airtime | Per-message seen-list | H [C1][C2] |
| Spray-and-Wait, esp. **Binary** (Spyropoulos et al.) | L copies total; each carrier hands off half its copies; direct delivery in wait phase | Near-epidemic in evaluations | **Bounded by construction (≤L copies)**; best delivery/overhead/delay balance in multiple comparisons | Copy counter per bundle (a few bits) | H [C1][C3] |
| PRoPHET | Delivery predictability from encounter history + transitivity | Good when mobility is recurrent | Per-destination predictability table — O(destinations) state, gossiped | Encounter history table | H [C2][C4] |
| MaxProp | Path likelihood + hop count priority + network-wide ACKs to purge delivered bundles | High in vehicular traces | Moderate; ACK flooding cost; designed for resourced vehicular nodes | Encounter probabilities + ACK set | H [C2] |
| BUBBLE Rap (Hui, Crowcroft, Yoneki) | Forward up the global popularity (centrality) gradient, then within destination's community | Significantly better than PRoPHET/SimBet on real human-contact traces | Low forwarding cost; needs community detection + centrality estimation | Social graph estimates | H [C5][C6] |
| Geographic | Forward toward destination coordinates | Good with dense nodes + accurate positions | GPS energy cost; indoor failure; **leaks location — conflicts with our threat model** | Position beacons | H [C7] |

Key replicated findings across independent comparisons [C1][C2][C3]:
1. Epidemic is an upper bound on delivery but its overhead collapses buffers/airtime exactly where BLE is weakest (Track B: airtime is the scarce resource).
2. Binary Spray-and-Wait achieves delivery close to epidemic at a small constant overhead, with state cheap enough for IoT (a copy counter per bundle).
3. Encounter-history methods (PRoPHET/MaxProp) and social methods (BUBBLE Rap) beat blind replication *when mobility has structure* — which human-carried phones have [C5][C6].
4. Buffer size strongly modulates everything: bigger buffers → higher delivery, lower relative overhead [C1] — supports our quota+priority retention choice.

### 3.2 Recommendation R-C1: tiered hybrid routing (H for components, M for composition)

- **Tier 0 — single-hop / neighborhood (≤~2 hops):** direct delivery + bounded managed flooding for control beacons only (Track A forbids flooding as data plane).
- **Tier 1 — default data plane:** **Binary Spray-and-Wait** (L adaptive to estimated network size, e.g. L≈⌈log₂N⌉-scaled) as the replication discipline — bounded copies, IoT-trivial state.
- **Tier 2 — utility steering:** spray *direction* biased by a utility score combining (a) PRoPHET-style encounter predictability toward the destination key and (b) BUBBLE-style social-tie strength derived from the contact/web-of-trust graph we already maintain for moderation. Utility only chooses *which* contacts get copies; it never increases copy count.
- **Geographic routing: rejected** — GPS energy + indoor failure + location leakage versus our pseudonymity goals [C7].
- **ACK back-propagation** (MaxProp-style delivered-bundle purging) adopted as a *summary* mechanism (delivered-ID Bloom filters) since it directly recovers buffer/airtime, but full network ACK flooding is not (overhead at 10k nodes).

### 3.3 Persona review (Track C)

- **Skeptical Professor — Potential flaw:** "Most comparisons use ONE-simulator synthetic mobility, not BLE radios; per-contact transfer capacity in those studies assumes WiFi-class links." *Resolution:* acknowledged (M on composition); doc 13 must re-model contact capacity at 50–150 kbps × contact duration, and OQ-4 records the need for BLE-parameterized simulation.
- **Distributed Systems Reviewer — Potential flaw:** "Utility tables (PRoPHET state) toward *every* destination key won't scale to 10k pseudonymous destinations." *Resolution:* utility state capped: per-contact tie strength (bounded by real contacts) + predictability only for destinations in active conversations; everything else falls back to pure spray. Recorded in R-C1.
- **Security Auditor:** encounter-history gossip is a route-poisoning surface (lying about predictability attracts traffic = sinkhole). Mitigation direction: utility inputs must be locally observed, never accepted from third parties; carried to doc 07 threat model.
- **Battery Specialist:** spray hand-off of half the copies on first contact can dump large transfers into short contacts; require contact-duration-aware transfer ordering (small/urgent first). Carried to doc 05.

## 4. Track D — Synchronization & Consistency

### 4.1 Evidence

**F-D1 (H).** CRDT family tradeoffs are well-characterized [D1][D2][D3]: *operation-based* CRDTs need reliable exactly-once causal broadcast — a guarantee a DTN mesh cannot provide; *state-based* CRDTs tolerate dropped/duplicated/reordered messages but ship whole states; *delta-state* CRDTs keep state-based robustness while shipping small deltas. Crucially, a dedicated study of **delta-state CRDT sync in opportunistic networks** exists and validates the approach in exactly our network model [D4].

**F-D2 (H).** For *message dissemination* (immutable signed messages, not mutable registers), the sync problem reduces to **set reconciliation**. The efficiency ladder from published work: full inventory exchange < Bloom-filter sketches (false positives, sized in advance) < IBLT < BCH-based sketches (Minisketch — communication-optimal for small differences, used in Bitcoin Erlay [D5][D6]) < **Range-Based Set Reconciliation** (RBSR / Negentropy — no per-connection state, no pre-sizing, divide-and-conquer over ranges, DoS-resistant; deployed in Nostr NIP-77) [D7][D8] and Rateless IBLT (SIGCOMM'24) with near-optimal communication without difference-size estimation [D9].

**F-D3 (H).** Merkle Search Trees (Auvolat & Taïani) and Merkle-CRDTs (IPFS) [D10][D11] show how content-addressed DAGs give deterministic convergence in open networks — but Merkle-tree sync costs round trips (log-depth traversal), which is painful over short BLE contacts; RBSR's authors explicitly position it as superior unless membership proofs are needed [D7].

**F-D4 (H).** Version vectors ≠ vector clocks [D12]: version vectors order *replica states* (O(replicas) entries), vector clocks order *events*. For 10k-node meshes, neither scales per-message — but **per-conversation causal references (parent hashes in a DAG)**, as in Matrix's room DAG and Merkle-CRDTs, encode causality in O(parents) per message with no global vector [D11]. This matches the already-fixed requirement (groups: causal ordering via Merkle-DAG references).

### 4.2 Recommendation R-D1 (H components / M composition)

Three data classes, three sync treatments:

1. **Messages (immutable, signed, TTL-bounded):** grow-only set with expiry, synced by **RBSR over GATT** during contacts; per-conversation causal DAG via parent-hash references. Bloom-filter *summaries* in beacons advertise "what I carry" cheaply (Track B beacon plane); RBSR does exact reconciliation once connected.
2. **Profile state (nickname claims, avatars — mutable, single-writer):** **last-writer-wins register per (key, field) with the owner's signature + monotonic sequence number** — a trivial delta-state CRDT; single-writer eliminates concurrency conflicts entirely (SSB's insight, F-A3). Avatars chunked as content-addressed blobs, fetched on demand, not pushed.
3. **Group membership (multi-writer, low rate):** delta-state OR-Set (add-wins) keyed by admin signatures — full CRDT machinery justified only here; tiny state, low churn. IoT nodes may hold sender keys for groups they participate in without materializing membership CRDTs (profile/membership sync optional for the IoT core).

**Rejected:** Automerge/Yjs-class general CRDTs (RAM/code size unjustified for chat-log semantics on `no_std` targets — they solve rich-text concurrent editing, which we don't have); Merkle-tree-walk sync as primary (round-trip cost over short contacts, F-D3); global version vectors (state explosion, F-D4).

### 4.3 Persona review (Track D)

- **Performance Engineer — Potential flaw:** "RBSR is round-trip-based too; over a 5-second BLE contact with 100 ms RTT you get ~50 rounds max." *Resolution:* RBSR rounds are O(log n) in practice with large branching per round [D7][D8]; combined with Bloom pre-filtering in beacons, expected rounds ≤3–4 for typical deltas. Worst-case behavior over BLE quantified in doc 13 (OQ-5).
- **Security Auditor — Potential flaw:** "LWW-by-sequence-number lets a compromised key rewrite profile history." *Resolution:* accepted — key compromise = identity compromise in our model (consistent with threat tier); sequence numbers prevent *replay of old states* by third parties, which is the actual attack surface.
- **Skeptical Professor:** Negentropy/RBSR deployment evidence is from Internet relays (Nostr), not BLE meshes — composition tagged (M), and OQ-5 covers BLE-link validation.
- **Inventive Scientist:** beacon-carried Bloom summaries double as *implicit ACKs* (carrier advertises message IDs it holds → neighbors stop re-offering), recovering MaxProp's ACK benefit (R-C1) for free. Adopted into doc 05/06 design inputs.

## 5. Track E — Network Coding & Erasure Coding

### 5.1 Evidence

**F-E1 (H).** RLNC in DTNs improves delivery/delay *under epidemic-style replication with constrained bandwidth and buffers*, mostly for **broadcast/multicast and large transfers**; gains for sparse unicast are modest and come with coefficient-vector overhead and decode cost [E1][E2][E3].

**F-E2 (H).** **In-network recoding breaks signature verification.** A relay that recodes packets produces data the origin never signed; defending recoded streams against *pollution attacks* requires homomorphic signatures or per-generation MACs with shared keys — both unacceptable on our trust model and IoT budget (pollution = one bad symbol poisons a whole generation decode). This is a structural conflict with the Bridgefy/bitchat lesson (authenticate everything end-to-end, F-A2).

**F-E3 (H).** Rateless **fountain codes at the source** avoid that conflict: the origin generates and signs coded symbols; any k(1+ε) symbols reconstruct. RaptorQ (RFC 6330) decodes "in most cases" from exactly k symbols, rarely needing slightly more [E4]; the commonly-cited figures (≥99% at 0 symbols overhead, ≥99.99% at +1, ≥99.9999% at +2) come from the Raptor codes design literature, not the RFC text itself [E7] (M — verify against the monograph before citing in final specs). Codecs are linear-time [E4]. IPR status: Qualcomm holds patents with an RFC-implementation licensing commitment; early patents began expiring ~2024 but continuations exist — **legal status unclear (L)**, must be assessed before adoption [E5][E6].

**F-E4 (H).** Reed-Solomon over GF(256) is patent-free, MDS-optimal (any k of n symbols suffice, zero overhead), and cheap at small k; its O(k·(n−k)) cost only hurts at large block counts irrelevant to avatar-sized blobs.

### 5.2 Mathematical justification (why coding, and where)

The win is the *multi-source coupon-collector problem*. A blob split into k uncoded fragments, fetched opportunistically from multiple relays that each hold random subsets, costs the requester duplicate receptions: collecting k distinct coupons by random draws needs ≈ k·H_k ≈ k·ln k receptions (k=40 → ≈150 receptions, 3.7× waste). With MDS/rateless coding where relays hold *distinct coded symbols*, any k receptions suffice — the waste term vanishes. Coding therefore pays exactly where Track B hurts most: short, lossy, interrupted contacts with different partners. For single-fragment payloads (typical text messages ≤ one GATT MTU) coding adds pure overhead — so it applies to the *blob plane only*.

### 5.3 Recommendation R-E1

- **Adopt source-level erasure coding for blobs** (avatars, attachments, long messages): v1 mandates **Reed-Solomon GF(256)** (patent-free, MDS, IoT-friendly at k ≤ ~64); wire format carries a coding-scheme ID so RaptorQ can be added once IPR is resolved (OQ-6).
- Symbols are individually authenticated against a signed BLAKE3 Merkle root of the source block (verify-before-store, defeating pollution at the symbol level).
- **Defer RLNC / in-network recoding indefinitely** (F-E2) — revisit only in doc 15 (future research) with homomorphic-authentication literature.
- Text messages: no coding; reliability comes from replication (R-C1) + reconciliation (R-D1).

### 5.4 Persona review (Track E)

- **Inventive Scientist — Alternative:** "Recoding relays would make every relay a useful symbol source." *Security Auditor veto:* pollution-attack surface + unverifiable relay output; rejected per F-E2.
- **Performance Engineer:** RS encode on sender (phone/PC) trivial; decode on nRF52 at k≤64 measured-class feasible (table-driven GF(256)); flagged for benchmark in Phase 5 (OQ-7).
- **Skeptical Professor:** coupon-collector model assumes uniformly random symbol holding; real relay populations correlate (everyone got symbols from the same origin burst). *Resolution:* origin rotates symbol windows per contact, restoring near-distinctness; noted in doc 05 design inputs (M).

---

## 6. Synthesis — Design Directions for Phase 2

The five tracks converge on a coherent architecture skeleton (each item traces to findings above):

1. **Two-plane BLE design** — beacon plane (31-byte legacy adv: presence, Bloom summaries, wake-up) + bulk plane (GATT, 50–150 kbps: RBSR sync, blob symbols). [C-B1]
2. **Store-carry-forward is the primary long-haul transport**; radio multi-hop only diffuses locally. "100+ hops" = cumulative DTN hops over space and time. [C-B3]
3. **Routing = Binary Spray-and-Wait copies + locally-observed utility steering (encounter predictability × social tie), no flooding data plane, no DHT, no geographic.** [R-C1, F-A1, F-A8]
4. **Sync = RBSR set reconciliation for messages + single-writer LWW deltas for profiles + OR-Set for group membership; causality via parent-hash DAG per conversation.** [R-D1]
5. **Blobs = content-addressed, signed Merkle root, RS-coded symbols fetched multi-source.** [R-E1]
6. **Everything authenticated end-to-end; headers that affect behavior are signed or MAC'd; identity = Ed25519 key.** [F-A2]
7. **IoT participation tiers**: sensors implement beacon + bulk-sync core only (no profile/membership CRDTs), with a Bluetooth-Mesh-friendship-style buffering relationship to phones. [F-A6]
8. **Platform asymmetry is normative**: iOS-background nodes are poll-only leaves; Android relays require foreground service; PCs are preferred high-uptime relays. [C-B2, C-B4]

Scalability, security, and battery validation of this skeleton are Phase 2–5 work; the open questions below carry the known unknowns.

## 7. Open Questions

Mirrored in `research/open-questions.md` (OQ-1 … OQ-7).
