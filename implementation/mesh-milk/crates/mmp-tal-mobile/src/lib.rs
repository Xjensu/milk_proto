//! mmp-tal-mobile — Android/iOS BLE glue (doc 12 §6).
//!
//! Deliberately (and permanently) thin: on mobile **the platform owns the
//! radio** (HANDOFF_MOBILE §1). Kotlin/Swift implement scanning, advertising,
//! and GATT natively and drive `mmp-ffi`'s node directly with the same
//! `TalEvent` vocabulary — there is no Rust radio code on phones, so this
//! crate only re-exports the shared TAL types and fragmentation helpers.
//!
//! See `MOBILE_INTEGRATION.md` §4 (Android) and §5 (iOS) for the platform
//! duty checklists this design imposes.

#![no_std]

pub use mmp_tal::{LinkId, LinkQuality, TalEvent};

/// ATT protocol overhead excluded from one Write-Without-Response or
/// Notification payload.
pub const ATT_HEADER_BYTES: usize = 3;

/// Bluetooth LE's minimum negotiated ATT MTU.
pub const MIN_ATT_MTU: u16 = 23;

/// Maximum ATT payload per Write-Without-Response / Notification at MTU `m`.
pub const fn att_chunk(mtu: u16) -> usize {
    (mtu as usize).saturating_sub(ATT_HEADER_BYTES)
}

/// Return the usable ATT payload for a validated BLE MTU.
///
/// The legacy [`att_chunk`] helper is retained for source compatibility and
/// deliberately preserves its saturating behavior.  Mobile hosts should use
/// this checked variant before starting fragmentation: an MTU below
/// [`MIN_ATT_MTU`] is not a valid BLE connection parameter and returns
/// `None`, preventing a zero-sized fragmentation loop.
pub const fn checked_att_chunk(mtu: u16) -> Option<usize> {
    if mtu < MIN_ATT_MTU {
        None
    } else {
        Some((mtu as usize) - ATT_HEADER_BYTES)
    }
}

/// An allocation-free iterator over ATT-sized fragments borrowed from an
/// input buffer.
///
/// The iterator is constructed by [`checked_att_chunks`]. Its chunk size is
/// always non-zero because construction rejects an MTU below
/// [`MIN_ATT_MTU`].
pub struct AttChunks<'a> {
    remaining: &'a [u8],
    chunk_len: usize,
}

impl<'a> Iterator for AttChunks<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining.is_empty() {
            return None;
        }

        let split_at = self.chunk_len.min(self.remaining.len());
        let (fragment, remaining) = self.remaining.split_at(split_at);
        self.remaining = remaining;
        Some(fragment)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.remaining.is_empty() {
            return (0, Some(0));
        }

        let length = self.remaining.len();
        let full_fragments = length / self.chunk_len;
        let fragments = full_fragments + usize::from(length > full_fragments * self.chunk_len);
        (fragments, Some(fragments))
    }
}

impl ExactSizeIterator for AttChunks<'_> {}

impl core::iter::FusedIterator for AttChunks<'_> {}

/// Split `bytes` into consecutive ATT payload slices for a validated MTU.
///
/// Returns `None` when `mtu` is below the Bluetooth LE minimum. Otherwise the
/// iterator yields zero or more borrowed slices, each no larger than the
/// negotiated MTU minus [`ATT_HEADER_BYTES`]. Empty input yields no fragments,
/// and no empty tail is emitted for an exact multiple.
pub fn checked_att_chunks(bytes: &[u8], mtu: u16) -> Option<AttChunks<'_>> {
    let chunk_len = checked_att_chunk(mtu)?;
    Some(AttChunks {
        remaining: bytes,
        chunk_len,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        att_chunk, checked_att_chunk, checked_att_chunks, ATT_HEADER_BYTES, MIN_ATT_MTU,
    };

    #[test]
    fn checked_chunk_accepts_minimum_and_large_mtu() {
        assert_eq!(ATT_HEADER_BYTES, 3);
        assert_eq!(MIN_ATT_MTU, 23);
        assert_eq!(checked_att_chunk(MIN_ATT_MTU), Some(20));
        assert_eq!(checked_att_chunk(u16::MAX), Some((u16::MAX as usize) - 3));
    }

    #[test]
    fn checked_chunk_rejects_invalid_mtu_without_zero_progress() {
        assert_eq!(checked_att_chunk(0), None);
        assert_eq!(checked_att_chunk(MIN_ATT_MTU - 1), None);
    }

    #[test]
    fn legacy_chunk_keeps_saturating_compatibility_behavior() {
        assert_eq!(att_chunk(0), 0);
        assert_eq!(att_chunk(2), 0);
        assert_eq!(att_chunk(3), 0);
        assert_eq!(att_chunk(4), 1);
    }

    #[test]
    fn checked_chunks_reject_invalid_mtu() {
        let data = [1u8, 2, 3];
        assert!(checked_att_chunks(&data, MIN_ATT_MTU - 1).is_none());
    }

    #[test]
    fn checked_chunks_accept_minimum_mtu() {
        let data = [0u8; 21];
        let mut fragments = checked_att_chunks(&data, MIN_ATT_MTU).expect("minimum MTU is valid");
        assert_eq!(fragments.len(), 2);
        assert_eq!(fragments.next(), Some(&data[..20]));
        assert_eq!(fragments.next(), Some(&data[20..]));
        assert_eq!(fragments.next(), None);
    }

    #[test]
    fn checked_chunks_empty_input_yields_no_fragments() {
        let fragments = checked_att_chunks(&[], MIN_ATT_MTU).expect("minimum MTU is valid");
        assert_eq!(fragments.len(), 0);
        assert_eq!(fragments.count(), 0);
    }

    #[test]
    fn checked_chunks_exact_multiple_has_no_empty_tail() {
        let data = [1u8; 40];
        let mut fragments = checked_att_chunks(&data, MIN_ATT_MTU).expect("valid MTU");
        assert_eq!(fragments.len(), 2);
        assert_eq!(fragments.next(), Some(&data[..20]));
        assert_eq!(fragments.next(), Some(&data[20..]));
        assert_eq!(fragments.next(), None);
    }

    #[test]
    fn checked_chunks_remainder_is_a_short_final_fragment() {
        let data = [1u8; 25];
        let mut fragments = checked_att_chunks(&data, 26).expect("valid MTU");
        assert_eq!(fragments.len(), 2);
        assert_eq!(fragments.next(), Some(&data[..23]));
        assert_eq!(fragments.next(), Some(&data[23..]));
        assert_eq!(fragments.next(), None);
    }

    #[test]
    fn checked_chunks_large_mtu_does_not_overrun_input() {
        let data = [9u8, 8, 7];
        let mut fragments = checked_att_chunks(&data, u16::MAX).expect("maximum MTU is valid");
        assert_eq!(fragments.len(), 1);
        assert_eq!(fragments.next(), Some(&data[..]));
        assert_eq!(fragments.next(), None);
    }
}
