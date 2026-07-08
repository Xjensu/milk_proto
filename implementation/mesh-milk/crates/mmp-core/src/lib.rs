//! mmp-core — bundle core, session FSM, sync engine (docs 03/06/11)
//!
//! Stage 0 scaffold (ROADMAP Stage 0): intentionally empty.
//! Layering (doc 12 s1): #![no_std], alloc-optional via the `alloc` feature.

#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;
