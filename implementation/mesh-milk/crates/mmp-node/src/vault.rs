//! Rust-internal boundary for a future platform-keystore snapshot provider.
//!
//! The provider sees only an authenticated envelope, its non-secret context,
//! and an opaque/wrapped data-encryption key. It must never receive the
//! plaintext aggregate. The Windows target has a cfg-isolated DPAPI adapter;
//! other platforms continue to use the provider-neutral test seam until their
//! native keystore contracts are implemented.

#![allow(dead_code)]

#[cfg(target_os = "windows")]
#[path = "vault_windows.rs"]
pub(crate) mod vault_windows;

#[cfg(target_os = "windows")]
pub(crate) use vault_windows::WindowsDpapiVault;

use std::collections::HashMap;

use mmp_crypto::conversation_id;
use mmp_crypto::snapshot::{self, KIND_NODE_SECRET_STATE};
use mmp_types::consts::LIFETIME_MESSAGE_MS;
use mmp_types::ids::{Id16, Key32};
use rand_core::CryptoRngCore;
use zeroize::Zeroizing;

use crate::secret_state::{self, RestoredState};
use crate::NodeCore;

const CONTEXT_MAGIC: [u8; 8] = *b"MMPCTX01";
const CONTEXT_VERSION: u8 = 1;
const CONTEXT_COVERAGE: [u8; 8] = *b"PAIRPK01";
const CONTEXT_HEADER_LEN: usize = CONTEXT_MAGIC.len() + 1 + CONTEXT_COVERAGE.len() + 32 + 16 + 2;
const CONTEXT_ENTRY_LEN: usize = 16 + 32;
const MAX_CONTEXT_ENTRIES: usize = 256;
const MAX_CONTEXT_BYTES: usize = CONTEXT_HEADER_LEN + MAX_CONTEXT_ENTRIES * CONTEXT_ENTRY_LEN;
/// Freshness bound for provider-local ratchet/prekey snapshots (doc 08 §5.3).
/// Keep this policy local to the vault boundary rather than inheriting the
/// generic envelope default implicitly.
const RATCHET_PERSIST_TTL_MS: u64 = LIFETIME_MESSAGE_MS;

/// Failure in context construction, sealed-record validation, or provider IO.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VaultError {
    /// Input was malformed, inconsistent, oversized, stale, or unauthenticated.
    Invalid,
    /// A candidate generation was not the next durable generation.
    Generation,
    /// The provider cannot currently supply or commit the required capability.
    Unavailable,
    /// Restore was attempted after live sessions or secret runtime state existed.
    Busy,
}

/// A restore operation that runs while the provider holds its identity-scoped
/// restore transaction.  Native providers must keep the transaction held for
/// the entire callback, including the caller's in-memory state swap; a
/// best-effort/no-op implementation would not satisfy the rollback invariant.
pub(crate) type RestoreOperation<'a> =
    dyn FnMut(&mut dyn SnapshotVault) -> Result<u64, VaultError> + 'a;

/// Ciphertext and provider-owned opaque key material for one node generation.
///
/// `context` is metadata needed to recompute the AEAD associated-data hash; it
/// contains identity/node binding and the set of conversation/peer pairs, but
/// no ratchet or prekey secret. `wrapped_dek` is intentionally opaque to this
/// module and must be produced by the platform provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SealedRecord {
    pub(crate) generation: u64,
    pub(crate) context: Vec<u8>,
    pub(crate) envelope: Vec<u8>,
    pub(crate) wrapped_dek: Vec<u8>,
}

/// A freshly sealed candidate before the provider wraps its DEK and commits
/// the complete record. This type intentionally contains no provider secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SealedCandidate {
    pub(crate) generation: u64,
    pub(crate) context: Vec<u8>,
    pub(crate) envelope: Vec<u8>,
}

/// Reject provider records whose Rust-owned components exceed their exact
/// resource contract. This runs before generation checks, context parsing, or
/// native key unwrap, so a malformed record cannot force expensive provider
/// work merely by claiming a valid generation. The wrapped-DEK handle is
/// intentionally not bounded here: its encoding belongs to the native
/// provider, which must enforce its own size/IPC limit before unwrapping.
fn validate_record_bounds(record: &SealedRecord) -> Result<(), VaultError> {
    if record.context.len() > MAX_CONTEXT_BYTES || record.envelope.len() > snapshot::MAX_ENVELOPE_BYTES {
        return Err(VaultError::Invalid);
    }
    Ok(())
}

/// Native provider contract. Implementations must make `commit` a durable
/// compare-and-swap: either the complete candidate record becomes visible, or
/// the prior record remains intact. The provider must persist no plaintext.
pub(crate) trait SnapshotVault {
    /// Run one restore operation while excluding commits from every process
    /// that can mutate this identity's durable record.  The provider must
    /// acquire an exclusive transaction/lease before invoking `operation` and
    /// release it only after the callback returns.  Implementations must not
    /// provide a default no-op: without a real provider lock, a concurrent
    /// commit can invalidate the state immediately after the final generation
    /// check and before the node installs it.
    fn with_restore_lock(&mut self, operation: &mut RestoreOperation<'_>) -> Result<u64, VaultError>;

    /// Load the last committed opaque record, if one exists.
    fn load(&mut self) -> Result<Option<SealedRecord>, VaultError>;

    /// Return the durable generation floor, including a floor retained after
    /// a record is lost or a crash occurs between journal records.
    fn generation_floor(&mut self) -> Result<u64, VaultError>;

    /// Return a trusted platform time in milliseconds. Process-local clocks
    /// and caller-provided wall time are not acceptable implementations.
    fn trusted_now_ms(&mut self) -> Result<u64, VaultError>;

    /// Atomically commit `candidate` only when its generation follows
    /// `expected_generation` (zero means no prior record).
    fn commit(&mut self, expected_generation: u64, candidate: SealedCandidate, dek: &[u8; 32]) -> Result<(), VaultError>;

    /// Unwrap a provider-owned DEK. The returned key is zeroizing and must
    /// stay inside Rust's seal/open path.
    fn unwrap_dek(&mut self, context: &[u8], wrapped_dek: &[u8]) -> Result<Zeroizing<[u8; 32]>, VaultError>;
}

/// Build the canonical non-secret context for a node's current pairwise set.
pub(crate) fn context_for_core(core: &NodeCore) -> Result<Vec<u8>, VaultError> {
    if core.convs.len() > MAX_CONTEXT_ENTRIES {
        return Err(VaultError::Invalid);
    }
    let mut entries: Vec<(Id16, Key32)> = core.convs.iter().map(|(id, conv)| (*id, conv.peer)).collect();
    entries.sort_unstable_by_key(|(id, _)| *id);
    let mut context = Vec::with_capacity(CONTEXT_HEADER_LEN + entries.len() * CONTEXT_ENTRY_LEN);
    context.extend_from_slice(&CONTEXT_MAGIC);
    context.push(CONTEXT_VERSION);
    context.extend_from_slice(&CONTEXT_COVERAGE);
    context.extend_from_slice(&core.public.0);
    context.extend_from_slice(&core.node_id.0);
    context.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    for (id, peer) in entries {
        context.extend_from_slice(&id.0);
        context.extend_from_slice(&peer.0);
    }
    Ok(context)
}

/// Seal the typed aggregate under a caller-owned fresh DEK.
pub(crate) fn seal_core(
    core: &NodeCore,
    generation: u64,
    written_at_ms: u64,
    dek: &[u8; 32],
    rng: &mut impl CryptoRngCore,
) -> Result<SealedCandidate, VaultError> {
    let context = context_for_core(core)?;
    let plaintext = secret_state::encode(core).map_err(|_| VaultError::Invalid)?;
    let envelope = snapshot::seal(
        KIND_NODE_SECRET_STATE,
        generation,
        written_at_ms,
        &context,
        dek,
        rng,
        &plaintext,
    )
    .map_err(|_| VaultError::Invalid)?;
    Ok(SealedCandidate { generation, context, envelope })
}

/// Check the CAS condition that every provider commit must enforce durably.
pub(crate) fn validate_commit_generation(expected_generation: u64, candidate: &SealedCandidate) -> Result<(), VaultError> {
    let next_generation = expected_generation.checked_add(1).ok_or(VaultError::Generation)?;
    if candidate.generation != next_generation {
        return Err(VaultError::Generation);
    }
    snapshot::validate_next_generation(expected_generation, candidate.generation).map_err(|_| VaultError::Generation)
}

/// Seal and submit one complete candidate through the provider's atomic
/// commit operation. The DEK is generated and zeroized inside this function;
/// callers never receive it and the provider receives no plaintext aggregate.
pub(crate) fn save_core(
    vault: &mut impl SnapshotVault,
    core: &NodeCore,
    expected_generation: u64,
    written_at_ms: u64,
    rng: &mut impl CryptoRngCore,
) -> Result<u64, VaultError> {
    let mut dek = Zeroizing::new([0u8; 32]);
    rng.fill_bytes(&mut *dek);
    let candidate = seal_core(core, expected_generation.checked_add(1).ok_or(VaultError::Generation)?, written_at_ms, &dek, rng)?;
    validate_commit_generation(expected_generation, &candidate)?;
    let generation = candidate.generation;
    vault.commit(expected_generation, candidate, &dek)?;
    Ok(generation)
}

/// Seal using the provider's durable generation floor and trusted clock. This
/// is the only intended save entry point for a native adapter; `save_core`
/// remains a lower-level test seam with explicit inputs.
pub(crate) fn save_core_from_vault(
    vault: &mut impl SnapshotVault,
    core: &NodeCore,
    rng: &mut impl CryptoRngCore,
) -> Result<u64, VaultError> {
    let expected_generation = vault.generation_floor()?;
    let written_at_ms = vault.trusted_now_ms()?;
    save_core(vault, core, expected_generation, written_at_ms, rng)
}

/// Open and validate a provider-loaded record for the target node.
pub(crate) fn open_record(
    node_public: &Key32,
    node_id: &Id16,
    record: &SealedRecord,
    dek: &[u8; 32],
    now_ms: u64,
    ttl_ms: u64,
    generation_floor: u64,
) -> Result<(u64, RestoredState), VaultError> {
    if ttl_ms != RATCHET_PERSIST_TTL_MS {
        return Err(VaultError::Invalid);
    }
    if record.generation == 0 || record.generation == u64::MAX {
        return Err(VaultError::Generation);
    }
    let entries = parse_context(&record.context, node_public, node_id)?;
    let (generation, plaintext) = snapshot::open(
        KIND_NODE_SECRET_STATE,
        &record.context,
        dek,
        &record.envelope,
        now_ms,
        ttl_ms,
    )
    .map_err(|_| VaultError::Invalid)?;
    if generation != record.generation || generation < generation_floor {
        return Err(VaultError::Generation);
    }
    let state = secret_state::decode(&plaintext).map_err(|_| VaultError::Invalid)?;
    if state.convs.len() != entries.len()
        || state.convs.iter().any(|(id, conv)| entries.get(id) != Some(&conv.peer))
        || state.convs.iter().any(|(id, conv)| conversation_id(node_public, &conv.peer) != *id)
    {
        return Err(VaultError::Invalid);
    }
    Ok((generation, state))
}

/// Load, unwrap, and validate one generation without exposing plaintext to
/// the provider. The caller must already hold the provider restore lock.
fn restore_record_unlocked(
    vault: &mut dyn SnapshotVault,
    node_public: &Key32,
    node_id: &Id16,
    record: SealedRecord,
) -> Result<(u64, RestoredState), VaultError> {
    // Bound all provider-controlled byte vectors before consulting the durable
    // floor or asking a native keystore to unwrap an opaque key handle.
    validate_record_bounds(&record)?;
    // Read the floor after loading the record. A different process may commit
    // a newer generation between provider calls; checking the latest floor
    // fences the stale record instead of accepting it under an older floor.
    let generation_floor = vault.generation_floor()?;
    // The floor is a durable high-water mark, not merely a best-effort hint.
    // A provider that returns a record newer than its reported floor is
    // internally inconsistent and could otherwise make rollback-unsafe state
    // reach the native unwrap path. Fail closed before asking the provider for
    // any opaque key material.
    if record.generation > generation_floor {
        return Err(VaultError::Generation);
    }
    // Validate the bounded identity/node/conversation context before asking a
    // provider to unwrap any key material.  The provider receives this context
    // as an authenticated lookup input; malformed or cross-node records must
    // not reach a native keystore boundary even if the provider would return a
    // key for the opaque handle.
    parse_context(&record.context, node_public, node_id)?;
    let dek = vault.unwrap_dek(&record.context, &record.wrapped_dek)?;
    let now_ms = vault.trusted_now_ms()?;
    let (generation, state) = open_record(
        node_public,
        node_id,
        &record,
        &dek,
        now_ms,
        RATCHET_PERSIST_TTL_MS,
        generation_floor,
    )?;
    // The provider floor is a monotonic durability fence, not a snapshot of
    // the record. A concurrent process may commit a newer generation after
    // the first floor read but before decryption completes; re-read it after
    // the expensive open so stale ratchet/prekey state cannot be accepted.
    if generation < vault.generation_floor()? {
        return Err(VaultError::Generation);
    }
    Ok((generation, state))
}

/// Restore one generation while holding the provider's transaction through
/// the caller's state installation.  This is the production-facing helper;
/// the lower-level decoder above intentionally does not claim linearizability
/// when called outside the lock.
pub(crate) fn restore_from_vault_into(
    vault: &mut impl SnapshotVault,
    node_public: &Key32,
    node_id: &Id16,
    apply: &mut dyn FnMut(RestoredState) -> Result<(), VaultError>,
) -> Result<u64, VaultError> {
    let mut operation = |locked: &mut dyn SnapshotVault| {
        let record = locked.load()?.ok_or(VaultError::Unavailable)?;
        let (generation, state) = restore_record_unlocked(locked, node_public, node_id, record)?;
        apply(state)?;
        Ok(generation)
    };
    vault.with_restore_lock(&mut operation)
}

/// Restore one generation while holding the provider transaction, treating a
/// brand-new provider with no committed record as an absent optional
/// capability.  The record load and state swap occur under the same lock, so
/// startup cannot miss a concurrent commit or accept a record that changed
/// between a preflight probe and the actual restore.
pub(crate) fn restore_from_vault_if_present_into(
    vault: &mut impl SnapshotVault,
    node_public: &Key32,
    node_id: &Id16,
    apply: &mut dyn FnMut(RestoredState) -> Result<(), VaultError>,
) -> Result<Option<u64>, VaultError> {
    let mut operation = |locked: &mut dyn SnapshotVault| {
        let Some(record) = locked.load()? else {
            // A missing record is optional only for a genuinely fresh
            // provider.  Once a durable floor exists, treating loss as
            // `Ok(None)` would silently convert record loss into a rollback
            // bypass and allow a caller to continue with empty secret state.
            if locked.generation_floor()? != 0 {
                return Err(VaultError::Generation);
            }
            return Ok(0);
        };
        let (generation, state) = restore_record_unlocked(locked, node_public, node_id, record)?;
        apply(state)?;
        Ok(generation)
    };
    let generation = vault.with_restore_lock(&mut operation)?;
    Ok((generation != 0).then_some(generation))
}

/// Load, unwrap, and validate one generation through a provider restore
/// transaction. This helper is retained for Rust-internal tests and callers
/// that need the decoded state; production node installation uses
/// [`restore_from_vault_into`] so the in-memory swap remains inside the lock.
pub(crate) fn restore_from_vault(
    vault: &mut impl SnapshotVault,
    node_public: &Key32,
    node_id: &Id16,
) -> Result<(u64, RestoredState), VaultError> {
    let mut restored = None;
    let mut apply = |state| {
        restored = Some(state);
        Ok(())
    };
    let generation = restore_from_vault_into(vault, node_public, node_id, &mut apply)?;
    let state = restored.ok_or(VaultError::Invalid)?;
    Ok((generation, state))
}

fn parse_context(context: &[u8], public: &Key32, node_id: &Id16) -> Result<HashMap<Id16, Key32>, VaultError> {
    if context.len() < CONTEXT_HEADER_LEN || context.len() > MAX_CONTEXT_BYTES {
        return Err(VaultError::Invalid);
    }
    if context[..CONTEXT_MAGIC.len()] != CONTEXT_MAGIC || context[CONTEXT_MAGIC.len()] != CONTEXT_VERSION {
        return Err(VaultError::Invalid);
    }
    let mut pos = CONTEXT_MAGIC.len() + 1;
    if context.get(pos..pos + CONTEXT_COVERAGE.len()) != Some(&CONTEXT_COVERAGE) {
        return Err(VaultError::Invalid);
    }
    pos += CONTEXT_COVERAGE.len();
    if context.get(pos..pos + 32) != Some(&public.0) {
        return Err(VaultError::Invalid);
    }
    pos += 32;
    if context.get(pos..pos + 16) != Some(&node_id.0) {
        return Err(VaultError::Invalid);
    }
    pos += 16;
    let count = u16::from_le_bytes(context.get(pos..pos + 2).ok_or(VaultError::Invalid)?.try_into().map_err(|_| VaultError::Invalid)?) as usize;
    pos += 2;
    if count > MAX_CONTEXT_ENTRIES || context.len() != CONTEXT_HEADER_LEN + count * CONTEXT_ENTRY_LEN {
        return Err(VaultError::Invalid);
    }
    let mut entries = HashMap::with_capacity(count);
    let mut previous = None;
    for _ in 0..count {
        let id = Id16(context.get(pos..pos + 16).ok_or(VaultError::Invalid)?.try_into().map_err(|_| VaultError::Invalid)?);
        pos += 16;
        let peer = Key32(context.get(pos..pos + 32).ok_or(VaultError::Invalid)?.try_into().map_err(|_| VaultError::Invalid)?);
        pos += 32;
        if previous.is_some_and(|old| old >= id) || entries.insert(id, peer).is_some() {
            return Err(VaultError::Invalid);
        }
        previous = Some(id);
    }
    Ok(entries)
}

#[cfg(test)]
#[path = "vault_freshness.rs"]
mod freshness_model;

#[cfg(test)]
mod tests {
    use super::*;
    use mmp_core::MemStore;
    use rand::{rngs::StdRng, SeedableRng};
    use std::sync::{atomic::{AtomicBool, Ordering}, Arc};
    use crate::{Node, Tier};

    struct ReferenceSnapshotVault {
        available: bool,
        high_water: u64,
        reported_floor: Option<u64>,
        trusted_now_ms: u64,
        record: Option<SealedRecord>,
        unwrap_calls: usize,
        restore_lock_calls: usize,
    }

    impl ReferenceSnapshotVault {
        /// This is a test-only provider. Its wrapped DEK is deliberately raw
        /// test material; no production or FFI path may use this type.
        fn new() -> Self {
            Self {
                available: true,
                high_water: 0,
                reported_floor: None,
                trusted_now_ms: 100_000,
                record: None,
                unwrap_calls: 0,
                restore_lock_calls: 0,
            }
        }

        fn set_available(&mut self, available: bool) {
            self.available = available;
        }

        fn high_water_generation(&self) -> u64 {
            self.high_water
        }

        fn set_reported_floor(&mut self, floor: Option<u64>) {
            self.reported_floor = floor;
        }

        fn simulate_record_loss(&mut self) {
            self.record = None;
        }
    }

    impl SnapshotVault for ReferenceSnapshotVault {
        fn with_restore_lock(&mut self, operation: &mut RestoreOperation<'_>) -> Result<u64, VaultError> {
            // This in-memory fixture has no external process.  Count the
            // callback so tests still prove the production path cannot bypass
            // the required provider transaction seam.
            self.restore_lock_calls += 1;
            operation(self)
        }

        fn load(&mut self) -> Result<Option<SealedRecord>, VaultError> {
            if !self.available {
                return Err(VaultError::Unavailable);
            }
            Ok(self.record.clone())
        }

        fn generation_floor(&mut self) -> Result<u64, VaultError> {
            if !self.available { return Err(VaultError::Unavailable); }
            Ok(self.reported_floor.unwrap_or(self.high_water))
        }

        fn trusted_now_ms(&mut self) -> Result<u64, VaultError> {
            if !self.available { return Err(VaultError::Unavailable); }
            Ok(self.trusted_now_ms)
        }

        fn commit(&mut self, expected_generation: u64, candidate: SealedCandidate, dek: &[u8; 32]) -> Result<(), VaultError> {
            if !self.available {
                return Err(VaultError::Unavailable);
            }
            if self.high_water != expected_generation
                || self.record.as_ref().map_or(0, |record| record.generation) != expected_generation
            {
                return Err(VaultError::Generation);
            }
            validate_commit_generation(expected_generation, &candidate)?;
            if candidate.generation != expected_generation.checked_add(1).ok_or(VaultError::Generation)? {
                return Err(VaultError::Generation);
            }
            let envelope_generation = envelope_generation(&candidate.envelope)?;
            if envelope_generation != candidate.generation || candidate.context.is_empty() {
                return Err(VaultError::Invalid);
            }

            // The assignment is the single commit point: all validation above
            // happens before either high-water or record state changes.
            let record = SealedRecord {
                generation: candidate.generation,
                context: candidate.context,
                envelope: candidate.envelope,
                wrapped_dek: dek.to_vec(),
            };
            self.high_water = candidate.generation;
            self.record = Some(record);
            Ok(())
        }

        fn unwrap_dek(&mut self, _context: &[u8], wrapped_dek: &[u8]) -> Result<Zeroizing<[u8; 32]>, VaultError> {
            if !self.available {
                return Err(VaultError::Unavailable);
            }
            self.unwrap_calls += 1;
            let key: [u8; 32] = wrapped_dek.try_into().map_err(|_| VaultError::Invalid)?;
            Ok(Zeroizing::new(key))
        }
    }

    /// Deterministic race fixture: a provider can observe a durable floor
    /// advance immediately after returning a record (for example, another
    /// process committed a newer generation).  The restore path must read the
    /// floor after loading the record so that this stale record is rejected.
    struct FloorAdvancingAfterLoadVault {
        inner: ReferenceSnapshotVault,
        advance_after_load: bool,
    }

    impl SnapshotVault for FloorAdvancingAfterLoadVault {
        fn with_restore_lock(&mut self, operation: &mut RestoreOperation<'_>) -> Result<u64, VaultError> {
            self.inner.restore_lock_calls += 1;
            operation(self)
        }

        fn load(&mut self) -> Result<Option<SealedRecord>, VaultError> {
            let record = self.inner.load()?;
            if self.advance_after_load {
                self.inner.high_water = self.inner.high_water.checked_add(1).ok_or(VaultError::Generation)?;
                self.advance_after_load = false;
            }
            Ok(record)
        }

        fn generation_floor(&mut self) -> Result<u64, VaultError> {
            self.inner.generation_floor()
        }

        fn trusted_now_ms(&mut self) -> Result<u64, VaultError> {
            self.inner.trusted_now_ms()
        }

        fn commit(&mut self, expected_generation: u64, candidate: SealedCandidate, dek: &[u8; 32]) -> Result<(), VaultError> {
            self.inner.commit(expected_generation, candidate, dek)
        }

        fn unwrap_dek(&mut self, context: &[u8], wrapped_dek: &[u8]) -> Result<Zeroizing<[u8; 32]>, VaultError> {
            self.inner.unwrap_dek(context, wrapped_dek)
        }
    }

    /// Deterministic race fixture for the final restore fence: another
    /// process commits a newer generation after the initial floor read but
    /// before the snapshot is opened.
    struct FloorAdvancingAfterClockVault {
        inner: ReferenceSnapshotVault,
        advance_after_clock: bool,
    }

    impl SnapshotVault for FloorAdvancingAfterClockVault {
        fn with_restore_lock(&mut self, operation: &mut RestoreOperation<'_>) -> Result<u64, VaultError> {
            self.inner.restore_lock_calls += 1;
            operation(self)
        }

        fn load(&mut self) -> Result<Option<SealedRecord>, VaultError> {
            self.inner.load()
        }

        fn generation_floor(&mut self) -> Result<u64, VaultError> {
            self.inner.generation_floor()
        }

        fn trusted_now_ms(&mut self) -> Result<u64, VaultError> {
            let now = self.inner.trusted_now_ms()?;
            if self.advance_after_clock {
                self.inner.high_water = self.inner.high_water.checked_add(1).ok_or(VaultError::Generation)?;
                self.advance_after_clock = false;
            }
            Ok(now)
        }

        fn commit(&mut self, expected_generation: u64, candidate: SealedCandidate, dek: &[u8; 32]) -> Result<(), VaultError> {
            self.inner.commit(expected_generation, candidate, dek)
        }

        fn unwrap_dek(&mut self, context: &[u8], wrapped_dek: &[u8]) -> Result<Zeroizing<[u8; 32]>, VaultError> {
            self.inner.unwrap_dek(context, wrapped_dek)
        }
    }

    /// Test adapter proving that the high-level helper keeps the provider
    /// transaction active while the caller installs the decoded state.  A
    /// native implementation must replace this boolean with an exclusive
    /// cross-process transaction/lease.
    struct LockObservingVault {
        inner: ReferenceSnapshotVault,
        active: Arc<AtomicBool>,
    }

    impl SnapshotVault for LockObservingVault {
        fn with_restore_lock(&mut self, operation: &mut RestoreOperation<'_>) -> Result<u64, VaultError> {
            assert!(!self.active.swap(true, Ordering::AcqRel), "restore lock must not be nested");
            let result = operation(self);
            self.active.store(false, Ordering::Release);
            result
        }

        fn load(&mut self) -> Result<Option<SealedRecord>, VaultError> {
            self.inner.load()
        }

        fn generation_floor(&mut self) -> Result<u64, VaultError> {
            self.inner.generation_floor()
        }

        fn trusted_now_ms(&mut self) -> Result<u64, VaultError> {
            self.inner.trusted_now_ms()
        }

        fn commit(&mut self, expected_generation: u64, candidate: SealedCandidate, dek: &[u8; 32]) -> Result<(), VaultError> {
            self.inner.commit(expected_generation, candidate, dek)
        }

        fn unwrap_dek(&mut self, context: &[u8], wrapped_dek: &[u8]) -> Result<Zeroizing<[u8; 32]>, VaultError> {
            self.inner.unwrap_dek(context, wrapped_dek)
        }
    }

    /// Fault points in the platform-neutral journal model. This is a test
    /// fixture only: it models the durable ordering a native adapter must
    /// implement without pretending to provide a keystore backend here.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum CrashPoint {
        Write,
        Fsync,
        Replace,
        Floor,
    }

    #[derive(Default)]
    struct JournalImage {
        current: Option<SealedRecord>,
        previous: Option<SealedRecord>,
        synced_temp: Option<SealedRecord>,
        floor: u64,
    }

    struct JournaledSnapshotVault {
        image: JournalImage,
        volatile_temp: Option<SealedRecord>,
        fault: Option<CrashPoint>,
        node_public: Key32,
        node_id: Id16,
        trusted_now_ms: u64,
    }

    impl JournaledSnapshotVault {
        fn new(core: &NodeCore) -> Self {
            Self {
                image: JournalImage::default(),
                volatile_temp: None,
                fault: None,
                node_public: core.public,
                node_id: core.node_id,
                trusted_now_ms: 100_000,
            }
        }

        fn inject_fault(&mut self, point: CrashPoint) {
            self.fault = Some(point);
        }

        fn reopen(self) -> Self {
            let mut reopened = Self {
                image: self.image,
                volatile_temp: None,
                fault: None,
                node_public: self.node_public,
                node_id: self.node_id,
                trusted_now_ms: self.trusted_now_ms,
            };
            reopened.recover();
            reopened
        }

        fn record_is_complete(&self, record: &SealedRecord) -> bool {
            let Ok(dek) = <[u8; 32]>::try_from(record.wrapped_dek.as_slice()) else {
                return false;
            };
            open_record(
                &self.node_public,
                &self.node_id,
                record,
                &dek,
                self.trusted_now_ms,
                RATCHET_PERSIST_TTL_MS,
                0,
            )
            .is_ok()
        }

        fn recover(&mut self) {
            let current = self.image.current.take();
            let previous = self.image.previous.take();
            let synced_temp = self.image.synced_temp.take();
            let floor = self.image.floor;

            // A complete current record wins. If replacement was interrupted
            // or the current record is corrupt, a fully fsynced next record
            // may be promoted; otherwise retain the previous complete record.
            let selected = current
                .filter(|record| record.generation >= floor && self.record_is_complete(record))
                .or_else(|| {
                    synced_temp
                        .filter(|record| record.generation >= floor && self.record_is_complete(record))
                })
                .or_else(|| {
                    previous
                        .filter(|record| record.generation >= floor && self.record_is_complete(record))
                });
            self.image.current = selected;
            if let Some(record) = self.image.current.as_ref() {
                self.image.floor = self.image.floor.max(record.generation);
            }
            // Volatile writes and any unselected temporary record are dropped
            // on reopen; the durable floor is never lowered.
            self.volatile_temp = None;
            self.image.synced_temp = None;
        }
    }

    impl SnapshotVault for JournaledSnapshotVault {
        fn with_restore_lock(&mut self, operation: &mut RestoreOperation<'_>) -> Result<u64, VaultError> {
            // The journal model is single-process and therefore cannot model
            // a native cross-process lock; retain the explicit callback seam
            // so production adapters cannot accidentally omit it.
            operation(self)
        }

        fn load(&mut self) -> Result<Option<SealedRecord>, VaultError> {
            Ok(self.image.current.clone())
        }

        fn generation_floor(&mut self) -> Result<u64, VaultError> {
            Ok(self.image.floor)
        }

        fn trusted_now_ms(&mut self) -> Result<u64, VaultError> {
            Ok(self.trusted_now_ms)
        }

        fn commit(&mut self, expected_generation: u64, candidate: SealedCandidate, dek: &[u8; 32]) -> Result<(), VaultError> {
            if self.image.floor != expected_generation
                || self.image.current.as_ref().map_or(0, |record| record.generation) != expected_generation
            {
                return Err(VaultError::Generation);
            }
            validate_commit_generation(expected_generation, &candidate)?;
            if envelope_generation(&candidate.envelope)? != candidate.generation || candidate.context.is_empty() {
                return Err(VaultError::Invalid);
            }
            let record = SealedRecord {
                generation: candidate.generation,
                context: candidate.context,
                envelope: candidate.envelope,
                wrapped_dek: dek.to_vec(),
            };
            let fault = self.fault.take();

            // Write to a volatile temporary slot first. A crash here leaves
            // the durable image unchanged.
            self.volatile_temp = Some(record.clone());
            if fault == Some(CrashPoint::Write) {
                return Err(VaultError::Unavailable);
            }

            // Fsync makes the temporary candidate durable, but it is not yet
            // visible as the current generation.
            self.image.synced_temp = self.volatile_temp.take();
            if fault == Some(CrashPoint::Fsync) {
                return Err(VaultError::Unavailable);
            }

            // Replace is modeled as one durable slot swap. Keep the previous
            // complete generation so recovery can fail back if needed.
            let next = self.image.synced_temp.take().ok_or(VaultError::Invalid)?;
            self.image.previous = self.image.current.replace(next);
            if fault == Some(CrashPoint::Replace) {
                return Err(VaultError::Unavailable);
            }

            // The floor advances only after a complete current record exists.
            self.image.floor = record.generation;
            if fault == Some(CrashPoint::Floor) {
                return Err(VaultError::Unavailable);
            }
            Ok(())
        }

        fn unwrap_dek(&mut self, _context: &[u8], wrapped_dek: &[u8]) -> Result<Zeroizing<[u8; 32]>, VaultError> {
            let key: [u8; 32] = wrapped_dek.try_into().map_err(|_| VaultError::Invalid)?;
            Ok(Zeroizing::new(key))
        }
    }

    fn envelope_generation(envelope: &[u8]) -> Result<u64, VaultError> {
        if envelope.len() < 14 || envelope[..4] != *b"MKS1" || envelope[4] != 1 || envelope[5] != KIND_NODE_SECRET_STATE {
            return Err(VaultError::Invalid);
        }
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&envelope[6..14]);
        Ok(u64::from_le_bytes(bytes))
    }

    #[test]
    fn journal_recovery_selects_previous_or_next_complete_generation() {
        let node = Node::new([107; 32], Box::new(MemStore::new()), Tier::Relay);
        let points = [
            CrashPoint::Write,
            CrashPoint::Fsync,
            CrashPoint::Replace,
            CrashPoint::Floor,
        ];

        for (index, point) in points.into_iter().enumerate() {
            let mut vault = JournaledSnapshotVault::new(&node.core);
            let mut rng = StdRng::seed_from_u64(20 + index as u64);
            assert_eq!(save_core(&mut vault, &node.core, 0, 10_000, &mut rng).unwrap(), 1);

            vault.inject_fault(point);
            assert_eq!(save_core(&mut vault, &node.core, 1, 20_000, &mut rng), Err(VaultError::Unavailable));

            let mut recovered = vault.reopen();
            let (generation, _) = restore_from_vault(&mut recovered, &node.core.public, &node.core.node_id).unwrap();
            let expected = match point {
                CrashPoint::Write | CrashPoint::Fsync => 1,
                CrashPoint::Replace | CrashPoint::Floor => 2,
            };
            assert_eq!(generation, expected, "recovery result for {point:?}");
            assert_eq!(recovered.generation_floor().unwrap(), expected);
            assert_eq!(recovered.load().unwrap().unwrap().generation, expected);
        }
    }

    #[test]
    fn journal_recovery_rejects_corrupt_current_without_lowering_floor() {
        let node = Node::new([108; 32], Box::new(MemStore::new()), Tier::Relay);
        let mut vault = JournaledSnapshotVault::new(&node.core);
        let mut rng = StdRng::seed_from_u64(24);
        assert_eq!(save_core(&mut vault, &node.core, 0, 30_000, &mut rng).unwrap(), 1);

        vault.inject_fault(CrashPoint::Replace);
        assert_eq!(save_core(&mut vault, &node.core, 1, 40_000, &mut rng), Err(VaultError::Unavailable));
        vault.image.current.as_mut().unwrap().envelope[0] ^= 1;

        let mut recovered = vault.reopen();
        let (generation, _) = restore_from_vault(&mut recovered, &node.core.public, &node.core.node_id).unwrap();
        assert_eq!(generation, 1, "corrupt next record falls back to the previous complete one");
        assert_eq!(recovered.generation_floor().unwrap(), 1);

        recovered.image.floor = 2;
        let mut rollback_unsafe = recovered.reopen();
        assert_eq!(rollback_unsafe.generation_floor().unwrap(), 2);
        assert!(matches!(
            restore_from_vault(&mut rollback_unsafe, &node.core.public, &node.core.node_id),
            Err(VaultError::Unavailable)
        ));
        assert_eq!(rollback_unsafe.generation_floor().unwrap(), 2);
    }

    #[test]
    fn restore_fences_record_when_floor_advances_after_load() {
        let node = Node::new([109; 32], Box::new(MemStore::new()), Tier::Relay);
        let mut inner = ReferenceSnapshotVault::new();
        let mut rng = StdRng::seed_from_u64(25);
        assert_eq!(save_core(&mut inner, &node.core, 0, 35_000, &mut rng).unwrap(), 1);

        let mut vault = FloorAdvancingAfterLoadVault { inner, advance_after_load: true };
        assert!(matches!(
            restore_from_vault(&mut vault, &node.core.public, &node.core.node_id),
            Err(VaultError::Generation)
        ));
        assert_eq!(vault.generation_floor().unwrap(), 2);
    }

    #[test]
    fn restore_fences_record_when_floor_advances_during_open() {
        let node = Node::new([108; 32], Box::new(MemStore::new()), Tier::Relay);
        let mut inner = ReferenceSnapshotVault::new();
        let mut rng = StdRng::seed_from_u64(27);
        assert_eq!(save_core(&mut inner, &node.core, 0, 35_000, &mut rng).unwrap(), 1);

        let mut vault = FloorAdvancingAfterClockVault { inner, advance_after_clock: true };
        assert!(matches!(
            restore_from_vault(&mut vault, &node.core.public, &node.core.node_id),
            Err(VaultError::Generation)
        ));
        assert_eq!(vault.generation_floor().unwrap(), 2);
    }

    #[test]
    fn restore_rejects_invalid_context_before_provider_unwrap() {
        let node = Node::new([117; 32], Box::new(MemStore::new()), Tier::Relay);
        let mut vault = ReferenceSnapshotVault::new();
        let mut rng = StdRng::seed_from_u64(28);
        assert_eq!(save_core(&mut vault, &node.core, 0, 35_000, &mut rng).unwrap(), 1);

        let mut record = vault.record.clone().expect("sealed fixture");
        record.context[0] ^= 1;
        vault.record = Some(record);

        assert!(matches!(
            restore_from_vault(&mut vault, &node.core.public, &node.core.node_id),
            Err(VaultError::Invalid)
        ));
        assert_eq!(vault.unwrap_calls, 0, "invalid context must not reach provider key unwrap");
    }

    #[test]
    fn restore_rejects_record_newer_than_provider_floor_before_unwrap() {
        let node = Node::new([118; 32], Box::new(MemStore::new()), Tier::Relay);
        let mut vault = ReferenceSnapshotVault::new();
        let mut rng = StdRng::seed_from_u64(29);
        assert_eq!(save_core_from_vault(&mut vault, &node.core, &mut rng).unwrap(), 1);
        vault.set_reported_floor(Some(0));

        assert!(matches!(
            restore_from_vault(&mut vault, &node.core.public, &node.core.node_id),
            Err(VaultError::Generation)
        ));
        assert_eq!(vault.unwrap_calls, 0, "an inconsistent floor must not reach provider key unwrap");
    }

    fn introduce(a: &mut Node, b: &mut Node) {
        for claim in a.own_claims() { b.ingest_claim(claim, true); }
        for claim in b.own_claims() { a.ingest_claim(claim, true); }
    }

    fn contact(a: &mut Node, b: &mut Node, link: u64, now: u64) {
        a.tick(now);
        b.tick(now);
        a.on_tal_event(mmp_tal::TalEvent::LinkUp {
            link,
            initiator: true,
            quality: Default::default(),
        });
        b.on_tal_event(mmp_tal::TalEvent::LinkUp {
            link,
            initiator: false,
            quality: Default::default(),
        });
        for _ in 0..300 {
            let mut quiet = true;
            while let Some((l, bytes)) = a.poll_send() {
                quiet = false;
                b.on_tal_event(mmp_tal::TalEvent::LinkData { link: l, bytes });
            }
            while let Some((l, bytes)) = b.poll_send() {
                quiet = false;
                a.on_tal_event(mmp_tal::TalEvent::LinkData { link: l, bytes });
            }
            if quiet {
                break;
            }
        }
        a.on_tal_event(mmp_tal::TalEvent::LinkDown { link });
        b.on_tal_event(mmp_tal::TalEvent::LinkDown { link });
    }

    #[test]
    fn sealed_record_binds_identity_conversations_and_generation() {
        let mut node = Node::new([91; 32], Box::new(MemStore::new()), Tier::Relay);
        let mut peer = Node::new([92; 32], Box::new(MemStore::new()), Tier::Relay);
        introduce(&mut node, &mut peer);
        node.send_text(&peer.node_id(), "vault").unwrap();
        let mut rng = StdRng::seed_from_u64(12);
        let candidate = seal_core(&node.core, 1, 10_000, &[7; 32], &mut rng).unwrap();
        assert!(validate_commit_generation(0, &candidate).is_ok());
        let record = SealedRecord {
            generation: candidate.generation,
            context: candidate.context,
            envelope: candidate.envelope,
            wrapped_dek: vec![1, 2, 3],
        };
        let (generation, state) = open_record(&node.core.public, &node.core.node_id, &record, &[7; 32], 10_001, RATCHET_PERSIST_TTL_MS, 1).unwrap();
        assert_eq!(generation, 1);
        assert_eq!(state.convs.len(), 1);

        let mut wrong = record.clone();
        wrong.context[9] ^= 1;
        assert!(open_record(&node.core.public, &node.core.node_id, &wrong, &[7; 32], 10_001, RATCHET_PERSIST_TTL_MS, 1).is_err());
        assert!(snapshot::validate_next_generation(generation, 2).is_ok());
        assert!(snapshot::validate_next_generation(generation, 1).is_err());
    }

    #[test]
    fn high_level_restore_rejects_cross_node_record_replay_without_mutation() {
        let source = Node::new([117; 32], Box::new(MemStore::new()), Tier::Relay);
        let mut source_vault = ReferenceSnapshotVault::new();
        let mut rng = StdRng::seed_from_u64(117);
        assert_eq!(save_core_from_vault(&mut source_vault, &source.core, &mut rng).unwrap(), 1);

        // A valid envelope and DEK are not sufficient for restore: the
        // authenticated context is bound to the source identity and node ID.
        // Model a provider replaying that complete record to a different node.
        let mut replay_vault = ReferenceSnapshotVault::new();
        replay_vault.high_water = source_vault.high_water;
        replay_vault.record = source_vault.record.clone();

        let mut target = Node::new([118; 32], Box::new(MemStore::new()), Tier::Relay);
        let before = target.secret_state_snapshot().unwrap();
        assert_eq!(target.restore_secret_state_from_vault(&mut replay_vault), Err(VaultError::Invalid));
        assert_eq!(target.secret_state_snapshot().unwrap().as_slice(), before.as_slice());
    }

    #[test]
    fn restore_into_keeps_provider_transaction_through_state_application() {
        let source = Node::new([119; 32], Box::new(MemStore::new()), Tier::Relay);
        let mut source_vault = ReferenceSnapshotVault::new();
        let mut rng = StdRng::seed_from_u64(119);
        assert_eq!(save_core_from_vault(&mut source_vault, &source.core, &mut rng).unwrap(), 1);

        let active = Arc::new(AtomicBool::new(false));
        let mut vault = LockObservingVault {
            inner: source_vault,
            active: Arc::clone(&active),
        };
        let mut applied_under_lock = false;
        let active_during_apply = Arc::clone(&active);
        let mut apply = |state: RestoredState| {
            assert!(active_during_apply.load(Ordering::Acquire));
            applied_under_lock = true;
            drop(state);
            Ok(())
        };

        let generation = restore_from_vault_into(
            &mut vault,
            &source.core.public,
            &source.core.node_id,
            &mut apply,
        )
        .unwrap();
        assert_eq!(generation, 1);
        assert!(applied_under_lock);
        assert!(!active.load(Ordering::Acquire));
    }

    #[test]
    fn commit_generation_rejects_gaps_and_overflow() {
        let node = Node::new([101; 32], Box::new(MemStore::new()), Tier::Leaf);
        let mut rng = StdRng::seed_from_u64(18);
        let gap = seal_core(&node.core, 3, 11_000, &[11; 32], &mut rng).unwrap();
        assert_eq!(validate_commit_generation(1, &gap), Err(VaultError::Generation));

        let overflow = seal_core(&node.core, u64::MAX - 1, 11_001, &[11; 32], &mut rng).unwrap();
        assert_eq!(validate_commit_generation(u64::MAX - 1, &overflow), Err(VaultError::Generation));
    }

    #[test]
    fn context_rejects_unsorted_or_duplicate_entries() {
        let node = Node::new([93; 32], Box::new(MemStore::new()), Tier::Leaf);
        let mut context = context_for_core(&node.core).unwrap();
        context.extend_from_slice(&[0; CONTEXT_ENTRY_LEN]);
        assert!(parse_context(&context, &node.core.public, &node.core.node_id).is_err());
    }

    #[test]
    fn save_core_couples_generation_sealing_and_provider_commit() {
        let node = Node::new([94; 32], Box::new(MemStore::new()), Tier::Relay);
        let mut vault = ReferenceSnapshotVault::new();
        let mut rng = StdRng::seed_from_u64(13);
        assert_eq!(save_core(&mut vault, &node.core, 0, 20_000, &mut rng).unwrap(), 1);
        let record = vault.load().unwrap().unwrap();
        let dek = vault.unwrap_dek(&record.context, &record.wrapped_dek).unwrap();
        let (generation, state) = open_record(
            &node.core.public,
            &node.core.node_id,
            &record,
            &dek,
            20_001,
            RATCHET_PERSIST_TTL_MS,
            1,
        )
        .unwrap();
        assert_eq!((generation, state.convs.len()), (1, 0));
        assert_eq!(save_core(&mut vault, &node.core, 1, 20_002, &mut rng).unwrap(), 2);

        let mut second = ReferenceSnapshotVault::new();
        assert_eq!(save_core_from_vault(&mut second, &node.core, &mut rng).unwrap(), 1);
        let (generation, state) = restore_from_vault(&mut second, &node.core.public, &node.core.node_id).unwrap();
        assert_eq!((generation, state.convs.len()), (1, 0));
    }

    #[test]
    fn node_vault_restore_is_atomic_and_keeps_secret_surface_internal() {
        let mut original = Node::new([97; 32], Box::new(MemStore::new()), Tier::Relay);
        let mut peer = Node::new([98; 32], Box::new(MemStore::new()), Tier::Relay);
        introduce(&mut original, &mut peer);
        original.send_text(&peer.node_id(), "restart").unwrap();
        let expected = original.secret_state_snapshot().unwrap();

        let mut vault = ReferenceSnapshotVault::new();
        let mut rng = StdRng::seed_from_u64(16);
        assert_eq!(save_core_from_vault(&mut vault, &original.core, &mut rng).unwrap(), 1);

        let mut restored = Node::new([97; 32], Box::new(MemStore::new()), Tier::Relay);
        assert_eq!(restored.restore_secret_state_from_vault(&mut vault).unwrap(), 1);
        assert_eq!(restored.secret_state_snapshot().unwrap().as_slice(), expected.as_slice());

        restored.on_tal_event(mmp_tal::TalEvent::LinkUp {
            link: 11,
            initiator: true,
            quality: Default::default(),
        });
        assert_eq!(restored.restore_secret_state_from_vault(&mut vault), Err(VaultError::Busy));
    }

    #[test]
    fn high_level_vault_unavailable_fails_closed_without_mutation() {
        let original = Node::new([115; 32], Box::new(MemStore::new()), Tier::Relay);
        let mut save_vault = ReferenceSnapshotVault::new();
        save_vault.set_available(false);
        let mut rng = StdRng::seed_from_u64(31);
        assert_eq!(
            save_core_from_vault(&mut save_vault, &original.core, &mut rng),
            Err(VaultError::Unavailable)
        );

        let mut restored = Node::new([115; 32], Box::new(MemStore::new()), Tier::Relay);
        let before = restored.secret_state_snapshot().unwrap();
        assert_eq!(
            restored.restore_secret_state_from_vault(&mut save_vault),
            Err(VaultError::Unavailable)
        );
        assert_eq!(restored.secret_state_snapshot().unwrap().as_slice(), before.as_slice());
    }

    #[test]
    fn hostile_provider_record_lengths_fail_closed_without_mutation() {
        let original = Node::new([116; 32], Box::new(MemStore::new()), Tier::Relay);
        let mut vault = ReferenceSnapshotVault::new();
        let mut rng = StdRng::seed_from_u64(32);
        assert_eq!(save_core_from_vault(&mut vault, &original.core, &mut rng).unwrap(), 1);

        let mut restored = Node::new([116; 32], Box::new(MemStore::new()), Tier::Relay);
        let before = restored.secret_state_snapshot().unwrap();

        // The provider boundary is allowed to return opaque bytes, so a
        // hostile or corrupted record must be rejected by the Rust restore
        // path before any state is adopted.  This deliberately exceeds the
        // authenticated envelope plaintext cap without attempting to decrypt
        // or parse the extra bytes.
        let mut oversized_envelope = vault.record.clone().expect("sealed fixture");
        oversized_envelope.envelope.resize(snapshot::MAX_ENVELOPE_BYTES + 1, 0);
        vault.record = Some(oversized_envelope);
        assert_eq!(
            restored.restore_secret_state_from_vault(&mut vault),
            Err(VaultError::Invalid)
        );
        assert_eq!(restored.secret_state_snapshot().unwrap().as_slice(), before.as_slice());

        // A context is also provider-supplied metadata.  Keep the envelope
        // valid and make only the context exceed the bounded parser surface;
        // restore must fail closed and preserve the target again.
        let mut oversized_context = vault.record.take().expect("oversized fixture");
        let valid = seal_core(&original.core, 1, 100_000, &[7; 32], &mut rng).unwrap();
        oversized_context.context = valid.context;
        oversized_context.envelope = valid.envelope;
        oversized_context.context.resize(MAX_CONTEXT_BYTES + 1, 0);
        vault.record = Some(oversized_context);
        assert_eq!(
            restored.restore_secret_state_from_vault(&mut vault),
            Err(VaultError::Invalid)
        );
        assert_eq!(restored.secret_state_snapshot().unwrap().as_slice(), before.as_slice());
    }

    #[test]
    fn expired_secret_restore_is_rejected_without_mutating_target() {
        let mut original = Node::new([110; 32], Box::new(MemStore::new()), Tier::Relay);
        let mut peer = Node::new([111; 32], Box::new(MemStore::new()), Tier::Relay);
        introduce(&mut original, &mut peer);
        original.send_text(&peer.node_id(), "expires").expect("first send");

        let mut vault = ReferenceSnapshotVault::new();
        let mut rng = StdRng::seed_from_u64(26);
        assert_eq!(save_core_from_vault(&mut vault, &original.core, &mut rng).unwrap(), 1);

        let mut restored = Node::new([110; 32], Box::new(MemStore::new()), Tier::Relay);
        let before = restored.secret_state_snapshot().unwrap();
        vault.trusted_now_ms = 100_000 + RATCHET_PERSIST_TTL_MS + 1;

        assert_eq!(restored.restore_secret_state_from_vault(&mut vault), Err(VaultError::Invalid));
        assert_eq!(restored.secret_state_snapshot().unwrap().as_slice(), before.as_slice());
    }

    #[test]
    fn restored_established_conversation_bumps_epoch_on_first_post_restore_contact() {
        let mut original = Node::new([99; 32], Box::new(MemStore::new()), Tier::Relay);
        let mut peer = Node::new([100; 32], Box::new(MemStore::new()), Tier::Relay);
        introduce(&mut original, &mut peer);

        original.send_text(&peer.node_id(), "before restart").unwrap();
        contact(&mut original, &mut peer, 21, 10_000);
        assert_eq!(original.conv_epoch(&peer.node_id()), Some((0, 0)));
        assert_eq!(peer.conv_epoch(&original.node_id()), Some((0, 0)));

        let mut vault = ReferenceSnapshotVault::new();
        let mut rng = StdRng::seed_from_u64(17);
        assert_eq!(save_core_from_vault(&mut vault, &original.core, &mut rng).unwrap(), 1);

        let mut restored = Node::new([99; 32], Box::new(MemStore::new()), Tier::Relay);
        assert_eq!(restored.restore_secret_state_from_vault(&mut vault).unwrap(), 1);
        // Identity claims are non-secret bootstrap state and are expected to
        // come from the ordinary snapshot/QR path alongside the vault.
        introduce(&mut restored, &mut peer);
        assert_eq!(restored.conv_epoch(&peer.node_id()), Some((0, 0)));

        contact(&mut restored, &mut peer, 22, 20_000);
        assert_eq!(restored.conv_epoch(&peer.node_id()), Some((1, 1)));
        assert_eq!(peer.conv_epoch(&restored.node_id()), Some((1, 1)));
    }

    #[test]
    fn reference_vault_rejects_stale_equal_and_forked_generations() {
        let node = Node::new([95; 32], Box::new(MemStore::new()), Tier::Relay);
        let mut vault = ReferenceSnapshotVault::new();
        let mut rng = StdRng::seed_from_u64(14);
        assert_eq!(save_core(&mut vault, &node.core, 0, 30_000, &mut rng).unwrap(), 1);
        let dek = [8; 32];

        let equal = seal_core(&node.core, 1, 30_002, &dek, &mut rng).unwrap();
        assert_eq!(vault.commit(1, equal, &dek), Err(VaultError::Generation));

        let stale = seal_core(&node.core, 1, 30_001, &dek, &mut rng).unwrap();
        assert_eq!(vault.commit(2, stale, &dek), Err(VaultError::Generation));

        let fork = seal_core(&node.core, 3, 30_003, &dek, &mut rng).unwrap();
        assert_eq!(vault.commit(1, fork, &dek), Err(VaultError::Generation));
        assert_eq!(vault.load().unwrap().unwrap().generation, 1);
        assert_eq!(vault.high_water_generation(), 1);
    }

    #[test]
    fn reference_vault_keeps_high_water_floor_after_record_loss() {
        let node = Node::new([97; 32], Box::new(MemStore::new()), Tier::Relay);
        let mut vault = ReferenceSnapshotVault::new();
        let mut rng = StdRng::seed_from_u64(16);
        assert_eq!(save_core(&mut vault, &node.core, 0, 50_000, &mut rng).unwrap(), 1);
        vault.simulate_record_loss();

        let candidate = seal_core(&node.core, 2, 50_001, &[10; 32], &mut rng).unwrap();
        assert_eq!(vault.commit(1, candidate, &[10; 32]), Err(VaultError::Generation));
        assert_eq!(vault.load().unwrap(), None);
        assert_eq!(vault.high_water_generation(), 1);
    }

    #[test]
    fn optional_restore_rejects_record_loss_after_initialization() {
        let node = Node::new([98; 32], Box::new(MemStore::new()), Tier::Relay);
        let mut vault = ReferenceSnapshotVault::new();
        let mut rng = StdRng::seed_from_u64(17);
        assert_eq!(save_core(&mut vault, &node.core, 0, 50_000, &mut rng).unwrap(), 1);
        vault.simulate_record_loss();

        let mut applied = false;
        let result = restore_from_vault_if_present_into(
            &mut vault,
            &node.core.public,
            &node.core.node_id,
            &mut |_state| {
                applied = true;
                Ok(())
            },
        );

        assert_eq!(result, Err(VaultError::Generation));
        assert!(!applied, "record loss must fail before state application");
        assert_eq!(vault.high_water_generation(), 1);
    }

    #[test]
    fn reference_vault_preserves_record_on_unavailable_or_invalid_commit() {
        let node = Node::new([96; 32], Box::new(MemStore::new()), Tier::Relay);
        let mut vault = ReferenceSnapshotVault::new();
        let mut rng = StdRng::seed_from_u64(15);
        assert_eq!(save_core(&mut vault, &node.core, 0, 40_000, &mut rng).unwrap(), 1);
        let before = vault.load().unwrap();
        let dek = [9; 32];

        vault.set_available(false);
        let unavailable = seal_core(&node.core, 2, 40_001, &dek, &mut rng).unwrap();
        assert_eq!(vault.commit(1, unavailable, &dek), Err(VaultError::Unavailable));
        assert_eq!(vault.load(), Err(VaultError::Unavailable));
        vault.set_available(true);
        assert_eq!(vault.load().unwrap(), before);
        assert_eq!(vault.high_water_generation(), 1);

        let mut invalid = seal_core(&node.core, 2, 40_002, &dek, &mut rng).unwrap();
        invalid.envelope.clear();
        assert_eq!(vault.commit(1, invalid, &dek), Err(VaultError::Invalid));
        assert_eq!(vault.load().unwrap(), before);
        assert_eq!(vault.high_water_generation(), 1);

        let mut mismatched = seal_core(&node.core, 2, 40_003, &dek, &mut rng).unwrap();
        mismatched.envelope[6..14].copy_from_slice(&1u64.to_le_bytes());
        assert_eq!(vault.commit(1, mismatched, &dek), Err(VaultError::Invalid));
        assert_eq!(vault.load().unwrap(), before);
    }
}
