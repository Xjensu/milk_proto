//! Sealed box (doc 08 §4.1): the always-works async baseline. Confidentiality
//! + recipient-only readability; NO forward secrecy, NO AEAD-layer sender auth
//!   (sender authenticity comes from the bundle signature). Honest limitation.

use alloc::vec::Vec;

use mmp_types::ids::Key32;
use rand_core::CryptoRngCore;

use crate::aead;
use crate::error::CryptoError;
use crate::identity::Identity;

/// Seal to the recipient's exchange-key claim. Returns (ephemeral pubkey, ct).
pub fn seal_box(
    rng: &mut impl CryptoRngCore,
    recipient_exchange: &Key32,
    ad: &[u8],
    plaintext: &[u8],
) -> (Key32, Vec<u8>) {
    let eph = x25519_dalek::EphemeralSecret::random_from_rng(rng);
    let eph_pub = Key32(x25519_dalek::PublicKey::from(&eph).to_bytes());
    let dh = eph.diffie_hellman(&x25519_dalek::PublicKey::from(recipient_exchange.0));
    let key = blake3::derive_key("mmp1 sealed", dh.as_bytes());
    (eph_pub, aead::seal(&key, ad, plaintext))
}

/// Open a box sealed by [`seal_box`]: redo the X25519 agreement with
/// `recipient`'s exchange secret and the sender's ephemeral public key `eph`,
/// then AEAD-open under the derived key.
pub fn open_box(
    recipient: &Identity,
    eph: &Key32,
    ad: &[u8],
    ciphertext: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let dh = recipient.exchange_dh(eph);
    let key = blake3::derive_key("mmp1 sealed", &dh);
    aead::open(&key, ad, ciphertext)
}
