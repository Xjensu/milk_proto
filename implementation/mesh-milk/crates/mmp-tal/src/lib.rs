//! mmp-tal — Transport Adaptation Layer seams (doc 03 §7, doc 12 §2).
//!
//! The protocol core is sans-IO; a TAL is whatever feeds it. The host (PC
//! btleplug loop, Android/iOS BLE glue, the simulator) owns the radio and
//! shuttles [`TalEvent`]s in and link bytes out. Frames are length-prefixed by
//! `mmp_types::frame`; the reassembly bound lives in the session engine
//! (AUDIT F10), so a TAL never buffers more than one radio burst.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::collections::VecDeque;
use alloc::vec::Vec;

/// Host-assigned identifier of one live link (GATT connection, mem pipe…).
pub type LinkId = u64;

/// Development-time 128-bit service UUID under the `mmp1` namespace — a
/// SIG-assigned 16-bit UUID is a production (not MVP) release blocker
/// (OQ-12). Single source of truth (E3-S3): `mmp-tal-btleplug` (central) and
/// `mmp-tal-peripheral` re-export this rather than each defining their own
/// literal, so both backends and `MOBILE_INTEGRATION.md` can never drift.
pub const DEV_SERVICE_UUID: &str = "6d6d7031-0000-4d4d-5031-6d6573686d6c";

/// Development-time TX characteristic (client→server, Write Without
/// Response), from the GATT **central**'s perspective. Doc 09 §5.1 does not
/// pin a UUID; see `mmp-tal-btleplug`'s module docs for how this placeholder
/// was derived and what still needs reconciling before SIG assignment.
pub const DEV_TX_CHARACTERISTIC_UUID: &str = "6d6d7031-0001-4d4d-5031-6d6573686d6c";

/// Development-time RX characteristic (server→client, Notify), from the GATT
/// **central**'s perspective. See [`DEV_TX_CHARACTERISTIC_UUID`] docs.
pub const DEV_RX_CHARACTERISTIC_UUID: &str = "6d6d7031-0002-4d4d-5031-6d6573686d6c";

/// Link-quality hints for transfer scheduling (doc 05 §6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinkQuality {
    pub mtu: u16,
    /// EWMA goodput estimate, bits/s.
    pub goodput_bps: u32,
}

impl Default for LinkQuality {
    fn default() -> Self {
        // Pessimistic BLE-GATT floor (doc 02 Track B).
        LinkQuality { mtu: 247, goodput_bps: 50_000 }
    }
}

/// Everything a radio can tell the node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TalEvent {
    /// A beacon advertisement was scanned.
    Beacon { payload: Vec<u8>, rssi: i8 },
    /// A reliable, ordered link came up (we dialed if `initiator`).
    LinkUp { link: LinkId, initiator: bool, quality: LinkQuality },
    /// Bytes arrived on a link.
    LinkData { link: LinkId, bytes: Vec<u8> },
    /// The link dropped (any reason — the protocol treats all alike).
    LinkDown { link: LinkId },
}

/// In-memory duplex link pair for tests and `mmp-sim` — the same seam real
/// TALs use; no cfg leakage (doc 12 §8).
#[derive(Default)]
pub struct MemLink {
    a_to_b: VecDeque<Vec<u8>>,
    b_to_a: VecDeque<Vec<u8>>,
}

impl MemLink {
    pub fn new() -> MemLink {
        MemLink::default()
    }

    pub fn send_a(&mut self, bytes: Vec<u8>) {
        self.a_to_b.push_back(bytes);
    }

    pub fn send_b(&mut self, bytes: Vec<u8>) {
        self.b_to_a.push_back(bytes);
    }

    pub fn recv_a(&mut self) -> Option<Vec<u8>> {
        self.b_to_a.pop_front()
    }

    pub fn recv_b(&mut self) -> Option<Vec<u8>> {
        self.a_to_b.pop_front()
    }

    pub fn is_quiet(&self) -> bool {
        self.a_to_b.is_empty() && self.b_to_a.is_empty()
    }
}
