//! Claim records (doc 10 §4.4, doc 06 §4) — single-writer LWW registers.

use alloc::vec::Vec;

use crate::consts::PREKEY_BATCH;
use crate::err::WireError;
use crate::ids::{Key32, Sig64};
use crate::wire::{BufExt, Reader};

/// Claim field registry (doc 06 §4.1; 0x07/0x08 are rev-B store-don't-interpret).
pub mod field {
    pub const NICKNAME: u8 = 0x01;
    pub const AVATAR_ROOT: u8 = 0x02;
    pub const EXCHANGE_KEY: u8 = 0x03;
    pub const PREKEY_BATCH: u8 = 0x04;
    pub const FRIENDSHIP: u8 = 0x05;
    pub const GROUP_EPOCH_ANCHOR: u8 = 0x06;
    pub const AUTHCERT: u8 = 0x07;
    pub const REVOCATION_LIST: u8 = 0x08;
}

/// `owner key32, field u8, seq u32, body varbytes, sig64` — signed under
/// `mmp1/claim` over `owner ‖ field ‖ seq ‖ body` (doc 10 §3.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claim {
    pub owner: Key32,
    pub field: u8,
    pub seq: u32,
    pub body: Vec<u8>,
    pub sig: Sig64,
}

impl Claim {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(32 + 1 + 4 + 2 + self.body.len() + 64);
        out.extend_from_slice(&self.owner.0);
        out.push(self.field);
        out.put_u32(self.seq);
        out.put_var(&self.body);
        out.extend_from_slice(&self.sig.0);
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Claim, WireError> {
        let mut r = Reader::new(bytes);
        let claim = Claim {
            owner: r.key32()?,
            field: r.u8()?,
            seq: r.u32()?,
            body: r.varbytes(crate::consts::MAX_BUNDLE)?.to_vec(),
            sig: r.sig64()?,
        };
        r.finish()?;
        Ok(claim)
    }

    /// The `object_bytes` under the `mmp1/claim` domain tag.
    pub fn signed_object(&self) -> Vec<u8> {
        let mut o = Vec::with_capacity(32 + 1 + 4 + self.body.len());
        o.extend_from_slice(&self.owner.0);
        o.push(self.field);
        o.put_u32(self.seq);
        o.extend_from_slice(&self.body);
        o
    }
}

/// Prekey batch claim body (doc 10 §4.4, doc 08 §4.2):
/// `signed_prekey key32, spk_sig sig64, count u8 × key32`.
/// `spk_sig` is under `mmp1/prekey` over `signed_prekey ‖ count ‖ prekeys…`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrekeyBatch {
    pub signed_prekey: Key32,
    pub spk_sig: Sig64,
    pub prekeys: Vec<Key32>,
}

impl PrekeyBatch {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(32 + 64 + 1 + 32 * self.prekeys.len());
        out.extend_from_slice(&self.signed_prekey.0);
        out.extend_from_slice(&self.spk_sig.0);
        out.push(self.prekeys.len() as u8);
        for pk in &self.prekeys {
            out.extend_from_slice(&pk.0);
        }
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<PrekeyBatch, WireError> {
        let mut r = Reader::new(bytes);
        let signed_prekey = r.key32()?;
        let spk_sig = r.sig64()?;
        let count = r.u8()? as usize;
        if count > PREKEY_BATCH {
            return Err(WireError::Oversize);
        }
        let mut prekeys = Vec::with_capacity(count);
        for _ in 0..count {
            prekeys.push(r.key32()?);
        }
        r.finish()?;
        Ok(PrekeyBatch { signed_prekey, spk_sig, prekeys })
    }

    /// The `object_bytes` under the `mmp1/prekey` domain tag.
    pub fn signed_object(&self) -> Vec<u8> {
        let mut o = Vec::with_capacity(32 + 1 + 32 * self.prekeys.len());
        o.extend_from_slice(&self.signed_prekey.0);
        o.push(self.prekeys.len() as u8);
        for pk in &self.prekeys {
            o.extend_from_slice(&pk.0);
        }
        o
    }
}

/// Group epoch anchor claim body (doc 06 §8.3): `gid id16, epoch u16, member-set root [32]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EpochAnchor {
    pub gid: crate::ids::Id16,
    pub epoch: u16,
    pub member_root: [u8; 32],
}

impl EpochAnchor {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(16 + 2 + 32);
        out.extend_from_slice(&self.gid.0);
        out.put_u16(self.epoch);
        out.extend_from_slice(&self.member_root);
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<EpochAnchor, WireError> {
        let mut r = Reader::new(bytes);
        let gid = r.id16()?;
        let epoch = r.u16()?;
        let root = r.take(32)?;
        r.finish()?;
        let mut member_root = [0u8; 32];
        member_root.copy_from_slice(root);
        Ok(EpochAnchor { gid, epoch, member_root })
    }
}
