# 10 — Message Format Specification

**mesh_milk binary wire formats.** Status: Phase 4 draft; **revised 2026-06-11 post-audit** (`AUDIT.md`, `CHANGELOG.md`). Single normative source for every byte on air. Integers little-endian; no implicit padding; all structures byte-aligned. Canonical-encoding rule: there is exactly one valid encoding of any value — parsers MUST reject non-canonical forms (overlong lengths, trailing bytes, unknown critical TLVs per doc 03 §4.4).

> **Audit revision summary (this doc):** §3 `origin` becomes the 32-byte inline public key (AUDIT F2); new §3.1 defines the **global Ed25519 signing-domain-separation** scheme (AUDIT F4); §6 adds the **canonical-S** verification mandate (AUDIT F9) and the **reassembly bound** (AUDIT F10); §2.5 records the copy-budget clamp (AUDIT F3); §7 size accounting recomputed (+16 B fixed overhead).

---

## 1. Primitives

```
u8/u16/u32/u40            little-endian unsigned
id16        [u8;16]       BundleID / NodeID / GroupID / root hash prefix
key32       [u8;32]       Ed25519 or X25519 public key
sig64       [u8;64]       Ed25519 signature
varbytes    u16 len ‖ bytes
```

## 2. Session frame envelope (bulk plane, doc 09 §5.1)

```
Frame { len: u16  // payload length, ≤ 4096
        typ: u8
        payload }
```

Frame types:

| typ | Name | Direction | Phase |
|---|---|---|---|
| 0x01 | HS1/HS2/HS3 | both | Noise XX messages (opaque to framing) |
| 0x02 | ID_PROOF | both | doc 08 §3.2 channel binding |
| 0x03 | CREDIT | both | flow control (doc 09 §5.2) |
| 0x10 | SUMMARY | both | doc 03 §5.2 phase 2 |
| 0x11 | RECON_INIT | initiator | RBSR open (per domain) |
| 0x12 | RECON_RANGE | both | RBSR fingerprints/splits |
| 0x13 | RECON_IDS | both | terminal ID lists |
| 0x14 | WANT | both | request bundle IDs / DAG parents |
| 0x20 | BUNDLE | both | one full bundle (doc 03 §4.1) |
| 0x21 | BUDGET | both | spray copy-budget grant |
| 0x30 | BLOB_REQ | both | request symbol ranges for a root |
| 0x31 | BLOB_HAVE | both | symbol availability bitmap |
| 0x40 | FRIEND_OFFER / 0x41 FRIEND_ACK / 0x42 PROXY_SUMMARY | — | doc 04 §4, doc 06 §7 |
| 0x7E | ERROR | both | code u8 + varbytes detail |
| 0x7F | CLOSE | both | clean teardown |

All frames after 0x01/0x02 are inside the AEAD channel; HS frames and ID_PROOF are the only pre-channel parsers (fuzz mandate, doc 07 §8). Unknown frame types: ERROR(0x01 unsupported) and CLOSE — sessions are versioned in HELLO, silent skipping invites desync.

### 2.1 HELLO (inside HS payload via Noise payloads)

`ver(u8) suites(varbytes) tier(u8) flags(u16: bit0 coc_avail, bit1 proxy_carrier, …) credits(u8) psm(u16, 0=none)`

### 2.2 ID_PROOF

`identity_pubkey(key32) sig64` where the signature uses the §3.1 `mmp1/chanbind` scheme over `noise_handshake_hash ‖ initiator_pubkey ‖ responder_pubkey` (AUDIT F4: the preimage commits to **both** identity public keys, foreclosing identity-misbinding/UKS, not just the handshake hash). Verify against claims/TOFU before any further frame (doc 08 §3.2). The Noise `h` already binds both ephemerals, so the proof is session-unique and non-replayable into another session.

### 2.3 SUMMARY

```
receipt_buckets: u8 count { bucket_epoch u16, bloom varbytes }   // newest-first
claim_digest:   u8 count { node_prefix u32, field u8, seq u32 }  // scope per doc 06 §9
group_hints:    u8 count { salted_gid id16 }                     // salt = session key derived
want_heads:     u8 count { conv id16, heads u8 count id16 }
```

### 2.4 RBSR frames

```
RECON_INIT : domain u8, window u8 (D3 24h-window flag), root_fp Range
RECON_RANGE: domain u8, count u8 × Range
RECON_IDS  : domain u8, range_bounds (lo id16, hi id16), count u16 × id16
Range      : lo id16, hi id16, count u32, fp u64        // doc 06 §3.1–3.2
```

### 2.5 BUNDLE + BUDGET

BUNDLE payload = exact doc 03 §4.1 bundle bytes (including the 32-byte inline `origin` pubkey and signature). BUDGET: `bundle id16, granted u8` — sent by the giver immediately after the BUNDLE frame it applies to (doc 05 §2.3); BUDGET without preceding BUNDLE for an unheld ID is an ERROR. The receiver MUST **clamp `granted` to `L_MAX = 32`** (doc 03 §4.5, doc 05 §2.3, AUDIT F3) before adopting it.

### 2.6 BLOB frames

`BLOB_REQ: root id16, block u16, sym_bitmap varbytes` (wanted symbols); `BLOB_HAVE: root id16, block u16, sym_bitmap varbytes`. Symbols themselves travel as class-4 BUNDLEs (SYMBOL_INFO TLV, doc 03 §4.4) — one verification path for everything (doc 08 §8).

## 3. Bundle layout (normative bit-level, expands doc 03 §4.1)

```
offset size field
0      1    ver_class   (hi nibble ver=1, lo nibble class)
1      1    flags
2      32   origin      (Ed25519 identity PUBLIC KEY, inline — AUDIT F2)
34     16   dest        NodeID/GroupID/scope
50     5    lifetime_ms (u40)
55     1    priority
56     2    ext_len
58     n    extensions (TLV: typ u8, len u16, val)
58+n   2    payload_len
60+n   m    payload
60+n+m 64   signature  (Ed25519, domain-separated §3.1, over lifetime-zeroed bytes [0 .. 60+n+m))
```

`origin` is the **full 32-byte public key**, not the 16-byte NodeID (AUDIT F2); `NodeID = BLAKE3(origin)[0..16]` is derived. Verifiers MUST check this derivation is consistent with any NodeID-keyed routing decision (doc 03 §4.1.2). Fixed overhead is therefore **124 + ext** bytes (was 108): header 60 + signature 64.

`BundleID = BLAKE3(bytes[0 .. 60+n+m) with the 5 lifetime_ms bytes zeroed)[0..16]` — **lifetime is zeroed for ID purposes** because carriers mutate it (doc 03 §4.3). The signature is over the **domain-separated** preimage of §3.1 applied to the same zeroed-lifetime canonical bytes. Receivers re-canonicalize (zero lifetime) before verifying. A relay that resets only `lifetime_ms` yields an *identical* BundleID → resurrection-resistant dedup (doc 03 §4.2, AUDIT F1). (This makes doc 03 §4.5's "outside the signed region" precise: the field is *in* the layout but normalized out of both digest scopes.)

### 3.1 Global signing-domain separation (normative, AUDIT F4)

Every Ed25519 signature produced anywhere in MMP/1 MUST be computed over a **context-tagged preimage**, never over raw bytes, so a signature minted for one purpose can never be replayed as another (cross-protocol / unknown-key-share hygiene). The signed message is:

```
signed_input = dom_tag ‖ object_bytes
```

where `dom_tag` is one of the fixed ASCII byte strings (each terminated by `0x01` to make the tag set prefix-free):

| Object | `dom_tag` | `object_bytes` |
|---|---|---|
| Bundle (§3) | `"mmp1/bundle"‖0x01` | lifetime-zeroed canonical bundle bytes `[0 .. sig_offset)` |
| Claim (§4.4) | `"mmp1/claim"‖0x01` | `owner ‖ field ‖ seq ‖ body` |
| Friendship record (§4.4) | `"mmp1/friend"‖0x01` | `friend ‖ expiry_hint ‖ peer_role` |
| Prekey batch (§4.4) | `"mmp1/prekey"‖0x01` | `signed_prekey ‖ count ‖ prekeys…` |
| Blob root (doc 08 §8) | `"mmp1/blobroot"‖0x01` | `root ‖ size ‖ k ‖ n ‖ rs_params` |
| ID_PROOF (§2.2) | `"mmp1/chanbind"‖0x01` | `noise_handshake_hash ‖ initiator_pubkey ‖ responder_pubkey` |
| FileManifest (rev. B, doc 16 §3.3) | `"mmp1/manifest"‖0x01` | `suite ‖ sharer ‖ count ‖ entries` |
| AuthCert (rev. B, doc 18 §2) | `"mmp1/authcert"‖0x01` | `suite ‖ subject ‖ scope ‖ caps ‖ priority_budget ‖ not_after ‖ issuer` |
| Announcement (rev. B, doc 18 §3) | `"mmp1/announce"‖0x01` | `suite ‖ scope ‖ ann_type ‖ not_after ‖ len(text):u16 ‖ text` |

The three rev. B rows are **registry additions only** — the tag set stays prefix-free by construction and no existing tag or verifier changes; the defining docs (16/18) own the object semantics. Implementations MUST reject a signature verified under the wrong tag. Golden vectors (§6.4) MUST include cross-tag negative cases (a bundle signature presented as a claim signature → reject), and rev. B extends the wanted set: a signature under any of `mmp1/manifest` / `mmp1/authcert` / `mmp1/announce` MUST NOT verify under any other tag (and vice versa).

## 4. Payload schemas (post-decryption where applicable)

### 4.1 Class 1 direct message (plaintext after AEAD)

`conv id16, ratchet_epoch u16, msg_index u32, body varbytes` — AEAD key per doc 08 §5; AD = bundle header bytes (binds ciphertext to routing header — header tampering breaks decryption even beyond signature).
Async first-contact form (no conversation yet): flags bit2 set; payload = sealed box (`eph key32 ‖ box`) or prekey form (`prekey_id u16, eph key32, box`) per doc 08 §4.

### 4.2 Class 2 group message

`gid id16, epoch u16, member NodeID? (sealed: member index u16), chain_index u32, body varbytes, member_sig sig64` (doc 08 §6.2).

### 4.3 Class 0 receipt

`delivered id16, mac [u8;16] = BLAKE3.keyed(receipt_key, delivered)[0..16]` (doc 08 §7).

### 4.4 Class 3 claim

`owner key32, field u8, seq u32, body varbytes, sig64` (doc 06 §4.1; full pubkey carried — claims are how peers *learn* keys). The `sig64` uses the §3.1 `mmp1/claim` scheme over `owner ‖ field ‖ seq ‖ body` (AUDIT F4). Prekey batch body (field 0x04): `signed_prekey key32, spk_sig sig64, count u8 × key32` — `spk_sig` uses the `mmp1/prekey` scheme (doc 08 §4.2). Friendship body (field 0x05): `friend key32, expiry_hint u32, both sigs` — each side's signature uses the `mmp1/friend` scheme.

### 4.5 Class 4 symbol

Payload = RS symbol bytes; SYMBOL_INFO TLV carries `(root id16, block u16, sym u16, k u8, n u8)`; verification path doc 08 §8.

### 4.6 Class 5 announcement (pointer only — rev. B, defined and owned by doc 18 §3)

Plaintext payload (never AEAD — announcements are public by design): `AnnouncementBody { ann_type u8, scope id16, not_after u32, text varbytes, sig64 }`, signed under `mmp1/announce` by the authority subject. `dest` carries `scope_id` verbatim, zero wire change (doc 18 §4). Same unknown-bundle-class rule as every other class doc 03 §8 defines: a rev-A node relays class-5 bundles blindly and correctly if the outer signature verifies, without understanding this section at all.

## 5. Beacon layout (doc 03 §5.1, doc 09 §3.1)

```
0   1   ver_role: ver(4b) tier(2b) battery(2b)   [bit7 of next byte: busy flag — see note]
1   8   ephem_id
9   1   epoch (summary schedule mod 4, doc 09 §3.3)
10  2   frag: structure(2b) offset(14b)
12  12  fragment bytes
```

(Busy flag placement: bit 7 of the `epoch` byte ⚠ — epochs only need 7 bits. Doc 09 §4 reference updated accordingly.)

## 6. Canonical-encoding and parser rules (normative)

1. Reject: trailing bytes, len fields exceeding frame, unknown critical TLVs, non-zero reserved bits, duplicate TLV types, out-of-order TLVs (ascending type required — single valid encoding).
2. Pre-auth parsers (beacon, HS1, ID_PROOF) MUST be fixed-size/streaming with zero heap allocation proportional to input.
3. **Canonical Ed25519 verification (normative, AUDIT F9):** verifiers MUST reject signatures whose scalar `S ≥ L` (the group order), per RFC 8032 §5.1.7 — non-canonical/overlarge-S forms are malleable on lax verifiers. (BundleID already excludes the signature, so malleability cannot fork dedup; this mandate keeps golden-vector determinism and forecloses any impl that treats `(R,S)` and `(R,S+L)` as distinct.) A golden vector (§6.4) MUST assert a non-canonical-S variant is rejected. The selected crate (`ed25519-dalek` `verify_strict`, doc 12) provides this; it MUST be used, not the permissive `verify`.
4. **Reassembly bound (normative, AUDIT F10):** the frame reassembly buffer is pre-allocated to `MAX_BUNDLE = 4096` and never grown by an attacker-supplied length prefix; a length prefix exceeding 4096 is an immediate `ERROR + CLOSE` (doc 11 §3). A partially-received frame that stalls beyond the session idle timer (doc 11 §7) is discarded (no cross-contact reassembly). This makes the slow-dribble buffer-pin within a session explicit (already bounded by the 60 s airtime cap, doc 09 §7).
5. Every parser MUST be fuzzed (doc 12 mandates targets per frame type).
6. Test vectors: Phase 5 deliverable (doc 12 §10) — golden bytes for each frame/bundle type with keys, to pin canonical forms across implementations; MUST include the cross-tag (§3.1) and non-canonical-S (rule 3) negative cases.

## 7. Size accounting (sanity check vs doc 02 Track B)

| Item | Bytes | Note |
|---|---|---|
| Bundle fixed overhead | **124 + ext** | header 60 (incl. 32-B inline origin pubkey, AUDIT F2) + sig 64 |
| 200-char direct message bundle | ~356 | header + PARENTS TLV + AEAD body+tag (+16 vs pre-audit) |
| Receipt bundle | ~166 | smallest common bundle (+16 vs pre-audit) |
| Claim (nickname) | ~226 | owner key already inline; +16 from header origin |
| RBSR Range msg (16 splits) | ~600 | unchanged (IDs are 16-B) |
| SUMMARY typical (2 buckets @ Core mini) | ~700 B / Standard ~20 KiB | doc 06 §9 Battery note |
At 50–150 kbps goodput (C-B1): a 5 s contact carries ≈ 30–90 KiB → handshake (≤1 KiB) + Standard SUMMARY (≤20 KiB) + dozens of message bundles. The +16 B/bundle from the inline origin pubkey is ≈4–5 % on a typical message and ≈10 % on a receipt; re-run through doc 13 §3 it raises per-node load from ≈1 MiB/day to ≈1.05 MiB/day — far inside quota. Consistent with the doc 05 §6 value-ordering design. Full model → doc 13.

## 8. Persona review checkpoints

- **Skeptical Professor — Potential flaw (caught in drafting):** "Doc 03 said lifetime sits 'outside the signed region' but gave no byte-exact mechanism; an implementer could sign the live value and break interop." *Resolution:* §3 defines zeroed-lifetime canonicalization for both BundleID and signature — one unambiguous rule; doc 03 §4.5 remains correct, now with a precise mechanism. This is the kind of gap test vectors (§6.4) exist to catch.
- **Security Auditor:** AEAD AD = header bytes (§4.1) means relays *can't* be tricked into delivering a re-headered ciphertext even if a signature check were skipped — defense in depth. Verified consistent with zeroed-lifetime rule (AD uses the same canonical form). Accepted.
- **Performance Engineer:** 124-byte fixed overhead (post-AUDIT F2; was 108) on a ~166-byte receipt is ~75% overhead. *Alternative considered:* unsigned compact receipts, or keeping origin as a 16-B NodeID. **Both rejected** — unsigned control traffic is exactly the bitchat failure class (doc 07 §4), and a 16-B NodeID makes verify-before-forward conditional on claim propagation (AUDIT F2); the +16 B inline pubkey buys unconditional verification. Cost accepted; receipts piggyback/batch where possible (doc 05 §12).
- **Distributed Systems Reviewer:** ascending-TLV-order canonicality (§6.1) makes future TLV insertion order-independent — confirmed no extension hazard.
- **Battery Specialist:** Standard SUMMARY at ~20 KiB is the largest fixed cost per contact; doc 06 §9's delta/mini-filter mitigations are REQUIRED for Core and RECOMMENDED for Standard when PeerCache has history. Re-affirmed.

## 9. Cross-references

Field semantics: bundles doc 03, RBSR doc 06, crypto payloads doc 08, transport framing doc 09. State machines consuming these frames → doc 11. Test vectors + fuzz targets → doc 12. Size/energy model → doc 13. Rev. B extensions reusing this doc's frames/schemas verbatim (zero wire change): file transfer's class-4 symbol payload and BLOB frames (§2.6, §4.5) → doc 16; deployment-profile constant presets (no new wire) → doc 17; class-5 announcement (§4.6), claim fields 0x07/0x08/0x09, and the two new signing-domain tags (§3.1) → doc 18.
