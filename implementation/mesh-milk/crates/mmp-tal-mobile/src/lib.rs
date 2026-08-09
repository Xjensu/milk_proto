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

#[cfg(test)]
mod tests {
    use super::{att_chunk, checked_att_chunk, ATT_HEADER_BYTES, MIN_ATT_MTU};

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
}
