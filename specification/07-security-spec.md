# 07 — Security Specification

**mesh_milk threat model and defenses.** Status: Phase 3 draft; **revised 2026-06-11 post-audit** (`AUDIT.md`, `CHANGELOG.md`). Consolidates security arguments scattered through docs 03–06 into one auditable document. Adversary tier fixed in discovery: **local active adversary**.

> **Audit revision summary (this doc):** rows #3 (replay) and #17 (zombie lifetime) re-rated with the SeenIndex-from-first-sight + per-class dedup argument (AUDIT F1); the metadata note records origin is now an inline 32-byte pubkey (AUDIT F2, no privacy regression); residual #2 expanded for prekey suppression (AUDIT F6b); #12 receipt suppression aligned to exact-purge (AUDIT F11).

---

## 1. Adversary model

**In scope — the adversary CAN:**
- Receive, record, inject, replay, and jam radio traffic anywhere it has physical presence.
- Run any number of protocol-conformant or malicious nodes (Sybil keys are free).
- Participate honestly for a while, then defect (insider relays).
- Join public groups it can get invited to; befriend careless users.
- Correlate observations from several local vantage points it controls.

**Out of scope (documented, not defended):**
- Global passive observation of the whole city's RF (nation-state tier — explicitly de-scoped in discovery; see [A22] for the cost of defending it).
- Physical device seizure with a cooperative or coerced user; OS-level malware.
- RF fingerprinting of radio hardware (below-protocol layer).
- Long-term traffic-volume statistics by an adversary present at *most* contacts a victim makes (a weaker form is in scope, §5).

## 2. Assets and goals

| Asset | Goal | Mechanism (doc) |
|---|---|---|
| Message content | confidentiality, integrity, authenticity | E2E AEAD + signatures (08) |
| Identity keys | unforgeability | Ed25519; keys never leave device (08) |
| Presence ("X is nearby") | hidden from strangers | ephemeral beacon IDs (03 §5.1.1) |
| Social graph | hidden from strangers; consented disclosure to friends | SUMMARY scoping (06 §9), friendship consent (04 §4) |
| Network service (delivery) | availability under attack | spray redundancy, quotas, budgets (05) |
| Node resources (battery, storage) | bounded abuse | quotas, verify-on-sight (no store-before-verify), burst limits (03–05) |

## 3. Attack → defense matrix

| # | Attack | Vector | Defense | Residual risk |
|---|---|---|---|---|
| 1 | Identity spoofing | claim someone's NodeID | identity = key; NodeID = hash(key); all bundles signed; verify-before-store (03 §4.1.2) | none beyond key theft |
| 2 | MITM on first contact | active relay of handshake | Noise XX gives channel security vs passive; *authentication* is TOFU + out-of-band verification ceremony (QR/numeric, §6.1) | unverified contacts MITM-able by an active adversary present at first contact — inherent to serverless TOFU; surfaced in UX |
| 3 | Replay / lifetime-resurrection | re-inject recorded bundles (optionally with lifetime reset to class-max) | SeenIndex dedupe persisting **from first sight for class-max** (03 §4.2, AUDIT F1); lifetime clamp (03 §4.3); BundleID excludes lifetime so a reset bundle has an *identical* ID; **per-class application dedup backstop** (03 §4.2: ratchet index / claim seq / receipt MAC / symbol coords); Noise ephemerals (08) | resurrection past the first-sight horizon re-injects *authentic* content only; per-class dedup prevents any duplicate user-visible event; residual = one quota-bounded relay store/offer cycle (not "storage waste only" — the prior wording understated the window; now bounded explicitly) |
| 4 | Sybil flood | many fresh keys spam relays | per-origin token buckets at base rate; WoT multiplier rewards socially-anchored keys (05 §8); verify-on-sight via inline origin key + store quota (03 §4.1.2, AUDIT F2) | adversary still gets N_keys × base-rate; bounded by relay airtime budget (04 §5) — degrades fairness, not safety |
| 5 | Eclipse | surround a victim so all its contacts are hostile | spray diversity (L copies over time across *different* contacts); contact scheduler randomization (04 §6); WoT-diverse receipt paths; no single-path dependency anywhere | a *fully* eclipsed node (adversary controls 100% of its contacts) is partitioned — detection only: delivery-receipt starvation alarm (§6.3) |
| 6 | Route poisoning / sinkhole | advertise attractiveness to attract then drop traffic | utility strictly locally-observed (05 §4); no transitive trust; observed-usefulness scoring (04 §3) | attacker exhibiting *real* good behavior then defecting still wastes ≤ its attracted copies; spray keeps parallel paths |
| 7 | Blackhole relay | accept, never forward | same as 6; receipts reveal end-to-end success independently | silent copy loss ≤ copies given to attacker |
| 8 | Storage exhaustion | spam large bundles | quotas + eviction order (04 §2); per-origin buckets; MAX_BUNDLE 4 KiB; blob symbols pull-only (03 §5.2) | quota churn under sustained spam — old spray traffic evicted earlier |
| 9 | Battery drain | beacon spam → scan bursts; junk sessions | burst rate-limit + hostile-neighborhood fallback (04 §8); session admission caps; relay airtime budget | adversary forces fallback mode → degraded discovery for everyone in range (graceful, reversible) |
| 10 | Cache poisoning (bitchat class) | inject unverifiable data that propagates | **no store-before-verify window at all** (AUDIT F2): origin pubkey is inline, so every bundle is verified on sight; failures are dropped, never held or forwarded (03 §4.1.2) | none — the quarantine that previously held key-unknown bundles is eliminated, strengthening the closure |
| 11 | Pollution of blobs | corrupt symbols | per-symbol verification against signed BLAKE3 Merkle root before store (02 R-E1, 08 §8) | none |
| 12 | Receipt forgery / suppression | fake "delivered" to purge live traffic | receipt MAC under conversation key — only true recipient forges (05 §7.1); **purge requires an exact authenticated D0 receipt, never a Bloom hit** (06 §6.3, AUDIT F11) | Bloom FP ≈1% can only *deprioritize an offer* now (no false purge); other carriers unaffected (06 §6.4) |
| 13 | Group infiltration | malicious member | membership is admin-signed (06 §8); member sees content by design — social problem; epoch re-key on removal limits future access (08 §6) | past traffic readable by past members (forward secrecy is per-epoch, not retroactive — standard sender-keys property [F2]) |
| 14 | Tracking via beacons | follow a device by its broadcasts | ephem_id rotation per coarse time slot, salt per slot (03 §5.1.1); no stable fields in beacons | correlation by RF fingerprint or by continuous physical following — out of scope |
| 15 | Traffic analysis (local) | who syncs with whom, volumes | sessions are AEAD-opaque; PADDING TLV (03 §4.4) for length shaping (SHOULD); domain structure invisible inside channel | contact *occurrence* between two physically present devices is observable — accepted at this tier; volumes leak coarsely |
| 16 | Jamming | RF denial | none (physics) | documented; store-carry-forward inherently routes *around* jammed areas via mobility |
| 17 | Zombie lifetimes / resurrection | relay inflates remaining lifetime or resurrects a captured bundle | clamp to class maximum (03 §4.3) + **SeenIndex-from-first-sight** (03 §4.2) + per-class app dedup (AUDIT F1) | bounded: ≤ one quota-capped relay store/offer cycle per resurrection past the dedup horizon; no duplicate user-visible delivery |
| 18 | WoT quota inference | measure quota treatment to map social ties | acknowledged side channel (05 §12); paranoid build: uniform quotas | coarse "relay favors origin" bits for a co-present adversary |
| 19 | Stolen conductor (AuthCert) key | attacker exfiltrates a live AuthCert's private key | per-cert budget × remaining-validity bound; issuer revocation list, sticky union (18 §6/§7 A1) | bundles admitted before the revocation list propagates cannot be un-admitted after local render — accounting, not prevention (18 §5.4(b)) |
| 20 | Profile-B self-issued authority | no operator root exists; attacker self-issues AuthCert-shaped claims | no trust root to pin/gossip against ⇒ priority 0/1 never admits; UI shows "unverified" for every sender (18 §7 A4) | depends entirely on the UX banner being non-bypassable — flagged for doc 12 conformance tests |
| 21 | False-panic announcement | a legitimately-authorized issuer signs a false EMERGENCY announcement | out of cryptographic scope by design; accountability via non-repudiable signature + identity-visible UX (18 §7 A5) | social/procedural, not protocol-solvable — documented, not hidden |
| 22 | Stolen operator ROOT key | attacker exfiltrates the root key, mints unlimited fresh AuthCerts | scope containment (a stolen root grants nothing outside its scope namespace); dual-control minting; re-pin recovery (18 §7 A6) | **unbounded within the scope until re-pin** — the standard, irreducible PKI-root risk, scoped to one operator's namespace rather than a global CA |
| 23 | Declared-priority queue-jump | any origin self-declares priority 0/1 on junk bundles; wire-phase ordering services it earlier | none at wire-phase ordering in v1, stated plainly; priority buys order, never volume — buckets/quotas stay priority-blind (18 §5.2.2/§7 A7) | earlier-in-session service for junk within unenlarged budgets — a scheduling nuisance, not a safety breach; structural fix filed as errata-candidate |
| 24 | Garbage-manifest quota exhaustion | flood of well-formed-but-useless file manifests (fake names, dangling roots) | costs bounded by the existing per-origin token bucket like any class 1/2 bundle; the extra manifest-verify cost falls only on decrypting recipients (16 §9.1) | Sybil residual unmodified (#4) — spreading across fresh identities still forces one verify-before-store per identity |

Doc 18's own §7 additionally covers A2 (replay of a valid announcement), which doc 18 itself cross-references back to this matrix's #3/#17 (SeenIndex-from-first-sight + per-class dedup) rather than introducing a new mechanism — no separate row needed here. A3 (cross-scope priority injection) is closed entirely by a mechanism specific to authority announcements (scope-match admission, doc 18 §5.2) with no general-catalog analog, so it is not pointed to from this matrix — see doc 18 §7 directly.

## 4. Bridgefy/bitchat failure classes → explicit closure

Doc 02 F-A2 catalog, item by item:

| Historical failure | mesh_milk closure |
|---|---|
| Bridgefy: tracking via stable IDs | ephem_id (03 §5.1.1); matrix #14 |
| Bridgefy: impersonation | identity=key, all-bundle signatures; #1 |
| Bridgefy: no confidentiality | E2E AEAD, doc 08 |
| Bridgefy: one-packet network DoS | no unauthenticated parsing influences behavior (03 §10); per-origin quotas; fuzzing mandate (doc 12 §test) |
| Bridgefy: metadata after libsignal adoption | metadata budget designed-in (§5), not bolted on |
| bitchat: payload-only signatures | signature covers all canonical bytes (03 §4.1) |
| bitchat: identity from packet fields | NodeID binds to key; keys verified before trust (03 §3.2) |
| bitchat: cache-before-verify propagation | no cache-before-verify exists — inline origin pubkey ⇒ verify-on-sight (03 §4.1.2, AUDIT F2); #10 |
| bitchat: identity-from-packet-field (re-confirmed) | origin field *is* the Ed25519 pubkey, signature-checked; NodeID derived, never trusted alone (03 §3.2) |
| bitchat: static session keys | Noise ephemerals per session; conversation ratchet (08 §5) |

## 5. Metadata budget (who learns what)

| Observer | Learns | Does NOT learn |
|---|---|---|
| Stranger in radio range | someone runs MMP nearby (beacon); session occurrences between devices it can see; coarse volumes | identities, content, destinations, social graph |
| Authenticated session peer (non-WoT) | your NodeID + claims you publish; which domains you sync (D0–D3 sizes); bundles you offer in D3 (origin pubkey + dest ID on relayed bundles — see note) | your conversations' content, your contact list, group memberships (GroupIDs salted, 06 §2) |
| WoT contact | + shared-contact claim digests (06 §9) | content of your other conversations |
| Friend (consented) | + your WANT set, receipt filters, claim digest — disclosed in FRIEND_OFFER UX (04 §8) | message content (still E2E) |
| Relay carrying your bundle | origin pubkey (= NodeID) & dest ID, size, lifetime, class | content, your other traffic |
| Group member | group content + membership | your non-group activity |

**Note — relay-visible addressing:** bundle headers expose the origin's full **identity public key** (32 B, inline since AUDIT F2 — `NodeID = BLAKE3(origin)[0..16]` derived) and the dest NodeID/GroupID to carriers (origin pubkey needed for unconditional verify-before-forward; dest needed for routing utility). The pubkey is no more linkable than the NodeID it derives to (the design was already pseudonymous, not anonymous, at this tier) — **no privacy regression.** This is the protocol's largest deliberate metadata concession at this threat tier. Mitigations available without redesign: dest-only addressing with origin sealed into payload (costs utility steering for receipts), or per-conversation ephemeral dest IDs negotiated in-channel (costs first-contact reachability). Both are specified as **optional privacy profiles** in v2 scope — recorded as OQ-9. The nation-state tier solution (mixing, cover traffic [A22]) remains out of scope.

## 6. UX-security requirements (normative for conforming apps)

6.1. **Verification ceremony:** apps MUST offer QR/numeric-comparison verification of identity keys; verified state MUST be visibly distinct; TOFU contacts MUST be distinguishable from verified ones (matrix #2).
6.2. **Friend disclosure:** FRIEND_OFFER UI MUST enumerate what the friend learns (§5 row 5) before consent (04 §8).
6.3. **Eclipse/starvation alarm:** if no receipt returns for any message for T_alarm (24 h ⚠) while contacts occur, the app SHOULD surface "your traffic may be intercepted or you may be isolated."
6.4. **Compromise indicator:** equal-seq conflicting claims (06 §4.2) MUST surface as a key-compromise warning to the claim owner and SHOULD to observers.
6.5. **Block silence:** blocking MUST NOT signal the blocked party (05 §8: silent drop).

## 7. Residual risk register

1. First-contact MITM window for unverified contacts (TOFU limit) — mitigated socially (ceremony), not cryptographically.
2. First-message forward secrecy absent when no prekey claim is available (08 §4) — message is still E2E confidential; compromise of recipient's *exchange key* later exposes it. An active relay-adversary can **force** this state by suppressing fresh prekey-batch claims (08 §4.2, AUDIT F6b) — the eclipse vector (#5) applied to prekeys; the floor is the always-available sealed-box baseline (no FS), never less.
3. Contact-occurrence and coarse-volume visibility to co-present observers (#15).
4. Fully-eclipsed node partition (#5) — detect, can't prevent.
5. WoT quota side channel (#18).
6. Receipt-filter FP suppression, ~1% per pair (#12).
7. Sybil base-rate consumption (#4) — fairness degradation under industrial Sybil presence.
8. Stolen operator ROOT key (#22, doc 18 §7 A6) — **unbounded within the compromised scope until re-pin**, the one entry in this register that is not merely bounded but genuinely open-ended pending an out-of-band recovery ceremony; contained to one operator's scope namespace, never global, which is the honest bound this entry actually has.
9. Stolen conductor (AuthCert) key (#19, doc 18 §7 A1) — bundles admitted before a revocation list propagates cannot be un-admitted after local render; bounded by budget × remaining validity, unlike #8.
10. Declared-priority queue-jump (#23, doc 18 §7 A7) — no defense at wire-phase ordering in v1; bounded to earlier-in-session service within unenlarged per-origin budgets, a scheduling nuisance rather than a safety breach.

Each is (a) stated honestly rather than hidden, (b) bounded except item 8 (explicitly flagged as the exception), (c) has a named escalation path (privacy profiles OQ-9, paranoid build, v2 scope, or — for items 8-10 — the doc 18 §6/§7 mechanisms themselves).

## 8. Persona review checkpoints

- **Security Auditor — Potential flaw:** "Matrix #2 understates risk: in a protest scenario the adversary is *expected* at first contact." *Resolution:* added 6.1 visible-distinction mandate + recommendation that high-risk deployments pre-exchange keys out-of-band (QR over existing channels) — the serverless setting offers no third option that doesn't fail like Bridgefy's; accepted as the honest boundary of this threat tier.
- **Skeptical Professor — Potential flaw:** "Claiming 'no unauthenticated parsing influences behavior' is unfalsifiable without implementation discipline." *Resolution:* converted to testable requirements: doc 12 mandates fuzzing of all pre-auth parsers (beacon, handshake first message) and a parser-isolation rule (no allocation proportional to unauthenticated input beyond fixed frames). Carried forward.
- **Distributed Systems Reviewer:** eclipse detection via receipt starvation produces false alarms in genuinely sparse deployments. *Resolution:* alarm conditions on *contacts occurring* without receipts, and wording is "may be isolated" — calibrated, not cried-wolf. Accepted.
- **Battery Specialist:** PADDING (length shaping) as SHOULD, not MUST — at this threat tier, mandatory padding spends airtime/energy on an out-of-scope adversary. Confirmed as SHOULD with profile escalation.
- **Inventive Scientist — Alternative considered:** proof-of-work admission for unknown origins instead of quota-only metering. Rejected (discovery decision: PoW hostile to IoT/battery; per-origin quotas + verify-on-sight achieve the same bound without burning joules).

## 9. Cross-references

Crypto mechanisms backing #1–3, 11–13 → doc 08. Quota/budget mechanics → docs 04–05. Fuzzing/parser mandates → doc 12. OQ-9 (privacy profiles) appended to research/open-questions.md. Failure-mode interactions (e.g., eclipse + storage pressure) → doc 14. File-transfer abuse analysis (#24) → doc 16 §9. Authority-announcement abuse analysis and the full A1–A7 catalog (#19–23) → doc 18 §7.
