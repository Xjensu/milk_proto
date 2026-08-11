//! BundleStore seam (doc 12 §2) + the in-memory reference implementation.
//!
//! Per-bundle FSM state is NOT object-per-bundle (doc 11 §8): it lives in the
//! record header (`SlotState`), and RBSR's contract is ordered `Id16` range
//! iteration.

use alloc::collections::BTreeMap;
use alloc::collections::BTreeSet;
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

/// A borrowed bundle write used by [`BundleStore::try_put_batch`].
///
/// The metadata is copied into the request while the encoded wire bytes are
/// borrowed for the duration of the provider call. Keeping the request
/// lifetime on the byte slice makes the method usable through `dyn BundleStore`
/// without an associated batch type or provider-specific allocation contract.
#[derive(Debug, Clone, Copy)]
pub struct StoreBatchEntry<'a> {
    /// Metadata that will be published with this bundle.
    pub meta: BundleMeta,
    /// Complete encoded wire bytes for the bundle.
    pub bytes: &'a [u8],
}

impl<'a> StoreBatchEntry<'a> {
    /// Construct one borrowed batch entry.
    pub const fn new(meta: BundleMeta, bytes: &'a [u8]) -> StoreBatchEntry<'a> {
        StoreBatchEntry { meta, bytes }
    }
}

/// Why a [`BundleStore::try_put_batch`] operation did not accept the batch.
///
/// This is deliberately separate from [`StorePutError`]. Adding variants to
/// `StorePutError` would break existing exhaustive receive-side mappings, and
/// a batch has two additional failure modes: unsupported capability and an
/// invalid aggregate request. `Capacity` has the same local-resource meaning
/// as [`StorePutError::Capacity`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreBatchError {
    /// The provider exposes only the legacy single-bundle write seam.
    Unsupported,
    /// The batch is malformed (for example a duplicate ID or size mismatch).
    Invalid,
    /// The batch cannot be represented within the provider's local accounting
    /// limits, including checked byte-counter overflow.
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
    /// Atomically admit a set of bundle writes when the provider supports the
    /// capability.
    ///
    /// The default is fail-closed: legacy stores do not receive a sequence of
    /// ordinary `try_put` calls, so they cannot accidentally be mistaken for
    /// an aggregate transaction. A successful call returns one outcome per
    /// input entry, in input order. Implementations must validate the whole
    /// request before making any write visible. This method only promises
    /// provider-call atomic visibility; it makes no claim about durable
    /// crash-recovery or unknown-commit handling.
    fn try_put_batch(
        &mut self,
        entries: &[StoreBatchEntry<'_>],
    ) -> Result<Vec<StorePutOutcome>, StoreBatchError> {
        let _ = entries;
        Err(StoreBatchError::Unsupported)
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

    fn try_put_batch(
        &mut self,
        entries: &[StoreBatchEntry<'_>],
    ) -> Result<Vec<StorePutOutcome>, StoreBatchError> {
        if entries.is_empty() {
            return Ok(Vec::new());
        }

        // Validate the existing accounting before calculating replacement
        // deltas. `put` is intentionally the legacy infallible seam, while a
        // batch must never publish a state whose byte counter is ambiguous.
        let mut accounted = 0u64;
        for (_, bytes) in self.map.values() {
            let len = u64::try_from(bytes.len()).map_err(|_| StoreBatchError::Capacity)?;
            accounted = accounted
                .checked_add(len)
                .ok_or(StoreBatchError::Capacity)?;
        }
        if accounted != self.bytes {
            return Err(StoreBatchError::Invalid);
        }

        let mut ids = BTreeSet::new();
        let mut added = 0u64;
        let mut replaced_bytes = 0u64;
        let mut outcomes = Vec::with_capacity(entries.len());

        // Preflight every item and the aggregate replacement delta before
        // cloning/staging the map. In particular, a later duplicate or bad
        // declared length cannot leave an earlier item visible.
        for entry in entries {
            let byte_len =
                u64::try_from(entry.bytes.len()).map_err(|_| StoreBatchError::Capacity)?;
            if byte_len != u64::from(entry.meta.size) || !ids.insert(entry.meta.id) {
                return Err(StoreBatchError::Invalid);
            }
            added = added
                .checked_add(byte_len)
                .ok_or(StoreBatchError::Capacity)?;
            if let Some((_, old_bytes)) = self.map.get(&entry.meta.id) {
                let old_len =
                    u64::try_from(old_bytes.len()).map_err(|_| StoreBatchError::Capacity)?;
                replaced_bytes = replaced_bytes
                    .checked_add(old_len)
                    .ok_or(StoreBatchError::Capacity)?;
                outcomes.push(StorePutOutcome::Replaced);
            } else {
                outcomes.push(StorePutOutcome::Inserted);
            }
        }

        let remaining = self
            .bytes
            .checked_sub(replaced_bytes)
            .ok_or(StoreBatchError::Invalid)?;
        let projected_bytes = remaining
            .checked_add(added)
            .ok_or(StoreBatchError::Capacity)?;

        // Stage all allocations and replacements in a private image. The
        // visible map and accounting counter are swapped only after every
        // entry has been accepted, so a rejected batch cannot expose a prefix.
        let mut staged = self.map.clone();
        for entry in entries {
            staged.insert(entry.meta.id, (entry.meta, entry.bytes.to_vec()));
        }
        self.map = staged;
        self.bytes = projected_bytes;
        Ok(outcomes)
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

#[cfg(test)]
mod tests {
    use alloc::vec;

    use super::{
        BundleMeta, BundleStore, MemStore, SlotState, StoreBatchEntry, StoreBatchError,
        StorePutOutcome,
    };
    use mmp_types::ids::Id16;

    fn meta(id: u8, size: usize) -> BundleMeta {
        BundleMeta {
            id: Id16([id; 16]),
            class: 1,
            origin: Id16([2; 16]),
            dest: Id16([3; 16]),
            priority: 1,
            effective_priority: 1,
            size: size as u32,
            stored_at: 0,
            expires_at: 60_000,
            budget: 1,
            own_origin: false,
            slot: SlotState::Stored,
        }
    }

    /// A complete legacy implementation that deliberately does not override
    /// the optional batch method. This keeps the default-capability test close
    /// to the trait-object boundary used by hosts.
    struct LegacyStore(MemStore);

    impl BundleStore for LegacyStore {
        fn put(&mut self, meta: BundleMeta, bytes: &[u8]) {
            self.0.put(meta, bytes);
        }

        fn get(&self, id: &Id16) -> Option<(BundleMeta, alloc::vec::Vec<u8>)> {
            self.0.get(id)
        }

        fn meta(&self, id: &Id16) -> Option<BundleMeta> {
            self.0.meta(id)
        }

        fn update_meta(&mut self, meta: BundleMeta) {
            self.0.update_meta(meta);
        }

        fn remove(&mut self, id: &Id16) -> bool {
            self.0.remove(id)
        }

        fn ids_range(&self, lo: &Id16, hi: &Id16) -> alloc::vec::Vec<Id16> {
            self.0.ids_range(lo, hi)
        }

        fn all_meta(&self) -> alloc::vec::Vec<BundleMeta> {
            self.0.all_meta()
        }

        fn total_bytes(&self) -> u64 {
            self.0.total_bytes()
        }

        fn len(&self) -> usize {
            self.0.len()
        }

        fn expire(&mut self, now_ms: u64) -> alloc::vec::Vec<Id16> {
            self.0.expire(now_ms)
        }
    }

    #[test]
    fn default_batch_capability_is_object_safe_and_fails_closed() {
        let mut store = LegacyStore(MemStore::new());
        let bytes = [1u8];
        let entries = [StoreBatchEntry::new(meta(1, bytes.len()), &bytes)];
        let trait_store: &mut dyn BundleStore = &mut store;
        assert_eq!(
            trait_store.try_put_batch(&entries),
            Err(StoreBatchError::Unsupported)
        );
        assert!(store.0.is_empty());
    }

    #[test]
    fn mem_store_batch_publishes_all_entries_and_reports_replacements() {
        let mut store = MemStore::new();
        assert_eq!(
            store.try_put(meta(1, 2), &[9, 9]),
            Ok(StorePutOutcome::Inserted)
        );

        let replacement = [1u8];
        let inserted = [2u8, 3, 4];
        let entries = [
            StoreBatchEntry::new(meta(1, replacement.len()), &replacement),
            StoreBatchEntry::new(meta(2, inserted.len()), &inserted),
        ];
        assert_eq!(
            store.try_put_batch(&entries),
            Ok(vec![StorePutOutcome::Replaced, StorePutOutcome::Inserted])
        );
        assert_eq!(store.total_bytes(), 4);
        assert_eq!(store.get(&Id16([1; 16])).map(|(_, b)| b), Some(vec![1]));
        assert_eq!(
            store.get(&Id16([2; 16])).map(|(_, b)| b),
            Some(vec![2, 3, 4])
        );
    }

    #[test]
    fn mem_store_batch_rejects_duplicate_or_bad_size_without_a_visible_prefix() {
        let mut store = MemStore::new();
        assert_eq!(
            store.try_put(meta(9, 1), &[9]),
            Ok(StorePutOutcome::Inserted)
        );

        let first = [1u8, 2];
        let duplicate = [3u8];
        let duplicate_entries = [
            StoreBatchEntry::new(meta(4, first.len()), &first),
            StoreBatchEntry::new(meta(4, duplicate.len()), &duplicate),
        ];
        assert_eq!(
            store.try_put_batch(&duplicate_entries),
            Err(StoreBatchError::Invalid)
        );
        assert_eq!(store.len(), 1);
        assert_eq!(store.total_bytes(), 1);
        assert!(store.get(&Id16([4; 16])).is_none());
        assert_eq!(store.get(&Id16([9; 16])).map(|(_, b)| b), Some(vec![9]));

        let bad_bytes = [5u8];
        let bad_size = [StoreBatchEntry::new(meta(5, 2), &bad_bytes)];
        assert_eq!(
            store.try_put_batch(&bad_size),
            Err(StoreBatchError::Invalid)
        );
        assert_eq!(store.len(), 1);
        assert_eq!(store.total_bytes(), 1);
        assert!(store.get(&Id16([5; 16])).is_none());
    }

    #[test]
    fn mem_store_batch_rejects_inconsistent_byte_accounting_without_mutation() {
        let mut store = MemStore::new();
        // This state can only arise from a provider bug or an interrupted
        // legacy operation; the batch seam must not compound it.
        store.bytes = u64::MAX;
        let bytes = [1u8];
        let entries = [StoreBatchEntry::new(meta(6, bytes.len()), &bytes)];
        assert_eq!(store.try_put_batch(&entries), Err(StoreBatchError::Invalid));
        assert_eq!(store.total_bytes(), u64::MAX);
        assert!(store.is_empty());
    }
}
