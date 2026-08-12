//! E7-S1 acceptance double, intentionally compiled only as an integration test.
//!
//! This is not a production provider and does not define a public
//! `mmp-core` capability. It gives the future provider/adapter work a concrete
//! token model and executable invariants without pretending that the current
//! `Box<dyn BundleStore + Send>` path can discover reservations. Capacity is
//! charged against the caller-supplied final encoded byte length, while the
//! in-memory payload is used only to verify the commit's declared length.

use std::collections::{BTreeMap, BTreeSet};

use mmp_core::store::{BundleMeta, StoreBatchReservation, StoreReservation};
use mmp_types::ids::Id16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReserveError {
    Capacity,
    Invalid,
    Busy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommitError {
    ReservationMismatch,
    Capacity,
    Invalid,
    InjectedFailure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd)]
struct BatchToken {
    nonce: u64,
}

/// Private provider-local reservation handle. It intentionally has no
/// constructor or wire representation outside this test module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd)]
struct ReservationToken {
    nonce: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Reservation {
    id: Id16,
    byte_len: u32,
    reserved_bytes: u64,
    reserved_slot: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BatchItem {
    id: Id16,
    byte_len: u32,
    old_len: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BatchReservation {
    items: Vec<BatchItem>,
    reserved_bytes: u64,
    reserved_slots: usize,
}

type EntryMap = BTreeMap<Id16, (BundleMeta, Vec<u8>)>;
type ReservationMap = BTreeMap<ReservationToken, Reservation>;
type BatchMap = BTreeMap<BatchToken, BatchReservation>;
type StoreState = (u64, u64, usize, usize, EntryMap, ReservationMap);

#[derive(Debug, Clone)]
struct FixedStore {
    max_slots: usize,
    max_bytes: u64,
    next_token: u64,
    used_bytes: u64,
    reserved_bytes: u64,
    reserved_slots: usize,
    entries: EntryMap,
    reservations: ReservationMap,
    batches: BatchMap,
    /// Last complete image. Reservations are deliberately not durable.
    durable: EntryMap,
    fail_next_commit: bool,
    /// If set, fail a batch after this many staged writes. Staging is done on
    /// a clone, so an injected mid-commit fault must leave the visible state
    /// and the reservation unchanged.
    fail_batch_after: Option<usize>,
}

impl FixedStore {
    fn new(max_slots: usize, max_bytes: u64) -> FixedStore {
        FixedStore {
            max_slots,
            max_bytes,
            next_token: 1,
            used_bytes: 0,
            reserved_bytes: 0,
            reserved_slots: 0,
            entries: BTreeMap::new(),
            reservations: BTreeMap::new(),
            batches: BTreeMap::new(),
            durable: BTreeMap::new(),
            fail_next_commit: false,
            fail_batch_after: None,
        }
    }

    fn reserve(&mut self, meta: BundleMeta, byte_len: u32) -> Result<ReservationToken, ReserveError> {
        if meta.size != byte_len {
            return Err(ReserveError::Invalid);
        }
        if self.reservations.values().any(|reservation| reservation.id == meta.id) {
            return Err(ReserveError::Busy);
        }

        let requested = u64::from(byte_len);
        if requested > self.max_bytes {
            return Err(ReserveError::Capacity);
        }
        let (old_len, new_slot) = self
            .entries
            .get(&meta.id)
            .map(|(_, body)| (u64::try_from(body.len()).unwrap_or(u64::MAX), false))
            .unwrap_or((0, true));
        let extra_bytes = requested.saturating_sub(old_len);
        let projected_bytes = self
            .used_bytes
            .checked_add(self.reserved_bytes)
            .and_then(|used| used.checked_add(extra_bytes))
            .ok_or(ReserveError::Capacity)?;
        let projected_slots = self
            .entries
            .len()
            .checked_add(self.reserved_slots)
            .and_then(|slots| slots.checked_add(usize::from(new_slot)))
            .ok_or(ReserveError::Capacity)?;
        if projected_bytes > self.max_bytes || projected_slots > self.max_slots {
            return Err(ReserveError::Capacity);
        }

        let token = ReservationToken { nonce: self.next_token };
        self.next_token = self.next_token.checked_add(1).ok_or(ReserveError::Capacity)?;
        let reservation = Reservation {
            id: meta.id,
            byte_len,
            reserved_bytes: extra_bytes,
            reserved_slot: new_slot,
        };
        self.reserved_bytes = self
            .reserved_bytes
            .checked_add(reservation.reserved_bytes)
            .ok_or(ReserveError::Capacity)?;
        if reservation.reserved_slot {
            self.reserved_slots = self.reserved_slots.checked_add(1).ok_or(ReserveError::Capacity)?;
        }
        self.reservations.insert(token, reservation);
        Ok(token)
    }

    fn total_reserved_bytes(&self) -> u64 {
        self.reserved_bytes
            .saturating_add(self.batches.values().map(|batch| batch.reserved_bytes).sum())
    }

    fn total_reserved_slots(&self) -> usize {
        self.reserved_slots
            .saturating_add(self.batches.values().map(|batch| batch.reserved_slots).sum())
    }

    fn id_is_reserved(&self, id: &Id16) -> bool {
        self.reservations.values().any(|reservation| &reservation.id == id)
            || self.batches.values().any(|batch| batch.items.iter().any(|item| &item.id == id))
    }

    /// Private aggregate reservation model for the file-symbol + manifest
    /// operation. Every item is preflighted before any capacity counter is
    /// changed, so a rejected batch cannot leave a partial reservation.
    fn reserve_batch(&mut self, requests: &[(BundleMeta, u32)]) -> Result<BatchToken, ReserveError> {
        if requests.is_empty() {
            return Err(ReserveError::Invalid);
        }

        let mut ids = BTreeSet::new();
        let mut items = Vec::with_capacity(requests.len());
        let mut reserved_bytes = 0u64;
        let mut reserved_slots = 0usize;
        for (meta, byte_len) in requests {
            if meta.size != *byte_len || !ids.insert(meta.id) {
                return Err(ReserveError::Invalid);
            }
            if self.id_is_reserved(&meta.id) {
                return Err(ReserveError::Busy);
            }
            let requested = u64::from(*byte_len);
            if requested > self.max_bytes {
                return Err(ReserveError::Capacity);
            }
            let (old_len, new_slot) = self
                .entries
                .get(&meta.id)
                .map(|(_, body)| (u64::try_from(body.len()).unwrap_or(u64::MAX), false))
                .unwrap_or((0, true));
            let extra_bytes = requested.saturating_sub(old_len);
            reserved_bytes = reserved_bytes.checked_add(extra_bytes).ok_or(ReserveError::Capacity)?;
            reserved_slots = reserved_slots
                .checked_add(usize::from(new_slot))
                .ok_or(ReserveError::Capacity)?;
            items.push(BatchItem {
                id: meta.id,
                byte_len: *byte_len,
                old_len,
            });
        }

        let projected_bytes = self
            .used_bytes
            .checked_add(self.total_reserved_bytes())
            .and_then(|used| used.checked_add(reserved_bytes))
            .ok_or(ReserveError::Capacity)?;
        let projected_slots = self
            .entries
            .len()
            .checked_add(self.total_reserved_slots())
            .and_then(|slots| slots.checked_add(reserved_slots))
            .ok_or(ReserveError::Capacity)?;
        if projected_bytes > self.max_bytes || projected_slots > self.max_slots {
            return Err(ReserveError::Capacity);
        }

        let nonce = self.next_token.checked_add(1).ok_or(ReserveError::Capacity)?;
        self.next_token = nonce;
        self.batches.insert(
            BatchToken { nonce },
            BatchReservation { items, reserved_bytes, reserved_slots },
        );
        Ok(BatchToken { nonce })
    }

    /// Commit all batch entries as one visible generation. The staged clone
    /// is the test double's stand-in for a provider journal/atomic rename:
    /// any injected failure leaves the old entries and reservation intact.
    fn commit_batch(
        &mut self,
        token: BatchToken,
        entries: &[(BundleMeta, Vec<u8>)],
    ) -> Result<(), CommitError> {
        let reservation = self.batches.get(&token).cloned().ok_or(CommitError::ReservationMismatch)?;
        if entries.len() != reservation.items.len() {
            return Err(CommitError::ReservationMismatch);
        }

        let mut supplied = BTreeMap::new();
        for (meta, bytes) in entries {
            if supplied.insert(meta.id, (meta, bytes)).is_some() {
                return Err(CommitError::ReservationMismatch);
            }
        }
        for item in &reservation.items {
            let Some((meta, bytes)) = supplied.get(&item.id) else {
                return Err(CommitError::ReservationMismatch);
            };
            if meta.size != item.byte_len
                || bytes.len() != item.byte_len as usize
                || self.entries.get(&item.id).map_or(0, |(_, body)| body.len() as u64) != item.old_len
            {
                return Err(CommitError::ReservationMismatch);
            }
        }

        let fail_after = self.fail_batch_after.take();
        let mut staged = self.entries.clone();
        for (index, (meta, bytes)) in entries.iter().enumerate() {
            if fail_after == Some(index) {
                return Err(CommitError::InjectedFailure);
            }
            staged.insert(meta.id, (*meta, bytes.clone()));
        }
        let new_used = staged
            .values()
            .try_fold(0u64, |used, (_, body)| used.checked_add(body.len() as u64))
            .ok_or(CommitError::Capacity)?;
        if new_used > self.max_bytes || staged.len() > self.max_slots {
            return Err(CommitError::Capacity);
        }

        self.entries = staged;
        self.used_bytes = new_used;
        self.batches.remove(&token);
        Ok(())
    }

    fn abort_batch(&mut self, token: BatchToken) {
        self.batches.remove(&token);
    }

    fn commit(
        &mut self,
        token: ReservationToken,
        meta: BundleMeta,
        bytes: &[u8],
    ) -> Result<(), CommitError> {
        let reservation = *self.reservations.get(&token).ok_or(CommitError::ReservationMismatch)?;
        if reservation.id != meta.id
            || reservation.byte_len != u32::try_from(bytes.len()).map_err(|_| CommitError::Invalid)?
            || meta.size != reservation.byte_len
        {
            return Err(CommitError::ReservationMismatch);
        }
        if self.fail_next_commit {
            self.fail_next_commit = false;
            return Err(CommitError::InjectedFailure);
        }

        let old_len = self
            .entries
            .get(&meta.id)
            .map(|(_, body)| u64::try_from(body.len()).unwrap_or(u64::MAX))
            .unwrap_or(0);
        let new_len = u64::try_from(bytes.len()).map_err(|_| CommitError::Invalid)?;
        let new_used = self
            .used_bytes
            .checked_sub(old_len)
            .and_then(|used| used.checked_add(new_len))
            .ok_or(CommitError::Capacity)?;
        let new_slots = self
            .entries
            .len()
            .checked_add(usize::from(!self.entries.contains_key(&meta.id)))
            .ok_or(CommitError::Capacity)?;
        if new_used > self.max_bytes || new_slots > self.max_slots {
            return Err(CommitError::Capacity);
        }

        self.entries.insert(meta.id, (meta, bytes.to_vec()));
        self.used_bytes = new_used;
        self.reservations.remove(&token);
        self.reserved_bytes = self.reserved_bytes.saturating_sub(reservation.reserved_bytes);
        if reservation.reserved_slot {
            self.reserved_slots = self.reserved_slots.saturating_sub(1);
        }
        Ok(())
    }

    /// Release an uncommitted token. Releasing an unknown or already-consumed
    /// token is deliberately idempotent and cannot remove a committed entry.
    fn abort(&mut self, token: ReservationToken) {
        let Some(reservation) = self.reservations.remove(&token) else { return };
        self.reserved_bytes = self.reserved_bytes.saturating_sub(reservation.reserved_bytes);
        if reservation.reserved_slot {
            self.reserved_slots = self.reserved_slots.saturating_sub(1);
        }
    }

    fn persist(&mut self) {
        self.durable = self.entries.clone();
    }

    fn recover(&self) -> FixedStore {
        let mut recovered = FixedStore::new(self.max_slots, self.max_bytes);
        recovered.entries = self.durable.clone();
        recovered.durable = self.durable.clone();
        recovered.used_bytes = recovered
            .entries
            .values()
            .try_fold(0u64, |used, (_, body)| used.checked_add(u64::try_from(body.len()).ok()?))
            .unwrap_or(u64::MAX);
        recovered
    }

    fn state(&self) -> StoreState {
        (
            self.used_bytes,
            self.reserved_bytes,
            self.entries.len(),
            self.reserved_slots,
            self.entries.clone(),
            self.reservations.clone(),
        )
    }
}

fn meta(id: u8, size: u32) -> BundleMeta {
    BundleMeta {
        id: Id16([id; 16]),
        class: 1,
        origin: Id16([2; 16]),
        dest: Id16([3; 16]),
        priority: 1,
        effective_priority: 1,
        size,
        stored_at: 10,
        expires_at: 1000,
        budget: 4,
        own_origin: false,
        slot: mmp_core::store::SlotState::Stored,
    }
}

#[test]
fn exact_fill_and_one_byte_over_rejection() {
    let mut store = FixedStore::new(1, 8);
    let token = store.reserve(meta(1, 8), 8).unwrap();
    store.commit(token, meta(1, 8), &[0xaa; 8]).unwrap();

    assert_eq!(store.reserve(meta(2, 1), 1), Err(ReserveError::Capacity));
    assert_eq!(store.used_bytes, 8);
    assert!(store.reservations.is_empty());
}

#[test]
fn replacement_uses_delta_instead_of_double_counting_old_bytes() {
    let mut store = FixedStore::new(1, 10);
    let first = store.reserve(meta(1, 8), 8).unwrap();
    store.commit(first, meta(1, 8), &[0xaa; 8]).unwrap();

    let replacement = store.reserve(meta(1, 10), 10).unwrap();
    store.commit(replacement, meta(1, 10), &[0xbb; 10]).unwrap();
    assert_eq!(store.used_bytes, 10);
    assert_eq!(store.entries.len(), 1);
}

#[test]
fn outstanding_reservations_consume_capacity_until_abort() {
    let mut store = FixedStore::new(2, 10);
    let first = store.reserve(meta(1, 6), 6).unwrap();
    assert_eq!(store.reserve(meta(1, 6), 6), Err(ReserveError::Busy));
    assert_eq!(store.reserve(meta(2, 5), 5), Err(ReserveError::Capacity));
    store.abort(first);
    let second = store.reserve(meta(2, 5), 5).unwrap();
    store.commit(second, meta(2, 5), &[0xbb; 5]).unwrap();
    assert_eq!(store.used_bytes, 5);
}

#[test]
fn abort_is_idempotent_and_committed_tokens_are_not_reusable() {
    let mut store = FixedStore::new(1, 8);
    let token = store.reserve(meta(1, 4), 4).unwrap();
    store.abort(token);
    store.abort(token);
    assert_eq!(store.used_bytes, 0);
    assert_eq!(store.reserved_bytes, 0);
    assert_eq!(store.commit(token, meta(1, 4), &[0xaa; 4]), Err(CommitError::ReservationMismatch));

    let committed = store.reserve(meta(1, 4), 4).unwrap();
    store.commit(committed, meta(1, 4), &[0xaa; 4]).unwrap();
    store.abort(committed);
    assert_eq!(store.entries.len(), 1);
    assert_eq!(store.used_bytes, 4);
}

#[test]
fn stale_wrong_id_and_wrong_length_commits_fail_without_consuming_reservation() {
    let mut store = FixedStore::new(1, 8);
    let token = store.reserve(meta(1, 4), 4).unwrap();
    assert_eq!(
        store.commit(token, meta(2, 4), &[0xaa; 4]),
        Err(CommitError::ReservationMismatch)
    );
    assert_eq!(
        store.commit(token, meta(1, 4), &[0xaa; 3]),
        Err(CommitError::ReservationMismatch)
    );
    assert!(store.reservations.contains_key(&token));
    store.commit(token, meta(1, 4), &[0xaa; 4]).unwrap();
    assert_eq!(store.commit(token, meta(1, 4), &[0xaa; 4]), Err(CommitError::ReservationMismatch));
}

#[test]
fn failed_commit_preserves_runtime_state_and_recovers_last_durable_generation() {
    let mut store = FixedStore::new(1, 8);
    let first = store.reserve(meta(1, 4), 4).unwrap();
    store.commit(first, meta(1, 4), &[0xaa; 4]).unwrap();
    store.persist();
    let before = store.state();

    let replacement = store.reserve(meta(1, 8), 8).unwrap();
    let reserved = store.state();
    store.fail_next_commit = true;
    assert_eq!(
        store.commit(replacement, meta(1, 8), &[0xbb; 8]),
        Err(CommitError::InjectedFailure)
    );
    assert_eq!(store.state(), reserved, "failed commit leaves reservation and bytes unchanged");

    let recovered = store.recover();
    assert_eq!(recovered.entries.get(&Id16([1; 16])).unwrap().1, &[0xaa; 4]);
    assert_eq!(recovered.used_bytes, before.0);
    store.abort(replacement);
    assert!(store.reservations.is_empty());
}

#[test]
fn aggregate_reservation_rejects_partial_capacity_and_duplicate_ids() {
    let mut store = FixedStore::new(3, 12);
    let first = store.reserve(meta(1, 4), 4).unwrap();
    store.commit(first, meta(1, 4), &[0xaa; 4]).unwrap();

    let before = store.state();
    assert_eq!(
        store.reserve_batch(&[(meta(2, 5), 5), (meta(3, 4), 4)]),
        Err(ReserveError::Capacity)
    );
    assert_eq!(store.state(), before, "a rejected batch must not reserve a partial prefix");
    assert_eq!(
        store.reserve_batch(&[(meta(2, 5), 5), (meta(2, 5), 5)]),
        Err(ReserveError::Invalid)
    );
    assert_eq!(store.state(), before, "duplicate IDs must not mutate capacity state");

    let batch = store.reserve_batch(&[(meta(2, 5), 5), (meta(3, 3), 3)]).unwrap();
    assert_eq!(store.total_reserved_bytes(), 8);
    assert_eq!(store.total_reserved_slots(), 2);
    store.abort_batch(batch);
    assert_eq!(store.total_reserved_bytes(), 0);
    assert_eq!(store.total_reserved_slots(), 0);
}

#[test]
fn aggregate_commit_failure_keeps_old_generation_and_success_publishes_complete_generation() {
    let mut store = FixedStore::new(3, 20);
    let first = store.reserve(meta(1, 4), 4).unwrap();
    store.commit(first, meta(1, 4), &[0xaa; 4]).unwrap();
    store.persist();
    let old_visible = store.entries.clone();
    let old_durable = store.durable.clone();

    let requests = [(meta(1, 6), 6), (meta(2, 5), 5), (meta(3, 3), 3)];
    let token = store.reserve_batch(&requests).unwrap();
    let reserved_bytes = store.total_reserved_bytes();
    store.fail_batch_after = Some(1);
    let entries = vec![
        (meta(1, 6), vec![0xbb; 6]),
        (meta(2, 5), vec![0xcc; 5]),
        (meta(3, 3), vec![0xdd; 3]),
    ];
    assert_eq!(store.commit_batch(token, &entries), Err(CommitError::InjectedFailure));
    assert_eq!(store.entries, old_visible, "failed batch must publish no visible prefix");
    assert_eq!(store.durable, old_durable, "failed batch must not rewrite the durable generation");
    assert!(store.batches.contains_key(&token), "failed batch remains abortable");
    assert_eq!(store.total_reserved_bytes(), reserved_bytes);
    assert_eq!(store.recover().entries, old_durable, "restart must recover the old generation");
    store.abort_batch(token);
    assert_eq!(store.total_reserved_bytes(), 0);

    let token = store.reserve_batch(&requests).unwrap();
    store.commit_batch(token, &entries).unwrap();
    assert_eq!(store.entries.len(), 3);
    assert!(store.batches.is_empty(), "successful batch consumes its reservation exactly once");
    store.persist();
    let recovered = store.recover();
    assert_eq!(recovered.entries, store.entries, "restart after success must recover the complete generation");
    assert_eq!(recovered.entries.get(&Id16([2; 16])).unwrap().1, vec![0xcc; 5]);
    assert_eq!(recovered.entries.get(&Id16([3; 16])).unwrap().1, vec![0xdd; 3]);
}

#[test]
fn external_provider_can_construct_runtime_reservation_handles() {
    let id = Id16([0x5a; 16]);
    let single = StoreReservation::from_parts(17, id, 4, 1);
    let single_next = StoreReservation::from_parts(17, id, 4, 2);
    assert_ne!(single.key(), single_next.key());

    let batch = StoreBatchReservation::from_parts(17, 3);
    let batch_next = StoreBatchReservation::from_parts(17, 4);
    assert_ne!(batch.key(), batch_next.key());
}
