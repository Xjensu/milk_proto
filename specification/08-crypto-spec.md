# 08 — Cryptography Specification

**mesh_milk key management and encryption.** Status: Phase 3 draft; **revised 2026-06-11 post-audit** (`AUDIT.md`, `CHANGELOG.md`); **§5.3 ratchet-persistence amendment 2026-07-16**. Citations [F*] in research/bibliography.md. Constants ⚠ provisional.

> **Audit revision summary (this doc):** §3.2 channel binding now uses the global signing-domain scheme and **commits both identity public keys** (AUDIT F4); §4.2 adds the **prekey-suppression** failure mode (AUDIT F6b); §10 mandates **canonical-S (`verify_strict`)** Ed25519 verification (AUDIT F9).

---

## 1. Cipher suite 0x01 (mandatory v1)

| Role | Primitive | Note |
|---|---|---|
| Signatures | Ed25519 | identity, claims, bundles |
| Key agreement | X25519 | exchange keys, Noise DH, sealed boxes |
| AEAD | ChaCha20-Poly1305 (IETF) | sessions, payloads, group msgs |
| Hash / KDF / MAC (application layer) | BLAKE3 (hash, derive_key, keyed modes) | BundleID, Merkle roots, ratchets, ephem_id |
| Handshake hash (inside Noise only) | BLAKE2s | §3 rationale |

**Suite 0x02 (reserved):** hybrid X25519+ML-KEM-768 — §9.

**Two hash functions, deliberately:** the session handshake instantiates a *standard, formally analyzed* Noise configuration — `Noise_XX_25519_ChaChaPoly_BLAKE2s` — because the symbolic-verification results from Noise Explorer [F3] apply to standard instantiations; substituting BLAKE3 into Noise would exit the verified space for zero functional gain. Everywhere outside Noise, BLAKE3 is used for its keyed/derive modes and `no_std` performance. Cost: ~2 KB extra code on Core targets — accepted (Performance Engineer reviewed).

## 2. Key hierarchy

```
Identity key (Ed25519, permanent, never leaves device)
 ├─ signs → Exchange key claim (X25519 static, seq, rotatable)
 ├─ signs → Prekey batch claim (N one-time X25519 pubkeys, §4.2)
 ├─ signs → all bundles, claims, friendship records
 └─ Noise XX static (the X25519 exchange key doubles as Noise s)
      └─ Session keys (Noise ephemeral → transport AEAD pair, per contact)
           └─ (first session between two identities) → Conversation root CK₀
                ├─ Conversation send/recv chains (symmetric ratchet, §5)
                ├─ Receipt MAC key  = BLAKE3.derive_key("mmp1 receipt", CK_epoch)
                └─ contributes to → Group sender keys (distributed pairwise, §6)
```

Identity keys never rotate (doc 03 §3.3): compromise = identity replacement; this is the explicit, simple rule consistent with petname identity (no registrar to attest a rotation anyway). Exchange keys and prekeys rotate freely via claims.

## 3. Session handshake

3.1. Pattern: **Noise XX** [F1], mutual authentication, both static keys transmitted *encrypted* (identity hiding vs passive observers — matches pseudonymous beacons; an IK responder would require knowing the peer's static beforehand, impossible for stranger contacts, and XX's identity-hiding for the initiator is strictly better for our beacon-pseudonymity model [F1 §7/identity-hiding]; one extra message costs ~1 RTT ≈ 50–100 ms per contact — negligible against multi-second contacts).

3.2. Static key = the identity's current X25519 exchange key; immediately after the handshake, each side sends (inside the channel) its identity pubkey + a fresh Ed25519 ID_PROOF signature — **channel binding**. Per the global signing-domain scheme (doc 10 §3.1, AUDIT F4) the signed preimage is `"mmp1/chanbind"‖0x01 ‖ noise_handshake_hash ‖ initiator_pubkey ‖ responder_pubkey`: committing to **both** identity public keys (not the handshake hash alone) forecloses identity-misbinding / unknown-key-share, and the domain tag prevents this signature being replayed as a bundle/claim signature. Because Noise XX's `h` binds both ephemerals, the proof is session-unique and cannot be replayed into another session. This proves the Ed25519 identity controls *this* session and prevents exchange-key substitution. Verify against known claims (or TOFU-record, doc 07 §6.1); verification MUST use canonical-S (`verify_strict`, §10).

3.3. Session keys are ephemeral per contact (forward secrecy at the transport layer — a recorded session is unreadable after both ephemerals are gone). Resumption tickets (PeerCache, doc 04) MAY skip re-handshake within 15 min ⚠ via Noise PSK-resumption; tickets are single-use.

## 4. Asynchronous (DTN) message keys — first contact between identities

The DTN problem: Alice writes to Bob who is *hops and hours away* — no session possible.

4.1. **Baseline (always works):** sealed box [F4] — ephemeral X25519 against Bob's exchange-key claim; ephemeral pubkey rides in the payload header; AEAD = ChaCha20-Poly1305 under BLAKE3-derived key. Properties: confidentiality + Bob-only readability; **no forward secrecy** until Bob's first reply (exchange-key compromise later exposes it) and no sender authentication at the AEAD layer — sender authenticity comes from the bundle's Ed25519 signature (doc 03 §4.1). Honest limitation, doc 07 residual #2.

4.2. **Prekey upgrade (SHOULD, X3DH-adapted [F2]):** identities gossip *prekey batch claims* — M=16 ⚠ one-time X25519 prekeys + 1 medium-term signed prekey, all Ed25519-signed (`mmp1/prekey` scheme, doc 10 §3.1). Senders consume an unused one-time prekey (marked locally; collisions across senders possible since no server coordinates consumption — handled: Bob accepts any of his unexpired prekeys, replay-scoped by BundleID dedupe). DH set: X3DH-style (sender-ephemeral × signed-prekey, sender-ephemeral × one-time, sender-static × signed-prekey) → first-message forward secrecy ≈ Signal's, minus the server's one-time-use guarantee (degradation: a one-time prekey may serve several first-messages within its lifetime — FS still holds vs *future* exchange-key compromise, slightly weaker compartmentalization).

**Two adversary-controllable degradations (both bounded; AUDIT F6b):**
- *Replay-consume:* an attacker grabs the published claim and "uses" all one-time prekeys network-wide. Unenforceable to prevent without a server (no one, including the attacker, can be blocked from using a published prekey); effect = FS quality for first messages degrades toward the signed-prekey level. No confidentiality/integrity loss.
- *Suppression (the more severe, newly documented):* an active relay-adversary withholds or fails to relay a victim's *fresh* prekey-batch claims, leaving senders with only a stale (lower-`seq`, still validly-signed) batch or — if it strips prekey claims entirely — the §4.1 sealed-box baseline (no FS). It cannot forge a higher-`seq` claim (unsigned), but it can *eclipse newer ones*. Net effect: an adversary that controls a sender's relays can force first-message FS down to the always-available baseline at will. This is the eclipse/relay-suppression vector (doc 07 #5) applied to prekeys; it never drops below the baseline confidentiality guarantee, so the floor is "no worse than sealed-box."

This serverless adaptation and both degradations are explicitly non-standard — flagged for external cryptographic review (OQ-10, scope now includes suppression).

4.3. First reply (or first co-located session) establishes the Conversation root CK₀ = BLAKE3.derive_key("mmp1 conv", DH transcripts ‖ both identities) and retires the async constructions for that pair.

## 5. Conversation ratchet (pairwise)

5.1. **Symmetric forward ratchet per direction:** message key K_i = BLAKE3.derive_key("mmp1 msg", CK_i); CK_{i+1} = BLAKE3.derive_key("mmp1 ck", CK_i); CK_i erased after advance. Out-of-order tolerance (DTN reordering is the norm): receiver stores skipped message keys up to W=512 ⚠ per conversation, bounded by message lifetime (skipped keys older than 72 h purge — a message that old is expired anyway).

5.2. **DH ratchet step at contact:** whenever the two conversation parties complete a live session (Noise XX), they mix fresh entropy: CK_epoch+1 = BLAKE3.derive_key("mmp1 epoch", CK_epoch ‖ session key material). This is the DTN adaptation of the Double Ratchet's DH step [F2-adjacent]: **post-compromise security is opportunistic — it heals when (and only when) the parties actually meet or exchange interactive traffic.** Between meetings there is forward secrecy (5.1) but no PCS. Stated plainly; matches the threat tier and the physics — an interactive ratchet cannot be required by a store-and-forward network.

5.3. **Ratchet-state persistence across restart (amendment 2026-07-16).** Decision: **on phones/PC, conversation ratchet state SHALL be persisted across process restart, and only under a platform-keystore-wrapped encryption key; ratchet state MUST NOT be written to storage outside that wrapping. Core/`no_std` targets do not persist ratchet state.** This supersedes the implementation handbook's earlier tentative "don't persist for v1" default; the reasoning follows.

*State inventory (what "ratchet state" is, exactly, per the reference implementation `mmp-crypto/src/ratchet.rs`):* per conversation, one send chain `(CK, epoch u16, next_index u32)` and one receive chain `(CK, epoch, next_index)` plus the skipped-key store — up to W (§5.1) already-derived **message keys** for mesh-reordered bundles, keyed by `(epoch, index)`. The conversation root CK₀ is a construction input only; it is **excluded from the persisted set** (normative — see below).

*Threat analysis — what a storage snapshot yields.* Adversary: copies the at-rest blob at time T (imaged/lost device, extracted backup) without live device compromise.

- **Backward secrecy holds for delivered traffic.** Chain advance is one-way (5.1: CK_{i+1} = BLAKE3.derive_key, spent CK erased), so message keys below the persisted `next_index` are not derivable from the snapshot. This is precisely why CK₀ must be excluded: both direction seeds derive from CK₀, so a persisted CK₀ re-opens the *entire* epoch-0 history in both directions. With CK₀ excluded, no past-delivered plaintext is exposed.
- **The skipped-key window is the one past-facing exposure:** the snapshot holds literal message keys for ≤ W messages sent before T but not yet arrived (DTN reordering). These decrypt if also captured from the mesh. Bounded by W and by bundle lifetime (72 h ⚠ — older traffic has expired out of the mesh).
- **Forward exposure within the epoch:** every key from the persisted CKs onward, both directions, derives from the snapshot — until the next epoch bump. Because PCS here is opportunistic (§5.2), the blast radius is "until the parties next meet": strictly wider than a continuously-ratcheting Double Ratchet (heals in one round trip), strictly narrower than a static key. This is **not a new concession** — the identical exposure already exists for the in-memory state against a live-device adversary, accepted in §5.2/§11.2. Persistence adds exactly one adversary class: *read access to at-rest app storage without keystore access*. Hence the keystore wrap is a MUST for this state class (elevating §10's general at-rest SHOULD). Hierarchy check: an attacker who defeats the keystore wrap obtains the identity Ed25519 secret (§10) — full impersonation — which strictly dominates chain-key theft; keystore-wrapped ratchet state therefore introduces no adversary who was not already catastrophic.

*Why "don't persist" is rejected — its availability cost is not what the handbook assumed.* The assumed cost was "in-flight messages to a restarted node are lost; the pair cheaply re-keys via prekeys at next contact." Tracing the reference implementation (`mmp-node` `send_app`/`open_direct`/`open_async`): (1) a sender that already holds conversation state never re-runs X3DH — after the peer restarts, **every** subsequent message, not only in-flight ones, fails to decrypt at the receiver, *silently* (no receipt returns; the sender sees delivery stall with no error signal); (2) prekey secrets are equally memory-only, so in-flight first-contact messages (§4.2) die with the restart too; (3) the only rebuild path is the restarted node initiating a fresh X3DH, which the peer adopts only if the initiator's key is canonically smaller (the concurrent-initiation tiebreak) — restarts of whichever party holds the canonically **larger** key therefore wedge the pair deterministically, both directions, until the *other* party also loses its state. On platforms that kill background apps routinely (Android), restart is a daily event; a deterministic wedge plus loss of all in-flight DTN traffic on every restart fails the protocol's core store-and-forward promise. An explicit re-key/session-reset signal (doc 11's session machine) is desirable belt-and-braces regardless — but it can only restore *future* traffic after the next reachability window; it cannot recover bundles already in flight under the discarded state. Only persistence can.

*Alternatives considered and rejected:*

- **(i) Don't persist + add a session-reset/REKEY signal.** Fixes the wedge, keeps the clean-RAM property. Rejected as primary mechanism: still forfeits every in-flight bundle on every restart, and the reset waits on the next contact — in a DTN measured in hours, the whole latency budget again. Retained as a complementary robustness mechanism (state corruption, version migration).
- **(ii) Persist current chain keys only, drop the skipped-key window.** Removes the ready-made message keys from the stolen blob, but discards exactly the keys that make DTN reordering survivable across restart — and once the blob is keystore-wrapped, window keys and CK-derivable future keys are the same exposure class. Poor trade; rejected.
- **(iii) Unencrypted persistence with a short TTL.** Bounds a *stale* snapshot's value but leaves a fresh snapshot fully valuable and contradicts §10's direction. Rejected — the TTL itself is adopted (below) because state older than the bundle lifetime has zero availability value by construction.

*Normative requirements (v1):*

- (a) Phones/PC: persist per-conversation `(send CK, epoch, next_index)`, `(recv CK, epoch, next_index)`, and the skipped-key map, encrypted under a keystore-wrapped key (Android Keystore / iOS Keychain / DPAPI). §10's at-rest SHOULD becomes a **MUST for this state class**; where no keystore-class facility exists, do not persist.
- (b) CK₀ MUST NOT be persisted, and SHOULD be erased from memory once both direction chains are derived (implementation note: the current `Conv` retains it).
- (c) The persisted blob carries a written-at timestamp; on load, state older than `RATCHET_PERSIST_TTL_MS = LIFETIME_MESSAGE_MS` (72 h ⚠ — new constant, local policy, not protocol-visible) MUST be discarded, falling back to fresh X3DH establishment.
- (d) The first live session after a restore-from-disk MUST perform the §5.2 epoch bump — restore is not an exemption from §5.2, and the bump is what bounds post-snapshot exposure at the next meeting. Implementation precondition: `bump_epoch` exists in `mmp-crypto` but is not yet wired into the node's session path; wiring it is a prerequisite for shipping persistence, not optional hardening.
- (e) One-time-prekey and signed-prekey secrets (§4.2) are the same state class — their loss kills in-flight first-contact messages — and follow the same rule: keystore-wrapped persistence on phones/PC, never in the clear.
- (f) Core/`no_std`: no ratchet persistence (RAM state only); flash-resident secrets remain limited to identity/exchange keys under §10's readback-protection regime.

No wire-format change: persistence is entirely node-local; peers cannot observe (and need not know) whether a counterpart restored or re-keyed.

## 6. Group messaging (sender keys + epochs)

6.1. Per (group, epoch, member): sender chain `SK = (chain key, public signature key)`, generated by the member and distributed to every other member **via the pairwise conversation channels** (§5) — the standard sender-keys construction [F5], whose formal analysis and known weaknesses (no PCS within an epoch; member-compromise reads the epoch) are documented in [F5].

6.2. Message encryption: K = derive(chain key, index); chain advances per message (forward secrecy within epoch); message carries (epoch, member, index) + Ed25519 signature under the member's *group signature key* (distinct from identity key — limits cross-context linkage of signatures).

6.3. **Epoch rotation:** any effective membership change (06 §8.3) → epoch++; all members generate fresh sender keys; a REMOVE'd member receives nothing for the new epoch (future secrecy vs removed members). Members learn of epochs via the admin's anchor claim; missing sender keys are requested in-channel or via pairwise channels at next reachability — late joiners cannot read pre-join epochs (no key history is sent backward by default ⚠ policy flag).

6.4. Out-of-order: per-sender skipped-key window as §5.1 (W=512 per sender ⚠); DTN delay beyond lifetime ⇒ message expired anyway.

6.5. **Why not MLS:** to be precise — RFC 9420 does *not* require the delivery service for security and tolerates unordered application messages [F6]; the problem is **concurrent commits**: only one commit can advance an epoch, and MLS deployments rely on the DS (or some agreement layer) to arbitrate which concurrent commit wins. A partitioned mesh generates concurrent commits routinely (two admins change membership on opposite sides of a partition, hours apart) and has no arbiter — the tree state forks and needs an out-of-protocol resolution mechanism. Sender keys degrade gracefully under exactly those conditions (each sender's chain is independent; membership converges via the OR-Set, 06 §8), at the documented cost of weaker PCS [F5]. Revisit for v2 with DCGKA-class decentralized group key agreement research (doc 15).

## 7. Receipts

Receipt MAC = BLAKE3.keyed(receipt_key, BundleID) where receipt_key derives from the conversation (or group-epoch) key (§2). Only holders of the conversation key can mint receipts — relays verify only the receipt *bundle's* origin signature (doc 05 §7.1); the MAC is checked by the origin (proof of delivery) and by WoT carriers that hold the conversation key (i.e., none — so carriers treat receipts as origin-signed assertions; purge authority comes from filter-union rules, 06 §6.3).

## 8. Blob integrity

Blob root claim: `(root = BLAKE3 Merkle root over source block hashes, size, k, n, RS params)`, Ed25519-signed by the publisher (e.g., avatar owner). Every RS symbol verifies against its block hash + Merkle path **before storage** (doc 02 R-E1; doc 07 #11). Merkle arity 4 ⚠; paths ≤ 5 levels at avatar scales (≤1 MiB ⚠ cap for class-4 auto-fetch; larger blobs are pull-on-demand only).

## 9. Suite 0x02 reservation (hybrid PQ)

Wire format reserves: 1-byte suite ID in handshake + claims; claims may carry *parallel* exchange keys per suite. Suite 0x02 = X25519+ML-KEM-768 hybrid (concatenated shared secrets into the Noise-style key schedule; exact construction pinned only when standardized hybrid Noise patterns mature — tracked OQ-11, with the note that current hybrid-Noise drafts and PQ-Noise papers exist but are not yet stable targets). Signatures remain Ed25519 in 0x02 (PQ signatures' size — ML-DSA ≈ 2.4 KB — would dominate every bundle; harvest-now-decrypt-later threatens *confidentiality*, not authentication, so KEM-first hybridization is the rational order).

## 10. Storage at rest & implementation rules

- Phones/PC: identity + exchange secrets in OS keystore (Android Keystore / iOS Keychain / DPAPI); stores encrypted at rest under a keystore-wrapped key (SHOULD).
- Core/`no_std`: secrets in MCU flash with readback protection where the part supports it (nRF52 APPROTECT class); documented as best-effort.
- Zeroize-on-drop for all secret material; constant-time comparisons; RNG = OS/hardware CSPRNG only (doc 12 pins crates).
- **Canonical Ed25519 verification (normative, AUDIT F9):** all signature verification MUST reject overlarge/non-canonical `S` (S ≥ group order L) per RFC 8032 §5.1.7 — i.e. `ed25519-dalek`'s `verify_strict`, never the permissive `verify`. Many Ed25519 implementations omit this check (observed in OpenSSL, tinyssh, PuTTY histories), admitting signature malleability. MMP's BundleID excludes the signature, so malleability cannot fork dedup, but the strict check is mandatory to keep golden-vector determinism and to foreclose any implementation treating `(R,S)` and `(R,S+L)` as distinct. A negative golden vector (doc 10 §6) pins this.
- **One signing-domain scheme (normative, AUDIT F4):** every Ed25519 signature uses the context-tagged preimage of doc 10 §3.1; no signature is ever produced over raw object bytes.
- No custom primitives anywhere; only the compositions in §3–§7 are project-specific, and each non-standard composition (4.2, 5.2) is explicitly flagged for external review (OQ-10).

## 11. What is deliberately NOT provided (honest disclosure)

1. **Deniability:** Ed25519 bundle signatures are non-repudiable by design (relays must verify; doc 02 F-A2). Signal-style deniable authentication conflicts with stranger-relay verification. Documented tradeoff.
2. **PCS between meetings** (§5.2) and **within a group epoch** (§6.1, [F5]); a persisted-state restore (§5.3) extends the same between-meetings window across a restart — healed, as always, at the next live session.
3. **Anonymity** beyond pseudonymity + beacon unlinkability (threat tier).
4. **Metadata-free relaying** (doc 07 §5 note, OQ-9).

## 12. Persona review checkpoints

- **Security Auditor — Potential flaw:** "§4.2 serverless prekeys: an attacker can replay-consume Bob's one-time prekeys network-wide (grab the claim, 'use' them all), reducing everyone to the medium-term prekey." *Resolution:* correct — but consumption isn't enforceable without a server *by anyone, including attackers*; senders can't be blocked from using any published prekey, so the attack reduces to: FS quality for first messages degrades toward the signed-prekey level under attack — exactly the baseline guarantee. No integrity/confidentiality loss. Documented in §4.2; OQ-10 asks an external reviewer to confirm.
- **Skeptical Professor — Potential flaw:** "Channel binding (§3.2) — signing the Noise handshake hash with the identity key — is a custom composition; cite precedent or flag." *Resolution:* the sign-the-session-hash pattern is the standard channel-binding approach used by libp2p's Noise spec (identity key signs the Noise static, [A19-adjacent libp2p noise spec [F7]]); adopted with that citation; still listed under OQ-10's external-review scope.
- **Performance Engineer:** skipped-key windows (512/conversation, 512/sender/group) on Core targets blow RAM. *Resolution:* Core tier W=32 ⚠ (sensors aren't chat-heavy); window is a local policy, not protocol-visible. Accepted.
- **Distributed Systems Reviewer:** group sender-key distribution via pairwise channels needs O(members) pairwise conversations established — cold-start cost for large groups in a DTN. *Resolution:* acknowledged; epoch anchors let members prioritize establishing the missing pairwise channels opportunistically; group remains readable per-sender as chains arrive (graceful). Sized in doc 13.
- **Security Auditor — Potential flaw:** "§5.3 persists live chain keys and derived message keys to disk — a captured device or extracted backup now reads future traffic until the next meeting, and this protocol has no continuous ratchet to shorten that window." *Resolution:* the window is identical to the one already accepted for in-memory state (§5.2/§11.2) — persistence only adds the at-rest-storage-without-keystore adversary, which is why the keystore wrap is a MUST (not §10's SHOULD) and unencrypted fallback is forbidden; CK₀ exclusion caps the backward reach at the skipped-key window (≤ W keys, ≤ 72 h ⚠); the 72 h load-TTL caps stale-snapshot value at zero; and an attacker through the keystore already holds the identity key, which dominates. Accepted with those four conditions; the non-persisting alternative was shown to wedge conversations deterministically (§5.3), which is the worse failure against this threat tier.
- **Battery Specialist — Potential flaw:** "§5.3 adds disk writes per conversation event — flash wear and wakeup cost on phones." *Resolution:* the persisted blob is tiny (two 32-byte CKs + counters per conversation, plus ≤ W×32 B skipped keys) and piggybacks on the existing snapshot flush cadence (30 s interval already paid for the bundle store); no new wakeups. Accepted.
- **Inventive Scientist — Alternative considered:** ratcheting via the *mesh itself* (mixing receipt round-trips into CK as pseudo-interactive entropy). Rejected for v1: receipt forgeability by the recipient only (fine) but timing unpredictability makes both sides' CK state diverge dangerously under loss; revisit in doc 15.

## 13. Cross-references

Wire encodings of handshake frames, prekey claims, group control → doc 10. Session phase machine → doc 11. Crate choices (ed25519-dalek, x25519-dalek, chacha20poly1305, blake3, snow/noise-rust candidates) and zeroize/fuzz mandates → doc 12. OQ-10 (external crypto review of §4.2/§3.2/§5.2), OQ-11 (hybrid Noise maturity) appended to research/open-questions.md.
