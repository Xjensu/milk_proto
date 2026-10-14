//! Sealed box (doc 08 §4.1): the always-works async baseline. Confidentiality
//! + recipient-only readability; NO forward secrecy, NO AEAD-layer sender auth
//!   (sender authenticity comes from the bundle signature). Honest limitation.

use alloc::vec::Vec;

use mmp_types::ids::Key32;
use rand_core::CryptoRngCore;

use crate::aead;
use crate::error::CryptoError;
use crate::identity::{reject_low_order, Identity};
use crate::prekey::sealed_ck0;

/// Result of sealing a payload while retaining the initial chain key for a
/// conversation bootstrap.
pub type SealedBoxWithRoot = (Key32, Vec<u8>, zeroize::Zeroizing<[u8; 32]>);

/// Seal to the recipient's exchange-key claim.
///
/// The recipient key is authenticated by the caller's claim path, but it is
/// still attacker-controlled input. Reject low-order X25519 points before
/// deriving a key; otherwise an all-zero DH result would produce a publicly
/// derivable sealed-box key.
pub fn try_seal_box(
    rng: &mut impl CryptoRngCore,
    recipient_exchange: &Key32,
    ad: &[u8],
    plaintext: &[u8],
) -> Result<(Key32, Vec<u8>), CryptoError> {
    let eph = x25519_dalek::EphemeralSecret::random_from_rng(rng);
    let eph_pub = Key32(x25519_dalek::PublicKey::from(&eph).to_bytes());
    let dh = eph.diffie_hellman(&x25519_dalek::PublicKey::from(recipient_exchange.0));
    reject_low_order(dh.as_bytes())?;
    let key = blake3::derive_key("mmp1 sealed", dh.as_bytes());
    Ok((eph_pub, aead::seal(&key, ad, plaintext)))
}

/// Seal a first-contact message and derive the sealed conversation root used
/// for receipt authentication and subsequent ratchet state. The root is
/// returned separately from the wire payload because the sealed payload only
/// carries `eph` and the ciphertext. `sender_id` and `recipient_id` are the
/// Ed25519 identity public keys in the same order used by the recipient.
pub fn try_seal_box_with_root(
    rng: &mut impl CryptoRngCore,
    recipient_exchange: &Key32,
    sender_id: &Key32,
    recipient_id: &Key32,
    ad: &[u8],
    plaintext: &[u8],
) -> Result<SealedBoxWithRoot, CryptoError> {
    let eph = x25519_dalek::EphemeralSecret::random_from_rng(rng);
    let eph_pub = Key32(x25519_dalek::PublicKey::from(&eph).to_bytes());
    let dh = eph.diffie_hellman(&x25519_dalek::PublicKey::from(recipient_exchange.0));
    reject_low_order(dh.as_bytes())?;
    let key = blake3::derive_key("mmp1 sealed", dh.as_bytes());
    let root = sealed_ck0(dh.as_bytes(), sender_id, recipient_id);
    Ok((eph_pub, aead::seal(&key, ad, plaintext), root))
}

/// Open a box sealed by [`try_seal_box`]: redo the X25519 agreement with
/// `recipient`'s exchange secret and the sender's ephemeral public key `eph`,
/// then AEAD-open under the derived key.
pub fn open_box(
    recipient: &Identity,
    eph: &Key32,
    ad: &[u8],
    ciphertext: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let dh = recipient.exchange_dh(eph);
    reject_low_order(&dh)?;
    let key = blake3::derive_key("mmp1 sealed", &dh);
    aead::open(&key, ad, ciphertext)
}
