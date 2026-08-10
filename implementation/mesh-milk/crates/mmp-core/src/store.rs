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
    /// Live in the store, normal store-and-forward state.
    Stored,
    /// Hand-off intent persisted (I3): survives a crash as "handed off".
    HandoffPending,
}

/// Per-bundle metadata record (doc 11 §8, doc 12 §2): the header that carries
/// FSM/lifecycle state alongside the payload, rather than a separate
/// object-per-bundle state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BundleMeta {
    /// The bundle's content-derived ID.
    pub id: Id16,
    /// The bundle's class byte (doc 03), which drives lifetime/eviction rules.
    pub class: u8,
    /// The bundle's origin node, derived from its inline origin key at
    /// ingest — never trusted from elsewhere on the wire.
    pub origin: NodeId,
    /// The bundle's destination (a `NodeId` for unicast, or a group/broadcast
    /// `Id16` depending on class).
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
    /// Wire-encoded byte size of the stored bundle.
    pub size: u32,
    /// Monotonic ms when stored.
    pub stored_at: u64,
    /// Monotonic deadline: stored_at + clamped remaining lifetime.
    pub expires_at: u64,
    /// Local spray copy budget (doc 05 §2), clamped ≤ L_MAX on adoption.
    pub budget: u8,
    /// I4: own originals are never evicted before expiry.
    pub own_origin: bool,
    /// Current lifecycle state of this slot.
    pub slot: SlotState,
}

/// Result of an accepted [`BundleStore::try_put`] operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorePutOutcome {
    /// No bundle with this ID was present before the write.
    Inserted,
    /// The write replaced the bytes and metadata for an existing ID.
    Replaced,
}

/// Why a [`BundleStore::try_put`] operation did not accept a bundle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorePutError {
    /// The local store has no capacity for this bundle. This is a local
    /// resource decision and carries no wire-level meaning.
    Capacity,
}

impl BundleMeta {
    /// Milliseconds remaining before `expires_at`, relative to the
    /// caller-supplied monotonic `now_ms` (saturates to 0 once past expiry).
    pub fn remaining_ms(&self, now_ms: u64) -> u64 {
        self.expires_at.saturating_sub(now_ms)
    }
}

/// Storage seam. Implementations: in-memory (here), redb (`mmp-store`, std).
pub trait BundleStore {
    /// Stores (or replaces) a bundle's metadata and raw wire bytes.
    fn put(&mut self, meta: BundleMeta, bytes: &[u8]);
    /// Bounded write seam for stores that can reject a local resource request.
    ///
    /// The default preserves the legacy [`Self::put`] behavior and reports
    /// whether the ID was new or replaced. Embedded stores should override
    /// this method so a capacity rejection reaches [`crate::ingest`] instead
    /// of being misreported as a successful durable store. An implementation
    /// returning `Err(StorePutError::Capacity)` must fail closed: it must not
    /// mutate payload bytes, metadata, byte counters, the ID set, or any
    /// auxiliary provider state. Unknown or post-write failures are not
    /// represented by this enum until the provider has an explicit recovery
    /// or journal contract.
    fn try_put(&mut self, meta: BundleMeta, bytes: &[u8]) -> Result<StorePutOutcome, StorePutError> {
        let replaced = self.meta(&meta.id).is_some();
        self.put(meta, bytes);
        Ok(if replaced { StorePutOutcome::Replaced } else { StorePutOutcome::Inserted })
    }
    /// Fetches a bundle's metadata and raw wire bytes, if present.
    fn get(&self, id: &Id16) -> Option<(BundleMeta, Vec<u8>)>;
    /// Fetches a bundle's metadata only, if present.
    fn meta(&self, id: &Id16) -> Option<BundleMeta>;
    /// Replaces the stored metadata for `meta.id`, if it's still present
    /// (no-op otherwise). Does not touch the stored bytes.
    fn update_meta(&mut self, meta: BundleMeta);
    /// Deletes a bundle by ID; returns whether it was present.
    fn remove(&mut self, id: &Id16) -> bool;
    /// Sorted IDs in `[lo, hi]` — the RBSR contract (doc 06 §3).
    fn ids_range(&self, lo: &Id16, hi: &Id16) -> Vec<Id16>;
    /// Metadata for every bundle currently in the store.
    fn all_meta(&self) -> Vec<BundleMeta>;
    /// Total wire-encoded byte size of every stored bundle.
    fn total_bytes(&self) -> u64;
    /// Number of bundles currently in the store.
    fn len(&self) -> usize;
    /// Whether the store holds no bundles.
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
    /// Builds an empty store: no bundles, zero total bytes.
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
