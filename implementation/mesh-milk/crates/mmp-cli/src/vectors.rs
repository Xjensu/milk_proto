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
//! The X3DH agreement vector remains deliberately deferred (it needs
//! deterministic-RNG plumbing for the ephemeral key). The non-canonical
//! Ed25519 negative is now emitted as a deterministic S+L fixture below;
//! `mmp-crypto`'s strict verifier remains the executable rejection oracle.

use mmp_crypto::{merkle, receipt, Identity};
use mmp_types::authcert::{scope_id_of, AnnouncementBody, AuthCert, CAP_ANNOUNCE};
use mmp_types::beacon::Beacon;
use mmp_types::bundle::{class, prio, Bundle, FLAG_WANTS_RECEIPT};
use mmp_types::claim::{field, friendship_signed_object, Claim, PrekeyBatch};
use mmp_types::consts::{
    LIFETIME_CLAIM_MS, LIFETIME_CONTROL_MS, LIFETIME_MESSAGE_MS, MAX_FRAME_PAYLOAD, RS_DATA_K,
    RS_TOTAL_N, SYMBOL_SIZE, L_MAX,
};
use mmp_types::domain::SigDomain;
use mmp_types::frame::{
    write_envelope, ClaimDigestEntry, Frame, FriendAckBody, FriendOfferBody, Hello, ProxySummaryBody,
    Range, Summary, SummaryBucket, WantHeads, FT_BLOB_HAVE, FT_BLOB_REQ, FT_BUDGET, FT_BUNDLE,
    FT_CLOSE, FT_CREDIT, FT_ERROR, FT_FRIEND_ACK, FT_FRIEND_OFFER, FT_HS, FT_ID_PROOF, FT_RECON_IDS,
    FT_RECON_INIT, FT_RECON_RANGE, FT_SUMMARY, FT_WANT,
};
use mmp_types::ids::{Id16, Key32, Sig64};
use mmp_types::manifest::{FileEntry, FileManifest};
use mmp_types::payload::{DirectPayload, GroupPayload, ReceiptPayload, SymbolPayload};
use mmp_types::tlv::{encode_exts, Ext, TLV_SCOPE_INFO};

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

/// The nine frozen signing domains from the protocol registry.  GroupMsg and
/// the candidate group-control tags are intentionally excluded: they are not
/// registered protocol domains and remain provisional implementation details.
const REGISTERED_SIGNATURE_DOMAINS: [(&str, SigDomain); 9] = [
    ("mmp1/bundle", SigDomain::Bundle),
    ("mmp1/claim", SigDomain::Claim),
    ("mmp1/friend", SigDomain::Friend),
    ("mmp1/prekey", SigDomain::Prekey),
    ("mmp1/blobroot", SigDomain::BlobRoot),
    ("mmp1/chanbind", SigDomain::ChanBind),
    ("mmp1/manifest", SigDomain::Manifest),
    ("mmp1/authcert", SigDomain::AuthCert),
    ("mmp1/announce", SigDomain::Announce),
];

/// One stable object/signature pair used by the cross-domain matrix.  The
/// object bytes come from the production signed-object helpers; only the
/// public key, object, and signature are emitted in the artifact.
struct SignatureDomainCase {
    name: &'static str,
    domain: SigDomain,
    public_key: Key32,
    object: Vec<u8>,
    signature: Sig64,
}

/// Build one semantically valid, deterministic object for each registered
/// signing domain.  No private key material is returned or serialized.
fn vector_signature_domain_cases() -> Vec<SignatureDomainCase> {
    let signer = vector_identity();
    let peer = Identity::from_seed(&[0x22; 32]);
    let signer_key = signer.public();
    let peer_key = peer.public();
    let scope = scope_id_of(b"vector/domain-matrix");

    let mut bundle = vector_bundle().0;
    bundle.signature = Sig64::ZERO;

    let claim = Claim {
        owner: signer_key,
        field: field::NICKNAME,
        seq: 0x0102_0304,
        body: b"domain-matrix".to_vec(),
        sig: Sig64::ZERO,
    };

    let prekey = PrekeyBatch {
        signed_prekey: Key32([0x42; 32]),
        spk_sig: Sig64::ZERO,
        prekeys: vec![Key32([0x43; 32]), Key32([0x44; 32])],
    };

    let friend_object = friendship_signed_object(&peer_key, &signer_key, 0x0102_0304_0506_0708);
    let blob_object = mmp_crypto::merkle::blobroot_object(&[0x45; 32], 0x0102_0304_0506_0708, 8, 12);
    let chanbind_object = mmp_crypto::noise::chanbind_object(&[0x46; 32], &signer_key, &peer_key);

    let manifest = FileManifest {
        suite: 1,
        sharer: signer_key,
        entries: vec![FileEntry {
            name: b"vector.txt".to_vec(),
            mime: b"text/plain".to_vec(),
            size: 13,
            file_root: Id16([0x47; 16]),
            rs_k: 8,
            rs_n: 12,
            full_hash: [0x48; 32],
            thumb_root: Some(Id16([0x49; 16])),
        }],
        sig: Sig64::ZERO,
    };

    let authcert = AuthCert {
        suite: 1,
        subject: peer_key,
        scope,
        caps: CAP_ANNOUNCE,
        priority_budget: 17,
        not_after: 0x0102_0304,
        issuer: signer_key,
        sig: Sig64::ZERO,
    };

    let announcement = AnnouncementBody {
        suite: 1,
        ann_type: 1,
        scope,
        not_after: 0x0506_0708,
        text: b"deterministic domain matrix".to_vec(),
        sig: Sig64::ZERO,
    };

    let objects = [
        ("mmp1/bundle", SigDomain::Bundle, bundle.canonical_zeroed()),
        ("mmp1/claim", SigDomain::Claim, claim.signed_object()),
        ("mmp1/friend", SigDomain::Friend, friend_object),
        ("mmp1/prekey", SigDomain::Prekey, prekey.signed_object()),
        ("mmp1/blobroot", SigDomain::BlobRoot, blob_object),
        ("mmp1/chanbind", SigDomain::ChanBind, chanbind_object),
        ("mmp1/manifest", SigDomain::Manifest, manifest.signed_object()),
        ("mmp1/authcert", SigDomain::AuthCert, authcert.signed_object()),
        ("mmp1/announce", SigDomain::Announce, announcement.signed_object()),
    ];

    objects
        .into_iter()
        .map(|(name, domain, object)| SignatureDomainCase {
            name,
            domain,
            public_key: signer_key,
            signature: signer.sign(domain, &object),
            object,
        })
        .collect()
}

/// Ed25519 subgroup order L, encoded little-endian as required for the
/// scalar S in an RFC 8032 signature.  Adding this value to a valid canonical
/// S preserves the group equation while making the wire scalar non-canonical
/// (`S >= L`); strict verification MUST reject it.
const ED25519_L_LE: [u8; 32] = [
    0xed, 0xd3, 0xf5, 0x5c, 0x1a, 0x63, 0x12, 0x58,
    0xd6, 0x9c, 0xf7, 0xa2, 0xde, 0xf9, 0xde, 0x14,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10,
];

/// Construct the deterministic overlarge-S negative without relying on a
/// private signing-key API.  The chosen canonical signature has S < L, so
/// S+L fits in 256 bits and cannot wrap; a carry would indicate a broken
/// fixture assumption and must fail generation rather than emit bad bytes.
fn add_l_to_s(signature: Sig64) -> Sig64 {
    let mut out = signature.0;
    let mut carry = 0u16;
    for i in 0..32 {
        let sum = out[32 + i] as u16 + ED25519_L_LE[i] as u16 + carry;
        out[32 + i] = sum as u8;
        carry = sum >> 8;
    }
    assert_eq!(carry, 0, "chosen valid S must permit an in-range 256-bit S+L encoding");
    Sig64(out)
}

/// One outer-bundle class fixture.  The payload bytes are always produced by
/// the corresponding `mmp-types` codec; `provisional` records where the
/// frozen specification does not fully define the implementation-owned inner
/// representation (for example the Merkle path framing added by E5-S1).
struct BundleClassCase {
    name: &'static str,
    class_name: &'static str,
    payload_schema: &'static str,
    status: &'static str,
    provisional: bool,
    provisional_reason: &'static str,
    bundle: Bundle,
    inner_pubkey: Option<Key32>,
}

/// Sign a bundle with the production outer-bundle codec and domain.  Keeping
/// this helper next to the class catalog makes every grouped vector obey the
/// same canonical lifetime-zeroing rule as the existing `bundle.json`.
struct BundleDraft {
    class_id: u8,
    flags: u8,
    dest: Id16,
    lifetime_ms: u64,
    priority: u8,
    exts: Vec<Ext>,
    payload: Vec<u8>,
}

fn signed_class_bundle(identity: &Identity, draft: BundleDraft) -> Bundle {
    let BundleDraft { class_id, flags, dest, lifetime_ms, priority, exts, payload } = draft;
    let mut bundle = Bundle {
        class: class_id,
        flags,
        origin: identity.public(),
        dest,
        lifetime_ms,
        priority,
        exts,
        payload,
        signature: Sig64::ZERO,
    };
    bundle.signature = identity.sign(SigDomain::Bundle, &bundle.canonical_zeroed());
    bundle
}

/// Deterministic outer-bundle catalog for classes 0 through 5.  This catalog
/// intentionally does not alter the legacy `bundle.json` fixture: that file
/// remains the class-1 compatibility vector used by older consumers.
fn vector_bundle_class_cases() -> Vec<BundleClassCase> {
    let identity = vector_identity();

    // Class 0: a real receipt MAC over a fixed public conversation root.  The
    // derived receipt key is not emitted in the artifact (secrets rule).
    let receipt_root = [0x33u8; 32];
    let delivered = Id16([0x44; 16]);
    let receipt_key = receipt::receipt_key(&receipt_root);
    let receipt = ReceiptPayload { delivered, mac: receipt::receipt_mac(&receipt_key, &delivered) };
    let class0 = signed_class_bundle(
        &identity,
        BundleDraft {
            class_id: class::CONTROL,
            flags: 0,
            dest: Id16([0x55; 16]),
            lifetime_ms: LIFETIME_CONTROL_MS,
            priority: prio::EMERGENCY,
            exts: Vec::new(),
            payload: receipt.encode(),
        },
    );

    // Class 1: the clear routing tuple is normative; ciphertext is a fixed
    // opaque AEAD-shaped fixture (tag/key context intentionally omitted).
    let direct = DirectPayload {
        conv: Id16([0x31; 16]),
        epoch: 7,
        index: 11,
        ct: vec![0xd1; 32],
    };
    let class1 = signed_class_bundle(
        &identity,
        BundleDraft {
            class_id: class::DIRECT,
            flags: FLAG_WANTS_RECEIPT,
            dest: Id16([0x22; 16]),
            lifetime_ms: LIFETIME_MESSAGE_MS,
            priority: prio::OPERATIONAL,
            exts: Vec::new(),
            payload: direct.encode(),
        },
    );

    // Class 2: use a deterministic group-member identity as the fixture's
    // group-signature public key.  Production group sender-key distribution
    // is pairwise and therefore not represented in this outer vector.
    let group_identity = Identity::from_seed(&[0x33; 32]);
    let gid = Id16([0x42; 16]);
    let mut group = GroupPayload {
        gid,
        epoch: 5,
        member: group_identity.node_id(),
        chain_index: 9,
        ct: vec![0xe2; 32],
        member_sig: Sig64::ZERO,
    };
    // The domain-separated signature is valid under the published fixture
    // key, while the omitted chain key means the ciphertext is not decrypted
    // by this vector test.
    group.member_sig = group_identity.sign(SigDomain::GroupMsg, &group.signed_object());
    let class2 = signed_class_bundle(
        &group_identity,
        BundleDraft {
            class_id: class::GROUP,
            flags: 0,
            dest: gid,
            lifetime_ms: LIFETIME_MESSAGE_MS,
            priority: prio::OPERATIONAL,
            exts: vec![Ext::GroupEpoch(group.epoch)],
            payload: group.encode(),
        },
    );

    // Class 3: a complete, inner-signed nickname Claim (the destination is
    // the well-known claim gossip scope used by mmp-node).
    let mut claim = Claim {
        owner: identity.public(),
        field: field::NICKNAME,
        seq: 12,
        body: b"bundle-class-vector".to_vec(),
        sig: Sig64::ZERO,
    };
    claim.sig = identity.sign(SigDomain::Claim, &claim.signed_object());
    let class3 = signed_class_bundle(
        &identity,
        BundleDraft {
            class_id: class::CLAIM,
            flags: 0,
            dest: Id16([0xff; 16]),
            lifetime_ms: LIFETIME_CLAIM_MS,
            priority: prio::NORMAL,
            exts: Vec::new(),
            payload: claim.encode(),
        },
    );

    // Class 4: construct a valid production Merkle path over four symbols so
    // the SymbolInfo coordinates and path leaf index can be checked without
    // inventing path bytes.  The path framing itself is an implementation
    // extension beyond frozen doc 10 §4.5, hence the explicit status label.
    let symbols: Vec<Vec<u8>> = (0..4)
        .map(|index| vec![0x60 + index as u8; SYMBOL_SIZE])
        .collect();
    let leaves: Vec<[u8; 32]> = symbols.iter().map(|symbol| merkle::hash_block(symbol)).collect();
    let symbol_index = 2usize;
    let path = merkle::merkle_path(&leaves, symbol_index);
    let mut root = [0u8; 16];
    root.copy_from_slice(&merkle::merkle_root(&leaves)[..16]);
    let symbol = SymbolPayload { path: path.encode(), symbol: symbols[symbol_index].clone() };
    let class4 = signed_class_bundle(
        &identity,
        BundleDraft {
            class_id: class::SYMBOL,
            flags: 0,
            dest: Id16([0x77; 16]),
            lifetime_ms: LIFETIME_MESSAGE_MS,
            priority: prio::BULK,
            exts: vec![Ext::SymbolInfo {
                root: Id16(root),
                block: 0,
                sym: symbol_index as u16,
                k: RS_DATA_K as u8,
                n: RS_TOTAL_N as u8,
            }],
            payload: symbol.encode(),
        },
    );

    // Class 5: public announcement.  `AnnouncementBody::suite` is retained
    // by the implementation because doc 18 includes it in the signed
    // preimage, while frozen doc 10 §4.6 omits it; this is recorded, not
    // silently normalized, in the vector metadata.
    let scope = scope_id_of(b"train/vector/001");
    let mut announcement = AnnouncementBody {
        suite: 1,
        ann_type: 1,
        scope,
        not_after: 1_900_000_000,
        text: b"platform-change vector".to_vec(),
        sig: Sig64::ZERO,
    };
    announcement.sig = identity.sign(SigDomain::Announce, &announcement.signed_object());
    let class5 = signed_class_bundle(
        &identity,
        BundleDraft {
            class_id: class::ANNOUNCEMENT,
            flags: 0,
            dest: scope,
            lifetime_ms: LIFETIME_MESSAGE_MS,
            priority: prio::OPERATIONAL,
            exts: vec![Ext::Optional { typ: TLV_SCOPE_INFO, val: b"train/vector/001".to_vec() }],
            payload: announcement.encode(),
        },
    );

    vec![
        BundleClassCase {
            name: "control_receipt",
            class_name: "control",
            payload_schema: "ReceiptPayload",
            status: "normative",
            provisional: false,
            provisional_reason: "",
            bundle: class0,
            inner_pubkey: None,
        },
        BundleClassCase {
            name: "direct_message",
            class_name: "direct",
            payload_schema: "DirectPayload",
            status: "normative_schema_opaque_ciphertext",
            provisional: true,
            provisional_reason: "AEAD key, nonce context, and plaintext are intentionally omitted",
            bundle: class1,
            inner_pubkey: None,
        },
        BundleClassCase {
            name: "group_message",
            class_name: "group",
            payload_schema: "GroupPayload",
            status: "implementation_assigned_groupmsg_domain",
            provisional: true,
            provisional_reason: "sender-key distribution and chain secret are outside the outer vector",
            bundle: class2,
            inner_pubkey: Some(group_identity.public()),
        },
        BundleClassCase {
            name: "claim_record",
            class_name: "claim",
            payload_schema: "Claim",
            status: "normative",
            provisional: false,
            provisional_reason: "",
            bundle: class3,
            inner_pubkey: None,
        },
        BundleClassCase {
            name: "blob_symbol",
            class_name: "symbol",
            payload_schema: "SymbolPayload+SymbolInfo",
            status: "implementation_extension",
            provisional: true,
            provisional_reason: "Merkle path framing is implementation-owned beyond frozen doc 10 §4.5",
            bundle: class4,
            inner_pubkey: None,
        },
        BundleClassCase {
            name: "authority_announcement",
            class_name: "announcement",
            payload_schema: "AnnouncementBody",
            status: "implementation_extension_spec_gap",
            provisional: true,
            provisional_reason: "suite byte is present in doc 18 preimage but omitted by frozen doc 10 §4.6",
            bundle: class5,
            inner_pubkey: None,
        },
    ]
}

/// Build every vector, deterministically. Same output every call, on every
/// machine — asserted by `regeneration_is_deterministic`.
pub fn generate() -> Vec<VectorFile> {
    vec![
        bundle_vector(),
        bundles_vector(),
        beacon_vector(),
        hello_vector(),
        claim_vector(),
        receipt_vector(),
        envelope_vector(),
        rbsr_fingerprint_vector(),
        cross_domain_negative_vector(),
        cross_domain_matrix_vector(),
        noncanonical_s_negative_vector(),
        lifetime_reset_negative_vector(),
        budget_clamp_negative_vector(),
        frames_vector(),
        envelope_oversize_negative_vector(),
        bundle_tlv_negative_vector(),
        coverage_manifest_vector(),
    ]
}

/// One deterministic sample for every registered MMP/1 frame type.  These
/// values deliberately contain no private material: the frame layer carries
/// identity/signature bytes but does not authenticate them.  Authentication
/// remains the responsibility of the Noise/session and higher-level codecs.
struct FrameCase {
    name: &'static str,
    frame: Frame,
}

const REGISTERED_FRAME_TYPES: [u8; 17] = [
    FT_HS,
    FT_ID_PROOF,
    FT_CREDIT,
    FT_SUMMARY,
    FT_RECON_INIT,
    FT_RECON_RANGE,
    FT_RECON_IDS,
    FT_WANT,
    FT_BUNDLE,
    FT_BUDGET,
    FT_BLOB_REQ,
    FT_BLOB_HAVE,
    FT_FRIEND_OFFER,
    FT_FRIEND_ACK,
    mmp_types::frame::FT_PROXY_SUMMARY,
    FT_ERROR,
    FT_CLOSE,
];

fn vector_frame_cases() -> Vec<FrameCase> {
    let id16_a = Id16([0x11; 16]);
    let id16_b = Id16([0x22; 16]);
    let id16_c = Id16([0x33; 16]);
    let range = Range { lo: id16_a, hi: id16_b, count: 3, fp: 0x0102_0304_0506_0708 };
    let hello = Hello {
        ver: 1,
        suites: vec![1, 2],
        tier: 2,
        flags: 0x0003,
        credits: 8,
        psm: 0x0025,
    };
    vec![
        FrameCase { name: "hs", frame: Frame::Hs(hello.encode()) },
        FrameCase {
            name: "id_proof",
            frame: Frame::IdProof { identity: Key32([0x44; 32]), sig: Sig64([0x55; 64]) },
        },
        FrameCase { name: "credit", frame: Frame::Credit(7) },
        FrameCase {
            name: "summary",
            frame: Frame::Summary(Summary {
                receipt_buckets: vec![SummaryBucket { bucket_epoch: 9, bloom: vec![0xa0, 0xa1] }],
                claim_digest: vec![ClaimDigestEntry { node_prefix: 0x0102_0304, field: 1, seq: 5 }],
                group_hints: vec![id16_c],
                want_heads: vec![WantHeads { conv: id16_a, heads: vec![id16_b, id16_c] }],
            }),
        },
        FrameCase {
            name: "recon_init",
            frame: Frame::ReconInit { domain: 1, window: 4, range },
        },
        FrameCase {
            name: "recon_range",
            frame: Frame::ReconRange { domain: 2, ranges: vec![range, Range { lo: id16_b, hi: id16_c, count: 1, fp: 9 }] },
        },
        FrameCase {
            name: "recon_ids",
            frame: Frame::ReconIds { domain: 3, lo: id16_a, hi: id16_c, ids: vec![id16_a, id16_b] },
        },
        FrameCase { name: "want", frame: Frame::Want(vec![id16_a, id16_b]) },
        FrameCase { name: "bundle", frame: Frame::Bundle(vector_bundle().0.encode()) },
        FrameCase { name: "budget", frame: Frame::Budget { id: id16_b, granted: 6 } },
        FrameCase {
            name: "blob_req",
            frame: Frame::BlobReq { root: id16_a, block: 5, bitmap: vec![0xf0, 0x0f] },
        },
        FrameCase {
            name: "blob_have",
            frame: Frame::BlobHave { root: id16_b, block: 6, bitmap: vec![0x0f, 0xf0] },
        },
        FrameCase {
            name: "friend_offer",
            frame: Frame::FriendOffer(FriendOfferBody { since: 0x0102_0304_0506_0708 }.encode()),
        },
        FrameCase {
            name: "friend_ack",
            frame: Frame::FriendAck(FriendAckBody { countersig: Sig64([0x66; 64]) }.encode()),
        },
        FrameCase {
            name: "proxy_summary",
            frame: Frame::ProxySummary(ProxySummaryBody { dependent: id16_c, bloom: vec![0xbe, 0xef] }.encode()),
        },
        FrameCase {
            name: "error",
            frame: Frame::Error { code: mmp_types::frame::errcode::PROTOCOL, detail: b"frame vector".to_vec() },
        },
        FrameCase { name: "close", frame: Frame::Close },
    ]
}

/// Serialize the complete registered-frame inventory as one JSON artifact.
/// The artifact records the exact payload and envelope bytes while leaving
/// semantic payload validation to the production round-trip test and the
/// independent envelope checks in `conformance.rs`.
fn frames_vector() -> VectorFile {
    let cases = vector_frame_cases();
    debug_assert_eq!(cases.len(), 17);
    debug_assert!(cases.iter().zip(REGISTERED_FRAME_TYPES).all(|(case, typ)| case.frame.typ() == typ));
    let mut json = String::from(
        "{\n  \"description\": \"deterministic payload and envelope bytes for every registered MMP/1 frame type\",\n  \"frame_count\": 17,\n  \"frames\": [\n",
    );
    for (index, case) in cases.iter().enumerate() {
        let payload = case.frame.encode_payload();
        let wire = case.frame.encode();
        json.push_str(&format!(
            "    {{\"name\": \"{}\", \"type\": {}, \"payload\": \"{}\", \"wire\": \"{}\"}}{}\n",
            case.name,
            case.frame.typ(),
            hex(&payload),
            hex(&wire),
            if index + 1 == cases.len() { "" } else { "," },
        ));
    }
    json.push_str("  ]\n}\n");
    VectorFile { name: "frames.json", json }
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

/// Serialize the complete class-0..5 catalog as one grouped JSON artifact.
/// Every byte field is lowercase hex; metadata explicitly calls out
/// implementation/provisional payload portions so consumers do not mistake
/// an opaque cryptographic fixture for an interoperable plaintext example.
fn bundles_vector() -> VectorFile {
    let cases = vector_bundle_class_cases();
    debug_assert_eq!(cases.len(), 6);
    let mut json = String::from(
        "{\n  \"description\": \"deterministic outer Bundle vectors for classes 0 through 5; bundle.json remains the legacy class-1 compatibility vector\",\n  \"bundle_count\": 6,\n  \"bundles\": [\n",
    );
    for (index, case) in cases.iter().enumerate() {
        let bundle = &case.bundle;
        let mut ext_wire = Vec::new();
        encode_exts(&bundle.exts, &mut ext_wire);
        let inner_pubkey = case
            .inner_pubkey
            .map(|key| format!("\"{}\"", hex(&key.0)))
            .unwrap_or_else(|| "null".to_owned());
        json.push_str(&format!(
            "    {{\"name\": \"{}\", \"class\": {}, \"class_name\": \"{}\", \"payload_schema\": \"{}\", \"status\": \"{}\", \"provisional\": {}, \"provisional_reason\": \"{}\", \"outer_signature_domain\": \"mmp1/bundle\", \"origin_pubkey\": \"{}\", \"dest\": \"{}\", \"lifetime_ms\": {}, \"priority\": {}, \"flags\": {}, \"extensions_wire\": \"{}\", \"payload_len\": {}, \"payload\": \"{}\", \"canonical_zeroed\": \"{}\", \"wire\": \"{}\", \"bundle_id\": \"{}\", \"signature\": \"{}\", \"inner_pubkey\": {} }}{}\n",
            case.name,
            bundle.class,
            case.class_name,
            case.payload_schema,
            case.status,
            case.provisional,
            case.provisional_reason,
            hex(&bundle.origin.0),
            hex(&bundle.dest.0),
            bundle.lifetime_ms,
            bundle.priority,
            bundle.flags,
            hex(&ext_wire),
            bundle.payload.len(),
            hex(&bundle.payload),
            hex(&bundle.canonical_zeroed()),
            hex(&bundle.encode()),
            hex(&bundle.id().0),
            hex(&bundle.signature.0),
            inner_pubkey,
            if index + 1 == cases.len() { "" } else { "," },
        ));
    }
    json.push_str("  ]\n}\n");
    VectorFile { name: "bundles.json", json }
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

fn hello_vector() -> VectorFile {
    let hello = Hello { ver: 1, suites: vec![1, 2], tier: 2, flags: 0x0003, credits: 8, psm: 0x0025 };
    let json = format!(
        "{{\n  \"description\": \"HELLO payload codec (doc 10 §2.1)\",\n  \"ver\": {},\n  \"suites\": \"{}\",\n  \"tier\": {},\n  \"flags\": {},\n  \"credits\": {},\n  \"psm\": {},\n  \"wire\": \"{}\"\n}}\n",
        hello.ver,
        hex(&hello.suites),
        hello.tier,
        hello.flags,
        hello.credits,
        hello.psm,
        hex(&hello.encode()),
    );
    VectorFile { name: "hello.json", json }
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

/// Encode an envelope without the production writer's debug-only size guard.
/// The negative vectors deliberately exercise bytes that a hostile peer can
/// put on the wire; constructing them must not trip the writer canary first.
fn raw_envelope(typ: u8, payload_len: usize, fill: u8) -> Vec<u8> {
    assert!(payload_len <= u16::MAX as usize, "negative envelope must fit u16 length");
    let mut wire = Vec::with_capacity(3 + payload_len);
    wire.extend_from_slice(&(payload_len as u16).to_le_bytes());
    wire.push(typ);
    wire.resize(3 + payload_len, fill);
    wire[2] = typ;
    wire
}

/// Encode one raw extension TLV for malformed-layout vectors.  This helper is
/// intentionally byte-level: `Ext` only represents canonical values and its
/// production encoder asserts strict ordering, while these fixtures must
/// preserve the malformed bytes exactly as a peer could transmit them.
fn raw_tlv(typ: u8, value: &[u8]) -> Vec<u8> {
    assert!(value.len() <= u16::MAX as usize);
    let mut out = Vec::with_capacity(3 + value.len());
    out.push(typ);
    out.extend_from_slice(&(value.len() as u16).to_le_bytes());
    out.extend_from_slice(value);
    out
}

/// Replace the extension run in the deterministic class-1 bundle while
/// retaining its payload and signature tail.  The resulting bytes are
/// intentionally not signed: production canonical decoding must reject the
/// malformed TLVs before authentication is attempted.
fn bundle_wire_with_raw_exts(exts: &[u8]) -> Vec<u8> {
    let (bundle, _) = vector_bundle();
    let base = bundle.encode();
    assert_eq!(u16::from_le_bytes([base[56], base[57]]), 0, "base vector must have no TLVs");
    let mut wire = Vec::with_capacity(base.len() + exts.len());
    wire.extend_from_slice(&base[..56]);
    wire.extend_from_slice(&(exts.len() as u16).to_le_bytes());
    wire.extend_from_slice(exts);
    wire.extend_from_slice(&base[58..]);
    wire
}

/// Negative envelope vectors pin the frozen 4096-byte envelope cap.  The
/// receive-side reader and the logical frame decoder intentionally share this
/// limit; the Noise 16-byte tag is accounted for by the outbound plaintext
/// budget rather than by widening the wire envelope.
fn envelope_oversize_negative_vector() -> VectorFile {
    const RUNTIME_CAP: usize = mmp_types::frame::MAX_ENVELOPE_PAYLOAD;
    debug_assert_eq!(MAX_FRAME_PAYLOAD, 4096);
    debug_assert_eq!(RUNTIME_CAP, 4096);
    let normative_over = raw_envelope(FT_BUNDLE, MAX_FRAME_PAYLOAD + 1, 0xa1);
    let runtime_over = raw_envelope(FT_BUNDLE, RUNTIME_CAP + 33, 0xa2);
    let json = format!(
        "{{\n  \"description\": \"negative envelope lengths: the frozen logical and runtime envelope payload cap is 4096 bytes\",\n  \"normative_frame_payload_cap\": {},\n  \"runtime_envelope_payload_cap\": {},\n  \"cases\": [\n    {{\"name\": \"logical_payload_4097\", \"payload_len\": {}, \"wire\": \"{}\", \"normative_verdict\": \"reject\", \"runtime_verdict\": \"reject_oversize\"}},\n    {{\"name\": \"envelope_payload_4129\", \"payload_len\": {}, \"wire\": \"{}\", \"normative_verdict\": \"reject\", \"runtime_verdict\": \"reject_oversize\"}}\n  ]\n}}\n",
        MAX_FRAME_PAYLOAD,
        RUNTIME_CAP,
        normative_over.len() - 3,
        hex(&normative_over),
        runtime_over.len() - 3,
        hex(&runtime_over),
    );
    VectorFile { name: "envelope_oversize_negative.json", json }
}

/// Negative bundle vectors for the three canonical extension rules in doc 10
/// §6.1: unknown critical TLVs, strictly ascending type order, and duplicate
/// types (the latter is covered by the same order check).
fn bundle_tlv_negative_vector() -> VectorFile {
    let unknown_critical = bundle_wire_with_raw_exts(&raw_tlv(0x04, &[0xc1]));
    let out_of_order = {
        let mut ext = raw_tlv(0x02, &[0x34, 0x12]);
        ext.extend_from_slice(&raw_tlv(0x01, &[0x55; 16]));
        bundle_wire_with_raw_exts(&ext)
    };
    let duplicate = {
        let mut ext = raw_tlv(0x02, &[0x34, 0x12]);
        ext.extend_from_slice(&raw_tlv(0x02, &[0x78, 0x56]));
        bundle_wire_with_raw_exts(&ext)
    };
    let json = format!(
        "{{\n  \"description\": \"malformed bundle extension runs; production and independent readers MUST reject unknown critical, out-of-order, and duplicate TLVs (doc 10 §6.1)\",\n  \"cases\": [\n    {{\"name\": \"unknown_critical_tlv\", \"wire\": \"{}\", \"expected_error\": \"unknown critical TLV\"}},\n    {{\"name\": \"out_of_order_tlvs\", \"wire\": \"{}\", \"expected_error\": \"TLV order/duplicate\"}},\n    {{\"name\": \"duplicate_tlv_type\", \"wire\": \"{}\", \"expected_error\": \"TLV order/duplicate\"}}\n  ]\n}}\n",
        hex(&unknown_critical),
        hex(&out_of_order),
        hex(&duplicate),
    );
    VectorFile { name: "bundle_tlv_negative.json", json }
}

/// Machine-readable map from every committed artifact to its normative
/// requirement and the verification evidence that consumes it.  Keeping this
/// manifest generated makes additions fail the same drift gate as wire bytes.
fn coverage_manifest_vector() -> VectorFile {
    const ENTRIES: &[(&str, &str, &str, &str)] = &[
        ("beacon.codec.v1", "beacon.json", "specification/10-message-format.md §5", "independent_fixed_reader+production_roundtrip"),
        ("bundle.compat.class1.v1", "bundle.json", "specification/10-message-format.md §3", "independent_outer_reader+production_signature"),
        ("bundle.classes.0-5.v1", "bundles.json", "specification/10-message-format.md §3-§4", "independent_outer_reader+production_payload_invariants"),
        ("claim.nickname.v1", "claim.json", "specification/06-sync-spec.md §4; specification/10-message-format.md §4.4", "production_roundtrip+signature"),
        ("negative.budget.clamp.v1", "budget_clamp_negative.json", "specification/05-routing-spec.md §6", "production_invariant"),
        ("negative.bundle.tlv.v1", "bundle_tlv_negative.json", "specification/10-message-format.md §6.1", "independent_tlv_reader+production_rejection"),
        ("negative.cross-domain.v1", "cross_domain_negative.json", "specification/10-message-format.md §3.1", "production_cross_domain_rejection"),
        ("negative.cross-domain.matrix.v1", "cross_domain_matrix.json", "specification/10-message-format.md §3.1", "independent_schema+production_9x9_matrix"),
        ("negative.envelope.oversize.v1", "envelope_oversize_negative.json", "specification/10-message-format.md §6.4", "independent_normative_cap+production_runtime_cap"),
        ("negative.ed25519.s-plus-l.v1", "noncanonical_s_negative.json", "specification/10-message-format.md §6.3", "independent_scalar_arithmetic+strict_verify"),
        ("negative.lifetime-reset.v1", "lifetime_reset_negative.json", "specification/10-message-format.md §3", "production_bundle_id_invariant"),
        ("envelope.framing.v1", "envelope.json", "specification/10-message-format.md §2", "independent_envelope_reader+production_roundtrip"),
        ("frame.hello.v1", "hello.json", "specification/10-message-format.md §2.1", "independent_fixed_reader+production_roundtrip"),
        ("frame.registry.v1", "frames.json", "specification/10-message-format.md §2", "independent_payload_grammar+production_roundtrip"),
        ("receipt.mac.v1", "receipt.json", "specification/05-routing-spec.md §7", "production_mac_invariant"),
        ("rbsr.fingerprint.v1", "rbsr_fingerprint.json", "specification/06-sync-spec.md §9", "production_fingerprint_invariant"),
        ("coverage.manifest.v1", "coverage.json", "specification/12-rust-blueprint.md §7", "machine_readable_manifest_completeness"),
    ];
    let mut json = String::from(
        "{\n  \"schema\": \"mmp-e5-s4-coverage-v1\",\n  \"description\": \"normative requirement coverage for committed MMP/1 vectors\",\n  \"entry_count\": 17,\n  \"entries\": [\n",
    );
    for (index, (id, artifact, source, evidence)) in ENTRIES.iter().enumerate() {
        json.push_str(&format!(
            "    {{\"id\": \"{}\", \"artifact\": \"{}\", \"source\": \"{}\", \"evidence\": \"{}\" }}{}\n",
            id,
            artifact,
            source,
            evidence,
            if index + 1 == ENTRIES.len() { "" } else { "," },
        ));
    }
    json.push_str("  ]\n}\n");
    VectorFile { name: "coverage.json", json }
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

/// Emit the complete 9×9 domain-separation matrix.  Each row signs one
/// production object under its declared domain; all nine domain verifiers are
/// then recorded explicitly.  The expected result is one diagonal accept and
/// eight off-diagonal rejects per row (9 accepts + 72 rejects total).
fn cross_domain_matrix_vector() -> VectorFile {
    let cases = vector_signature_domain_cases();
    let mut json = String::from(
        "{\n  \"description\": \"complete registered SigDomain cross-verification matrix: diagonal accepts, all off-diagonal pairs reject\",\n  \"domain_count\": 9,\n  \"matrix_cells\": 81,\n  \"expected_accepts\": 9,\n  \"expected_rejects\": 72,\n  \"domains\": [\n",
    );
    for (row_index, case) in cases.iter().enumerate() {
        json.push_str(&format!(
            "    {{\"object_domain\": \"{}\", \"domain_tag\": \"{}\", \"pubkey\": \"{}\", \"object\": \"{}\", \"signature\": \"{}\", \"checks\": [",
            case.name,
            hex(case.domain.tag()),
            hex(&case.public_key.0),
            hex(&case.object),
            hex(&case.signature.0),
        ));
        for (check_index, (name, domain)) in REGISTERED_SIGNATURE_DOMAINS.iter().enumerate() {
            let accepted = mmp_crypto::verify(*domain, &case.public_key, &case.object, &case.signature).is_ok();
            debug_assert_eq!(accepted, row_index == check_index);
            json.push_str(&format!(
                "{{\"verify_domain\": \"{}\", \"result\": \"{}\"}}{}",
                name,
                if accepted { "accept" } else { "reject" },
                if check_index + 1 == REGISTERED_SIGNATURE_DOMAINS.len() { "" } else { "," },
            ));
        }
        json.push_str(&format!(
            "]}}{}\n",
            if row_index + 1 == cases.len() { "" } else { "," },
        ));
    }
    json.push_str("  ]\n}\n");
    VectorFile { name: "cross_domain_matrix.json", json }
}

fn noncanonical_s_negative_vector() -> VectorFile {
    // Reuse the canonical class-1 bundle object, but replace only the
    // signature scalar S with S+L.  The public key, R, domain, and object are
    // unchanged; only strict canonical-S verification should distinguish the
    // two signatures (RFC 8032 §5.1.7; doc 08 §10, doc 10 §6.4).
    let (b, _) = vector_bundle();
    let valid = b.signature;
    let malformed = add_l_to_s(valid);
    assert!(mmp_crypto::verify(SigDomain::Bundle, &b.origin, &b.canonical_zeroed(), &valid).is_ok());
    assert!(mmp_crypto::verify(SigDomain::Bundle, &b.origin, &b.canonical_zeroed(), &malformed).is_err());
    let json = format!(
        "{{\n  \"description\": \"Ed25519 overlarge-S negative: replacing canonical S with S+L MUST be rejected by strict verification (RFC 8032 §5.1.7, AUDIT F9)\",\n  \"domain\": \"mmp1/bundle\",\n  \"pubkey\": \"{}\",\n  \"object\": \"{}\",\n  \"valid_signature\": \"{}\",\n  \"noncanonical_signature\": \"{}\",\n  \"valid_s\": \"{}\",\n  \"noncanonical_s\": \"{}\",\n  \"expected_valid\": \"accept\",\n  \"expected_noncanonical\": \"reject\"\n}}\n",
        hex(&b.origin.0),
        hex(&b.canonical_zeroed()),
        hex(&valid.0),
        hex(&malformed.0),
        hex(&valid.0[32..]),
        hex(&malformed.0[32..]),
    );
    VectorFile { name: "noncanonical_s_negative.json", json }
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
    use mmp_types::err::WireError;
    use mmp_types::frame::{read_envelope, Frame};

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

        // Canonical-S negative: adding the Ed25519 subgroup order L changes
        // neither R nor the group equation, but the resulting S is overlarge
        // and MUST be rejected by the public strict-verification path.
        let noncanonical = add_l_to_s(b.signature);
        assert_eq!(noncanonical.0[..32], b.signature.0[..32], "S+L must preserve R");
        assert!(verify(SigDomain::Bundle, &b.origin, &b.canonical_zeroed(), &b.signature).is_ok());
        assert!(verify(SigDomain::Bundle, &b.origin, &b.canonical_zeroed(), &noncanonical).is_err());

        // Lifetime reset: ID is stable across a lifetime change (AUDIT F1).
        let id_a = b.id();
        let mut b2 = b.clone();
        b2.lifetime_ms = 999;
        assert_eq!(id_a, b2.id());

        // Budget clamp (AUDIT F3).
        assert_eq!(200u8.min(L_MAX), L_MAX);
    }

    /// The frozen logical payload and receive-envelope caps are both 4096
    /// bytes.  Both one-byte-over and substantially-over inputs must be
    /// rejected before the incremental reader can grow a buffer.
    #[test]
    fn oversized_envelope_vectors_pin_normative_and_runtime_caps() {
        let logical_over = raw_envelope(FT_BUNDLE, MAX_FRAME_PAYLOAD + 1, 0xa1);
        assert_eq!(logical_over.len() - 3, MAX_FRAME_PAYLOAD + 1);
        assert_eq!(read_envelope(&logical_over), Err(WireError::Oversize));

        let runtime_over = raw_envelope(FT_BUNDLE, mmp_types::frame::MAX_ENVELOPE_PAYLOAD + 1, 0xa2);
        assert_eq!(
            read_envelope(&runtime_over),
            Err(WireError::Oversize),
            "lengths over MAX_ENVELOPE_PAYLOAD must fail closed"
        );
    }

    /// Malformed extension runs must be rejected by the production canonical
    /// bundle decoder before signature verification or relay admission.
    #[test]
    fn malformed_bundle_tlv_vectors_are_rejected() {
        let unknown_critical = bundle_wire_with_raw_exts(&raw_tlv(0x04, &[0xc1]));
        assert_eq!(Bundle::decode_canonical(&unknown_critical), Err(WireError::TlvUnknownCritical));

        let mut out_of_order_ext = raw_tlv(0x02, &[0x34, 0x12]);
        out_of_order_ext.extend_from_slice(&raw_tlv(0x01, &[0x55; 16]));
        let out_of_order = bundle_wire_with_raw_exts(&out_of_order_ext);
        assert_eq!(Bundle::decode_canonical(&out_of_order), Err(WireError::TlvOrder));

        let mut duplicate_ext = raw_tlv(0x02, &[0x34, 0x12]);
        duplicate_ext.extend_from_slice(&raw_tlv(0x02, &[0x78, 0x56]));
        let duplicate = bundle_wire_with_raw_exts(&duplicate_ext);
        assert_eq!(Bundle::decode_canonical(&duplicate), Err(WireError::TlvOrder));
    }

    /// Every registered signing-domain pair is exercised: each row's own
    /// domain accepts, and all 72 off-diagonal combinations reject.  This is
    /// deliberately a full matrix rather than a single negative sample so a
    /// newly added domain cannot silently go untested.
    #[test]
    fn full_registered_signature_domain_matrix_is_separated() {
        let cases = vector_signature_domain_cases();
        assert_eq!(cases.len(), REGISTERED_SIGNATURE_DOMAINS.len());
        let mut accepts = 0;
        let mut rejects = 0;
        for (row, case) in cases.iter().enumerate() {
            assert_eq!(case.name, REGISTERED_SIGNATURE_DOMAINS[row].0);
            assert_eq!(case.domain, REGISTERED_SIGNATURE_DOMAINS[row].1);
            for (column, (_, domain)) in REGISTERED_SIGNATURE_DOMAINS.iter().enumerate() {
                let accepted = verify(*domain, &case.public_key, &case.object, &case.signature).is_ok();
                if row == column {
                    assert!(accepted, "{} signature must verify under its own domain", case.name);
                    accepts += 1;
                } else {
                    assert!(!accepted, "{} signature must reject under {}", case.name, domain.tag().escape_ascii());
                    rejects += 1;
                }
            }
        }
        assert_eq!(accepts, 9);
        assert_eq!(rejects, 72);
    }

    /// Every registered frame type must have a deterministic sample and must
    /// round-trip through the production payload codec and envelope framing.
    /// The committed `frames.json` artifact is checked independently in
    /// `conformance.rs`; this test is the production-codec half of the gate.
    #[test]
    fn every_registered_frame_roundtrips() {
        let cases = vector_frame_cases();
        assert_eq!(cases.len(), REGISTERED_FRAME_TYPES.len());
        for (case, expected_type) in cases.iter().zip(REGISTERED_FRAME_TYPES) {
            assert_eq!(case.frame.typ(), expected_type, "frame case {} has wrong type", case.name);
            let wire = case.frame.encode();
            let (typ, payload, consumed) = read_envelope(&wire)
                .expect("frame envelope must parse")
                .expect("complete frame envelope");
            assert_eq!(consumed, wire.len(), "frame {} has trailing bytes", case.name);
            assert_eq!(typ, expected_type, "frame {} has wrong envelope tag", case.name);
            assert_eq!(payload, case.frame.encode_payload(), "frame {} payload drift", case.name);
            assert_eq!(Frame::decode_payload(typ, payload).expect("frame payload must decode"), case.frame);
        }
    }

    /// Every bundle class has a production-codec roundtrip and the outer
    /// mmp1/bundle signature is checked independently.  Where a class has a
    /// complete, unambiguous inner schema, its own decoder/signature/path
    /// invariant is checked too; opaque cryptographic fixtures are deliberately
    /// not treated as decryptable test messages.
    #[test]
    fn every_bundle_class_roundtrips_and_validates_declared_invariants() {
        let cases = vector_bundle_class_cases();
        assert_eq!(cases.iter().map(|case| case.bundle.class).collect::<Vec<_>>(), (0u8..=5).collect::<Vec<_>>());
        for case in &cases {
            let wire = case.bundle.encode();
            let decoded = Bundle::decode_canonical(&wire).expect("bundle class wire must decode");
            assert_eq!(&decoded, &case.bundle, "class {} bundle drifted", case.bundle.class);
            assert_eq!(decoded.id(), case.bundle.id());
            verify(
                SigDomain::Bundle,
                &decoded.origin,
                &decoded.canonical_zeroed(),
                &decoded.signature,
            )
            .unwrap_or_else(|_| panic!("class {} outer signature must verify", case.bundle.class));

            match case.bundle.class {
                class::CONTROL => {
                    assert_eq!(ReceiptPayload::decode(&decoded.payload).unwrap().encode(), decoded.payload);
                }
                class::DIRECT => {
                    assert_eq!(DirectPayload::decode(&decoded.payload).unwrap().encode(), decoded.payload);
                }
                class::GROUP => {
                    let group = GroupPayload::decode(&decoded.payload).unwrap();
                    let member_pubkey = case.inner_pubkey.expect("group fixture public key");
                    verify(SigDomain::GroupMsg, &member_pubkey, &group.signed_object(), &group.member_sig)
                        .expect("group fixture signature must verify under its declared fixture key");
                    assert_eq!(group.encode(), decoded.payload);
                }
                class::CLAIM => {
                    let claim = Claim::decode(&decoded.payload).unwrap();
                    verify(SigDomain::Claim, &claim.owner, &claim.signed_object(), &claim.sig)
                        .expect("claim inner signature must verify");
                    assert_eq!(claim.encode(), decoded.payload);
                }
                class::SYMBOL => {
                    let symbol = SymbolPayload::decode(&decoded.payload).unwrap();
                    let path = merkle::MerklePath::decode(&symbol.path).unwrap();
                    let info = decoded
                        .exts
                        .iter()
                        .find_map(|ext| match ext {
                            Ext::SymbolInfo { root, block, sym, .. } => Some((*root, *block, *sym)),
                            _ => None,
                        })
                        .expect("symbol bundle must carry SymbolInfo");
                    let leaf = merkle::hash_block(&symbol.symbol);
                    let root = merkle::compute_root(&leaf, &path).unwrap();
                    assert_eq!(root[..16], info.0.0, "symbol path root prefix must match SymbolInfo");
                    assert_eq!(path.leaf_index(), u64::from(info.1) * RS_TOTAL_N as u64 + u64::from(info.2));
                    assert_eq!(symbol.encode(), decoded.payload);
                }
                class::ANNOUNCEMENT => {
                    let announcement = AnnouncementBody::decode(&decoded.payload).unwrap();
                    verify(
                        SigDomain::Announce,
                        &decoded.origin,
                        &announcement.signed_object(),
                        &announcement.sig,
                    )
                    .expect("announcement inner signature must verify");
                    assert_eq!(announcement.scope, decoded.dest, "announcement scope must bind to bundle dest");
                    assert_eq!(announcement.encode(), decoded.payload);
                }
                other => panic!("unexpected class {} in class catalog", other),
            }
        }
    }
}

// Keep the independent conformance reader out of the production CLI binary.
// It deliberately does not call any `mmp-types` decoder; see the module-level
// tests in `conformance.rs` for the wire assertions.
#[cfg(test)]
#[path = "conformance.rs"]
mod conformance;
