//! E7-S1 acceptance double, intentionally compiled only as an integration test.
//!
//! This is not a production provider and does not define a public
//! `mmp-core` capability. It gives the future provider/adapter work a concrete
//! token model and executable invariants without pretending that the current
//! `Box<dyn BundleStore + Send>` path can discover reservations. Capacity is
//! charged against the caller-supplied final encoded byte length, while the
//! in-memory payload is used only to verify the commit's declared length.

use std::collections::BTreeMap;

use mmp_core::store::BundleMeta;
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

type EntryMap = BTreeMap<Id16, (BundleMeta, Vec<u8>)>;
type ReservationMap = BTreeMap<ReservationToken, Reservation>;
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
    /// Last complete image. Reservations are deliberately not durable.
    durable: EntryMap,
    fail_next_commit: bool,
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
            durable: BTreeMap::new(),
            fail_next_commit: false,
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
