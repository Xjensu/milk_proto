//! mmp-tal-embedded — nRF52 (nrf-softdevice) / ESP32 (NimBLE) TAL.
//!
//! Status: out of MVP scope by decision (HANDOFF §0 — "keep the `no_std`
//! discipline, ship no embedded TAL"). The lower protocol layers are
//! `no_std`-clean so this crate can be filled in without touching them; the
//! Core-profile budget gate (≤256 KiB flash / 48 KiB RAM, doc 12 §7.6) and the
//! OQ-7 RS-decode bench are the acceptance criteria when this work starts.

#![no_std]

pub use mmp_tal::{LinkId, LinkQuality};
