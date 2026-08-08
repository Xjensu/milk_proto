//! Claim records (doc 10 §4.4, doc 06 §4) — single-writer LWW registers.

use alloc::vec::Vec;

use crate::consts::PREKEY_BATCH;
use crate::err::WireError;
use crate::ids::{Key32, Sig64};
use crate::wire::{BufExt, Reader};

/// Claim field registry (doc 06 §4.1; 0x07/0x08 are rev-B store-don't-interpret).
pub mod field {
    /// Display nickname claim body (raw UTF-8 bytes, capped by `NICKNAME_MAX`).
    pub const NICKNAME: u8 = 0x01;
    /// Content-addressed avatar root claim body.
    pub const AVATAR_ROOT: u8 = 0x02;
    /// X25519 exchange (key-agreement) public key claim body.
    pub const EXCHANGE_KEY: u8 = 0x03;
    /// [`super::PrekeyBatch`] claim body.
    pub const PREKEY_BATCH: u8 = 0x04;
    /// [`super::Friendship`] claim body.
    pub const FRIENDSHIP: u8 = 0x05;
    /// Group epoch anchor claim body (`super::EpochAnchor`).
    pub const GROUP_EPOCH_ANCHOR: u8 = 0x06;
    /// `crate::authcert::AuthCert` claim body (rev-B: store-don't-interpret).
    pub const AUTHCERT: u8 = 0x07;
    /// `crate::authcert::RevocationList` claim body (rev-B: store-don't-interpret).
    pub const REVOCATION_LIST: u8 = 0x08;
    /// General identity revocation + succession (doc 06 §4.1, handbook §4.6,
    /// E5-S3). Distinct from `REVOCATION_LIST` (0x08, doc 18 §6): that field
    /// revokes an AuthCert *grant*; this one revokes the *identity itself*.
    pub const REVOCATION: u8 = 0x09;
}

/// `owner key32, field u8, seq u32, body varbytes, sig64` — signed under
/// `mmp1/claim` over `owner ‖ field ‖ seq ‖ body` (doc 10 §3.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claim {
    /// The claim's single-writer identity — the [`field`] register belongs to
    /// this key, and this key's signature (`sig`) is what authenticates it.
    pub owner: Key32,
    /// Which [`field`] register this claim updates.
    pub field: u8,
    /// Monotonic sequence number; higher `seq` wins on merge (LWW).
    pub seq: u32,
    /// The field-specific body bytes (format depends on `field`).
    pub body: Vec<u8>,
    /// Owner's signature over [`Claim::signed_object`] under `SigDomain::Claim`.
    pub sig: Sig64,
}

impl Claim {
    /// Encode to `owner ‖ field ‖ seq ‖ body(varbytes) ‖ sig`.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(32 + 1 + 4 + 2 + self.body.len() + 64);
        out.extend_from_slice(&self.owner.0);
        out.push(self.field);
        out.put_u32(self.seq);
        out.put_var(&self.body);
        out.extend_from_slice(&self.sig.0);
        out
    }

    /// Strict canonical decode of the wire form produced by [`Claim::encode`].
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
    /// The medium-term signed prekey (X3DH), signed by the identity key.
    pub signed_prekey: Key32,
    /// Signature over `signed_prekey ‖ count ‖ prekeys…`, proving the identity
    /// key vouches for `signed_prekey`.
    pub spk_sig: Sig64,
    /// One-time prekeys offered for first-contact key establishment
    /// (doc 08 §4.2), capped at [`crate::consts::PREKEY_BATCH`].
    pub prekeys: Vec<Key32>,
}

impl PrekeyBatch {
    /// Encode to `signed_prekey ‖ spk_sig ‖ count(u8) ‖ prekeys…`.
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

    /// Strict decode; rejects a `count` exceeding [`crate::consts::PREKEY_BATCH`].
    pub fn decode(bytes: &[u8]) -> Result<PrekeyBatch, WireError> {
        let mut r = Reader::new(bytes);
        let signed_prekey = r.key32()?;
        let spk_sig = r.sig64()?;
        let count = r.u8()? as usize;
        if count > PREKEY_BATCH {
            return Err(WireError::Oversize);
        }
        if count > r.remaining() / 32 {
            return Err(WireError::Truncated);
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
    /// The group this epoch anchor belongs to.
    pub gid: crate::ids::Id16,
    /// The sender-key epoch this anchor commits to.
    pub epoch: u16,
    /// Root committing to the epoch's member set.
    pub member_root: [u8; 32],
}

impl EpochAnchor {
    /// Encode to `gid ‖ epoch ‖ member_root`.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(16 + 2 + 32);
        out.extend_from_slice(&self.gid.0);
        out.put_u16(self.epoch);
        out.extend_from_slice(&self.member_root);
        out
    }

    /// Strict decode of the wire form produced by [`EpochAnchor::encode`].
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

/// Identity revocation & succession claim body (claim field `0x09`, doc 06
/// §4.1, handbook §4.6): `revoked_key: key32, reason: u8, has_successor: u8
/// (0x00/0x01), [successor: key32, successor_countersig: sig64]` — the last
/// two fields are present iff `has_successor == 0x01`; any other flag byte
/// value is rejected (strict parsing, doc 10 §6.1's "one valid encoding"
/// rule). Signed by the REVOKED identity as the outer [`Claim`]'s owner,
/// under the ordinary `mmp1/claim` domain (`Claim::signed_object`) — this
/// struct is only the tombstone body. The succession *counter*-signature is a
/// separate signature, by the SUCCESSOR key, over
/// [`succession_signed_object`], also under `mmp1/claim`; verifying it is
/// `mmp-core`'s job (stateful — needs the successor's own public key), not
/// this crate's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Revocation {
    /// The identity key being revoked — signed (as the outer [`Claim`]'s
    /// `owner`) by this same key.
    pub revoked_key: Key32,
    /// Caller-defined revocation reason code; not interpreted by this crate.
    pub reason: u8,
    /// `None` unless a valid, counter-signed successor accompanies this
    /// revocation. Present/absent together with `successor_countersig`.
    pub successor: Option<Key32>,
    /// Successor's counter-signature over [`succession_signed_object`],
    /// present iff `successor` is `Some`.
    pub successor_countersig: Option<Sig64>,
}

impl Revocation {
    /// Encode to `revoked_key ‖ reason ‖ has_successor(0x00/0x01) [‖
    /// successor ‖ successor_countersig]`.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(32 + 1 + 1 + 32 + 64);
        out.extend_from_slice(&self.revoked_key.0);
        out.push(self.reason);
        match (self.successor, self.successor_countersig) {
            (Some(successor), Some(sig)) => {
                out.push(0x01);
                out.extend_from_slice(&successor.0);
                out.extend_from_slice(&sig.0);
            }
            _ => out.push(0x00),
        }
        out
    }

    /// Strict decode; rejects any `has_successor` flag byte other than
    /// `0x00`/`0x01` (doc 10 §6.1's one-valid-encoding rule).
    pub fn decode(bytes: &[u8]) -> Result<Revocation, WireError> {
        let mut r = Reader::new(bytes);
        let revoked_key = r.key32()?;
        let reason = r.u8()?;
        let has_successor = r.u8()?;
        let (successor, successor_countersig) = match has_successor {
            0x00 => (None, None),
            0x01 => (Some(r.key32()?), Some(r.sig64()?)),
            _ => return Err(WireError::BadValue("revocation.has_successor")),
        };
        r.finish()?;
        Ok(Revocation { revoked_key, reason, successor, successor_countersig })
    }
}

/// Friendship claim body (claim field `0x05`, doc 04 §4.1, doc 06 §4.1,
/// handbook §4.5): `friend key32, since u64, countersig sig64` — the outer
/// [`Claim`]'s `owner` is the DEPENDENT; `friend` is the PATRON. `countersig`
/// is the patron's Ed25519 signature, under `SigDomain::Friend`, over
/// [`friendship_signed_object`] — proof the patron actually agreed to buffer
/// for `owner`, not merely that `owner` claims so.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Friendship {
    /// The PATRON's identity key — the outer [`Claim`]'s `owner` is the
    /// dependent.
    pub friend: Key32,
    /// Timestamp the friendship began, per [`friendship_signed_object`].
    pub since: u64,
    /// The patron's signature (`SigDomain::Friend`) over
    /// [`friendship_signed_object`], proving the patron agreed to buffer.
    pub countersig: Sig64,
}

impl Friendship {
    /// Encode to `friend ‖ since ‖ countersig`.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(32 + 8 + 64);
        out.extend_from_slice(&self.friend.0);
        out.put_u64(self.since);
        out.extend_from_slice(&self.countersig.0);
        out
    }

    /// Strict decode of the wire form produced by [`Friendship::encode`].
    pub fn decode(bytes: &[u8]) -> Result<Friendship, WireError> {
        let mut r = Reader::new(bytes);
        let friendship =
            Friendship { friend: r.key32()?, since: r.u64()?, countersig: r.sig64()? };
        r.finish()?;
        Ok(friendship)
    }
}

/// The 72-byte countersig preimage for a friendship, signed by the PATRON
/// (`friend`) under `SigDomain::Friend`: `owner ‖ friend ‖ since`, with
/// `since` as a fixed-width 8-byte big-endian integer — deliberately NOT the
/// crate's usual little-endian wire convention ([`crate::wire::BufExt`]):
/// this is a signed preimage, not a wire encoding, so it doesn't inherit that
/// convention, and a fixed explicit layout avoids ever conflating the two.
/// Shared between the FRIEND_OFFER/FRIEND_ACK handshake (session layer,
/// `mmp-core`/`mmp-node`) and this claim body's own countersig check
/// (`ClaimStore::merge`) — sign and verify must always call this same
/// function.
pub fn friendship_signed_object(owner: &Key32, friend: &Key32, since: u64) -> Vec<u8> {
    let mut o = Vec::with_capacity(72);
    o.extend_from_slice(&owner.0);
    o.extend_from_slice(&friend.0);
    o.extend_from_slice(&since.to_be_bytes());
    o
}

/// The `object_bytes` for a succession counter-signature under `mmp1/claim`
/// (signed by the SUCCESSOR key — distinct from the outer [`Claim`]'s owner
/// signature, which is by the REVOKED key): `revoked_key ‖ successor`, 64 B.
pub fn succession_signed_object(revoked_key: &Key32, successor: &Key32) -> Vec<u8> {
    let mut o = Vec::with_capacity(64);
    o.extend_from_slice(&revoked_key.0);
    o.extend_from_slice(&successor.0);
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prekey_batch_rejects_count_before_allocating_entries() {
        // The fixed prekey header is complete, but the declared batch has no
        // 32-byte key records following it.
        let mut bytes = [0u8; 32 + 64 + 1];
        bytes[96] = PREKEY_BATCH as u8;
        assert_eq!(PrekeyBatch::decode(&bytes), Err(WireError::Truncated));
    }

    fn revocation_no_successor() -> Revocation {
        Revocation { revoked_key: Key32([1; 32]), reason: 3, successor: None, successor_countersig: None }
    }

    fn revocation_with_successor() -> Revocation {
        Revocation {
            revoked_key: Key32([1; 32]),
            reason: 0,
            successor: Some(Key32([2; 32])),
            successor_countersig: Some(Sig64([9; 64])),
        }
    }

    #[test]
    fn revocation_roundtrip_without_successor() {
        let r = revocation_no_successor();
        let bytes = r.encode();
        assert_eq!(bytes.len(), 32 + 1 + 1); // no successor/countersig tail
        assert_eq!(bytes[33], 0x00);
        assert_eq!(Revocation::decode(&bytes).unwrap(), r);
    }

    #[test]
    fn revocation_roundtrip_with_successor() {
        let r = revocation_with_successor();
        let bytes = r.encode();
        assert_eq!(bytes.len(), 32 + 1 + 1 + 32 + 64);
        assert_eq!(bytes[33], 0x01);
        assert_eq!(Revocation::decode(&bytes).unwrap(), r);
    }

    #[test]
    fn revocation_decode_rejects_bad_flag_byte() {
        let mut bytes = revocation_no_successor().encode();
        bytes[33] = 0x02; // neither 0x00 nor 0x01
        assert_eq!(Revocation::decode(&bytes), Err(WireError::BadValue("revocation.has_successor")));
    }

    #[test]
    fn revocation_decode_rejects_truncated_body() {
        // Flag says a successor follows, but the bytes stop short of it.
        let mut bytes = revocation_no_successor().encode();
        bytes[33] = 0x01;
        assert_eq!(Revocation::decode(&bytes), Err(WireError::Truncated));

        // Also truncated before even the fixed revoked_key/reason/flag prefix.
        assert_eq!(Revocation::decode(&[0u8; 10]), Err(WireError::Truncated));
    }

    #[test]
    fn revocation_decode_rejects_trailing_bytes() {
        let mut bytes = revocation_with_successor().encode();
        bytes.push(0);
        assert_eq!(Revocation::decode(&bytes), Err(WireError::Trailing));
    }

    #[test]
    fn friendship_roundtrip() {
        let f = Friendship { friend: Key32([2; 32]), since: 1_700_000_000, countersig: Sig64([3; 64]) };
        let bytes = f.encode();
        assert_eq!(bytes.len(), 32 + 8 + 64);
        assert_eq!(Friendship::decode(&bytes).unwrap(), f);
    }

    #[test]
    fn friendship_decode_rejects_truncated_body() {
        let bytes = Friendship { friend: Key32([2; 32]), since: 1, countersig: Sig64([3; 64]) }.encode();
        assert_eq!(Friendship::decode(&bytes[..bytes.len() - 1]), Err(WireError::Truncated));
    }

    #[test]
    fn friendship_decode_rejects_trailing_bytes() {
        let mut bytes = Friendship { friend: Key32([2; 32]), since: 1, countersig: Sig64([3; 64]) }.encode();
        bytes.push(0);
        assert_eq!(Friendship::decode(&bytes), Err(WireError::Trailing));
    }

    #[test]
    fn friendship_signed_object_is_owner_friend_since_be() {
        let owner = Key32([5; 32]);
        let friend = Key32([6; 32]);
        let pre = friendship_signed_object(&owner, &friend, 0x0102_0304_0506_0708);
        assert_eq!(pre.len(), 72);
        assert_eq!(&pre[..32], &owner.0);
        assert_eq!(&pre[32..64], &friend.0);
        assert_eq!(&pre[64..], &[1, 2, 3, 4, 5, 6, 7, 8]); // big-endian
    }

    #[test]
    fn succession_signed_object_is_revoked_key_then_successor() {
        let revoked = Key32([7; 32]);
        let successor = Key32([8; 32]);
        let pre = succession_signed_object(&revoked, &successor);
        assert_eq!(pre.len(), 64);
        assert_eq!(&pre[..32], &revoked.0);
        assert_eq!(&pre[32..], &successor.0);
    }
}
