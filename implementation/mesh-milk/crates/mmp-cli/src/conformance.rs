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
    "coverage.json",
    "cross_domain_negative.json",
    "cross_domain_matrix.json",
    "envelope.json",
    "envelope_oversize_negative.json",
    "bundle_tlv_negative.json",
    "frames.json",
    "hello.json",
    "lifetime_reset_negative.json",
    "noncanonical_s_negative.json",
    "reserved_bits_negative.json",
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

fn json_bool(json: &str, key: &str) -> bool {
    let needle = format!("\"{key}\"");
    let key_at = json
        .find(&needle)
        .unwrap_or_else(|| panic!("missing JSON field {key:?}"));
    let after_key = &json[key_at + needle.len()..];
    let colon = after_key
        .find(':')
        .unwrap_or_else(|| panic!("missing colon after JSON field {key:?}"));
    let value = after_key[colon + 1..].trim_start();
    if value.starts_with("true") {
        true
    } else if value.starts_with("false") {
        false
    } else {
        panic!("JSON field {key:?} is not a boolean");
    }
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

/// Collect strings from one JSON array field in the deliberately small
/// generator-owned format.  This is intentionally limited to unescaped
/// strings and stops at that array's closing bracket, so later fields cannot
/// accidentally become part of the RBSR input.
fn json_array_strings(json: &str, key: &str) -> Vec<String> {
    let needle = format!("\"{key}\"");
    let key_at = json
        .find(&needle)
        .unwrap_or_else(|| panic!("missing JSON field {key:?}"));
    let after_key = &json[key_at + needle.len()..];
    let colon = after_key
        .find(':')
        .unwrap_or_else(|| panic!("missing colon after JSON field {key:?}"));
    let value = after_key[colon + 1..].trim_start();
    assert!(value.starts_with('['), "JSON field {key:?} is not an array");
    let end = value
        .find(']')
        .unwrap_or_else(|| panic!("unterminated JSON array field {key:?}"));
    let array = &value[1..end];
    let mut values = Vec::new();
    let mut cursor = 0;
    while let Some(relative) = array[cursor..].find('"') {
        let start = cursor + relative + 1;
        let body = &array[start..];
        let close = body
            .find('"')
            .unwrap_or_else(|| panic!("unterminated JSON array string {key:?}"));
        values.push(body[..close].to_owned());
        cursor = start + close + 1;
    }
    values
}

fn decode_hex(field: &str, value: &str) -> Vec<u8> {
    assert!(
        value.len().is_multiple_of(2),
        "{field} hex value has odd length"
    );
    let mut out = Vec::with_capacity(value.len() / 2);
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let hi =
            hex_nibble(pair[0]).unwrap_or_else(|| panic!("invalid {field} hex at byte {index}"));
        let lo =
            hex_nibble(pair[1]).unwrap_or_else(|| panic!("invalid {field} hex at byte {index}"));
        out.push((hi << 4) | lo);
    }
    // Empty payloads are valid for the CLOSE frame; all other empty fields in
    // the current artifacts remain a likely generator error.
    assert!(
        !value.is_empty() || field == "empty" || field == "payload",
        "empty hex field {field}"
    );
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

/// Apply the frozen doc-10 logical frame cap to the independent envelope
/// reader.  The production reader intentionally has a separate +32-byte
/// receive allowance for encrypted envelopes; keeping this check here makes
/// that implementation policy visible instead of silently redefining the
/// normative 4096-byte payload bound.
fn read_envelope_normative(wire: &[u8]) -> Result<(u8, &[u8]), &'static str> {
    let (typ, payload) = read_envelope_independent(wire)?;
    if payload.len() > 4096 {
        return Err("payload exceeds doc 10 logical cap");
    }
    Ok((typ, payload))
}

/// A tiny fixed-size reader used only by the conformance tests below.  It is
/// intentionally separate from `mmp_types::wire::Reader`: these checks must
/// remain useful if the production decoder and the vector generator drift in
/// the same way.
struct PayloadReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> PayloadReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], &'static str> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or("payload length overflow")?;
        if end > self.bytes.len() {
            return Err("payload truncated");
        }
        let value = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(value)
    }

    fn u8(&mut self) -> Result<u8, &'static str> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, &'static str> {
        let bytes = self.take(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn u32(&mut self) -> Result<u32, &'static str> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn u64(&mut self) -> Result<u64, &'static str> {
        let bytes = self.take(8)?;
        Ok(u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    fn varbytes(&mut self, max: usize) -> Result<&'a [u8], &'static str> {
        let length = self.u16()? as usize;
        if length > max {
            return Err("variable field exceeds bound");
        }
        self.take(length)
    }

    fn finish(self) -> Result<(), &'static str> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err("payload has trailing bytes")
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct BeaconView<'a> {
    ver: u8,
    tier: u8,
    battery: u8,
    ephem_id: &'a [u8],
    busy: bool,
    epoch: u8,
    frag_structure: u8,
    frag_offset: u16,
    fragment: &'a [u8],
}

/// Parse the fixed 24-byte beacon layout from doc 10 §5 without constructing
/// an `mmp_types::Beacon`.  Every bit in the two packed bytes is accounted for
/// by the schema, so there is no permissive/reserved-bit path here.
fn read_beacon_independent(bytes: &[u8]) -> Result<BeaconView<'_>, &'static str> {
    if bytes.len() < 24 {
        return Err("beacon truncated");
    }
    if bytes.len() > 24 {
        return Err("beacon has trailing bytes");
    }
    let ver_role = bytes[0];
    let ver = ver_role >> 4;
    if ver != 1 {
        return Err("beacon version is not 1");
    }
    let frag = u16::from_le_bytes([bytes[10], bytes[11]]);
    Ok(BeaconView {
        ver,
        tier: (ver_role >> 2) & 0x03,
        battery: ver_role & 0x03,
        ephem_id: &bytes[1..9],
        busy: bytes[9] & 0x80 != 0,
        epoch: bytes[9] & 0x7f,
        frag_structure: (frag >> 14) as u8,
        frag_offset: frag & 0x3fff,
        fragment: &bytes[12..24],
    })
}

fn read_id_count(reader: &mut PayloadReader<'_>, count: usize) -> Result<(), &'static str> {
    let bytes = count.checked_mul(16).ok_or("id list length overflow")?;
    reader.take(bytes).map(|_| ())
}

fn read_range(reader: &mut PayloadReader<'_>) -> Result<(), &'static str> {
    // Range = lo id16, hi id16, count u32, fingerprint u64.
    reader.take(32)?;
    reader.u32()?;
    reader.u64()?;
    Ok(())
}

/// Validate one registered frame payload using only doc 10 field widths and
/// little-endian length rules.  HS is intentionally opaque (Noise owns its
/// inner grammar); BUNDLE delegates only to this module's independent outer
/// bundle reader.  No production frame/payload decoder is called here.
fn read_frame_payload_independent(typ: u8, payload: &[u8]) -> Result<(), &'static str> {
    if payload.len() > 4096 {
        return Err("frame payload exceeds normative cap");
    }
    let mut reader = PayloadReader::new(payload);
    match typ {
        mmp_types::frame::FT_HS => {
            // Noise handshake messages are opaque to the frame layer.  The
            // envelope test still bounds them; their cryptographic grammar is
            // covered by the Noise implementation, not this reader.
            return Ok(());
        }
        mmp_types::frame::FT_ID_PROOF => {
            reader.take(32)?;
            reader.take(64)?;
        }
        mmp_types::frame::FT_CREDIT => {
            reader.u8()?;
        }
        mmp_types::frame::FT_SUMMARY => {
            let bucket_count = reader.u8()? as usize;
            for _ in 0..bucket_count {
                reader.u16()?;
                reader.varbytes(4096)?;
            }
            let claim_count = reader.u8()? as usize;
            for _ in 0..claim_count {
                reader.take(4)?;
                reader.u8()?;
                reader.u32()?;
            }
            let group_count = reader.u8()? as usize;
            read_id_count(&mut reader, group_count)?;
            let heads_count = reader.u8()? as usize;
            for _ in 0..heads_count {
                reader.take(16)?;
                let head_count = reader.u8()? as usize;
                read_id_count(&mut reader, head_count)?;
            }
        }
        mmp_types::frame::FT_RECON_INIT => {
            reader.u8()?;
            reader.u8()?;
            read_range(&mut reader)?;
        }
        mmp_types::frame::FT_RECON_RANGE => {
            reader.u8()?;
            let range_count = reader.u8()? as usize;
            for _ in 0..range_count {
                read_range(&mut reader)?;
            }
        }
        mmp_types::frame::FT_RECON_IDS => {
            reader.u8()?;
            reader.take(16)?;
            reader.take(16)?;
            let id_count = reader.u16()? as usize;
            read_id_count(&mut reader, id_count)?;
        }
        mmp_types::frame::FT_WANT => {
            let id_count = reader.u16()? as usize;
            read_id_count(&mut reader, id_count)?;
        }
        mmp_types::frame::FT_BUNDLE => {
            read_bundle_independent(payload)?;
            return Ok(());
        }
        mmp_types::frame::FT_BUDGET => {
            reader.take(16)?;
            reader.u8()?;
        }
        mmp_types::frame::FT_BLOB_REQ | mmp_types::frame::FT_BLOB_HAVE => {
            reader.take(16)?;
            reader.u16()?;
            reader.varbytes(4096)?;
        }
        mmp_types::frame::FT_FRIEND_OFFER => {
            reader.u64()?;
        }
        mmp_types::frame::FT_FRIEND_ACK => {
            reader.take(64)?;
        }
        mmp_types::frame::FT_PROXY_SUMMARY => {
            reader.take(16)?;
            reader.varbytes(4096)?;
        }
        mmp_types::frame::FT_ERROR => {
            reader.u8()?;
            reader.varbytes(256)?;
        }
        mmp_types::frame::FT_CLOSE => {}
        _ => return Err("unknown frame type"),
    }
    reader.finish()
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

/// Validate only the canonical ordering and critical-type rules for a raw
/// extension run.  This deliberately does not call `mmp-types::tlv` (or any
/// production decoder): the negative vectors must remain useful if both the
/// generator and that decoder drift together.
fn validate_tlvs_independent(bytes: &[u8]) -> Result<(), &'static str> {
    let mut cursor = 0usize;
    let mut previous: Option<u8> = None;
    while cursor < bytes.len() {
        let typ = *bytes.get(cursor).ok_or("truncated TLV type")?;
        cursor += 1;
        if previous.is_some_and(|old| typ <= old) {
            return Err("TLV order/duplicate");
        }
        previous = Some(typ);
        let len_bytes = bytes
            .get(cursor..cursor + 2)
            .ok_or("truncated TLV length")?;
        let len = u16::from_le_bytes([len_bytes[0], len_bytes[1]]) as usize;
        cursor += 2;
        let value = bytes
            .get(cursor..cursor + len)
            .ok_or("truncated TLV value")?;
        cursor += len;
        // The critical registry has fixed (or bounded, for PARENTS) value
        // widths.  Checking only the type/order here would leave a malformed
        // known TLV looking valid to the independent reader even though the
        // production codec rejects it.  Keep these checks specification-
        // derived and byte-level so the conformance reader remains useful if
        // both production generation and decoding drift together.
        match typ {
            0x01 if value.is_empty() || !value.len().is_multiple_of(16) || value.len() > 64 => {
                return Err("known TLV has invalid length");
            }
            0x02 if value.len() != 2 => return Err("known TLV has invalid length"),
            0x03 if value.len() != 22 => return Err("known TLV has invalid length"),
            _ => {}
        }
        // The frozen registry currently occupies critical types 0x01–0x03;
        // any other type below 0x80 is unknown-critical and rejects the run.
        if typ < 0x80 && !(1..=3).contains(&typ) {
            return Err("unknown critical TLV");
        }
    }
    Ok(())
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
    bytes.iter().enumerate().fold(0u64, |value, (shift, byte)| {
        value | u64::from(*byte) << (shift * 8)
    })
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
    let mut expected = VECTOR_NAMES
        .iter()
        .map(|name| (*name).to_owned())
        .collect::<Vec<_>>();
    expected.sort();
    assert_eq!(
        actual, expected,
        "update VECTOR_NAMES when a vector is added/removed"
    );
}

#[test]
fn coverage_manifest_maps_every_committed_vector() {
    let json = vector("coverage.json");
    assert_eq!(json_string(&json, "schema"), "mmp-e5-s4-coverage-v1");
    assert_eq!(json_u64(&json, "entry_count"), 38);

    let ids = json_all_strings(&json, "id");
    let artifacts = json_all_strings(&json, "artifact");
    let sources = json_all_strings(&json, "source");
    let evidence = json_all_strings(&json, "evidence");
    assert_eq!(ids.len(), 38);
    assert_eq!(artifacts.len(), 38);
    assert_eq!(sources.len(), 38);
    assert_eq!(evidence.len(), 38);
    assert!(sources
        .iter()
        .all(|source| source.starts_with("specification/")));
    assert!(evidence.iter().all(|entry| !entry.is_empty()));

    let mut actual = artifacts;
    actual.sort();
    actual.dedup();
    let mut expected = VECTOR_NAMES
        .iter()
        .map(|name| (*name).to_owned())
        .collect::<Vec<_>>();
    expected.sort();
    assert_eq!(
        actual, expected,
        "coverage manifest must map every committed vector"
    );

    let mut unique_ids = ids.clone();
    unique_ids.sort();
    unique_ids.dedup();
    assert_eq!(unique_ids.len(), ids.len(), "coverage IDs must be unique");
}

#[test]
fn coverage_manifest_lists_each_registered_frame_requirement() {
    let json = vector("coverage.json");
    let ids = json_all_strings(&json, "id");
    let artifacts = json_all_strings(&json, "artifact");
    assert_eq!(ids.len(), artifacts.len());
    let required = [
        "frame.type.0x01.hs.v1",
        "frame.type.0x02.id-proof.v1",
        "frame.type.0x03.credit.v1",
        "frame.type.0x10.summary.v1",
        "frame.type.0x11.recon-init.v1",
        "frame.type.0x12.recon-range.v1",
        "frame.type.0x13.recon-ids.v1",
        "frame.type.0x14.want.v1",
        "frame.type.0x20.bundle.v1",
        "frame.type.0x21.budget.v1",
        "frame.type.0x30.blob-req.v1",
        "frame.type.0x31.blob-have.v1",
        "frame.type.0x40.friend-offer.v1",
        "frame.type.0x41.friend-ack.v1",
        "frame.type.0x42.proxy-summary.v1",
        "frame.type.0x7e.error.v1",
        "frame.type.0x7f.close.v1",
    ];
    for requirement in required {
        assert!(
            ids.iter().any(|id| id == requirement),
            "coverage manifest is missing {requirement}"
        );
    }
    for (id, artifact) in ids.iter().zip(&artifacts) {
        if id.starts_with("frame.type.") {
            assert_eq!(
                artifact, "frames.json",
                "{id} must consume the complete frame inventory"
            );
        }
    }
    assert_eq!(
        ids.iter()
            .filter(|id| id.starts_with("frame.type."))
            .count(),
        required.len(),
        "frame requirement IDs must be one-to-one with the registered frame table"
    );
}

#[test]
fn coverage_manifest_lists_required_canonical_negative_cases() {
    let json = vector("coverage.json");
    let ids = json_all_strings(&json, "id");
    let artifacts = json_all_strings(&json, "artifact");
    assert_eq!(ids.len(), artifacts.len());
    for (requirement, artifact) in [
        (
            "negative.bundle.reserved-bits.v1",
            "reserved_bits_negative.json",
        ),
        (
            "negative.tlv.unknown-critical.v1",
            "bundle_tlv_negative.json",
        ),
        ("negative.tlv.out-of-order.v1", "bundle_tlv_negative.json"),
        ("negative.tlv.duplicate.v1", "bundle_tlv_negative.json"),
    ] {
        let index = ids
            .iter()
            .position(|id| id == requirement)
            .unwrap_or_else(|| panic!("coverage manifest is missing {requirement}"));
        assert_eq!(
            artifacts[index], artifact,
            "{requirement} must map to {artifact}"
        );
    }
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
    assert_eq!(
        u16::from_le_bytes([wire[0], wire[1]]) as usize,
        payload.len()
    );
}

#[test]
fn hex_decoder_uses_the_value_not_the_field_name() {
    assert_eq!(decode_hex("wire", "aAbB"), [0xaa, 0xbb]);
}

#[test]
fn envelope_reader_rejects_truncation_and_trailing_bytes() {
    let json = vector("envelope.json");
    let wire = parse_hex(&json, "wire");
    assert_eq!(
        read_envelope_independent(&wire[..2]),
        Err("truncated envelope header")
    );
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
fn oversized_envelope_vector_records_normative_and_runtime_cap() {
    let json = vector("envelope_oversize_negative.json");
    let payload_lengths = json_all_u64(&json, "payload_len");
    let wires = json_all_strings(&json, "wire");
    let normative = json_all_strings(&json, "normative_verdict");
    let runtime = json_all_strings(&json, "runtime_verdict");
    assert_eq!(json_u64(&json, "normative_frame_payload_cap"), 4096);
    assert_eq!(json_u64(&json, "runtime_envelope_payload_cap"), 4096);
    assert_eq!(payload_lengths, vec![4097, 4129]);
    assert_eq!(normative, vec!["reject", "reject"]);
    assert_eq!(runtime, vec!["reject_oversize", "reject_oversize"]);

    for (index, wire_hex) in wires.iter().enumerate() {
        let wire = decode_hex("wire", wire_hex);
        assert_eq!(wire.len(), 3 + payload_lengths[index] as usize);
        assert_eq!(
            u16::from_le_bytes([wire[0], wire[1]]) as usize,
            payload_lengths[index] as usize
        );
        assert_eq!(wire[2], mmp_types::frame::FT_BUNDLE);
        assert_eq!(
            read_envelope_normative(&wire),
            Err("payload exceeds doc 10 logical cap")
        );
        // The independent structural reader accepts both byte streams; the
        // production reader must reject them at the frozen cap boundary.
        assert!(read_envelope_independent(&wire).is_ok());
        assert!(payload_lengths[index] > 4096);
    }
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
    assert_eq!(
        u16::from_le_bytes([wire[tier_offset + 1], wire[tier_offset + 2]]),
        expected_flags
    );
    assert_eq!(wire[tier_offset + 3], expected_credits);
    assert_eq!(
        u16::from_le_bytes([wire[tier_offset + 4], wire[tier_offset + 5]]),
        expected_psm
    );
    assert_eq!(wire.len(), tier_offset + 6, "HELLO has trailing bytes");
}

#[test]
fn beacon_vector_is_accepted_by_independent_fixed_reader() {
    let json = vector("beacon.json");
    let wire = parse_hex(&json, "wire");
    let view = read_beacon_independent(&wire).expect("valid fixed beacon");
    assert_eq!(view.ver, json_u64(&json, "ver") as u8);
    assert_eq!(view.tier, json_u64(&json, "tier") as u8);
    assert_eq!(view.battery, json_u64(&json, "battery") as u8);
    assert_eq!(
        view.ephem_id,
        decode_hex("ephem_id", json_string(&json, "ephem_id"))
    );
    assert_eq!(view.busy, json_bool(&json, "busy"));
    assert_eq!(view.epoch, json_u64(&json, "epoch") as u8);
    assert_eq!(view.frag_structure, json_u64(&json, "frag_structure") as u8);
    assert_eq!(view.frag_offset, json_u64(&json, "frag_offset") as u16);
    assert_eq!(
        view.fragment,
        decode_hex("fragment", json_string(&json, "fragment"))
    );
    assert_eq!(wire.len(), 24, "beacon must remain fixed-size");

    assert_eq!(
        read_beacon_independent(&wire[..23]),
        Err("beacon truncated")
    );
    let mut trailing = wire.clone();
    trailing.push(0);
    assert_eq!(
        read_beacon_independent(&trailing),
        Err("beacon has trailing bytes")
    );
    let mut wrong_version = wire;
    wrong_version[0] = 0x29;
    assert_eq!(
        read_beacon_independent(&wrong_version),
        Err("beacon version is not 1")
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

#[test]
fn reserved_bits_negative_vector_is_rejected_by_independent_bundle_reader() {
    let json = vector("reserved_bits_negative.json");
    let wire = parse_hex(&json, "wire");
    assert_eq!(json_u64(&json, "reserved_mask"), 0x80);
    assert_eq!(
        json_string(&json, "expected_error"),
        "reserved bundle flags are set"
    );
    assert_ne!(wire[1] & 0x80, 0, "fixture must set a reserved bundle flag");
    assert!(matches!(
        read_bundle_independent(&wire),
        Err("reserved bundle flags are set")
    ));
}

#[test]
fn malformed_tlv_vectors_are_rejected_by_independent_rules() {
    let json = vector("bundle_tlv_negative.json");
    let names = json_all_strings(&json, "name");
    let wires = json_all_strings(&json, "wire");
    let expected = json_all_strings(&json, "expected_error");
    assert_eq!(
        names,
        vec![
            "unknown_critical_tlv",
            "out_of_order_tlvs",
            "duplicate_tlv_type"
        ]
    );
    assert_eq!(
        expected,
        vec![
            "unknown critical TLV",
            "TLV order/duplicate",
            "TLV order/duplicate"
        ]
    );
    assert_eq!(wires.len(), 3);

    for (index, wire_hex) in wires.iter().enumerate() {
        let wire = decode_hex("wire", wire_hex);
        let view = read_bundle_independent(&wire)
            .expect("malformed TLV bytes retain a valid outer layout");
        let expected_error = match index {
            0 => "unknown critical TLV",
            1 | 2 => "TLV order/duplicate",
            _ => unreachable!("vector has exactly three malformed TLV cases"),
        };
        assert_eq!(
            validate_tlvs_independent(view.extensions),
            Err(expected_error)
        );
    }
}

#[test]
fn independent_tlv_reader_rejects_known_type_length_mismatches() {
    fn tlv(typ: u8, value: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(3 + value.len());
        bytes.push(typ);
        bytes.extend_from_slice(&(value.len() as u16).to_le_bytes());
        bytes.extend_from_slice(value);
        bytes
    }

    // PARENTS is 1–4 BundleIDs; GROUP_EPOCH and SYMBOL_INFO are fixed-width
    // critical types.  These malformed streams retain valid TLV framing so a
    // length-only implementation cannot accidentally accept them.
    assert_eq!(
        validate_tlvs_independent(&tlv(0x01, &[0; 15])),
        Err("known TLV has invalid length")
    );
    assert_eq!(
        validate_tlvs_independent(&tlv(0x01, &[0; 80])),
        Err("known TLV has invalid length")
    );
    assert_eq!(
        validate_tlvs_independent(&tlv(0x02, &[0])),
        Err("known TLV has invalid length")
    );
    assert_eq!(
        validate_tlvs_independent(&tlv(0x03, &[0; 21])),
        Err("known TLV has invalid length")
    );

    // Boundary-positive checks ensure the new guard does not reject valid
    // encodings: one parent and the largest four-parent list.
    assert_eq!(validate_tlvs_independent(&tlv(0x01, &[0; 16])), Ok(()));
    assert_eq!(validate_tlvs_independent(&tlv(0x01, &[0; 64])), Ok(()));
    assert_eq!(validate_tlvs_independent(&tlv(0x02, &[0; 2])), Ok(()));
    assert_eq!(validate_tlvs_independent(&tlv(0x03, &[0; 22])), Ok(()));
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
    assert_eq!(
        &valid_signature[..32],
        &noncanonical_signature[..32],
        "S+L must preserve R"
    );
    assert_eq!(json_string(&json, "expected_valid"), "accept");
    assert_eq!(json_string(&json, "expected_noncanonical"), "reject");

    // Recompute S+L independently from the artifact.  This is deliberately
    // a tiny byte-level check rather than a call to the production verifier.
    const L_LE: [u8; 32] = [
        0xed, 0xd3, 0xf5, 0x5c, 0x1a, 0x63, 0x12, 0x58, 0xd6, 0x9c, 0xf7, 0xa2, 0xde, 0xf9, 0xde,
        0x14, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x10,
    ];
    let mut carry = 0u16;
    for index in 0..32 {
        let sum = valid_s[index] as u16 + L_LE[index] as u16 + carry;
        assert_eq!(
            sum as u8, noncanonical_s[index],
            "S+L differs at byte {index}"
        );
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
        assert_eq!(
            payload.len(),
            payload_lengths[index] as usize,
            "{} payload length",
            names[index]
        );
        let view = read_bundle_independent(&wire)
            .expect("grouped bundle must satisfy independent layout reader");
        validate_tlvs_independent(view.extensions)
            .unwrap_or_else(|error| panic!("{} extensions rejected: {error}", names[index]));
        assert_eq!(
            u64::from(view.class),
            classes[index],
            "{} class",
            names[index]
        );
        assert_eq!(view.payload, payload, "{} payload bytes", names[index]);
        assert_eq!(
            view.canonical, expected_canonical,
            "{} canonical bytes",
            names[index]
        );
        assert_eq!(
            view.origin, expected_origin,
            "{} origin binding",
            names[index]
        );
        assert_eq!(view.dest, expected_dest, "{} destination", names[index]);
        assert_eq!(
            view.signature, expected_signature,
            "{} outer signature",
            names[index]
        );
        assert_eq!(view.flags & !0b111, 0, "{} reserved flags", names[index]);
        assert!(view.priority <= 3, "{} priority", names[index]);
        assert_eq!(
            view.extensions.len(),
            u16::from_le_bytes([wire[56], wire[57]]) as usize
        );
        assert_eq!(
            view.canonical.len(),
            wire.len() - 64,
            "{} canonical size",
            names[index]
        );
        let digest = blake3::hash(&view.canonical);
        assert_eq!(
            &digest.as_bytes()[..16],
            expected_id,
            "{} content address",
            names[index]
        );
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
    assert_eq!(
        results
            .iter()
            .filter(|value| value.as_str() == "accept")
            .count(),
        9
    );
    assert_eq!(
        results
            .iter()
            .filter(|value| value.as_str() == "reject")
            .count(),
        72
    );
}

#[test]
fn committed_cross_domain_signatures_match_declared_verdicts() {
    let json = vector("cross_domain_matrix.json");
    let object_domains = json_all_strings(&json, "object_domain");
    let public_keys = json_all_strings(&json, "pubkey");
    let objects = json_all_strings(&json, "object");
    let signatures = json_all_strings(&json, "signature");
    let verify_domains = json_all_strings(&json, "verify_domain");
    let results = json_all_strings(&json, "result");
    assert_eq!(object_domains.len(), 9);
    assert_eq!(public_keys.len(), 9);
    assert_eq!(objects.len(), 9);
    assert_eq!(signatures.len(), 9);
    assert_eq!(verify_domains.len(), 81);
    assert_eq!(results.len(), 81);

    fn domain(name: &str) -> mmp_types::domain::SigDomain {
        match name {
            "mmp1/bundle" => mmp_types::domain::SigDomain::Bundle,
            "mmp1/claim" => mmp_types::domain::SigDomain::Claim,
            "mmp1/friend" => mmp_types::domain::SigDomain::Friend,
            "mmp1/prekey" => mmp_types::domain::SigDomain::Prekey,
            "mmp1/blobroot" => mmp_types::domain::SigDomain::BlobRoot,
            "mmp1/chanbind" => mmp_types::domain::SigDomain::ChanBind,
            "mmp1/manifest" => mmp_types::domain::SigDomain::Manifest,
            "mmp1/authcert" => mmp_types::domain::SigDomain::AuthCert,
            "mmp1/announce" => mmp_types::domain::SigDomain::Announce,
            other => panic!("unknown signature domain in committed matrix: {other}"),
        }
    }

    for row in 0..9 {
        assert_eq!(object_domains[row], verify_domains[row * 9 + row]);
        let public_key = mmp_types::ids::Key32(
            decode_hex("pubkey", &public_keys[row])
                .try_into()
                .expect("matrix public keys must be 32 bytes"),
        );
        let object = decode_hex("object", &objects[row]);
        let signature = mmp_types::ids::Sig64(
            decode_hex("signature", &signatures[row])
                .try_into()
                .expect("matrix signatures must be 64 bytes"),
        );
        for column in 0..9 {
            let index = row * 9 + column;
            let accepted = mmp_crypto::verify(
                domain(&verify_domains[index]),
                &public_key,
                &object,
                &signature,
            )
            .is_ok();
            let declared = results[index].as_str() == "accept";
            assert_eq!(
                accepted, declared,
                "committed matrix verdict drifted at ({row},{column})"
            );
        }
    }
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
        assert_eq!(
            types[index],
            u64::from(*expected_type),
            "frame {} tag drifted",
            names[index]
        );
        assert!(
            !seen[*expected_type as usize],
            "duplicate frame type {}",
            expected_type
        );
        seen[*expected_type as usize] = true;

        let payload = decode_hex("payload", &payloads[index]);
        let wire = decode_hex("wire", &wires[index]);
        assert_eq!(
            wire.len(),
            payload.len() + 3,
            "frame {} envelope length mismatch",
            names[index]
        );
        assert_eq!(
            u16::from_le_bytes([wire[0], wire[1]]) as usize,
            payload.len()
        );
        assert_eq!(
            wire[2], *expected_type,
            "frame {} envelope tag mismatch",
            names[index]
        );
        assert_eq!(
            &wire[3..],
            payload.as_slice(),
            "frame {} payload mismatch",
            names[index]
        );
        assert_eq!(
            read_envelope_independent(&wire).unwrap(),
            (*expected_type, payload.as_slice())
        );
    }
}

#[test]
fn frame_inventory_payloads_match_independent_registered_grammars() {
    let json = vector("frames.json");
    let count = json_u64(&json, "frame_count") as usize;
    let names = json_all_strings(&json, "name");
    let types = json_all_u64(&json, "type");
    let payloads = json_all_strings(&json, "payload");
    assert_eq!(count, 17);
    assert_eq!(names.len(), count);
    assert_eq!(types.len(), count);
    assert_eq!(payloads.len(), count);

    // The frame inventory is ordered by the registry in doc 10 §2.  Keeping
    // this list local to the independent test makes omissions visible if the
    // production enum gains a new tag without a corresponding grammar check.
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

    for index in 0..count {
        let typ = types[index] as u8;
        assert_eq!(
            typ, expected_types[index],
            "frame {} type drifted",
            names[index]
        );
        let payload = decode_hex("payload", &payloads[index]);
        read_frame_payload_independent(typ, &payload)
            .unwrap_or_else(|error| panic!("frame {} payload rejected: {error}", names[index]));

        // Every typed payload has a complete grammar and therefore rejects a
        // trailing byte.  HS is deliberately opaque at this layer; its Noise
        // message length is owned by the handshake parser, not doc 10 frame
        // framing.
        if typ != mmp_types::frame::FT_HS {
            let mut trailing = payload.clone();
            trailing.push(0);
            assert!(
                read_frame_payload_independent(typ, &trailing).is_err(),
                "frame {} grammar accepted trailing bytes",
                names[index]
            );
        }

        // A non-empty typed payload must not become valid after truncation.
        // CLOSE is the sole zero-byte payload and is exercised by the
        // trailing-byte assertion above.
        if !payload.is_empty() && typ != mmp_types::frame::FT_HS {
            let truncated = &payload[..payload.len() - 1];
            assert!(
                read_frame_payload_independent(typ, truncated).is_err(),
                "frame {} grammar accepted truncation",
                names[index]
            );
        }
    }
}

#[test]
fn rbsr_fingerprint_vector_matches_independent_reader() {
    // Recompute the doc-06 fingerprint directly from the committed bytes.
    // This deliberately does not call mmp_core::fingerprint: a production
    // change to the fold, count encoding, or byte order must disagree with
    // this specification-derived check instead of updating both paths.
    let json = vector("rbsr_fingerprint.json");
    let id_hex = json_array_strings(&json, "ids");
    assert!(
        !id_hex.is_empty(),
        "RBSR vector must contain at least one ID"
    );

    let mut xor_fold = [0u8; 16];
    for (index, encoded) in id_hex.iter().enumerate() {
        let id = decode_hex("id", encoded);
        assert_eq!(id.len(), 16, "RBSR ID {index} must be exactly 16 bytes");
        for (fold_byte, id_byte) in xor_fold.iter_mut().zip(id) {
            *fold_byte ^= id_byte;
        }
    }

    let mut hasher = blake3::Hasher::new();
    hasher.update(&xor_fold);
    hasher.update(&(id_hex.len() as u32).to_le_bytes());
    let digest = hasher.finalize();
    let fingerprint_bytes = &digest.as_bytes()[..8];
    let expected_bytes = decode_hex("fingerprint_hex", json_string(&json, "fingerprint_hex"));
    assert_eq!(fingerprint_bytes, expected_bytes.as_slice());
    let expected_u64 = json_u64(&json, "fingerprint_u64");
    assert_eq!(
        u64::from_le_bytes(fingerprint_bytes.try_into().unwrap()),
        expected_u64
    );
}
