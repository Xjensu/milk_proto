//! Group service (doc 06 §8, doc 08 §6): sender keys + epochs over an OR-Set
//! membership CRDT. Sender keys travel through pairwise channels (apptype
//! 0x02); every effective membership change bumps the epoch and re-keys.

use std::collections::HashMap;

use mmp_core::orset::{MembershipOp, OrSet};
use mmp_crypto::group::{LocalSenderKey, RemoteSenderKey, SenderKeyDist};
use mmp_types::consts::RATCHET_WINDOW;
use mmp_types::ids::{Id16, NodeId};
use rand_core::CryptoRngCore;

pub struct GroupState {
    pub gid: Id16,
    pub epoch: u16,
    pub members: OrSet,
    pub local: LocalSenderKey,
    /// (member, epoch) → their sender key.
    pub remotes: HashMap<(NodeId, u16), RemoteSenderKey>,
    /// (member, epoch) → the original dist, retained so re-invites can
    /// forward it to newly-added members (`RemoteSenderKey` itself does not
    /// keep the wire bytes around).
    pub dists: HashMap<(NodeId, u16), SenderKeyDist>,
    pub is_admin: bool,
    next_tag: u64,
    /// A restored group has a freshly generated local sender key but no
    /// persisted remote distribution. The first post-restart send must
    /// re-gossip that key before publishing group ciphertext.
    needs_distribution: bool,
}

impl GroupState {
    pub fn create(rng: &mut impl CryptoRngCore, gid: Id16, me: NodeId, is_admin: bool) -> GroupState {
        let epoch = 1;
        let mut members = OrSet::new();
        members.apply(&MembershipOp::Add { member: me, tag: 1 });
        GroupState {
            gid,
            epoch,
            members,
            local: LocalSenderKey::generate(rng, gid, epoch),
            remotes: HashMap::new(),
            dists: HashMap::new(),
            is_admin,
            next_tag: 2,
            needs_distribution: false,
        }
    }

    pub fn add_member(&mut self, member: NodeId) -> u64 {
        let tag = self.next_tag;
        self.next_tag += 1;
        self.members.apply(&MembershipOp::Add { member, tag });
        tag
    }

    /// REMOVE + epoch bump + fresh local sender key (future secrecy vs the
    /// removed member, doc 08 §6.3). Returns the new epoch. Removal takes
    /// effect as the network reconverges — never claim instant removal
    /// (doc 06 §8.6).
    pub fn remove_member(&mut self, rng: &mut impl CryptoRngCore, member: &NodeId) -> u16 {
        let tags = self.members.tags_of(member);
        self.members.apply(&MembershipOp::Remove { tags });
        self.epoch += 1;
        self.local = LocalSenderKey::generate(rng, self.gid, self.epoch);
        // Old-epoch remote keys stay valid for in-flight traffic; new-epoch
        // keys arrive via fresh dists.
        self.epoch
    }

    /// Learn a sender-key distribution once. Replacing an existing receiver
    /// chain for the same `(member, epoch)` would reset its replay window and
    /// permit an already-seen group message to be accepted again.
    pub fn learn_remote(&mut self, member: NodeId, dist: &SenderKeyDist) -> bool {
        if self.dists.contains_key(&(member, dist.epoch)) {
            return false;
        }
        self.remotes
            .insert((member, dist.epoch), RemoteSenderKey::from_dist(dist, RATCHET_WINDOW));
        self.dists.insert((member, dist.epoch), dist.clone());
        true
    }

    /// Persistence encoding (mmp-store snapshot, handbook §4.3 step 2):
    /// membership + epoch bookkeeping ONLY. Deliberately excludes `local`,
    /// `remotes`, and `dists` — those hold sender-key ratchet material
    /// (`SendChain`/`RecvChain` state and the `chain_seed` symmetric secret
    /// that seeds them, `crates/mmp-crypto/src/ratchet.rs`). Persisting a
    /// chain seed to disk would let an attacker who captures the file
    /// decrypt every message derivable from it (the same PCS concern
    /// documented for the pairwise conversation ratchet, handbook §4.3 step
    /// 4); reconstructing `local`'s `SendChain` from a frozen seed and
    /// calling `next()` again would additionally reuse an already-spent
    /// message key. So: no group crypto material survives restart, only
    /// "who is in the group, at what epoch." Layout:
    /// `gid[16] ‖ epoch u16 ‖ is_admin u8 ‖ next_tag u64 ‖ orset-len u32 ‖ orset bytes`.
    pub fn encode(&self) -> Vec<u8> {
        let orset = self.members.encode();
        let mut out = Vec::with_capacity(16 + 2 + 1 + 8 + 4 + orset.len());
        out.extend_from_slice(&self.gid.0);
        out.extend_from_slice(&self.epoch.to_le_bytes());
        out.push(self.is_admin as u8);
        out.extend_from_slice(&self.next_tag.to_le_bytes());
        out.extend_from_slice(&(orset.len() as u32).to_le_bytes());
        out.extend_from_slice(&orset);
        out
    }

    /// Inverse of [`GroupState::encode`]. A fresh local sender key is
    /// generated for the restored epoch via `rng` (never a persisted one —
    /// see the encode doc); `remotes`/`dists` start empty and re-populate
    /// through the normal pairwise dist-gossip path on next contact, exactly
    /// like a freshly-invited member would receive them. `None` on any
    /// truncated/malformed input — the caller (mmp-store) treats that like a
    /// corrupt checksummed record: drop, don't repair.
    pub fn decode(rng: &mut impl CryptoRngCore, bytes: &[u8]) -> Option<GroupState> {
        const HDR: usize = 16 + 2 + 1 + 8 + 4;
        if bytes.len() < HDR {
            return None;
        }
        let mut gid = [0u8; 16];
        gid.copy_from_slice(&bytes[0..16]);
        let gid = Id16(gid);
        let epoch = u16::from_le_bytes(bytes[16..18].try_into().ok()?);
        let is_admin = bytes[18] != 0;
        let next_tag = u64::from_le_bytes(bytes[19..27].try_into().ok()?);
        let orset_len = u32::from_le_bytes(bytes[27..31].try_into().ok()?) as usize;
        let end = 31usize.checked_add(orset_len)?;
        let orset_bytes = bytes.get(31..end)?;
        if bytes.len() != end {
            return None; // trailing garbage: treat as corrupt
        }
        let members = OrSet::decode(orset_bytes)?;
        Some(GroupState {
            gid,
            epoch,
            members,
            local: LocalSenderKey::generate(rng, gid, epoch),
            remotes: HashMap::new(),
            dists: HashMap::new(),
            is_admin,
            next_tag,
            needs_distribution: true,
        })
    }

    /// True after a non-secret group restore until the fresh local sender key
    /// has been scheduled for re-gossip.
    pub fn needs_distribution(&self) -> bool {
        self.needs_distribution
    }

    /// Clear the restart re-gossip marker after scheduling distributions.
    pub fn mark_distribution_attempted(&mut self) {
        self.needs_distribution = false;
    }
}

#[cfg(test)]
mod tests {
    use super::GroupState;
    use rand::{rngs::StdRng, SeedableRng};

    fn header(orset_len: u32) -> Vec<u8> {
        let mut bytes = vec![0u8; 31];
        bytes[27..31].copy_from_slice(&orset_len.to_le_bytes());
        bytes
    }

    #[test]
    fn decode_rejects_overflowed_or_trailing_group_state() {
        let mut rng = StdRng::seed_from_u64(1);
        let overflow = header(u32::MAX);
        assert!(GroupState::decode(&mut rng, &overflow).is_none());

        // Empty OR-Set encoding is two four-byte counts (member count and
        // tombstone count).  A trailing byte must not be accepted as part of
        // an otherwise valid group snapshot.
        let mut trailing = header(8);
        trailing.extend_from_slice(&[0u8; 8]);
        trailing.push(0);
        assert!(GroupState::decode(&mut rng, &trailing).is_none());
    }

    #[test]
    fn restored_group_requires_one_local_key_regossip() {
        let mut rng = StdRng::seed_from_u64(2);
        let original = GroupState::create(&mut rng, mmp_types::ids::Id16([7; 16]), mmp_types::ids::Id16([8; 16]), true);
        let restored = GroupState::decode(&mut rng, &original.encode()).expect("valid group snapshot");
        assert!(restored.needs_distribution());
    }
}
