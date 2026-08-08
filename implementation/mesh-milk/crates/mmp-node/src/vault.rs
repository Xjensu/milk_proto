//! Rust-internal boundary for a future platform-keystore snapshot provider.
//!
//! The provider sees only an authenticated envelope, its non-secret context,
//! and an opaque/wrapped data-encryption key. It must never receive the
//! plaintext aggregate. This module does not implement a platform backend;
//! it makes the contract testable before Android/iOS/Linux adapters exist.

#![allow(dead_code)]

use std::collections::HashMap;

use mmp_crypto::conversation_id;
use mmp_crypto::snapshot::{self, KIND_NODE_SECRET_STATE};
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

/// Failure in context construction, sealed-record validation, or provider IO.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VaultError {
    /// Input was malformed, inconsistent, oversized, stale, or unauthenticated.
    Invalid,
    /// A candidate generation was not the next durable generation.
    Generation,
    /// The provider cannot currently supply or commit the required capability.
    Unavailable,
}

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

/// Native provider contract. Implementations must make `commit` a durable
/// compare-and-swap: either the complete candidate record becomes visible, or
/// the prior record remains intact. The provider must persist no plaintext.
pub(crate) trait SnapshotVault {
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
    if ttl_ms != snapshot::DEFAULT_TTL_MS {
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
/// the provider. Every failed step returns before any caller-side restore.
pub(crate) fn restore_from_vault(
    vault: &mut impl SnapshotVault,
    node_public: &Key32,
    node_id: &Id16,
) -> Result<(u64, RestoredState), VaultError> {
    let generation_floor = vault.generation_floor()?;
    let record = vault.load()?.ok_or(VaultError::Unavailable)?;
    let dek = vault.unwrap_dek(&record.context, &record.wrapped_dek)?;
    let now_ms = vault.trusted_now_ms()?;
    open_record(
        node_public,
        node_id,
        &record,
        &dek,
        now_ms,
        snapshot::DEFAULT_TTL_MS,
        generation_floor,
    )
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
mod tests {
    use super::*;
    use mmp_core::MemStore;
    use rand::{rngs::StdRng, SeedableRng};
    use crate::{Node, Tier};

    struct ReferenceSnapshotVault {
        available: bool,
        high_water: u64,
        trusted_now_ms: u64,
        record: Option<SealedRecord>,
    }

    impl ReferenceSnapshotVault {
        /// This is a test-only provider. Its wrapped DEK is deliberately raw
        /// test material; no production or FFI path may use this type.
        fn new() -> Self {
            Self { available: true, high_water: 0, trusted_now_ms: 100_000, record: None }
        }

        fn set_available(&mut self, available: bool) {
            self.available = available;
        }

        fn high_water_generation(&self) -> u64 {
            self.high_water
        }

        fn simulate_record_loss(&mut self) {
            self.record = None;
        }
    }

    impl SnapshotVault for ReferenceSnapshotVault {
        fn load(&mut self) -> Result<Option<SealedRecord>, VaultError> {
            if !self.available {
                return Err(VaultError::Unavailable);
            }
            Ok(self.record.clone())
        }

        fn generation_floor(&mut self) -> Result<u64, VaultError> {
            if !self.available { return Err(VaultError::Unavailable); }
            Ok(self.high_water)
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

    fn introduce(a: &mut Node, b: &mut Node) {
        for claim in a.own_claims() { b.ingest_claim(claim, true); }
        for claim in b.own_claims() { a.ingest_claim(claim, true); }
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
        let (generation, state) = open_record(&node.core.public, &node.core.node_id, &record, &[7; 32], 10_001, snapshot::DEFAULT_TTL_MS, 1).unwrap();
        assert_eq!(generation, 1);
        assert_eq!(state.convs.len(), 1);

        let mut wrong = record.clone();
        wrong.context[9] ^= 1;
        assert!(open_record(&node.core.public, &node.core.node_id, &wrong, &[7; 32], 10_001, snapshot::DEFAULT_TTL_MS, 1).is_err());
        assert!(snapshot::validate_next_generation(generation, 2).is_ok());
        assert!(snapshot::validate_next_generation(generation, 1).is_err());
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
            snapshot::DEFAULT_TTL_MS,
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
