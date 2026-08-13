//! Fixed-capacity reference provider for the embedded seam.
//!
//! Store admission and reads are allocation-free. Snapshot validation reuses
//! the bounded canonical bundle decoder, so callers should provide an allocator
//! when taking or restoring snapshots. This is a host-testable capacity model,
//! not a flash driver: it makes no durability or power-loss claim and does not
//! implement the legacy allocating [`crate::BundleStore`] compatibility trait.

use mmp_types::bundle::Bundle;
use mmp_types::consts::{L_MAX, MAX_BUNDLE};
use mmp_types::domain::SigDomain;
use mmp_types::ids::Id16;

use crate::embedded_store::EmbeddedBundleStore;
use crate::store::{BundleMeta, StorePutOutcome, StoreReadError};

use core::sync::atomic::{AtomicUsize, Ordering};

static NEXT_INSTANCE_ID: AtomicUsize = AtomicUsize::new(1);

const SNAPSHOT_MAGIC: [u8; 4] = *b"FBS1";
const SNAPSHOT_VERSION: u8 = 1;
const SNAPSHOT_HEADER_LEN: usize = 26;
const SNAPSHOT_META_LEN: usize = 74;

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

/// Errors returned by the bounded fixed-store snapshot codec.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixedSnapshotError {
    /// The caller-provided output buffer is too small.
    OutputTooSmall {
        /// The exact buffer length required by the snapshot.
        required: usize,
    },
    /// A checked size calculation overflowed.
    ArithmeticOverflow,
    /// The snapshot magic is not supported.
    InvalidMagic,
    /// The image version is newer or otherwise unsupported.
    UnsupportedVersion,
    /// Reserved header bits are not zero.
    NonZeroReserved,
    /// The image ended before a required field or record.
    Truncated,
    /// The image contains bytes after the declared records.
    Trailing,
    /// The image does not match this compile-time store configuration.
    CapacityMismatch,
    /// The record count exceeds the fixed slot bound.
    CountOutOfRange,
    /// Occupied slot indices are not strictly increasing or are out of range.
    SlotOrder,
    /// More than one record claims the same BundleID.
    DuplicateId,
    /// Local metadata is outside the bounded provider policy.
    InvalidMetadata,
    /// The embedded bundle is not an exact canonical MMP wire encoding.
    InvalidBundle,
    /// The bundle signature does not verify under its origin key.
    BadSignature,
    /// The metadata BundleID differs from the canonical bundle-derived ID.
    BundleIdMismatch,
    /// The metadata origin differs from the bundle-derived origin NodeID.
    OriginMismatch,
    /// Signed bundle routing metadata or encoded length differs from metadata.
    MetadataMismatch,
    /// Recomputed committed-byte accounting overflowed.
    AccountingOverflow,
    /// Recomputed committed bytes exceed the configured aggregate budget.
    Capacity,
    /// The compile-time provider configuration cannot be constructed.
    InvalidConfiguration,
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
        if SLOTS == 0
            || B == 0
            || TOTAL == 0
            || B > MAX_BUNDLE
            || SLOTS > u32::MAX as usize
            || B > u32::MAX as usize
        {
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

    /// Return the exact byte length of the deterministic caller-owned snapshot.
    ///
    /// The preflight validates every live record, including its canonical wire
    /// encoding and signature, before an encoder is allowed to mutate output.
    pub fn snapshot_encoded_len(&self) -> Result<usize, FixedSnapshotError> {
        let _ = Self::snapshot_dimensions()?;
        let mut count = 0usize;
        let mut required = SNAPSHOT_HEADER_LEN;
        for (index, slot) in self.slots.iter().enumerate() {
            let Some(slot) = slot else { continue };
            self.validate_snapshot_slot(index, slot)?;
            count = count
                .checked_add(1)
                .ok_or(FixedSnapshotError::ArithmeticOverflow)?;
            required = required
                .checked_add(4)
                .and_then(|value| value.checked_add(SNAPSHOT_META_LEN))
                .and_then(|value| value.checked_add(slot.len))
                .ok_or(FixedSnapshotError::ArithmeticOverflow)?;
        }
        let _ = u32::try_from(count).map_err(|_| FixedSnapshotError::ArithmeticOverflow)?;
        Ok(required)
    }

    /// Encode a complete provider snapshot into a caller-owned buffer.
    pub fn encode_snapshot(&self, out: &mut [u8]) -> Result<usize, FixedSnapshotError> {
        let required = self.snapshot_encoded_len()?;
        if out.len() < required {
            return Err(FixedSnapshotError::OutputTooSmall { required });
        }
        let count = self.len();
        let (slots, slot_bytes) = Self::snapshot_dimensions()?;
        let mut writer = SnapshotWriter::new(out);
        writer.bytes(&SNAPSHOT_MAGIC)?;
        writer.u8(SNAPSHOT_VERSION)?;
        writer.u8(0)?;
        writer.u32(slots)?;
        writer.u32(slot_bytes)?;
        writer.u64(TOTAL)?;
        writer.u32(count as u32)?;
        for (index, slot) in self.slots.iter().enumerate() {
            let Some(slot) = slot else { continue };
            writer.u32(index as u32)?;
            writer.bytes(&slot.meta.id.0)?;
            writer.u8(slot.meta.class)?;
            writer.bytes(&slot.meta.origin.0)?;
            writer.bytes(&slot.meta.dest.0)?;
            writer.u8(slot.meta.priority)?;
            writer.u8(slot.meta.effective_priority)?;
            writer.u32(slot.meta.size)?;
            writer.u64(slot.meta.stored_at)?;
            writer.u64(slot.meta.expires_at)?;
            writer.u8(slot.meta.budget)?;
            writer.u8(u8::from(slot.meta.own_origin))?;
            writer.u8(match slot.meta.slot {
                crate::store::SlotState::Stored => 0,
                crate::store::SlotState::HandoffPending => 1,
            })?;
            writer.bytes(&slot.bytes[..slot.len])?;
        }
        Ok(required)
    }

    /// Restore a complete snapshot into a fresh provider instance.
    ///
    /// The provider namespace is supplied by the caller and runtime-only
    /// reservations/nonces are intentionally not serialized. Any validation
    /// failure drops the staged store without mutating an externally visible
    /// provider.
    pub fn restore_snapshot(provider: u64, input: &[u8]) -> Result<Self, FixedSnapshotError> {
        let (slots, slot_bytes) = Self::snapshot_dimensions()?;
        let mut reader = SnapshotReader::new(input);
        if reader.bytes(4)? != SNAPSHOT_MAGIC {
            return Err(FixedSnapshotError::InvalidMagic);
        }
        if reader.u8()? != SNAPSHOT_VERSION {
            return Err(FixedSnapshotError::UnsupportedVersion);
        }
        if reader.u8()? != 0 {
            return Err(FixedSnapshotError::NonZeroReserved);
        }
        if reader.u32()? != slots || reader.u32()? != slot_bytes || reader.u64()? != TOTAL {
            return Err(FixedSnapshotError::CapacityMismatch);
        }
        let count = reader.u32()? as usize;
        if count > SLOTS {
            return Err(FixedSnapshotError::CountOutOfRange);
        }
        let mut staged =
            Self::new(provider).map_err(|_| FixedSnapshotError::InvalidConfiguration)?;
        let mut previous = None;
        for _ in 0..count {
            let index = reader.u32()? as usize;
            if index >= SLOTS || previous.is_some_and(|last| index <= last) {
                return Err(FixedSnapshotError::SlotOrder);
            }
            previous = Some(index);
            let meta = SnapshotReader::read_meta(&mut reader)?;
            let size =
                usize::try_from(meta.size).map_err(|_| FixedSnapshotError::InvalidMetadata)?;
            if size > B || size > MAX_BUNDLE {
                return Err(FixedSnapshotError::InvalidMetadata);
            }
            let bytes = reader.bytes(size)?;
            staged.validate_snapshot_meta(meta)?;
            staged.validate_bundle(meta, bytes)?;
            if staged.find_slot(&meta.id).is_some() {
                return Err(FixedSnapshotError::DuplicateId);
            }
            let next_used = staged
                .used_bytes
                .checked_add(size as u64)
                .ok_or(FixedSnapshotError::AccountingOverflow)?;
            if next_used > TOTAL {
                return Err(FixedSnapshotError::Capacity);
            }
            staged.write_slot(index, meta, bytes);
            staged.used_bytes = next_used;
        }
        if reader.remaining() != 0 {
            return Err(FixedSnapshotError::Trailing);
        }
        Ok(staged)
    }

    fn validate_snapshot_slot(
        &self,
        _index: usize,
        slot: &FixedSlot<B>,
    ) -> Result<(), FixedSnapshotError> {
        self.validate_snapshot_meta(slot.meta)?;
        self.validate_bundle(slot.meta, &slot.bytes[..slot.len])
    }

    fn snapshot_dimensions() -> Result<(u32, u32), FixedSnapshotError> {
        Ok((
            u32::try_from(SLOTS).map_err(|_| FixedSnapshotError::CapacityMismatch)?,
            u32::try_from(B).map_err(|_| FixedSnapshotError::CapacityMismatch)?,
        ))
    }

    fn validate_snapshot_meta(&self, meta: BundleMeta) -> Result<(), FixedSnapshotError> {
        let size = usize::try_from(meta.size).map_err(|_| FixedSnapshotError::InvalidMetadata)?;
        if size > B
            || size > MAX_BUNDLE
            || meta.priority > 3
            || meta.effective_priority > 3
            || meta.budget > L_MAX
            || meta.expires_at < meta.stored_at
        {
            return Err(FixedSnapshotError::InvalidMetadata);
        }
        Ok(())
    }

    fn validate_bundle(&self, meta: BundleMeta, bytes: &[u8]) -> Result<(), FixedSnapshotError> {
        let bundle =
            Bundle::decode_canonical(bytes).map_err(|_| FixedSnapshotError::InvalidBundle)?;
        if bundle.encode() != bytes {
            return Err(FixedSnapshotError::InvalidBundle);
        }
        if bundle.id() != meta.id {
            return Err(FixedSnapshotError::BundleIdMismatch);
        }
        if bundle.origin_node() != meta.origin {
            return Err(FixedSnapshotError::OriginMismatch);
        }
        if bundle.class != meta.class
            || bundle.dest != meta.dest
            || bundle.priority != meta.priority
            || bytes.len() != meta.size as usize
        {
            return Err(FixedSnapshotError::MetadataMismatch);
        }
        mmp_crypto::verify(
            SigDomain::Bundle,
            &bundle.origin,
            &bundle.canonical_zeroed(),
            &bundle.signature,
        )
        .map_err(|_| FixedSnapshotError::BadSignature)
    }
}

struct SnapshotWriter<'a> {
    out: &'a mut [u8],
    pos: usize,
}

impl<'a> SnapshotWriter<'a> {
    fn new(out: &'a mut [u8]) -> Self {
        Self { out, pos: 0 }
    }

    fn bytes(&mut self, bytes: &[u8]) -> Result<(), FixedSnapshotError> {
        let end = self
            .pos
            .checked_add(bytes.len())
            .ok_or(FixedSnapshotError::ArithmeticOverflow)?;
        if end > self.out.len() {
            return Err(FixedSnapshotError::OutputTooSmall { required: end });
        }
        self.out[self.pos..end].copy_from_slice(bytes);
        self.pos = end;
        Ok(())
    }

    fn u8(&mut self, value: u8) -> Result<(), FixedSnapshotError> {
        self.bytes(&[value])
    }
    fn u32(&mut self, value: u32) -> Result<(), FixedSnapshotError> {
        self.bytes(&value.to_le_bytes())
    }
    fn u64(&mut self, value: u64) -> Result<(), FixedSnapshotError> {
        self.bytes(&value.to_le_bytes())
    }
}

struct SnapshotReader<'a> {
    input: &'a [u8],
    pos: usize,
}

impl<'a> SnapshotReader<'a> {
    fn new(input: &'a [u8]) -> Self {
        Self { input, pos: 0 }
    }
    fn remaining(&self) -> usize {
        self.input.len().saturating_sub(self.pos)
    }

    fn bytes(&mut self, len: usize) -> Result<&'a [u8], FixedSnapshotError> {
        let end = self
            .pos
            .checked_add(len)
            .ok_or(FixedSnapshotError::Truncated)?;
        if end > self.input.len() {
            return Err(FixedSnapshotError::Truncated);
        }
        let bytes = &self.input[self.pos..end];
        self.pos = end;
        Ok(bytes)
    }

    fn u8(&mut self) -> Result<u8, FixedSnapshotError> {
        Ok(self.bytes(1)?[0])
    }
    fn u32(&mut self) -> Result<u32, FixedSnapshotError> {
        let bytes = self.bytes(4)?;
        Ok(u32::from_le_bytes(
            bytes.try_into().expect("fixed reader width"),
        ))
    }
    fn u64(&mut self) -> Result<u64, FixedSnapshotError> {
        let bytes = self.bytes(8)?;
        Ok(u64::from_le_bytes(
            bytes.try_into().expect("fixed reader width"),
        ))
    }

    fn read_meta(&mut self) -> Result<BundleMeta, FixedSnapshotError> {
        let id = Id16(self.bytes(16)?.try_into().expect("fixed reader width"));
        let class = self.u8()?;
        let origin = Id16(self.bytes(16)?.try_into().expect("fixed reader width"));
        let dest = Id16(self.bytes(16)?.try_into().expect("fixed reader width"));
        let priority = self.u8()?;
        let effective_priority = self.u8()?;
        let size = self.u32()?;
        let stored_at = self.u64()?;
        let expires_at = self.u64()?;
        let budget = self.u8()?;
        let own_origin = match self.u8()? {
            0 => false,
            1 => true,
            _ => return Err(FixedSnapshotError::InvalidMetadata),
        };
        let slot = match self.u8()? {
            0 => crate::store::SlotState::Stored,
            1 => crate::store::SlotState::HandoffPending,
            _ => return Err(FixedSnapshotError::InvalidMetadata),
        };
        Ok(BundleMeta {
            id,
            class,
            origin,
            dest,
            priority,
            effective_priority,
            size,
            stored_at,
            expires_at,
            budget,
            own_origin,
            slot,
        })
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
    use super::{FixedBundleStore, FixedSnapshotError, FixedStoreError};
    use crate::embedded_store::EmbeddedBundleStore;
    use crate::store::{BundleMeta, SlotState, StorePutOutcome, StoreReadError};
    use mmp_crypto::Identity;
    use mmp_types::bundle::{class, Bundle};
    use mmp_types::domain::SigDomain;
    use mmp_types::ids::Id16;
    use mmp_types::ids::Sig64;

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

    fn signed_bundle(seed: u8, payload: &[u8]) -> Bundle {
        let identity = Identity::from_seed(&[seed; 32]);
        let mut bundle = Bundle {
            class: class::DIRECT,
            flags: 0,
            origin: identity.public(),
            dest: Id16([3; 16]),
            lifetime_ms: 60_000,
            priority: 1,
            exts: alloc::vec::Vec::new(),
            payload: payload.to_vec(),
            signature: Sig64::ZERO,
        };
        bundle.signature = identity.sign(SigDomain::Bundle, &bundle.canonical_zeroed());
        bundle
    }

    fn bundle_meta(bundle: &Bundle, now: u64) -> BundleMeta {
        BundleMeta {
            id: bundle.id(),
            class: bundle.class,
            origin: bundle.origin_node(),
            dest: bundle.dest,
            priority: bundle.priority,
            effective_priority: bundle.priority,
            size: bundle.encode().len() as u32,
            stored_at: now,
            expires_at: now + bundle.lifetime_ms,
            budget: 4,
            own_origin: true,
            slot: SlotState::Stored,
        }
    }

    #[test]
    fn snapshot_round_trip_preserves_live_records_and_drops_reservations() {
        type SnapshotStore = FixedBundleStore<3, 512, 1_024>;
        let bundle = signed_bundle(17, b"snapshot payload");
        let wire = bundle.encode();
        let bundle_metadata = bundle_meta(&bundle, 123);
        let mut store = SnapshotStore::new(9).unwrap();
        store.try_put(bundle_metadata, &wire).unwrap();
        let reservation = store.try_reserve(meta(2, 4, 900)).unwrap();

        let required = store.snapshot_encoded_len().unwrap();
        let mut image = alloc::vec![0u8; required];
        assert_eq!(store.encode_snapshot(&mut image).unwrap(), required);

        let restored = SnapshotStore::restore_snapshot(11, &image).unwrap();
        assert_eq!(restored.len(), 1);
        assert_eq!(restored.used_bytes(), wire.len() as u64);
        let mut read = [0u8; 512];
        assert_eq!(
            restored.read_bundle(&bundle.id(), &mut read).unwrap(),
            wire.len()
        );
        assert_eq!(&read[..wire.len()], wire.as_slice());
        assert_eq!(
            restored.read_meta(
                &bundle.id(),
                &mut BundleMeta {
                    id: Id16::MIN,
                    class: 0,
                    origin: Id16::MIN,
                    dest: Id16::MIN,
                    priority: 0,
                    effective_priority: 0,
                    size: 0,
                    stored_at: 0,
                    expires_at: 0,
                    budget: 0,
                    own_origin: false,
                    slot: SlotState::Stored,
                }
            ),
            Ok(())
        );
        assert_eq!(store.abort_reservation(&reservation), Ok(()));
    }

    #[test]
    fn snapshot_encode_preflights_output_without_mutation() {
        type SnapshotStore = FixedBundleStore<1, 512, 1_024>;
        let bundle = signed_bundle(18, b"preflight");
        let wire = bundle.encode();
        let mut store = SnapshotStore::new(1).unwrap();
        store.try_put(bundle_meta(&bundle, 1), &wire).unwrap();
        let required = store.snapshot_encoded_len().unwrap();
        let mut too_small = alloc::vec![0xA5; required - 1];
        assert_eq!(
            store.encode_snapshot(&mut too_small),
            Err(FixedSnapshotError::OutputTooSmall { required })
        );
        assert!(too_small.iter().all(|byte| *byte == 0xA5));
    }

    #[test]
    fn snapshot_restore_rejects_trailing_bytes_and_forged_wire() {
        type SnapshotStore = FixedBundleStore<1, 512, 1_024>;
        let bundle = signed_bundle(19, b"reject");
        let wire = bundle.encode();
        let mut store = SnapshotStore::new(1).unwrap();
        store.try_put(bundle_meta(&bundle, 1), &wire).unwrap();
        let mut image = alloc::vec![0u8; store.snapshot_encoded_len().unwrap()];
        store.encode_snapshot(&mut image).unwrap();

        let mut trailing = image.clone();
        trailing.push(0);
        assert!(matches!(
            SnapshotStore::restore_snapshot(1, &trailing),
            Err(FixedSnapshotError::Trailing)
        ));

        let last = image.len() - 1;
        image[last] ^= 1;
        assert!(matches!(
            SnapshotStore::restore_snapshot(1, &image),
            Err(FixedSnapshotError::BadSignature)
        ));
    }
}
