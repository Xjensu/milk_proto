# 06 — Synchronization Specification

**mesh_milk sync layer.** Status: Phase 3 draft; **revised 2026-06-11 post-audit** (`AUDIT.md`, `CHANGELOG.md`). Implements R-D1 (doc 02 §4): set reconciliation for bundles, single-writer LWW for claims, OR-Set for group membership, causal DAG for conversations. Runs inside the authenticated session channel (doc 03 §5.2). Constants ⚠ provisional until doc 13.

> **Audit revision summary (this doc):** §6.3 makes **purge authority exact** (Bloom is a hint, never deletes; AUDIT F11); §8.5 adds **OR-Set tag/tombstone GC** via epoch anchors (AUDIT F5); §4.1/§4.2 reference the global signing-domain scheme (AUDIT F4).

---

## 1. What is synchronized

| Data class | Structure | Mechanism | Section |
|---|---|---|---|
| Bundles (msgs, receipts, symbols) | expiring set of (BundleID → bundle) | RBSR | §2–3 |
| Claims (profile fields, exchange keys, prekeys, friendships) | per-(NodeID, field) LWW register | digest + LWW merge | §4 |
| Conversation order | causal DAG via PARENTS TLV | carried in bundles; WANT repair | §5 |
| Delivered-set | ReceiptFilter (rotating Blooms) | filter union | §6 |
| Group membership | OR-Set per group | deltas inside group channel | §8 |

## 2. Reconciliation domains

Bundle reconciliation is partitioned into **domains** processed in priority order; a dying contact loses only the tail:

1. `D0`: class 0 control (receipts, key claims) — always first.
2. `D1`: class 1 direct bundles **destined to either session peer** (or their friends).
3. `D2`: class 2 group bundles for groups both peers carry (group interest advertised as GroupID hashes in SUMMARY — salted per-session to avoid cross-session linkage ⚠).
4. `D3`: relay spray inventory (everything else carried, subject to doc 05 admission).
5. `D4`: blob symbol availability (root hashes only; symbols transfer on request).

Items with remaining lifetime < 10 min ⚠ are excluded from offers (OQ-1: expiry-race waste; an item about to die is not worth a transfer).

## 3. RBSR (Range-Based Set Reconciliation) over BundleIDs

Adapted from Meyer's RBSR / Negentropy [D7][D8]; element space = 128-bit BundleID, uniformly distributed (BLAKE3 output → no adversarial clustering of the *ID space*; see §3.5).

### 3.1 Range fingerprints

`FP(range) = BLAKE3(xor-fold of IDs in range ‖ count)[0..8]` ⚠ — 64-bit fingerprint. XOR-fold alone is forgeable by ID choice; hashing the fold with the count restores second-preimage resistance for session purposes (IDs themselves are hashes an adversary cannot freely choose without producing valid signed bundles).

### 3.2 Round structure

```
INIT:  A → B: domain, [full-range FP, count]
STEP:  B compares; for each mismatched range:
         count ≤ T_list (=24 ⚠)  → send ID list (terminal)
         else                     → split into B_f (=16 ⚠) equal sub-ranges, send FPs
       B → A: range msgs (+ its own ID lists for terminal ranges)
       …alternate until no mismatches…
WANT:  each side requests unknown IDs; TRANSFER phase sends bundles.
```

Expected rounds ≈ ⌈log_{B_f}(n/T_list)⌉ + 1 → ≤3 rounds for n ≤ 100k items; messages are compact (16 FPs × 12 B ≈ 200 B per split). With the SUMMARY-phase Bloom pre-filter (doc 03 §5.2) most contacts skip straight to small terminal lists (OQ-5 modeling in doc 13).

### 3.3 State

RBSR is stateless between sessions (no per-peer sync cursors — peers at 10k scale would explode cursor state; rationale [D7] DoS-resistance). The only cross-session memory is the ReceiptFilter (§6) and the BundleStore itself.

### 3.4 Resumability

Each round is idempotent; a dropped session leaves both stores valid (doc 03 §5.2). Re-contact restarts RBSR; the Bloom pre-filter makes the restart cheap.

### 3.5 Adversarial considerations

A malicious peer can lie about FPs/IDs — worst case it wastes the session's airtime (bounded by doc 04 relay budget) or withholds items (equivalent to not carrying them — spray redundancy covers). It cannot corrupt the store: every transferred bundle is re-verified (doc 03 §4.1.2). Fingerprint collisions across *honest* stores: 2⁻⁶⁴ per comparison — negligible at our scales.

## 4. Claims: single-writer LWW

### 4.1 Claim record

`Claim { owner: key32, field: u8, seq: u32, body, sig: Ed25519 }` — the signature uses the global signing-domain scheme `mmp1/claim` over `owner ‖ field ‖ seq ‖ body` (doc 10 §3.1, AUDIT F4). `owner` is the full 32-byte identity public key (claims are how peers learn keys); `NodeID = BLAKE3(owner)[0..16]` derived.
Fields v1: 0x01 nickname (≤32 B UTF-8), 0x02 avatar_root (blob hash), 0x03 exchange_key, 0x04 prekey_batch (doc 08), 0x05 friendship (counter-signed), 0x06 group_epoch_anchor (doc 08 §6).

**Rev. B registry additions (pointers only — defined and owned by doc 18, no v1 format change):** claim field `0x07 = AuthCert` (subject-signed grant-holding assertion; issuer's independent `mmp1/authcert` signature lives inside the body — doc 18 §2.1) and claim field `0x08 = REVOCATION_LIST` (issuer-owned, whole-state revocation value — doc 18 §6). Mirrors doc 03 §4.4's pointer-only pattern for the TLV registry; this is the field registry those pointers refer back to.

**Unknown-field handling (normative).** A claim record whose `field` value this node does not implement is stored and re-offered exactly like any recognized claim — the claim wrapper's own `mmp1/claim` signature over `owner ‖ field ‖ seq ‖ body` is independently verifiable regardless of whether `body`'s internal schema is understood — but `body` itself is never interpreted or acted on. This mirrors the doc 03 §8 unknown-bundle-class rule and the doc 03 §4.4 unknown-optional-TLV rule (ignore-if-unknown, relay/store if the outer signature verifies): the same posture, applied at the claim-field granularity. (Confirms doc 18 §2.1's assumption of this rule; that doc previously flagged it as unconfirmed pending this amendment.)

### 4.2 Merge rule (normative)

Higher `seq` wins. Equal `seq`, different body (key compromise or owner bug): the body with lexicographically smaller BLAKE3(body) wins — *deterministic everywhere*, no clock involved; the anomaly is surfaced to the owner's UI as a compromise indicator (doc 07). Claims expire per LIFETIME_CLAIM (7 d) but a node retains the latest claim of every ContactGraph peer indefinitely (your contacts don't vanish; strangers' claims do).

### 4.3 Claim sync

SUMMARY carries a **claim digest**: for each peer in the intersection of interest (session peers themselves + shared contacts up to 64 ⚠), `(NodeID[0..4], field, seq)` tuples — a few hundred bytes. Differing seq → ship the newer claim. Full ProfileStore walks never happen on-air.

## 5. Conversation DAG

5.1. Every class 1/2 message bundle SHOULD carry PARENTS = up to 4 BundleIDs: the sender's current heads of that conversation. Heads = messages with no known children.

5.2. **Display order** (normative for consistent UX): topological order of the DAG; concurrent siblings ordered by (DAG height, then BundleID ascending). No wall clocks in ordering (devices lie; doc 03 §4.3 philosophy).

5.3. **Orphan repair:** a message whose parent is unknown renders with a gap marker; the node adds missing parent IDs to its WANT set, requested in every session's RECONCILE for that conversation's domain. Orphans older than the class lifetime resolve to permanent gaps (the parent expired network-wide — by design; the DAG tolerates holes because edges are advisory ordering, not integrity: each message is independently signed and readable).

5.4. **Why not stronger:** doc 02 F-D4 — global vector clocks rejected (state explosion); per-sender seq rejected for ordering (OQ-1: expiry gaps would block). The DAG gives exactly causal-when-known, eventual-everywhere semantics, matching minutes-tier latency expectations.

## 6. ReceiptFilter

6.1. Structure: **12 time buckets × 6 h** ⚠ (covers 72 h max lifetime). Per bucket: Bloom filter, k=7 hash functions (BLAKE3-derived indices), sized at 9.6 bits/element for target FP ≈ 1% at capacity (textbook Bloom dimensioning: FP = (1−e^(−kn/m))^k). Standard capacity 8192 receipts/bucket → 9.8 KiB/bucket, 118 KiB total ⚠ (Anchor: ×4; Core: 2 buckets × 256 entries ≈ 600 B).

6.2. Rotation: buckets age out with their time window (monotonic clock); rotation never deletes bundles — only the receipt *index* expires (the bundles it suppressed have expired too, same window).

6.3. Union on contact: peers exchange bucket Blooms in SUMMARY (newest 2 buckets full ⚠, older on demand); receiving node ORs them into its own same-window buckets **only for receipts it can attribute to an authenticated session**. **Purge authority is exact, not probabilistic (AUDIT F11):** the OR'd Bloom is used solely to *deprioritize offering* a bundle to a peer; it MUST NOT trigger deletion. A stored bundle is purged (eviction class 2, doc 04 §2) only when the **exact receipt bundle** for it is reconciled in domain D0 (an authenticated class-0 receipt whose origin signature verifies, doc 05 §7.1). Rationale: a Bloom false positive (~1 %) would otherwise delete an *undelivered* copy — loss in the unsafe direction. Beacons deprioritize, session Blooms deprioritize, **only exact D0 receipts purge.**

6.4. FP analysis (post-AUDIT F11): a false positive only suppresses *offering* a live bundle to one peer (other carriers unaffected) — it can **no longer purge** a stored copy, since purge now requires an exact D0 receipt (§6.3). Residual loss is therefore one missed offer per FP per pair, fully recovered by other carriers and spray redundancy L ≥ 4; the prior "may purge one stored copy" hazard is eliminated. System-level delivery impact modeled in doc 13.

## 7. Friendship proxy sync

7.1. The dependent leaves with its friend a **signed proxy summary**: its WANT set (conversations, group interests), its ReceiptFilter top buckets, and its claim digest — all signed by the dependent with an issue timestamp (dependent's monotonic counter, not wall clock) and a validity horizon (24 h ⚠).

7.2. The friend answers D1/D2 reconciliation on the dependent's behalf, marking answers `proxy(age=…)`. Peers MAY discount stale proxies (age > 12 h ⚠ → treat as hints).

7.3. On poll, the dependent and friend reconcile directly (ordinary RBSR, friend's buffered set vs dependent's store); the friend then garbage-collects buffered bundles the dependent now holds (receipt semantics stay end-to-end — doc 04 §4.4).

## 8. Group membership OR-Set

8.1. Per group: `MemberSet = OR-Set of (member NodeID, add_tag)` with admin-signed operations: `ADD(member, tag, admin_sig)`, `REMOVE(set of observed add_tags, admin_sig)`. Add-wins semantics on concurrent ADD/REMOVE of the same member (standard OR-Set [D1][D2]); deterministic, no coordination.

8.2. Operations travel **only inside the group-encrypted channel** (class 2 control messages) — non-members never see membership, only opaque GroupID traffic volumes (metadata budget, doc 07).

8.3. Every effective membership change bumps the **epoch**; the admin signs an epoch anchor claim (group_epoch_anchor: epoch, member-set BLAKE3 root); senders re-key per doc 08 §6. Members who miss epochs catch up via the anchor + missing ops requested in-channel.

8.4. Admin model: 1..n admin keys listed at group creation (immutable set v1 ⚠ — admin rotation deferred to v2, recorded as OQ-8); any admin signs ops; conflicting concurrent admin ops merge by OR-Set rules.

8.5. **Tag/tombstone GC (normative, AUDIT F5).** An OR-Set otherwise accumulates add-tags and removal tombstones monotonically — unbounded for a long-lived churny group. Compaction uses the epoch anchor (§8.3) as a mutual checkpoint: when both (all) admins have signed/acknowledged an epoch anchor carrying the membership BLAKE3 root for epoch `e`, every add-tag and tombstone whose effect is already reflected in that root MAY be discarded; the anchor's root becomes the new convergence baseline. This is safe because compaction never crosses a checkpoint the parties have not both observed — concurrent ops still in flight reference tags newer than the last mutual anchor and are retained. Result: membership state is bounded by `O(current members) + O(in-flight ops since last mutual anchor)`, not by lifetime churn. A node that has missed the anchor requests it (with the post-anchor op log) before compacting.

8.6. **Removal latency is the security parameter (AUDIT F6, disclosure).** Under partition, a `REMOVE(X)` takes effect for a given node only once that node observes it; until then, members in a disjoint partition keep encrypting to an epoch X can read (no decentralized scheme — MLS included — can do better without an arbiter, §6.5 / doc 08 §6.5). Apps SHOULD surface that removal "takes effect as the network reconverges," not instantly (doc 07 §6).

## 9. Persona review checkpoints

- **Distributed Systems Reviewer — Potential flaw:** "Stateless RBSR re-reconciles the whole domain every contact; at Anchor scale (millions of IDs) the first FP exchange is fine but splits get deep." *Resolution:* domains partition the space (D0–D4), and D3 relay inventory is additionally windowed to bundles admitted in the last 24 h ⚠ for stranger sessions (full-depth reconciliation reserved for Anchor↔Anchor and friend pairs). Accepted; depth math in doc 13.
- **Security Auditor — Potential flaw:** "Claim-digest in SUMMARY leaks which contacts two strangers share (4-byte NodeID prefixes)." *Resolution:* digest scope for non-WoT sessions reduced to the two session identities only; shared-contact digests exchanged only when both peers are in each other's WoT (consented social layer). Accepted — tightens doc 03 §5.2 SUMMARY definition.
- **Skeptical Professor — Potential flaw:** "Bloom FP math assumes honest insertion counts; an adversary spamming receipts could saturate buckets, raising FP for everyone." *Resolution:* receipts only enter the filter after origin-signature + conversation-MAC verification (doc 05 §7); saturation requires real deliverable traffic, which per-origin quotas bound. Residual risk noted in doc 07.
- **Battery Specialist:** SUMMARY fixed cost ≈ 2 Blooms (19.6 KiB) is too heavy for a 3-second sensor contact. *Resolution:* Core tier exchanges 600 B mini-filters; Standard sends bucket *deltas* since last contact with that peer when PeerCache remembers one (optimization, not protocol state — falls back to full). Accepted.
- **Inventive Scientist — Alternative considered:** skip-list/Merkle-tree store digests instead of RBSR (single-RTT diff for near-identical stores). Rejected for v1: per-peer tree state + round trips on divergence (F-D3); RBSR + Bloom pre-filter achieves the same best case statelessly.

## 10. Cross-references

Frame layouts (RBSR messages, claim records, filter encodings) → doc 10. Crypto for receipts MAC, group channel, prekeys → doc 08. Metadata implications of SUMMARY content → doc 07. ⚠ constants + Bloom/RBSR depth math → doc 13. New open question OQ-8 (admin rotation) appended to research/open-questions.md.
