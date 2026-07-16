//! mmp-crypto — cipher suite 0x01 (doc 08).
//!
//! Ed25519 (strict/canonical-S only) · X25519 · ChaCha20-Poly1305 · BLAKE3,
//! with BLAKE2s inside the standard Noise XX configuration (doc 08 §1).
//! No custom primitives; the project-specific compositions (§4.2 prekeys,
//! §3.2 channel binding, §5.2 opportunistic epoching) are flagged for external
//! review (OQ-10) in the spec.
//!
//! Secrets zeroize on drop; RNG only via injected `rand_core::CryptoRngCore`.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod aead;
pub mod error;
pub mod group;
pub mod identity;
pub mod merkle;
#[cfg(feature = "std")]
pub mod noise;
pub mod prekey;
pub mod ratchet;
pub mod receipt;
pub mod sealed;

pub use error::CryptoError;
pub use identity::{verify, Identity};
#[cfg(feature = "std")]
pub use noise::{HandshakeOutcome, NoiseChannel, NoiseHandshake, PeerIntro};

/// Conversation id: `BLAKE3("mmp1 conv-id" ‖ sorted(pk_a, pk_b))[0..16]` —
/// commutative, so both parties agree without negotiation.
pub fn conversation_id(a: &mmp_types::Key32, b: &mmp_types::Key32) -> mmp_types::Id16 {
    let (lo, hi) = if a.0 <= b.0 { (a, b) } else { (b, a) };
    let mut h = blake3::Hasher::new_derive_key("mmp1 conv-id");
    h.update(&lo.0);
    h.update(&hi.0);
    let mut id = [0u8; 16];
    id.copy_from_slice(&h.finalize().as_bytes()[..16]);
    mmp_types::Id16(id)
}

/// Rotating beacon pseudonym (doc 03 §5.1.1):
/// `ephem_id = BLAKE3(identity_pubkey ‖ coarse_time_slot ‖ beacon_salt)[0..8]`.
pub fn ephem_id(identity_pk: &mmp_types::Key32, slot: u64, beacon_salt: &[u8; 16]) -> [u8; 8] {
    let mut h = blake3::Hasher::new();
    h.update(&identity_pk.0);
    h.update(&slot.to_le_bytes());
    h.update(beacon_salt);
    let mut id = [0u8; 8];
    id.copy_from_slice(&h.finalize().as_bytes()[..8]);
    id
}

/// Beacon summary re-salt (doc 03 §5.1.1.1, AUDIT F8a):
/// `slot_salt = BLAKE3("mmp1 beacon-salt" ‖ slot ‖ beacon_salt)`.
pub fn beacon_slot_salt(slot: u64, beacon_salt: &[u8; 16]) -> [u8; 32] {
    let mut h = blake3::Hasher::new_derive_key("mmp1 beacon-salt");
    h.update(&slot.to_le_bytes());
    h.update(beacon_salt);
    *h.finalize().as_bytes()
}

#[cfg(test)]
mod tests;
