//! ChaCha20-Poly1305 helpers. Every key here is single-use (per-message ratchet
//! keys, sealed-box keys), so a zero nonce is sound and canonical.

use alloc::vec::Vec;

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};

use crate::error::CryptoError;

/// Seal `plaintext` under `key` with associated data `ad`, using the fixed
/// zero nonce (sound here because every key is single-use, see module docs).
pub fn seal(key: &[u8; 32], ad: &[u8], plaintext: &[u8]) -> Vec<u8> {
    let cipher = ChaCha20Poly1305::new(key.into());
    cipher
        .encrypt(&Nonce::default(), Payload { msg: plaintext, aad: ad })
        .expect("chacha20poly1305 encrypt is infallible for in-memory buffers")
}

/// Open `ciphertext` under `key` with associated data `ad`, using the fixed
/// zero nonce. Fails with [`CryptoError::Aead`] if the tag does not verify.
pub fn open(key: &[u8; 32], ad: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let cipher = ChaCha20Poly1305::new(key.into());
    cipher
        .decrypt(&Nonce::default(), Payload { msg: ciphertext, aad: ad })
        .map_err(|_| CryptoError::Aead)
}

/// AEAD context commitment (doc 16 §4.2, CR-S2): bind (key, context) so a valid
/// tag implies a unique intended (conversation, epoch, index) pair.
pub fn committed_key(base: &[u8; 32], context: &[u8]) -> [u8; 32] {
    *blake3::keyed_hash(base, context).as_bytes()
}
