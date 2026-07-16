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

#[derive(Default)]
pub struct ConversationDag {
    nodes: BTreeMap<Id16, MsgNode>,
    /// Messages with no known children — the PARENTS of our next send.
    heads: BTreeSet<Id16>,
    /// Referenced-but-unknown parents (WANT set feed).
    gaps: BTreeSet<Id16>,
}

impl ConversationDag {
    pub fn new() -> ConversationDag {
        ConversationDag::default()
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
        self.nodes.insert(id, MsgNode { parents: parents.to_vec(), height });
        // It's a head unless something already claims it as parent (late arrival).
        let has_child = self.nodes.values().any(|n| n.parents.contains(&id));
        if !has_child {
            self.heads.insert(id);
        }
        missing
    }

    /// Current heads, ≤ 4, for the PARENTS TLV of an outgoing message.
    pub fn heads(&self) -> Vec<Id16> {
        self.heads.iter().take(mmp_types::tlv::MAX_PARENTS).copied().collect()
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

    pub fn contains(&self, id: &Id16) -> bool {
        self.nodes.contains_key(id)
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}
