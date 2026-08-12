//! Allocation-free journal framing and recovery scanning for embedded providers.
//!
//! This crate deliberately stops at a byte-level recovery primitive. It does
//! not implement `BundleStore`, choose a board capacity, or claim that a
//! particular flash controller makes a write atomic. A provider must still
//! supply its own aligned read/write/erase and power-loss policy.

#![no_std]
#![warn(missing_docs)]

use core::convert::TryFrom;

/// Four-byte format marker for this implementation-local journal record.
pub const MAGIC: [u8; 4] = *b"FJ01";
/// Four-byte marker written last by a provider after the record body is valid.
pub const COMMIT_MARKER: [u8; 4] = *b"CMT!";
/// Maximum payload accepted by the scanner and encoder.
///
/// This is a parser-safety bound, not a board storage quota or a wire limit.
pub const MAX_PAYLOAD_LEN: usize = 65_536;
const HEADER_LEN: usize = 4 + 8 + 4;
const CHECKSUM_LEN: usize = 32;
const TRAILER_LEN: usize = CHECKSUM_LEN + COMMIT_MARKER.len();
const MIN_RECORD_LEN: usize = HEADER_LEN + TRAILER_LEN;

/// A valid committed record borrowed from the caller's journal image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Record<'a> {
    /// Monotonically increasing provider generation.
    pub generation: u64,
    /// Payload bytes, borrowed from the scanned image.
    pub payload: &'a [u8],
}

/// Result of scanning a journal image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScanResult<'a> {
    /// Highest-generation committed record, if one is valid.
    pub selected: Option<Record<'a>>,
    /// Number of candidate magic markers encountered.
    pub candidates: u32,
    /// Number of records with valid checksum and commit marker.
    pub valid: u32,
    /// Number of candidates rejected before the scanner reached the end.
    pub rejected: u32,
}

/// Errors returned when encoding a record into caller-provided storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodeError {
    /// The payload exceeds [`MAX_PAYLOAD_LEN`].
    PayloadTooLarge,
    /// The output slice cannot hold the complete framed record.
    OutputTooSmall,
}

/// Return the encoded length for a payload, if it is representable.
pub const fn encoded_len(payload_len: usize) -> Option<usize> {
    if payload_len > MAX_PAYLOAD_LEN {
        return None;
    }
    let Some(with_header) = HEADER_LEN.checked_add(payload_len) else {
        return None;
    };
    with_header.checked_add(TRAILER_LEN)
}

/// Encode one complete record into caller-owned memory.
///
/// The function writes the commit marker into the returned image, but this is
/// only a complete byte representation. A flash provider must arrange its
/// physical write order so the marker is committed last and must report an
/// unknown outcome conservatively to its caller.
pub fn encode<'a>(
    generation: u64,
    payload: &[u8],
    out: &'a mut [u8],
) -> Result<&'a [u8], EncodeError> {
    if payload.len() > MAX_PAYLOAD_LEN {
        return Err(EncodeError::PayloadTooLarge);
    }
    let length = encoded_len(payload.len()).ok_or(EncodeError::PayloadTooLarge)?;
    if out.len() < length {
        return Err(EncodeError::OutputTooSmall);
    }

    out[..4].copy_from_slice(&MAGIC);
    out[4..12].copy_from_slice(&generation.to_le_bytes());
    let payload_len = u32::try_from(payload.len()).map_err(|_| EncodeError::PayloadTooLarge)?;
    out[12..16].copy_from_slice(&payload_len.to_le_bytes());
    out[16..16 + payload.len()].copy_from_slice(payload);

    let checksum_start = 16 + payload.len();
    let checksum = checksum(&out[..checksum_start]);
    out[checksum_start..checksum_start + CHECKSUM_LEN].copy_from_slice(&checksum);
    out[checksum_start + CHECKSUM_LEN..length].copy_from_slice(&COMMIT_MARKER);
    Ok(&out[..length])
}

/// Scan an implementation-local journal image without allocating.
///
/// Invalid checksums, invalid commit markers, and impossible candidate
/// lengths are skipped. A truncated candidate at the end is treated as an
/// incomplete append and terminates the scan; earlier valid generations remain
/// eligible. If multiple valid records share a generation, the first valid
/// record wins deterministically.
pub fn scan(image: &[u8]) -> ScanResult<'_> {
    let mut cursor = 0usize;
    let mut result = ScanResult {
        selected: None,
        candidates: 0,
        valid: 0,
        rejected: 0,
    };

    while cursor.saturating_add(4) <= image.len() {
        if image[cursor..cursor + 4] != MAGIC {
            cursor += 1;
            continue;
        }
        result.candidates = result.candidates.saturating_add(1);
        if image.len() - cursor < MIN_RECORD_LEN {
            break;
        }

        let generation = u64::from_le_bytes(image[cursor + 4..cursor + 12].try_into().unwrap());
        let payload_len =
            u32::from_le_bytes(image[cursor + 12..cursor + 16].try_into().unwrap()) as usize;
        let Some(record_len) = encoded_len(payload_len) else {
            result.rejected = result.rejected.saturating_add(1);
            cursor += 1;
            continue;
        };
        if record_len > image.len() - cursor {
            // A final partial append cannot be distinguished from a future
            // record without an index. Preserve earlier complete generations.
            break;
        }

        let payload_start = cursor + HEADER_LEN;
        let payload_end = payload_start + payload_len;
        let checksum_end = payload_end + CHECKSUM_LEN;
        let expected = checksum(&image[cursor..payload_end]);
        if image[payload_end..checksum_end] != expected
            || image[checksum_end..checksum_end + COMMIT_MARKER.len()] != COMMIT_MARKER
        {
            result.rejected = result.rejected.saturating_add(1);
            cursor += 1;
            continue;
        }

        result.valid = result.valid.saturating_add(1);
        let record = Record {
            generation,
            payload: &image[payload_start..payload_end],
        };
        let replace = result
            .selected
            .map(|current| generation > current.generation)
            .unwrap_or(true);
        if replace {
            result.selected = Some(record);
        }
        cursor += record_len;
    }

    result
}

fn checksum(body: &[u8]) -> [u8; CHECKSUM_LEN] {
    let mut hasher = blake3::Hasher::new_derive_key("mmp flash journal v1");
    hasher.update(body);
    *hasher.finalize().as_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn append(image: &mut [u8], used: &mut usize, generation: u64, payload: &[u8]) -> usize {
        let mut record = [0u8; 16 + 64 + 32 + 4];
        let encoded = encode(generation, payload, &mut record).unwrap();
        image[*used..*used + encoded.len()].copy_from_slice(encoded);
        *used += encoded.len();
        encoded.len()
    }

    #[test]
    fn selects_highest_generation_and_borrows_payload() {
        let mut image = [0u8; 256];
        let mut used = 0;
        append(&mut image, &mut used, 2, b"new");
        append(&mut image, &mut used, 1, b"old");
        append(&mut image, &mut used, 3, b"latest");
        let result = scan(&image[..used]);
        assert_eq!(result.selected.unwrap().generation, 3);
        assert_eq!(result.selected.unwrap().payload, b"latest");
        assert_eq!(result.valid, 3);
    }

    #[test]
    fn truncated_tail_preserves_previous_generation() {
        let mut image = [0u8; 256];
        let mut used = 0;
        append(&mut image, &mut used, 4, b"complete");
        image[used..used + 4].copy_from_slice(&MAGIC);
        image[used + 4..used + 12].fill(0xaa);
        let result = scan(&image[..used + 12]);
        assert_eq!(result.selected.unwrap().generation, 4);
        assert_eq!(result.valid, 1);
    }

    #[test]
    fn bad_checksum_and_marker_are_rejected() {
        let mut image = [0u8; 256];
        let mut used = 0;
        append(&mut image, &mut used, 1, b"bad");
        image[20] ^= 0x80;
        let mut second = [0u8; 128];
        let second_len = encoded_len(b"also bad".len()).unwrap();
        encode(2, b"also bad", &mut second).unwrap();
        second[second_len - 1] ^= 1;
        image[used..used + second_len].copy_from_slice(&second[..second_len]);
        used += second_len;
        let result = scan(&image[..used]);
        assert_eq!(result.selected, None);
        assert_eq!(result.valid, 0);
        assert!(result.rejected >= 2);
    }

    #[test]
    fn impossible_length_is_skipped_and_later_record_is_recovered() {
        let mut image = [0u8; 512];
        let mut used = 16;
        image[..4].copy_from_slice(&MAGIC);
        image[4..12].copy_from_slice(&99u64.to_le_bytes());
        image[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
        append(&mut image, &mut used, 7, b"usable");
        let result = scan(&image[..used]);
        assert_eq!(result.selected.unwrap().generation, 7);
        assert_eq!(result.valid, 1);
        assert!(result.rejected >= 1);
    }

    #[test]
    fn duplicate_generation_keeps_first_valid_record() {
        let mut image = [0u8; 256];
        let mut used = 0;
        append(&mut image, &mut used, 8, b"first");
        append(&mut image, &mut used, 8, b"second");
        let result = scan(&image[..used]);
        assert_eq!(result.selected.unwrap().payload, b"first");
    }

    #[test]
    fn encode_rejects_small_output_and_large_payload() {
        let mut small = [0u8; 8];
        assert_eq!(
            encode(1, b"payload", &mut small),
            Err(EncodeError::OutputTooSmall)
        );
        let mut out = [0u8; 128];
        let too_large = [0u8; MAX_PAYLOAD_LEN + 1];
        assert_eq!(
            encode(1, &too_large, &mut out),
            Err(EncodeError::PayloadTooLarge)
        );
    }
}
