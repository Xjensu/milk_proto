# 08 — Cryptography Specification

**mesh_milk key management and encryption.** Status: Phase 3 draft; **revised 2026-06-11 post-audit** (`AUDIT.md`, `CHANGELOG.md`). Citations [F*] in research/bibliography.md. Constants ⚠ provisional.

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
2. **PCS between meetings** (§5.2) and **within a group epoch** (§6.1, [F5]).
3. **Anonymity** beyond pseudonymity + beacon unlinkability (threat tier).
4. **Metadata-free relaying** (doc 07 §5 note, OQ-9).

## 12. Persona review checkpoints

- **Security Auditor — Potential flaw:** "§4.2 serverless prekeys: an attacker can replay-consume Bob's one-time prekeys network-wide (grab the claim, 'use' them all), reducing everyone to the medium-term prekey." *Resolution:* correct — but consumption isn't enforceable without a server *by anyone, including attackers*; senders can't be blocked from using any published prekey, so the attack reduces to: FS quality for first messages degrades toward the signed-prekey level under attack — exactly the baseline guarantee. No integrity/confidentiality loss. Documented in §4.2; OQ-10 asks an external reviewer to confirm.
- **Skeptical Professor — Potential flaw:** "Channel binding (§3.2) — signing the Noise handshake hash with the identity key — is a custom composition; cite precedent or flag." *Resolution:* the sign-the-session-hash pattern is the standard channel-binding approach used by libp2p's Noise spec (identity key signs the Noise static, [A19-adjacent libp2p noise spec [F7]]); adopted with that citation; still listed under OQ-10's external-review scope.
- **Performance Engineer:** skipped-key windows (512/conversation, 512/sender/group) on Core targets blow RAM. *Resolution:* Core tier W=32 ⚠ (sensors aren't chat-heavy); window is a local policy, not protocol-visible. Accepted.
- **Distributed Systems Reviewer:** group sender-key distribution via pairwise channels needs O(members) pairwise conversations established — cold-start cost for large groups in a DTN. *Resolution:* acknowledged; epoch anchors let members prioritize establishing the missing pairwise channels opportunistically; group remains readable per-sender as chains arrive (graceful). Sized in doc 13.
- **Inventive Scientist — Alternative considered:** ratcheting via the *mesh itself* (mixing receipt round-trips into CK as pseudo-interactive entropy). Rejected for v1: receipt forgeability by the recipient only (fine) but timing unpredictability makes both sides' CK state diverge dangerously under loss; revisit in doc 15.

## 13. Cross-references

Wire encodings of handshake frames, prekey claims, group control → doc 10. Session phase machine → doc 11. Crate choices (ed25519-dalek, x25519-dalek, chacha20poly1305, blake3, snow/noise-rust candidates) and zeroize/fuzz mandates → doc 12. OQ-10 (external crypto review of §4.2/§3.2/§5.2), OQ-11 (hybrid Noise maturity) appended to research/open-questions.md.
