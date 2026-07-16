# Council Review — External Multi-Model Review of the Rev-B Extension Docs

**What this document is.** During the rev-B extension phase (docs 16–18, drafted
2026-07-02–2026-07-03), the project's usual internal adversarial-persona review was
supplemented by an **external multi-model council** — the step rev-A's audit
(`AUDIT.md`) could not run, because `/ask-council`'s target subagent type was not
registered in that environment; by rev-B time it was available and was used. The
council ran across a **Round 1** (initial findings) and two follow-up passes,
**Round 2** and **Round 2-bis**, whose fixes are cited throughout docs 16–18 as
`CR-A*`, `N*`, `CR2-*`, `CR2bis-*`, and `CR-S*`.

**Provenance and honesty note (reconstructed 2026-07-16, backlog item E0-S2).**
This file did not exist in the repository, even though docs 16–18 cite it by name
more than twenty times and doc 15 §6 attributes several R-items to its dispositions.
**No original council transcript, deliberation log, or Round-1 finding-write-up
survives in this repository** — only docs 16, 17, 18, 15 (§6), and
`specification/research/open-questions.md` quote or paraphrase specific
disposition IDs. This document reconstructs an entry **only** for IDs actually
quoted somewhere in the repository, using **only** the context the citing document
provides. Where a disposition is cited by ID but its content is not paraphrased
anywhere (i.e., the ID appears only in a list of "fixes applied" with no
surrounding explanation), the entry says so explicitly — **text not preserved; see
citing doc's cross-reference** — rather than inventing plausible-sounding
disposition reasoning. Per this project's CLAUDE.md methodology, no entry below
states a rationale, alternative, or tradeoff that is not directly attributable to
the cited document's own words.

If the original council output is recovered later (e.g., from session logs outside
this repo), it should **replace** the corresponding reconstructed entry rather than
be merged with it.

---

## Round 1 — initial findings

Round 1 IDs are cited in docs 16–18 as constraints the design already satisfies or
must satisfy; no single "Round 1 report" survives, so each entry below is built
from the doc(s) that invoke the ID.

| ID | Disposition (as recoverable) | Cited / applied in |
|---|---|---|
| **CR-A1** | "A bare priority byte with per-origin token buckets cannot bound priority abuse — because origin identity is free." The council's Sybil finding showed that any admission scheme metered purely by origin key is defeated by minting fresh keys (same base-rate-times-N_keys arithmetic as doc 07 matrix #4). Disposition: named priorities are honored in receiver-local admission only when gated by a verified AuthCert with remaining token-bucket budget (doc 18 §5.2); ordinary per-origin buckets (doc 05 §8) remain the residual bound elsewhere (doc 16 §9.1's garbage-manifest analysis cites the same residual for file-share flooding). | doc 18 §5.2 heading, §7 row A1; doc 16 §9.1, §12 |
| **CR-A3** | Leaf-fraction assumption: every profile must state plainly what fraction of passenger/participant devices it assumes are relay-capable. Doc 17's disposition: Train Profile A assumes passenger phones are ~100% T1-leaf-equivalent for relay purposes and places zero reliance on them (the anchor rail + conductor-ferry supplies relay capacity instead); Profile B inherits the unmitigated ~90% T1-leaf baseline (doc 04 §3/§5) as its explicit, undefended posture. | doc 17 §3.6 (heading "Leaf-fraction assumption (CR-A3)"), §4.1, §11 |
| **CR-A4** | Reframes doc 15 R4's spray-parameter-learning input from a population estimate N̂ to an **encounter-rate EWMA** (rate of distinct-peer contacts per unit time) — same locally-observable-only constraint (doc 05 §4 poisoning lesson), but better-behaved under the relay-scarcity regime CR-A3 identified. | doc 15 §1 R4 ("Rev. B addendum") |
| **CR-A5** (also cited as **CA4**) | Short-contact / setup-amortization finding: a contact's setup cost (handshake + SUMMARY + RBSR) can consume the entire contact window before the BLOB phase (or any payload phase) is ever reached, so per-contact throughput arithmetic that assumes every airtime second is usable goodput can be a severe underestimate for short (3–10 s) drive-by contacts. Doc 16 §11's Performance Engineer checkpoint treats this as inherited, unresolved by that doc, and defers it to doc 15 R1's simulator. Doc 17 §2.5 applies the same finding concretely to opposing-train passing contacts (verdict: "not a degraded channel, they are not a channel"). | doc 16 §8.2 ("CR-A5, §11 Performance Engineer"), §11; doc 17 §2.5 ("CR-A5/CA4's short-contact finding"), §9 |
| **CR-A6** | "No shared clock" — unanimous HIGH finding (per doc 15 R19): TTL expiry, SeenIndex pruning, epoch rotation, and (once doc 18 existed) AuthCert `not_after` all lean on wall-clock time a partitioned DTN mesh cannot establish or agree on. Disposition: prefer elapsed-local-time decrement and receiver-local clock checks with a stated skew tolerance (15 min ⚠ for AuthCert `not_after`, doc 18 §6) over any online-CRL-style or synchronized-clock mechanism; damage from a badly-wrong receiver clock is contained to that receiver only. | doc 18 §2 (`not_after` field comment), §6 (heading "Why not an online CRL (CR-A6, no shared clock)"), §9; doc 17 §3.4 ("No shared clock is assumed anywhere in this path (CR-A6)"); doc 15 §6 R19 |
| **CR-A7** | AuthCert revocation-over-DTN is not a new primitive — it is a **scoped instance of the general self-certifying-key-revocation-over-DTN problem** that applies equally to identity keys, group-admin keys, and exchange keys (doc 15 R20 generalizes it). Doc 18 §6 reuses claim-LWW tombstoning specifically because that mechanism is already general-purpose. | doc 18 §6 (heading "Scoped instance of a general problem (CR-A7)"); doc 15 §6 R20 |
| **N1** | Novelty re-grading for contact-window-aware session admission (residual-time estimation from RSSI slope or BLE Channel Sounding, re-planning in-session sync-phase order): the honest claim is a **fresh BLE-specific composition**, not an unprecedented idea — closest prior art is Ott & Kutscher, "Drive-thru Internet" (IEEE INFOCOM 2004), which phases transfer across a contact window's entry/production/exit zones for vehicular WiFi. | doc 15 §6 R16 ("COUNCIL-REVIEW N1 disposition"); doc 17 §2.3 ("v1 fallback for novelty N1"), §2.7, §11 (Drive-thru Internet citation) |
| **N2** | Corridor (linear-topology) spray novelty finding: the mechanism (directional copy-budget split toward unvisited ends on a detected 1-D topology) is not itself the weak point — **detection** is: inferring "I occupy a corridor position" from locally-observable contact sequences alone is unresolved in the general case. Doc 17's train profile sidesteps the open part by making an anchor's line position a **provisioned fact** rather than something inferred. | doc 15 §6 R17 ("COUNCIL-REVIEW N2's weak point"); doc 17 §7 (table row "Corridor-spray (N2, advisory)"), §9, §11 |
| **N3** | The council's N3 disposition (Round 1) asked for "a short, receiver-local, DTN-honest bound that survives arbitrary relay-count amplification" for authority-gated priority admission — in contrast to unbounded schemes like BPv7 priority classes, which carry no sender authentication and no abuse resistance at all. Doc 18 §5.4 supplies this as a formal lemma (bound `B + β·T ≤ β·(T+1)` per certificate) and explicitly disclaims cryptographic novelty (§5.4a: Teranishi et al. k-times auth and Chaum et al. offline e-cash both already achieve stronger, cryptographically-enforced budgets; MMP's contribution is narrower — a proved accounting bound for a DTN with no online arbiter). | doc 18 §5.4 (heading "N3 — honesty and formal bound"), §11 ("tasked with N3") |
| **N4** | "Honesty about what this is" — the council's verdict on deployment profiles (doc 17): a named, versioned constant-vector-plus-conformance-composition is **methodology, not protocol science**; prior art (FODA feature models, 3GPP/Thread conformance classes, TTCN-3 test suites) makes the pattern itself unremarkable. Doc 17 does not contest this and narrows its claim to one testable thing: turning an informal deployment appendix into a versioned object a drill can fail against (the D-8 misdeployment check, §1.4). | doc 17 §1.3 (heading "Honesty about what this is (N4)"), §1.4, §9, §11; doc 15 §6 R18 ("the council downgraded this claim (N4)") |
| **N5** | The eager-thumbnail / lazy-full-file split (doc 16 §5.1) is named as the council's N5 novelty item: previews propagate at gossip speed, payloads move only when someone actually wants them — positioned against the doc 16 §1.3 landscape review (Haggle, PodNet, bitchat, Reticulum LXMF, Meshtastic, AWDL, AirDrop) as presently unoccupied ground for a *signed, content-addressed* two-tier manifest specifically. | doc 16 §5.1 ("This eager/lazy split (N5) is the whole design"); doc 16 §12 ("N5 (§5.1–§5.2)") |
| **N6** (cited as **CR-N6**) | "Provable non-amplification and gossip-free misbehavior bounds" — flagged in doc 15 as **the extension phase's headline theoretical direction**, proposed independently by both novelty-seat models on the council. Two facets: (a) hint-plane non-amplification implies a bounded energy-DoS ceiling set by authenticated-session admission rate, not beacon volume (with the explicit caveat that CR-A1's Sybil-defeated buckets are where the residual actually lives); (b) gossip-free misbehavior visibility — a receiver flags anomalous per-hop dwell from cryptographically-bound fields plus its own locally-observed statistics only, no third-party accusations. Recorded as a research direction (R21), not yet proved. | doc 15 §6 R21 (heading names "CR-N6"), §7 ("R21 ← CR-N6") |

---

## Round 2 — fixes applied to docs 16 and 18

| ID | Disposition (as recoverable) | Applied in |
|---|---|---|
| **CR2-1** | Legacy-body disambiguation must be fail-safe: a rev-A sender emits untagged bodies, so a legacy plaintext body whose first byte happens to be `0x01` is a priori indistinguishable from a tagged `FileManifest`. Disposition (normative): on `body[0] = 0x01`, attempt a full canonical manifest parse; on **any** parse failure, render the whole body as opaque text/bytes — never drop, never error the session. | doc 16 §3.1 (heading "Legacy-body disambiguation (normative, CR2-1)") |
| **CR2-2** | Soft spot in doc 18 §7 row A6 (stolen operator root key): every containment listed there except scope isolation presupposes the operator *notices* the compromise, and no detection signal was specified. Disposition: the natural candidate is gossip-consistency divergence on the root claim (two verifying-but-different roots for one scope observed by one node); folded into OQ-16's UX-study/pilot resolution path as a compromise-detection-latency question, not resolved outright. | `specification/research/open-questions.md` OQ-16 ("Round 2-bis annotation (CR2-2 soft spot)"); doc 18 §1 (heading "Disclosure (CR2-4)" references the same root-of-trust framing) |
| **CR2-3** | Tighten the doc 18 §5.4(c) admission-bound lemma: the initially-full-bucket term (`B`) must be bounded **explicitly** in the proof rather than absorbed into the rate term, giving `B + β·T ≤ β·(T+1)` rather than an looser or implicit form. | doc 18 §5.4 ("Lemma (tightened per CR2-3)") |
| **CR2-4** | Disclosure obligation: the operator root key introduced by doc 18 is this suite's **first deliberate trust anchor** — a scoped, opt-in departure from the "no registration authority" axiom that docs 01–15 hold everywhere else. Disposition: state this plainly rather than understating it; the departure is honest and contained (scoped, expiring, budget-bounded, pin-or-gossip visible to the user), and Profile B exists precisely so the protocol never *requires* it. | doc 18 §1 (heading "Disclosure (CR2-4)") |
| **CR2-5** | Scope correction for the AEAD context-commitment requirement (doc 16 §4.2): blob symbols themselves are signed plaintext verified via Merkle paths — no AEAD involved there. The commitment requirement binds only where AEAD actually carries the manifest, i.e., the class 1/2 message body. | doc 16 §4.2 (heading "AEAD context commitment (normative, CR-S2; scope corrected per CR2-5)") |

---

## Round 2-bis — fixes applied to docs 16, 17, and 18

| ID | Disposition (as recoverable) | Applied in |
|---|---|---|
| **CR2bis-1** | The `FileManifest.sig` must verify against an **inline `sharer` key** (32 B), never against the carrying bundle's `origin` — because §3.4's re-share detaches the manifest from any bundle whose origin equals the sharer, and without an inline key a re-share recipient would have no way to know which key to verify against. Same rationale as AUDIT finding F2 (inline origin pubkey for bundles). | doc 16 §2.1 (field comment "CR2bis-1"), §3.3 (heading "Verification (normative, CR2bis-1)"), §3.4 |
| **CR2bis-2** | Corrects the doc 18 §6 revocation design: a **subject-signed tombstone** only works for retirement/device loss, not key theft — an attacker holding the stolen subject key holds the same LWW pen and can out-`seq` any tombstone indefinitely; it must never be presented as a defense against the stolen-key attack (row A1). The theft-capable path requires an **issuer-signed** revocation object instead (see CR2bis-10). | doc 18 §6 (heading "Explicit revocation (backstop, corrected per CR2bis-2)"), §7 row A1; `specification/research/open-questions.md` OQ-17 ("Round 2-bis annotation (CR2bis-2)") |
| **CR2bis-3** | Normative fix: RBSR reconciliation-set partitioning (doc 03 §5.2 phase 3) **must key on the wire-visible, signed `priority` byte**, never the receiver-local clamped/effective priority — because bucket state and AuthCert visibility differ per receiver, so two honest peers can compute different effective priorities for the same bundle; if the clamped value fed partitioning, honest peers would systematically diverge on RBSR fingerprints. The clamp therefore governs only receiver-local admission and rendering, never wire-phase ordering. | doc 18 §5.2 (heading "Reconciliation-set partitioning uses the DECLARED priority, never the clamped one (normative, CR2bis-3)"), §5.2.2 |
| **CR2bis-4** | Sender-side default for the doc 16 apptype tag: senders of plain text SHOULD default to **untagged** bodies and MAY only emit the explicit `0x00` tag once the conversation has evidence the peer understands tags — because a `0x00`-tagged text body delivered to a rev-A client would render with a spurious leading byte, the opposite-direction regression from the one CR2-1's fail-safe rule already guards against. | doc 16 §3.1 (parenthetical "a regression the fail-safe rule above exists to avoid in the other direction (CR2bis-4)") |
| **CR2bis-5** | Elevated (priority 0/1) class-5 announcements SHOULD carry their AuthCert inline via a new optional-range TLV `0x81 AUTHCERT_ATTACH`, so a receiver's admission decision (doc 18 §5.2) doesn't have to wait for the cert to arrive separately via claim gossip — since in the target deployment the audience is mostly strangers to the announcer and the one-shot clamp would otherwise leave a genuine EMERGENCY rendered and queued as NORMAL exactly when it matters. | doc 18 §5.2.1 (heading "Self-contained elevated announcements (RECOMMENDED, CR2bis-5)"), §9 (Constants table) |
| **CR2bis-6** | Cited alongside CR2bis-5 in doc 18's status banner as "self-contained AuthCert TLV 0x81" — **text not preserved beyond the header mention; no separate disposition text is quoted anywhere in the citing document.** Likely refers to a companion aspect of the same TLV-0x81 fix (CR2bis-5) rather than a distinct mechanism, but this cannot be confirmed from repository text alone. | doc 18 header only ("Round 2-bis fixes CR2bis-2/10 …, CR2bis-3/11 …, CR2bis-5/6 (self-contained AuthCert TLV 0x81) applied") |
| **CR2bis-7** | Cost placement for garbage-manifest flooding (doc 16 §9.1): the manifest itself is AEAD-interior and invisible to relays, so the extra Ed25519 manifest-signature verify and the bounded parse fall only on **decrypting recipients**, never on relays (which pay only the ordinary bundle-signature-verify cost any class 1/2 bundle costs). | doc 16 §9.1 (parenthetical "(CR2bis-7)") |
| **CR2bis-8** | The thumbnail/full-file eager-vs-pull split is **role-gated, never size-gated**: a `file_root` MUST NOT be auto-fetched even if it happens to be small enough to sit under the generic ≤1 MiB auto-fetch ceiling — the manifest's declared *role* (`thumb_root` vs `file_root`), not byte size, decides eagerness. | doc 16 §5.1 ("Scope caveat (CR2bis-8)"), §7.1 (heading includes "CR2bis-8") |
| **CR2bis-9** | Scopes doc 17 §3.3's extension of doc 18's priority-admission clamp (from public class-5 announcements to private class-1 carriage telemetry) to **provisioned nodes only** — anchors and the conductor's device, the only nodes that receive or act on telemetry; passengers are un-provisioned and never apply the rule. Recorded explicitly so the broadening of doc 18's literal framing is never mistaken for a silent amendment to doc 18 itself. | doc 17 header ("Round 2-bis fix CR2bis-9 applied — §3.3 scoped to provisioned nodes"), §3.3, §9, §11 |
| **CR2bis-10** | Corrects the doc 18 §6 revocation slot design: the issuer **cannot** publish per-subject revocations under its own `(owner, field)` claim slot, because doc 06 §4 makes each such slot a **singleton LWW register** — a later revocation for subject B would silently supersede (un-gossip) subject A's in claim-digest sync. Disposition: the issuer instead publishes its **entire current revocation state as one value**, new claim field `0x08 REVOCATION_LIST`, body = a bounded list of `RevEntry` records (`count ≤ REVLIST_MAX = 48 ⚠`), with a "sticky union" receiver rule (once a receiver has seen a verified RevEntry, it stays revoked at that receiver even if a later list omits it, a deliberate departure from ordinary LWW symmetry). | doc 18 §6 (heading "Issuer-signed revocation list (the theft-capable path; slot design corrected per CR2bis-10)"), §7 row A1, §9 (Constants table, two rows) |
| **CR2bis-11** | Names and accepts the "declared-priority queue-jump" residual (doc 18 abuse row A7): because RBSR partitioning and TRANSFER sort key on the *declared* priority byte (CR2bis-3), any origin can self-declare priority 0/1 on junk and get earlier-in-session service with no AuthCert anywhere in the path. Disposition: this is not closeable by the clamp (which is receiver-local and per-receiver-divergent by construction); it is bounded instead by priority-blind per-origin token buckets, SeenIndex dedup, and the 60 s session cap — "priority buys order, never volume." An earlier draft's claim that the clamp governs "TRANSFER ordering" is explicitly withdrawn as unsupported by doc 05 §6's actual rule. The structural fix (an admission-aware sort key in doc 05 §6) is filed as an errata-candidate in `CHANGELOG.md` rather than applied in rev-B. | doc 18 §5.2.2 (heading "What the clamp does NOT govern: wire-phase ordering (honest residual, CR2bis-11)"), §7 row A7 |

---

## CR-S series — wire-format / signing hygiene fixes

These are cited as a distinct short series (`CR-S2`, `CR-S4`) alongside the `CR-A`/`N`/`CR2`
lines; no other `CR-S` numbers (e.g. `CR-S1`, `CR-S3`) are quoted anywhere in the
repository, so no entry is reconstructed for them.

| ID | Disposition (as recoverable) | Applied in |
|---|---|---|
| **CR-S2** | AEAD context-commitment requirement: plain ChaCha20-Poly1305 is not key-committing, so a multi-key-holding sender could craft ciphertext that decrypts differently for different recipients of the same nominal object (the "invisible salamander" partitioning attack). Disposition: wherever ChaCha20-Poly1305 carries a manifest/message body, the construction MUST commit to context via a BLAKE3 key/context-derived commitment. Recorded as a suite-level (doc 08) errata-candidate, inherited unmodified by doc 16's file-transfer manifest. | doc 16 §4.2 (heading "AEAD context commitment (normative, CR-S2; scope corrected per CR2-5)") |
| **CR-S4** | Normative signing-preimage hygiene, applied identically in docs 16 and 18: (a) the cipher-`suite` selector rides **in-preimage** even for fixed-width fields, so a weaker suite can never be silently substituted without invalidating the signature under `verify_strict`; (b) every variable-length field in a signed object is length-prefixed (`u16 len ‖ bytes`), never bare-concatenated, foreclosing preimage ambiguity. | doc 16 §2.1 (field comment "suite … CR-S4"), §2.2 ("CR-S4, §3.3"); doc 18 §2 ("Signed preimage (normative, CR-S4)"), §3 ("Signed preimage (CR-S4)") |

---

## IDs referenced by the backlog but not found quoted anywhere in the repository

`docs/AGILE_BACKLOG.md` §E0-S2 names the expected ID ranges as "CR-A1 through CR-A7"
and "N1–N6." The following numbers in those ranges are **not** quoted or paraphrased
in any specification document, README, or other file found by an exhaustive grep of
`specification/` and `implementation/` for `CR-A\d`, `N[1-6]`, `CR2-\d+`, `CR2bis-\d+`,
and `CR-S\d`:

- **CR-A2** — not found. (CR-A1, CR-A3, CR-A4, CR-A5/CA4, CR-A6, CR-A7 are all
  attested; CR-A2 is the one gap in the numeric sequence.)

No entry is fabricated for it. If the original council record surfaces later, this
gap should be the first thing checked against it.

---

## Cross-reference

Every ID above resolves to at least one specification citation. See also
`CHANGELOG.md`'s `[rev-B]` entry for the resulting wire/spec deltas, and
`docs/AGILE_BACKLOG.md` Epic 1 for the registry-doc reconciliation these findings
still require (E1-S1…S6).
