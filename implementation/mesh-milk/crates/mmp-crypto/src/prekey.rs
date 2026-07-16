//! Serverless prekeys, X3DH-adapted (doc 08 §4.2).
//!
//! DH set: (eph × SPK), (eph × OTK if present), (sender-static × SPK)
//! → CK₀ = derive_key("mmp1 conv", dh1 ‖ dh2 ‖ dh3 ‖ sender_id ‖ recipient_id).
//! No server coordinates consumption: the recipient accepts ANY unexpired own
//! prekey (replay is scoped by BundleID dedup); suppression/replay-consume
//! degradations are documented in doc 08 §4.2 and bounded by the sealed-box floor.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use mmp_types::claim::PrekeyBatch;
use mmp_types::domain::SigDomain;
use mmp_types::ids::Key32;
use rand_core::CryptoRngCore;
use zeroize::Zeroizing;

use crate::error::CryptoError;
use crate::identity::{verify, Identity};

/// Sentinel: no one-time prekey consumed (signed-prekey-only agreement).
pub const PREKEY_NONE: u16 = 0xffff;

/// The recipient side's private prekey material.
pub struct PrekeyStore {
    spk: x25519_dalek::StaticSecret,
    spk_public: Key32,
    one_time: BTreeMap<u16, x25519_dalek::StaticSecret>,
}

impl PrekeyStore {
    pub fn generate(rng: &mut impl CryptoRngCore, count: usize) -> PrekeyStore {
        let spk = x25519_dalek::StaticSecret::random_from_rng(&mut *rng);
        let spk_public = Key32(x25519_dalek::PublicKey::from(&spk).to_bytes());
        let mut one_time = BTreeMap::new();
        for id in 0..count as u16 {
            one_time.insert(id, x25519_dalek::StaticSecret::random_from_rng(&mut *rng));
        }
        PrekeyStore { spk, spk_public, one_time }
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
}

/// Sender-side agreement output.
pub struct SenderAgreement {
    pub eph_pub: Key32,
    pub prekey_id: u16,
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
