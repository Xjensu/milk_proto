//! Group service (doc 06 §8, doc 08 §6): sender keys + epochs over an OR-Set
//! membership CRDT. Sender keys travel through pairwise channels (apptype
//! 0x02); every effective membership change bumps the epoch and re-keys.

use std::collections::HashMap;

use mmp_core::orset::{MembershipOp, OrSet};
use mmp_crypto::group::{LocalSenderKey, RemoteSenderKey, SenderKeyDist};
use mmp_types::consts::RATCHET_WINDOW;
use mmp_types::ids::{Id16, NodeId};
use rand_core::CryptoRngCore;

/// Local cap on group state admitted from network/persistence inputs. Group
/// bootstrap is TOFU in the current v1 implementation, so an authenticated
/// peer can otherwise allocate one fresh `GroupState` per random group id.
pub(crate) const MAX_GROUPS: usize = 256;

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
    pub fn create(
        rng: &mut impl CryptoRngCore,
        gid: Id16,
        me: NodeId,
        is_admin: bool,
    ) -> GroupState {
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

    /// Reserve a fresh OR-Set tag for a member addition.  Once the finite
    /// local cursor reaches `u64::MAX`, fail closed instead of wrapping and
    /// reusing an old tag.
    pub fn add_member(&mut self, member: NodeId) -> Option<u64> {
        let tag = self.next_tag;
        self.next_tag = self.next_tag.checked_add(1)?;
        self.members.apply(&MembershipOp::Add { member, tag });
        Some(tag)
    }

    /// Whether a fresh membership tag can still be allocated.
    pub fn can_add_member(&self) -> bool {
        self.next_tag < u64::MAX
    }

    /// REMOVE + epoch bump + fresh local sender key (future secrecy vs the
    /// removed member, doc 08 §6.3). Returns the new epoch. Removal takes
    /// effect as the network reconverges — never claim instant removal
    /// (doc 06 §8.6).
    /// Returns `None` when the finite `u16` epoch domain is exhausted.
    pub fn remove_member(&mut self, rng: &mut impl CryptoRngCore, member: &NodeId) -> Option<u16> {
        if !self.members.contains(member) {
            return None;
        }
        let next_epoch = self.epoch.checked_add(1)?;
        let tags = self.members.tags_of(member);
        self.members.apply(&MembershipOp::Remove { tags });
        self.epoch = next_epoch;
        self.local = LocalSenderKey::generate(rng, self.gid, self.epoch);
        self.needs_distribution = true;
        // Old-epoch remote keys stay valid for in-flight traffic; new-epoch
        // keys arrive via fresh dists.
        Some(self.epoch)
    }

    /// Learn a sender-key distribution once. Replacing an existing receiver
    /// chain for the same `(member, epoch)` would reset its replay window and
    /// permit an already-seen group message to be accepted again.
    pub fn learn_remote(&mut self, member: NodeId, dist: &SenderKeyDist) -> bool {
        if self.dists.contains_key(&(member, dist.epoch)) {
            return false;
        }
        self.remotes.insert(
            (member, dist.epoch),
            RemoteSenderKey::from_dist(dist, RATCHET_WINDOW),
        );
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
        if epoch == 0 {
            return None;
        }
        let is_admin = match bytes[18] {
            0 => false,
            1 => true,
            _ => return None,
        };
        let next_tag = u64::from_le_bytes(bytes[19..27].try_into().ok()?);
        // Tag 1 is reserved for the creator's bootstrap membership. A
        // cursor below 2 cannot represent a valid GroupState.
        if next_tag < 2 {
            return None;
        }
        let orset_len = u32::from_le_bytes(bytes[27..31].try_into().ok()?) as usize;
        let end = 31usize.checked_add(orset_len)?;
        let orset_bytes = bytes.get(31..end)?;
        if bytes.len() != end {
            return None; // trailing garbage: treat as corrupt
        }
        let members = OrSet::decode(orset_bytes)?;
        // The cursor must be strictly greater than every tag in live
        // membership or tombstone history, otherwise a restored node could
        // issue a duplicate tag and make later REMOVE operations ambiguous.
        if members.max_tag().is_some_and(|tag| next_tag <= tag) {
            return None;
        }
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

    /// Keep the current sender key scheduled for re-gossip after a provider
    /// or peer-facing distribution attempt fails.
    pub fn mark_distribution_needed(&mut self) {
        self.needs_distribution = true;
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
        let original = GroupState::create(
            &mut rng,
            mmp_types::ids::Id16([7; 16]),
            mmp_types::ids::Id16([8; 16]),
            true,
        );
        let encoded = original.encode();
        let expected_len = 31 + original.members.encode().len();
        assert_eq!(
            encoded.len(),
            expected_len,
            "group snapshot contains metadata only"
        );

        let original_dist = original.local.dist().clone();
        let mut restored = GroupState::decode(&mut rng, &encoded).expect("valid group snapshot");
        assert!(restored.needs_distribution());
        assert!(restored.remotes.is_empty());
        assert!(restored.dists.is_empty());
        assert_ne!(
            restored.local.dist(),
            &original_dist,
            "restore must mint a fresh chain and signing key"
        );

        let first = restored
            .local
            .encrypt(restored.members.members()[0], &[], b"first");
        assert_eq!(first.epoch, restored.epoch);
        assert_eq!(
            first.chain_index, 0,
            "fresh local chain starts at index zero"
        );
    }

    #[test]
    fn decode_rejects_zero_epoch_and_noncanonical_admin() {
        let mut rng = StdRng::seed_from_u64(3);
        let original = GroupState::create(
            &mut rng,
            mmp_types::ids::Id16([9; 16]),
            mmp_types::ids::Id16([10; 16]),
            true,
        );
        let mut zero_epoch = original.encode();
        zero_epoch[16..18].fill(0);
        assert!(GroupState::decode(&mut rng, &zero_epoch).is_none());

        let mut bad_admin = original.encode();
        bad_admin[18] = 2;
        assert!(GroupState::decode(&mut rng, &bad_admin).is_none());
    }

    #[test]
    fn remove_member_fails_closed_at_epoch_limit() {
        let mut rng = StdRng::seed_from_u64(4);
        let me = mmp_types::ids::Id16([11; 16]);
        let member = mmp_types::ids::Id16([12; 16]);
        let mut group = GroupState::create(&mut rng, mmp_types::ids::Id16([13; 16]), me, true);
        group
            .add_member(member)
            .expect("fresh group has tag capacity");
        group.epoch = u16::MAX;
        let before = group.members.encode();
        assert!(group.remove_member(&mut rng, &member).is_none());
        assert_eq!(group.epoch, u16::MAX);
        assert_eq!(
            group.members.encode(),
            before,
            "failed rotation must not mutate membership"
        );
    }

    #[test]
    fn remove_unknown_member_does_not_rotate_epoch() {
        let mut rng = StdRng::seed_from_u64(5);
        let me = mmp_types::ids::Id16([21; 16]);
        let unknown = mmp_types::ids::Id16([22; 16]);
        let mut group = GroupState::create(&mut rng, mmp_types::ids::Id16([23; 16]), me, true);
        let before_epoch = group.epoch;
        let before_dist = group.local.dist().clone();
        let before_members = group.members.encode();

        assert!(group.remove_member(&mut rng, &unknown).is_none());
        assert_eq!(group.epoch, before_epoch);
        assert_eq!(group.local.dist(), &before_dist);
        assert_eq!(group.members.encode(), before_members);
    }

    #[test]
    fn decode_rejects_reused_next_tag_cursor() {
        let mut rng = StdRng::seed_from_u64(6);
        let me = mmp_types::ids::Id16([31; 16]);
        let member = mmp_types::ids::Id16([32; 16]);
        let mut group = GroupState::create(&mut rng, mmp_types::ids::Id16([33; 16]), me, true);
        group
            .add_member(member)
            .expect("fresh group has tag capacity");
        let mut encoded = group.encode();
        // Bootstrap uses tag 1 and the added member uses tag 2; next_tag is
        // normally 3. Rewinding it to 2 would reuse an existing tag.
        encoded[19..27].copy_from_slice(&2u64.to_le_bytes());
        assert!(GroupState::decode(&mut rng, &encoded).is_none());
    }

    #[test]
    fn add_member_fails_closed_at_tag_limit() {
        let mut rng = StdRng::seed_from_u64(7);
        let me = mmp_types::ids::Id16([41; 16]);
        let member = mmp_types::ids::Id16([42; 16]);
        let mut group = GroupState::create(&mut rng, mmp_types::ids::Id16([43; 16]), me, true);
        group.next_tag = u64::MAX;
        let before = group.members.encode();
        assert!(!group.can_add_member());
        assert!(group.add_member(member).is_none());
        assert_eq!(
            group.members.encode(),
            before,
            "failed allocation must not mutate membership"
        );
    }
}
