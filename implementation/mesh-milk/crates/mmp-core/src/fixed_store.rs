//! Fixed-capacity, allocation-free reference provider for the embedded seam.
//!
//! This is a host-testable capacity model, not a flash driver. It deliberately
//! makes no durability or power-loss claim and does not implement the legacy
//! allocating [`crate::BundleStore`] compatibility trait.

use mmp_types::consts::MAX_BUNDLE;
use mmp_types::ids::Id16;

use crate::embedded_store::EmbeddedBundleStore;
use crate::store::{BundleMeta, StorePutOutcome, StoreReadError};

use core::sync::atomic::{AtomicUsize, Ordering};

static NEXT_INSTANCE_ID: AtomicUsize = AtomicUsize::new(1);

fn allocate_instance_id() -> Option<u64> {
    NEXT_INSTANCE_ID
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .ok()
        .map(|id| id as u64)
}

/// Construction and local admission errors for [`FixedBundleStore`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixedStoreError {
    /// A const capacity is zero or exceeds the protocol bundle cap.
    InvalidConfiguration,
    /// The requested write cannot fit the fixed slot/byte budget.
    Capacity,
    /// The ID already has an outstanding reservation.
    Busy,
    /// The metadata or payload does not match the reserved request.
    InvalidRequest,
    /// The reserved record changed before commit.
    Conflict,
    /// The token is stale, forged, or belongs to another provider instance.
    InvalidToken,
}

/// Process-local token returned by [`FixedBundleStore::try_reserve`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedReservation {
    provider: u64,
    instance: u64,
    nonce: u64,
    id: Id16,
    declared_size: u32,
    old_len: usize,
}

#[derive(Debug, Clone, Copy)]
struct FixedSlot<const B: usize> {
    meta: BundleMeta,
    bytes: [u8; B],
    len: usize,
}

#[derive(Debug, Clone, Copy)]
struct PendingReservation {
    token: FixedReservation,
    reserved_bytes: u64,
    reserves_slot: bool,
}

/// Fixed-capacity reference provider.
///
/// `SLOTS` is both the live-slot and reservation-table bound. `B` is the
/// inline payload capacity per slot. `TOTAL` is the aggregate encoded-byte
/// budget. All three limits are local implementation policy, not wire fields.
pub struct FixedBundleStore<const SLOTS: usize, const B: usize, const TOTAL: u64> {
    slots: [Option<FixedSlot<B>>; SLOTS],
    reservations: [Option<PendingReservation>; SLOTS],
    used_bytes: u64,
    reserved_bytes: u64,
    reserved_slots: usize,
    next_nonce: u64,
    provider: u64,
    instance: u64,
}

impl<const SLOTS: usize, const B: usize, const TOTAL: u64> FixedBundleStore<SLOTS, B, TOTAL> {
    /// Construct an empty provider after validating static capacity inputs.
    pub fn new(provider: u64) -> Result<Self, FixedStoreError> {
        if SLOTS == 0 || B == 0 || TOTAL == 0 || B > MAX_BUNDLE {
            return Err(FixedStoreError::InvalidConfiguration);
        }
        let instance = allocate_instance_id().ok_or(FixedStoreError::InvalidConfiguration)?;
        Ok(Self {
            slots: [None; SLOTS],
            reservations: [None; SLOTS],
            used_bytes: 0,
            reserved_bytes: 0,
            reserved_slots: 0,
            next_nonce: 0,
            provider,
            instance,
        })
    }

    /// Bytes currently committed, excluding outstanding reservations.
    pub fn used_bytes(&self) -> u64 {
        self.used_bytes
    }

    /// Number of live records currently committed.
    pub fn len(&self) -> usize {
        self.slots.iter().flatten().count()
    }

    /// Whether no live records are committed.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Admit one record without a reservation.
    pub fn try_put(
        &mut self,
        meta: BundleMeta,
        bytes: &[u8],
    ) -> Result<StorePutOutcome, FixedStoreError> {
        self.validate_request(meta, bytes)?;
        if self.is_reserved(&meta.id) {
            return Err(FixedStoreError::Busy);
        }
        let existing = self.find_slot(&meta.id);
        let old_len = existing.map_or(0, |index| self.slots[index].expect("index is live").len);
        let projected = self.projected_bytes(old_len, bytes.len())?;
        if projected > TOTAL {
            return Err(FixedStoreError::Capacity);
        }
        let index = match existing {
            Some(index) => index,
            None => {
                if self.len().saturating_add(self.reserved_slots) >= SLOTS {
                    return Err(FixedStoreError::Capacity);
                }
                self.free_slot().ok_or(FixedStoreError::Capacity)?
            }
        };
        self.write_slot(index, meta, bytes);
        self.used_bytes = projected - self.reserved_bytes;
        Ok(if existing.is_some() {
            StorePutOutcome::Replaced
        } else {
            StorePutOutcome::Inserted
        })
    }

    /// Reserve capacity without publishing payload bytes.
    pub fn try_reserve(&mut self, meta: BundleMeta) -> Result<FixedReservation, FixedStoreError> {
        self.validate_meta(meta)?;
        if self.is_reserved(&meta.id) {
            return Err(FixedStoreError::Busy);
        }
        let existing = self.find_slot(&meta.id);
        let old_len = existing.map_or(0, |index| self.slots[index].expect("index is live").len);
        let requested = usize::try_from(meta.size).map_err(|_| FixedStoreError::InvalidRequest)?;
        if requested > B {
            return Err(FixedStoreError::Capacity);
        }
        let reserved_bytes = requested.saturating_sub(old_len) as u64;
        let projected = self
            .used_bytes
            .checked_add(self.reserved_bytes)
            .and_then(|used| used.checked_add(reserved_bytes))
            .ok_or(FixedStoreError::Capacity)?;
        if projected > TOTAL {
            return Err(FixedStoreError::Capacity);
        }
        let reserves_slot = existing.is_none();
        if reserves_slot && self.len().saturating_add(self.reserved_slots) >= SLOTS {
            return Err(FixedStoreError::Capacity);
        }
        let pending = self.free_reservation().ok_or(FixedStoreError::Capacity)?;
        let nonce = self
            .next_nonce
            .checked_add(1)
            .ok_or(FixedStoreError::Capacity)?;
        let token = FixedReservation {
            provider: self.provider,
            instance: self.instance,
            nonce,
            id: meta.id,
            declared_size: meta.size,
            old_len,
        };
        let next_reserved_bytes = self
            .reserved_bytes
            .checked_add(reserved_bytes)
            .ok_or(FixedStoreError::Capacity)?;
        let next_reserved_slots = self
            .reserved_slots
            .checked_add(usize::from(reserves_slot))
            .ok_or(FixedStoreError::Capacity)?;
        self.next_nonce = nonce;
        self.reservations[pending] = Some(PendingReservation {
            token,
            reserved_bytes,
            reserves_slot,
        });
        self.reserved_bytes = next_reserved_bytes;
        self.reserved_slots = next_reserved_slots;
        Ok(token)
    }

    /// Publish a record against a reservation. Failed validation keeps the
    /// reservation and all visible state unchanged.
    pub fn commit_reserved(
        &mut self,
        token: &FixedReservation,
        meta: BundleMeta,
        bytes: &[u8],
    ) -> Result<StorePutOutcome, FixedStoreError> {
        let pending_index = self
            .pending_index(token)
            .ok_or(FixedStoreError::InvalidToken)?;
        let pending = self.reservations[pending_index].expect("pending index is live");
        if meta.id != token.id
            || meta.size != token.declared_size
            || bytes.len() != token.declared_size as usize
        {
            return Err(FixedStoreError::InvalidRequest);
        }
        let existing = self.find_slot(&token.id);
        let old_len = existing.map_or(0, |index| self.slots[index].expect("index is live").len);
        if old_len != token.old_len {
            return Err(FixedStoreError::Conflict);
        }
        let remaining_reserved = self
            .reserved_bytes
            .checked_sub(pending.reserved_bytes)
            .ok_or(FixedStoreError::Conflict)?;
        let projected =
            self.projected_bytes_with_reserved(old_len, bytes.len(), remaining_reserved)?;
        if projected > TOTAL {
            return Err(FixedStoreError::Capacity);
        }
        let index = match existing {
            Some(index) => index,
            None => self.free_slot().ok_or(FixedStoreError::Capacity)?,
        };
        self.write_slot(index, meta, bytes);
        self.used_bytes = projected - remaining_reserved;
        self.reserved_bytes = self.reserved_bytes.saturating_sub(pending.reserved_bytes);
        if pending.reserves_slot {
            self.reserved_slots = self.reserved_slots.saturating_sub(1);
        }
        self.reservations[pending_index] = None;
        Ok(if existing.is_some() {
            StorePutOutcome::Replaced
        } else {
            StorePutOutcome::Inserted
        })
    }

    /// Abort a reservation. Repeating abort for an already-consumed token is
    /// harmless; a token from another provider is rejected.
    pub fn abort_reservation(&mut self, token: &FixedReservation) -> Result<(), FixedStoreError> {
        if token.provider != self.provider || token.instance != self.instance {
            return Err(FixedStoreError::InvalidToken);
        }
        let Some(index) = self.pending_index(token) else {
            return Ok(());
        };
        let pending = self.reservations[index]
            .take()
            .expect("pending index is live");
        self.reserved_bytes = self.reserved_bytes.saturating_sub(pending.reserved_bytes);
        if pending.reserves_slot {
            self.reserved_slots = self.reserved_slots.saturating_sub(1);
        }
        Ok(())
    }

    fn validate_request(&self, meta: BundleMeta, bytes: &[u8]) -> Result<(), FixedStoreError> {
        self.validate_meta(meta)?;
        if usize::try_from(meta.size).ok() != Some(bytes.len()) {
            return Err(FixedStoreError::InvalidRequest);
        }
        Ok(())
    }

    fn validate_meta(&self, meta: BundleMeta) -> Result<(), FixedStoreError> {
        if usize::try_from(meta.size).ok().is_none_or(|size| size > B) {
            return Err(FixedStoreError::InvalidRequest);
        }
        Ok(())
    }

    fn projected_bytes(&self, old_len: usize, new_len: usize) -> Result<u64, FixedStoreError> {
        self.projected_bytes_with_reserved(old_len, new_len, self.reserved_bytes)
    }

    fn projected_bytes_with_reserved(
        &self,
        old_len: usize,
        new_len: usize,
        reserved_bytes: u64,
    ) -> Result<u64, FixedStoreError> {
        self.used_bytes
            .checked_sub(old_len as u64)
            .and_then(|used| used.checked_add(new_len as u64))
            .and_then(|used| used.checked_add(reserved_bytes))
            .ok_or(FixedStoreError::Capacity)
    }

    fn find_slot(&self, id: &Id16) -> Option<usize> {
        self.slots
            .iter()
            .position(|slot| slot.is_some_and(|slot| slot.meta.id == *id))
    }

    fn free_slot(&self) -> Option<usize> {
        self.slots.iter().position(Option::is_none)
    }

    fn free_reservation(&self) -> Option<usize> {
        self.reservations.iter().position(Option::is_none)
    }

    fn is_reserved(&self, id: &Id16) -> bool {
        Self::reservations_contain(&self.reservations, id)
    }

    fn reservations_contain(reservations: &[Option<PendingReservation>], id: &Id16) -> bool {
        reservations
            .iter()
            .flatten()
            .any(|pending| pending.token.id == *id)
    }

    fn pending_index(&self, token: &FixedReservation) -> Option<usize> {
        self.reservations
            .iter()
            .position(|pending| pending.is_some_and(|pending| pending.token == *token))
    }

    fn write_slot(&mut self, index: usize, meta: BundleMeta, bytes: &[u8]) {
        let mut inline = [0; B];
        inline[..bytes.len()].copy_from_slice(bytes);
        self.slots[index] = Some(FixedSlot {
            meta,
            bytes: inline,
            len: bytes.len(),
        });
    }
}

impl<const SLOTS: usize, const B: usize, const TOTAL: u64> EmbeddedBundleStore
    for FixedBundleStore<SLOTS, B, TOTAL>
{
    fn read_meta(&self, id: &Id16, out: &mut BundleMeta) -> Result<(), StoreReadError> {
        let Some(index) = self.find_slot(id) else {
            return Err(StoreReadError::NotFound);
        };
        *out = self.slots[index].expect("index is live").meta;
        Ok(())
    }

    fn read_bundle(&self, id: &Id16, out: &mut [u8]) -> Result<usize, StoreReadError> {
        let Some(index) = self.find_slot(id) else {
            return Err(StoreReadError::NotFound);
        };
        let slot = self.slots[index].expect("index is live");
        if slot.len > out.len() {
            return Err(StoreReadError::OutputTooSmall { required: slot.len });
        }
        out[..slot.len].copy_from_slice(&slot.bytes[..slot.len]);
        Ok(slot.len)
    }

    fn visit_ids(&self, lo: &Id16, hi: &Id16, visitor: &mut dyn FnMut(Id16)) {
        let mut visited = [false; SLOTS];
        for _ in 0..SLOTS {
            let next = self
                .slots
                .iter()
                .enumerate()
                .filter(|(index, slot)| {
                    !visited[*index]
                        && slot.is_some_and(|slot| slot.meta.id >= *lo && slot.meta.id <= *hi)
                })
                .min_by_key(|(_, slot)| slot.expect("filtered entry").meta.id);
            let Some((index, slot)) = next else { break };
            visited[index] = true;
            visitor(slot.expect("filtered entry").meta.id);
        }
    }

    fn visit_meta(&self, visitor: &mut dyn FnMut(BundleMeta)) {
        for slot in self.slots.iter().flatten() {
            visitor(slot.meta);
        }
    }

    fn expire_into(&mut self, now_ms: u64, out: &mut [Id16]) -> Result<usize, StoreReadError> {
        let required = self
            .slots
            .iter()
            .flatten()
            .filter(|slot| slot.meta.expires_at <= now_ms && !self.is_reserved(&slot.meta.id))
            .count();
        if required > out.len() {
            return Err(StoreReadError::OutputTooSmall { required });
        }
        let mut count = 0;
        let reservations = &self.reservations;
        for slot in &mut self.slots {
            if slot.is_some_and(|slot| {
                slot.meta.expires_at <= now_ms
                    && !Self::reservations_contain(reservations, &slot.meta.id)
            }) {
                let expired = slot.take().expect("expired slot");
                out[count] = expired.meta.id;
                count += 1;
                self.used_bytes = self.used_bytes.saturating_sub(expired.len as u64);
            }
        }
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::{FixedBundleStore, FixedStoreError};
    use crate::embedded_store::EmbeddedBundleStore;
    use crate::store::{BundleMeta, SlotState, StorePutOutcome, StoreReadError};
    use mmp_types::ids::Id16;

    type Store = FixedBundleStore<2, 8, 12>;

    fn meta(id: u8, size: usize, expires_at: u64) -> BundleMeta {
        BundleMeta {
            id: Id16([id; 16]),
            class: 1,
            origin: Id16([2; 16]),
            dest: Id16([3; 16]),
            priority: 1,
            effective_priority: 1,
            size: size as u32,
            stored_at: 0,
            expires_at,
            budget: 4,
            own_origin: false,
            slot: SlotState::Stored,
        }
    }

    #[test]
    fn exact_fill_and_replacement_charge_only_positive_delta() {
        let mut store = Store::new(1).unwrap();
        assert_eq!(
            store.try_put(meta(1, 8, 100), &[0; 8]),
            Ok(StorePutOutcome::Inserted)
        );
        assert_eq!(store.used_bytes(), 8);
        assert_eq!(
            store.try_put(meta(1, 12, 100), &[1; 12]),
            Err(FixedStoreError::InvalidRequest)
        );
        assert_eq!(
            store.try_put(meta(1, 4, 100), &[1; 4]),
            Ok(StorePutOutcome::Replaced)
        );
        assert_eq!(store.used_bytes(), 4);
        assert_eq!(
            store.try_put(meta(2, 8, 100), &[2; 8]),
            Ok(StorePutOutcome::Inserted)
        );
        assert_eq!(store.used_bytes(), 12);
        assert_eq!(
            store.try_put(meta(3, 1, 100), &[3]),
            Err(FixedStoreError::Capacity)
        );
    }

    #[test]
    fn reservation_consumes_capacity_and_abort_is_idempotent() {
        let mut store = FixedBundleStore::<1, 8, 12>::new(7).unwrap();
        let token = store.try_reserve(meta(1, 8, 100)).unwrap();
        assert_eq!(
            store.try_reserve(meta(2, 8, 100)),
            Err(FixedStoreError::Capacity)
        );
        assert_eq!(
            store.try_put(meta(2, 4, 100), &[5; 4]),
            Err(FixedStoreError::Capacity)
        );
        assert_eq!(
            store.commit_reserved(&token, meta(1, 8, 100), &[4; 8]),
            Ok(StorePutOutcome::Inserted)
        );
        assert_eq!(store.abort_reservation(&token), Ok(()));
        assert_eq!(store.used_bytes(), 8);

        let mut second = Store::new(8).unwrap();
        let second_token = second.try_reserve(meta(2, 4, 100)).unwrap();
        assert_eq!(second.abort_reservation(&second_token), Ok(()));
        assert_eq!(second.abort_reservation(&second_token), Ok(()));
        assert_eq!(
            second.try_put(meta(2, 4, 100), &[5; 4]),
            Ok(StorePutOutcome::Inserted)
        );
    }

    #[test]
    fn wrong_and_stale_tokens_fail_closed_without_mutation() {
        let mut first = Store::new(1).unwrap();
        let token = first.try_reserve(meta(1, 4, 100)).unwrap();
        let mut wrong = token;
        wrong.id = Id16([9; 16]);
        assert_eq!(
            first.commit_reserved(&wrong, meta(9, 4, 100), &[1; 4]),
            Err(FixedStoreError::InvalidToken)
        );
        assert_eq!(first.abort_reservation(&wrong), Ok(()));
        assert_eq!(
            first.commit_reserved(&token, meta(1, 4, 100), &[1; 4]),
            Ok(StorePutOutcome::Inserted)
        );
        assert_eq!(
            first.commit_reserved(&token, meta(1, 4, 100), &[1; 4]),
            Err(FixedStoreError::InvalidToken)
        );

        let mut other = Store::new(2).unwrap();
        assert_eq!(
            other.abort_reservation(&token),
            Err(FixedStoreError::InvalidToken)
        );
    }

    #[test]
    fn same_provider_namespace_cannot_cross_instance() {
        let mut first = Store::new(7).unwrap();
        let token = first.try_reserve(meta(1, 4, 100)).unwrap();

        let mut second = Store::new(7).unwrap();
        let second_token = second.try_reserve(meta(1, 4, 100)).unwrap();
        assert_ne!(token, second_token);
        assert_eq!(
            second.commit_reserved(&token, meta(1, 4, 100), &[1; 4]),
            Err(FixedStoreError::InvalidToken)
        );
        assert_eq!(
            second.abort_reservation(&token),
            Err(FixedStoreError::InvalidToken)
        );
        assert_eq!(
            second.commit_reserved(&second_token, meta(1, 4, 100), &[1; 4]),
            Ok(StorePutOutcome::Inserted)
        );
    }

    #[test]
    fn short_reads_expiry_preflight_and_ordered_visitation_are_bounded() {
        let mut store = Store::new(1).unwrap();
        store.try_put(meta(9, 3, 10), b"abc").unwrap();
        let mut bytes = [0xA5; 2];
        assert_eq!(
            store.read_bundle(&Id16([9; 16]), &mut bytes),
            Err(StoreReadError::OutputTooSmall { required: 3 })
        );
        assert_eq!(bytes, [0xA5; 2]);

        let mut expired = [Id16([0xEE; 16]); 0];
        assert_eq!(
            store.expire_into(10, &mut expired),
            Err(StoreReadError::OutputTooSmall { required: 1 })
        );
        let mut meta_out = meta(7, 0, 0);
        assert_eq!(store.read_meta(&Id16([9; 16]), &mut meta_out), Ok(()));

        let token = store.try_reserve(meta(9, 3, 10)).unwrap();
        let mut no_expiry = [Id16([0; 16]); 1];
        assert_eq!(store.expire_into(10, &mut no_expiry), Ok(0));
        assert_eq!(
            store.commit_reserved(&token, meta(9, 3, 10), b"def"),
            Ok(StorePutOutcome::Replaced)
        );

        let mut ids = [0; 2];
        let mut count = 0;
        store.visit_ids(&Id16::MIN, &Id16::MAX, &mut |id| {
            ids[count] = id.0[0];
            count += 1;
        });
        assert_eq!(&ids[..count], &[9]);
    }

    #[test]
    fn invalid_configuration_is_rejected() {
        assert!(FixedBundleStore::<0, 8, 12>::new(1).is_err());
        assert!(FixedBundleStore::<1, 0, 12>::new(1).is_err());
        assert!(FixedBundleStore::<1, 8, 0>::new(1).is_err());
    }
}
