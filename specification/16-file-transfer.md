# 16 — File Transfer Application Layer

**mesh_milk photo and document sharing over the existing blob plane.** Status: extension-phase (rev. B) **FINAL**, 2026-07-03 (drafted 2026-07-02; council Round-2 fixes CR2-1/CR2-5 and Round 2-bis fixes CR2bis-1 (inline `sharer` key), CR2bis-4, CR2bis-7/8 applied; cross-references resolved in the rev. B consistency pass). Citations [K*] in research/bibliography.md. Constants ⚠ provisional.

> Adds a signed application-layer manifest (`FileManifest`, `mmp1/manifest` signing domain) carried inside the existing class 1/2 message body, plus a two-tier blob binding (full file + optional thumbnail) that reuses BLOB_REQ/BLOB_HAVE and the doc 11 §6 per-root FSM unchanged. **Zero new wire allocations:** no TLV, no bundle class, no frame type — every mechanism here is either an application-layer byte inside already-opaque AEAD payload, or a reuse of doc 08 §8's existing blob-integrity plane. Scope: photos/documents ≤ 8 MiB, ≤16 files/album; larger transfers deferred to a future WiFi-Direct TAL (doc 15 R5).

---

## 1. Scope & non-goals

1.1. In scope: sharing photos and documents ≤ **8 MiB** ⚠ per file, individually or as an album of up to **16** ⚠ files under one manifest (§6). Content is opaque bytes to the protocol; MIME type is advisory metadata only (§2), never protocol-interpreted.

1.2. Non-goals (v1): streaming media (no progressive-playback contract — a file is either fully ASSEMBLED or it isn't, doc 11 §6); resumable cross-identity shares (identity keys never rotate — compromise means identity replacement, doc 03 §3.3 — so a manifest whose sharer key changes simply stops verifying; no inherited/anonymous resume); files > 8 MiB (deferred to doc 15 R5's WiFi-Direct TAL — the 8 MiB ceiling exists precisely to stay honest about BLE's 50–150 kbps floor, §8); in-place editing or versioning (content-addressing makes any edit a new object by construction — a new manifest, new roots).

1.3. **Landscape.** Eager-metadata/lazy-payload splitting is not new — Haggle's attribute-tagged data objects [K1]–[K3] and PodNet's channel pub/sub with opportunistic episode download [K4] pioneered it — but both left metadata and payload **unsigned**. Among the 2024–26 mesh/local-transfer field: bitchat ships no file/image/attachment transfer at all as of v1.5.3 [K14]; Reticulum's LXMF attaches whole files inline with no preview tier and no content-addressed manifest [K15]; Meshtastic has no native file transfer [K16]. The closest deployed-at-scale local-transfer analogs — AWDL [K8] and AirDrop's contact-hash-gated file offers [K9] — predate signed, content-addressed manifests entirely, and AirDrop's own offer-parsing surface is exactly the vulnerability class §9.3 guards against [K10]. A signed, two-tier (eager-preview/lazy-payload) manifest over a DTN blob plane is presently unoccupied ground.

## 2. File object model

2.1. `FileManifest` — one signed object per share (single file or album), carried as an application payload inside a class 1/2 message body (§3):

```
FileManifest {
  suite     : u8              // cipher-suite selector (doc 03 §8) — binds this manifest to a suite, CR-S4
  sharer    : key32           // original sharer's Ed25519 identity PUBLIC KEY, inline (CR2bis-1 —
                              // same rationale as the bundle's inline origin, AUDIT F2: the manifest
                              // must verify standalone, because §3.4 re-share detaches it from any
                              // bundle whose origin equals the sharer)
  count     : u8              // 1..16 ⚠ (ALBUM_MAX_FILES)
  entries   : FileEntry × count
  sig       : sig64           // mmp1/manifest domain (§3.3) over suite‖sharer‖count‖entries,
                              // verified (verify_strict) against the embedded sharer key
}

FileEntry {
  name        : varbytes      // UTF-8, ≤255 B
  mime        : varbytes      // UTF-8 MIME string, ≤64 B ⚠ (advisory only, §9.2)
  size        : u32           // exact plaintext byte size of the assembled file
  file_root   : id16          // BLAKE3 Merkle root prefix (doc 08 §8) — blob address
  rs_k        : u8            // RS params echo (doc 08 §8) — pre-fetch estimate only
  rs_n        : u8
  full_hash   : [u8;32]       // BLAKE3-256 over the complete plaintext (end-to-end check,
                               // independent of per-symbol Merkle-path verification)
  flags       : u8            // bit0: has_thumbnail
  thumb_root  : id16          // present iff flags bit0 — separate thumbnail blob address (§5)
}
```

2.2. Every variable-length field is length-prefixed per doc 10 §1 (`u16 len ‖ bytes`) — nothing is bare-concatenated (CR-S4, §3.3). `rs_k`/`rs_n` are an **echo**, not authority: the binding of `file_root` to actual bytes is the per-symbol Merkle-path verification of doc 08 §8; a manifest with a wrong RS echo only misleads a pre-fetch cost estimate, never the integrity check.

## 3. Wire mapping

3.1. **Body-interior placement.** The class 1 (`conv id16, ratchet_epoch u16, msg_index u32, body varbytes`, doc 10 §4.1) or class 2 (`gid id16, epoch u16, member, chain_index u32, body varbytes, member_sig sig64`, doc 10 §4.2) message body, once AEAD-decrypted, begins with a one-byte application type tag:

```
body = apptype(u8) ‖ app_payload
apptype 0x00 = plain text (implicit — no tag required for legacy bodies)
apptype 0x01 = FileManifest (this doc)
```

This is the MIME-in-a-message-body pattern exactly: `apptype` and everything after it lives inside AEAD ciphertext that only conversation/group-key holders can open (doc 08 §5/§6) — **invisible to every relay**, so no relay's forward decision ever depends on it, and old clients render an unrecognized body no worse than any other opaque payload.

**Legacy-body disambiguation (normative, CR2-1).** A rev-A sender emits *untagged* bodies, so a legacy plaintext whose first byte happens to be `0x01` is indistinguishable a priori from a tagged manifest. The receiver rule is fail-safe by construction: on `body[0] = 0x01`, attempt a full §9.3 canonical manifest parse; **on any parse failure, render the entire body as opaque text/bytes — never drop, never error the session**. A false positive therefore costs one bounded parse attempt and degrades to exactly the legacy rendering; it can never lose a message. (`0x01` is a C0 control byte, invalid as leading UTF-8 text in practice, so collisions are rare — but the rule removes the ambiguity rather than betting on rarity.) Receivers MUST accept both tagged and untagged text indefinitely. Senders of plain text SHOULD default to **untagged** bodies and MAY emit the explicit `0x00` tag only once the conversation has evidence the peer understands tags (e.g., a previously received tagged body from that peer) — there is no capability negotiation (by design, §3.2), and a `0x00`-tagged text body delivered to a rev-A client would render with a spurious leading byte, a regression the fail-safe rule above exists to avoid in the other direction (CR2bis-4).

3.2. **Rejected alternatives.**

- **New critical TLV.** A `FILE_MANIFEST` TLV in the 0x00–0x7F critical range (doc 03 §4.4) would force every relay — including old-firmware ones — to understand it, since an unknown critical TLV **MUST NOT** be forwarded (doc 03 §4.4): a network-wide flag day for a payload no relay ever needs to see. Rejected.
- **New bundle class.** Classes 0–4 are already allocated (doc 03 §4.1); a file class would duplicate class 1/2's AEAD, PARENTS, and receipt machinery for no semantic gain — a file share *is* a message with an unusual body. Rejected as unnecessary.
- **New frame type.** A dedicated session-frame type (doc 10 §2) needs HELLO capability negotiation (doc 10 §2.1) before either peer can use it, buying nothing a body-interior byte doesn't already give for free. Rejected.

3.3. **Manifest signing (normative).** Signed preimage per the global scheme (doc 10 §3.1): `dom_tag = "mmp1/manifest"‖0x01`, `object_bytes = suite ‖ sharer ‖ count ‖ entries`. This is a domain **additive** to the doc 10 §3.1 registry — prefix-free by construction — requiring no change to any existing tag or verifier. `suite` rides **in-preimage** (CR-S4): a relay cannot claim a weaker suite produced this manifest, because tampering with `suite` invalidates the signature under `verify_strict` (doc 08 §10); every variable field inside `entries` is length-delimited (§2.2), foreclosing preimage ambiguity.

**Verification (normative, CR2bis-1).** `sig` is verified against the **embedded `sharer` key**, never against the carrying bundle's `origin` — the manifest must be independently verifiable because §3.4 re-share detaches it from any bundle whose origin equals the sharer. `sharer` rides in-preimage, so it cannot be swapped to re-attribute a manifest. A manifest whose signature fails verification is handled by the §3.1 fail-safe rule (render the body as opaque text/bytes; never drop, never error the session). On a first-hop share `sharer` will normally equal the bundle `origin`; a mismatch is not an error (it is what a re-share looks like, §3.4), but UIs SHOULD attribute the *files* to `sharer` and the *forwarding* to the bundle origin, distinctly.

3.4. **Attribution across re-share.** `FileManifest.sig` is by the original **sharer's** identity key — carried inline as the `sharer` field (§2.1), independent of whichever bundle currently carries the manifest. Re-sharing a received file into a new conversation re-signs the outer bundle (ordinary class 1/2 semantics) but leaves the manifest bytes, including `sharer` and `sig`, unchanged — the manifest still *names and proves* the original sharer as the party vouching for name↔content, which §5.2's accountability guarantee depends on. (Without the inline key this guarantee would be vacuous beyond the first hop: a re-share recipient has no other way to know which key to verify `sig` against — CR2bis-1.)

## 4. Blob-plane binding

4.1. `file_root` and `thumb_root` address content-addressed blobs exactly as doc 08 §8 specifies — BLAKE3 Merkle root over RS GF(256)-encoded source blocks (k≤64, OQ-7 governs decode-cost validation), arity-4 Merkle paths — **unchanged**. Symbols travel as class-4 bundles carrying SYMBOL_INFO (doc 03 §4.4, doc 10 §2.6/§4.5); requesters use BLOB_REQ/BLOB_HAVE (doc 10 §2.6) and the doc 11 §6 per-root FSM verbatim: WANTED → FETCHING → BLOCK_DONE* → ASSEMBLED, partial caches remain servable. Multi-source fountain/RS swarming over intermittent DTN/vehicular contacts is established prior art [K5]–[K7]; this doc reuses MMP's own existing plane rather than introducing a file-specific coding scheme.

4.2. **AEAD context commitment (normative, CR-S2; scope corrected per CR2-5).** Blob symbols themselves are *signed plaintext* verified via Merkle paths (doc 08 §8) — no AEAD is involved there. The commitment requirement binds where AEAD actually carries the manifest: the class 1/2 message body (doc 08 §5/§6). Wherever that path uses ChaCha20-Poly1305 (doc 08 §1), the construction MUST commit to context — a BLAKE3 key/context-derived commitment — such that a valid tag implies a unique intended (key, file) pair. Plain ChaCha20-Poly1305 is not key-committing; an uncommitted tag would let a multi-key-holding sender craft ciphertext that decrypts differently for different recipients of the same nominal manifest (partitioning/"invisible salamander"). This is a suite-level fix (doc 08 errata-candidate, CR-S2) inherited here unmodified — file transfer carries no exemption.

4.3. **Recommended (non-normative) sizing for 8 MiB.** Symbol payload ≈ `4096 − 124 (bundle fixed overhead, doc 10 §3) − 25 (SYMBOL_INFO TLV: 3 B header + 22 B value)` ≈ **3947 B** ⚠. A block of `k=64` ⚠ data symbols carries ≈ 64 × 3947 ≈ **246.7 KiB** of source data; an 8 MiB file needs ⌈8,388,608 / 252,608⌉ ≈ **34 blocks** ⚠. Total symbols sent depends on the RS redundancy rate (n/k), which doc 08 §8 deliberately leaves unpinned — this is capacity-planning guidance for §8, not a wire commitment.

## 5. Thumbnail strategy

5.1. A **separate** blob root, `thumb_root`, capped at **32 KiB** ⚠ (OQ-18 governs the exact codec/size floor — resolved during implementation benchmarking, not pinned here). Because 32 KiB sits under doc 08 §8's existing ≤1 MiB auto-fetch ceiling, thumbnails **spread eagerly** with no user action required — while `file_root` stays **pull-only**, fetched only on explicit user action (§7, which makes the eager/lazy split role-gated, not size-gated). Scope caveat (CR2bis-8): "eagerly" means among **conversation/group key holders** — `thumb_root` is learned only by decrypting the manifest, unlike avatar roots, which are public claims; relays store and serve the class-4 symbols without knowing what they belong to, but the *fetching* population is the recipients, not the mesh at large. This eager/lazy split (**N5**) is the whole design: previews propagate at gossip speed, payloads move only when someone actually wants them (§1.3's landscape gap).

5.2. **Thumbnail-mismatch attack (formalized).** Nothing here cryptographically proves the thumbnail *depicts* the file. `file_root` and `thumb_root` are independent content addresses; the only thing binding them is `FileManifest.sig` — a statement by the sharer that "these two roots belong together, under this name." An identity that is malicious or compromised can publish a manifest whose `thumb_root` is innocuous while `file_root` is arbitrary — including harmful — content. The mesh verifies each blob's bytes against its own signed root (doc 08 §8); it has no mechanism to verify semantic correspondence between two independently-coded blobs.

**Security Auditor — Potential flaw:** "A user who trusts a thumbnail preview is trusting a claim, not a proof — that gap is exactly where 'the crypto looks strong but the actual guarantee is weaker than users assume' failures live." *Resolution:* accepted as accurate and not patchable without solving general content-understanding, which is out of scope for a transport protocol. The correct framing is **accountability, not prevention**: `FileManifest.sig` verifies against the manifest's own inline `sharer` key (§2.1/§3.3 — inline precisely so the attribution survives re-share, CR2bis-1), an identity the recipient can hold responsible (petname/WoT context, doc 07); a mismatch is discoverable after the fact and attributable to a specific, non-repudiable key (doc 08 §11 point 1) — the same trust model as every other signed claim in this suite. This mirrors documented preview-mismatch classes: sharing-card/link-preview forgery [K17] and fake-thumbnail-icon malware distribution [K18], neither fully foreclosed by any transport-layer signature scheme. UIs SHOULD surface sender identity prominently next to any thumbnail (doc 07 §6 UX duties) and add friction to first-open-from-stranger. Documented honestly as a residual, consistent with doc 08 §11's disclosure discipline.

## 6. Multi-file (albums)

6.1. `count` ≤ **16** ⚠ (`ALBUM_MAX_FILES`, §10) files share one `FileManifest`, one signature, one apptype-tagged body. Each `FileEntry` is independently addressed and independently fetched — "album" is a bookkeeping grouping at the application layer only; nothing below §2 knows it exists. Partial delivery (some entries fetched, peer vanishing mid-album) is not a new failure mode: each entry's blob FSM (doc 11 §6) runs and can fail independently, exactly as a lone avatar_root would (§11 Distributed Systems Reviewer checkpoint).

6.2. The 16-file cap bounds worst-case eager-thumbnail cost (16 × 32 KiB ≈ 512 KiB per manifest, §11 Battery Specialist) and worst-case manifest parse cost (§9.3) to a fixed, small ceiling — chosen for headroom against typical photo-album/document-bundle sharing, not derived from a formal model; revisit if usage data disagrees.

## 7. Fetch policy & quota interaction

7.1. **Thumbnails** auto-fetch on manifest receipt (§5.1), no different from avatar blobs. **Full files** are pull-only: a BLOB_REQ (doc 10 §2.6) for `file_root` is emitted only on explicit user action, never automatically. The split is enforced at the requester's fetch-policy layer, and it is **role-gated, never size-gated** (CR2bis-8): a `file_root` MUST NOT be auto-fetched even when the file is small enough to sit under doc 08 §8's generic ≤1 MiB auto-fetch ceiling — the roles in the manifest (`thumb_root` vs `file_root`), not byte sizes, decide eagerness. The ≤32 KiB thumbnail cap merely guarantees thumbnails *also* qualify under the generic ceiling; it is not the mechanism.

7.2. **Quota.** Fetched blobs (thumbnail or full) count against the ordinary phone store quota (`QUOTA_STANDARD = 128 MiB`, doc 03 §9) like any other stored object — no file-transfer-specific quota exists. A fetched-and-viewed full file SHOULD be user-pinnable (survives eviction pressure) vs. opportunistically cached (evicted under doc 04 §2 order like any low-priority object) — UI/policy distinction, not a protocol one.

7.3. **TTL and re-fetch.** Symbol bundles carry the class-4 default lifetime (**72 h** ⚠, doc 03 §4.3), same as any blob. A file whose symbols fully expire network-wide before a requester finishes is simply gone; re-fetch is the existing BLOB_REQ retry path against any surviving ABANDONED-state partial cache (doc 11 §6) — no new mechanism.

## 8. Airtime & fairness

8.1. BLOB-phase (doc 03 §5.2 phase 5) traffic, including file-share fetches, is recommended (⚠, an admission-policy constant — not a wire change) to stay within a **≤40% sub-budget** of the 60 s session airtime cap (doc 11 §7) when other domains are queued, mirroring the existing D3 sub-budget discipline (doc 11 §8) rather than inventing a new one. WANT-driven BLOB pre-emption after D0/D1 complete (doc 11 §8) is unchanged; a session with nothing else queued MAY use its full cap on file symbols.

8.2. **Honest transfer-time arithmetic.** An 8 MiB file is 67,108,864 bits. At the doc 02 Track B planning range of 50–150 kbps goodput (C-B1), pure transfer time is **≈ 7.5–22.4 minutes**; at a 24 s (40%-of-60 s) contact-slice, that is **≈ 19–56 contacts** (worst case — more if RS redundancy overhead or per-contact setup cost (CR-A5, §11 Performance Engineer) eats into the slice). On modern 2M-PHY + DLE hardware, [I14]'s measured 250–680 kbps range (phone↔MCU, not yet phone↔phone — same caveat as OQ-2) cuts this to **≈ 1.6–4.5 minutes**, **≈ 5–11 contacts** at the 24 s slice, or as few as **2 contacts** at a full 60 s slice. The range is wide honestly: it is bounded by OQ-2 exactly as every other throughput claim in this suite.

## 9. Abuse analysis

9.1. **Garbage-manifest quota exhaustion.** A flood of well-formed-but-useless manifests (fake names, dangling roots) costs each **relay** exactly what any class 1/2 bundle costs — one bundle-signature verify; the manifest itself is AEAD-interior and invisible to relays (§3.1), so the additional Ed25519 manifest verify and bounded parse (§9.3) fall only on **decrypting recipients** (CR2bis-7). Both costs are bounded by the existing per-origin token bucket (doc 05 §8: 32 KiB/h default, ×4 WoT multiplier). **Caveat (CR-A1):** origin buckets meter a *given* identity's rate; they do not stop an attacker spreading the flood across unlimited fresh identities, which still forces the verify-before-store cost per identity (doc 03 §4.1.2) — the general CR-A1 residual, unmodified here. File manifests carry **no elevated priority** and inherit exactly the same residual as any other class 1/2 bundle. Where genuinely abuse-resistant admission matters (operator/authority broadcast traffic), doc 18's cost-/authority-gated admission is the design that confronts CR-A1 directly; ordinary peer-to-peer file sharing does not warrant that machinery.

9.2. **MIME confusion / name spoofing.** `mime` and `name` are advisory, never protocol-interpreted (§2.1) — a lying `mime` can mislead a rendering UI (don't auto-execute on it) but cannot make the parser or blob plane do anything unsafe. `name` collisions are permitted by design (same non-registration philosophy as nicknames, doc 03 §3.4); display SHOULD show sender identity alongside `name`, same UI mitigation as §5.2.

9.3. **Parser hardening (normative).** Manifest decoding happens immediately after AEAD decryption but **before any human or reputational trust decision** — the sender may be an entirely unvetted stranger via sealed-box first contact (doc 08 §4.1), so the bytes are fully attacker-controlled in the same sense a pre-authentication parser's input is. [K10] found six pre-authentication vulnerabilities in AirDrop/Quick Share's file-offer parsing, including a plist-recursion bug — exactly the class a naive manifest decoder invites. The doc 10 §6 canonical-reject discipline applies in full: bounded recursion (moot here — `FileManifest`/`FileEntry` are flat, non-recursive by design), fixed field caps (`count ≤ 16`, `name ≤ 255 B`, `mime ≤ 64 B` ⚠, all checked before allocation), and a zero-heap-allocation-proportional-to-input pre-verify path (reject on any length field exceeding its cap before touching the bytes it claims to bound).

## 10. Constants (⚠ provisional)

| Constant | Value | Note |
|---|---|---|
| FILE_MAX_SIZE | 8 MiB | §1 scope cap; larger → doc 15 R5 |
| ALBUM_MAX_FILES | 16 | §6, bounds parse + eager-thumbnail cost |
| NAME_MAX | 255 B | §2, UTF-8, length-prefixed |
| MIME_MAX | 64 B ⚠ | §2, advisory only |
| THUMB_MAX | 32 KiB ⚠ | §5, OQ-18 governs exact floor |
| BLOB_AUTO_FETCH (reused) | ≤1 MiB | doc 08 §8, unchanged — governs thumbnail eager-fetch |
| SYMBOL_PAYLOAD (derived) | ≈3947 B ⚠ | §4.3, non-normative sizing guidance |
| BLOCK_SIZE (derived, k=64) | ≈246.7 KiB ⚠ | §4.3, non-normative |
| BLOB_SUBBUDGET | ≤40% of session cap ⚠ | §8, admission policy, mirrors doc 11 §8 D3 rule |
| CLASS4_TTL (reused) | 72 h | doc 03 §4.3, unchanged |
| QUOTA_STANDARD (reused) | 128 MiB | doc 03 §9, unchanged |

## 11. Persona review checkpoints

- **Skeptical Professor — Potential flaw:** "Calling this 'zero new wire allocations' is rhetorical — a body-interior `apptype` byte is functionally a TLV, just hidden from relays. You've moved the extension mechanism, not eliminated it." *Resolution:* the distinction is load-bearing. A TLV lives in the signed-but-unencrypted extension space every relay parses (doc 10 §3) — a new critical TLV forces every relay to understand it or refuse to forward (doc 03 §4.4), a network-wide flag day. `apptype` lives inside AEAD ciphertext (doc 10 §4.1/§4.2) only conversation/group-key holders can see — no relay decision ever depends on it, so old relays and new senders stay fully interoperable. That relay-visible-vs-recipient-only asymmetry is precisely what "zero wire allocation" means. Accepted.
- **Security Auditor:** thumbnail-mismatch formalized in §5.2; a second concern — unsolicited manifests from unvetted strangers via sealed-box first contact (doc 08 §4.1) — is bounded by per-origin buckets (doc 05 §8) and canonical-reject parsing (§9.3), with the residual (one well-formed manifest still renders) treated as a UI-consent decision. The CR-A1 Sybil-defeated-bucket caveat applies here exactly as elsewhere in the suite — not worse, not better. Accepted with residual documented.
- **Performance Engineer — Potential flaw:** "§8's contact-count arithmetic assumes every airtime-slice second is usable transfer goodput. CR-A5 established that short (3–10 s) drive-by contacts can spend their entire duration on handshake + SUMMARY + RBSR setup before BLOB phase is ever reached — the low end of §8's range could be a severe underestimate." *Resolution:* correct, and not solved here — inherits the CR-A5 gap. §8 assumes a contact that reaches the BLOB phase at all; for the short-contact regime, expected per-contact progress should be modeled as near-zero, and the effective contact count is dominated by what fraction of contacts clear setup, not raw duration. Deferred to doc 15 R1's BLE-parameterized simulation and CR-A5's RBSR-checkpoint-resumability fix; this doc states the honest range and stops there.
- **Distributed Systems Reviewer — Potential flaw:** "The doc 11 §6 blob FSM has no notion of 'this root is part of album Y' — what tracks whether a 16-file share is 'done,' and what happens if the peer disappears mid-album?" *Resolution:* deliberately out of the FSM's scope. Each `file_root`/`thumb_root` runs its own independent doc 11 §6 instance, unchanged; "album complete" is purely an application-layer aggregate over N independent FSM outcomes (§6.1). A vanished peer leaves some entries ABANDONED exactly like any lone blob — partial symbols remain servable — no new state, no new failure mode. Accepted.
- **Battery Specialist — Potential flaw:** "Thumbnails auto-fetch eagerly; a 16-file album is up to 512 KiB of unsolicited download the instant a manifest is seen by every device in range, whether or not anyone opens the conversation." *Resolution:* real cost, but not a budget bypass — eager thumbnail fetch competes for the same per-hour relay/fetch airtime budget (doc 04 §5) as everything else, bounded worst-case by ALBUM_MAX_FILES=16 (§10); 512 KiB is negligible against the 128 MiB phone quota (doc 03 §9) as a *storage* cost — the real cost is airtime, already rationed generically. Flagged for doc 13's energy model to add "album thumbnails" as an explicit workload class; not a new mechanism.
- **Inventive Scientist — Alternative considered:** derive an eager preview from the first RS block of the full file's own encoding (progressive-decode style) instead of a separate `thumb_root`, saving a second blob entirely. *Rejected for v1:* couples preview quality to source-codec progressive-decode support (not guaranteed for arbitrary photo/document formats in scope, §1), and a large file's first RS block can itself exceed the 1 MiB auto-fetch ceiling — silently reopening the eager/lazy boundary N5 exists to keep as a clean size threshold rather than a codec-dependent heuristic. Filed as a v2 idea for a progressive-codec-aware transport (doc 15), not adopted.

## 12. Cross-references

Wire truth (frame types, body layout, signing-domain registry) → doc 10 §2–§4. Bundle/TLV rules this doc deliberately does not touch → doc 03 §4.1.1/§4.4/§8. Blob integrity and RS/Merkle mechanism (unchanged) → doc 08 §8. Per-root FSM (reused unchanged) → doc 11 §6. Profile/claim field pattern (avatar_root precedent) → doc 06 §4.1. Routing quotas and fairness → doc 05 §8. Session airtime and phase budgets → doc 11 §7–§8. Large-file future transport → doc 15 R5. Council constraints folded in: CR-S4 (§3.3), CR-S2 (§4.2), CR-A1 (§9.1), N5 (§5.1–§5.2) — full findings in `COUNCIL-REVIEW.md`. Open questions: OQ-18 (thumbnail codec/size floor, §5.1), OQ-2/OQ-7 (throughput/RS-decode measurement feeding §8/§4.3).
