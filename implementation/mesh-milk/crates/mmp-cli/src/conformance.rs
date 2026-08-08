//! Specification-derived checks for the committed E5-S4 vectors.
//!
//! This module is test-only and intentionally has no dependency on the
//! production frame or bundle decoders.  A vector is useful as an
//! anti-ossification artifact only when an independent reader can reject
//! malformed bytes even if both the generator and production decoder drift in
//! the same way.

use std::fs;
use std::path::{Path, PathBuf};

const VECTOR_NAMES: &[&str] = &[
    "beacon.json",
    "budget_clamp_negative.json",
    "bundle.json",
    "bundles.json",
    "claim.json",
    "cross_domain_negative.json",
    "cross_domain_matrix.json",
    "envelope.json",
    "frames.json",
    "hello.json",
    "lifetime_reset_negative.json",
    "noncanonical_s_negative.json",
    "rbsr_fingerprint.json",
    "receipt.json",
];

fn vectors_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vectors")
}

fn vector(name: &str) -> String {
    let path = vectors_dir().join(name);
    fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read committed vector {}: {e}", path.display()))
}

/// Read a JSON string field from the deliberately simple, generator-owned
/// vector format.  The vectors contain hex strings and descriptions but no
/// escaped quote in the fields consumed below, so this parser remains small
/// and auditable instead of importing a second serialization implementation
/// into the CLI.
fn json_string<'a>(json: &'a str, key: &str) -> &'a str {
    let needle = format!("\"{key}\"");
    let key_at = json
        .find(&needle)
        .unwrap_or_else(|| panic!("missing JSON field {key:?}"));
    let after_key = &json[key_at + needle.len()..];
    let colon = after_key
        .find(':')
        .unwrap_or_else(|| panic!("missing colon after JSON field {key:?}"));
    let value = after_key[colon + 1..].trim_start();
    assert!(value.starts_with('"'), "JSON field {key:?} is not a string");
    let body = &value[1..];
    let end = body
        .find('"')
        .unwrap_or_else(|| panic!("unterminated JSON string field {key:?}"));
    &body[..end]
}

fn json_u64(json: &str, key: &str) -> u64 {
    let needle = format!("\"{key}\"");
    let key_at = json
        .find(&needle)
        .unwrap_or_else(|| panic!("missing JSON field {key:?}"));
    let after_key = &json[key_at + needle.len()..];
    let colon = after_key
        .find(':')
        .unwrap_or_else(|| panic!("missing colon after JSON field {key:?}"));
    let value = after_key[colon + 1..].trim_start();
    let end = value
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(value.len());
    value[..end]
        .parse()
        .unwrap_or_else(|e| panic!("invalid numeric JSON field {key:?}: {e}"))
}

/// Collect repeated primitive fields from the generator-owned artifact.  The
/// frame inventory intentionally contains only unescaped names, decimal type
/// tags, and hex strings, so this small parser stays auditable and independent
/// from the production JSON/wire implementation.
fn json_all_strings(json: &str, key: &str) -> Vec<String> {
    let needle = format!("\"{key}\"");
    let mut values = Vec::new();
    let mut cursor = 0;
    while let Some(relative) = json[cursor..].find(&needle) {
        let key_at = cursor + relative;
        let after_key = &json[key_at + needle.len()..];
        let colon = after_key
            .find(':')
            .unwrap_or_else(|| panic!("missing colon after JSON field {key:?}"));
        let value = after_key[colon + 1..].trim_start();
        assert!(value.starts_with('"'), "JSON field {key:?} is not a string");
        let body = &value[1..];
        let end = body
            .find('"')
            .unwrap_or_else(|| panic!("unterminated JSON string field {key:?}"));
        values.push(body[..end].to_owned());
        cursor = key_at + needle.len();
    }
    values
}

fn json_all_u64(json: &str, key: &str) -> Vec<u64> {
    let needle = format!("\"{key}\"");
    let mut values = Vec::new();
    let mut cursor = 0;
    while let Some(relative) = json[cursor..].find(&needle) {
        let key_at = cursor + relative;
        let after_key = &json[key_at + needle.len()..];
        let colon = after_key
            .find(':')
            .unwrap_or_else(|| panic!("missing colon after JSON field {key:?}"));
        let value = after_key[colon + 1..].trim_start();
        let end = value
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(value.len());
        values.push(
            value[..end]
                .parse()
                .unwrap_or_else(|e| panic!("invalid numeric JSON field {key:?}: {e}")),
        );
        cursor = key_at + needle.len();
    }
    values
}

fn decode_hex(field: &str, value: &str) -> Vec<u8> {
    assert!(value.len().is_multiple_of(2), "{field} hex value has odd length");
    let mut out = Vec::with_capacity(value.len() / 2);
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let hi = hex_nibble(pair[0]).unwrap_or_else(|| panic!("invalid {field} hex at byte {index}"));
        let lo = hex_nibble(pair[1]).unwrap_or_else(|| panic!("invalid {field} hex at byte {index}"));
        out.push((hi << 4) | lo);
    }
    // Empty payloads are valid for the CLOSE frame; all other empty fields in
    // the current artifacts remain a likely generator error.
    assert!(!value.is_empty() || field == "empty" || field == "payload", "empty hex field {field}");
    out
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn parse_hex(json: &str, key: &str) -> Vec<u8> {
    let value = json_string(json, key);
    decode_hex(key, value)
}

/// Parse the envelope format from doc 10 §2 independently:
/// `[payload_len: u16 little endian][type: u8][payload]`.
fn read_envelope_independent(wire: &[u8]) -> Result<(u8, &[u8]), &'static str> {
    if wire.len() < 3 {
        return Err("truncated envelope header");
    }
    let payload_len = u16::from_le_bytes([wire[0], wire[1]]) as usize;
    let total = 3usize
        .checked_add(payload_len)
        .ok_or("envelope length overflow")?;
    if wire.len() != total {
        return Err("envelope has a truncated or trailing byte");
    }
    Ok((wire[2], &wire[3..]))
}

/// Read the fixed part of a bundle and verify its canonical layout without
/// constructing an `mmp_types::Bundle`.
fn read_bundle_independent(wire: &[u8]) -> Result<BundleView<'_>, &'static str> {
    const SIG_LEN: usize = 64;
    const FIXED_HEADER_LEN: usize = 58;
    if wire.len() < FIXED_HEADER_LEN + 1 + SIG_LEN {
        return Err("bundle shorter than fixed header, payload length, and signature");
    }
    let ver_class = wire[0];
    if ver_class >> 4 != 1 {
        return Err("unsupported bundle wire version");
    }
    if wire[1] & !0b111 != 0 {
        return Err("reserved bundle flags are set");
    }
    if wire[55] > 3 {
        return Err("bundle priority exceeds the v1 range");
    }
    let ext_len = u16::from_le_bytes([wire[56], wire[57]]) as usize;
    let ext_end = FIXED_HEADER_LEN
        .checked_add(ext_len)
        .ok_or("extension length overflow")?;
    let unsigned_len = wire.len() - SIG_LEN;
    if ext_end + 2 > unsigned_len {
        return Err("extension bytes overlap payload or signature");
    }
    let payload_len = u16::from_le_bytes([wire[ext_end], wire[ext_end + 1]]) as usize;
    let payload_len_bytes = 2;
    let payload_start = ext_end + payload_len_bytes;
    let payload_end = payload_start
        .checked_add(payload_len)
        .ok_or("payload length overflow")?;
    if payload_end != wire.len() - SIG_LEN {
        return Err("payload length does not consume the unsigned bundle");
    }
    let mut canonical = wire[..wire.len() - SIG_LEN].to_vec();
    canonical[50..55].fill(0);
    Ok(BundleView {
        wire,
        canonical,
        class: ver_class & 0x0f,
        flags: wire[1],
        lifetime_ms: decode_u40(&wire[50..55]),
        origin: &wire[2..34],
        dest: &wire[34..50],
        priority: wire[55],
        extensions: &wire[FIXED_HEADER_LEN..ext_end],
        payload: &wire[payload_start..payload_end],
        signature: &wire[wire.len() - SIG_LEN..],
    })
}

struct BundleView<'a> {
    wire: &'a [u8],
    canonical: Vec<u8>,
    class: u8,
    flags: u8,
    lifetime_ms: u64,
    origin: &'a [u8],
    dest: &'a [u8],
    priority: u8,
    extensions: &'a [u8],
    payload: &'a [u8],
    signature: &'a [u8],
}

fn decode_u40(bytes: &[u8]) -> u64 {
    assert_eq!(bytes.len(), 5);
    bytes
        .iter()
        .enumerate()
        .fold(0u64, |value, (shift, byte)| value | u64::from(*byte) << (shift * 8))
}

#[test]
fn committed_vector_inventory_is_explicit_and_complete() {
    let mut actual: Vec<String> = fs::read_dir(vectors_dir())
        .unwrap_or_else(|e| panic!("cannot enumerate vectors directory: {e}"))
        .map(|entry| entry.expect("vector directory entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    actual.sort();
    let mut expected = VECTOR_NAMES.iter().map(|name| (*name).to_owned()).collect::<Vec<_>>();
    expected.sort();
    assert_eq!(actual, expected, "update VECTOR_NAMES when a vector is added/removed");
}

#[test]
fn envelope_vector_is_accepted_by_independent_reader() {
    let json = vector("envelope.json");
    let wire = parse_hex(&json, "wire");
    let payload = parse_hex(&json, "payload");
    let frame_type = json_u64(&json, "frame_type");
    let (actual_type, actual_payload) = read_envelope_independent(&wire).expect("valid envelope");
    assert_eq!(u64::from(actual_type), frame_type);
    assert_eq!(actual_payload, payload);
    assert_eq!(wire.len(), 3 + payload.len());
    assert_eq!(u16::from_le_bytes([wire[0], wire[1]]) as usize, payload.len());
}

#[test]
fn hex_decoder_uses_the_value_not_the_field_name() {
    assert_eq!(decode_hex("wire", "aAbB"), [0xaa, 0xbb]);
}

#[test]
fn envelope_reader_rejects_truncation_and_trailing_bytes() {
    let json = vector("envelope.json");
    let wire = parse_hex(&json, "wire");
    assert_eq!(read_envelope_independent(&wire[..2]), Err("truncated envelope header"));
    assert_eq!(
        read_envelope_independent(&wire[..wire.len() - 1]),
        Err("envelope has a truncated or trailing byte")
    );
    let mut trailing = wire.clone();
    trailing.push(0);
    assert_eq!(
        read_envelope_independent(&trailing),
        Err("envelope has a truncated or trailing byte")
    );
}

#[test]
fn hello_vector_is_accepted_by_independent_reader() {
    let json = vector("hello.json");
    let wire = parse_hex(&json, "wire");
    let suites = parse_hex(&json, "suites");
    let expected_ver = json_u64(&json, "ver") as u8;
    let expected_tier = json_u64(&json, "tier") as u8;
    let expected_flags = json_u64(&json, "flags") as u16;
    let expected_credits = json_u64(&json, "credits") as u8;
    let expected_psm = json_u64(&json, "psm") as u16;

    assert!(wire.len() >= 1 + 2 + 1 + 2 + 1 + 2);
    assert_eq!(wire[0], expected_ver);
    let suite_len = u16::from_le_bytes([wire[1], wire[2]]) as usize;
    assert_eq!(suite_len, suites.len());
    assert_eq!(&wire[3..3 + suite_len], suites.as_slice());
    let tier_offset = 3 + suite_len;
    assert_eq!(wire[tier_offset], expected_tier);
    assert_eq!(u16::from_le_bytes([wire[tier_offset + 1], wire[tier_offset + 2]]), expected_flags);
    assert_eq!(wire[tier_offset + 3], expected_credits);
    assert_eq!(
        u16::from_le_bytes([wire[tier_offset + 4], wire[tier_offset + 5]]),
        expected_psm
    );
    assert_eq!(wire.len(), tier_offset + 6, "HELLO has trailing bytes");
}

#[test]
fn bundle_vector_is_independently_canonical_and_content_addressed() {
    let json = vector("bundle.json");
    let wire = parse_hex(&json, "wire");
    let expected_canonical = parse_hex(&json, "canonical_zeroed");
    let expected_id = parse_hex(&json, "bundle_id");
    let expected_origin = parse_hex(&json, "origin_pubkey");
    let expected_signature = parse_hex(&json, "signature");
    let expected_lifetime = json_u64(&json, "lifetime_ms");
    let view = read_bundle_independent(&wire).expect("valid bundle");
    assert_eq!(view.wire, wire);
    assert_eq!(view.canonical, expected_canonical);
    assert_eq!(view.origin, expected_origin);
    assert_eq!(view.signature, expected_signature);
    assert_eq!(view.lifetime_ms, expected_lifetime);
    let digest = blake3::hash(&view.canonical);
    assert_eq!(&digest.as_bytes()[..16], expected_id);
}

#[test]
fn bundle_reader_rejects_tampered_layout_without_production_decoder() {
    let json = vector("bundle.json");
    let wire = parse_hex(&json, "wire");
    let mut reserved = wire.clone();
    reserved[1] |= 0x80;
    assert!(matches!(
        read_bundle_independent(&reserved),
        Err("reserved bundle flags are set")
    ));

    let mut bad_priority = wire.clone();
    bad_priority[55] = 4;
    assert!(matches!(
        read_bundle_independent(&bad_priority),
        Err("bundle priority exceeds the v1 range")
    ));

    let mut bad_payload_length = wire.clone();
    let ext_len = u16::from_le_bytes([bad_payload_length[56], bad_payload_length[57]]) as usize;
    let payload_len_offset = 58 + ext_len;
    let old_payload_len = u16::from_le_bytes([
        bad_payload_length[payload_len_offset],
        bad_payload_length[payload_len_offset + 1],
    ]);
    bad_payload_length[payload_len_offset..payload_len_offset + 2]
        .copy_from_slice(&old_payload_len.wrapping_add(1).to_le_bytes());
    assert!(matches!(
        read_bundle_independent(&bad_payload_length),
        Err("payload length does not consume the unsigned bundle")
    ));
}

#[test]
fn noncanonical_s_vector_pins_little_endian_s_plus_l() {
    let json = vector("noncanonical_s_negative.json");
    let valid_signature = parse_hex(&json, "valid_signature");
    let noncanonical_signature = parse_hex(&json, "noncanonical_signature");
    let valid_s = parse_hex(&json, "valid_s");
    let noncanonical_s = parse_hex(&json, "noncanonical_s");
    assert_eq!(valid_signature.len(), 64);
    assert_eq!(noncanonical_signature.len(), 64);
    assert_eq!(valid_s.len(), 32);
    assert_eq!(noncanonical_s.len(), 32);
    assert_eq!(&valid_signature[32..], valid_s.as_slice());
    assert_eq!(&noncanonical_signature[32..], noncanonical_s.as_slice());
    assert_eq!(&valid_signature[..32], &noncanonical_signature[..32], "S+L must preserve R");
    assert_eq!(json_string(&json, "expected_valid"), "accept");
    assert_eq!(json_string(&json, "expected_noncanonical"), "reject");

    // Recompute S+L independently from the artifact.  This is deliberately
    // a tiny byte-level check rather than a call to the production verifier.
    const L_LE: [u8; 32] = [
        0xed, 0xd3, 0xf5, 0x5c, 0x1a, 0x63, 0x12, 0x58,
        0xd6, 0x9c, 0xf7, 0xa2, 0xde, 0xf9, 0xde, 0x14,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10,
    ];
    let mut carry = 0u16;
    for index in 0..32 {
        let sum = valid_s[index] as u16 + L_LE[index] as u16 + carry;
        assert_eq!(sum as u8, noncanonical_s[index], "S+L differs at byte {index}");
        carry = sum >> 8;
    }
    assert_eq!(carry, 0, "S+L must fit in the 256-bit signature scalar");
}

#[test]
fn grouped_bundle_vector_covers_classes_zero_through_five() {
    let json = vector("bundles.json");
    let classes = json_all_u64(&json, "class");
    let names = json_all_strings(&json, "name");
    let statuses = json_all_strings(&json, "status");
    let origins = json_all_strings(&json, "origin_pubkey");
    let dests = json_all_strings(&json, "dest");
    let payloads = json_all_strings(&json, "payload");
    let canonicals = json_all_strings(&json, "canonical_zeroed");
    let wires = json_all_strings(&json, "wire");
    let ids = json_all_strings(&json, "bundle_id");
    let signatures = json_all_strings(&json, "signature");
    let payload_lengths = json_all_u64(&json, "payload_len");
    assert_eq!(json_u64(&json, "bundle_count"), 6);
    assert_eq!(classes, vec![0, 1, 2, 3, 4, 5]);
    assert_eq!(names.len(), 6);
    assert_eq!(statuses.len(), 6);
    assert_eq!(origins.len(), 6);
    assert_eq!(dests.len(), 6);
    assert_eq!(payloads.len(), 6);
    assert_eq!(canonicals.len(), 6);
    assert_eq!(wires.len(), 6);
    assert_eq!(ids.len(), 6);
    assert_eq!(signatures.len(), 6);
    assert_eq!(payload_lengths.len(), 6);

    // The metadata labels the two known spec/implementation boundaries rather
    // than presenting them as frozen-wire facts.
    assert_eq!(statuses[4], "implementation_extension");
    assert_eq!(statuses[5], "implementation_extension_spec_gap");

    for index in 0..6 {
        let wire = decode_hex("wire", &wires[index]);
        let payload = decode_hex("payload", &payloads[index]);
        let expected_canonical = decode_hex("canonical_zeroed", &canonicals[index]);
        let expected_id = decode_hex("bundle_id", &ids[index]);
        let expected_origin = decode_hex("origin_pubkey", &origins[index]);
        let expected_dest = decode_hex("dest", &dests[index]);
        let expected_signature = decode_hex("signature", &signatures[index]);
        assert_eq!(payload.len(), payload_lengths[index] as usize, "{} payload length", names[index]);
        let view = read_bundle_independent(&wire).expect("grouped bundle must satisfy independent layout reader");
        assert_eq!(u64::from(view.class), classes[index], "{} class", names[index]);
        assert_eq!(view.payload, payload, "{} payload bytes", names[index]);
        assert_eq!(view.canonical, expected_canonical, "{} canonical bytes", names[index]);
        assert_eq!(view.origin, expected_origin, "{} origin binding", names[index]);
        assert_eq!(view.dest, expected_dest, "{} destination", names[index]);
        assert_eq!(view.signature, expected_signature, "{} outer signature", names[index]);
        assert_eq!(view.flags & !0b111, 0, "{} reserved flags", names[index]);
        assert!(view.priority <= 3, "{} priority", names[index]);
        assert_eq!(view.extensions.len(), u16::from_le_bytes([wire[56], wire[57]]) as usize);
        assert_eq!(view.canonical.len(), wire.len() - 64, "{} canonical size", names[index]);
        let digest = blake3::hash(&view.canonical);
        assert_eq!(&digest.as_bytes()[..16], expected_id, "{} content address", names[index]);
    }
}

#[test]
fn cross_domain_matrix_has_all_registered_pairs_and_expected_results() {
    let json = vector("cross_domain_matrix.json");
    let names = json_all_strings(&json, "object_domain");
    let tags = json_all_strings(&json, "domain_tag");
    let pubkeys = json_all_strings(&json, "pubkey");
    let objects = json_all_strings(&json, "object");
    let signatures = json_all_strings(&json, "signature");
    let verify_domains = json_all_strings(&json, "verify_domain");
    let results = json_all_strings(&json, "result");
    let expected_names = [
        "mmp1/bundle",
        "mmp1/claim",
        "mmp1/friend",
        "mmp1/prekey",
        "mmp1/blobroot",
        "mmp1/chanbind",
        "mmp1/manifest",
        "mmp1/authcert",
        "mmp1/announce",
    ];
    let expected_tags: [&[u8]; 9] = [
        b"mmp1/bundle\x01",
        b"mmp1/claim\x01",
        b"mmp1/friend\x01",
        b"mmp1/prekey\x01",
        b"mmp1/blobroot\x01",
        b"mmp1/chanbind\x01",
        b"mmp1/manifest\x01",
        b"mmp1/authcert\x01",
        b"mmp1/announce\x01",
    ];
    assert_eq!(json_u64(&json, "domain_count"), 9);
    assert_eq!(json_u64(&json, "matrix_cells"), 81);
    assert_eq!(json_u64(&json, "expected_accepts"), 9);
    assert_eq!(json_u64(&json, "expected_rejects"), 72);
    assert_eq!(names, expected_names);
    assert_eq!(tags.len(), 9);
    assert_eq!(pubkeys.len(), 9);
    assert_eq!(objects.len(), 9);
    assert_eq!(signatures.len(), 9);
    assert_eq!(verify_domains.len(), 81);
    assert_eq!(results.len(), 81);

    for row in 0..9 {
        assert_eq!(decode_hex("domain_tag", &tags[row]), expected_tags[row]);
        assert_eq!(decode_hex("pubkey", &pubkeys[row]).len(), 32);
        assert!(!decode_hex("object", &objects[row]).is_empty());
        assert_eq!(decode_hex("signature", &signatures[row]).len(), 64);
        for (column, expected_name) in expected_names.iter().enumerate() {
            let index = row * 9 + column;
            assert_eq!(verify_domains[index], *expected_name);
            let expected = if row == column { "accept" } else { "reject" };
            assert_eq!(results[index], expected, "matrix cell ({row},{column})");
        }
    }
    assert_eq!(results.iter().filter(|value| value.as_str() == "accept").count(), 9);
    assert_eq!(results.iter().filter(|value| value.as_str() == "reject").count(), 72);
}

#[test]
fn frame_inventory_has_all_registered_types_and_valid_envelopes() {
    let json = vector("frames.json");
    let count = json_u64(&json, "frame_count");
    assert_eq!(count, 17);

    let names = json_all_strings(&json, "name");
    let types = json_all_u64(&json, "type");
    let payloads = json_all_strings(&json, "payload");
    let wires = json_all_strings(&json, "wire");
    assert_eq!(names.len(), count as usize);
    assert_eq!(types.len(), count as usize);
    assert_eq!(payloads.len(), count as usize);
    assert_eq!(wires.len(), count as usize);

    let expected_types = [
        mmp_types::frame::FT_HS,
        mmp_types::frame::FT_ID_PROOF,
        mmp_types::frame::FT_CREDIT,
        mmp_types::frame::FT_SUMMARY,
        mmp_types::frame::FT_RECON_INIT,
        mmp_types::frame::FT_RECON_RANGE,
        mmp_types::frame::FT_RECON_IDS,
        mmp_types::frame::FT_WANT,
        mmp_types::frame::FT_BUNDLE,
        mmp_types::frame::FT_BUDGET,
        mmp_types::frame::FT_BLOB_REQ,
        mmp_types::frame::FT_BLOB_HAVE,
        mmp_types::frame::FT_FRIEND_OFFER,
        mmp_types::frame::FT_FRIEND_ACK,
        mmp_types::frame::FT_PROXY_SUMMARY,
        mmp_types::frame::FT_ERROR,
        mmp_types::frame::FT_CLOSE,
    ];
    let mut seen = [false; 256];
    for (index, expected_type) in expected_types.iter().enumerate() {
        assert_eq!(types[index], u64::from(*expected_type), "frame {} tag drifted", names[index]);
        assert!(!seen[*expected_type as usize], "duplicate frame type {}", expected_type);
        seen[*expected_type as usize] = true;

        let payload = decode_hex("payload", &payloads[index]);
        let wire = decode_hex("wire", &wires[index]);
        assert_eq!(wire.len(), payload.len() + 3, "frame {} envelope length mismatch", names[index]);
        assert_eq!(u16::from_le_bytes([wire[0], wire[1]]) as usize, payload.len());
        assert_eq!(wire[2], *expected_type, "frame {} envelope tag mismatch", names[index]);
        assert_eq!(&wire[3..], payload.as_slice(), "frame {} payload mismatch", names[index]);
        assert_eq!(read_envelope_independent(&wire).unwrap(), (*expected_type, payload.as_slice()));
    }
}
