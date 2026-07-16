//! Pure, hardware-free chunking arithmetic for the notify path.
//!
//! `chunks` is byte-identical in behavior to `mmp_tal_btleplug::framing::chunks`
//! (doc 09 §5.1: ATT payloads of (MTU−3) bytes, reassembly by length prefix
//! only). It is *duplicated* here rather than imported because importing would
//! drag the whole btleplug central stack into this crate's normal dependency
//! graph for a 3-line function — the wrong trade (the central crate is a
//! dev-dependency for constant-compat tests only).

/// ATT opcode (1 B) + attribute handle (2 B): the per-PDU overhead that
/// separates the exchanged ATT MTU from the usable notification payload.
pub const ATT_HEADER: usize = 3;

/// Conservative floor: 23-byte ATT MTU minimum − [`ATT_HEADER`]. Matches the
/// central crate's `DEFAULT_CHUNK` (its only option — btleplug exposes no
/// negotiated MTU); here it is merely the floor under [`chunk_size_for_mtu`].
pub const DEFAULT_CHUNK: usize = 20;

/// Map the ATT MTU that BlueZ reports on `AcquireWrite`/`AcquireNotify` to a
/// per-notification chunk size: `max(mtu − 3, 20)`.
///
/// BlueZ hands over the raw *exchanged* ATT MTU, so the usable payload is
/// MTU − [`ATT_HEADER`] (doc 09 §5.1). The floor guards against a nonsense
/// report (< 23; the ATT minimum makes that impossible from a conforming
/// stack, so the floor can never exceed the true capacity in practice).
///
/// This is the M5b improvement over the central crate's fixed 20 B: a
/// 247-byte negotiated MTU yields 244-byte chunks — the doc 09 §4 target —
/// instead of 20-byte ones. Follow-up (deliberately NOT done here): give the
/// central crate per-platform MTU escape hatches so its write path matches.
pub fn chunk_size_for_mtu(att_mtu: usize) -> usize {
    att_mtu.saturating_sub(ATT_HEADER).max(DEFAULT_CHUNK)
}

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
    use super::{chunk_size_for_mtu, chunks, DEFAULT_CHUNK};

    #[test]
    fn mtu_floor_is_20_at_the_att_minimum() {
        assert_eq!(chunk_size_for_mtu(23), DEFAULT_CHUNK);
    }

    #[test]
    fn nonsense_mtus_clamp_to_the_floor_instead_of_underflowing() {
        assert_eq!(chunk_size_for_mtu(0), DEFAULT_CHUNK);
        assert_eq!(chunk_size_for_mtu(2), DEFAULT_CHUNK);
        assert_eq!(chunk_size_for_mtu(22), DEFAULT_CHUNK);
    }

    #[test]
    fn negotiated_247_yields_the_doc09_target_244() {
        assert_eq!(chunk_size_for_mtu(247), 244);
    }

    #[test]
    fn chunk_size_is_always_mtu_minus_att_header_above_the_floor() {
        assert_eq!(chunk_size_for_mtu(100), 97);
        assert_eq!(chunk_size_for_mtu(517), 514);
    }

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
    #[should_panic(expected = "chunk size must be non-zero")]
    fn zero_chunk_size_panics_instead_of_looping_forever() {
        let data = [1u8, 2, 3];
        let _ = chunks(&data, 0).count();
    }
}
