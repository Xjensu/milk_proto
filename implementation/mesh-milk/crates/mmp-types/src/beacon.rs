//! Beacon payload (doc 10 §5) — 24 bytes, fixed-size parser, zero allocation
//! (pre-auth parser rule, doc 10 §6.2).

use crate::err::WireError;

/// Fixed encoded length of a [`Beacon`], bytes (doc 10 §5).
pub const BEACON_LEN: usize = 24;

/// Unauthenticated pre-connection advertisement (doc 10 §5): the fixed
/// 24-byte payload nodes broadcast so peers can decide whether to dial before
/// spending any handshake cost.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Beacon {
    /// Wire version (4 bits, must be 1).
    pub ver: u8,
    /// Tier (2 bits, doc 04).
    pub tier: u8,
    /// Battery class (2 bits).
    pub battery: u8,
    /// Rotating pseudonym (doc 03 §5.1.1).
    pub ephem_id: [u8; 8],
    /// Busy flag — bit 7 of the epoch byte (doc 10 §5 note). Unauthenticated;
    /// setting it only suppresses dials to the setter's own broadcasts.
    pub busy: bool,
    /// Summary rotation counter (7 bits; schedule = epoch mod 4, doc 09 §3.3).
    pub epoch: u8,
    /// Fragment structure selector (2 bits).
    pub frag_structure: u8,
    /// Fragment offset (14 bits).
    pub frag_offset: u16,
    /// Rotating summary fragment, re-salted per ephem epoch (AUDIT F8a).
    pub fragment: [u8; 12],
}

impl Beacon {
    /// Encode to the fixed 24-byte wire form (doc 10 §5).
    pub fn encode(&self) -> [u8; BEACON_LEN] {
        debug_assert!(self.tier <= 3 && self.battery <= 3 && self.epoch <= 0x7f);
        debug_assert!(self.frag_structure <= 3 && self.frag_offset < (1 << 14));
        let mut b = [0u8; BEACON_LEN];
        b[0] = (self.ver << 4) | (self.tier << 2) | self.battery;
        b[1..9].copy_from_slice(&self.ephem_id);
        b[9] = self.epoch | if self.busy { 0x80 } else { 0 };
        let frag = (u16::from(self.frag_structure) << 14) | self.frag_offset;
        b[10..12].copy_from_slice(&frag.to_le_bytes());
        b[12..24].copy_from_slice(&self.fragment);
        b
    }

    /// Strict fixed-size decode: rejects any length other than exactly
    /// [`BEACON_LEN`] and any non-1 wire version.
    pub fn decode(bytes: &[u8]) -> Result<Beacon, WireError> {
        if bytes.len() != BEACON_LEN {
            return Err(if bytes.len() < BEACON_LEN { WireError::Truncated } else { WireError::Trailing });
        }
        let ver = bytes[0] >> 4;
        if ver != crate::bundle::WIRE_VERSION {
            return Err(WireError::BadVersion);
        }
        let mut ephem_id = [0u8; 8];
        ephem_id.copy_from_slice(&bytes[1..9]);
        let frag = u16::from_le_bytes([bytes[10], bytes[11]]);
        let mut fragment = [0u8; 12];
        fragment.copy_from_slice(&bytes[12..24]);
        Ok(Beacon {
            ver,
            tier: (bytes[0] >> 2) & 0x03,
            battery: bytes[0] & 0x03,
            ephem_id,
            busy: bytes[9] & 0x80 != 0,
            epoch: bytes[9] & 0x7f,
            frag_structure: (frag >> 14) as u8,
            frag_offset: frag & 0x3fff,
            fragment,
        })
    }
}
