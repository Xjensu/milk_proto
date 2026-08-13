# E5-S4 Protocol Conformance Vectors Design

**Date:** 2026-07-25
**Status:** Approved implementation design
**Backlog item:** `docs/AGILE_BACKLOG.md` E5-S4
**Normative authorities:** `specification/10-message-format.md` and
`specification/12-rust-blueprint.md`

## 1. Objective

Complete E5-S4 as a specification-grade conformance deliverable for the Rust
reference implementation. The result must help independent mobile, desktop,
and IoT implementations reproduce MMP/1 wire bytes without sharing the Rust
production codecs.

The deliverable separates four kinds of evidence:

1. committed known-answer vectors derived from the immutable specification;
2. a specification-derived conformance reader independent of production
   decoders;
3. production-code round-trip and drift tests;
4. security-negative tests for malformed or context-confused inputs.

Passing these tests does not claim that the entire protocol is secure or that
two independent implementations already interoperate. It proves only the
enumerated wire and rejection rules.

## 2. Constraints

- Never modify `specification/`.
- Never invent missing normative bytes or registry values.
- Production codecs remain hand-written; serde is not introduced on the wire.
- The lower crate layering and `no_std` boundaries remain unchanged.
- Vector generation is deterministic, independent of wall-clock time,
  environment state, and operating-system RNG.
- No production secret, private identity, ratchet key, or platform-keystore
  material is committed. Fixed test seeds are allowed only as published test
  inputs and must be labelled non-production.
- Existing consumers of `mmp-types`, `mmp-crypto`, and `mmp-core` must not need
  API changes merely to consume vectors.

## 3. Scope

### 3.1 Included

- Beacon encoding.
- HELLO encoding and every frame type registered by document 10:
  HS, ID_PROOF, CREDIT, SUMMARY, RECON_INIT, RECON_RANGE, RECON_IDS, WANT,
  BUNDLE, BUDGET, BLOB_REQ, BLOB_HAVE, FRIEND_OFFER, FRIEND_ACK,
  PROXY_SUMMARY, ERROR, and CLOSE.
- One outer bundle vector for each class 0 through 5.
- Canonical claims, prekey batches, file manifests, AuthCerts, announcement
  bodies, receipts, RBSR fingerprints, and link envelopes where their byte
  layout is unambiguous in the specification.
- The nine signing domains explicitly registered by document 10: bundle,
  claim, friendship, prekey, blob root, channel binding, manifest, AuthCert,
  and announcement.
- Full cross-domain rejection for signatures minted under each registered
  domain and presented under every other registered domain.
- Required negative cases: non-canonical Ed25519 `S`, lifetime reset, budget
  clamp, oversized envelope, reserved bits, malformed/trailing lengths,
  unknown critical TLV, and out-of-order/duplicate TLV.
- A machine-readable coverage manifest mapping normative requirements to
  vector files and consuming tests.
- Human-readable discrepancy records for specification/implementation
  conflicts discovered while building the suite.

### 3.2 Excluded

- Claiming real cross-language interoperability before a second decoder is
  implemented outside this Rust workspace.
- Mobile GATT modules, platform keystores, BLE hardware validation, and IoT
  fixed-capacity conversion.
- Mutation-testing infrastructure or installation of the optional
  `vector-forge` skill.
- New signing domains, frame types, bundle classes, or wire allocations.
- Resolving specification omissions by editing the specification.

## 4. Existing State

The current `mmp-cli::vectors` module produces nine JSON files and has four
green tests: deterministic regeneration, committed-file drift, positive
invariants, and negative invariants. This is a useful foundation, but it does
not complete E5-S4 because:

- only one generic envelope is represented instead of every frame;
- only a class-1 bundle is represented instead of all bundle classes;
- the consuming checks call the same production codecs that generated the
  bytes;
- the cross-domain negative starts with only a bundle signature and tests only
  four alternate domains;
- the mandatory non-canonical-S vector is explicitly deferred;
- no coverage manifest demonstrates `MUST`-level completeness.

## 5. Architecture

### 5.1 Vector model and generator

Keep `crates/mmp-cli/src/vectors.rs` as the CLI entry point, but split focused
modules when the implementation begins to grow:

- `vectors/generate.rs`: deterministic construction through production
  encoders and cryptographic APIs;
- `vectors/conformance.rs`: specification-derived byte reader and structural
  assertions; it must not call production decode functions;
- `vectors/catalog.rs`: stable vector names, requirement identifiers, and the
  coverage manifest;
- `vectors/tests.rs`: drift, conformance, security-negative, and coverage
  tests.

The exact split may remain fewer files if the modules stay small, but the
dependency rule is fixed: conformance code may use basic byte slices, fixed
integers, hex/JSON parsing, BLAKE3, and the public signature verifier; it must
not call `Frame::decode_payload`, `Bundle::decode_canonical`, TLV readers, or
payload `decode` methods.

### 5.2 Artifact layout

Continue using `implementation/mesh-milk/vectors/`, the location required by
document 12's workspace layout. Use stable grouped artifacts to avoid dozens
of nearly empty files:

- `beacon.json`
- `frames.json`
- `bundles.json`
- `claims.json`
- `signed_objects.json`
- `cross_domain_negative.json`
- `canonical_s_negative.json`
- `canonical_encoding_negative.json`
- `routing_invariants.json`
- `coverage.json`

Existing filenames that external consumers may already use remain available
unless their content is incorporated without loss and a compatibility note is
added. Regeneration deletes no unrecognized file.

Every case has a stable ID such as `frame.close.v1`, `bundle.class4.v1`, or
`negative.ed25519.s_plus_l.v1`. Each case records its normative source,
inputs, expected wire bytes or verdict, and a short security rationale.

### 5.3 Independent conformance reader

The reader implements only the grammar necessary to validate committed
vectors. It uses explicit little-endian reads and bounded slices. It checks:

- exact consumption with no trailing bytes;
- frame length and type;
- fixed and variable field boundaries;
- bundle offsets, lifetime-zeroed canonical bytes, and BundleID;
- class-specific representative payload structure where the specification is
  unambiguous;
- TLV order, uniqueness, criticality, and reserved bits;
- signed-object preimages and registered domain tags.

It is intentionally not reusable by production code. Reuse would destroy the
independence this component exists to provide.

### 5.4 Security-negative construction

For the Ed25519 scalar case, start from a valid signature `(R, S)`, add the
group order `L` to `S` in little-endian form, and retain `R`. The vector stores
both signatures and requires the valid signature to pass and the `S + L`
variant to fail through the public MMP verification function.

The signing-domain matrix uses the same fixed public test identity and the
same unambiguous object bytes. For each registered domain, mint one signature
under that domain, assert success under the original domain, and assert
failure under the other eight. The coverage test requires all 9 x 9 verdicts;
diagonal cases pass and all 72 off-diagonal cases fail.

Malformed wire cases are committed only when the expected rejection follows
directly from a normative rule. Random fuzz inputs remain in `fuzz/`, not in
the golden suite.

## 6. Data Flow

1. `mmp-cli vectors` builds deterministic semantic objects.
2. Production encoders produce candidate wire bytes.
3. The generator serializes stable JSON with requirement and case IDs.
4. Tests load committed JSON.
5. Drift tests compare regenerated JSON with committed JSON.
6. The independent reader parses committed bytes directly from the documented
   grammar and compares every field with the JSON expectations.
7. Production decoders separately consume the same committed bytes and must
   reproduce the semantic object or expected rejection.
8. Coverage tests reject missing frame types, bundle classes, signing domains,
   and mandatory negative rules.

A production encoder bug therefore disagrees with the independent reader; an
independent-reader bug is exposed by direct review against the compact
document-10 grammar; and a stale committed artifact is exposed by the drift
test.

## 7. Error Handling

- Generator failures identify the case ID and stop without partially replacing
  committed artifacts.
- Conformance errors report case ID, byte offset, expected field, and observed
  value.
- Unknown vector schema versions fail closed.
- Missing required cases fail the coverage test.
- Extra unknown case IDs fail unless registered in the catalog.
- Negative cases must assert a specific rejection category where the public
  API exposes one; otherwise they assert rejection without depending on
  unstable internal error wording.
- No test accepts panic as a valid parser rejection.

## 8. Specification Discrepancy Policy

Discrepancies are implementation blockers for the affected normative vector,
not permission to revise the protocol specification.

Each record contains:

- stable discrepancy ID;
- exact specification sections;
- exact Rust symbols/files;
- on-wire or security consequence;
- specification-exact behavior where determinable;
- safe implementation proposal;
- whether the affected vector is normative, provisional, or omitted.

The initial register contains:

### CONFORMANCE-001: Group message signing domain

Document 08 requires an inner group member signature and document 10 requires
every signature to use one of its fixed domain tags, but no group-message tag
is registered. Rust uses the additional `mmp1/groupmsg\x01` tag.

**Treatment:** retain the Rust behavior as a clearly labelled provisional
extension because removing domain separation would reduce security. Exclude it
from the normative nine-domain matrix. The class-2 outer bundle remains fully
covered because its outer signature uses the registered bundle domain.

### CONFORMANCE-002: Friendship preimage

Document 10 registers `friend || expiry_hint || peer_role`; Rust and the
implementation handbook use `owner || friend || since_be`.

**Treatment:** do not publish the Rust preimage as a normative vector. Publish
the document-10 bytes as the normative signing-domain input only when no
unstated enum meaning is required. Preserve current runtime behavior until a
separate implementation-conformance change can safely migrate stored claims
and peer compatibility.

### CONFORMANCE-003: Announcement suite byte

Documents 10 and 18 include `suite` in the signed preimage but omit it from the
AnnouncementBody wire schema. Rust serializes it as the first payload byte.

**Treatment:** record the current byte as a provisional implementation
extension. Do not call it normative. A later conformance change should infer
suite `0x01` for v1 or otherwise provide a specification-grounded source; no
wire migration is bundled into E5-S4 without a separately approved design.

## 9. Testing and Verification

Implementation is complete only when all of the following pass:

1. `cargo test -p mmp-cli vectors`
2. `cargo run -p mmp-cli -- vectors` followed by a clean vector diff
3. `cargo test --workspace`
4. `cargo clippy --workspace --all-targets --all-features -- -D warnings`
5. `cargo run -p layer-check`
6. `git diff --check`

Additional acceptance assertions:

- every document-10 frame type appears exactly once or more in `coverage.json`;
- bundle classes 0 through 5 all appear;
- every normative signing domain has one positive and eight negative
  cross-domain verdicts;
- `S + L` is rejected through `mmp_crypto::verify`;
- the independent reader contains no calls to production decode functions;
- repeated generation is byte-identical;
- vector documentation states that passing is not a security certification.

## 10. Delivery Sequence

1. Add the catalog and coverage test first; it should expose the missing cases.
2. Add independent byte-reading primitives and focused unit tests.
3. Add complete frame and bundle vectors.
4. Add signed-object and cross-domain vectors.
5. Add canonical-S and canonical-encoding negatives.
6. Regenerate and review committed artifacts.
7. Update `vectors/README.md`, backlog status, implementation handbook, and
   security-review dossier without editing `specification/`.
8. Run the full verification set and commit the implementation in logical
   changes.

## 11. Alternatives Considered

### Production-code-only vectors

Smallest change, but it fails the independent-decoder mandate and can preserve
the same bug in generator and consumer. Rejected.

### A second external-language implementation now

Best long-term interoperability evidence, but too broad for a single backlog
item and would mix protocol conformance with Kotlin, Swift, or embedded work.
Deferred until this vector contract is stable.

### Mutation-guided vector generation first

Useful after baseline coverage exists, but it optimizes code-path coverage
rather than proving the specification's enumerated wire contract. Deferred.

## 12. Follow-on Order

After E5-S4:

1. E7-S1 fixed-capacity lower crates, unlocking embedded transport and the
   Embassy shell while bounding attacker-controlled memory use.
2. E4 native Android/iOS radio shells and the keystore-backed persistence
   integration required by document 08.
3. Remaining E5-S5 authority persistence and RBSR/D3 integration.
4. A genuinely independent decoder in a non-Rust mobile or embedded harness.
