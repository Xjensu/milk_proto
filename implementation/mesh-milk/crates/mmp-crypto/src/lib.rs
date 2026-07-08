//! mmp-crypto — suite 0x01, ratchets, sealed/prekey (doc 08)
//!
//! Stage 0 scaffold (ROADMAP Stage 0): intentionally empty.
//! Layering (doc 12 s1): #![no_std], alloc-optional via the `alloc` feature.

#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;
