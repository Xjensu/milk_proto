//! Public-API regression tests for provider-local snapshot envelopes.
//!
//! These tests deliberately stay outside the implementation module so that
//! the persistence boundary is tested exactly as a provider sees it.  The
//! envelope is not an MMP wire format; the byte offsets used for malformed
//! length cases are limited to the documented fixed header prefix needed to
//! exercise the public decoder's fail-closed behavior.

use mmp_crypto::snapshot::{
    open, seal, validate_next_generation, DEFAULT_TTL_MS, KIND_NODE_SECRET_STATE,
    MAX_ENVELOPE_BYTES, MAX_PLAINTEXT_BYTES,
};
use rand::{rngs::StdRng, SeedableRng};

const CONTEXT: &[u8] = b"mmp1/ratchet-snapshot-v1/identity/node";
const WRONG_CONTEXT: &[u8] = b"mmp1/ratchet-snapshot-v1/identity/other";
const KEY: [u8; 32] = [0x2a; 32];
const WRONG_KEY: [u8; 32] = [0x2b; 32];

// MAGIC(4) + VERSION(1) + KIND(1) + GENERATION(8) + WRITTEN_AT(8).
const CIPHERTEXT_LENGTH_OFFSET: usize = 22;
const AEAD_TAG_BYTES: usize = 16;

fn make_envelope(
    rng: &mut StdRng,
    generation: u64,
    written_at_ms: u64,
    plaintext: &[u8],
) -> Vec<u8> {
    seal(
        KIND_NODE_SECRET_STATE,
        generation,
        written_at_ms,
        CONTEXT,
        &KEY,
        rng,
        plaintext,
    )
    .expect("valid test snapshot envelope")
}

fn open_at(envelope: &[u8], now_ms: u64, ttl_ms: u64) -> bool {
    open(
        KIND_NODE_SECRET_STATE,
        CONTEXT,
        &KEY,
        envelope,
        now_ms,
        ttl_ms,
    )
    .is_ok()
}

fn overwrite_ciphertext_length(envelope: &mut [u8], declared_length: usize) {
    let declared_length = u32::try_from(declared_length).expect("test length fits u32");
    envelope[CIPHERTEXT_LENGTH_OFFSET..CIPHERTEXT_LENGTH_OFFSET + 4]
        .copy_from_slice(&declared_length.to_le_bytes());
}

#[test]
fn freshness_accepts_exact_boundaries_and_rejects_future_or_expired_state() {
    let mut rng = StdRng::seed_from_u64(0x51_01);
    let written_at_ms = 10_000;
    let envelope = make_envelope(&mut rng, 1, written_at_ms, b"state");

    assert!(open_at(&envelope, written_at_ms, DEFAULT_TTL_MS));
    assert!(open_at(
        &envelope,
        written_at_ms + DEFAULT_TTL_MS,
        DEFAULT_TTL_MS
    ));
    assert!(!open_at(
        &envelope,
        written_at_ms + DEFAULT_TTL_MS + 1,
        DEFAULT_TTL_MS
    ));
    assert!(!open_at(&envelope, written_at_ms - 1, DEFAULT_TTL_MS));

    // A zero TTL still permits the same trusted timestamp, but not any later
    // time. This locks the inclusive boundary without inventing a new horizon.
    assert!(open_at(&envelope, written_at_ms, 0));
    assert!(!open_at(&envelope, written_at_ms + 1, 0));

    let future = make_envelope(&mut rng, 2, written_at_ms + 1, b"future");
    assert!(!open_at(&future, written_at_ms, DEFAULT_TTL_MS));
}

#[test]
fn generation_validation_rejects_replay_and_reserved_values() {
    let cases = [
        (0, 1, true),
        (1, 2, true),
        (1, 1, false),
        (1, 0, false),
        (1, u64::MAX, false),
        (u64::MAX - 2, u64::MAX - 1, true),
        (u64::MAX - 1, u64::MAX, false),
        (u64::MAX, u64::MAX - 1, false),
    ];

    for (committed, candidate, accepted) in cases {
        assert_eq!(
            validate_next_generation(committed, candidate).is_ok(),
            accepted,
            "unexpected generation result for committed={committed}, candidate={candidate}"
        );
    }
}

#[test]
fn envelope_binding_rejects_wrong_context_kind_key_and_authenticated_header() {
    let mut rng = StdRng::seed_from_u64(0x51_02);
    let envelope = make_envelope(&mut rng, 7, 20_000, b"state");

    assert!(open_at(&envelope, 20_001, DEFAULT_TTL_MS));
    assert!(open(
        KIND_NODE_SECRET_STATE,
        WRONG_CONTEXT,
        &KEY,
        &envelope,
        20_001,
        DEFAULT_TTL_MS
    )
    .is_err());
    assert!(open(
        KIND_NODE_SECRET_STATE + 1,
        CONTEXT,
        &KEY,
        &envelope,
        20_001,
        DEFAULT_TTL_MS
    )
    .is_err());
    assert!(open(
        KIND_NODE_SECRET_STATE,
        CONTEXT,
        &WRONG_KEY,
        &envelope,
        20_001,
        DEFAULT_TTL_MS
    )
    .is_err());

    // Generation and timestamp are authenticated AAD, so changing either
    // field cannot turn an old envelope into an apparently newer one.
    let mut forged = envelope.clone();
    forged[6..14].copy_from_slice(&1u64.to_le_bytes());
    assert!(!open_at(&forged, 20_001, DEFAULT_TTL_MS));
    let mut forged_time = envelope;
    forged_time[14..22].copy_from_slice(&19_000u64.to_le_bytes());
    assert!(!open_at(&forged_time, 20_001, DEFAULT_TTL_MS));
}

#[test]
fn decoder_rejects_every_truncation_trailing_bytes_and_bad_ciphertext_lengths() {
    let mut rng = StdRng::seed_from_u64(0x51_03);
    let envelope = make_envelope(&mut rng, 1, 30_000, b"bounded state");

    for length in 0..envelope.len() {
        assert!(
            !open_at(&envelope[..length], 30_001, DEFAULT_TTL_MS),
            "truncated envelope of length {length} was accepted"
        );
    }

    let mut trailing = envelope.clone();
    trailing.extend_from_slice(&[0xa5, 0x5a]);
    assert!(!open_at(&trailing, 30_001, DEFAULT_TTL_MS));

    let header_len = MAX_ENVELOPE_BYTES - MAX_PLAINTEXT_BYTES - AEAD_TAG_BYTES;
    let actual_ciphertext_length = envelope.len() - header_len;
    for declared_length in [
        0,
        AEAD_TAG_BYTES - 1,
        actual_ciphertext_length - 1,
        actual_ciphertext_length + 1,
        MAX_PLAINTEXT_BYTES + AEAD_TAG_BYTES + 1,
    ] {
        let mut malformed = envelope.clone();
        overwrite_ciphertext_length(&mut malformed, declared_length);
        assert!(
            !open_at(&malformed, 30_001, DEFAULT_TTL_MS),
            "declared ciphertext length {declared_length} was accepted"
        );
    }
}

#[test]
fn published_plaintext_and_envelope_limits_are_enforced_at_the_boundary() {
    let mut rng = StdRng::seed_from_u64(0x51_04);
    let plaintext = vec![0x5a; MAX_PLAINTEXT_BYTES];
    let envelope = make_envelope(&mut rng, 1, 40_000, &plaintext);

    assert_eq!(envelope.len(), MAX_ENVELOPE_BYTES);
    let (generation, restored) = open(
        KIND_NODE_SECRET_STATE,
        CONTEXT,
        &KEY,
        &envelope,
        40_000,
        DEFAULT_TTL_MS,
    )
    .expect("maximum published envelope should round-trip");
    assert_eq!(generation, 1);
    assert_eq!(restored.len(), MAX_PLAINTEXT_BYTES);
    assert!(restored.iter().all(|byte| *byte == 0x5a));

    let oversized = vec![0u8; MAX_PLAINTEXT_BYTES + 1];
    assert!(seal(
        KIND_NODE_SECRET_STATE,
        1,
        40_000,
        CONTEXT,
        &KEY,
        &mut rng,
        &oversized,
    )
    .is_err());

    let mut overlong_envelope = envelope;
    overlong_envelope.push(0);
    assert!(!open_at(&overlong_envelope, 40_000, DEFAULT_TTL_MS));
}
