# vectors/ — golden test vectors (doc 10 §6.4, doc 12 §7.1, handbook §4.7)

Fixed-seed, wall-clock-free, RNG-free vectors that a **second** MMP/1
implementation must reproduce byte-for-byte. They are the anti-ossification
tool (roadmap risk #7): with only one implementation, spec ambiguities silently
resolve to "whatever this code does", and these files pin that behavior so a
second implementation — or a hostile fuzzer — exposes any divergence.

## Regenerating

```
cargo run -p mmp-cli -- vectors
```

This overwrites every `*.json` here with byte-identical content **unless a codec
changed**. The generator is `crates/mmp-cli/src/vectors.rs`; every expected byte
is produced by the real `mmp-types`/`mmp-crypto` codecs at generation time —
never hand-authored — so a vector can never silently disagree with the code.

The consuming test (`mmp-cli`, `vectors::tests`) is the guard. It:
1. asserts regeneration is deterministic (`regeneration_is_deterministic`);
2. asserts every committed file still byte-matches current codec output
   (`committed_vectors_match_current_codecs`) — CRLF-normalized, since this repo
   is developed on Windows;
3. re-verifies the production invariants — recomputed BundleID, signature
   verification, and cross-domain rejection
   (`positive_vector_invariants_hold`, `negative_vector_invariants_hold`);
4. runs a separate, test-only byte reader over the committed beacon, HELLO,
   envelope, frame-payload, bundle, and grouped-class vectors
   (`vectors::conformance`) so those fixed-field, variable-length,
   payload-boundary, class, and lifetime-canonicalization checks do not call
   the production decoders.

The independent frame reader checks the document-10 grammar for all 17
registered tags: fixed key/signature and budget fields, nested SUMMARY and
RBSR counts, ID lists, blob/proxy/error varbytes, friendship body widths,
strict CLOSE emptiness, and the BUNDLE outer layout. HS payloads remain
opaque by design because Noise owns their inner grammar. It also does not
decrypt class-1/2 ciphertext or verify the class-2 sender-key chain. The
grouped class artifact records those fields as provisional rather than
presenting opaque bytes as interoperable plaintext.

If step 2 fails, a wire-format change shifted a golden byte: regenerate, then
**review the diff before committing**. That review is the checkpoint doc 10
§6.4 calls for.

The grouped artifact also records the complete registered signing-domain
matrix. The conformance reader checks its 81-cell schema and declared verdicts;
the production test independently exercises the nine diagonal accepts and 72
off-diagonal rejects. No private key material is emitted.

## What's here

Positive:
- `bundles.json` — one deterministic outer Bundle for every registered class
  0 through 5. Each record includes class metadata, extension/payload bytes,
  canonical and wire bytes, content address, and outer signature. Class 4's
  Merkle-path framing and class 5's `suite` byte are explicitly marked
  implementation/provisional because the frozen docs do not describe those
  implementation details identically; class-1/2 cryptographic interiors are
  opaque fixtures and carry the same warning.
- `bundle.json` — class-1 bundle: canonical (lifetime-zeroed) bytes, full wire
  bytes, `BundleID = BLAKE3(canonical)[0..16]`, and the `mmp1/bundle` signature.
- `beacon.json` — the 24-byte beacon codec (doc 10 §5).
- `claim.json` — nickname claim (field 0x01): `mmp1/claim` signature preimage,
  wire bytes, signature.
- `receipt.json` — receipt MAC over a fixed conversation root (the derived MAC
  key is a secret and is intentionally omitted).
- `envelope.json` — link envelope framing `[len u16 LE][type u8][payload]`.
- `frames.json` — deterministic payload and envelope bytes for all 17
  registered session frame types; the HS sample carries the same deterministic
  HELLO payload as `hello.json`.
- `hello.json` — standalone HELLO payload fields and canonical bytes.
- `rbsr_fingerprint.json` — RBSR range fingerprint of a known Id16 set.

- `cross_domain_matrix.json` contains deterministic objects and signatures for
  all nine registered `SigDomain` tags and the full 9x9 accept/reject matrix;
  `GroupMsg` and candidate group-control tags are intentionally excluded as
  provisional/unregistered.

Negative (each states the rejection/invariant it pins):
- `noncanonical_s_negative.json` — the same valid Ed25519 signature with its
  scalar changed from canonical `S` to little-endian `S+L` MUST be rejected by
  strict verification (RFC 8032 §5.1.7; AUDIT F9). The artifact carries both
  signatures and both scalar encodings, without private key material.
- `cross_domain_negative.json` — the `mmp1/bundle` signature MUST reject under
  `mmp1/claim`, `mmp1/manifest`, `mmp1/authcert`, `mmp1/announce` (AUDIT F4;
  rev-B tag set, doc 10 §3.1 / §6 rule 6).
- `lifetime_reset_negative.json` — resetting the ID-excluded `lifetime_ms`
  yields the **same** BundleID (AUDIT F1).
- `budget_clamp_negative.json` — a budget grant over `L_MAX` is clamped on
  receive (AUDIT F3).
- `envelope_oversize_negative.json` — two deterministic length-prefix cases:
  doc 10's normative logical payload cap is 4096 bytes, while the current
  receive parser permits 4128 bytes (`MAX_ENVELOPE_PAYLOAD`) for encrypted
  AEAD headroom. The 4097-byte case is rejected by the normative independent
  reader but accepted by the runtime allowance; 4129 bytes is rejected by
  both.
- `bundle_tlv_negative.json` — production and independent readers reject an
  unknown critical TLV, out-of-order types, and duplicate types. The malformed
  extension runs retain otherwise valid outer bundle framing so rejection is
  attributable to canonical TLV rules.

## Secrets rule

No private key, ratchet secret, or MAC key appears in any vector — only public
keys, signatures, hashes, ciphertext, and fixed public inputs. Same boundary as
the FFI surface.

## Deferred (not faked)

- **X3DH agreement vector** — needs deterministic-RNG plumbing for the ephemeral
  key; the property is covered today by `mmp-crypto`'s
  `prekey_agreement_matches_both_sides`.

The overlarge-S / non-canonical Ed25519 negative is no longer deferred: it is
pinned by `noncanonical_s_negative.json`, and the `mmp-cli` strict-verification
test exercises the public rejection path (AUDIT F9).

## Path note

Some spec/doc text refers to `specification/test-vectors/*.json`. The vectors
live **here** (`implementation/mesh-milk/vectors/`), the location this README
has always named and where the generator and consuming test resolve them. The
`specification/test-vectors/` mention is an errata candidate (single home, not
two) — see CHANGELOG.md.
