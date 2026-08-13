//! Provider-local authenticated envelope for secret-state snapshots.
//!
//! This module is deliberately narrower than a persistence provider. It uses
//! a fresh random ChaCha20-Poly1305 nonce, binds the snapshot kind/context and
//! metadata as associated data, enforces a bounded plaintext, and requires a
//! caller-supplied trusted time for the 72-hour freshness check. The caller is
//! still responsible for generating/protecting the DEK, durable monotonic
//! generation, and atomic commit/recovery of the complete node state.
//!
//! The envelope is not an MMP wire format and must not be sent over the mesh.

use alloc::vec::Vec;

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};
use rand_core::CryptoRngCore;
use zeroize::Zeroizing;

use crate::error::CryptoError;

const MAGIC: [u8; 4] = *b"MKS1";
const VERSION: u8 = 1;
const NONCE_LEN: usize = 12;
const CONTEXT_HASH_LEN: usize = 32;
const TAG_LEN: usize = 16;
const HEADER_LEN: usize = 4 + 1 + 1 + 8 + 8 + 4 + CONTEXT_HASH_LEN + NONCE_LEN;

/// Maximum plaintext accepted in one sealed secret-state envelope.
pub const MAX_PLAINTEXT_BYTES: usize = 256 * 1024;

/// Maximum encoded envelope size accepted by [`open`].  Keeping this bound
/// public lets persistence providers reject an oversized opaque record before
/// passing it to a native key unwrap operation, instead of duplicating the
/// private header/tag arithmetic in every adapter.
pub const MAX_ENVELOPE_BYTES: usize = HEADER_LEN + MAX_PLAINTEXT_BYTES + TAG_LEN;

/// Snapshot envelope kind for the complete node secret-state aggregate.
pub const KIND_NODE_SECRET_STATE: u8 = 1;

/// Default freshness bound from specification/08 §5.3 (72 hours).
pub const DEFAULT_TTL_MS: u64 = 72 * 60 * 60 * 1_000;

fn snapshot_error() -> CryptoError {
    CryptoError::Snapshot
}

fn context_hash(context: &[u8]) -> [u8; CONTEXT_HASH_LEN] {
    *blake3::hash(context).as_bytes()
}

fn build_header(
    kind: u8,
    generation: u64,
    written_at_ms: u64,
    ciphertext_len: usize,
    context_hash: &[u8; CONTEXT_HASH_LEN],
    nonce: &[u8; NONCE_LEN],
) -> Result<Vec<u8>, CryptoError> {
    let ciphertext_len = u32::try_from(ciphertext_len).map_err(|_| snapshot_error())?;
    let mut header = Vec::with_capacity(HEADER_LEN);
    header.extend_from_slice(&MAGIC);
    header.push(VERSION);
    header.push(kind);
    header.extend_from_slice(&generation.to_le_bytes());
    header.extend_from_slice(&written_at_ms.to_le_bytes());
    header.extend_from_slice(&ciphertext_len.to_le_bytes());
    header.extend_from_slice(context_hash);
    header.extend_from_slice(nonce);
    Ok(header)
}

/// Seal a typed secret-state plaintext under a caller-owned DEK.
///
/// `context` is hashed into the envelope and authenticated as AAD. Callers
/// should include the schema, local identity, node ID, and all conversation
/// identities covered by the generation. `written_at_ms` must come from the
/// durable platform clock, not the post-restart simulation clock. The DEK is
/// not persisted by this function; a platform vault must wrap it or replace
/// it with an opaque handle.
pub fn seal(
    kind: u8,
    generation: u64,
    written_at_ms: u64,
    context: &[u8],
    dek: &[u8; 32],
    rng: &mut impl CryptoRngCore,
    plaintext: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    if kind == 0
        || plaintext.len() > MAX_PLAINTEXT_BYTES
        || generation == 0
        || generation == u64::MAX
        || written_at_ms == 0
        || written_at_ms == u64::MAX
    {
        return Err(snapshot_error());
    }

    let mut nonce = [0u8; NONCE_LEN];
    rng.fill_bytes(&mut nonce);
    let ciphertext_len = plaintext
        .len()
        .checked_add(TAG_LEN)
        .ok_or_else(snapshot_error)?;
    let header = build_header(
        kind,
        generation,
        written_at_ms,
        ciphertext_len,
        &context_hash(context),
        &nonce,
    )?;
    let cipher = ChaCha20Poly1305::new(dek.into());
    let ciphertext = cipher
        .encrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: plaintext,
                aad: &header,
            },
        )
        .map_err(|_| snapshot_error())?;
    debug_assert_eq!(ciphertext.len(), ciphertext_len);

    let total_len = header
        .len()
        .checked_add(ciphertext.len())
        .ok_or_else(snapshot_error)?;
    let mut envelope = Vec::with_capacity(total_len);
    envelope.extend_from_slice(&header);
    envelope.extend_from_slice(&ciphertext);
    Ok(envelope)
}

/// Open a sealed envelope after checking kind, context, bounded length, and
/// trusted-clock freshness. The returned plaintext is zeroizing, but callers
/// must still avoid copying it across an FFI or JavaScript boundary.
pub fn open(
    expected_kind: u8,
    expected_context: &[u8],
    dek: &[u8; 32],
    envelope: &[u8],
    now_ms: u64,
    ttl_ms: u64,
) -> Result<(u64, Zeroizing<Vec<u8>>), CryptoError> {
    if expected_kind == 0 || envelope.len() < HEADER_LEN + TAG_LEN {
        return Err(snapshot_error());
    }
    let header = &envelope[..HEADER_LEN];
    if header[..4] != MAGIC || header[4] != VERSION || header[5] != expected_kind {
        return Err(snapshot_error());
    }

    let mut generation_bytes = [0u8; 8];
    generation_bytes.copy_from_slice(&header[6..14]);
    let generation = u64::from_le_bytes(generation_bytes);
    if generation == 0 || generation == u64::MAX {
        return Err(snapshot_error());
    }
    let mut written_at_bytes = [0u8; 8];
    written_at_bytes.copy_from_slice(&header[14..22]);
    let written_at_ms = u64::from_le_bytes(written_at_bytes);
    if written_at_ms == 0
        || written_at_ms == u64::MAX
        || written_at_ms > now_ms
        || now_ms.saturating_sub(written_at_ms) > ttl_ms
    {
        return Err(snapshot_error());
    }
    let mut ciphertext_len_bytes = [0u8; 4];
    ciphertext_len_bytes.copy_from_slice(&header[22..26]);
    let ciphertext_len = u32::from_le_bytes(ciphertext_len_bytes) as usize;
    let max_ciphertext_len = MAX_PLAINTEXT_BYTES
        .checked_add(TAG_LEN)
        .ok_or_else(snapshot_error)?;
    if ciphertext_len < TAG_LEN || ciphertext_len > max_ciphertext_len {
        return Err(snapshot_error());
    }
    let expected_len = HEADER_LEN
        .checked_add(ciphertext_len)
        .ok_or_else(snapshot_error)?;
    if envelope.len() != expected_len {
        return Err(snapshot_error());
    }
    let expected_context_hash = context_hash(expected_context);
    if header[26..58] != expected_context_hash {
        return Err(snapshot_error());
    }

    let mut nonce = [0u8; NONCE_LEN];
    nonce.copy_from_slice(&header[58..70]);
    let cipher = ChaCha20Poly1305::new(dek.into());
    let plaintext = cipher
        .decrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: &envelope[HEADER_LEN..],
                aad: header,
            },
        )
        .map_err(|_| snapshot_error())?;
    if plaintext.len() > MAX_PLAINTEXT_BYTES {
        return Err(snapshot_error());
    }
    Ok((generation, Zeroizing::new(plaintext)))
}

/// Validate that `candidate` is the next durable generation after
/// `committed`. Providers must perform this check atomically with their
/// compare-and-swap/commit operation; this helper cannot itself prevent a
/// rollback in a stateless caller.
pub fn validate_next_generation(committed: u64, candidate: u64) -> Result<(), CryptoError> {
    if candidate == 0 || candidate == u64::MAX || candidate <= committed {
        Err(snapshot_error())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{rngs::StdRng, SeedableRng};

    const CONTEXT: &[u8] = b"mmp1/ratchet-snapshot-v1/identity/node";

    fn envelope(rng: &mut StdRng) -> Vec<u8> {
        seal(
            KIND_NODE_SECRET_STATE,
            1,
            1_000,
            CONTEXT,
            &[7u8; 32],
            rng,
            b"typed secret aggregate",
        )
        .unwrap()
    }

    #[test]
    fn roundtrip_binds_context_and_uses_distinct_nonces() {
        let mut rng = StdRng::seed_from_u64(7);
        let first = envelope(&mut rng);
        let second = envelope(&mut rng);
        assert_ne!(first, second);
        let (generation, plaintext) = open(
            KIND_NODE_SECRET_STATE,
            CONTEXT,
            &[7u8; 32],
            &first,
            1_001,
            DEFAULT_TTL_MS,
        )
        .unwrap();
        assert_eq!(generation, 1);
        assert_eq!(&*plaintext, b"typed secret aggregate");
        assert!(open(
            KIND_NODE_SECRET_STATE,
            b"wrong",
            &[7u8; 32],
            &first,
            1_001,
            DEFAULT_TTL_MS
        )
        .is_err());
    }

    #[test]
    fn rejects_tamper_truncation_wrong_key_kind_and_version() {
        let mut rng = StdRng::seed_from_u64(8);
        let original = envelope(&mut rng);
        let mut tampered = original.clone();
        tampered[HEADER_LEN] ^= 1;
        assert!(open(
            KIND_NODE_SECRET_STATE,
            CONTEXT,
            &[7u8; 32],
            &tampered,
            1_001,
            DEFAULT_TTL_MS
        )
        .is_err());
        assert!(open(
            KIND_NODE_SECRET_STATE,
            CONTEXT,
            &[8u8; 32],
            &original,
            1_001,
            DEFAULT_TTL_MS
        )
        .is_err());
        assert!(open(2, CONTEXT, &[7u8; 32], &original, 1_001, DEFAULT_TTL_MS).is_err());
        let mut truncated = original.clone();
        truncated.pop();
        assert!(open(
            KIND_NODE_SECRET_STATE,
            CONTEXT,
            &[7u8; 32],
            &truncated,
            1_001,
            DEFAULT_TTL_MS
        )
        .is_err());
        let mut bad_version = original;
        bad_version[4] = VERSION + 1;
        assert!(open(
            KIND_NODE_SECRET_STATE,
            CONTEXT,
            &[7u8; 32],
            &bad_version,
            1_001,
            DEFAULT_TTL_MS
        )
        .is_err());
    }

    #[test]
    fn rejects_stale_future_and_oversized_plaintext() {
        let mut rng = StdRng::seed_from_u64(9);
        assert!(validate_next_generation(0, 1).is_ok());
        assert!(validate_next_generation(1, 1).is_err());
        assert!(validate_next_generation(1, 0).is_err());
        assert!(seal(
            KIND_NODE_SECRET_STATE,
            1,
            0,
            CONTEXT,
            &[7u8; 32],
            &mut rng,
            b"x"
        )
        .is_err());
        assert!(seal(
            KIND_NODE_SECRET_STATE,
            1,
            u64::MAX,
            CONTEXT,
            &[7u8; 32],
            &mut rng,
            b"x"
        )
        .is_err());
        let stale = seal(
            KIND_NODE_SECRET_STATE,
            1,
            1_000,
            CONTEXT,
            &[7u8; 32],
            &mut rng,
            b"x",
        )
        .unwrap();
        assert!(open(
            KIND_NODE_SECRET_STATE,
            CONTEXT,
            &[7u8; 32],
            &stale,
            1_001 + DEFAULT_TTL_MS,
            DEFAULT_TTL_MS
        )
        .is_err());
        assert!(open(
            KIND_NODE_SECRET_STATE,
            CONTEXT,
            &[7u8; 32],
            &stale,
            999,
            DEFAULT_TTL_MS
        )
        .is_err());
        let oversized = vec![0u8; MAX_PLAINTEXT_BYTES + 1];
        assert!(seal(
            KIND_NODE_SECRET_STATE,
            1,
            1_000,
            CONTEXT,
            &[7u8; 32],
            &mut rng,
            &oversized
        )
        .is_err());
    }

    #[test]
    fn generation_metadata_is_authenticated_against_rollback() {
        let mut rng = StdRng::seed_from_u64(10);
        let original = seal(
            KIND_NODE_SECRET_STATE,
            9,
            1_000,
            CONTEXT,
            &[7u8; 32],
            &mut rng,
            b"secret-state",
        )
        .unwrap();

        // A valid envelope exposes its generation only after AEAD verification,
        // and the provider must still reject a candidate at or below its
        // durable floor. This is the rollback boundary from the dossier.
        let (generation, _) = open(
            KIND_NODE_SECRET_STATE,
            CONTEXT,
            &[7u8; 32],
            &original,
            1_001,
            DEFAULT_TTL_MS,
        )
        .unwrap();
        assert_eq!(generation, 9);
        assert!(validate_next_generation(8, generation).is_ok());
        assert!(validate_next_generation(9, generation).is_err());

        // The generation is authenticated as part of the header AAD. A stale
        // writer cannot lower it in place and make the envelope look newer to
        // a provider that has already committed generation 9.
        let mut forged = original;
        forged[6..14].copy_from_slice(&1u64.to_le_bytes());
        assert!(open(
            KIND_NODE_SECRET_STATE,
            CONTEXT,
            &[7u8; 32],
            &forged,
            1_001,
            DEFAULT_TTL_MS,
        )
        .is_err());
    }
}
