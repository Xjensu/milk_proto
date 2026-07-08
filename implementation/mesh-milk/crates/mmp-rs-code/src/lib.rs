//! mmp-rs-code — no_std GF(256) Reed-Solomon (doc 08 s8; OQ-7 bench lives here)
//!
//! Stage 0 scaffold (ROADMAP Stage 0): intentionally empty.
//! Layering (doc 12 s1): #![no_std], alloc-optional via the `alloc` feature.

#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;
