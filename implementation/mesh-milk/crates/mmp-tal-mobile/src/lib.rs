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

pub use mmp_tal::{LinkId, LinkQuality, TalEvent};

/// Maximum ATT payload per Write-Without-Response / Notification at MTU `m`.
pub const fn att_chunk(mtu: u16) -> usize {
    (mtu as usize).saturating_sub(3)
}
