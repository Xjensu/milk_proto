//! Journal-backed fixed-capacity provider reference implementation.
//!
//! This crate composes the existing fixed-store snapshot codec and
//! provider-neutral journal primitives. It uses two erase-aligned journal
//! slots so a failed append leaves the previously selected generation intact.
//! It is a host-testable provider boundary, not a claim that any particular
//! flash controller is power-loss atomic.

#![no_std]
#![warn(missing_docs)]

extern crate alloc;

use mmp_core::{FixedBundleStore, FixedSnapshotError, FixedStoreError};
use mmp_flash_journal::{
    encoded_len, scan_first, scan_first_validated, write_record, GenerationFloor, JournalBackend,
    JournalTransaction, JournalTransactionError, JournalTransactionState, JournalWriteError,
    RecoveryClassification, ScanResult,
};

/// Errors while validating a two-slot journal layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutError {
    /// A required size or payload bound is zero.
    ZeroValue,
    /// The region or slot is not aligned to the backend's program unit.
    WriteUnaligned,
    /// The region or slot is not aligned to the backend's erase unit.
    EraseUnaligned,
    /// The region arithmetic overflowed or exceeds backend capacity.
    OutOfBounds,
    /// The maximum payload exceeds the journal parser bound.
    PayloadTooLarge,
    /// One slot cannot contain the complete aligned record.
    RecordTooLarge,
}

/// Two-slot layout for one complete snapshot per journal record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JournalLayout {
    region_offset: usize,
    slot_bytes: usize,
    max_payload_bytes: usize,
}

impl JournalLayout {
    /// Validate a two-slot region against a backend and maximum snapshot size.
    pub fn new<B: JournalBackend>(
        backend: &B,
        region_offset: usize,
        slot_bytes: usize,
        max_payload_bytes: usize,
    ) -> Result<Self, LayoutError> {
        let geometry = backend.geometry();
        if slot_bytes == 0 || max_payload_bytes == 0 {
            return Err(LayoutError::ZeroValue);
        }
        if geometry.capacity == 0 || geometry.write_alignment == 0 || geometry.erase_alignment == 0
        {
            return Err(LayoutError::ZeroValue);
        }
        if !region_offset.is_multiple_of(geometry.write_alignment)
            || !slot_bytes.is_multiple_of(geometry.write_alignment)
        {
            return Err(LayoutError::WriteUnaligned);
        }
        if !region_offset.is_multiple_of(geometry.erase_alignment)
            || !slot_bytes.is_multiple_of(geometry.erase_alignment)
        {
            return Err(LayoutError::EraseUnaligned);
        }
        let required_record = encoded_len(max_payload_bytes).ok_or(LayoutError::PayloadTooLarge)?;
        let required_record =
            align_up(required_record, geometry.write_alignment).ok_or(LayoutError::OutOfBounds)?;
        if required_record > slot_bytes {
            return Err(LayoutError::RecordTooLarge);
        }
        let region_bytes = slot_bytes.checked_mul(2).ok_or(LayoutError::OutOfBounds)?;
        let end = region_offset
            .checked_add(region_bytes)
            .ok_or(LayoutError::OutOfBounds)?;
        if end > geometry.capacity {
            return Err(LayoutError::OutOfBounds);
        }
        Ok(Self {
            region_offset,
            slot_bytes,
            max_payload_bytes,
        })
    }

    /// Return the region-relative offset of slot zero or one.
    pub const fn slot_offset(self, slot: usize) -> Option<usize> {
        if slot > 1 {
            return None;
        }
        match self.slot_bytes.checked_mul(slot) {
            Some(delta) => self.region_offset.checked_add(delta),
            None => None,
        }
    }

    /// Return the bytes required for the caller-owned recovery image.
    pub const fn image_bytes(self) -> Option<usize> {
        self.slot_bytes.checked_mul(2)
    }

    /// Return one slot's capacity in bytes.
    pub const fn slot_bytes(self) -> usize {
        self.slot_bytes
    }

    /// Return the configured maximum snapshot payload size.
    pub const fn max_payload_bytes(self) -> usize {
        self.max_payload_bytes
    }

    fn validate_backend<B: JournalBackend>(self, backend: &B) -> Result<(), LayoutError> {
        let checked = Self::new(
            backend,
            self.region_offset,
            self.slot_bytes,
            self.max_payload_bytes,
        )?;
        if checked != self {
            return Err(LayoutError::OutOfBounds);
        }
        Ok(())
    }

    fn padded_record_len<B: JournalBackend>(
        self,
        backend: &B,
        payload_len: usize,
    ) -> Result<usize, LayoutError> {
        if payload_len > self.max_payload_bytes {
            return Err(LayoutError::RecordTooLarge);
        }
        let encoded = encoded_len(payload_len).ok_or(LayoutError::PayloadTooLarge)?;
        align_up(encoded, backend.geometry().write_alignment).ok_or(LayoutError::OutOfBounds)
    }
}

/// Errors returned by the journal-backed provider.
#[derive(Debug, PartialEq, Eq)]
pub enum ProviderError<E> {
    /// The layout or backend geometry is not usable.
    Layout(LayoutError),
    /// The caller's recovery image is too small.
    ImageBufferTooSmall {
        /// Minimum image length in bytes.
        required: usize,
    },
    /// The caller's snapshot payload buffer is too small.
    PayloadBufferTooSmall {
        /// Minimum snapshot payload buffer length in bytes.
        required: usize,
    },
    /// The caller's journal-record scratch buffer is too small.
    RecordBufferTooSmall {
        /// Minimum aligned journal scratch length in bytes.
        required: usize,
    },
    /// Fixed-store construction failed.
    Store(FixedStoreError),
    /// Snapshot decoding or bundle authenticity validation failed.
    Snapshot(FixedSnapshotError),
    /// A backend read failed while recovering a slot.
    Read {
        /// Slot whose image could not be read.
        slot: usize,
        /// Backend-specific read error.
        source: E,
    },
    /// Erasing a target slot produced an indeterminate provider state.
    Erase {
        /// Generation being attempted.
        generation: u64,
        /// Inactive slot selected for the attempt.
        slot: usize,
        /// Backend-specific erase error.
        source: E,
    },
    /// Writing a target record produced an indeterminate provider state.
    Write {
        /// Generation being attempted.
        generation: u64,
        /// Inactive slot selected for the attempt.
        slot: usize,
        /// Backend or framing error from the write operation.
        source: JournalWriteError<E>,
    },
    /// A post-write read failed while the provider was quarantined.
    RecoveryRead {
        /// Generation whose recovery read failed.
        generation: u64,
        /// Slot whose image could not be read.
        slot: usize,
        /// Backend-specific read error.
        source: E,
    },
    /// The two slots contain the same generation and selection is ambiguous.
    AmbiguousGeneration {
        /// Generation present in both slots.
        generation: u64,
    },
    /// A transaction state transition was rejected.
    Transaction(JournalTransactionError),
}

/// Result of a committed, rejected, or unresolved provider transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitOutcome {
    /// The attempted generation was recovered and published.
    Committed {
        /// Generation proven visible and validated.
        generation: u64,
    },
    /// Recovery proved that the attempted generation was not visible.
    NotCommitted {
        /// Generation that was attempted but not selected by recovery.
        generation: u64,
    },
    /// Recovery cannot distinguish the attempted generation from a torn write.
    Indeterminate {
        /// Generation that remains unresolved and quarantined.
        generation: u64,
    },
}

/// Host-testable two-slot journal provider for a fixed-capacity store.
pub struct JournaledFixedStore<
    B,
    const SLOTS: usize,
    const BUNDLE_BYTES: usize,
    const TOTAL_BYTES: u64,
> {
    backend: B,
    layout: JournalLayout,
    provider: u64,
    store: FixedBundleStore<SLOTS, BUNDLE_BYTES, TOTAL_BYTES>,
    transaction: JournalTransaction,
    active_slot: usize,
}

impl<B, const SLOTS: usize, const BUNDLE_BYTES: usize, const TOTAL_BYTES: u64>
    JournaledFixedStore<B, SLOTS, BUNDLE_BYTES, TOTAL_BYTES>
where
    B: JournalBackend,
{
    /// Recover the newest complete and validated snapshot from both slots.
    pub fn recover(
        backend: B,
        layout: JournalLayout,
        provider: u64,
        image: &mut [u8],
    ) -> Result<Self, ProviderError<B::Error>> {
        layout
            .validate_backend(&backend)
            .map_err(ProviderError::Layout)?;
        let required = layout
            .image_bytes()
            .ok_or(ProviderError::Layout(LayoutError::OutOfBounds))?;
        if image.len() < required {
            return Err(ProviderError::ImageBufferTooSmall { required });
        }
        let recovered = read_and_restore(&backend, layout, provider, &mut image[..required])?;
        let generation = recovered.generation.unwrap_or(0);
        let active_slot = recovered.slot.unwrap_or(0);
        let store = recovered.store;
        Ok(Self {
            backend,
            layout,
            provider,
            store,
            transaction: JournalTransaction::new(GenerationFloor::new(generation)),
            active_slot,
        })
    }

    /// Borrow the recovered fixed-capacity store.
    pub fn store(&self) -> &FixedBundleStore<SLOTS, BUNDLE_BYTES, TOTAL_BYTES> {
        &self.store
    }

    /// Borrow the mutable store that will be included in the next snapshot.
    pub fn store_mut(&mut self) -> &mut FixedBundleStore<SLOTS, BUNDLE_BYTES, TOTAL_BYTES> {
        &mut self.store
    }

    /// Return the configured journal layout.
    pub const fn layout(&self) -> JournalLayout {
        self.layout
    }

    /// Return the highest generation proven by recovery.
    pub const fn generation(&self) -> u64 {
        self.transaction.known_floor()
    }

    /// Return the currently selected physical slot.
    pub const fn active_slot(&self) -> usize {
        self.active_slot
    }

    /// Return the transaction lifecycle state.
    pub const fn transaction_state(&self) -> JournalTransactionState {
        self.transaction.state()
    }

    /// Commit the current store into the inactive slot and verify it by scan.
    ///
    /// `image` must hold both slot images, `payload` must hold the complete
    /// FBS1 snapshot, and `record_scratch` must hold the aligned journal
    /// record. All are caller-owned; no input-derived allocation occurs here.
    pub fn commit_snapshot(
        &mut self,
        image: &mut [u8],
        payload: &mut [u8],
        record_scratch: &mut [u8],
    ) -> Result<CommitOutcome, ProviderError<B::Error>> {
        self.ensure_ready()?;
        let image_required = self
            .layout
            .image_bytes()
            .ok_or(ProviderError::Layout(LayoutError::OutOfBounds))?;
        if image.len() < image_required {
            return Err(ProviderError::ImageBufferTooSmall {
                required: image_required,
            });
        }
        let payload_len = self
            .store
            .snapshot_encoded_len()
            .map_err(ProviderError::Snapshot)?;
        if payload_len > payload.len() {
            return Err(ProviderError::PayloadBufferTooSmall {
                required: payload_len,
            });
        }
        let record_len = self
            .layout
            .padded_record_len(&self.backend, payload_len)
            .map_err(ProviderError::Layout)?;
        if record_len > record_scratch.len() {
            return Err(ProviderError::RecordBufferTooSmall {
                required: record_len,
            });
        }
        self.store
            .encode_snapshot(&mut payload[..payload_len])
            .map_err(ProviderError::Snapshot)?;

        let generation = self
            .transaction
            .reserve_generation()
            .map_err(ProviderError::Transaction)?;
        let target_slot = 1 - self.active_slot;
        let target_offset = self
            .layout
            .slot_offset(target_slot)
            .ok_or(ProviderError::Layout(LayoutError::OutOfBounds))?;

        if let Err(source) = self.backend.erase(target_offset, self.layout.slot_bytes) {
            self.quarantine(generation)?;
            return Err(ProviderError::Erase {
                generation,
                slot: target_slot,
                source,
            });
        }
        if let Err(source) = write_record(
            &mut self.backend,
            target_offset,
            generation,
            &payload[..payload_len],
            record_scratch,
        ) {
            self.quarantine(generation)?;
            return Err(ProviderError::Write {
                generation,
                slot: target_slot,
                source,
            });
        }

        self.resolve_after_write(image, generation)
    }

    /// Re-scan both slots after an indeterminate write outcome.
    pub fn recover_in_place(
        &mut self,
        image: &mut [u8],
    ) -> Result<CommitOutcome, ProviderError<B::Error>> {
        let generation = match self.transaction.state() {
            JournalTransactionState::InFlight { generation }
            | JournalTransactionState::Quarantined { generation } => generation,
            JournalTransactionState::Ready => {
                return Err(ProviderError::Transaction(
                    JournalTransactionError::NoAttempt,
                ));
            }
        };
        self.resolve_after_write(image, generation)
    }

    /// Borrow the underlying backend for board-specific diagnostics.
    pub fn backend(&self) -> &B {
        &self.backend
    }

    /// Consume the provider and return its backend for restart simulation or
    /// transfer into a board-owned runtime.
    pub fn into_backend(self) -> B {
        self.backend
    }

    fn ensure_ready(&self) -> Result<(), ProviderError<B::Error>> {
        match self.transaction.state() {
            JournalTransactionState::Ready => Ok(()),
            JournalTransactionState::InFlight { generation } => Err(ProviderError::Transaction(
                JournalTransactionError::AlreadyInFlight { generation },
            )),
            JournalTransactionState::Quarantined { generation } => Err(ProviderError::Transaction(
                JournalTransactionError::Quarantined { generation },
            )),
        }
    }

    fn quarantine(&mut self, generation: u64) -> Result<(), ProviderError<B::Error>> {
        self.transaction
            .mark_indeterminate(generation)
            .map_err(ProviderError::Transaction)
    }

    fn resolve_after_write(
        &mut self,
        image: &mut [u8],
        generation: u64,
    ) -> Result<CommitOutcome, ProviderError<B::Error>> {
        let required = self
            .layout
            .image_bytes()
            .ok_or(ProviderError::Layout(LayoutError::OutOfBounds))?;
        if image.len() < required {
            return Err(ProviderError::ImageBufferTooSmall { required });
        }
        let recovered = match read_and_restore(
            &self.backend,
            self.layout,
            self.provider,
            &mut image[..required],
        ) {
            Ok(recovered) => recovered,
            Err(error) => {
                self.quarantine(generation)?;
                return Err(error);
            }
        };
        let classification = self
            .transaction
            .resolve_scan(generation, recovered.scan)
            .map_err(ProviderError::Transaction)?;
        match classification {
            RecoveryClassification::Committed { generation } => {
                self.store = recovered.store;
                self.active_slot = recovered.slot.unwrap_or(self.active_slot);
                Ok(CommitOutcome::Committed { generation })
            }
            RecoveryClassification::NotCommitted => {
                self.store = recovered.store;
                self.active_slot = recovered.slot.unwrap_or(self.active_slot);
                Ok(CommitOutcome::NotCommitted { generation })
            }
            RecoveryClassification::Indeterminate => {
                Ok(CommitOutcome::Indeterminate { generation })
            }
        }
    }
}

struct Recovered<'a, const SLOTS: usize, const BUNDLE_BYTES: usize, const TOTAL_BYTES: u64> {
    store: FixedBundleStore<SLOTS, BUNDLE_BYTES, TOTAL_BYTES>,
    generation: Option<u64>,
    slot: Option<usize>,
    scan: ScanResult<'a>,
}

fn read_and_restore<'a, B, const SLOTS: usize, const BUNDLE_BYTES: usize, const TOTAL_BYTES: u64>(
    backend: &B,
    layout: JournalLayout,
    provider: u64,
    image: &'a mut [u8],
) -> Result<Recovered<'a, SLOTS, BUNDLE_BYTES, TOTAL_BYTES>, ProviderError<B::Error>>
where
    B: JournalBackend,
{
    let slot_bytes = layout.slot_bytes;
    for slot in 0..2 {
        let offset = layout
            .slot_offset(slot)
            .ok_or(ProviderError::Layout(LayoutError::OutOfBounds))?;
        let start = slot * slot_bytes;
        backend
            .read(offset, &mut image[start..start + slot_bytes])
            .map_err(|source| ProviderError::Read { slot, source })?;
    }
    let first = scan_first(&image[..slot_bytes]);
    let second = scan_first(&image[slot_bytes..slot_bytes * 2]);
    let selected = match (first.selected, second.selected) {
        (None, None) => None,
        (Some(record), None) => Some((0, record)),
        (None, Some(record)) => Some((1, record)),
        (Some(first), Some(second)) if first.generation > second.generation => Some((0, first)),
        (Some(first), Some(second)) if second.generation > first.generation => Some((1, second)),
        (Some(first), Some(_)) => {
            return Err(ProviderError::AmbiguousGeneration {
                generation: first.generation,
            });
        }
    };

    let Some((slot, record)) = selected else {
        let store = FixedBundleStore::new(provider).map_err(ProviderError::Store)?;
        return Ok(Recovered {
            store,
            generation: None,
            slot: None,
            scan: ScanResult {
                selected: None,
                candidates: first.candidates.saturating_add(second.candidates),
                valid: first.valid.saturating_add(second.valid),
                rejected: first.rejected.saturating_add(second.rejected),
            },
        });
    };

    let slot_image = if slot == 0 {
        &image[..slot_bytes]
    } else {
        &image[slot_bytes..slot_bytes * 2]
    };
    if record.payload.len() > layout.max_payload_bytes {
        return Err(ProviderError::Layout(LayoutError::RecordTooLarge));
    }
    let validated = scan_first_validated(slot_image, |selected| {
        if selected.generation != record.generation {
            return Err(FixedSnapshotError::InvalidBundle);
        }
        FixedBundleStore::<SLOTS, BUNDLE_BYTES, TOTAL_BYTES>::restore_snapshot(
            provider,
            selected.payload,
        )
        .map(|_| ())
    })
    .map_err(ProviderError::Snapshot)?;
    let selected = validated
        .selected
        .ok_or(ProviderError::Snapshot(FixedSnapshotError::InvalidBundle))?;
    let store = FixedBundleStore::restore_snapshot(provider, selected.payload)
        .map_err(ProviderError::Snapshot)?;
    Ok(Recovered {
        store,
        generation: Some(selected.generation),
        slot: Some(slot),
        scan: ScanResult {
            selected: Some(selected),
            candidates: first.candidates.saturating_add(second.candidates),
            valid: first.valid.saturating_add(second.valid),
            rejected: first.rejected.saturating_add(second.rejected),
        },
    })
}

const fn align_up(value: usize, alignment: usize) -> Option<usize> {
    if alignment == 0 {
        return None;
    }
    let remainder = value % alignment;
    if remainder == 0 {
        Some(value)
    } else {
        value.checked_add(alignment - remainder)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::Cell;
    use mmp_core::{BundleMeta, EmbeddedBundleStore, SlotState};
    use mmp_crypto::Identity;
    use mmp_flash_journal::{BackendError, Geometry};
    use mmp_types::bundle::{class, Bundle};
    use mmp_types::domain::SigDomain;
    use mmp_types::ids::{Id16, Sig64};

    const CAPACITY: usize = 2_048;
    const JOURNAL_SLOT: usize = 1_024;
    const MAX_PAYLOAD: usize = 768;
    type Provider = JournaledFixedStore<MemoryBackend, 2, 512, 2_048>;

    struct MemoryBackend {
        bytes: [u8; CAPACITY],
        fail_next_write: bool,
        fail_next_read: Cell<bool>,
        fail_next_erase: bool,
        partial_write: Option<(usize, usize)>,
        write_count: usize,
    }

    impl MemoryBackend {
        fn new() -> Self {
            Self {
                bytes: [0xff; CAPACITY],
                fail_next_write: false,
                fail_next_read: Cell::new(false),
                fail_next_erase: false,
                partial_write: None,
                write_count: 0,
            }
        }

        fn fail_partial_write_at_call(&mut self, call: usize, bytes: usize) {
            self.partial_write = Some((call, bytes));
        }

        fn check(&self, offset: usize, len: usize) -> Result<(), BackendError> {
            let end = offset.checked_add(len).ok_or(BackendError::OutOfBounds)?;
            if end > CAPACITY {
                Err(BackendError::OutOfBounds)
            } else {
                Ok(())
            }
        }
    }

    impl JournalBackend for MemoryBackend {
        type Error = BackendError;

        fn geometry(&self) -> Geometry {
            Geometry {
                capacity: CAPACITY,
                write_alignment: 4,
                erase_alignment: 256,
            }
        }

        fn read(&self, offset: usize, out: &mut [u8]) -> Result<(), Self::Error> {
            self.check(offset, out.len())?;
            if self.fail_next_read.replace(false) {
                return Err(BackendError::InjectedFailure);
            }
            out.copy_from_slice(&self.bytes[offset..offset + out.len()]);
            Ok(())
        }

        fn write(&mut self, offset: usize, bytes: &[u8]) -> Result<(), Self::Error> {
            self.check(offset, bytes.len())?;
            if !offset.is_multiple_of(4) || !bytes.len().is_multiple_of(4) {
                return Err(BackendError::Unaligned);
            }
            let call = self.write_count;
            self.write_count = self.write_count.saturating_add(1);
            if self.fail_next_write {
                self.fail_next_write = false;
                return Err(BackendError::InjectedFailure);
            }
            if bytes.iter().enumerate().any(|(index, value)| {
                value | self.bytes[offset + index] != self.bytes[offset + index]
            }) {
                return Err(BackendError::ProgramViolation);
            }
            if let Some((target, limit)) = self.partial_write {
                if target == call {
                    self.partial_write = None;
                    let written = limit.min(bytes.len());
                    for (index, value) in bytes[..written].iter().enumerate() {
                        self.bytes[offset + index] &= *value;
                    }
                    return Err(BackendError::InjectedFailure);
                }
            }
            for (index, value) in bytes.iter().enumerate() {
                self.bytes[offset + index] &= *value;
            }
            Ok(())
        }

        fn erase(&mut self, offset: usize, len: usize) -> Result<(), Self::Error> {
            self.check(offset, len)?;
            if !offset.is_multiple_of(256) || !len.is_multiple_of(256) {
                return Err(BackendError::Unaligned);
            }
            if self.fail_next_erase {
                self.fail_next_erase = false;
                return Err(BackendError::InjectedFailure);
            }
            self.bytes[offset..offset + len].fill(0xff);
            Ok(())
        }
    }

    fn layout(backend: &MemoryBackend) -> JournalLayout {
        JournalLayout::new(backend, 0, JOURNAL_SLOT, MAX_PAYLOAD).unwrap()
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

    fn bundle_meta(bundle: &Bundle) -> BundleMeta {
        BundleMeta {
            id: bundle.id(),
            class: bundle.class,
            origin: bundle.origin_node(),
            dest: bundle.dest,
            priority: bundle.priority,
            effective_priority: bundle.priority,
            size: bundle.encode().len() as u32,
            stored_at: 1,
            expires_at: 60_001,
            budget: 4,
            own_origin: true,
            slot: SlotState::Stored,
        }
    }

    fn buffers() -> ([u8; CAPACITY], [u8; MAX_PAYLOAD], [u8; JOURNAL_SLOT]) {
        ([0; CAPACITY], [0; MAX_PAYLOAD], [0; JOURNAL_SLOT])
    }

    #[test]
    fn layout_rejects_a_slot_that_cannot_hold_the_declared_payload() {
        let backend = MemoryBackend::new();
        assert_eq!(
            JournalLayout::new(&backend, 0, 256, MAX_PAYLOAD),
            Err(LayoutError::RecordTooLarge)
        );
    }

    #[test]
    fn committed_snapshot_survives_restart_with_bundle_validation() {
        let backend = MemoryBackend::new();
        let journal_layout = layout(&backend);
        let (mut image, mut payload, mut scratch) = buffers();
        let mut provider = Provider::recover(backend, journal_layout, 7, &mut image).unwrap();
        let bundle = signed_bundle(41, b"persisted");
        let wire = bundle.encode();
        provider
            .store_mut()
            .try_put(bundle_meta(&bundle), &wire)
            .unwrap();

        assert_eq!(
            provider
                .commit_snapshot(&mut image, &mut payload, &mut scratch)
                .unwrap(),
            CommitOutcome::Committed { generation: 1 }
        );
        let backend = provider.into_backend();
        let mut restart_image = [0u8; CAPACITY];
        let recovered = Provider::recover(backend, journal_layout, 7, &mut restart_image).unwrap();
        assert_eq!(recovered.generation(), 1);
        assert_eq!(recovered.store().len(), 1);
        let mut read = [0u8; 512];
        assert_eq!(
            recovered
                .store()
                .read_bundle(&bundle.id(), &mut read)
                .unwrap(),
            wire.len()
        );
        assert_eq!(&read[..wire.len()], wire.as_slice());
    }

    #[test]
    fn failed_write_quarantines_and_recovery_proves_not_committed() {
        let mut backend = MemoryBackend::new();
        backend.fail_next_write = true;
        let journal_layout = layout(&backend);
        let (mut image, mut payload, mut scratch) = buffers();
        let mut provider = Provider::recover(backend, journal_layout, 7, &mut image).unwrap();

        let error = provider
            .commit_snapshot(&mut image, &mut payload, &mut scratch)
            .unwrap_err();
        assert!(matches!(error, ProviderError::Write { generation: 1, .. }));
        assert_eq!(
            provider.transaction_state(),
            JournalTransactionState::Quarantined { generation: 1 }
        );
        assert_eq!(
            provider.recover_in_place(&mut image).unwrap(),
            CommitOutcome::NotCommitted { generation: 1 }
        );
        assert_eq!(provider.transaction_state(), JournalTransactionState::Ready);
        assert_eq!(provider.generation(), 0);
    }

    #[test]
    fn a_quarantined_provider_cannot_start_a_second_commit() {
        let mut backend = MemoryBackend::new();
        backend.fail_next_write = true;
        let journal_layout = layout(&backend);
        let (mut image, mut payload, mut scratch) = buffers();
        let mut provider = Provider::recover(backend, journal_layout, 7, &mut image).unwrap();
        let _ = provider.commit_snapshot(&mut image, &mut payload, &mut scratch);
        assert!(matches!(
            provider.commit_snapshot(&mut image, &mut payload, &mut scratch),
            Err(ProviderError::Transaction(
                JournalTransactionError::Quarantined { generation: 1 }
            ))
        ));
    }

    #[test]
    fn erase_failure_quarantines_and_recovery_proves_not_committed() {
        let mut backend = MemoryBackend::new();
        backend.fail_next_erase = true;
        let journal_layout = layout(&backend);
        let (mut image, mut payload, mut scratch) = buffers();
        let mut provider = Provider::recover(backend, journal_layout, 7, &mut image).unwrap();
        let bundle = signed_bundle(42, b"erase fault");
        let wire = bundle.encode();
        provider
            .store_mut()
            .try_put(bundle_meta(&bundle), &wire)
            .unwrap();

        assert!(matches!(
            provider.commit_snapshot(&mut image, &mut payload, &mut scratch),
            Err(ProviderError::Erase {
                generation: 1,
                slot: 1,
                source: BackendError::InjectedFailure,
            })
        ));
        assert_eq!(
            provider.transaction_state(),
            JournalTransactionState::Quarantined { generation: 1 }
        );
        assert_eq!(
            provider.recover_in_place(&mut image).unwrap(),
            CommitOutcome::NotCommitted { generation: 1 }
        );
        assert_eq!(provider.transaction_state(), JournalTransactionState::Ready);
        assert_eq!(provider.generation(), 0);
        // Recovery replaces the uncommitted in-memory view with the durable
        // empty journal state; the failed snapshot is not published.
        assert_eq!(provider.store().len(), 0);
    }

    #[test]
    fn torn_commit_write_quarantines_and_does_not_promote_partial_marker() {
        let mut backend = MemoryBackend::new();
        // An empty provider emits one body write followed by one final
        // commit-marker write. Mutate only the first marker byte, then report
        // failure, modelling a torn physical write.
        backend.fail_partial_write_at_call(1, 1);
        let journal_layout = layout(&backend);
        let (mut image, mut payload, mut scratch) = buffers();
        let mut provider = Provider::recover(backend, journal_layout, 7, &mut image).unwrap();
        let bundle = signed_bundle(43, b"torn write");
        let wire = bundle.encode();
        provider
            .store_mut()
            .try_put(bundle_meta(&bundle), &wire)
            .unwrap();

        assert!(matches!(
            provider.commit_snapshot(&mut image, &mut payload, &mut scratch),
            Err(ProviderError::Write {
                generation: 1,
                slot: 1,
                source: JournalWriteError::CommitWrite(BackendError::InjectedFailure),
            })
        ));
        assert_eq!(
            provider.transaction_state(),
            JournalTransactionState::Quarantined { generation: 1 }
        );
        assert_eq!(
            provider.recover_in_place(&mut image).unwrap(),
            CommitOutcome::NotCommitted { generation: 1 }
        );
        assert_eq!(provider.transaction_state(), JournalTransactionState::Ready);
        assert_eq!(provider.generation(), 0);
        assert_eq!(provider.store().len(), 0);
    }

    #[test]
    fn post_write_read_failure_quarantines_then_recovery_commits() {
        let backend = MemoryBackend::new();
        let journal_layout = layout(&backend);
        let (mut image, mut payload, mut scratch) = buffers();
        let mut provider = Provider::recover(backend, journal_layout, 7, &mut image).unwrap();
        let bundle = signed_bundle(44, b"read fault");
        let wire = bundle.encode();
        provider
            .store_mut()
            .try_put(bundle_meta(&bundle), &wire)
            .unwrap();

        // The append succeeds, but the first verification read fails before
        // the provider can classify the generation.
        provider.backend.fail_next_read.set(true);
        assert!(matches!(
            provider.commit_snapshot(&mut image, &mut payload, &mut scratch),
            Err(ProviderError::Read {
                slot: 0,
                source: BackendError::InjectedFailure,
            })
        ));
        assert_eq!(
            provider.transaction_state(),
            JournalTransactionState::Quarantined { generation: 1 }
        );

        // A retry with healthy reads proves the committed generation and
        // publishes the validated snapshot.
        assert_eq!(
            provider.recover_in_place(&mut image).unwrap(),
            CommitOutcome::Committed { generation: 1 }
        );
        assert_eq!(provider.transaction_state(), JournalTransactionState::Ready);
        assert_eq!(provider.generation(), 1);
        assert_eq!(provider.store().len(), 1);
        let mut read = [0u8; 512];
        assert_eq!(
            provider
                .store()
                .read_bundle(&bundle.id(), &mut read)
                .unwrap(),
            wire.len()
        );
        assert_eq!(&read[..wire.len()], wire.as_slice());
    }

    #[test]
    fn recovery_read_failure_keeps_provider_quarantined_until_a_clean_retry() {
        let backend = MemoryBackend::new();
        let journal_layout = layout(&backend);
        let (mut image, mut payload, mut scratch) = buffers();
        let mut provider = Provider::recover(backend, journal_layout, 7, &mut image).unwrap();
        let bundle = signed_bundle(45, b"quarantine");
        let wire = bundle.encode();
        provider
            .store_mut()
            .try_put(bundle_meta(&bundle), &wire)
            .unwrap();

        provider.backend.fail_next_read.set(true);
        let _ = provider.commit_snapshot(&mut image, &mut payload, &mut scratch);
        assert_eq!(
            provider.transaction_state(),
            JournalTransactionState::Quarantined { generation: 1 }
        );

        provider.backend.fail_next_read.set(true);
        assert!(matches!(
            provider.recover_in_place(&mut image),
            Err(ProviderError::Read {
                slot: 0,
                source: BackendError::InjectedFailure,
            })
        ));
        assert_eq!(
            provider.transaction_state(),
            JournalTransactionState::Quarantined { generation: 1 }
        );
        assert_eq!(
            provider.recover_in_place(&mut image).unwrap(),
            CommitOutcome::Committed { generation: 1 }
        );
        assert_eq!(provider.transaction_state(), JournalTransactionState::Ready);
        assert_eq!(provider.store().len(), 1);
    }
}
