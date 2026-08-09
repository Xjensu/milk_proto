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

use mmp_core::store::{BundleMeta, BundleStore, StorePutError, StorePutOutcome};
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
    fn try_put(&mut self, meta: BundleMeta, bytes: &[u8]) -> Result<StorePutOutcome, StorePutError> {
        self.lock().try_put(meta, bytes)
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
