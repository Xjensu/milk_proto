//! Canonical group-control and bootstrap wire objects.
//!
//! These structures deliberately contain no policy or cryptographic provider
//! code.  They only establish an unambiguous byte representation; signature
//! verification, administrator-slot authorization and epoch transitions are
//! performed by `mmp-crypto`/`mmp-core`.

use alloc::vec::Vec;

use blake3::Hasher;

use crate::err::WireError;
use crate::ids::{node_id_of, Id16, Key32, Sig64};
use crate::wire::{BufExt, Reader};

/// Maximum immutable administrator entries in a group welcome.
pub const MAX_ADMINS: usize = 32;
/// Maximum observed tags in one REMOVE operation.
pub const MAX_REMOVE_TAGS: usize = 1024;
/// Maximum operations in one control batch.
pub const MAX_CONTROL_OPS: usize = 256;
/// Maximum sender-key grants/distributions in one welcome.
pub const MAX_WELCOME_DISTS: usize = 32;

/// Immutable administrator identity binding.
///
/// Slots are assigned by ascending `admin_node_id` (slot zero is the
/// deterministic coordinator).  Keeping the wire object independent of a
/// slot avoids duplicate representations when an admin list is re-encoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdminKey {
    /// Node identifier derived from `admin_identity_pubkey`.
    pub admin_node_id: Id16,
    /// Ed25519 identity public key.
    pub admin_identity_pubkey: Key32,
}

/// Name used by the specification for an immutable administrator entry.
pub type AdminInfo = AdminKey;

impl AdminKey {
    /// Canonical fixed-width encoding (`node_id || identity_pubkey`).
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(48);
        out.extend_from_slice(&self.admin_node_id.0);
        out.extend_from_slice(&self.admin_identity_pubkey.0);
        out
    }

    /// Strictly decode and validate the NodeID/public-key binding.
    pub fn decode(bytes: &[u8]) -> Result<Self, WireError> {
        let mut r = Reader::new(bytes);
        let admin_node_id = r.id16()?;
        let admin_identity_pubkey = r.key32()?;
        r.finish()?;
        if node_id_of(&admin_identity_pubkey) != admin_node_id {
            return Err(WireError::BadValue("admin node id"));
        }
        Ok(Self {
            admin_node_id,
            admin_identity_pubkey,
        })
    }
}

/// Globally unique OR-Set add tag packed as `(admin_slot << 48) | counter`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AddTag {
    /// Immutable administrator slot that issued the tag.
    pub admin_slot: u16,
    /// Per-slot monotonic counter in the inclusive range `1..2^48-1`.
    pub counter: u64,
}

impl AddTag {
    /// Largest valid per-slot counter.
    pub const MAX_COUNTER: u64 = (1u64 << 48) - 1;

    /// Construct a tag, rejecting zero/overflow counters.
    pub const fn new(admin_slot: u16, counter: u64) -> Option<Self> {
        if counter == 0 || counter > Self::MAX_COUNTER {
            None
        } else {
            Some(Self {
                admin_slot,
                counter,
            })
        }
    }

    /// Return the compact wire value.
    pub const fn packed(self) -> u64 {
        ((self.admin_slot as u64) << 48) | self.counter
    }

    /// Canonical fixed-width encoding.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(8);
        out.put_u64(self.packed());
        out
    }

    /// Strictly decode a packed tag.
    pub fn decode(bytes: &[u8]) -> Result<Self, WireError> {
        let mut r = Reader::new(bytes);
        let packed = r.u64()?;
        r.finish()?;
        let admin_slot = (packed >> 48) as u16;
        let counter = packed & Self::MAX_COUNTER;
        Self::new(admin_slot, counter).ok_or(WireError::BadValue("add tag counter"))
    }
}

/// Signed administrator membership operation (`kind`: 0 ADD, 1 REMOVE).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedMembershipOp {
    /// Candidate codec version; must be one.
    pub version: u8,
    /// Group identifier.
    pub gid: Id16,
    /// Epoch against which this operation was authored.
    pub base_epoch: u16,
    /// BLAKE3-128 of the unsigned operation object.
    pub op_id: Id16,
    /// `0` for ADD, `1` for REMOVE.
    pub kind: u8,
    /// Member affected by ADD (zero for REMOVE).
    pub member: Id16,
    /// Tag affected by ADD (sentinel for REMOVE).
    pub tag: AddTag,
    /// Tags observed by REMOVE, strictly ascending.
    pub remove_tags: Vec<AddTag>,
    /// Administrator Ed25519 public key.
    pub signer_pubkey: Key32,
    /// Signature over [`Self::signed_object`] in `SigDomain::GroupControl`.
    pub signature: Sig64,
}

impl SignedMembershipOp {
    fn unsigned_object(&self, include_op_id: bool) -> Vec<u8> {
        let mut out =
            Vec::with_capacity(1 + 16 + 2 + 16 + 1 + 16 + 8 + 2 + self.remove_tags.len() * 8 + 32);
        out.push(self.version);
        out.extend_from_slice(&self.gid.0);
        out.put_u16(self.base_epoch);
        if include_op_id {
            out.extend_from_slice(&self.op_id.0);
        }
        out.push(self.kind);
        match self.kind {
            0 => {
                out.extend_from_slice(&self.member.0);
                out.extend_from_slice(&self.tag.encode());
            }
            1 => {
                out.put_u16(self.remove_tags.len() as u16);
                for tag in &self.remove_tags {
                    out.extend_from_slice(&tag.encode());
                }
            }
            _ => {}
        }
        out.extend_from_slice(&self.signer_pubkey.0);
        out
    }

    /// Bytes hashed to derive `op_id` (all unsigned fields except `op_id`).
    pub fn op_id_object(&self) -> Vec<u8> {
        self.unsigned_object(false)
    }

    /// Compute the canonical operation identifier.
    pub fn compute_op_id(&self) -> Id16 {
        let digest = blake3::hash(&self.op_id_object());
        let mut id = [0u8; 16];
        id.copy_from_slice(&digest.as_bytes()[..16]);
        Id16(id)
    }

    /// Signature object: all fields except the signature itself.
    pub fn signed_object(&self) -> Vec<u8> {
        self.unsigned_object(true)
    }

    /// Canonical encoding including the signature.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = self.signed_object();
        out.extend_from_slice(&self.signature.0);
        out
    }

    fn decode_from(r: &mut Reader<'_>) -> Result<Self, WireError> {
        let version = r.u8()?;
        if version != 1 {
            return Err(WireError::BadVersion);
        }
        let gid = r.id16()?;
        let base_epoch = r.u16()?;
        if base_epoch == 0 {
            return Err(WireError::BadValue("membership base epoch"));
        }
        let op_id = r.id16()?;
        let kind = r.u8()?;
        let (member, tag, remove_tags) = match kind {
            0 => (r.id16()?, AddTag::decode(r.take(8)?)?, Vec::new()),
            1 => {
                let count = r.u16()? as usize;
                if count == 0 || count > MAX_REMOVE_TAGS {
                    return Err(WireError::BadValue("remove tag count"));
                }
                if r.remaining() < count.saturating_mul(8).saturating_add(32 + 64) {
                    return Err(WireError::Truncated);
                }
                let mut tags = Vec::with_capacity(count);
                for _ in 0..count {
                    let t = AddTag::decode(r.take(8)?)?;
                    if tags.last().is_some_and(|last| *last >= t) {
                        return Err(WireError::TlvOrder);
                    }
                    tags.push(t);
                }
                (
                    Id16::MIN,
                    AddTag {
                        admin_slot: 0,
                        counter: 1,
                    },
                    tags,
                )
            }
            _ => return Err(WireError::BadValue("membership kind")),
        };
        let signer_pubkey = r.key32()?;
        let signature = r.sig64()?;
        let op = Self {
            version,
            gid,
            base_epoch,
            op_id,
            kind,
            member,
            tag,
            remove_tags,
            signer_pubkey,
            signature,
        };
        if op.compute_op_id() != op_id {
            return Err(WireError::BadValue("membership op id"));
        }
        Ok(op)
    }

    /// Strictly decode a complete operation.
    pub fn decode(bytes: &[u8]) -> Result<Self, WireError> {
        let mut r = Reader::new(bytes);
        let op = Self::decode_from(&mut r)?;
        r.finish()?;
        Ok(op)
    }
}

/// Extended epoch anchor committing membership and causal-control state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EpochAnchor {
    /// Group identifier.
    pub gid: Id16,
    /// Sender-key epoch (non-zero).
    pub epoch: u16,
    /// BLAKE3 root over canonical live `(member, tag)` entries, tombstones and frontier.
    pub state_root: [u8; 32],
    /// Hash of the preceding anchor, or zero for the initial anchor.
    pub previous_anchor_hash: [u8; 32],
    /// Hash of the signed-control frontier incorporated by this anchor.
    pub control_frontier_hash: [u8; 32],
    /// Deterministic coordinator slot (lowest immutable admin slot).
    pub coordinator_slot: u16,
}

impl EpochAnchor {
    /// Fixed canonical body length.
    pub const ENCODED_LEN: usize = 16 + 2 + 32 + 32 + 32 + 2;

    /// Canonical encoding.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(Self::ENCODED_LEN);
        out.extend_from_slice(&self.gid.0);
        out.put_u16(self.epoch);
        out.extend_from_slice(&self.state_root);
        out.extend_from_slice(&self.previous_anchor_hash);
        out.extend_from_slice(&self.control_frontier_hash);
        out.put_u16(self.coordinator_slot);
        out
    }

    /// BLAKE3 hash used to link the next anchor.
    pub fn hash(&self) -> [u8; 32] {
        *blake3::hash(&self.encode()).as_bytes()
    }

    /// Strictly decode the fixed body.
    pub fn decode(bytes: &[u8]) -> Result<Self, WireError> {
        let mut r = Reader::new(bytes);
        let anchor = Self::decode_from(&mut r)?;
        r.finish()?;
        Ok(anchor)
    }

    fn decode_from(r: &mut Reader<'_>) -> Result<Self, WireError> {
        let gid = r.id16()?;
        let epoch = r.u16()?;
        if epoch == 0 {
            return Err(WireError::BadValue("anchor epoch"));
        }
        let mut state_root = [0; 32];
        state_root.copy_from_slice(r.take(32)?);
        let mut previous_anchor_hash = [0; 32];
        previous_anchor_hash.copy_from_slice(r.take(32)?);
        let mut control_frontier_hash = [0; 32];
        control_frontier_hash.copy_from_slice(r.take(32)?);
        let coordinator_slot = r.u16()?;
        Ok(Self {
            gid,
            epoch,
            state_root,
            previous_anchor_hash,
            control_frontier_hash,
            coordinator_slot,
        })
    }
}

/// Coordinator signature over an epoch anchor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpochAnchorSignature {
    /// Signed anchor value.
    pub anchor: EpochAnchor,
    /// Immutable slot of the signing administrator.
    pub signer_slot: u16,
    /// Signing administrator identity key.
    pub signer_pubkey: Key32,
    /// Signature under the group-control domain.
    pub signature: Sig64,
}

impl EpochAnchorSignature {
    /// Bytes covered by the signature.
    pub fn signed_object(&self) -> Vec<u8> {
        let mut out = self.anchor.encode();
        out.put_u16(self.signer_slot);
        out.extend_from_slice(&self.signer_pubkey.0);
        out
    }

    /// Canonical encoding.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = self.signed_object();
        out.extend_from_slice(&self.signature.0);
        out
    }

    /// Strict decode.
    pub fn decode(bytes: &[u8]) -> Result<Self, WireError> {
        let mut r = Reader::new(bytes);
        let anchor = EpochAnchor::decode_from(&mut r)?;
        let signer_slot = r.u16()?;
        let signer_pubkey = r.key32()?;
        let signature = r.sig64()?;
        r.finish()?;
        Ok(Self {
            anchor,
            signer_slot,
            signer_pubkey,
            signature,
        })
    }
}

/// Administrator acknowledgement for an epoch anchor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnchorAck {
    /// Group and epoch acknowledged.
    pub anchor: EpochAnchor,
    /// Immutable slot of the acknowledging administrator.
    pub signer_slot: u16,
    /// Acknowledging administrator identity key.
    pub signer_pubkey: Key32,
    /// Signature over the anchor and signer slot/key.
    pub signature: Sig64,
}

impl AnchorAck {
    /// Bytes covered by the acknowledgement signature.
    pub fn signed_object(&self) -> Vec<u8> {
        EpochAnchorSignature {
            anchor: self.anchor,
            signer_slot: self.signer_slot,
            signer_pubkey: self.signer_pubkey,
            signature: self.signature,
        }
        .signed_object()
    }

    /// Canonical encoding.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = self.signed_object();
        out.extend_from_slice(&self.signature.0);
        out
    }

    /// Strict decoding.
    pub fn decode(bytes: &[u8]) -> Result<Self, WireError> {
        let x = EpochAnchorSignature::decode(bytes)?;
        Ok(Self {
            anchor: x.anchor,
            signer_slot: x.signer_slot,
            signer_pubkey: x.signer_pubkey,
            signature: x.signature,
        })
    }
}

/// Sender-key grant binding a member identity to a group signing key/epoch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SenderKeyGrant {
    /// Group identifier.
    pub gid: Id16,
    /// Authorized epoch (non-zero).
    pub epoch: u16,
    /// Authorized member NodeID.
    pub member: Id16,
    /// Member identity public key (proof that `member` is not a bare ID).
    pub member_identity_pubkey: Key32,
    /// Group Ed25519 signing public key.
    pub sig_pub: Key32,
    /// Hash of the accepted epoch anchor.
    pub anchor_hash: [u8; 32],
    /// Issuing administrator identity key.
    pub issuer_pubkey: Key32,
    /// Issuer signature under the group-grant domain.
    pub signature: Sig64,
}

impl SenderKeyGrant {
    /// Bytes covered by the grant signature.
    pub fn signed_object(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(16 + 2 + 16 + 32 * 4);
        out.extend_from_slice(&self.gid.0);
        out.put_u16(self.epoch);
        out.extend_from_slice(&self.member.0);
        out.extend_from_slice(&self.member_identity_pubkey.0);
        out.extend_from_slice(&self.sig_pub.0);
        out.extend_from_slice(&self.anchor_hash);
        out.extend_from_slice(&self.issuer_pubkey.0);
        out
    }

    /// Canonical encoding.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = self.signed_object();
        out.extend_from_slice(&self.signature.0);
        out
    }

    /// Strict decode.
    pub fn decode(bytes: &[u8]) -> Result<Self, WireError> {
        let mut r = Reader::new(bytes);
        let gid = r.id16()?;
        let epoch = r.u16()?;
        if epoch == 0 {
            return Err(WireError::BadValue("grant epoch"));
        }
        let member = r.id16()?;
        let member_identity_pubkey = r.key32()?;
        if node_id_of(&member_identity_pubkey) != member {
            return Err(WireError::BadValue("grant member node id"));
        }
        let sig_pub = r.key32()?;
        let mut anchor_hash = [0; 32];
        anchor_hash.copy_from_slice(r.take(32)?);
        let issuer_pubkey = r.key32()?;
        let signature = r.sig64()?;
        r.finish()?;
        Ok(Self {
            gid,
            epoch,
            member,
            member_identity_pubkey,
            sig_pub,
            anchor_hash,
            issuer_pubkey,
            signature,
        })
    }
}

/// Pairwise sender-key distribution.  The seed is expected to be encrypted by
/// the session layer; this codec does not expose a recipient-facing plaintext.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SenderKeyDist {
    /// Group identifier.
    pub gid: Id16,
    /// Sender-key epoch.
    pub epoch: u16,
    /// Recipient chain seed.
    pub chain_seed: [u8; 32],
    /// Group signature public key.
    pub sig_pub: Key32,
}

impl SenderKeyDist {
    /// Canonical encoding.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(82);
        out.extend_from_slice(&self.gid.0);
        out.put_u16(self.epoch);
        out.extend_from_slice(&self.chain_seed);
        out.extend_from_slice(&self.sig_pub.0);
        out
    }

    /// Strict decode.
    pub fn decode(bytes: &[u8]) -> Result<Self, WireError> {
        let mut r = Reader::new(bytes);
        let gid = r.id16()?;
        let epoch = r.u16()?;
        if epoch == 0 {
            return Err(WireError::BadValue("distribution epoch"));
        }
        let mut chain_seed = [0; 32];
        chain_seed.copy_from_slice(r.take(32)?);
        let sig_pub = r.key32()?;
        r.finish()?;
        Ok(Self {
            gid,
            epoch,
            chain_seed,
            sig_pub,
        })
    }
}

/// A grant and its pairwise distribution carried in a welcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapDistribution {
    /// Coordinator-signed grant.
    pub grant: SenderKeyGrant,
    /// Pairwise encrypted/transport-bound distribution.
    pub dist: SenderKeyDist,
}

/// Authenticated first-invite bootstrap envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapInvite {
    /// Candidate codec version; must be one.
    pub version: u8,
    /// Pairwise recipient binding.
    pub recipient_node_id: Id16,
    /// Group identifier.
    pub gid: Id16,
    /// Initial sender-key epoch.
    pub epoch: u16,
    /// Immutable administrator set sorted by NodeID.
    pub admins: Vec<AdminInfo>,
    /// Signed initial ADD operation.
    pub initial_add: SignedMembershipOp,
    /// Signed epoch anchor.
    pub anchor: EpochAnchorSignature,
    /// Grants and distributions for the recipient.
    pub distributions: Vec<BootstrapDistribution>,
    /// Pairwise replay nonce (must not be all zero).
    pub bootstrap_nonce: Id16,
}

impl BootstrapInvite {
    /// Canonical encoding.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.push(self.version);
        out.extend_from_slice(&self.recipient_node_id.0);
        out.extend_from_slice(&self.gid.0);
        out.put_u16(self.epoch);
        out.put_u16(self.admins.len() as u16);
        for admin in &self.admins {
            out.extend_from_slice(&admin.encode());
        }
        let add = self.initial_add.encode();
        out.put_u16(add.len() as u16);
        out.extend_from_slice(&add);
        let anchor = self.anchor.encode();
        out.put_u16(anchor.len() as u16);
        out.extend_from_slice(&anchor);
        out.put_u16(self.distributions.len() as u16);
        for item in &self.distributions {
            let grant = item.grant.encode();
            let dist = item.dist.encode();
            out.put_u16(grant.len() as u16);
            out.extend_from_slice(&grant);
            out.put_u16(dist.len() as u16);
            out.extend_from_slice(&dist);
        }
        out.extend_from_slice(&self.bootstrap_nonce.0);
        out
    }

    /// Strict canonical decode with cross-field binding checks.
    pub fn decode(bytes: &[u8]) -> Result<Self, WireError> {
        let mut r = Reader::new(bytes);
        let version = r.u8()?;
        if version != 1 {
            return Err(WireError::BadVersion);
        }
        let recipient_node_id = r.id16()?;
        let gid = r.id16()?;
        let epoch = r.u16()?;
        if epoch == 0 {
            return Err(WireError::BadValue("bootstrap epoch"));
        }

        let count = r.u16()? as usize;
        if count == 0 || count > MAX_ADMINS || r.remaining() < count.saturating_mul(48) {
            return Err(WireError::BadValue("admin count"));
        }
        let mut admins = Vec::with_capacity(count);
        for _ in 0..count {
            let admin = AdminKey::decode(r.take(48)?)?;
            if admins
                .last()
                .is_some_and(|last: &AdminInfo| last.admin_node_id >= admin.admin_node_id)
            {
                return Err(WireError::TlvOrder);
            }
            admins.push(admin);
        }

        let add_bytes = r.varbytes(16 * 1024)?;
        let initial_add = SignedMembershipOp::decode(add_bytes)?;
        let anchor_bytes = r.varbytes(256)?;
        let anchor = EpochAnchorSignature::decode(anchor_bytes)?;
        let dcount = r.u16()? as usize;
        if dcount == 0 || dcount > MAX_WELCOME_DISTS {
            return Err(WireError::BadValue("welcome distribution count"));
        }
        let mut distributions = Vec::with_capacity(dcount);
        for _ in 0..dcount {
            let grant = SenderKeyGrant::decode(r.varbytes(256)?)?;
            let dist = SenderKeyDist::decode(r.varbytes(256)?)?;
            distributions.push(BootstrapDistribution { grant, dist });
        }
        let bootstrap_nonce = r.id16()?;
        r.finish()?;
        let out = Self {
            version,
            recipient_node_id,
            gid,
            epoch,
            admins,
            initial_add,
            anchor,
            distributions,
            bootstrap_nonce,
        };
        out.validate_bindings()?;
        Ok(out)
    }

    fn validate_bindings(&self) -> Result<(), WireError> {
        if self.bootstrap_nonce == Id16::MIN {
            return Err(WireError::BadValue("bootstrap nonce"));
        }
        if self.admins.is_empty() || self.admins.len() > MAX_ADMINS {
            return Err(WireError::BadValue("admin count"));
        }
        for pair in self.admins.windows(2) {
            if pair[0].admin_node_id >= pair[1].admin_node_id {
                return Err(WireError::TlvOrder);
            }
        }
        if self.initial_add.kind != 0
            || self.initial_add.gid != self.gid
            || self.initial_add.base_epoch != self.epoch
            || self.initial_add.member != self.recipient_node_id
        {
            return Err(WireError::BadValue("bootstrap initial add"));
        }
        if self.anchor.anchor.gid != self.gid || self.anchor.anchor.epoch != self.epoch {
            return Err(WireError::BadValue("bootstrap anchor"));
        }
        if !self
            .admins
            .iter()
            .any(|a| a.admin_identity_pubkey == self.anchor.signer_pubkey)
        {
            return Err(WireError::BadValue("bootstrap anchor signer"));
        }
        if !self
            .admins
            .iter()
            .any(|a| a.admin_identity_pubkey == self.initial_add.signer_pubkey)
        {
            return Err(WireError::BadValue("bootstrap add signer"));
        }
        let anchor_hash = self.anchor.anchor.hash();
        for item in &self.distributions {
            let grant = &item.grant;
            let dist = &item.dist;
            if grant.gid != self.gid
                || grant.epoch != self.epoch
                || grant.member != self.recipient_node_id
                || grant.anchor_hash != anchor_hash
            {
                return Err(WireError::BadValue("bootstrap grant"));
            }
            if node_id_of(&grant.member_identity_pubkey) != grant.member {
                return Err(WireError::BadValue("bootstrap member proof"));
            }
            if !self
                .admins
                .iter()
                .any(|a| a.admin_identity_pubkey == grant.issuer_pubkey)
            {
                return Err(WireError::BadValue("bootstrap grant issuer"));
            }
            if dist.gid != self.gid || dist.epoch != self.epoch || dist.sig_pub != grant.sig_pub {
                return Err(WireError::BadValue("bootstrap distribution"));
            }
        }
        Ok(())
    }
}

/// Signed batch of membership operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupControlBatch {
    /// Candidate codec version; must be one.
    pub version: u8,
    /// Group identifier.
    pub gid: Id16,
    /// Hash of the anchor on which this batch is based.
    pub base_anchor_hash: [u8; 32],
    /// Monotonic sequence for the signing administrator.
    pub op_seq: u64,
    /// Membership operations in canonical order.
    pub operations: Vec<SignedMembershipOp>,
    /// Immutable administrator slot of the batch signer.
    pub signer_slot: u16,
    /// Batch signature under the group-control domain.
    pub signature: Sig64,
}

impl GroupControlBatch {
    /// Bytes covered by the batch signature.
    pub fn signed_object(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.push(self.version);
        out.extend_from_slice(&self.gid.0);
        out.extend_from_slice(&self.base_anchor_hash);
        out.put_u64(self.op_seq);
        out.put_u16(self.operations.len() as u16);
        for op in &self.operations {
            let bytes = op.encode();
            out.put_u16(bytes.len() as u16);
            out.extend_from_slice(&bytes);
        }
        out.put_u16(self.signer_slot);
        out
    }

    /// Canonical encoding including the signature.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = self.signed_object();
        out.extend_from_slice(&self.signature.0);
        out
    }

    /// Strict decode.
    pub fn decode(bytes: &[u8]) -> Result<Self, WireError> {
        let mut r = Reader::new(bytes);
        let version = r.u8()?;
        if version != 1 {
            return Err(WireError::BadVersion);
        }
        let gid = r.id16()?;
        let mut base_anchor_hash = [0; 32];
        base_anchor_hash.copy_from_slice(r.take(32)?);
        let op_seq = r.u64()?;
        if op_seq == 0 {
            return Err(WireError::BadValue("control sequence"));
        }
        let count = r.u16()? as usize;
        if count == 0 || count > MAX_CONTROL_OPS {
            return Err(WireError::BadValue("control operation count"));
        }
        let mut operations = Vec::with_capacity(count);
        for _ in 0..count {
            let op = SignedMembershipOp::decode(r.varbytes(16 * 1024)?)?;
            if op.gid != gid {
                return Err(WireError::BadValue("control operation group"));
            }
            operations.push(op);
        }
        let signer_slot = r.u16()?;
        let signature = r.sig64()?;
        r.finish()?;
        Ok(Self {
            version,
            gid,
            base_anchor_hash,
            op_seq,
            operations,
            signer_slot,
            signature,
        })
    }
}

/// Hash a canonical control object for inclusion in an anchor frontier.
pub fn control_frontier_hash(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Hasher::new();
    hasher.update(bytes);
    *hasher.finalize().as_bytes()
}
