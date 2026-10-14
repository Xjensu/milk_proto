//! Group membership OR-Set (doc 06 §8): add-wins, admin-signed ops, epoch
//! anchors as GC checkpoints (AUDIT F5).
//!
//! Signature verification of ops happens in the group service (node layer);
//! this structure is the pure CRDT.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;

use mmp_types::ids::{Id16, NodeId};

/// A unique tag stamped on each ADD op, used to reference it from a later
/// REMOVE and to distinguish concurrent adds of the same member.
pub type Tag = u64;

/// An admin-signed group-membership CRDT operation (doc 06 §8). Signature
/// verification happens one layer up (node/group service); `OrSet` only
/// applies already-verified ops.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MembershipOp {
    /// Adds `member` to the set under a fresh `tag`, unless that tag is
    /// already tombstoned.
    Add {
        /// The node being added to the group.
        member: NodeId,
        /// The unique tag stamped on this add, later referenced by a REMOVE.
        tag: Tag,
    },
    /// Tombstones every tag in `tags`, removing it from every member's live
    /// add-set (add-wins: a member with no live tags left is no longer a
    /// member).
    Remove {
        /// The add-tags being tombstoned.
        tags: Vec<Tag>,
    },
}

/// Add-wins group membership CRDT (doc 06 §8): each member's presence is the
/// union of live (non-tombstoned) add-tags, epoch anchors serve as GC
/// checkpoints, and compaction below an anchor discards resolved history.
#[derive(Default, Debug, Clone)]
pub struct OrSet {
    /// member → live add-tags.
    adds: BTreeMap<NodeId, BTreeSet<Tag>>,
    tombstones: BTreeSet<Tag>,
}

impl OrSet {
    /// Builds an empty set: no members, no tombstones.
    pub fn new() -> OrSet {
        OrSet::default()
    }

    /// Applies one already-verified [`MembershipOp`]: `Add` inserts the tag
    /// unless it's already tombstoned; `Remove` tombstones each tag, strips
    /// it from every member's live set, and drops any member left with no
    /// live tags.
    pub fn apply(&mut self, op: &MembershipOp) {
        match op {
            MembershipOp::Add { member, tag } => {
                if !self.tombstones.contains(tag) {
                    self.adds.entry(*member).or_default().insert(*tag);
                }
            }
            MembershipOp::Remove { tags } => {
                for tag in tags {
                    self.tombstones.insert(*tag);
                    for set in self.adds.values_mut() {
                        set.remove(tag);
                    }
                }
                self.adds.retain(|_, set| !set.is_empty());
            }
        }
    }

    /// Observed add-tags for `member` — what a REMOVE must reference.
    pub fn tags_of(&self, member: &NodeId) -> Vec<Tag> {
        self.adds
            .get(member)
            .map(|s| s.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Every member with at least one live add-tag.
    pub fn members(&self) -> Vec<NodeId> {
        self.adds.keys().copied().collect()
    }

    /// Whether `member` currently has at least one live add-tag.
    pub fn contains(&self, member: &NodeId) -> bool {
        self.adds.contains_key(member)
    }

    /// BLAKE3 root over the sorted member set — the epoch-anchor payload
    /// (doc 06 §8.3).
    pub fn member_root(&self) -> [u8; 32] {
        let mut h = blake3::Hasher::new_derive_key("mmp1 member-root");
        for m in self.adds.keys() {
            h.update(&m.0);
        }
        *h.finalize().as_bytes()
    }

    /// Compaction below a mutually-acknowledged anchor (doc 06 §8.5): drop
    /// tombstones and clamp per-member tag sets to one representative — every
    /// discarded tag's effect is already reflected in the anchored root.
    pub fn compact_at_anchor(&mut self, anchored_root: &[u8; 32]) {
        if &self.member_root() != anchored_root {
            return; // we have in-flight ops beyond the anchor — retain state
        }
        self.tombstones.clear();
        for set in self.adds.values_mut() {
            if let Some(&keep) = set.iter().next() {
                set.clear();
                set.insert(keep);
            }
        }
    }

    /// State size in tags+tombstones (bounded-growth test hook).
    pub fn state_size(&self) -> usize {
        self.tombstones.len() + self.adds.values().map(|s| s.len()).sum::<usize>()
    }

    /// Highest tag present in either live adds or tombstones.
    ///
    /// Group snapshots persist the next local tag cursor separately from the
    /// OR-Set.  The group layer uses this metadata-only view to reject a
    /// restored cursor that could reuse a tag already observed in membership
    /// history.  It does not alter the wire or persistence encoding.
    pub fn max_tag(&self) -> Option<Tag> {
        self.adds
            .values()
            .flat_map(|tags| tags.iter().copied())
            .chain(self.tombstones.iter().copied())
            .max()
    }

    /// Persistence encoding (mmp-store snapshot, handbook §4.3 step 2): a
    /// minimal hand-written codec, no serde (handbook §0 rule 1). Layout:
    /// `member-count u32 ‖ { member NodeId[16] ‖ tag-count u32 ‖ tag u64× }…
    /// ‖ tombstone-count u32 ‖ tombstone u64×`. Self-delimiting — the caller
    /// need not length-prefix this blob to know where it ends, but callers
    /// embedding it in a larger record still may for simplicity.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(self.adds.len() as u32).to_le_bytes());
        for (member, tags) in &self.adds {
            out.extend_from_slice(&member.0);
            out.extend_from_slice(&(tags.len() as u32).to_le_bytes());
            for tag in tags {
                out.extend_from_slice(&tag.to_le_bytes());
            }
        }
        out.extend_from_slice(&(self.tombstones.len() as u32).to_le_bytes());
        for tag in &self.tombstones {
            out.extend_from_slice(&tag.to_le_bytes());
        }
        out
    }

    /// Inverse of [`OrSet::encode`]. `None` on any truncation/malformed
    /// count — the caller (mmp-store) treats that exactly like a corrupt
    /// checksummed record: drop, don't repair (doc 04 §7).
    pub fn decode(bytes: &[u8]) -> Option<OrSet> {
        let mut pos = 0usize;
        let u32_at = |b: &[u8], p: &mut usize| -> Option<u32> {
            let end = p.checked_add(4)?;
            let v = b.get(*p..end)?;
            *p = end;
            Some(u32::from_le_bytes(v.try_into().ok()?))
        };
        let u64_at = |b: &[u8], p: &mut usize| -> Option<u64> {
            let end = p.checked_add(8)?;
            let v = b.get(*p..end)?;
            *p = end;
            Some(u64::from_le_bytes(v.try_into().ok()?))
        };

        let member_count = u32_at(bytes, &mut pos)?;
        // Each member needs a 16-byte ID and a 4-byte tag count, followed by
        // the mandatory 4-byte tombstone count.  Reject impossible counts
        // before entering the member loop; this keeps a hostile snapshot
        // from turning a tiny count field into an expensive parse.
        const MEMBER_MIN_BYTES: usize = 16 + 4;
        if bytes.len().saturating_sub(pos) < 4
            || member_count as usize > (bytes.len().saturating_sub(pos) - 4) / MEMBER_MIN_BYTES
        {
            return None;
        }
        let mut adds = BTreeMap::new();
        // A tag identifies one logical ADD operation.  Keep a second index
        // while decoding so a corrupt snapshot cannot assign one tag to two
        // members; the normal apply/encode path never produces that state.
        let mut seen_add_tags = BTreeSet::new();
        for _ in 0..member_count {
            let member_end = pos.checked_add(16)?;
            let member_bytes = bytes.get(pos..member_end)?;
            pos = member_end;
            let mut arr = [0u8; 16];
            arr.copy_from_slice(member_bytes);
            let member: NodeId = Id16(arr);
            if adds.contains_key(&member) {
                return None; // non-canonical duplicate member record
            }
            let tag_count = u32_at(bytes, &mut pos)?;
            if tag_count == 0 {
                return None; // live members always have at least one tag
            }
            if tag_count as usize > bytes.len().saturating_sub(pos) / 8 {
                return None;
            }
            let mut tags = BTreeSet::new();
            for _ in 0..tag_count {
                let tag = u64_at(bytes, &mut pos)?;
                if !tags.insert(tag) || !seen_add_tags.insert(tag) {
                    return None; // duplicate tag within/across member records
                }
            }
            adds.insert(member, tags);
        }
        let tombstone_count = u32_at(bytes, &mut pos)?;
        if tombstone_count as usize > bytes.len().saturating_sub(pos) / 8 {
            return None;
        }
        let mut tombstones = BTreeSet::new();
        for _ in 0..tombstone_count {
            let tag = u64_at(bytes, &mut pos)?;
            if !tombstones.insert(tag) || seen_add_tags.contains(&tag) {
                return None; // duplicate tombstone or live/tombstone overlap
            }
        }
        if pos != bytes.len() {
            return None; // trailing garbage: treat as corrupt, not forward-compat
        }
        Some(OrSet { adds, tombstones })
    }
}

#[cfg(test)]
mod tests {
    use super::OrSet;

    fn member_record(out: &mut Vec<u8>, id: u8, tags: &[u64]) {
        out.extend_from_slice(&[id; 16]);
        out.extend_from_slice(&(tags.len() as u32).to_le_bytes());
        for tag in tags {
            out.extend_from_slice(&tag.to_le_bytes());
        }
    }

    fn tombstone_records(out: &mut Vec<u8>, tags: &[u64]) {
        out.extend_from_slice(&(tags.len() as u32).to_le_bytes());
        for tag in tags {
            out.extend_from_slice(&tag.to_le_bytes());
        }
    }

    #[test]
    fn decode_rejects_hostile_member_tag_and_tombstone_counts() {
        // The counts are disk-controlled.  They must be rejected from the
        // remaining-byte arithmetic before any unbounded loop begins.
        let mut members = Vec::new();
        members.extend_from_slice(&u32::MAX.to_le_bytes());
        assert!(OrSet::decode(&members).is_none());

        let mut tags = Vec::new();
        tags.extend_from_slice(&1u32.to_le_bytes()); // one member
        tags.extend_from_slice(&[0u8; 16]);
        tags.extend_from_slice(&u32::MAX.to_le_bytes());
        tags.extend_from_slice(&0u32.to_le_bytes()); // trailing tombstone_count
        assert!(OrSet::decode(&tags).is_none());

        let mut tombstones = Vec::new();
        tombstones.extend_from_slice(&0u32.to_le_bytes()); // member_count
        tombstones.extend_from_slice(&u32::MAX.to_le_bytes());
        assert!(OrSet::decode(&tombstones).is_none());
    }

    #[test]
    fn decode_rejects_duplicate_live_tags() {
        // The same ADD tag cannot belong to two members.  A decoder that
        // accepts this snapshot would make a later REMOVE ambiguous.
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&2u32.to_le_bytes());
        member_record(&mut bytes, 1, &[7]);
        member_record(&mut bytes, 2, &[7]);
        tombstone_records(&mut bytes, &[]);
        assert!(OrSet::decode(&bytes).is_none());

        // Duplicate tags within one member are non-canonical as well; the
        // in-memory BTreeSet would otherwise silently discard one copy.
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&1u32.to_le_bytes());
        member_record(&mut bytes, 3, &[8, 8]);
        tombstone_records(&mut bytes, &[]);
        assert!(OrSet::decode(&bytes).is_none());
    }

    #[test]
    fn decode_rejects_live_tombstone_overlap_and_duplicate_tombstones() {
        // A tag cannot be live and tombstoned in a valid encoded OR-Set.
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&1u32.to_le_bytes());
        member_record(&mut bytes, 4, &[9]);
        tombstone_records(&mut bytes, &[9]);
        assert!(OrSet::decode(&bytes).is_none());

        // Tombstone records are canonical BTreeSet output and cannot repeat.
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0u32.to_le_bytes());
        tombstone_records(&mut bytes, &[10, 10]);
        assert!(OrSet::decode(&bytes).is_none());
    }

    #[test]
    fn decode_rejects_duplicate_member_and_empty_live_records() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&2u32.to_le_bytes());
        member_record(&mut bytes, 5, &[11]);
        member_record(&mut bytes, 5, &[12]);
        tombstone_records(&mut bytes, &[]);
        assert!(OrSet::decode(&bytes).is_none());

        let mut bytes = Vec::new();
        bytes.extend_from_slice(&1u32.to_le_bytes());
        member_record(&mut bytes, 6, &[]);
        tombstone_records(&mut bytes, &[]);
        assert!(OrSet::decode(&bytes).is_none());
    }
}
