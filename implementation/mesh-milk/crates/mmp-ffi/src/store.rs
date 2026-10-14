//! Persistence adapter: wraps a shared, lockable [`SnapshotStore`] so it can
//! back a [`mmp_node::Node`] as an ordinary `Box<dyn BundleStore + Send>`
//! while the FFI layer *also* keeps a handle to drive persistence policy
//! (periodic flush on [`crate::MmpNode::tick`], explicit
//! [`crate::MmpNode::flush`]).
//!
//! `Node` owns its store as a `Box`, so the FFI keeps a second `Arc` handle
//! purely to drive persistence policy — persistence stays a host concern per
//! the sans-IO layering (doc 04 §7); the core crates (`mmp-core`,
//! `mmp-node`) stay untouched.
//!
//! # Lock ordering
//!
//! Fixed order everywhere in this crate: the [`crate::MmpNode`] node mutex
//! first, this store's mutex second. `Node`'s own methods take the node
//! lock and, while it is held, may call into this store (node → store, one
//! direction). Persistence-policy code (`tick`'s periodic flush, `flush`)
//! only ever locks the store mutex *after* the node lock has already been
//! released — never while it is held — so the two locks are never nested in
//! the reverse order and a deadlock cannot occur.

use std::sync::{Arc, Mutex, MutexGuard};

use mmp_core::store::{
    BundleMeta, BundleStore, StoreBatchEntry, StoreBatchError, StoreBatchReservation,
    StoreCommitError, StorePutError, StorePutOutcome, StoreReserveError,
};
use mmp_store::SnapshotStore;
use mmp_types::ids::Id16;

/// A [`SnapshotStore`] shared between the [`mmp_node::Node`] (which drives
/// it as its `BundleStore`) and the FFI layer (which drives persistence
/// timing). Every trait method just locks and delegates.
pub(crate) struct SharedStore(pub(crate) Arc<Mutex<SnapshotStore>>);

impl SharedStore {
    fn lock(&self) -> MutexGuard<'_, SnapshotStore> {
        self.0.lock().unwrap_or_else(|poison| poison.into_inner())
    }
}

impl BundleStore for SharedStore {
    fn put(&mut self, meta: BundleMeta, bytes: &[u8]) {
        self.lock().put(meta, bytes)
    }
    fn try_put(
        &mut self,
        meta: BundleMeta,
        bytes: &[u8],
    ) -> Result<StorePutOutcome, StorePutError> {
        self.lock().try_put(meta, bytes)
    }
    fn try_put_batch(
        &mut self,
        entries: &[StoreBatchEntry<'_>],
    ) -> Result<Vec<StorePutOutcome>, StoreBatchError> {
        self.lock().try_put_batch(entries)
    }
    fn try_reserve_batch_atomic(
        &mut self,
        metas: &[BundleMeta],
    ) -> Result<StoreBatchReservation, StoreReserveError> {
        self.lock().try_reserve_batch_atomic(metas)
    }
    fn commit_reserved_batch(
        &mut self,
        batch: &StoreBatchReservation,
        entries: &[StoreBatchEntry<'_>],
    ) -> Result<Vec<StorePutOutcome>, StoreCommitError> {
        self.lock().commit_reserved_batch(batch, entries)
    }
    fn abort_reserved_batch(
        &mut self,
        batch: &StoreBatchReservation,
    ) -> Result<(), mmp_core::store::StoreAbortError> {
        self.lock().abort_reserved_batch(batch)
    }
    fn get(&self, id: &Id16) -> Option<(BundleMeta, Vec<u8>)> {
        self.lock().get(id)
    }
    fn meta(&self, id: &Id16) -> Option<BundleMeta> {
        self.lock().meta(id)
    }
    fn update_meta(&mut self, meta: BundleMeta) {
        self.lock().update_meta(meta)
    }
    fn remove(&mut self, id: &Id16) -> bool {
        self.lock().remove(id)
    }
    fn ids_range(&self, lo: &Id16, hi: &Id16) -> Vec<Id16> {
        self.lock().ids_range(lo, hi)
    }
    fn all_meta(&self) -> Vec<BundleMeta> {
        self.lock().all_meta()
    }
    fn total_bytes(&self) -> u64 {
        self.lock().total_bytes()
    }
    fn len(&self) -> usize {
        self.lock().len()
    }
    fn expire(&mut self, now_ms: u64) -> Vec<Id16> {
        self.lock().expire(now_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmp_core::store::SlotState;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_PATH: AtomicU64 = AtomicU64::new(0);

    fn store() -> SharedStore {
        let path = std::env::temp_dir().join(format!(
            "mmp-ffi-shared-store-{}-{}",
            std::process::id(),
            NEXT_PATH.fetch_add(1, Ordering::Relaxed)
        ));
        SharedStore(Arc::new(Mutex::new(SnapshotStore::open(path))))
    }

    fn meta(tag: u8, size: u32) -> BundleMeta {
        BundleMeta {
            id: Id16([tag; 16]),
            class: 1,
            origin: Id16([0xa0; 16]),
            dest: Id16([0xb0; 16]),
            priority: 1,
            effective_priority: 1,
            size,
            stored_at: 1,
            expires_at: 10_000,
            budget: 4,
            own_origin: true,
            slot: SlotState::Stored,
        }
    }

    #[test]
    fn shared_store_forwards_atomic_batch_reserve_and_commit() {
        let mut store = store();
        let first = meta(1, 3);
        let second = meta(2, 2);
        let reservation = store
            .try_reserve_batch_atomic(&[first, second])
            .expect("SnapshotStore supports atomic batch reservations");
        let entries = [
            StoreBatchEntry::new(first, b"abc"),
            StoreBatchEntry::new(second, b"de"),
        ];

        assert_eq!(
            store.commit_reserved_batch(&reservation, &entries).unwrap(),
            [StorePutOutcome::Inserted, StorePutOutcome::Inserted,]
        );
        assert_eq!(store.len(), 2);
        assert_eq!(store.get(&first.id).unwrap().1, b"abc");
        assert_eq!(store.get(&second.id).unwrap().1, b"de");
    }

    #[test]
    fn shared_store_forwards_atomic_batch_abort() {
        let mut store = store();
        let candidate = meta(3, 4);
        let reservation = store
            .try_reserve_batch_atomic(&[candidate])
            .expect("SnapshotStore supports atomic batch reservations");

        assert_eq!(store.abort_reserved_batch(&reservation), Ok(()));
        assert_eq!(store.meta(&candidate.id), None);
        // A successful retry proves the adapter delegated the release rather
        // than retaining the provider's reservation locally.
        let retry = store
            .try_reserve_batch_atomic(&[candidate])
            .expect("aborted reservation must be released by SnapshotStore");
        let entries = [StoreBatchEntry::new(candidate, b"data")];
        assert_eq!(
            store.commit_reserved_batch(&retry, &entries).unwrap(),
            [StorePutOutcome::Inserted]
        );
    }
}
