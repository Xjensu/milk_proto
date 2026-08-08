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
    "claim.json",
    "cross_domain_negative.json",
    "envelope.json",
    "lifetime_reset_negative.json",
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

fn decode_hex(field: &str, value: &str) -> Vec<u8> {
    assert!(value.len().is_multiple_of(2), "{field} hex value has odd length");
    let mut out = Vec::with_capacity(value.len() / 2);
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let hi = hex_nibble(pair[0]).unwrap_or_else(|| panic!("invalid {field} hex at byte {index}"));
        let lo = hex_nibble(pair[1]).unwrap_or_else(|| panic!("invalid {field} hex at byte {index}"));
        out.push((hi << 4) | lo);
    }
    // Keep this assertion close to the conversion call sites: it catches a
    // malformed field before any wire-level interpretation is attempted.
    assert!(!value.is_empty() || field == "empty", "empty hex field {field}");
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
        lifetime_ms: decode_u40(&wire[50..55]),
        origin: &wire[2..34],
        signature: &wire[wire.len() - SIG_LEN..],
    })
}

struct BundleView<'a> {
    wire: &'a [u8],
    canonical: Vec<u8>,
    lifetime_ms: u64,
    origin: &'a [u8],
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
