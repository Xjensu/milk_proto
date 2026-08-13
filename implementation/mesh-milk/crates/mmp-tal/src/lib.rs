//! mmp-tal — Transport Adaptation Layer seams (doc 03 §7, doc 12 §2).
//!
//! The protocol core is sans-IO; a TAL is whatever feeds it. The host (PC
//! btleplug loop, Android/iOS BLE glue, the simulator) owns the radio and
//! shuttles [`TalEvent`]s in and link bytes out. Frames are length-prefixed by
//! `mmp_types::frame`; the reassembly bound lives in the session engine
//! (AUDIT F10), so a TAL never buffers more than one radio burst.

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]

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
    /// Maximum bytes the link will carry in one write (ATT MTU for BLE
    /// backends, or an equivalent framing ceiling for other transports).
    pub mtu: u16,
    /// EWMA goodput estimate, bits/s.
    pub goodput_bps: u32,
}

impl Default for LinkQuality {
    fn default() -> Self {
        // Pessimistic BLE-GATT floor (doc 02 Track B).
        LinkQuality {
            mtu: 247,
            goodput_bps: 50_000,
        }
    }
}

/// Everything a radio can tell the node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TalEvent {
    /// A beacon advertisement was scanned.
    Beacon {
        /// The raw advertisement payload as scanned, undecoded.
        payload: Vec<u8>,
        /// Received signal strength of the advertisement, in dBm.
        rssi: i8,
    },
    /// A reliable, ordered link came up (we dialed if `initiator`).
    LinkUp {
        /// Host-assigned identifier of the link that came up.
        link: LinkId,
        /// `true` if this side dialed the link, `false` if it accepted an
        /// incoming connection.
        initiator: bool,
        /// Link-quality hints to seed the transfer scheduler with.
        quality: LinkQuality,
    },
    /// Bytes arrived on a link.
    LinkData {
        /// The link the bytes arrived on.
        link: LinkId,
        /// The raw bytes received, undecoded.
        bytes: Vec<u8>,
    },
    /// The link dropped (any reason — the protocol treats all alike).
    LinkDown {
        /// The link that dropped.
        link: LinkId,
    },
}

/// In-memory duplex link pair for tests and `mmp-sim` — the same seam real
/// TALs use; no cfg leakage (doc 12 §8).
#[derive(Default)]
pub struct MemLink {
    a_to_b: VecDeque<Vec<u8>>,
    b_to_a: VecDeque<Vec<u8>>,
}

impl MemLink {
    /// Creates an empty link pair with both queues drained.
    pub fn new() -> MemLink {
        MemLink::default()
    }

    /// Queues `bytes` as sent from side A, to be read back by [`MemLink::recv_b`].
    pub fn send_a(&mut self, bytes: Vec<u8>) {
        self.a_to_b.push_back(bytes);
    }

    /// Queues `bytes` as sent from side B, to be read back by [`MemLink::recv_a`].
    pub fn send_b(&mut self, bytes: Vec<u8>) {
        self.b_to_a.push_back(bytes);
    }

    /// Pops the oldest byte chunk sent by side B, if any is queued.
    pub fn recv_a(&mut self) -> Option<Vec<u8>> {
        self.b_to_a.pop_front()
    }

    /// Pops the oldest byte chunk sent by side A, if any is queued.
    pub fn recv_b(&mut self) -> Option<Vec<u8>> {
        self.a_to_b.pop_front()
    }

    /// `true` when neither direction has any bytes queued.
    pub fn is_quiet(&self) -> bool {
        self.a_to_b.is_empty() && self.b_to_a.is_empty()
    }
}
