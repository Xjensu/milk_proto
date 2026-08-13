# E5-S4 Conformance Vectors Implementation Plan

> **Historical planning artifact (2026-08-14):** the implementation described
> here has since landed and is tracked as E5-S4 `DONE` in
> `docs/AGILE_BACKLOG.md`. Do not restart these steps from this document.
> Current unfinished work is limited to the measured fuzz campaign/corpus and
> other gates listed in `docs/AGILE_REMAINING_WORK.md`.

> **Legacy execution instructions:** the original task-by-task instructions are
> retained below for auditability. They are not a new implementation request;
> do not edit `specification/` or restart this plan from the historical file.

**Goal:** Complete the Rust reference implementation's E5-S4 golden-vector deliverable with exhaustive frame/class coverage, independent specification-derived decoding, and mandatory cryptographic negative cases.

**Architecture:** Keep production encoding in `mmp-types` and deterministic object construction in `mmp-cli`, while adding a test-only conformance reader that parses committed bytes without invoking production decoders. A catalog and coverage manifest make missing normative cases a test failure. Specification ambiguities remain recorded implementation deviations rather than invented normative bytes.

**Tech Stack:** Rust 1.96, `mmp-types`, `mmp-crypto`, `mmp-core`, BLAKE3, `serde_json` as an `mmp-cli` dev dependency, Cargo workspace tests.

---

## File map

- Create `implementation/mesh-milk/crates/mmp-cli/src/vectors/catalog.rs`: stable case IDs, normative requirement IDs, and coverage checks.
- Create `implementation/mesh-milk/crates/mmp-cli/src/vectors/conformance.rs`: test-only independent byte reader and wire assertions.
- Create `implementation/mesh-milk/crates/mmp-cli/src/vectors/cases.rs`: deterministic semantic fixtures and grouped JSON generation.
- Create `implementation/mesh-milk/crates/mmp-cli/src/vectors/tests.rs`: drift, independent conformance, negative-security, and coverage tests.
- Replace `implementation/mesh-milk/crates/mmp-cli/src/vectors.rs` with the small module facade and `generate()` entry point.
- Modify `implementation/mesh-milk/crates/mmp-cli/Cargo.toml`: add test-only JSON parsing.
- Replace/update files under `implementation/mesh-milk/vectors/`: grouped vector artifacts plus coverage manifest.
- Modify `implementation/mesh-milk/vectors/README.md`: schema, guarantees, regeneration, discrepancy boundary.
- Modify `docs/AGILE_BACKLOG.md`, `docs/IMPLEMENTATION_HANDBOOK.md`, and `docs/SECURITY_REVIEW_DOSSIER.md`: accurate E5-S4 completion evidence.

## Task 1: Catalog and coverage gate

**Files:**

- Create: `implementation/mesh-milk/crates/mmp-cli/src/vectors/catalog.rs`
- Modify: `implementation/mesh-milk/crates/mmp-cli/src/vectors.rs`
- Test: `implementation/mesh-milk/crates/mmp-cli/src/vectors/tests.rs`

- [ ] **Step 1: Add a failing test for normative coverage**

Create `vectors/tests.rs` with the initial test:

```rust
use super::catalog::{CaseKind, REQUIRED_BUNDLE_CLASSES, REQUIRED_FRAME_TYPES, REQUIRED_NEGATIVES};
use super::generate_cases;

#[test]
fn catalog_covers_every_normative_frame_class_and_negative() {
    let cases = generate_cases();
    for typ in REQUIRED_FRAME_TYPES {
        assert!(cases.iter().any(|c| c.kind == CaseKind::Frame(*typ)), "missing frame type {typ:#04x}");
    }
    for class in REQUIRED_BUNDLE_CLASSES {
        assert!(cases.iter().any(|c| c.kind == CaseKind::BundleClass(*class)), "missing bundle class {class}");
    }
    for id in REQUIRED_NEGATIVES {
        assert!(cases.iter().any(|c| c.id == *id), "missing negative case {id}");
    }
}
```

- [ ] **Step 2: Run the focused test and confirm the intended failure**

Run:

```powershell
cargo test -p mmp-cli vectors::tests::catalog_covers_every_normative_frame_class_and_negative -- --exact
```

Expected: compilation fails because `catalog` and `generate_cases` do not exist.

- [ ] **Step 3: Add the catalog types and fixed registries**

Create `catalog.rs`:

```rust
use mmp_types::frame::{
    FT_BLOB_HAVE, FT_BLOB_REQ, FT_BUDGET, FT_BUNDLE, FT_CLOSE, FT_CREDIT, FT_ERROR,
    FT_FRIEND_ACK, FT_FRIEND_OFFER, FT_HS, FT_ID_PROOF, FT_PROXY_SUMMARY, FT_RECON_IDS,
    FT_RECON_INIT, FT_RECON_RANGE, FT_SUMMARY, FT_WANT,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseKind {
    Beacon,
    Hello,
    Frame(u8),
    BundleClass(u8),
    SignedObject,
    Negative,
    RoutingInvariant,
}

pub const REQUIRED_FRAME_TYPES: &[u8] = &[
    FT_HS, FT_ID_PROOF, FT_CREDIT, FT_SUMMARY, FT_RECON_INIT, FT_RECON_RANGE,
    FT_RECON_IDS, FT_WANT, FT_BUNDLE, FT_BUDGET, FT_BLOB_REQ, FT_BLOB_HAVE,
    FT_FRIEND_OFFER, FT_FRIEND_ACK, FT_PROXY_SUMMARY, FT_ERROR, FT_CLOSE,
];
pub const REQUIRED_BUNDLE_CLASSES: &[u8] = &[0, 1, 2, 3, 4, 5];
pub const REQUIRED_NEGATIVES: &[&str] = &[
    "negative.ed25519.s_plus_l.v1",
    "negative.domain.matrix.v1",
    "negative.lifetime.reset.v1",
    "negative.budget.clamp.v1",
    "negative.envelope.oversize.v1",
    "negative.bundle.reserved_bits.v1",
    "negative.bundle.trailing_bytes.v1",
    "negative.tlv.unknown_critical.v1",
    "negative.tlv.out_of_order.v1",
    "negative.tlv.duplicate.v1",
];

#[derive(Debug, Clone)]
pub struct VectorCase {
    pub id: &'static str,
    pub requirement: &'static str,
    pub kind: CaseKind,
    pub wire: Vec<u8>,
    pub expected: &'static str,
}
```

Expose `mod catalog; mod cases; #[cfg(test)] mod tests;` from the facade and temporarily return the existing cases from `generate_cases()`.

- [ ] **Step 4: Run the test and verify it now fails for missing cases**

Expected: the test compiles and fails with the first missing frame type or bundle class, proving that incompleteness is executable.

- [ ] **Step 5: Code checkpoint**

Review only the module boundary and stable registries. Do not commit the neutral plan/design files. A later code commit may include `catalog.rs`, the facade, and its tests.

## Task 2: Independent byte reader

**Files:**

- Create: `implementation/mesh-milk/crates/mmp-cli/src/vectors/conformance.rs`
- Test: `implementation/mesh-milk/crates/mmp-cli/src/vectors/tests.rs`

- [ ] **Step 1: Add failing primitive-reader tests**

```rust
#[test]
fn independent_reader_is_little_endian_and_rejects_trailing_bytes() {
    let mut r = super::conformance::Reader::new(&[0x34, 0x12, 0xaa]);
    assert_eq!(r.u16().unwrap(), 0x1234);
    assert_eq!(r.finish(), Err(super::conformance::ConformanceError::Trailing { remaining: 1 }));
}

#[test]
fn independent_reader_rejects_short_input_at_exact_offset() {
    let mut r = super::conformance::Reader::new(&[1]);
    assert_eq!(
        r.u16(),
        Err(super::conformance::ConformanceError::Truncated { offset: 0, needed: 2, remaining: 1 })
    );
}
```

- [ ] **Step 2: Run and observe failure**

Run `cargo test -p mmp-cli vectors::tests::independent_reader -- --nocapture`.
Expected: compile failure because `conformance::Reader` does not exist.

- [ ] **Step 3: Implement bounded independent reads**

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConformanceError {
    Truncated { offset: usize, needed: usize, remaining: usize },
    Trailing { remaining: usize },
    WrongValue { offset: usize, field: &'static str, expected: u64, actual: u64 },
    Oversize { field: &'static str, actual: usize, max: usize },
}

pub struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self { Self { bytes, offset: 0 } }
    pub fn offset(&self) -> usize { self.offset }
    pub fn take(&mut self, n: usize) -> Result<&'a [u8], ConformanceError> {
        let remaining = self.bytes.len().saturating_sub(self.offset);
        if remaining < n {
            return Err(ConformanceError::Truncated { offset: self.offset, needed: n, remaining });
        }
        let start = self.offset;
        self.offset += n;
        Ok(&self.bytes[start..self.offset])
    }
    pub fn u8(&mut self) -> Result<u8, ConformanceError> { Ok(self.take(1)?[0]) }
    pub fn u16(&mut self) -> Result<u16, ConformanceError> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }
    pub fn u32(&mut self) -> Result<u32, ConformanceError> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    pub fn u40(&mut self) -> Result<u64, ConformanceError> {
        let b = self.take(5)?;
        Ok(u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], 0, 0, 0]))
    }
    pub fn varbytes(&mut self, max: usize, field: &'static str) -> Result<&'a [u8], ConformanceError> {
        let len = self.u16()? as usize;
        if len > max { return Err(ConformanceError::Oversize { field, actual: len, max }); }
        self.take(len)
    }
    pub fn finish(self) -> Result<(), ConformanceError> {
        let remaining = self.bytes.len() - self.offset;
        if remaining == 0 { Ok(()) } else { Err(ConformanceError::Trailing { remaining }) }
    }
}
```

- [ ] **Step 4: Add envelope and outer-bundle parsing without production decoders**

Add `parse_envelope` and `parse_bundle` returning plain conformance structs. Enforce document-10 offsets exactly: header 60 bytes, five-byte lifetime at 50, extension length at 56, payload length after extensions, and a 64-byte signature tail. Do not import `mmp_types::frame::read_envelope`, `Frame::decode_payload`, `Bundle::decode_canonical`, or `mmp_types::wire::Reader`.

- [ ] **Step 5: Verify focused tests pass**

Run `cargo test -p mmp-cli vectors::tests::independent_reader -- --nocapture`.
Expected: both reader tests pass.

## Task 3: Complete session-frame vectors

**Files:**

- Create/modify: `implementation/mesh-milk/crates/mmp-cli/src/vectors/cases.rs`
- Modify: `implementation/mesh-milk/crates/mmp-cli/src/vectors/conformance.rs`
- Test: `implementation/mesh-milk/crates/mmp-cli/src/vectors/tests.rs`

- [ ] **Step 1: Add a failing exact-frame coverage test**

```rust
#[test]
fn frame_catalog_has_exactly_the_registered_type_set() {
    let mut actual: Vec<u8> = generate_cases().iter().filter_map(|c| match c.kind {
        CaseKind::Frame(typ) => Some(typ),
        _ => None,
    }).collect();
    actual.sort_unstable();
    actual.dedup();
    let mut expected = REQUIRED_FRAME_TYPES.to_vec();
    expected.sort_unstable();
    assert_eq!(actual, expected);
}
```

- [ ] **Step 2: Confirm it fails against the current single-envelope vector**

Run the exact test; expected failure lists only `0x20` or no registered frame cases.

- [ ] **Step 3: Construct one deterministic `Frame` value for each variant**

Use the field values already exercised by `mmp-types::tests::frames_roundtrip`, including non-empty SUMMARY sections and friendship/proxy bodies. Encode each using `Frame::encode()` and register stable IDs `frame.hs.v1` through `frame.close.v1`.

The fixture list must explicitly include:

```rust
Frame::FriendOffer(vec![0x10, 0x11]),
Frame::FriendAck(vec![0x20, 0x21]),
Frame::ProxySummary(vec![0x30, 0x31]),
Frame::Error { code: errcode::UNSUPPORTED, detail: b"unsupported".to_vec() },
Frame::Close,
```

Friendship payloads remain opaque at the frame layer, avoiding invention of the disputed claim preimage.

- [ ] **Step 4: Independently validate every envelope and payload grammar**

For each case, `parse_envelope` checks `len`, `typ`, total consumption, and the documented payload fields. The BUNDLE frame additionally calls independent outer-bundle parsing. HS payload stays opaque; ID_PROOF is exactly 96 bytes; CLOSE is empty.

- [ ] **Step 5: Run focused frame tests**

Run `cargo test -p mmp-cli vectors::tests::frame_ -- --nocapture`.
Expected: all frame coverage and independent parsing tests pass.

## Task 4: Bundle classes 0 through 5

**Files:**

- Modify: `implementation/mesh-milk/crates/mmp-cli/src/vectors/cases.rs`
- Modify: `implementation/mesh-milk/crates/mmp-cli/src/vectors/conformance.rs`
- Test: `implementation/mesh-milk/crates/mmp-cli/src/vectors/tests.rs`

- [ ] **Step 1: Add a failing six-class assertion**

```rust
#[test]
fn bundle_vectors_cover_all_v1_classes() {
    let mut classes: Vec<u8> = generate_cases().iter().filter_map(|c| match c.kind {
        CaseKind::BundleClass(class) => Some(class),
        _ => None,
    }).collect();
    classes.sort_unstable();
    classes.dedup();
    assert_eq!(classes, vec![0, 1, 2, 3, 4, 5]);
}
```

- [ ] **Step 2: Build deterministic representative payloads**

Use production payload encoders for generation:

- class 0: `ReceiptPayload`;
- class 1: `DirectPayload` with fixed ciphertext/body bytes;
- class 2: `GroupPayload` with a fixed member signature treated as opaque because `mmp1/groupmsg` is not normative;
- class 3: canonical nickname `Claim`;
- class 4: `SymbolPayload` with a bounded deterministic symbol and Merkle-path bytes;
- class 5: `AnnouncementBody` bytes labelled provisional where the Rust-only suite byte appears.

Every outer bundle uses one fixed test identity, fixed destination, fixed lifetime, no wall clock, and an outer `SigDomain::Bundle` signature.

- [ ] **Step 3: Verify outer bundle grammar independently**

For every class, assert version nibble 1, expected class nibble, zeroed lifetime in the canonical copy, BLAKE3 BundleID, exact payload length, and 64-byte signature tail. Validate the outer signature through `mmp_crypto::verify` only after independently reconstructing the canonical bytes.

- [ ] **Step 4: Run class coverage and conformance tests**

Run `cargo test -p mmp-cli vectors::tests::bundle_ -- --nocapture`.
Expected: classes `0..=5` pass independent outer parsing and production round trip.

## Task 5: Normative signing-domain matrix

**Files:**

- Modify: `implementation/mesh-milk/crates/mmp-cli/src/vectors/cases.rs`
- Test: `implementation/mesh-milk/crates/mmp-cli/src/vectors/tests.rs`

- [ ] **Step 1: Add a failing 9 x 9 coverage test**

Define a local normative registry, excluding `GroupMsg`:

```rust
const NORMATIVE_DOMAINS: &[(SigDomain, &str)] = &[
    (SigDomain::Bundle, "mmp1/bundle"),
    (SigDomain::Claim, "mmp1/claim"),
    (SigDomain::Friend, "mmp1/friend"),
    (SigDomain::Prekey, "mmp1/prekey"),
    (SigDomain::BlobRoot, "mmp1/blobroot"),
    (SigDomain::ChanBind, "mmp1/chanbind"),
    (SigDomain::Manifest, "mmp1/manifest"),
    (SigDomain::AuthCert, "mmp1/authcert"),
    (SigDomain::Announce, "mmp1/announce"),
];
```

The test asserts 81 verdicts and exactly 72 off-diagonal rejections.

- [ ] **Step 2: Generate the matrix from one fixed object**

For each source domain, sign `b"mmp1 domain-matrix object v1"` with the fixed identity. Record source name, target name, public key, object, signature, and `must_accept` boolean. Do not use `SigDomain::GroupMsg` in this normative artifact.

- [ ] **Step 3: Verify all verdicts through the public API**

Call `mmp_crypto::verify(target, &public, object, &signature)` and compare `is_ok()` with `source == target`.

- [ ] **Step 4: Run the focused matrix test**

Expected: 9 diagonal accepts and 72 off-diagonal rejects.

## Task 6: Non-canonical Ed25519 S vector

**Files:**

- Modify: `implementation/mesh-milk/crates/mmp-cli/src/vectors/cases.rs`
- Test: `implementation/mesh-milk/crates/mmp-cli/src/vectors/tests.rs`

- [ ] **Step 1: Add the failing public-path rejection test**

```rust
#[test]
fn noncanonical_s_plus_l_is_rejected_by_public_verify() {
    let (pk, object, valid, malformed) = super::cases::canonical_s_fixture();
    assert!(mmp_crypto::verify(SigDomain::Claim, &pk, &object, &valid).is_ok());
    assert!(mmp_crypto::verify(SigDomain::Claim, &pk, &object, &malformed).is_err());
}
```

- [ ] **Step 2: Implement little-endian `S + L` construction**

Use the Ed25519 group order bytes:

```rust
const L_LE: [u8; 32] = [
    0xed, 0xd3, 0xf5, 0x5c, 0x1a, 0x63, 0x12, 0x58,
    0xd6, 0x9c, 0xf7, 0xa2, 0xde, 0xf9, 0xde, 0x14,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10,
];

fn add_l_to_s(signature: Sig64) -> Sig64 {
    let mut out = signature.0;
    let mut carry = 0u16;
    for i in 0..32 {
        let sum = out[32 + i] as u16 + L_LE[i] as u16 + carry;
        out[32 + i] = sum as u8;
        carry = sum >> 8;
    }
    assert_eq!(carry, 0, "chosen valid S must permit an in-range 256-bit S+L encoding");
    Sig64(out)
}
```

If the chosen deterministic signature overflows 256 bits after adding `L`, change only the fixed public test message until the assertion holds; do not reduce modulo `L`.

- [ ] **Step 3: Commit the positive and malformed bytes to JSON**

Record the public key, domain, object, valid signature, malformed signature, `S`, `S + L`, and expected reject verdict.

- [ ] **Step 4: Run focused and crypto tests**

Run:

```powershell
cargo test -p mmp-cli noncanonical_s_plus_l_is_rejected_by_public_verify -- --exact
cargo test -p mmp-crypto sign_verify_and_cross_domain_rejects -- --exact
```

Expected: both pass.

## Task 7: Canonical-encoding negative suite

**Files:**

- Modify: `implementation/mesh-milk/crates/mmp-cli/src/vectors/cases.rs`
- Modify: `implementation/mesh-milk/crates/mmp-cli/src/vectors/conformance.rs`
- Test: `implementation/mesh-milk/crates/mmp-cli/src/vectors/tests.rs`

- [ ] **Step 1: Add one test table covering every required negative ID**

Each entry contains case ID, malformed bytes, independent expected error category, and a closure that invokes the relevant production decoder.

- [ ] **Step 2: Construct exact malformed cases**

- Envelope length `4097` must reject as oversized.
- Bundle reserved flag bit `0x80` must reject.
- Append one trailing byte to a canonical bundle.
- Add an unknown critical TLV type in `0x00..=0x7f`.
- Encode two TLVs in descending type order.
- Encode the same TLV type twice.
- Preserve lifetime-reset and budget-clamp invariants as non-parser negatives.

- [ ] **Step 3: Require both independent and production rejection**

The independent reader returns a stable `ConformanceError` category. Production APIs need only return `Err`; tests must not match unstable display strings.

- [ ] **Step 4: Run negative tests**

Run `cargo test -p mmp-cli vectors::tests::negative_ -- --nocapture`.
Expected: every catalogued negative passes and no case panics.

## Task 8: Stable JSON artifacts and coverage manifest

**Files:**

- Modify: `implementation/mesh-milk/crates/mmp-cli/Cargo.toml`
- Modify: `implementation/mesh-milk/crates/mmp-cli/src/vectors/cases.rs`
- Modify: `implementation/mesh-milk/crates/mmp-cli/src/vectors/tests.rs`
- Replace/update: `implementation/mesh-milk/vectors/*.json`

- [ ] **Step 1: Add test-only JSON support**

Add:

```toml
[dev-dependencies]
serde_json = "1"
```

Keep generator output manual/deterministic or use a sorted map; never rely on hash-map iteration order.

- [ ] **Step 2: Add JSON schema-version and duplicate-ID tests**

Every grouped file has `schema_version: 1` and `cases`. Load all committed JSON, reject unknown schema versions, and assert every case ID is globally unique.

- [ ] **Step 3: Generate `coverage.json`**

For each normative requirement, record document/section, case IDs, evidence kind, and status. The three known discrepancies use status `provisional-extension` rather than `normative`.

- [ ] **Step 4: Make output replacement safe**

Write each artifact to a sibling `.tmp` file, flush it, then rename it over the known target. On error, remove only that known temporary file. Never delete unknown files in the vector directory.

- [ ] **Step 5: Regenerate twice and compare**

Run `cargo run -p mmp-cli -- vectors` twice. Expected: the second run produces no diff.

## Task 9: Documentation and full verification

**Files:**

- Modify: `implementation/mesh-milk/vectors/README.md`
- Modify: `docs/AGILE_BACKLOG.md`
- Modify: `docs/IMPLEMENTATION_HANDBOOK.md`
- Modify: `docs/SECURITY_REVIEW_DOSSIER.md`

- [ ] **Step 1: Update vector documentation**

Document grouped files, stable case IDs, independent-reader limits, regeneration, secrets policy, and the three discrepancy IDs. State explicitly that the suite is not a security certification or proof of cross-language interoperability.

- [ ] **Step 2: Update backlog truthfully**

Mark E5-S4 complete only if every document-10 frame, classes 0 through 5, all nine normative signing domains, independent consumption, and mandatory negatives are present and green. Keep specification discrepancies listed as follow-on implementation-conformance work.

- [ ] **Step 3: Run focused verification**

```powershell
cargo test -p mmp-cli vectors -- --nocapture
cargo run -p mmp-cli -- vectors
git diff --check -- implementation/mesh-milk/vectors implementation/mesh-milk/crates/mmp-cli docs
```

Expected: tests pass, regeneration is stable, and diff check is clean.

- [ ] **Step 4: Run workspace verification**

```powershell
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo run -p layer-check
```

Expected: all non-ignored tests pass, Clippy reports no warnings, and layer-check succeeds.

- [ ] **Step 5: Review the final diff against immutable specifications**

Confirm `git diff --name-only` contains no path under `specification/`. Confirm the code contains no new wire tag, frame type, bundle class, or registry allocation. Confirm neutral design/plan documents remain uncommitted unless the user separately requests them.
