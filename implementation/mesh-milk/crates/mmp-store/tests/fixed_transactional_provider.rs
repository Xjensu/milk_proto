//! Private E7-S1 provider prototype.
//!
//! This is deliberately an integration-test-only implementation.  It proves
//! the reservation arithmetic and commit invariants without exposing a public
//! `TransactionalBundleStore` API before a production provider and producer
//! rollback path exist.

use std::collections::BTreeMap;

use mmp_core::store::{BundleMeta, BundleStore, SlotState, StorePutError, StorePutOutcome};
use mmp_types::ids::Id16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ReservationToken {
    nonce: u64,
    id: Id16,
    byte_len: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReserveError {
    Capacity,
    TooLarge,
    Busy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommitError {
    ReservationMismatch,
    Unavailable,
}

#[derive(Debug, Clone)]
struct Reservation {
    id: Id16,
    byte_len: u32,
    /// Additional bytes that must be available while the reservation is
    /// outstanding.  A replacement that shrinks an existing record does not
    /// free the old bytes until commit, so negative deltas reserve zero.
    reserved_bytes: u64,
    old_len: u64,
}

/// A bounded provider-local model.  Its token is intentionally private to the
/// test module, mirroring the opaque process-local handle required by E7-S1.
#[derive(Default)]
struct FixedProvider {
    entries: BTreeMap<Id16, (BundleMeta, Vec<u8>)>,
    max_bytes: u64,
    next_nonce: u64,
    reservations: BTreeMap<u64, Reservation>,
    fail_next_commit: bool,
}

impl FixedProvider {
    fn new(max_bytes: u64) -> Self {
        Self { max_bytes, ..Self::default() }
    }

    fn total_bytes(&self) -> u64 {
        self.entries.values().map(|(_, bytes)| bytes.len() as u64).sum()
    }

    fn reserved_bytes(&self) -> u64 {
        self.reservations
            .values()
            .map(|reservation| reservation.reserved_bytes)
            .sum()
    }

    fn outstanding_bytes(&self) -> u64 {
        self.reserved_bytes()
    }

    fn reserve_bundle(&mut self, meta: BundleMeta, byte_len: u32) -> Result<ReservationToken, ReserveError> {
        if usize::try_from(byte_len).ok() != Some(meta.size as usize) {
            return Err(ReserveError::TooLarge);
        }
        if self.reservations.values().any(|reservation| reservation.id == meta.id) {
            return Err(ReserveError::Busy);
        }
        let old_len = self.entries.get(&meta.id).map_or(0, |(_, bytes)| bytes.len() as u64);
        let projected = self
            .total_bytes()
            .checked_sub(old_len)
            .and_then(|used| used.checked_add(u64::from(byte_len)))
            .and_then(|used| used.checked_add(self.reserved_bytes()));
        if projected.is_none_or(|used| used > self.max_bytes) {
            return Err(ReserveError::Capacity);
        }

        self.next_nonce = self.next_nonce.checked_add(1).expect("test nonce exhausted");
        let nonce = self.next_nonce;
        let reserved_bytes = u64::from(byte_len).saturating_sub(old_len);
        self.reservations.insert(nonce, Reservation { id: meta.id, byte_len, reserved_bytes, old_len });
        Ok(ReservationToken { nonce, id: meta.id, byte_len })
    }

    fn commit_bundle(
        &mut self,
        token: ReservationToken,
        meta: BundleMeta,
        bytes: &[u8],
    ) -> Result<StorePutOutcome, CommitError> {
        let Some(reservation) = self.reservations.get(&token.nonce) else {
            return Err(CommitError::ReservationMismatch);
        };
        if reservation.id != token.id
            || reservation.byte_len != token.byte_len
            || meta.id != reservation.id
            || meta.size != reservation.byte_len
            || bytes.len() != reservation.byte_len as usize
            || self.entries.get(&reservation.id).map_or(0, |(_, stored)| stored.len() as u64) != reservation.old_len
        {
            return Err(CommitError::ReservationMismatch);
        }
        // Simulate a provider failure before publication.  Keeping the token
        // outstanding lets the caller explicitly abort/recover it.
        if self.fail_next_commit {
            self.fail_next_commit = false;
            return Err(CommitError::Unavailable);
        }

        let replaced = self.entries.insert(meta.id, (meta, bytes.to_vec())).is_some();
        self.reservations.remove(&token.nonce);
        Ok(if replaced { StorePutOutcome::Replaced } else { StorePutOutcome::Inserted })
    }

    fn abort_bundle(&mut self, token: ReservationToken) {
        let Some(reservation) = self.reservations.get(&token.nonce) else {
            return;
        };
        if reservation.id == token.id && reservation.byte_len == token.byte_len {
            self.reservations.remove(&token.nonce);
        }
    }
}

impl BundleStore for FixedProvider {
    fn put(&mut self, meta: BundleMeta, bytes: &[u8]) {
        self.entries.insert(meta.id, (meta, bytes.to_vec()));
    }

    fn try_put(&mut self, meta: BundleMeta, bytes: &[u8]) -> Result<StorePutOutcome, StorePutError> {
        if meta.size as usize != bytes.len() {
            return Err(StorePutError::Capacity);
        }
        if self.reservations.values().any(|reservation| reservation.id == meta.id) {
            return Err(StorePutError::Capacity);
        }
        let old_len = self.entries.get(&meta.id).map_or(0, |(_, old)| old.len() as u64);
        let projected = self
            .total_bytes()
            .saturating_sub(old_len)
            .saturating_add(bytes.len() as u64)
            .saturating_add(self.reserved_bytes());
        if projected > self.max_bytes {
            return Err(StorePutError::Capacity);
        }
        let replaced = self.entries.insert(meta.id, (meta, bytes.to_vec())).is_some();
        Ok(if replaced { StorePutOutcome::Replaced } else { StorePutOutcome::Inserted })
    }

    fn get(&self, id: &Id16) -> Option<(BundleMeta, Vec<u8>)> {
        self.entries.get(id).cloned()
    }

    fn meta(&self, id: &Id16) -> Option<BundleMeta> {
        self.entries.get(id).map(|(meta, _)| *meta)
    }

    fn update_meta(&mut self, meta: BundleMeta) {
        if let Some((stored, _)) = self.entries.get_mut(&meta.id) {
            *stored = meta;
        }
    }

    fn remove(&mut self, id: &Id16) -> bool {
        self.entries.remove(id).is_some()
    }

    fn ids_range(&self, lo: &Id16, hi: &Id16) -> Vec<Id16> {
        self.entries.range(*lo..=*hi).map(|(id, _)| *id).collect()
    }

    fn all_meta(&self) -> Vec<BundleMeta> {
        self.entries.values().map(|(meta, _)| *meta).collect()
    }

    fn total_bytes(&self) -> u64 {
        Self::total_bytes(self)
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    fn expire(&mut self, now_ms: u64) -> Vec<Id16> {
        let expired: Vec<_> = self
            .entries
            .iter()
            .filter(|(_, (meta, _))| meta.expires_at <= now_ms)
            .map(|(id, _)| *id)
            .collect();
        for id in &expired {
            self.entries.remove(id);
        }
        expired
    }
}

fn meta(id: u8, size: usize) -> BundleMeta {
    BundleMeta {
        id: Id16([id; 16]),
        class: 1,
        origin: Id16([2; 16]),
        dest: Id16([3; 16]),
        priority: 1,
        effective_priority: 1,
        size: size as u32,
        stored_at: 10,
        expires_at: 1000,
        budget: 4,
        own_origin: true,
        slot: SlotState::Stored,
    }
}

#[test]
fn exact_fill_and_replacement_charge_only_delta() {
    let mut provider = FixedProvider::new(12);
    let first = provider.reserve_bundle(meta(1, 8), 8).unwrap();
    assert_eq!(provider.commit_bundle(first, meta(1, 8), &[0; 8]), Ok(StorePutOutcome::Inserted));

    let replacement = provider.reserve_bundle(meta(1, 12), 12).unwrap();
    assert_eq!(provider.outstanding_bytes(), 4);
    assert_eq!(provider.commit_bundle(replacement, meta(1, 12), &[1; 12]), Ok(StorePutOutcome::Replaced));
    assert_eq!(provider.total_bytes(), 12);
    assert_eq!(provider.reserve_bundle(meta(2, 1), 1), Err(ReserveError::Capacity));
}

#[test]
fn outstanding_reservations_consume_capacity_and_abort_is_idempotent() {
    let mut provider = FixedProvider::new(5);
    let first = provider.reserve_bundle(meta(1, 3), 3).unwrap();
    assert_eq!(provider.reserve_bundle(meta(2, 3), 3), Err(ReserveError::Capacity));
    assert_eq!(provider.outstanding_bytes(), 3);

    provider.abort_bundle(first);
    provider.abort_bundle(first);
    assert_eq!(provider.outstanding_bytes(), 0);
    let second = provider.reserve_bundle(meta(2, 3), 3).unwrap();
    provider.abort_bundle(second);
    assert_eq!(provider.outstanding_bytes(), 0);
}

#[test]
fn stale_wrong_id_and_wrong_length_tokens_fail_closed() {
    let mut provider = FixedProvider::new(16);
    let token = provider.reserve_bundle(meta(1, 4), 4).unwrap();

    let mut wrong_id = token;
    wrong_id.id = Id16([9; 16]);
    assert_eq!(provider.commit_bundle(wrong_id, meta(9, 4), &[0; 4]), Err(CommitError::ReservationMismatch));
    assert_eq!(provider.outstanding_bytes(), 4);

    let mut wrong_len = token;
    wrong_len.byte_len = 3;
    assert_eq!(provider.commit_bundle(wrong_len, meta(1, 3), &[0; 3]), Err(CommitError::ReservationMismatch));
    assert_eq!(provider.outstanding_bytes(), 4);

    assert_eq!(provider.commit_bundle(token, meta(1, 4), &[0; 4]), Ok(StorePutOutcome::Inserted));
    assert_eq!(provider.outstanding_bytes(), 0);
    assert_eq!(provider.commit_bundle(token, meta(1, 4), &[0; 4]), Err(CommitError::ReservationMismatch));
    provider.abort_bundle(token); // consumed tokens cannot remove a commit
    assert_eq!(provider.get(&Id16([1; 16])).unwrap().1, vec![0; 4]);
}

#[test]
fn failed_commit_publishes_nothing_and_can_be_aborted() {
    let mut provider = FixedProvider::new(8);
    let token = provider.reserve_bundle(meta(1, 5), 5).unwrap();
    provider.fail_next_commit = true;

    assert_eq!(provider.commit_bundle(token, meta(1, 5), &[7; 5]), Err(CommitError::Unavailable));
    assert_eq!(provider.len(), 0);
    assert_eq!(provider.total_bytes(), 0);
    assert_eq!(provider.outstanding_bytes(), 5);

    provider.abort_bundle(token);
    assert_eq!(provider.outstanding_bytes(), 0);
    assert!(provider.get(&Id16([1; 16])).is_none());
}

#[test]
fn receive_try_put_is_bounded_and_replacement_uses_delta() {
    let mut provider = FixedProvider::new(4);
    assert_eq!(provider.try_put(meta(1, 4), &[0; 4]), Ok(StorePutOutcome::Inserted));
    assert_eq!(provider.try_put(meta(2, 1), &[1]), Err(StorePutError::Capacity));

    assert_eq!(provider.try_put(meta(1, 2), &[2; 2]), Ok(StorePutOutcome::Replaced));
    assert_eq!(provider.total_bytes(), 2);
    assert_eq!(provider.try_put(meta(2, 2), &[3; 2]), Ok(StorePutOutcome::Inserted));
    assert_eq!(provider.total_bytes(), 4);
}
