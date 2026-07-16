//! Bundle codec (doc 10 §3) — the foundation everything sits on.
//!
//! Normative layout (offsets fixed):
//! ```text
//! 0  ver_class(1)  1 flags(1)  2 origin(32)  34 dest(16)
//! 50 lifetime_ms(u40)  55 priority(1)  56 ext_len(u16)
//! 58 TLVs…  payload_len(u16)  payload…  signature(64)
//! ```
//! BundleID and the signature preimage are both computed over the canonical
//! bytes with the 5 `lifetime_ms` bytes ZEROED (doc 10 §3, AUDIT F1): carriers
//! mutate lifetime per hop, and a lifetime-reset must yield an identical ID so
//! dedup is resurrection-resistant.

use alloc::vec::Vec;

use crate::consts::MAX_BUNDLE;
use crate::domain::{signed_preimage, SigDomain};
use crate::err::WireError;
use crate::ids::{node_id_of, Id16, Key32, NodeId, Sig64, U40_MAX};
use crate::tlv::{decode_exts, encode_exts, Ext, TLV_PARENTS};
use crate::wire::{BufExt, Reader};

pub const WIRE_VERSION: u8 = 1;
pub const SIG_LEN: usize = 64;
pub const LIFETIME_OFFSET: usize = 50;
pub const FIXED_HEADER_LEN: usize = 58;

/// Bundle classes (doc 03 §4.1; class 5 defined by doc 18, relay-blind here).
pub mod class {
    pub const CONTROL: u8 = 0;
    pub const DIRECT: u8 = 1;
    pub const GROUP: u8 = 2;
    pub const CLAIM: u8 = 3;
    pub const SYMBOL: u8 = 4;
    pub const ANNOUNCEMENT: u8 = 5;
}

/// Named priority classes for class-5 bundles (doc 18 §5.1). The `priority`
/// field itself is unchanged wire (doc 03 §4.1, values 0-3); this just names
/// the values class-5 admission (doc 18 §5.2) cares about.
pub mod prio {
    pub const EMERGENCY: u8 = 0;
    pub const OPERATIONAL: u8 = 1;
    pub const NORMAL: u8 = 2;
    pub const BULK: u8 = 3;
}

/// Defined flag bits; all others are reserved-zero (doc 10 §6.1).
pub const FLAG_HAS_PARENTS: u8 = 0b001;
pub const FLAG_WANTS_RECEIPT: u8 = 0b010;
/// Async first-contact key establishment payload form (doc 10 §4.1).
pub const FLAG_ASYNC_KEYS: u8 = 0b100;
const FLAGS_KNOWN: u8 = FLAG_HAS_PARENTS | FLAG_WANTS_RECEIPT | FLAG_ASYNC_KEYS;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bundle {
    pub class: u8,
    pub flags: u8,
    /// Origin's full Ed25519 identity public key, inline (AUDIT F2) —
    /// verify-before-forward is unconditional.
    pub origin: Key32,
    pub dest: Id16,
    /// REMAINING lifetime; relay-mutable, excluded from ID and signature.
    pub lifetime_ms: u64,
    pub priority: u8,
    /// Strictly ascending by TLV type (enforced on decode, asserted on encode).
    pub exts: Vec<Ext>,
    pub payload: Vec<u8>,
    pub signature: Sig64,
}

impl Bundle {
    /// Full wire bytes including signature.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = self.encode_unsigned();
        out.extend_from_slice(&self.signature.0);
        debug_assert!(out.len() <= MAX_BUNDLE);
        out
    }

    /// Canonical bytes `[0 .. sig_offset)` with the live lifetime — the form
    /// the wire carries before the signature.
    fn encode_unsigned(&self) -> Vec<u8> {
        debug_assert!(self.class <= 0x0f && self.priority <= 3);
        debug_assert!(self.lifetime_ms <= U40_MAX);
        let mut out = Vec::with_capacity(FIXED_HEADER_LEN + 64 + self.payload.len() + SIG_LEN);
        out.push((WIRE_VERSION << 4) | self.class);
        out.push(self.flags);
        out.extend_from_slice(&self.origin.0);
        out.extend_from_slice(&self.dest.0);
        out.put_u40(self.lifetime_ms);
        out.push(self.priority);
        let ext_len_at = out.len();
        out.put_u16(0);
        encode_exts(&self.exts, &mut out);
        let ext_len = (out.len() - ext_len_at - 2) as u16;
        out[ext_len_at..ext_len_at + 2].copy_from_slice(&ext_len.to_le_bytes());
        out.put_var(&self.payload);
        out
    }

    /// Lifetime-zeroed canonical bytes — the common input of BundleID and the
    /// signature preimage (doc 10 §3). One function for both paths (HANDOFF §1.5).
    pub fn canonical_zeroed(&self) -> Vec<u8> {
        let mut bytes = self.encode_unsigned();
        zero_lifetime(&mut bytes);
        bytes
    }

    /// The exact Ed25519 message for this bundle's signature.
    pub fn signing_message(&self) -> Vec<u8> {
        signed_preimage(SigDomain::Bundle, &self.canonical_zeroed())
    }

    /// `BundleID = BLAKE3(canonical_zeroed)[0..16]`.
    pub fn id(&self) -> Id16 {
        let hash = blake3::hash(&self.canonical_zeroed());
        let mut id = [0u8; 16];
        id.copy_from_slice(&hash.as_bytes()[..16]);
        Id16(id)
    }

    /// Derived origin NodeID (doc 03 §3.2).
    pub fn origin_node(&self) -> NodeId {
        node_id_of(&self.origin)
    }

    pub fn parents(&self) -> &[Id16] {
        for ext in &self.exts {
            if let Ext::Parents(p) = ext {
                return p;
            }
        }
        &[]
    }

    /// Strict canonical decode (doc 10 §6.1). Does NOT verify the signature —
    /// that is `mmp-crypto`'s job; ingest (doc 03 §4.1.2) sequences both.
    pub fn decode_canonical(bytes: &[u8]) -> Result<Bundle, WireError> {
        if bytes.len() > MAX_BUNDLE {
            return Err(WireError::Oversize);
        }
        let mut r = Reader::new(bytes);
        let ver_class = r.u8()?;
        if ver_class >> 4 != WIRE_VERSION {
            return Err(WireError::BadVersion);
        }
        let class = ver_class & 0x0f;
        let flags = r.u8()?;
        if flags & !FLAGS_KNOWN != 0 {
            return Err(WireError::ReservedBits);
        }
        let origin = r.key32()?;
        let dest = r.id16()?;
        let lifetime_ms = r.u40()?;
        let priority = r.u8()?;
        if priority > 3 {
            return Err(WireError::BadValue("priority"));
        }
        let ext_len = r.u16()? as usize;
        let ext_bytes = r.take(ext_len)?;
        let exts = decode_exts(ext_bytes)?;
        let payload = r.varbytes(MAX_BUNDLE)?.to_vec();
        let signature = r.sig64()?;
        r.finish()?;
        // Canonical single-encoding: the has_parents flag mirrors the TLV.
        let has_parents = exts.iter().any(|e| e.typ() == TLV_PARENTS);
        if has_parents != (flags & FLAG_HAS_PARENTS != 0) {
            return Err(WireError::BadValue("has_parents flag"));
        }
        Ok(Bundle { class, flags, origin, dest, lifetime_ms, priority, exts, payload, signature })
    }

    /// True if any extension is critical-unknown per this implementation —
    /// decode already rejects those, so a decoded bundle never has them; kept
    /// for unknown-class relaying decisions (doc 03 §8).
    pub fn has_only_known_critical(&self) -> bool {
        true
    }
}

/// Zero the 5 lifetime bytes of an encoded (or unsigned-encoded) bundle in place.
pub fn zero_lifetime(bytes: &mut [u8]) {
    bytes[LIFETIME_OFFSET..LIFETIME_OFFSET + 5].fill(0);
}

/// BundleID straight from wire bytes (avoids a re-encode on the ingest path).
pub fn id_of_encoded(bytes: &[u8]) -> Id16 {
    let mut canon = bytes[..bytes.len() - SIG_LEN].to_vec();
    zero_lifetime(&mut canon);
    let hash = blake3::hash(&canon);
    let mut id = [0u8; 16];
    id.copy_from_slice(&hash.as_bytes()[..16]);
    Id16(id)
}
