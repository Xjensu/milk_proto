//! Conversation DAG (doc 06 §5): causal partial order via PARENTS TLV.
//!
//! Display order is topological, ties by (height, BundleID asc) — no wall
//! clocks. Unknown parents become gap markers + WANT entries; edges are
//! advisory ordering, never integrity.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;

use mmp_types::ids::Id16;

#[derive(Debug, Clone)]
struct MsgNode {
    parents: Vec<Id16>,
    height: u32,
}

/// Causal partial order over one conversation's messages via the PARENTS TLV
/// (doc 06 §5): tracks per-message height, the current heads (frontier), and
/// gaps (referenced parents not yet held).
#[derive(Default)]
pub struct ConversationDag {
    nodes: BTreeMap<Id16, MsgNode>,
    /// Messages with no known children — the PARENTS of our next send.
    heads: BTreeSet<Id16>,
    /// Referenced-but-unknown parents (WANT set feed).
    gaps: BTreeSet<Id16>,
}

const SNAPSHOT_MAGIC: [u8; 8] = *b"MMPDAG01";
const SNAPSHOT_VERSION: u8 = 1;
const SNAPSHOT_HEADER_LEN: usize = 8 + 1 + 4 + 4 + 4;
const SNAPSHOT_NODE_FIXED_LEN: usize = 16 + 4 + 1;
const SNAPSHOT_ID_LEN: usize = 16;
const MAX_SNAPSHOT_NODES: usize = 4096;
const MAX_SNAPSHOT_HEADS: usize = 4096;
const MAX_SNAPSHOT_GAPS: usize = 4096;
const MAX_SNAPSHOT_PARENTS: usize = mmp_types::tlv::MAX_PARENTS;

/// Failure to decode or encode a provider-local conversation-DAG snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DagSnapshotError {
    /// The snapshot is malformed, non-canonical, oversized, or inconsistent.
    Invalid,
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::ConversationDag;
    use mmp_types::ids::Id16;

    const MAGIC_LEN: usize = 8;
    const HEADER_LEN: usize = MAGIC_LEN + 1 + 4 + 4 + 4;
    const NODE_FIXED_LEN: usize = 16 + 4 + 1;

    fn id(value: u8) -> Id16 {
        Id16([value; 16])
    }

    fn populated() -> (ConversationDag, Vec<u8>) {
        let mut dag = ConversationDag::new();
        let first = id(1);
        let second = id(2);
        let missing = id(9);
        dag.insert(first, &[]);
        dag.insert(second, &[first]);
        dag.insert(id(3), &[second, missing]);
        let encoded = dag.snapshot().expect("valid DAG snapshot");
        (dag, encoded)
    }

    #[test]
    fn snapshot_roundtrip_preserves_canonical_state() {
        let (original, encoded) = populated();
        let restored = ConversationDag::restore(&encoded).expect("valid DAG snapshot");

        assert_eq!(restored.snapshot().unwrap(), encoded);
        assert_eq!(restored.len(), original.len());
        assert_eq!(restored.display_order(), original.display_order());
        assert_eq!(restored.heads(), original.heads());
        assert_eq!(
            restored.gaps().copied().collect::<Vec<_>>(),
            original.gaps().copied().collect::<Vec<_>>()
        );
        for known in [id(1), id(2), id(3)] {
            assert!(restored.contains(&known));
        }
        assert!(!restored.contains(&id(9)));
    }

    #[test]
    fn snapshot_rejects_bad_version_trailing_bytes_and_count_amplification() {
        let (_, encoded) = populated();

        let mut bad_version = encoded.clone();
        bad_version[MAGIC_LEN] = bad_version[MAGIC_LEN].wrapping_add(1);
        assert!(ConversationDag::restore(&bad_version).is_err());

        let mut trailing = encoded.clone();
        trailing.push(0);
        assert!(ConversationDag::restore(&trailing).is_err());

        let mut huge_count = encoded;
        huge_count[MAGIC_LEN + 1..MAGIC_LEN + 5].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(ConversationDag::restore(&huge_count).is_err());
    }

    #[test]
    fn snapshot_rejects_duplicate_nodes_and_parent_cap() {
        let (_, encoded) = populated();

        // The first node has no parents and therefore occupies the fixed
        // portion of one node record. Make the second ID equal to the first.
        let second_node = HEADER_LEN + NODE_FIXED_LEN;
        let mut duplicate = encoded.clone();
        duplicate[second_node..second_node + 16].copy_from_slice(&id(1).0);
        assert!(ConversationDag::restore(&duplicate).is_err());

        let mut too_many_parents = encoded;
        let first_parent_count = HEADER_LEN + 16 + 4;
        too_many_parents[first_parent_count] = 5;
        assert!(ConversationDag::restore(&too_many_parents).is_err());
    }

    #[test]
    fn snapshot_rejects_unsorted_parents_and_invalid_frontier_members() {
        let mut dag = ConversationDag::new();
        let first = id(1);
        let second = id(2);
        dag.insert(first, &[]);
        dag.insert(second, &[]);
        dag.insert(id(3), &[second, first]);
        let encoded = dag.snapshot().expect("valid DAG snapshot");

        // The canonical encoder sorts parent IDs. Reverse the two IDs in the
        // child record; restore must reject a non-canonical parent ordering.
        let child = HEADER_LEN + (NODE_FIXED_LEN) + (NODE_FIXED_LEN);
        let parents = child + NODE_FIXED_LEN;
        let mut unsorted = encoded.clone();
        let first_parent = unsorted[parents..parents + 16].to_vec();
        let second_parent = unsorted[parents + 16..parents + 32].to_vec();
        unsorted[parents..parents + 16].copy_from_slice(&second_parent);
        unsorted[parents + 16..parents + 32].copy_from_slice(&first_parent);
        assert!(ConversationDag::restore(&unsorted).is_err());

        // Replace the one encoded head with an ID not present in the node set.
        let head_offset = unsorted.len() - 16 - 16; // one head, one gap
        let mut bad_head = encoded;
        bad_head[head_offset..head_offset + 16].copy_from_slice(&id(8).0);
        assert!(ConversationDag::restore(&bad_head).is_err());
    }
}

impl ConversationDag {
    /// An empty DAG (no messages, no heads, no gaps).
    pub fn new() -> ConversationDag {
        ConversationDag::default()
    }

    /// Encode the complete causal state as a bounded provider-local snapshot.
    ///
    /// This is not an MMP wire format and carries no cryptographic material.
    /// Parents, node IDs, heads, and gaps are canonicalized so two equal DAGs
    /// have byte-identical snapshots.
    pub fn snapshot(&self) -> Result<Vec<u8>, DagSnapshotError> {
        if self.nodes.len() > MAX_SNAPSHOT_NODES
            || self.heads.len() > MAX_SNAPSHOT_HEADS
            || self.gaps.len() > MAX_SNAPSHOT_GAPS
        {
            return Err(DagSnapshotError::Invalid);
        }

        let mut out = Vec::with_capacity(SNAPSHOT_HEADER_LEN);
        out.extend_from_slice(&SNAPSHOT_MAGIC);
        out.push(SNAPSHOT_VERSION);
        out.extend_from_slice(&(self.nodes.len() as u32).to_le_bytes());
        out.extend_from_slice(&(self.heads.len() as u32).to_le_bytes());
        out.extend_from_slice(&(self.gaps.len() as u32).to_le_bytes());

        for (id, node) in &self.nodes {
            if node.parents.len() > MAX_SNAPSHOT_PARENTS || node.parents.len() > u8::MAX as usize {
                return Err(DagSnapshotError::Invalid);
            }
            let mut parents = node.parents.clone();
            parents.sort_unstable();
            if parents.windows(2).any(|pair| pair[0] == pair[1]) {
                return Err(DagSnapshotError::Invalid);
            }
            out.extend_from_slice(&id.0);
            out.extend_from_slice(&node.height.to_le_bytes());
            out.push(parents.len() as u8);
            for parent in parents {
                out.extend_from_slice(&parent.0);
            }
        }
        for id in &self.heads {
            out.extend_from_slice(&id.0);
        }
        for id in &self.gaps {
            out.extend_from_slice(&id.0);
        }
        Ok(out)
    }

    /// Restore a complete DAG snapshot without accepting trailing or
    /// allocation-amplifying input. The resulting state is validated against
    /// the encoded head/gap frontiers before it is returned.
    pub fn restore(bytes: &[u8]) -> Result<ConversationDag, DagSnapshotError> {
        if bytes.len() < SNAPSHOT_HEADER_LEN
            || bytes[..SNAPSHOT_MAGIC.len()] != SNAPSHOT_MAGIC
            || bytes[SNAPSHOT_MAGIC.len()] != SNAPSHOT_VERSION
        {
            return Err(DagSnapshotError::Invalid);
        }
        let node_count = u32::from_le_bytes([bytes[9], bytes[10], bytes[11], bytes[12]]) as usize;
        let head_count = u32::from_le_bytes([bytes[13], bytes[14], bytes[15], bytes[16]]) as usize;
        let gap_count = u32::from_le_bytes([bytes[17], bytes[18], bytes[19], bytes[20]]) as usize;
        if node_count > MAX_SNAPSHOT_NODES
            || head_count > MAX_SNAPSHOT_HEADS
            || gap_count > MAX_SNAPSHOT_GAPS
        {
            return Err(DagSnapshotError::Invalid);
        }
        let min_nodes = node_count
            .checked_mul(SNAPSHOT_NODE_FIXED_LEN)
            .ok_or(DagSnapshotError::Invalid)?;
        let min_frontier = head_count
            .checked_add(gap_count)
            .and_then(|count| count.checked_mul(SNAPSHOT_ID_LEN))
            .ok_or(DagSnapshotError::Invalid)?;
        let min_len = SNAPSHOT_HEADER_LEN
            .checked_add(min_nodes)
            .and_then(|len| len.checked_add(min_frontier))
            .ok_or(DagSnapshotError::Invalid)?;
        if bytes.len() < min_len {
            return Err(DagSnapshotError::Invalid);
        }

        let mut nodes = BTreeMap::new();
        let mut previous_node: Option<Id16> = None;
        let mut pos = SNAPSHOT_HEADER_LEN;
        for _ in 0..node_count {
            let id = read_id(bytes, &mut pos)?;
            if previous_node.is_some_and(|old| old >= id) {
                return Err(DagSnapshotError::Invalid);
            }
            let height = read_u32(bytes, &mut pos)?;
            let parent_count = read_u8(bytes, &mut pos)? as usize;
            if parent_count > MAX_SNAPSHOT_PARENTS {
                return Err(DagSnapshotError::Invalid);
            }
            let parent_bytes = parent_count
                .checked_mul(SNAPSHOT_ID_LEN)
                .ok_or(DagSnapshotError::Invalid)?;
            if bytes.len().saturating_sub(pos) < parent_bytes {
                return Err(DagSnapshotError::Invalid);
            }
            let mut parents = Vec::with_capacity(parent_count);
            let mut previous_parent: Option<Id16> = None;
            for _ in 0..parent_count {
                let parent = read_id(bytes, &mut pos)?;
                if previous_parent.is_some_and(|old| old >= parent) {
                    return Err(DagSnapshotError::Invalid);
                }
                parents.push(parent);
                previous_parent = Some(parent);
            }
            if nodes.insert(id, MsgNode { parents, height }).is_some() {
                return Err(DagSnapshotError::Invalid);
            }
            previous_node = Some(id);
        }

        let mut heads = BTreeSet::new();
        read_frontier(bytes, &mut pos, head_count, &mut heads)?;
        let mut gaps = BTreeSet::new();
        read_frontier(bytes, &mut pos, gap_count, &mut gaps)?;
        if pos != bytes.len() || heads.iter().any(|id| !nodes.contains_key(id)) {
            return Err(DagSnapshotError::Invalid);
        }

        let mut computed_gaps = BTreeSet::new();
        let mut referenced = BTreeSet::new();
        for node in nodes.values() {
            for parent in &node.parents {
                referenced.insert(*parent);
                if !nodes.contains_key(parent) {
                    computed_gaps.insert(*parent);
                }
            }
        }
        if computed_gaps != gaps || gaps.iter().any(|id| nodes.contains_key(id)) {
            return Err(DagSnapshotError::Invalid);
        }
        let computed_heads: BTreeSet<Id16> = nodes
            .keys()
            .filter(|id| !referenced.contains(id))
            .copied()
            .collect();
        if computed_heads != heads {
            return Err(DagSnapshotError::Invalid);
        }
        Ok(ConversationDag { nodes, heads, gaps })
    }

    /// Insert a message with its declared parents. Returns parent IDs we don't
    /// hold yet (gap markers, to WANT).
    pub fn insert(&mut self, id: Id16, parents: &[Id16]) -> Vec<Id16> {
        if self.nodes.contains_key(&id) {
            return Vec::new();
        }
        let mut missing = Vec::new();
        let mut height = 0u32;
        for p in parents {
            match self.nodes.get(p) {
                Some(n) => height = height.max(n.height + 1),
                None => {
                    self.gaps.insert(*p);
                    missing.push(*p);
                    height = height.max(1); // unknown parent still implies depth
                }
            }
            self.heads.remove(p);
        }
        self.gaps.remove(&id);
        self.nodes.insert(
            id,
            MsgNode {
                parents: parents.to_vec(),
                height,
            },
        );
        // It's a head unless something already claims it as parent (late arrival).
        let has_child = self.nodes.values().any(|n| n.parents.contains(&id));
        if !has_child {
            self.heads.insert(id);
        }
        missing
    }

    /// Current heads, ≤ 4, for the PARENTS TLV of an outgoing message.
    pub fn heads(&self) -> Vec<Id16> {
        self.heads
            .iter()
            .take(mmp_types::tlv::MAX_PARENTS)
            .copied()
            .collect()
    }

    /// Unresolved parent references (feed the session WANT set).
    pub fn gaps(&self) -> impl Iterator<Item = &Id16> {
        self.gaps.iter()
    }

    /// Normative display order: (height, BundleID asc) — identical on every
    /// device (doc 06 §5.2).
    pub fn display_order(&self) -> Vec<Id16> {
        let mut ids: Vec<Id16> = self.nodes.keys().copied().collect();
        ids.sort_by_key(|id| (self.nodes[id].height, *id));
        ids
    }

    /// Whether `id` is already known to this DAG.
    pub fn contains(&self, id: &Id16) -> bool {
        self.nodes.contains_key(id)
    }

    /// Number of messages currently held (does not count unresolved gaps).
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// True if no messages have been inserted yet.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

fn read_u8(bytes: &[u8], pos: &mut usize) -> Result<u8, DagSnapshotError> {
    let value = *bytes.get(*pos).ok_or(DagSnapshotError::Invalid)?;
    *pos += 1;
    Ok(value)
}

fn read_u32(bytes: &[u8], pos: &mut usize) -> Result<u32, DagSnapshotError> {
    let end = pos.checked_add(4).ok_or(DagSnapshotError::Invalid)?;
    let slice = bytes.get(*pos..end).ok_or(DagSnapshotError::Invalid)?;
    let value = u32::from_le_bytes(slice.try_into().map_err(|_| DagSnapshotError::Invalid)?);
    *pos = end;
    Ok(value)
}

fn read_id(bytes: &[u8], pos: &mut usize) -> Result<Id16, DagSnapshotError> {
    let end = pos
        .checked_add(SNAPSHOT_ID_LEN)
        .ok_or(DagSnapshotError::Invalid)?;
    let slice = bytes.get(*pos..end).ok_or(DagSnapshotError::Invalid)?;
    let mut id = [0u8; 16];
    id.copy_from_slice(slice);
    *pos = end;
    Ok(Id16(id))
}

fn read_frontier(
    bytes: &[u8],
    pos: &mut usize,
    count: usize,
    out: &mut BTreeSet<Id16>,
) -> Result<(), DagSnapshotError> {
    let mut previous = None;
    for _ in 0..count {
        let id = read_id(bytes, pos)?;
        if previous.is_some_and(|old| old >= id) || !out.insert(id) {
            return Err(DagSnapshotError::Invalid);
        }
        previous = Some(id);
    }
    Ok(())
}
