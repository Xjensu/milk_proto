//! Serverless prekeys, X3DH-adapted (doc 08 §4.2).
//!
//! DH set: (eph × SPK), (eph × OTK if present), (sender-static × SPK)
//! → CK₀ = derive_key("mmp1 conv", dh1 ‖ dh2 ‖ dh3 ‖ sender_id ‖ recipient_id).
//! No server coordinates consumption: the recipient accepts ANY unexpired own
//! prekey (replay is scoped by BundleID dedup); suppression/replay-consume
//! degradations are documented in doc 08 §4.2 and bounded by the sealed-box floor.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;

use mmp_types::claim::PrekeyBatch;
use mmp_types::consts::PREKEY_BATCH;
use mmp_types::domain::SigDomain;
use mmp_types::ids::Key32;
use rand_core::CryptoRngCore;
use zeroize::Zeroizing;

use crate::error::CryptoError;
use crate::identity::{verify, Identity};

/// Version of the provider-local prekey snapshot.
///
/// This is not a wire format. The returned plaintext must be authenticated
/// and encrypted by the platform keystore before persistence.
const SNAPSHOT_VERSION: u8 = 1;
const SNAPSHOT_HEADER_LEN: usize = 1 + 32 + 2;
const SNAPSHOT_RECORD_LEN: usize = 2 + 32;
const STATE_SNAPSHOT_VERSION: u8 = 2;
const STATE_SNAPSHOT_HEADER_LEN: usize = 1 + 32 + 8 + 8 + 8 + 2 + 2;

/// Sentinel: no one-time prekey consumed (signed-prekey-only agreement).
pub const PREKEY_NONE: u16 = 0xffff;

/// The recipient side's private prekey material.
pub struct PrekeyStore {
    spk: x25519_dalek::StaticSecret,
    spk_public: Key32,
    one_time: BTreeMap<u16, x25519_dalek::StaticSecret>,
    /// Local lifecycle metadata. The wire batch remains unchanged; consumed
    /// IDs are retained as tombstones so restart logic cannot forget use.
    consumed: BTreeSet<u16>,
    generation: u64,
    spk_not_after_ms: u64,
    otk_not_after_ms: u64,
}

impl PrekeyStore {
    /// Generate a fresh signed-prekey and `count` one-time prekeys, ids
    /// numbered `0..count`.
    pub fn generate(rng: &mut impl CryptoRngCore, count: usize) -> PrekeyStore {
        let spk = x25519_dalek::StaticSecret::random_from_rng(&mut *rng);
        let spk_public = Key32(x25519_dalek::PublicKey::from(&spk).to_bytes());
        let mut one_time = BTreeMap::new();
        for id in 0..count as u16 {
            one_time.insert(id, x25519_dalek::StaticSecret::random_from_rng(&mut *rng));
        }
        PrekeyStore {
            spk,
            spk_public,
            one_time,
            consumed: BTreeSet::new(),
            generation: 1,
            spk_not_after_ms: 0,
            otk_not_after_ms: 0,
        }
    }

    /// The publishable, identity-signed batch claim body (doc 10 §4.4).
    pub fn batch(&self, identity: &Identity) -> PrekeyBatch {
        let prekeys: Vec<Key32> = self
            .one_time
            .values()
            .map(|s| Key32(x25519_dalek::PublicKey::from(s).to_bytes()))
            .collect();
        let mut batch =
            PrekeyBatch { signed_prekey: self.spk_public, spk_sig: mmp_types::ids::Sig64::ZERO, prekeys };
        batch.spk_sig = identity.sign(SigDomain::Prekey, &batch.signed_object());
        batch
    }

    /// Encode the signed-prekey and one-time private keys for a provider-local
    /// keystore payload. This is not MMP wire data and does not itself provide
    /// restart continuity. The batch cap is enforced rather than silently
    /// truncating private material.
    pub fn snapshot(&self) -> Result<Zeroizing<Vec<u8>>, CryptoError> {
        if self.one_time.len() > PREKEY_BATCH || self.one_time.len() > u16::MAX as usize {
            return Err(CryptoError::Snapshot);
        }
        let mut out = Zeroizing::new(Vec::with_capacity(
            SNAPSHOT_HEADER_LEN + self.one_time.len() * SNAPSHOT_RECORD_LEN,
        ));
        out.push(SNAPSHOT_VERSION);
        let spk = Zeroizing::new(self.spk.to_bytes());
        out.extend_from_slice(&*spk);
        out.extend_from_slice(&(self.one_time.len() as u16).to_le_bytes());
        for (id, secret) in &self.one_time {
            out.extend_from_slice(&id.to_le_bytes());
            let secret = Zeroizing::new(secret.to_bytes());
            out.extend_from_slice(&*secret);
        }
        Ok(out)
    }

    /// Restore private prekey material from a provider-local keystore
    /// payload. Version, exact length, batch cap, sorted unique ids and full
    /// input consumption are required before any map entry is allocated.
    /// The input is assumed to have been authenticated by the keystore layer.
    pub fn restore(bytes: &[u8]) -> Result<PrekeyStore, CryptoError> {
        if bytes.len() < SNAPSHOT_HEADER_LEN || bytes.first().copied() != Some(SNAPSHOT_VERSION) {
            return Err(CryptoError::Snapshot);
        }
        let count = u16::from_le_bytes([bytes[33], bytes[34]]) as usize;
        if count > PREKEY_BATCH {
            return Err(CryptoError::Snapshot);
        }
        let records_len = count.checked_mul(SNAPSHOT_RECORD_LEN).ok_or(CryptoError::Snapshot)?;
        let expected_len = SNAPSHOT_HEADER_LEN.checked_add(records_len).ok_or(CryptoError::Snapshot)?;
        if bytes.len() != expected_len {
            return Err(CryptoError::Snapshot);
        }

        let mut spk_bytes = Zeroizing::new([0u8; 32]);
        spk_bytes.copy_from_slice(&bytes[1..33]);
        let spk = x25519_dalek::StaticSecret::from(*spk_bytes);
        let spk_public = Key32(x25519_dalek::PublicKey::from(&spk).to_bytes());
        let mut one_time = BTreeMap::new();
        let mut previous = None;
        let mut pos = SNAPSHOT_HEADER_LEN;
        for expected_id in 0..count {
            let id = u16::from_le_bytes([bytes[pos], bytes[pos + 1]]);
            if id != expected_id as u16 || previous.is_some_and(|old| old >= id) {
                return Err(CryptoError::Snapshot);
            }
            let mut secret_bytes = Zeroizing::new([0u8; 32]);
            secret_bytes.copy_from_slice(&bytes[pos + 2..pos + SNAPSHOT_RECORD_LEN]);
            let secret = x25519_dalek::StaticSecret::from(*secret_bytes);
            one_time.insert(id, secret);
            previous = Some(id);
            pos += SNAPSHOT_RECORD_LEN;
        }
        if pos != bytes.len() {
            return Err(CryptoError::Snapshot);
        }
        Ok(PrekeyStore {
            spk,
            spk_public,
            one_time,
            consumed: BTreeSet::new(),
            generation: 1,
            spk_not_after_ms: 0,
            otk_not_after_ms: 0,
        })
    }

    /// Record successful use of a local one-time prekey without changing the
    /// published wire batch. The tombstone is provider-local state and is
    /// intentionally idempotent for duplicate bundles.
    pub fn mark_otk_consumed(&mut self, id: u16) -> Result<(), CryptoError> {
        if !self.one_time.contains_key(&id) || !self.consumed.insert(id) {
            return Ok(());
        }
        self.generation = self.generation.checked_add(1).ok_or(CryptoError::Snapshot)?;
        Ok(())
    }

    /// True when this generation has recorded successful use of `id`.
    pub fn otk_consumed(&self, id: u16) -> bool {
        self.consumed.contains(&id)
    }

    /// Set local, trusted expiry metadata for the signed and one-time prekey
    /// material. Zero means not provisioned; this does not alter the wire
    /// claim because expiry is not represented by the frozen batch grammar.
    pub fn set_expiry(&mut self, spk_not_after_ms: u64, otk_not_after_ms: u64) -> Result<(), CryptoError> {
        if spk_not_after_ms == u64::MAX || otk_not_after_ms == u64::MAX {
            return Err(CryptoError::Snapshot);
        }
        self.spk_not_after_ms = spk_not_after_ms;
        self.otk_not_after_ms = otk_not_after_ms;
        self.generation = self.generation.checked_add(1).ok_or(CryptoError::Snapshot)?;
        Ok(())
    }

    /// Encode private keys plus local generation/expiry/consumption metadata.
    /// This is provider-local and must be sealed before persistence.
    pub fn snapshot_state(&self) -> Result<Zeroizing<Vec<u8>>, CryptoError> {
        if self.one_time.len() > PREKEY_BATCH
            || self.one_time.len() > u16::MAX as usize
            || self.consumed.len() > PREKEY_BATCH
            || self.generation == 0
            || self.spk_not_after_ms == u64::MAX
            || self.otk_not_after_ms == u64::MAX
        {
            return Err(CryptoError::Snapshot);
        }
        let mut out = Zeroizing::new(Vec::with_capacity(
            STATE_SNAPSHOT_HEADER_LEN + self.one_time.len() * SNAPSHOT_RECORD_LEN + self.consumed.len() * 2,
        ));
        out.push(STATE_SNAPSHOT_VERSION);
        let spk = Zeroizing::new(self.spk.to_bytes());
        out.extend_from_slice(&*spk);
        out.extend_from_slice(&self.generation.to_le_bytes());
        out.extend_from_slice(&self.spk_not_after_ms.to_le_bytes());
        out.extend_from_slice(&self.otk_not_after_ms.to_le_bytes());
        out.extend_from_slice(&(self.one_time.len() as u16).to_le_bytes());
        out.extend_from_slice(&(self.consumed.len() as u16).to_le_bytes());
        for (id, secret) in &self.one_time {
            out.extend_from_slice(&id.to_le_bytes());
            let secret = Zeroizing::new(secret.to_bytes());
            out.extend_from_slice(&*secret);
        }
        for id in &self.consumed {
            out.extend_from_slice(&id.to_le_bytes());
        }
        Ok(out)
    }

    /// Restore private keys and lifecycle metadata from an authenticated
    /// provider-local state snapshot.
    pub fn restore_state(bytes: &[u8]) -> Result<PrekeyStore, CryptoError> {
        if bytes.len() < STATE_SNAPSHOT_HEADER_LEN || bytes.first().copied() != Some(STATE_SNAPSHOT_VERSION) {
            return Err(CryptoError::Snapshot);
        }
        let generation = u64::from_le_bytes(bytes[33..41].try_into().map_err(|_| CryptoError::Snapshot)?);
        let spk_not_after_ms = u64::from_le_bytes(bytes[41..49].try_into().map_err(|_| CryptoError::Snapshot)?);
        let otk_not_after_ms = u64::from_le_bytes(bytes[49..57].try_into().map_err(|_| CryptoError::Snapshot)?);
        let count = u16::from_le_bytes(bytes[57..59].try_into().map_err(|_| CryptoError::Snapshot)?) as usize;
        let consumed_count = u16::from_le_bytes(bytes[59..61].try_into().map_err(|_| CryptoError::Snapshot)?) as usize;
        if generation == 0
            || spk_not_after_ms == u64::MAX
            || otk_not_after_ms == u64::MAX
            || count > PREKEY_BATCH
            || consumed_count > PREKEY_BATCH
        {
            return Err(CryptoError::Snapshot);
        }
        let expected_len = STATE_SNAPSHOT_HEADER_LEN
            .checked_add(count.checked_mul(SNAPSHOT_RECORD_LEN).ok_or(CryptoError::Snapshot)?)
            .and_then(|n| n.checked_add(consumed_count.checked_mul(2)?))
            .ok_or(CryptoError::Snapshot)?;
        if bytes.len() != expected_len {
            return Err(CryptoError::Snapshot);
        }
        let mut spk_bytes = Zeroizing::new([0u8; 32]);
        spk_bytes.copy_from_slice(&bytes[1..33]);
        let spk = x25519_dalek::StaticSecret::from(*spk_bytes);
        let spk_public = Key32(x25519_dalek::PublicKey::from(&spk).to_bytes());
        let mut one_time = BTreeMap::new();
        let mut previous = None;
        let mut pos = STATE_SNAPSHOT_HEADER_LEN;
        for expected_id in 0..count {
            let id = u16::from_le_bytes([bytes[pos], bytes[pos + 1]]);
            if id != expected_id as u16 || previous.is_some_and(|old| old >= id) {
                return Err(CryptoError::Snapshot);
            }
            let mut secret_bytes = Zeroizing::new([0u8; 32]);
            secret_bytes.copy_from_slice(&bytes[pos + 2..pos + SNAPSHOT_RECORD_LEN]);
            one_time.insert(id, x25519_dalek::StaticSecret::from(*secret_bytes));
            previous = Some(id);
            pos += SNAPSHOT_RECORD_LEN;
        }
        let mut consumed = BTreeSet::new();
        let mut previous_consumed = None;
        for _ in 0..consumed_count {
            let id = u16::from_le_bytes([bytes[pos], bytes[pos + 1]]);
            if id as usize >= count || previous_consumed.is_some_and(|old| old >= id) || !consumed.insert(id) {
                return Err(CryptoError::Snapshot);
            }
            previous_consumed = Some(id);
            pos += 2;
        }
        if pos != bytes.len() {
            return Err(CryptoError::Snapshot);
        }
        Ok(PrekeyStore {
            spk,
            spk_public,
            one_time,
            consumed,
            generation,
            spk_not_after_ms,
            otk_not_after_ms,
        })
    }
}

/// Sender-side agreement output.
pub struct SenderAgreement {
    /// The sender's fresh ephemeral X25519 public key, to be sent to the
    /// recipient alongside the message.
    pub eph_pub: Key32,
    /// The one-time prekey id consumed, or [`PREKEY_NONE`] if none was used.
    pub prekey_id: u16,
    /// The derived initial conversation chain key.
    pub ck0: Zeroizing<[u8; 32]>,
}

/// Sender: verify the batch signature, run the DH set. `prekey_index` picks a
/// one-time key from the batch (local marking only — collisions are possible
/// and tolerated by design).
pub fn sender_agree(
    rng: &mut impl CryptoRngCore,
    sender: &Identity,
    recipient_identity: &Key32,
    batch: &PrekeyBatch,
    prekey_index: Option<u16>,
) -> Result<SenderAgreement, CryptoError> {
    verify(SigDomain::Prekey, recipient_identity, &batch.signed_object(), &batch.spk_sig)
        .map_err(|_| CryptoError::Prekey)?;

    let eph = x25519_dalek::StaticSecret::random_from_rng(&mut *rng);
    let eph_pub = Key32(x25519_dalek::PublicKey::from(&eph).to_bytes());
    let spk = x25519_dalek::PublicKey::from(batch.signed_prekey.0);

    let dh1 = eph.diffie_hellman(&spk);
    let (prekey_id, dh2): (u16, Option<x25519_dalek::SharedSecret>) = match prekey_index {
        Some(i) => {
            let otk = batch.prekeys.get(i as usize).ok_or(CryptoError::Prekey)?;
            (i, Some(eph.diffie_hellman(&x25519_dalek::PublicKey::from(otk.0))))
        }
        None => (PREKEY_NONE, None),
    };
    let dh3 = sender.exchange_dh(&batch.signed_prekey);

    let ck0 = derive_ck0(
        dh1.as_bytes(),
        dh2.as_ref().map(|d| d.as_bytes()),
        &dh3,
        &sender.public(),
        recipient_identity,
    );
    Ok(SenderAgreement { eph_pub, prekey_id, ck0 })
}

/// Recipient: mirror the DH set with private prekey material.
pub fn recipient_agree(
    store: &PrekeyStore,
    recipient: &Identity,
    sender_identity: &Key32,
    sender_exchange: &Key32,
    eph: &Key32,
    prekey_id: u16,
) -> Result<Zeroizing<[u8; 32]>, CryptoError> {
    let eph_pk = x25519_dalek::PublicKey::from(eph.0);
    let dh1 = store.spk.diffie_hellman(&eph_pk);
    let dh2 = if prekey_id == PREKEY_NONE {
        None
    } else {
        let otk = store.one_time.get(&prekey_id).ok_or(CryptoError::Prekey)?;
        Some(otk.diffie_hellman(&eph_pk))
    };
    let dh3 = store.spk.diffie_hellman(&x25519_dalek::PublicKey::from(sender_exchange.0));

    Ok(derive_ck0(
        dh1.as_bytes(),
        dh2.as_ref().map(|d| d.as_bytes()),
        dh3.as_bytes(),
        sender_identity,
        &recipient.public(),
    ))
}

fn derive_ck0(
    dh1: &[u8; 32],
    dh2: Option<&[u8; 32]>,
    dh3: &[u8; 32],
    sender_id: &Key32,
    recipient_id: &Key32,
) -> Zeroizing<[u8; 32]> {
    let mut transcript = Zeroizing::new(Vec::with_capacity(32 * 5));
    transcript.extend_from_slice(dh1);
    if let Some(d2) = dh2 {
        transcript.extend_from_slice(d2);
    }
    transcript.extend_from_slice(dh3);
    transcript.extend_from_slice(&sender_id.0);
    transcript.extend_from_slice(&recipient_id.0);
    Zeroizing::new(blake3::derive_key("mmp1 conv", &transcript))
}

/// Sealed-box fallback conversation root (doc 08 §4.1/§4.3 MVP simplification:
/// a proper root is re-derived at first live session; this keys the interim
/// pairwise state). No FS until then — the documented baseline.
pub fn sealed_ck0(dh: &[u8; 32], sender_id: &Key32, recipient_id: &Key32) -> Zeroizing<[u8; 32]> {
    derive_ck0(dh, None, dh, sender_id, recipient_id)
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    use rand::{rngs::StdRng, SeedableRng};

    #[test]
    fn state_snapshot_preserves_consumed_tombstones_and_expiry() {
        let mut rng = StdRng::seed_from_u64(41);
        let mut original = PrekeyStore::generate(&mut rng, 4);
        original.mark_otk_consumed(2).unwrap();
        original.set_expiry(10_000, 20_000).unwrap();
        let snapshot = original.snapshot_state().unwrap();
        let restored = PrekeyStore::restore_state(&snapshot).unwrap();
        assert_eq!(restored.snapshot_state().unwrap().as_slice(), snapshot.as_slice());
        assert!(restored.otk_consumed(2));
        assert!(!restored.otk_consumed(1));
    }

    #[test]
    fn state_snapshot_rejects_unsorted_or_unknown_tombstones() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut original = PrekeyStore::generate(&mut rng, 4);
        original.mark_otk_consumed(0).unwrap();
        original.mark_otk_consumed(1).unwrap();
        let mut snapshot = original.snapshot_state().unwrap().to_vec();
        // Header: version + SPK + generation + two expiries + counts.
        const HEADER: usize = STATE_SNAPSHOT_HEADER_LEN;
        const RECORD: usize = SNAPSHOT_RECORD_LEN;
        let tombstones = HEADER + 4 * RECORD;
        snapshot[tombstones..tombstones + 2].copy_from_slice(&1u16.to_le_bytes());
        snapshot[tombstones + 2..tombstones + 4].copy_from_slice(&0u16.to_le_bytes());
        assert!(PrekeyStore::restore_state(&snapshot).is_err());

        let mut unknown = original.snapshot_state().unwrap().to_vec();
        unknown[tombstones..tombstones + 2].copy_from_slice(&4u16.to_le_bytes());
        assert!(PrekeyStore::restore_state(&unknown).is_err());

        let mut duplicate = original.snapshot_state().unwrap().to_vec();
        duplicate[tombstones + 2..tombstones + 4].copy_from_slice(&0u16.to_le_bytes());
        assert!(PrekeyStore::restore_state(&duplicate).is_err());
    }

    #[test]
    fn state_snapshot_rejects_every_truncation_and_trailing_bytes() {
        let mut rng = StdRng::seed_from_u64(43);
        let mut original = PrekeyStore::generate(&mut rng, 3);
        original.mark_otk_consumed(1).unwrap();
        original.set_expiry(10_000, 20_000).unwrap();
        let snapshot = original.snapshot_state().unwrap();

        for len in 0..snapshot.len() {
            assert!(
                PrekeyStore::restore_state(&snapshot[..len]).is_err(),
                "truncated state at {len} bytes must be rejected"
            );
        }
        for suffix in [0u8, 1, u8::MAX] {
            let mut trailing = snapshot.to_vec();
            trailing.push(suffix);
            assert!(PrekeyStore::restore_state(&trailing).is_err());
        }
    }

    #[test]
    fn state_snapshot_rejects_count_amplification_before_allocation() {
        let mut rng = StdRng::seed_from_u64(44);
        let original = PrekeyStore::generate(&mut rng, 2);
        let snapshot = original.snapshot_state().unwrap();

        let mut key_count = snapshot.to_vec();
        key_count[57..59].copy_from_slice(&u16::MAX.to_le_bytes());
        assert!(PrekeyStore::restore_state(&key_count).is_err());

        let mut tombstone_count = snapshot.to_vec();
        tombstone_count[59..61].copy_from_slice(&u16::MAX.to_le_bytes());
        assert!(PrekeyStore::restore_state(&tombstone_count).is_err());
    }

    #[test]
    fn state_snapshot_rejects_invalid_generation_and_expiry_values() {
        let mut rng = StdRng::seed_from_u64(45);
        let original = PrekeyStore::generate(&mut rng, 2);
        let snapshot = original.snapshot_state().unwrap();

        let mut zero_generation = snapshot.to_vec();
        zero_generation[33..41].copy_from_slice(&0u64.to_le_bytes());
        assert!(PrekeyStore::restore_state(&zero_generation).is_err());

        let mut max_spk_expiry = snapshot.to_vec();
        max_spk_expiry[41..49].copy_from_slice(&u64::MAX.to_le_bytes());
        assert!(PrekeyStore::restore_state(&max_spk_expiry).is_err());

        let mut max_otk_expiry = snapshot.to_vec();
        max_otk_expiry[49..57].copy_from_slice(&u64::MAX.to_le_bytes());
        assert!(PrekeyStore::restore_state(&max_otk_expiry).is_err());

        let mut invalid_live = PrekeyStore::generate(&mut rng, 1);
        invalid_live.generation = 0;
        assert!(invalid_live.snapshot_state().is_err());
        invalid_live.generation = 1;
        invalid_live.spk_not_after_ms = u64::MAX;
        assert!(invalid_live.snapshot_state().is_err());
    }
}
