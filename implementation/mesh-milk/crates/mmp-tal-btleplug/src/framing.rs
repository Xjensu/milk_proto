//! Pure, hardware-free byte chunking for the write-without-response path
//! (doc 09 §5.1: ATT payloads of (MTU−3) bytes, reassembly by length prefix
//! only — chunking itself carries no framing of its own).
//!
//! Kept separate from [`crate::BleCentral`] so the CI-covered logic (how
//! bytes get sliced) never depends on a live radio.

/// Split `bytes` into consecutive slices of at most `chunk` bytes each.
///
/// Pure and total: an empty input yields zero chunks, a length that's an
/// exact multiple of `chunk` yields no short final chunk, and any remainder
/// becomes a final short chunk.
///
/// # Panics
///
/// Panics if `chunk == 0` (a zero-sized chunk can never make progress).
pub fn chunks(bytes: &[u8], chunk: usize) -> impl Iterator<Item = &[u8]> {
    assert!(chunk > 0, "chunk size must be non-zero");
    bytes.chunks(chunk)
}

#[cfg(test)]
mod tests {
    use super::chunks;

    #[test]
    fn empty_input_yields_zero_chunks() {
        let data: [u8; 0] = [];
        assert_eq!(chunks(&data, 4).count(), 0);
    }

    #[test]
    fn exact_multiple_splits_evenly_with_no_short_tail() {
        let data = [1u8, 2, 3, 4, 5, 6];
        let got: Vec<&[u8]> = chunks(&data, 3).collect();
        assert_eq!(got, vec![&[1, 2, 3][..], &[4, 5, 6][..]]);
    }

    #[test]
    fn remainder_produces_a_short_final_chunk() {
        let data = [1u8, 2, 3, 4, 5, 6, 7];
        let got: Vec<&[u8]> = chunks(&data, 3).collect();
        assert_eq!(got, vec![&[1, 2, 3][..], &[4, 5, 6][..], &[7][..]]);
    }

    #[test]
    fn chunk_of_one_yields_one_byte_per_chunk() {
        let data = [9u8, 8, 7];
        let got: Vec<&[u8]> = chunks(&data, 1).collect();
        assert_eq!(got, vec![&[9][..], &[8][..], &[7][..]]);
    }

    #[test]
    #[should_panic(expected = "chunk size must be non-zero")]
    fn zero_chunk_size_panics_instead_of_looping_forever() {
        let data = [1u8, 2, 3];
        let _ = chunks(&data, 0).count();
    }
}
