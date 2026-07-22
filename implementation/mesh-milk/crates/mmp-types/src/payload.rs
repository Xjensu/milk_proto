//! Per-class bundle payload schemas (doc 10 §4).
//!
//! Class 1/2 carry AEAD ciphertext; the addressing fields (conv/epoch/index)
//! ride in clear inside the payload so the receiver can select the ratchet key,
//! while the body — including the doc-16 apptype byte — is inside the ciphertext.

use alloc::vec::Vec;

use crate::consts::{MAX_BUNDLE, SYMBOL_SIZE};
use crate::err::WireError;
use crate::ids::{Id16, Key32, Sig64};
use crate::wire::{BufExt, Reader};

/// Class 1 direct message: `conv id16, ratchet_epoch u16, msg_index u32, ct varbytes`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectPayload {
    pub conv: Id16,
    pub epoch: u16,
    pub index: u32,
    pub ct: Vec<u8>,
}

impl DirectPayload {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(16 + 2 + 4 + 2 + self.ct.len());
        out.extend_from_slice(&self.conv.0);
        out.put_u16(self.epoch);
        out.put_u32(self.index);
        out.put_var(&self.ct);
        out
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, WireError> {
        let mut r = Reader::new(bytes);
        let p = DirectPayload {
            conv: r.id16()?,
            epoch: r.u16()?,
            index: r.u32()?,
            ct: r.varbytes(MAX_BUNDLE)?.to_vec(),
        };
        r.finish()?;
        Ok(p)
    }
}

/// Class 1 async first-contact form (flags bit2, doc 10 §4.1):
/// sealed box or prekey-upgraded X3DH (doc 08 §4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AsyncPayload {
    /// `mode 0x00, eph key32, ct varbytes`.
    Sealed { eph: Key32, ct: Vec<u8> },
    /// `mode 0x01, prekey_id u16, sender_xpk key32, eph key32, ct varbytes`.
    /// `sender_xpk` is the sender's static exchange key, carried in clear (as
    /// in X3DH's first message) because the receiver needs it for the DH set
    /// before it can decrypt — it is public claim data anyway (doc 08 §4.2).
    Prekey { prekey_id: u16, sender_xpk: Key32, eph: Key32, ct: Vec<u8> },
}

impl AsyncPayload {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        match self {
            AsyncPayload::Sealed { eph, ct } => {
                out.push(0x00);
                out.extend_from_slice(&eph.0);
                out.put_var(ct);
            }
            AsyncPayload::Prekey { prekey_id, sender_xpk, eph, ct } => {
                out.push(0x01);
                out.put_u16(*prekey_id);
                out.extend_from_slice(&sender_xpk.0);
                out.extend_from_slice(&eph.0);
                out.put_var(ct);
            }
        }
        out
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, WireError> {
        let mut r = Reader::new(bytes);
        let p = match r.u8()? {
            0x00 => AsyncPayload::Sealed { eph: r.key32()?, ct: r.varbytes(MAX_BUNDLE)?.to_vec() },
            0x01 => AsyncPayload::Prekey {
                prekey_id: r.u16()?,
                sender_xpk: r.key32()?,
                eph: r.key32()?,
                ct: r.varbytes(MAX_BUNDLE)?.to_vec(),
            },
            _ => return Err(WireError::BadValue("async mode")),
        };
        r.finish()?;
        Ok(p)
    }
}

/// Class 2 group message (doc 10 §4.2):
/// `gid id16, epoch u16, member id16, chain_index u32, ct varbytes, member_sig sig64`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupPayload {
    pub gid: Id16,
    pub epoch: u16,
    pub member: Id16,
    pub chain_index: u32,
    pub ct: Vec<u8>,
    pub member_sig: Sig64,
}

impl GroupPayload {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(16 + 2 + 16 + 4 + 2 + self.ct.len() + 64);
        out.extend_from_slice(&self.gid.0);
        out.put_u16(self.epoch);
        out.extend_from_slice(&self.member.0);
        out.put_u32(self.chain_index);
        out.put_var(&self.ct);
        out.extend_from_slice(&self.member_sig.0);
        out
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, WireError> {
        let mut r = Reader::new(bytes);
        let p = GroupPayload {
            gid: r.id16()?,
            epoch: r.u16()?,
            member: r.id16()?,
            chain_index: r.u32()?,
            ct: r.varbytes(MAX_BUNDLE)?.to_vec(),
            member_sig: r.sig64()?,
        };
        r.finish()?;
        Ok(p)
    }
    /// The `object_bytes` under the `mmp1/groupmsg` domain tag.
    pub fn signed_object(&self) -> Vec<u8> {
        let mut o = Vec::with_capacity(16 + 2 + 16 + 4 + self.ct.len());
        o.extend_from_slice(&self.gid.0);
        o.put_u16(self.epoch);
        o.extend_from_slice(&self.member.0);
        o.put_u32(self.chain_index);
        o.extend_from_slice(&self.ct);
        o
    }
}

/// Class 0 receipt (doc 10 §4.3): `delivered id16, mac [u8;16]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReceiptPayload {
    pub delivered: Id16,
    pub mac: [u8; 16],
}

impl ReceiptPayload {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(32);
        out.extend_from_slice(&self.delivered.0);
        out.extend_from_slice(&self.mac);
        out
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, WireError> {
        let mut r = Reader::new(bytes);
        let delivered = r.id16()?;
        let mac_bytes = r.take(16)?;
        r.finish()?;
        let mut mac = [0u8; 16];
        mac.copy_from_slice(mac_bytes);
        Ok(ReceiptPayload { delivered, mac })
    }
}

/// Body-interior application type tags (doc 16 §3.1).
pub mod apptype {
    pub const TEXT: u8 = 0x00;
    pub const FILE_MANIFEST: u8 = 0x01;
    /// Group sender-key distribution, sent over the pairwise channel
    /// (doc 08 §6.1; implementation-assigned tag, invisible to relays).
    pub const SENDER_KEY: u8 = 0x02;
}

/// Class-4 (`class::SYMBOL`) bundle payload (E5-S1 decision D1): the Merkle
/// inclusion path for this symbol, then the symbol bytes themselves.
/// Positioning (root/stripe/index-in-stripe) lives ONLY in the sibling
/// `Ext::SymbolInfo` TLV — never duplicated here — so this struct carries
/// exactly what SYMBOL_INFO cannot: the variable-length proof.
///
/// Wire: `u16 path_len(LE) ‖ MerklePath wire bytes (path_len B) ‖ symbol
/// bytes (exactly SYMBOL_SIZE)`. `path` is kept as opaque bytes here (not
/// `mmp_crypto::merkle::MerklePath`) because `mmp-types` sits below
/// `mmp-crypto` in the layering — the codec for the path's *contents* lives
/// next to the type in `mmp-crypto/src/merkle.rs`; this is only the outer
/// framing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolPayload {
    pub path: Vec<u8>,
    pub symbol: Vec<u8>,
}

/// Cap on `path_len` before any allocation (§9.3 discipline): a Merkle path
/// at the maximum depth (`MERKLE_PATH_MAX_LEVELS = 8`, `mmp-crypto`) encodes
/// to at most `1 + 8*(2+3*32) = 785` bytes; 1024 leaves comfortable headroom
/// without opening an unbounded-allocation avenue for attacker input.
const SYMBOL_PATH_MAX: usize = 1024;

impl SymbolPayload {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(2 + self.path.len() + self.symbol.len());
        out.put_var(&self.path);
        out.extend_from_slice(&self.symbol);
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<SymbolPayload, WireError> {
        let mut r = Reader::new(bytes);
        let path = r.varbytes(SYMBOL_PATH_MAX)?.to_vec();
        let symbol = r.take(SYMBOL_SIZE)?.to_vec();
        r.finish()?;
        Ok(SymbolPayload { path, symbol })
    }
}
