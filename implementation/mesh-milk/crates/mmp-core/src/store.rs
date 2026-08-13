//! BundleStore seam (doc 12 §2) + the in-memory reference implementation.
//!
//! Per-bundle FSM state is NOT object-per-bundle (doc 11 §8): it lives in the
//! record header (`SlotState`), and RBSR's contract is ordered `Id16` range
//! iteration.

use alloc::collections::BTreeMap;
use alloc::collections::BTreeSet;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicUsize, Ordering};

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

/// Why a bounded store read could not produce its result.
///
/// The caller-owned read methods below are additive to the legacy allocating
/// methods. A provider may report the required output length without mutating
/// the caller's buffers, allowing a fixed-capacity caller to apply its own
/// admission policy before retrying.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreReadError {
    /// No bundle exists for the requested ID.
    NotFound,
    /// The output slice is too small; no output or store state was mutated.
    OutputTooSmall {
        /// Minimum number of elements/bytes required for a retry.
        required: usize,
    },
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

/// Provider-local capability returned by a successful reservation.
///
/// A reservation is intentionally not serializable and carries no wire
/// meaning. Its fields remain private, but independent provider
/// implementations may construct a token with [`StoreReservation::from_parts`].
/// The constructor is not an authorization boundary: the owning provider must
/// validate its namespace, nonce, ID, and declared size again on commit or
/// abort. Reservations are runtime-only and are not durable.
#[derive(Debug, PartialEq, Eq)]
pub struct StoreReservation {
    provider: u64,
    id: Id16,
    declared_size: u32,
    nonce: u64,
}

/// Provider-local capability for one all-or-nothing set of reserved bundle
/// publications. It is runtime-only, non-serializable, and not cloneable so
/// callers cannot mistake it for a wire or persistence value.
#[derive(Debug, PartialEq, Eq)]
pub struct StoreBatchReservation {
    provider: u64,
    nonce: u64,
}

/// Opaque identity used by adapters that need to track reservation metadata
/// alongside a [`StoreReservation`]. It cannot be constructed outside this
/// module and carries no authorization by itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct StoreReservationKey {
    provider: u64,
    nonce: u64,
}

/// Opaque identity used by adapters that need to account for a
/// [`StoreBatchReservation`] alongside their provider-local runtime state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct StoreBatchReservationKey {
    provider: u64,
    nonce: u64,
}

impl StoreReservation {
    /// Construct a provider-local reservation token.
    ///
    /// This is provided for `BundleStore` implementations outside `mmp-core`.
    /// `provider` is an implementation-chosen namespace and `nonce` must be
    /// checked against provider-owned in-memory state before any publication.
    /// A caller that forges either value must be rejected by the provider; the
    /// token itself does not grant storage authority.
    pub const fn from_parts(
        provider: u64,
        id: Id16,
        declared_size: u32,
        nonce: u64,
    ) -> StoreReservation {
        StoreReservation {
            provider,
            id,
            declared_size,
            nonce,
        }
    }

    /// Return the provider-local opaque identity for adapter bookkeeping.
    pub fn key(&self) -> StoreReservationKey {
        StoreReservationKey {
            provider: self.provider,
            nonce: self.nonce,
        }
    }
}

impl StoreBatchReservation {
    /// Construct a provider-local atomic-batch reservation token.
    ///
    /// This is provided for `BundleStore` implementations outside `mmp-core`.
    /// The owning provider must validate the namespace and nonce against its
    /// own pending-batch table before committing or aborting the batch.
    pub const fn from_parts(provider: u64, nonce: u64) -> StoreBatchReservation {
        StoreBatchReservation { provider, nonce }
    }

    /// Return the provider-local opaque identity for adapter bookkeeping.
    pub fn key(&self) -> StoreBatchReservationKey {
        StoreBatchReservationKey {
            provider: self.provider,
            nonce: self.nonce,
        }
    }
}

/// Why a provider could not create a reservation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreReserveError {
    /// The provider does not implement reservations.
    Unsupported,
    /// The request is malformed or conflicts with an active reservation.
    Invalid,
    /// Local byte, slot, or nonce accounting rejected the request.
    Capacity,
}

/// Why a provider could not publish a reserved bundle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreCommitError {
    /// The provider does not implement reservations.
    Unsupported,
    /// The token, metadata, or payload does not match the reservation.
    Invalid,
    /// The stored replacement changed since reservation or another local
    /// mutation made the reservation impossible to publish safely.
    Conflict,
    /// Local accounting limits rejected the commit.
    Capacity,
}

/// Why a provider could not release a reservation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreAbortError {
    /// The provider does not implement reservations.
    Unsupported,
    /// The token does not belong to this provider.
    Invalid,
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
    fn try_put(
        &mut self,
        meta: BundleMeta,
        bytes: &[u8],
    ) -> Result<StorePutOutcome, StorePutError> {
        let replaced = self.meta(&meta.id).is_some();
        self.put(meta, bytes);
        Ok(if replaced {
            StorePutOutcome::Replaced
        } else {
            StorePutOutcome::Inserted
        })
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
    /// Reserve capacity for one future bundle publication without allocating
    /// or storing its payload. The default is fail-closed for legacy
    /// providers. A reservation is provider-local, runtime-only state; it is
    /// not a wire or persistence token.
    fn try_reserve(&mut self, meta: BundleMeta) -> Result<StoreReservation, StoreReserveError> {
        let _ = meta;
        Err(StoreReserveError::Unsupported)
    }
    /// Reserve capacity for several future publications as one preflight.
    /// Implementations must reject duplicate IDs and leave both visible state
    /// and reservation accounting unchanged on failure. The returned tokens
    /// can be committed or aborted independently; this method does not claim
    /// multi-record commit atomicity.
    fn try_reserve_batch(
        &mut self,
        metas: &[BundleMeta],
    ) -> Result<Vec<StoreReservation>, StoreReserveError> {
        let _ = metas;
        Err(StoreReserveError::Unsupported)
    }
    /// Reserve one ordered, all-or-nothing future publication batch. The
    /// opaque token has provider-local runtime meaning only. Unlike
    /// [`Self::try_reserve_batch`], its entries can only be committed or
    /// aborted together.
    fn try_reserve_batch_atomic(
        &mut self,
        metas: &[BundleMeta],
    ) -> Result<StoreBatchReservation, StoreReserveError> {
        let _ = metas;
        Err(StoreReserveError::Unsupported)
    }
    /// Publish bytes against a prior reservation. All token/metadata/length
    /// and replacement checks happen before any visible mutation. A failed
    /// commit retains the reservation for retry or abort; a successful commit
    /// consumes it exactly once.
    fn commit_reserved(
        &mut self,
        reservation: &StoreReservation,
        meta: BundleMeta,
        bytes: &[u8],
    ) -> Result<StorePutOutcome, StoreCommitError> {
        let _ = (reservation, meta, bytes);
        Err(StoreCommitError::Unsupported)
    }
    /// Atomically publish entries against an ordered batch reservation. The
    /// count, order, IDs, declared lengths, replacement generation, and
    /// capacity must all validate before any entry becomes visible. A failed
    /// commit leaves the batch available for retry or abort.
    fn commit_reserved_batch(
        &mut self,
        batch: &StoreBatchReservation,
        entries: &[StoreBatchEntry<'_>],
    ) -> Result<Vec<StorePutOutcome>, StoreCommitError> {
        let _ = (batch, entries);
        Err(StoreCommitError::Unsupported)
    }
    /// Release a prior reservation. Aborting an already released reservation
    /// is idempotent and does not affect committed entries.
    fn abort_reservation(&mut self, reservation: &StoreReservation) -> Result<(), StoreAbortError> {
        let _ = reservation;
        Err(StoreAbortError::Unsupported)
    }
    /// Idempotently release an uncommitted batch reservation. Aborting a
    /// consumed batch cannot delete records that its commit already published.
    fn abort_reserved_batch(
        &mut self,
        batch: &StoreBatchReservation,
    ) -> Result<(), StoreAbortError> {
        let _ = batch;
        Err(StoreAbortError::Unsupported)
    }
    /// Copy one bundle into caller-owned storage without requiring the
    /// provider to allocate a payload vector. On failure, `meta_out`,
    /// `bytes_out`, and provider state remain unchanged.
    fn get_into(
        &self,
        id: &Id16,
        meta_out: &mut BundleMeta,
        bytes_out: &mut [u8],
    ) -> Result<usize, StoreReadError> {
        let Some((meta, bytes)) = self.get(id) else {
            return Err(StoreReadError::NotFound);
        };
        if bytes.len() > bytes_out.len() {
            return Err(StoreReadError::OutputTooSmall {
                required: bytes.len(),
            });
        }
        bytes_out[..bytes.len()].copy_from_slice(&bytes);
        *meta_out = meta;
        Ok(bytes.len())
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
    /// Copy sorted IDs in `[lo, hi]` into caller-owned storage. The provider
    /// must leave the output slice unchanged when it reports insufficient
    /// capacity.
    fn ids_range_into(
        &self,
        lo: &Id16,
        hi: &Id16,
        out: &mut [Id16],
    ) -> Result<usize, StoreReadError> {
        let ids = self.ids_range(lo, hi);
        if ids.len() > out.len() {
            return Err(StoreReadError::OutputTooSmall {
                required: ids.len(),
            });
        }
        out[..ids.len()].copy_from_slice(&ids);
        Ok(ids.len())
    }
    /// Metadata for every bundle currently in the store.
    fn all_meta(&self) -> Vec<BundleMeta>;
    /// Visit every bundle metadata record in provider order without
    /// requiring the provider to allocate a result vector. The callback must
    /// not mutate the store or retain the copied metadata after it returns.
    /// Providers with a stable order should document it; callers that need a
    /// canonical order must sort their own output.
    fn for_each_meta(&self, visitor: &mut dyn FnMut(BundleMeta)) {
        for meta in self.all_meta() {
            visitor(meta);
        }
    }
    /// Copy all bundle metadata into caller-owned storage. The provider must
    /// leave the output slice unchanged when it reports insufficient capacity.
    fn all_meta_into(&self, out: &mut [BundleMeta]) -> Result<usize, StoreReadError> {
        let metas = self.all_meta();
        if metas.len() > out.len() {
            return Err(StoreReadError::OutputTooSmall {
                required: metas.len(),
            });
        }
        out[..metas.len()].copy_from_slice(&metas);
        Ok(metas.len())
    }
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
    /// Delete expired bundles and copy their IDs into caller-owned storage.
    /// Capacity is checked before deletion, so an insufficient output slice
    /// leaves both the store and the output unchanged.
    fn expire_into(&mut self, now_ms: u64, out: &mut [Id16]) -> Result<usize, StoreReadError> {
        let expired = self
            .all_meta()
            .into_iter()
            .filter(|meta| meta.expires_at <= now_ms)
            .count();
        if expired > out.len() {
            return Err(StoreReadError::OutputTooSmall { required: expired });
        }
        let ids = self.expire(now_ms);
        if ids.len() > out.len() {
            return Err(StoreReadError::OutputTooSmall {
                required: ids.len(),
            });
        }
        out[..ids.len()].copy_from_slice(&ids);
        Ok(ids.len())
    }
}

#[derive(Debug, Clone, Copy)]
struct ReservationRecord {
    id: Id16,
    meta: BundleMeta,
    declared_size: u32,
    old_len: Option<u64>,
    old_meta: Option<BundleMeta>,
    old_digest: Option<[u8; 32]>,
    old_generation: u64,
    reserved_bytes: u64,
    reserved_slot: bool,
}

#[derive(Debug, Clone)]
struct BatchReservationRecord {
    records: Vec<ReservationRecord>,
}

static NEXT_PROVIDER_ID: AtomicUsize = AtomicUsize::new(1);

fn allocate_provider_id() -> u64 {
    let id = NEXT_PROVIDER_ID.fetch_add(1, Ordering::Relaxed) as u64;
    if id == 0 {
        1
    } else {
        id
    }
}

/// Reference in-memory store (also the sim/test store).
pub struct MemStore {
    map: BTreeMap<Id16, (BundleMeta, Vec<u8>)>,
    generations: BTreeMap<Id16, u64>,
    bytes: u64,
    max_bytes: Option<u64>,
    max_slots: Option<usize>,
    reserved_bytes: u64,
    reserved_slots: usize,
    reservations: BTreeMap<u64, ReservationRecord>,
    batch_reservations: BTreeMap<u64, BatchReservationRecord>,
    next_nonce: u64,
    provider_id: u64,
}

impl Default for MemStore {
    fn default() -> Self {
        Self {
            map: BTreeMap::new(),
            generations: BTreeMap::new(),
            bytes: 0,
            max_bytes: None,
            max_slots: None,
            reserved_bytes: 0,
            reserved_slots: 0,
            reservations: BTreeMap::new(),
            batch_reservations: BTreeMap::new(),
            next_nonce: 1,
            provider_id: allocate_provider_id(),
        }
    }
}

impl MemStore {
    /// Builds an empty store: no bundles, zero total bytes.
    pub fn new() -> MemStore {
        MemStore::default()
    }

    /// Builds a store with explicit byte and slot limits. The limits apply to
    /// the checked [`BundleStore::try_put`] / [`BundleStore::try_put_batch`]
    /// seams, committed payloads, and all outstanding positive reservation
    /// deltas. The legacy infallible [`BundleStore::put`] seam remains an
    /// unchecked compatibility escape hatch; callers enforcing a quota must
    /// use one of the checked methods. Limits are local resource policy and
    /// have no wire-level meaning.
    pub fn with_limits(max_bytes: u64, max_slots: usize) -> MemStore {
        MemStore {
            max_bytes: Some(max_bytes),
            max_slots: Some(max_slots),
            ..MemStore::default()
        }
    }

    /// Bytes reserved for future publications, excluding already committed
    /// payload bytes.
    pub fn reserved_bytes(&self) -> u64 {
        self.reserved_bytes
    }

    /// New slots reserved for future publications.
    pub fn reserved_slots(&self) -> usize {
        self.reserved_slots
    }

    fn generation_for(&self, id: &Id16) -> u64 {
        self.generations.get(id).copied().unwrap_or(0)
    }

    fn bump_generation(&mut self, id: Id16) {
        let generation = self.generations.entry(id).or_insert(0);
        *generation = generation.wrapping_add(1);
    }

    fn next_reservation(
        &mut self,
        record: ReservationRecord,
    ) -> Result<StoreReservation, StoreReserveError> {
        let nonce = self.next_nonce;
        let next_nonce = self
            .next_nonce
            .checked_add(1)
            .ok_or(StoreReserveError::Capacity)?;
        let reserved_bytes = self
            .reserved_bytes
            .checked_add(record.reserved_bytes)
            .ok_or(StoreReserveError::Capacity)?;
        let reserved_slots = self
            .reserved_slots
            .checked_add(usize::from(record.reserved_slot))
            .ok_or(StoreReserveError::Capacity)?;
        self.next_nonce = next_nonce;
        self.reserved_bytes = reserved_bytes;
        self.reserved_slots = reserved_slots;
        self.reservations.insert(nonce, record);
        Ok(StoreReservation::from_parts(
            self.provider_id,
            record.id,
            record.declared_size,
            nonce,
        ))
    }

    fn reservation_for(
        &self,
        reservation: &StoreReservation,
    ) -> Result<ReservationRecord, StoreCommitError> {
        let record = self
            .reservations
            .get(&reservation.nonce)
            .copied()
            .ok_or(StoreCommitError::Invalid)?;
        if reservation.provider != self.provider_id
            || record.id != reservation.id
            || record.declared_size != reservation.declared_size
        {
            return Err(StoreCommitError::Invalid);
        }
        Ok(record)
    }

    fn batch_reservation_for(
        &self,
        batch: &StoreBatchReservation,
    ) -> Result<BatchReservationRecord, StoreCommitError> {
        if batch.provider != self.provider_id {
            return Err(StoreCommitError::Invalid);
        }
        self.batch_reservations
            .get(&batch.nonce)
            .cloned()
            .ok_or(StoreCommitError::Invalid)
    }

    fn reservation_id_is_pending(&self, id: Id16) -> bool {
        self.reservations
            .values()
            .any(|reservation| reservation.id == id)
            || self
                .batch_reservations
                .values()
                .any(|batch| batch.records.iter().any(|reservation| reservation.id == id))
    }

    fn preflight_reservations(
        &self,
        metas: &[BundleMeta],
    ) -> Result<Vec<ReservationRecord>, StoreReserveError> {
        if metas.is_empty() {
            return Err(StoreReserveError::Invalid);
        }

        let mut accounted = 0u64;
        for (stored_meta, body) in self.map.values() {
            if usize::try_from(stored_meta.size).ok() != Some(body.len()) {
                return Err(StoreReserveError::Invalid);
            }
            accounted = accounted
                .checked_add(u64::try_from(body.len()).map_err(|_| StoreReserveError::Capacity)?)
                .ok_or(StoreReserveError::Capacity)?;
        }
        if accounted != self.bytes {
            return Err(StoreReserveError::Invalid);
        }

        let mut ids = BTreeSet::new();
        let mut additional_bytes = 0u64;
        let mut additional_slots = 0usize;
        let mut records = Vec::with_capacity(metas.len());
        for meta in metas {
            if !ids.insert(meta.id) || self.reservation_id_is_pending(meta.id) {
                return Err(StoreReserveError::Invalid);
            }

            let (old_len, old_meta, old_digest) = self
                .map
                .get(&meta.id)
                .map(|(stored_meta, bytes)| {
                    (
                        Some(u64::try_from(bytes.len()).unwrap_or(u64::MAX)),
                        Some(*stored_meta),
                        Some(*blake3::hash(bytes).as_bytes()),
                    )
                })
                .unwrap_or((None, None, None));
            let requested = u64::from(meta.size);
            let reserved_bytes = requested.saturating_sub(old_len.unwrap_or(0));
            let reserved_slot = old_len.is_none();
            additional_bytes = additional_bytes
                .checked_add(reserved_bytes)
                .ok_or(StoreReserveError::Capacity)?;
            additional_slots = additional_slots
                .checked_add(usize::from(reserved_slot))
                .ok_or(StoreReserveError::Capacity)?;
            records.push(ReservationRecord {
                id: meta.id,
                meta: *meta,
                declared_size: meta.size,
                old_len,
                old_meta,
                old_digest,
                old_generation: self.generation_for(&meta.id),
                reserved_bytes,
                reserved_slot,
            });
        }

        let projected_bytes = self
            .bytes
            .checked_add(self.reserved_bytes)
            .and_then(|value| value.checked_add(additional_bytes))
            .ok_or(StoreReserveError::Capacity)?;
        if self.max_bytes.is_some_and(|limit| projected_bytes > limit) {
            return Err(StoreReserveError::Capacity);
        }
        let projected_slots = self
            .map
            .len()
            .checked_add(self.reserved_slots)
            .and_then(|value| value.checked_add(additional_slots))
            .ok_or(StoreReserveError::Capacity)?;
        if self.max_slots.is_some_and(|limit| projected_slots > limit) {
            return Err(StoreReserveError::Capacity);
        }
        Ok(records)
    }
}

impl BundleStore for MemStore {
    fn put(&mut self, meta: BundleMeta, bytes: &[u8]) {
        self.bytes += bytes.len() as u64;
        if let Some((_, old)) = self.map.insert(meta.id, (meta, bytes.to_vec())) {
            self.bytes -= old.len() as u64;
        }
        self.bump_generation(meta.id);
    }

    fn try_put(
        &mut self,
        meta: BundleMeta,
        bytes: &[u8],
    ) -> Result<StorePutOutcome, StorePutError> {
        if self.max_bytes.is_none()
            && self.max_slots.is_none()
            && self.reservations.is_empty()
            && self.batch_reservations.is_empty()
        {
            let replaced = self.map.contains_key(&meta.id);
            self.put(meta, bytes);
            return Ok(if replaced {
                StorePutOutcome::Replaced
            } else {
                StorePutOutcome::Inserted
            });
        }
        if usize::try_from(meta.size).ok() != Some(bytes.len())
            || !self.reservations.is_empty()
            || !self.batch_reservations.is_empty()
        {
            return Err(StorePutError::Capacity);
        }

        let mut accounted = 0u64;
        for (_, body) in self.map.values() {
            accounted = accounted
                .checked_add(u64::try_from(body.len()).map_err(|_| StorePutError::Capacity)?)
                .ok_or(StorePutError::Capacity)?;
        }
        if accounted != self.bytes {
            return Err(StorePutError::Capacity);
        }

        let old_len = self
            .map
            .get(&meta.id)
            .map(|(_, body)| u64::try_from(body.len()).unwrap_or(u64::MAX))
            .unwrap_or(0);
        let new_len = u64::try_from(bytes.len()).map_err(|_| StorePutError::Capacity)?;
        let projected_bytes = self
            .bytes
            .checked_sub(old_len)
            .and_then(|value| value.checked_add(new_len))
            .ok_or(StorePutError::Capacity)?;
        if self.max_bytes.is_some_and(|limit| projected_bytes > limit) {
            return Err(StorePutError::Capacity);
        }

        let replaced = self.map.contains_key(&meta.id);
        let projected_slots = self
            .map
            .len()
            .checked_add(usize::from(!replaced))
            .ok_or(StorePutError::Capacity)?;
        if self.max_slots.is_some_and(|limit| projected_slots > limit) {
            return Err(StorePutError::Capacity);
        }

        self.map.insert(meta.id, (meta, bytes.to_vec()));
        self.bytes = projected_bytes;
        self.bump_generation(meta.id);
        Ok(if replaced {
            StorePutOutcome::Replaced
        } else {
            StorePutOutcome::Inserted
        })
    }

    fn try_put_batch(
        &mut self,
        entries: &[StoreBatchEntry<'_>],
    ) -> Result<Vec<StorePutOutcome>, StoreBatchError> {
        if entries.is_empty() {
            return Ok(Vec::new());
        }
        // A legacy aggregate write has no reservation-aware accounting. Do
        // not let it bypass outstanding capabilities or publish a state whose
        // future reserved capacity is no longer meaningful.
        if !self.reservations.is_empty() || !self.batch_reservations.is_empty() {
            return Err(StoreBatchError::Invalid);
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
        if self.max_bytes.is_some_and(|limit| projected_bytes > limit) {
            return Err(StoreBatchError::Capacity);
        }
        let added_slots = entries
            .iter()
            .filter(|entry| !self.map.contains_key(&entry.meta.id))
            .count();
        let projected_slots = self
            .map
            .len()
            .checked_add(added_slots)
            .ok_or(StoreBatchError::Capacity)?;
        if self.max_slots.is_some_and(|limit| projected_slots > limit) {
            return Err(StoreBatchError::Capacity);
        }

        // Stage all allocations and replacements in a private image. The
        // visible map and accounting counter are swapped only after every
        // entry has been accepted, so a rejected batch cannot expose a prefix.
        let mut staged = self.map.clone();
        for entry in entries {
            staged.insert(entry.meta.id, (entry.meta, entry.bytes.to_vec()));
        }
        self.map = staged;
        self.bytes = projected_bytes;
        for entry in entries {
            self.bump_generation(entry.meta.id);
        }
        Ok(outcomes)
    }

    fn try_reserve(&mut self, meta: BundleMeta) -> Result<StoreReservation, StoreReserveError> {
        let records = self.preflight_reservations(&[meta])?;
        self.next_reservation(records[0])
    }

    fn try_reserve_batch(
        &mut self,
        metas: &[BundleMeta],
    ) -> Result<Vec<StoreReservation>, StoreReserveError> {
        let records = self.preflight_reservations(metas)?;
        let additional_bytes = records.iter().try_fold(0u64, |total, record| {
            total
                .checked_add(record.reserved_bytes)
                .ok_or(StoreReserveError::Capacity)
        })?;
        let additional_slots = records.iter().try_fold(0usize, |total, record| {
            total
                .checked_add(usize::from(record.reserved_slot))
                .ok_or(StoreReserveError::Capacity)
        })?;
        let count = u64::try_from(records.len()).map_err(|_| StoreReserveError::Capacity)?;
        let next_nonce = self
            .next_nonce
            .checked_add(count)
            .ok_or(StoreReserveError::Capacity)?;
        let reserved_bytes = self
            .reserved_bytes
            .checked_add(additional_bytes)
            .ok_or(StoreReserveError::Capacity)?;
        let reserved_slots = self
            .reserved_slots
            .checked_add(additional_slots)
            .ok_or(StoreReserveError::Capacity)?;

        let mut tokens = Vec::with_capacity(records.len());
        for (offset, record) in records.into_iter().enumerate() {
            let nonce = self
                .next_nonce
                .checked_add(u64::try_from(offset).map_err(|_| StoreReserveError::Capacity)?)
                .ok_or(StoreReserveError::Capacity)?;
            self.reservations.insert(nonce, record);
            tokens.push(StoreReservation::from_parts(
                self.provider_id,
                record.id,
                record.declared_size,
                nonce,
            ));
        }
        self.next_nonce = next_nonce;
        self.reserved_bytes = reserved_bytes;
        self.reserved_slots = reserved_slots;
        Ok(tokens)
    }

    fn try_reserve_batch_atomic(
        &mut self,
        metas: &[BundleMeta],
    ) -> Result<StoreBatchReservation, StoreReserveError> {
        let records = self.preflight_reservations(metas)?;
        let additional_bytes = records.iter().try_fold(0u64, |total, record| {
            total
                .checked_add(record.reserved_bytes)
                .ok_or(StoreReserveError::Capacity)
        })?;
        let additional_slots = records.iter().try_fold(0usize, |total, record| {
            total
                .checked_add(usize::from(record.reserved_slot))
                .ok_or(StoreReserveError::Capacity)
        })?;
        let nonce = self.next_nonce;
        let next_nonce = nonce.checked_add(1).ok_or(StoreReserveError::Capacity)?;
        let reserved_bytes = self
            .reserved_bytes
            .checked_add(additional_bytes)
            .ok_or(StoreReserveError::Capacity)?;
        let reserved_slots = self
            .reserved_slots
            .checked_add(additional_slots)
            .ok_or(StoreReserveError::Capacity)?;

        self.next_nonce = next_nonce;
        self.reserved_bytes = reserved_bytes;
        self.reserved_slots = reserved_slots;
        self.batch_reservations
            .insert(nonce, BatchReservationRecord { records });
        Ok(StoreBatchReservation::from_parts(self.provider_id, nonce))
    }

    fn commit_reserved(
        &mut self,
        reservation: &StoreReservation,
        meta: BundleMeta,
        bytes: &[u8],
    ) -> Result<StorePutOutcome, StoreCommitError> {
        let record = self.reservation_for(reservation)?;
        if meta != record.meta
            || meta.id != reservation.id
            || meta.size != reservation.declared_size
            || usize::try_from(meta.size).ok() != Some(bytes.len())
        {
            return Err(StoreCommitError::Invalid);
        }

        let current_old_len = self
            .map
            .get(&meta.id)
            .map(|(_, body)| u64::try_from(body.len()).unwrap_or(u64::MAX));
        if current_old_len != record.old_len
            || self.generation_for(&meta.id) != record.old_generation
        {
            return Err(StoreCommitError::Conflict);
        }
        if let Some((stored_meta, body)) = self.map.get(&meta.id) {
            if usize::try_from(stored_meta.size).ok() != Some(body.len())
                || Some(*stored_meta) != record.old_meta
                || Some(*blake3::hash(body).as_bytes()) != record.old_digest
            {
                return Err(StoreCommitError::Conflict);
            }
        } else if record.old_meta.is_some() || record.old_digest.is_some() {
            return Err(StoreCommitError::Conflict);
        }

        let old_len = current_old_len.unwrap_or(0);
        let new_len = u64::from(meta.size);
        let projected_bytes = self
            .bytes
            .checked_sub(old_len)
            .and_then(|value| value.checked_add(new_len))
            .ok_or(StoreCommitError::Conflict)?;
        let other_reserved = self
            .reserved_bytes
            .checked_sub(record.reserved_bytes)
            .ok_or(StoreCommitError::Conflict)?;
        if self.max_bytes.is_some_and(|limit| {
            projected_bytes
                .checked_add(other_reserved)
                .is_none_or(|value| value > limit)
        }) {
            return Err(StoreCommitError::Capacity);
        }
        let new_slot = current_old_len.is_none();
        let other_slots = self
            .reserved_slots
            .checked_sub(usize::from(record.reserved_slot))
            .ok_or(StoreCommitError::Conflict)?;
        let projected_slots = self
            .map
            .len()
            .checked_add(other_slots)
            .ok_or(StoreCommitError::Conflict)?;
        if self.max_slots.is_some_and(|limit| {
            projected_slots
                .checked_add(usize::from(new_slot))
                .is_none_or(|value| value > limit)
        }) {
            return Err(StoreCommitError::Capacity);
        }

        // Every fallible validation is complete before consuming the token or
        // allocating payload bytes. A successful publication is one visible
        // map mutation and consumes this reservation exactly once.
        let replaced = self.map.insert(meta.id, (meta, bytes.to_vec())).is_some();
        self.bytes = projected_bytes;
        self.bump_generation(meta.id);
        self.reservations.remove(&reservation.nonce);
        self.reserved_bytes -= record.reserved_bytes;
        if record.reserved_slot {
            self.reserved_slots -= 1;
        }
        Ok(if replaced {
            StorePutOutcome::Replaced
        } else {
            StorePutOutcome::Inserted
        })
    }

    fn abort_reservation(&mut self, reservation: &StoreReservation) -> Result<(), StoreAbortError> {
        if reservation.provider != self.provider_id {
            return Err(StoreAbortError::Invalid);
        }
        let Some(record) = self.reservations.get(&reservation.nonce).copied() else {
            // Abort is deliberately idempotent: an already committed or
            // aborted token cannot remove a committed entry or consume space.
            return Ok(());
        };
        if record.id != reservation.id || record.declared_size != reservation.declared_size {
            return Err(StoreAbortError::Invalid);
        }
        self.reservations.remove(&reservation.nonce);
        self.reserved_bytes -= record.reserved_bytes;
        if record.reserved_slot {
            self.reserved_slots -= 1;
        }
        Ok(())
    }

    fn commit_reserved_batch(
        &mut self,
        batch: &StoreBatchReservation,
        entries: &[StoreBatchEntry<'_>],
    ) -> Result<Vec<StorePutOutcome>, StoreCommitError> {
        let batch_record = self.batch_reservation_for(batch)?;
        if entries.len() != batch_record.records.len() {
            return Err(StoreCommitError::Invalid);
        }

        let mut ids = BTreeSet::new();
        let mut replaced_bytes = 0u64;
        let mut added_bytes = 0u64;
        let mut reserved_bytes = 0u64;
        let mut reserved_slots = 0usize;
        let mut outcomes = Vec::with_capacity(entries.len());
        for (entry, record) in entries.iter().zip(&batch_record.records) {
            if !ids.insert(entry.meta.id)
                || entry.meta.id != record.id
                || entry.meta != record.meta
                || entry.meta.size != record.declared_size
                || usize::try_from(entry.meta.size).ok() != Some(entry.bytes.len())
            {
                return Err(StoreCommitError::Invalid);
            }
            let current = self.map.get(&entry.meta.id);
            let current_old_len =
                current.map(|(_, body)| u64::try_from(body.len()).unwrap_or(u64::MAX));
            if current_old_len != record.old_len
                || self.generation_for(&entry.meta.id) != record.old_generation
            {
                return Err(StoreCommitError::Conflict);
            }
            if let Some((stored_meta, body)) = current {
                if usize::try_from(stored_meta.size).ok() != Some(body.len())
                    || Some(*stored_meta) != record.old_meta
                    || Some(*blake3::hash(body).as_bytes()) != record.old_digest
                {
                    return Err(StoreCommitError::Conflict);
                }
            } else if record.old_meta.is_some() || record.old_digest.is_some() {
                return Err(StoreCommitError::Conflict);
            }
            replaced_bytes = replaced_bytes
                .checked_add(current_old_len.unwrap_or(0))
                .ok_or(StoreCommitError::Conflict)?;
            added_bytes = added_bytes
                .checked_add(u64::from(entry.meta.size))
                .ok_or(StoreCommitError::Capacity)?;
            reserved_bytes = reserved_bytes
                .checked_add(record.reserved_bytes)
                .ok_or(StoreCommitError::Conflict)?;
            reserved_slots = reserved_slots
                .checked_add(usize::from(record.reserved_slot))
                .ok_or(StoreCommitError::Conflict)?;
            outcomes.push(if record.old_len.is_some() {
                StorePutOutcome::Replaced
            } else {
                StorePutOutcome::Inserted
            });
        }

        let projected_bytes = self
            .bytes
            .checked_sub(replaced_bytes)
            .and_then(|value| value.checked_add(added_bytes))
            .ok_or(StoreCommitError::Conflict)?;
        let other_reserved_bytes = self
            .reserved_bytes
            .checked_sub(reserved_bytes)
            .ok_or(StoreCommitError::Conflict)?;
        if self.max_bytes.is_some_and(|limit| {
            projected_bytes
                .checked_add(other_reserved_bytes)
                .is_none_or(|value| value > limit)
        }) {
            return Err(StoreCommitError::Capacity);
        }
        let other_reserved_slots = self
            .reserved_slots
            .checked_sub(reserved_slots)
            .ok_or(StoreCommitError::Conflict)?;
        let projected_slots = self
            .map
            .len()
            .checked_add(other_reserved_slots)
            .and_then(|value| value.checked_add(reserved_slots))
            .ok_or(StoreCommitError::Conflict)?;
        if self.max_slots.is_some_and(|limit| projected_slots > limit) {
            return Err(StoreCommitError::Capacity);
        }

        let mut staged = self.map.clone();
        for entry in entries {
            staged.insert(entry.meta.id, (entry.meta, entry.bytes.to_vec()));
        }
        self.map = staged;
        self.bytes = projected_bytes;
        for entry in entries {
            self.bump_generation(entry.meta.id);
        }
        self.batch_reservations.remove(&batch.nonce);
        self.reserved_bytes -= reserved_bytes;
        self.reserved_slots -= reserved_slots;
        Ok(outcomes)
    }

    fn abort_reserved_batch(
        &mut self,
        batch: &StoreBatchReservation,
    ) -> Result<(), StoreAbortError> {
        if batch.provider != self.provider_id {
            return Err(StoreAbortError::Invalid);
        }
        let Some(record) = self.batch_reservations.get(&batch.nonce).cloned() else {
            return Ok(());
        };
        let reserved_bytes = record
            .records
            .iter()
            .try_fold(0u64, |total, item| total.checked_add(item.reserved_bytes));
        let reserved_slots = record.records.iter().try_fold(0usize, |total, item| {
            total.checked_add(usize::from(item.reserved_slot))
        });
        let (Some(reserved_bytes), Some(reserved_slots)) = (reserved_bytes, reserved_slots) else {
            return Err(StoreAbortError::Invalid);
        };
        let Some(next_bytes) = self.reserved_bytes.checked_sub(reserved_bytes) else {
            return Err(StoreAbortError::Invalid);
        };
        let Some(next_slots) = self.reserved_slots.checked_sub(reserved_slots) else {
            return Err(StoreAbortError::Invalid);
        };
        self.batch_reservations.remove(&batch.nonce);
        self.reserved_bytes = next_bytes;
        self.reserved_slots = next_slots;
        Ok(())
    }

    fn get_into(
        &self,
        id: &Id16,
        meta_out: &mut BundleMeta,
        bytes_out: &mut [u8],
    ) -> Result<usize, StoreReadError> {
        let Some((meta, bytes)) = self.map.get(id) else {
            return Err(StoreReadError::NotFound);
        };
        if bytes.len() > bytes_out.len() {
            return Err(StoreReadError::OutputTooSmall {
                required: bytes.len(),
            });
        }
        bytes_out[..bytes.len()].copy_from_slice(bytes);
        *meta_out = *meta;
        Ok(bytes.len())
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
            self.bump_generation(meta.id);
        }
    }

    fn remove(&mut self, id: &Id16) -> bool {
        match self.map.remove(id) {
            Some((_, b)) => {
                self.bytes -= b.len() as u64;
                self.bump_generation(*id);
                true
            }
            None => false,
        }
    }

    fn ids_range(&self, lo: &Id16, hi: &Id16) -> Vec<Id16> {
        self.map.range(*lo..=*hi).map(|(id, _)| *id).collect()
    }

    fn ids_range_into(
        &self,
        lo: &Id16,
        hi: &Id16,
        out: &mut [Id16],
    ) -> Result<usize, StoreReadError> {
        let required = self.map.range(*lo..=*hi).count();
        if required > out.len() {
            return Err(StoreReadError::OutputTooSmall { required });
        }
        for (slot, (id, _)) in out.iter_mut().zip(self.map.range(*lo..=*hi)) {
            *slot = *id;
        }
        Ok(required)
    }

    fn all_meta(&self) -> Vec<BundleMeta> {
        self.map.values().map(|(m, _)| *m).collect()
    }

    fn for_each_meta(&self, visitor: &mut dyn FnMut(BundleMeta)) {
        for (meta, _) in self.map.values() {
            visitor(*meta);
        }
    }

    fn all_meta_into(&self, out: &mut [BundleMeta]) -> Result<usize, StoreReadError> {
        let required = self.map.len();
        if required > out.len() {
            return Err(StoreReadError::OutputTooSmall { required });
        }
        for (slot, (meta, _)) in out.iter_mut().zip(self.map.values()) {
            *slot = *meta;
        }
        Ok(required)
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

    fn expire_into(&mut self, now_ms: u64, out: &mut [Id16]) -> Result<usize, StoreReadError> {
        let required = self
            .map
            .values()
            .filter(|(meta, _)| meta.expires_at <= now_ms)
            .count();
        if required > out.len() {
            return Err(StoreReadError::OutputTooSmall { required });
        }
        let mut count = 0;
        for (id, (meta, _)) in &self.map {
            if meta.expires_at <= now_ms {
                out[count] = *id;
                count += 1;
            }
        }
        for id in &out[..count] {
            self.remove(id);
        }
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use super::{
        BundleMeta, BundleStore, MemStore, SlotState, StoreBatchEntry, StoreBatchError,
        StoreBatchReservation, StoreCommitError, StorePutOutcome, StoreReadError, StoreReservation,
        StoreReserveError,
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
    fn bounded_reads_are_object_safe_and_fail_without_mutation() {
        let mut store = MemStore::new();
        store.put(meta(1, 3), &[7, 8, 9]);
        let mut expiring = meta(2, 1);
        expiring.expires_at = 5;
        store.put(expiring, &[4]);

        let mut output_meta = meta(99, 0);
        let mut bytes = [0u8; 3];
        let trait_store: &mut dyn BundleStore = &mut store;
        assert_eq!(
            trait_store.get_into(&Id16([1; 16]), &mut output_meta, &mut bytes),
            Ok(3)
        );
        assert_eq!(output_meta.id, Id16([1; 16]));
        assert_eq!(bytes, [7, 8, 9]);

        let before_meta = output_meta;
        let before_bytes = bytes;
        let mut too_small = [0u8; 2];
        assert_eq!(
            trait_store.get_into(&Id16([1; 16]), &mut output_meta, &mut too_small),
            Err(StoreReadError::OutputTooSmall { required: 3 })
        );
        assert_eq!(output_meta, before_meta);
        assert_eq!(too_small, [0, 0]);
        assert_eq!(bytes, before_bytes);
        assert_eq!(
            trait_store.get_into(&Id16([8; 16]), &mut output_meta, &mut bytes),
            Err(StoreReadError::NotFound)
        );

        let mut ids = [Id16([0; 16]); 1];
        assert_eq!(
            trait_store.ids_range_into(&Id16([1; 16]), &Id16([2; 16]), &mut ids),
            Err(StoreReadError::OutputTooSmall { required: 2 })
        );
        assert_eq!(ids, [Id16([0; 16]); 1]);

        let mut metas = [meta(99, 0); 1];
        assert_eq!(
            trait_store.all_meta_into(&mut metas),
            Err(StoreReadError::OutputTooSmall { required: 2 })
        );
        assert_eq!(metas[0].id, Id16([99; 16]));

        let mut expired = [Id16([0; 16]); 1];
        assert_eq!(trait_store.expire_into(5, &mut expired), Ok(1));
        assert_eq!(expired[0], Id16([2; 16]));
        assert_eq!(trait_store.meta(&Id16([2; 16])), None);
    }

    #[test]
    fn for_each_meta_visits_every_record_in_provider_order() {
        let mut store = MemStore::new();
        store.put(meta(3, 0), &[]);
        store.put(meta(1, 0), &[]);
        store.put(meta(2, 0), &[]);

        let mut ids = Vec::new();
        let trait_store: &dyn BundleStore = &store;
        trait_store.for_each_meta(&mut |record| ids.push(record.id));
        assert_eq!(ids, vec![Id16([1; 16]), Id16([2; 16]), Id16([3; 16])]);
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

    #[test]
    fn reservation_accounts_exact_capacity_and_replacement_delta() {
        let mut store = MemStore::with_limits(4, 2);
        assert_eq!(
            store.try_put(meta(1, 2), &[9, 9]),
            Ok(StorePutOutcome::Inserted)
        );

        let replacement = store.try_reserve(meta(1, 4)).unwrap();
        assert_eq!(store.reserved_bytes(), 2);
        assert_eq!(store.reserved_slots(), 0);
        assert_eq!(
            store.try_reserve(meta(2, 1)),
            Err(StoreReserveError::Capacity)
        );
        assert_eq!(
            store.commit_reserved(&replacement, meta(1, 4), &[1, 2, 3, 4]),
            Ok(StorePutOutcome::Replaced)
        );
        assert_eq!(store.total_bytes(), 4);
        assert_eq!(store.reserved_bytes(), 0);
    }

    #[test]
    fn bounded_try_put_rejects_over_capacity_without_mutation() {
        let mut store = MemStore::with_limits(2, 1);
        assert_eq!(
            store.try_put(meta(15, 3), &[1, 2, 3]),
            Err(super::StorePutError::Capacity)
        );
        assert!(store.is_empty());
        assert_eq!(store.total_bytes(), 0);

        assert_eq!(
            store.try_put(meta(15, 2), &[1, 2]),
            Ok(StorePutOutcome::Inserted)
        );
        assert_eq!(
            store.try_put(meta(16, 1), &[3]),
            Err(super::StorePutError::Capacity)
        );
        assert_eq!(store.len(), 1);
        assert_eq!(store.total_bytes(), 2);
        assert_eq!(store.get(&Id16([15; 16])).map(|(_, b)| b), Some(vec![1, 2]));

        assert_eq!(
            store.try_put(meta(15, 3), &[4, 5, 6]),
            Err(super::StorePutError::Capacity)
        );
        assert_eq!(store.total_bytes(), 2);
        assert_eq!(store.get(&Id16([15; 16])).map(|(_, b)| b), Some(vec![1, 2]));
    }

    #[test]
    fn reservation_rejects_duplicate_stale_wrong_id_and_length_without_mutation() {
        let mut store = MemStore::with_limits(8, 2);
        let token = store.try_reserve(meta(3, 2)).unwrap();
        assert_eq!(
            store.try_reserve(meta(3, 2)),
            Err(StoreReserveError::Invalid)
        );
        assert_eq!(
            store.commit_reserved(&token, meta(4, 2), &[1, 2]),
            Err(StoreCommitError::Invalid)
        );
        assert_eq!(
            store.commit_reserved(&token, meta(3, 3), &[1, 2, 3]),
            Err(StoreCommitError::Invalid)
        );
        assert_eq!(
            store.commit_reserved(&token, meta(3, 2), &[1]),
            Err(StoreCommitError::Invalid)
        );
        assert!(store.is_empty());
        assert_eq!(store.total_bytes(), 0);
        assert_eq!(store.reserved_bytes(), 2);
    }

    #[test]
    fn successful_commit_consumes_reservation_exactly_once() {
        let mut store = MemStore::with_limits(8, 1);
        let token = store.try_reserve(meta(5, 2)).unwrap();
        assert_eq!(
            store.commit_reserved(&token, meta(5, 2), &[1, 2]),
            Ok(StorePutOutcome::Inserted)
        );
        assert_eq!(
            store.commit_reserved(&token, meta(5, 2), &[1, 2]),
            Err(StoreCommitError::Invalid)
        );
        assert_eq!(store.total_bytes(), 2);
        assert_eq!(store.reserved_bytes(), 0);
    }

    #[test]
    fn abort_is_idempotent_and_releases_capacity_without_deleting_entries() {
        let mut store = MemStore::with_limits(8, 2);
        assert_eq!(
            store.try_put(meta(6, 1), &[8]),
            Ok(StorePutOutcome::Inserted)
        );
        let token = store.try_reserve(meta(7, 2)).unwrap();
        assert_eq!(store.abort_reservation(&token), Ok(()));
        assert_eq!(store.abort_reservation(&token), Ok(()));
        assert_eq!(store.reserved_bytes(), 0);
        assert_eq!(store.len(), 1);
        assert_eq!(store.get(&Id16([6; 16])).map(|(_, b)| b), Some(vec![8]));
    }

    #[test]
    fn changed_replacement_leaves_failed_commit_and_reservation_unchanged() {
        let mut store = MemStore::with_limits(8, 1);
        assert_eq!(
            store.try_put(meta(8, 2), &[7, 7]),
            Ok(StorePutOutcome::Inserted)
        );
        let token = store.try_reserve(meta(8, 4)).unwrap();
        store.put(meta(8, 3), &[6, 6, 6]);
        assert_eq!(
            store.commit_reserved(&token, meta(8, 4), &[1, 2, 3, 4]),
            Err(StoreCommitError::Conflict)
        );
        assert_eq!(
            store.get(&Id16([8; 16])).map(|(_, b)| b),
            Some(vec![6, 6, 6])
        );
        assert_eq!(store.reserved_bytes(), 2);
    }

    #[test]
    fn same_length_replacement_is_detected_before_commit() {
        let mut store = MemStore::with_limits(8, 1);
        assert_eq!(
            store.try_put(meta(10, 2), &[7, 7]),
            Ok(StorePutOutcome::Inserted)
        );
        let token = store.try_reserve(meta(10, 4)).unwrap();
        store.put(meta(10, 2), &[6, 6]);
        assert_eq!(
            store.commit_reserved(&token, meta(10, 4), &[1, 2, 3, 4]),
            Err(StoreCommitError::Conflict)
        );
        assert_eq!(store.get(&Id16([10; 16])).map(|(_, b)| b), Some(vec![6, 6]));
        assert_eq!(store.reserved_bytes(), 2);
    }

    #[test]
    fn batch_reservation_preflights_duplicates_and_nonce_exhaustion_without_prefix() {
        let mut store = MemStore::with_limits(8, 3);
        let duplicate = [meta(11, 1), meta(11, 1)];
        assert_eq!(
            store.try_reserve_batch(&duplicate),
            Err(StoreReserveError::Invalid)
        );
        assert_eq!(store.reserved_bytes(), 0);
        assert_eq!(store.reserved_slots(), 0);

        store.next_nonce = u64::MAX;
        let request = [meta(12, 1), meta(13, 1)];
        assert_eq!(
            store.try_reserve_batch(&request),
            Err(StoreReserveError::Capacity)
        );
        assert_eq!(store.reserved_bytes(), 0);
        assert_eq!(store.reserved_slots(), 0);
        assert!(store.reservations.is_empty());
    }

    #[test]
    fn provider_local_namespace_rejects_a_token_from_another_store() {
        let mut first = MemStore::with_limits(4, 1);
        let token = first.try_reserve(meta(14, 1)).unwrap();
        let mut second = MemStore::with_limits(4, 1);
        assert_eq!(
            second.commit_reserved(&token, meta(14, 1), &[1]),
            Err(StoreCommitError::Invalid)
        );
        assert!(second.is_empty());
        assert_eq!(second.reserved_bytes(), 0);
    }

    #[test]
    fn provider_factories_are_constructible_but_namespace_validation_remains_required() {
        let mut store = MemStore::with_limits(4, 1);
        let valid = store.try_reserve(meta(17, 1)).unwrap();

        let wrong_provider = StoreReservation::from_parts(
            valid.provider.wrapping_add(1),
            valid.id,
            valid.declared_size,
            valid.nonce,
        );
        assert_eq!(
            store.commit_reserved(&wrong_provider, meta(17, 1), &[1]),
            Err(StoreCommitError::Invalid)
        );

        let wrong_batch = StoreBatchReservation::from_parts(valid.provider, valid.nonce);
        assert_eq!(
            store.commit_reserved_batch(&wrong_batch, &[StoreBatchEntry::new(meta(17, 1), &[1])],),
            Err(StoreCommitError::Invalid)
        );
        assert_eq!(store.reserved_bytes(), 1);
        assert!(store.is_empty());
    }

    #[test]
    fn atomic_batch_reservation_rejects_duplicates_and_nonce_exhaustion_without_prefix() {
        let mut store = MemStore::with_limits(8, 3);
        assert_eq!(
            store.try_reserve_batch_atomic(&[meta(20, 1), meta(20, 1)]),
            Err(StoreReserveError::Invalid)
        );
        assert_eq!(store.reserved_bytes(), 0);
        assert_eq!(store.reserved_slots(), 0);

        store.next_nonce = u64::MAX;
        assert_eq!(
            store.try_reserve_batch_atomic(&[meta(21, 1), meta(22, 1)]),
            Err(StoreReserveError::Capacity)
        );
        assert_eq!(store.reserved_bytes(), 0);
        assert_eq!(store.reserved_slots(), 0);
        assert!(store.reservations.is_empty());
    }

    #[test]
    fn atomic_batch_commit_publishes_all_entries_and_consumes_the_token() {
        let mut store = MemStore::with_limits(8, 3);
        let metas = [meta(23, 2), meta(24, 2)];
        let batch = store.try_reserve_batch_atomic(&metas).unwrap();
        let entries = [
            StoreBatchEntry::new(metas[0], b"ab"),
            StoreBatchEntry::new(metas[1], b"cd"),
        ];

        assert_eq!(
            store.commit_reserved_batch(&batch, &entries),
            Ok(vec![StorePutOutcome::Inserted, StorePutOutcome::Inserted])
        );
        assert_eq!(store.get(&metas[0].id).unwrap().1, b"ab");
        assert_eq!(store.get(&metas[1].id).unwrap().1, b"cd");
        assert_eq!(store.reserved_bytes(), 0);
        assert_eq!(
            store.commit_reserved_batch(&batch, &entries),
            Err(StoreCommitError::Invalid)
        );
    }

    #[test]
    fn checked_writes_cannot_bypass_an_outstanding_atomic_batch_reservation() {
        let mut store = MemStore::with_limits(2, 1);
        let reserved = meta(35, 2);
        let batch = store.try_reserve_batch_atomic(&[reserved]).unwrap();
        let other = meta(36, 2);

        assert_eq!(
            store.try_put(other, b"cd"),
            Err(super::StorePutError::Capacity)
        );
        assert_eq!(
            store.try_put_batch(&[StoreBatchEntry::new(other, b"cd")]),
            Err(StoreBatchError::Invalid)
        );
        store
            .commit_reserved_batch(&batch, &[StoreBatchEntry::new(reserved, b"ab")])
            .unwrap();
        assert_eq!(store.get(&reserved.id).unwrap().1, b"ab");
        assert!(store.get(&other.id).is_none());
    }

    #[test]
    fn failed_atomic_batch_commit_retains_reservation_for_retry() {
        let mut store = MemStore::with_limits(8, 2);
        let metas = [meta(25, 2), meta(26, 2)];
        let batch = store.try_reserve_batch_atomic(&metas).unwrap();
        let malformed = [
            StoreBatchEntry::new(metas[0], b"ab"),
            StoreBatchEntry::new(metas[1], b"c"),
        ];
        assert_eq!(
            store.commit_reserved_batch(&batch, &malformed),
            Err(StoreCommitError::Invalid)
        );
        assert!(store.is_empty());
        assert_eq!(store.reserved_bytes(), 4);

        let entries = [
            StoreBatchEntry::new(metas[0], b"ab"),
            StoreBatchEntry::new(metas[1], b"cd"),
        ];
        assert_eq!(
            store.commit_reserved_batch(&batch, &entries).unwrap().len(),
            2
        );
    }

    #[test]
    fn atomic_batch_abort_is_idempotent_and_cannot_delete_committed_records() {
        let mut store = MemStore::with_limits(8, 2);
        let metas = [meta(27, 2)];
        let batch = store.try_reserve_batch_atomic(&metas).unwrap();
        let entries = [StoreBatchEntry::new(metas[0], b"ab")];
        store.commit_reserved_batch(&batch, &entries).unwrap();
        assert_eq!(store.abort_reserved_batch(&batch), Ok(()));
        assert_eq!(store.abort_reserved_batch(&batch), Ok(()));
        assert_eq!(store.get(&metas[0].id).unwrap().1, b"ab");
    }

    #[test]
    fn atomic_batch_detects_replacement_conflict_before_any_publication() {
        let mut store = MemStore::with_limits(8, 2);
        store.try_put(meta(28, 3), b"old").unwrap();
        let metas = [meta(28, 3), meta(29, 2)];
        let batch = store.try_reserve_batch_atomic(&metas).unwrap();
        store.put(meta(28, 3), b"new");
        let entries = [
            StoreBatchEntry::new(metas[0], b"aaa"),
            StoreBatchEntry::new(metas[1], b"bb"),
        ];
        assert_eq!(
            store.commit_reserved_batch(&batch, &entries),
            Err(StoreCommitError::Conflict)
        );
        assert_eq!(store.get(&metas[0].id).unwrap().1, b"new");
        assert!(store.get(&metas[1].id).is_none());
        assert_eq!(store.reserved_bytes(), 2);
    }

    #[test]
    fn atomic_batch_rejects_an_identical_replacement_after_remove_and_reinsert() {
        let mut store = MemStore::with_limits(8, 1);
        let existing = meta(34, 2);
        store.try_put(existing, b"ok").unwrap();
        let batch = store.try_reserve_batch_atomic(&[existing]).unwrap();
        assert!(store.remove(&existing.id));
        store.put(existing, b"ok");
        let entries = [StoreBatchEntry::new(existing, b"ok")];
        assert_eq!(
            store.commit_reserved_batch(&batch, &entries),
            Err(StoreCommitError::Conflict)
        );
        assert_eq!(store.get(&existing.id).unwrap().1, b"ok");
    }

    #[test]
    fn atomic_batch_tokens_are_rejected_by_other_providers() {
        let mut first = MemStore::with_limits(4, 1);
        let batch = first.try_reserve_batch_atomic(&[meta(30, 1)]).unwrap();
        let mut second = MemStore::with_limits(4, 1);
        let entries = [StoreBatchEntry::new(meta(30, 1), b"a")];
        assert_eq!(
            second.commit_reserved_batch(&batch, &entries),
            Err(StoreCommitError::Invalid)
        );
        assert_eq!(
            second.abort_reserved_batch(&batch),
            Err(super::StoreAbortError::Invalid)
        );
        assert!(second.is_empty());
    }
}
