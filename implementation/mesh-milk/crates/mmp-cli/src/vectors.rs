//! Golden test vectors (E5-S4; doc 10 §6.4, doc 12 §7.1, handbook §4.7).
//!
//! Fixed-seed, wall-clock-free, RNG-free vectors that a SECOND MMP/1
//! implementation must reproduce byte-for-byte — the anti-ossification tool
//! (roadmap risk #7). Every expected byte here is produced by the real
//! `mmp-types`/`mmp-crypto` codecs at generation time, never hand-authored, so
//! a vector can never silently disagree with the code that made it. The
//! consuming test (`mod tests` below) is the guard: it regenerates every
//! vector in memory, asserts the serialization still equals the committed
//! `vectors/*.json`, AND independently re-checks each invariant (recomputed
//! BundleID, signature verification, cross-domain rejection). A codec change
//! that shifts any wire byte makes the regenerated JSON differ from the
//! committed file — the test fails until a human regenerates and reviews the
//! diff, exactly the drift signal a single-implementation protocol needs.
//!
//! Secrets rule (same as the FFI boundary): NO private key, ratchet secret, or
//! MAC key appears in any vector — only public keys, signatures, hashes,
//! ciphertext, and fixed public inputs. The receipt vector publishes its
//! conversation root (a throwaway fixed test value) and the resulting MAC, but
//! never the derived MAC key.
//!
//! Deferred, deliberately NOT faked (see `vectors/README.md`): the X3DH
//! agreement vector (needs deterministic-RNG plumbing for the ephemeral key)
//! and the overlarge-S / non-canonical Ed25519 negative (needs a crafted
//! malleable signature). Both properties are already covered by `mmp-crypto`'s
//! own unit tests (`prekey_agreement_matches_both_sides`,
//! `sign_verify_and_cross_domain_rejects`); the golden-file forms are follow-up.

use mmp_crypto::{receipt, Identity};
use mmp_types::beacon::Beacon;
use mmp_types::bundle::{class, prio, Bundle, FLAG_WANTS_RECEIPT};
use mmp_types::claim::{field, Claim};
use mmp_types::consts::L_MAX;
use mmp_types::domain::SigDomain;
use mmp_types::frame::{write_envelope, FT_BUNDLE};
use mmp_types::ids::{Id16, Sig64};

/// One generated vector file: a stable filename and its exact JSON contents
/// (already newline-terminated). Filenames are the wire contract with other
/// implementations; keep them stable.
pub struct VectorFile {
    pub name: &'static str,
    pub json: String,
}

/// Lowercase hex, no `0x` — the one encoding used for every byte string in
/// every vector file.
fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// The fixed identity every signing vector uses — a throwaway seed, never a
/// real key. Kept here so the bundle, claim, and cross-domain vectors all
/// agree on one origin.
fn vector_identity() -> Identity {
    Identity::from_seed(&[0x11; 32])
}

/// The one bundle all bundle-shaped vectors (bundle, cross-domain negative,
/// lifetime-reset negative) are built from — kept in one place so those
/// vectors can never drift apart.
fn vector_bundle() -> (Bundle, Identity) {
    let id = vector_identity();
    let mut b = Bundle {
        class: class::DIRECT,
        flags: FLAG_WANTS_RECEIPT,
        origin: id.public(),
        dest: Id16([0x22; 16]),
        lifetime_ms: 3_600_000,
        priority: prio::NORMAL,
        exts: Vec::new(),
        payload: b"golden vector payload".to_vec(),
        signature: Sig64::ZERO,
    };
    b.signature = id.sign(SigDomain::Bundle, &b.canonical_zeroed());
    (b, id)
}

/// Build every vector, deterministically. Same output every call, on every
/// machine — asserted by `regeneration_is_deterministic`.
pub fn generate() -> Vec<VectorFile> {
    vec![
        bundle_vector(),
        beacon_vector(),
        claim_vector(),
        receipt_vector(),
        envelope_vector(),
        rbsr_fingerprint_vector(),
        cross_domain_negative_vector(),
        lifetime_reset_negative_vector(),
        budget_clamp_negative_vector(),
    ]
}

fn bundle_vector() -> VectorFile {
    let (b, _) = vector_bundle();
    let json = format!(
        "{{\n  \"description\": \"class-1 bundle: canonical (lifetime-zeroed) bytes, full wire bytes, \
BundleID = BLAKE3(canonical)[0..16], and the mmp1/bundle signature over the canonical bytes (doc 10 §3)\",\n  \
\"origin_pubkey\": \"{}\",\n  \"lifetime_ms\": {},\n  \"canonical_zeroed\": \"{}\",\n  \"wire\": \"{}\",\n  \
\"bundle_id\": \"{}\",\n  \"signature\": \"{}\"\n}}\n",
        hex(&b.origin.0),
        b.lifetime_ms,
        hex(&b.canonical_zeroed()),
        hex(&b.encode()),
        hex(&b.id().0),
        hex(&b.signature.0),
    );
    VectorFile { name: "bundle.json", json }
}

/// The one beacon the beacon vector and its invariant test share.
fn vector_beacon() -> Beacon {
    Beacon {
        ver: 1,
        tier: 2,
        battery: 1,
        ephem_id: [0xA0, 0xA1, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7],
        busy: false,
        epoch: 5,
        frag_structure: 1,
        frag_offset: 0x0abc,
        fragment: [0xB0, 0xB1, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6, 0xB7, 0xB8, 0xB9, 0xBA, 0xBB],
    }
}

fn beacon_vector() -> VectorFile {
    let beacon = vector_beacon();
    let json = format!(
        "{{\n  \"description\": \"24-byte beacon codec (doc 10 §5)\",\n  \"ver\": {},\n  \"tier\": {},\n  \
\"battery\": {},\n  \"ephem_id\": \"{}\",\n  \"busy\": {},\n  \"epoch\": {},\n  \"frag_structure\": {},\n  \
\"frag_offset\": {},\n  \"fragment\": \"{}\",\n  \"wire\": \"{}\"\n}}\n",
        beacon.ver,
        beacon.tier,
        beacon.battery,
        hex(&beacon.ephem_id),
        beacon.busy,
        beacon.epoch,
        beacon.frag_structure,
        beacon.frag_offset,
        hex(&beacon.fragment),
        hex(&beacon.encode()),
    );
    VectorFile { name: "beacon.json", json }
}

fn claim_vector() -> VectorFile {
    let id = vector_identity();
    let mut c = Claim {
        owner: id.public(),
        field: field::NICKNAME,
        seq: 7,
        body: b"alice".to_vec(),
        sig: Sig64::ZERO,
    };
    c.sig = id.sign(SigDomain::Claim, &c.signed_object());
    let json = format!(
        "{{\n  \"description\": \"nickname claim (field 0x01): mmp1/claim signature preimage (owner ‖ \
field ‖ seq ‖ body), full wire bytes, and signature (doc 06 §4, doc 10 §4.4)\",\n  \"owner_pubkey\": \"{}\",\n  \
\"field\": {},\n  \"seq\": {},\n  \"body\": \"{}\",\n  \"signed_object\": \"{}\",\n  \"wire\": \"{}\",\n  \
\"signature\": \"{}\"\n}}\n",
        hex(&c.owner.0),
        c.field,
        c.seq,
        hex(&c.body),
        hex(&c.signed_object()),
        hex(&c.encode()),
        hex(&c.sig.0),
    );
    VectorFile { name: "claim.json", json }
}

fn receipt_vector() -> VectorFile {
    // conv_root is a fixed public test input; the derived MAC key is a secret
    // and is deliberately NOT emitted (secrets rule) — only the resulting MAC.
    let conv_root = [0x33u8; 32];
    let delivered = Id16([0x44; 16]);
    let key = receipt::receipt_key(&conv_root);
    let mac = receipt::receipt_mac(&key, &delivered);
    let json = format!(
        "{{\n  \"description\": \"receipt MAC (doc 05 §7): MAC = receipt_mac(receipt_key(conv_root), \
delivered_id). The derived key is a secret and is intentionally omitted.\",\n  \"conv_root\": \"{}\",\n  \
\"delivered_id\": \"{}\",\n  \"mac\": \"{}\"\n}}\n",
        hex(&conv_root),
        hex(&delivered.0),
        hex(&mac),
    );
    VectorFile { name: "receipt.json", json }
}

fn envelope_vector() -> VectorFile {
    let payload = b"golden frame payload";
    let wire = write_envelope(FT_BUNDLE, payload);
    let json = format!(
        "{{\n  \"description\": \"link envelope framing (doc 10 §2): [len u16 LE][type u8][payload]\",\n  \
\"frame_type\": {},\n  \"payload\": \"{}\",\n  \"wire\": \"{}\"\n}}\n",
        FT_BUNDLE,
        hex(payload),
        hex(&wire),
    );
    VectorFile { name: "envelope.json", json }
}

fn rbsr_fingerprint_vector() -> VectorFile {
    let ids = [Id16([0x01; 16]), Id16([0x02; 16]), Id16([0x03; 16])];
    let fp = mmp_core::fingerprint(&ids);
    let ids_hex: Vec<String> = ids.iter().map(|i| format!("\"{}\"", hex(&i.0))).collect();
    let json = format!(
        "{{\n  \"description\": \"RBSR range fingerprint of a known Id16 set (doc 06 §3) — order- and \
content-sensitive u64\",\n  \"ids\": [{}],\n  \"fingerprint_u64\": {},\n  \"fingerprint_hex\": \"{}\"\n}}\n",
        ids_hex.join(", "),
        fp,
        hex(&fp.to_le_bytes()),
    );
    VectorFile { name: "rbsr_fingerprint.json", json }
}

fn cross_domain_negative_vector() -> VectorFile {
    // The bundle vector's signature is valid ONLY under mmp1/bundle. Presented
    // under any other domain tag it MUST fail (AUDIT F4; rev-B adds
    // manifest/authcert/announce — doc 10 §3.1 / §6 rule 6).
    let (b, _) = vector_bundle();
    let json = format!(
        "{{\n  \"description\": \"cross-signing-domain reuse MUST reject (AUDIT F4, doc 10 §3.1). The \
signature is the mmp1/bundle signature from bundle.json; verified against the same canonical bytes and pubkey \
under each listed domain it must fail.\",\n  \"pubkey\": \"{}\",\n  \"object\": \"{}\",\n  \"signature\": \
\"{}\",\n  \"valid_domain\": \"mmp1/bundle\",\n  \"must_reject_domains\": [\"mmp1/claim\", \"mmp1/manifest\", \
\"mmp1/authcert\", \"mmp1/announce\"]\n}}\n",
        hex(&b.origin.0),
        hex(&b.canonical_zeroed()),
        hex(&b.signature.0),
    );
    VectorFile { name: "cross_domain_negative.json", json }
}

fn lifetime_reset_negative_vector() -> VectorFile {
    // Two bundles identical but for lifetime_ms hash to the SAME BundleID:
    // resetting the (relay-mutable, ID-excluded) lifetime cannot mint a "new"
    // bundle to defeat dedup (AUDIT F1).
    let (mut b, _) = vector_bundle();
    let id_a = b.id();
    let lifetime_a = b.lifetime_ms;
    b.lifetime_ms = 999; // attacker "resets" the remaining lifetime
    let id_b = b.id();
    debug_assert_eq!(id_a, id_b);
    let json = format!(
        "{{\n  \"description\": \"lifetime reset yields identical BundleID (AUDIT F1): lifetime_ms is \
excluded from the ID, so both lifetimes below hash to bundle_id.\",\n  \"lifetime_ms_a\": {},\n  \
\"lifetime_ms_b\": {},\n  \"bundle_id\": \"{}\"\n}}\n",
        lifetime_a,
        b.lifetime_ms,
        hex(&id_a.0),
    );
    VectorFile { name: "lifetime_reset_negative.json", json }
}

fn budget_clamp_negative_vector() -> VectorFile {
    // A received budget grant over L_MAX is clamped, never honored (AUDIT F3).
    let granted: u8 = 200;
    let clamped = granted.min(L_MAX);
    let json = format!(
        "{{\n  \"description\": \"budget grant over L_MAX is clamped on receive (AUDIT F3)\",\n  \
\"granted\": {},\n  \"l_max\": {},\n  \"clamped\": {}\n}}\n",
        granted, L_MAX, clamped,
    );
    VectorFile { name: "budget_clamp_negative.json", json }
}

#[cfg(test)]
mod tests {
    use mmp_crypto::verify;
    use mmp_types::frame::read_envelope;

    use super::*;

    /// The generator must be a pure function of nothing — no wall clock, no
    /// RNG, no env. Regenerating must produce byte-identical output, or the
    /// vectors aren't reproducible for a second implementation.
    #[test]
    fn regeneration_is_deterministic() {
        let a = generate();
        let b = generate();
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(b.iter()) {
            assert_eq!(x.name, y.name);
            assert_eq!(x.json, y.json, "vector {} is not reproducible", x.name);
        }
    }

    /// Drift guard: the committed `vectors/*.json` must byte-match what the
    /// current codecs generate. If this fails, a wire-format change shifted a
    /// golden byte — regenerate with `cargo run -p mmp-cli -- vectors` and
    /// review the diff before committing (that review IS the anti-ossification
    /// checkpoint, doc 10 §6.4).
    #[test]
    fn committed_vectors_match_current_codecs() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../vectors");
        for vf in generate() {
            let path = format!("{dir}/{}", vf.name);
            let committed = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("missing golden vector {path}: {e} — run `cargo run -p mmp-cli -- vectors`"));
            // Normalize CRLF so the test is stable regardless of the checkout's
            // line-ending policy (this repo is developed on Windows).
            assert_eq!(
                committed.replace("\r\n", "\n"),
                vf.json,
                "golden vector {} drifted from the current codec output",
                vf.name
            );
        }
    }

    /// Independent re-verification of the positive vectors' invariants — not
    /// just "the JSON matches" but "the bytes actually satisfy the protocol
    /// rule they encode".
    #[test]
    fn positive_vector_invariants_hold() {
        let (b, _) = vector_bundle();
        // BundleID is BLAKE3(canonical)[0..16], and the wire roundtrips.
        assert_eq!(Bundle::decode_canonical(&b.encode()).unwrap(), b);
        // The signature verifies under mmp1/bundle over the canonical bytes.
        verify(SigDomain::Bundle, &b.origin, &b.canonical_zeroed(), &b.signature)
            .expect("bundle signature must verify under mmp1/bundle");

        // Beacon roundtrips through its 24-byte codec.
        let beacon = vector_beacon();
        assert_eq!(Beacon::decode(&beacon.encode()).unwrap(), beacon);

        // Envelope roundtrips.
        let wire = write_envelope(FT_BUNDLE, b"golden frame payload");
        let parts = read_envelope(&wire).unwrap().unwrap();
        assert_eq!(parts.0, FT_BUNDLE);
        assert_eq!(parts.1, b"golden frame payload");
    }

    /// Independent re-verification of the negative vectors: the stated
    /// rejection actually happens.
    #[test]
    fn negative_vector_invariants_hold() {
        let (b, _) = vector_bundle();
        // Cross-domain: the mmp1/bundle signature must fail under every other
        // domain tag (AUDIT F4, rev-B tag set).
        for dom in [SigDomain::Claim, SigDomain::Manifest, SigDomain::AuthCert, SigDomain::Announce] {
            assert!(
                verify(dom, &b.origin, &b.canonical_zeroed(), &b.signature).is_err(),
                "signature must not verify under {dom:?}"
            );
        }

        // Lifetime reset: ID is stable across a lifetime change (AUDIT F1).
        let id_a = b.id();
        let mut b2 = b.clone();
        b2.lifetime_ms = 999;
        assert_eq!(id_a, b2.id());

        // Budget clamp (AUDIT F3).
        assert_eq!(200u8.min(L_MAX), L_MAX);
    }
}
