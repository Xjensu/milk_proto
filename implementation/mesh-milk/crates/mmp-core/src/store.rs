//! BundleStore seam (doc 12 §2) + the in-memory reference implementation.
//!
//! Per-bundle FSM state is NOT object-per-bundle (doc 11 §8): it lives in the
//! record header (`SlotState`), and RBSR's contract is ordered `Id16` range
//! iteration.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use mmp_types::ids::{Id16, NodeId};

/// Bundle lifecycle slot state (doc 11 §4). Terminal states are deletions —
/// they never appear in a live store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotState {
    Stored,
    /// Hand-off intent persisted (I3): survives a crash as "handed off".
    HandoffPending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BundleMeta {
    pub id: Id16,
    pub class: u8,
    pub origin: NodeId,
    pub dest: Id16,
    /// Declared, signed wire byte — feeds `transfer_order` (doc 05 §6) and,
    /// when RBSR priority-class partitioning is eventually built (doc 03 §5.2
    /// phase 3), the partition key too (doc 18 §5.2 CR2bis-3: partitioning
    /// keys on the declared byte, never the clamped one). Never mutated
    /// locally — that's the whole point of the field split below.
    pub priority: u8,
    /// Receiver-local effective priority (doc 18 §5.2 admission/rendering
    /// clamp). Equal to `priority` for every class except 5 (ANNOUNCEMENT),
    /// where it may be clamped to `prio::NORMAL` absent a verified,
    /// budget-remaining AuthCert. Feeds `eviction_order` only — local store
    /// policy, never wire-phase ordering.
    pub effective_priority: u8,
    pub size: u32,
    /// Monotonic ms when stored.
    pub stored_at: u64,
    /// Monotonic deadline: stored_at + clamped remaining lifetime.
    pub expires_at: u64,
    /// Local spray copy budget (doc 05 §2), clamped ≤ L_MAX on adoption.
    pub budget: u8,
    /// I4: own originals are never evicted before expiry.
    pub own_origin: bool,
    pub slot: SlotState,
}

impl BundleMeta {
    pub fn remaining_ms(&self, now_ms: u64) -> u64 {
        self.expires_at.saturating_sub(now_ms)
    }
}

/// Storage seam. Implementations: in-memory (here), redb (`mmp-store`, std).
pub trait BundleStore {
    fn put(&mut self, meta: BundleMeta, bytes: &[u8]);
    fn get(&self, id: &Id16) -> Option<(BundleMeta, Vec<u8>)>;
    fn meta(&self, id: &Id16) -> Option<BundleMeta>;
    fn update_meta(&mut self, meta: BundleMeta);
    fn remove(&mut self, id: &Id16) -> bool;
    /// Sorted IDs in `[lo, hi]` — the RBSR contract (doc 06 §3).
    fn ids_range(&self, lo: &Id16, hi: &Id16) -> Vec<Id16>;
    fn all_meta(&self) -> Vec<BundleMeta>;
    fn total_bytes(&self) -> u64;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Delete expired bundles; returns their IDs (diag + receipts bookkeeping).
    fn expire(&mut self, now_ms: u64) -> Vec<Id16>;
}

/// Reference in-memory store (also the sim/test store).
#[derive(Default)]
pub struct MemStore {
    map: BTreeMap<Id16, (BundleMeta, Vec<u8>)>,
    bytes: u64,
}

impl MemStore {
    pub fn new() -> MemStore {
        MemStore::default()
    }
}

impl BundleStore for MemStore {
    fn put(&mut self, meta: BundleMeta, bytes: &[u8]) {
        self.bytes += bytes.len() as u64;
        if let Some((_, old)) = self.map.insert(meta.id, (meta, bytes.to_vec())) {
            self.bytes -= old.len() as u64;
        }
    }

    fn get(&self, id: &Id16) -> Option<(BundleMeta, Vec<u8>)> {
        self.map.get(id).cloned()
    }

    fn meta(&self, id: &Id16) -> Option<BundleMeta> {
        self.map.get(id).map(|(m, _)| *m)
    }

    fn update_meta(&mut self, meta: BundleMeta) {
        if let Some((m, _)) = self.map.get_mut(&meta.id) {
            *m = meta;
        }
    }

    fn remove(&mut self, id: &Id16) -> bool {
        match self.map.remove(id) {
            Some((_, b)) => {
                self.bytes -= b.len() as u64;
                true
            }
            None => false,
        }
    }

    fn ids_range(&self, lo: &Id16, hi: &Id16) -> Vec<Id16> {
        self.map.range(*lo..=*hi).map(|(id, _)| *id).collect()
    }

    fn all_meta(&self) -> Vec<BundleMeta> {
        self.map.values().map(|(m, _)| *m).collect()
    }

    fn total_bytes(&self) -> u64 {
        self.bytes
    }

    fn len(&self) -> usize {
        self.map.len()
    }

    fn expire(&mut self, now_ms: u64) -> Vec<Id16> {
        let dead: Vec<Id16> = self
            .map
            .iter()
            .filter(|(_, (m, _))| m.expires_at <= now_ms)
            .map(|(id, _)| *id)
            .collect();
        for id in &dead {
            self.remove(id);
        }
        dead
    }
}
