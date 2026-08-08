use mmp_core::bloom::Bloom;
use mmp_core::{ReceiptFilter, SeenIndex};
use mmp_types::consts::{BLOOM_BITS_PER_ELEMENT, BLOOM_CAPACITY, FILTER_BUCKETS};

#[test]
fn bloom_decode_rejects_zero_modulus_and_oversized_bit_arrays() {
    // A zero modulus would make the post-restore membership check panic on
    // `hash % m`; persisted bytes are untrusted even when their outer
    // SnapshotStore checksum is valid.
    let mut zero = Vec::new();
    zero.extend_from_slice(&0u32.to_le_bytes());
    zero.extend_from_slice(&0u32.to_le_bytes());
    assert!(Bloom::decode(&zero).is_none());

    // Production filters are sized by BLOOM_CAPACITY × bits/element.  A
    // larger m must not be accepted merely because the attacker supplied a
    // matching-length bit array in the snapshot record.
    let oversized_bits = BLOOM_CAPACITY * BLOOM_BITS_PER_ELEMENT / 8 + 1;
    let mut oversized = Vec::with_capacity(8 + oversized_bits);
    oversized.extend_from_slice(&((oversized_bits * 8) as u32).to_le_bytes());
    oversized.extend_from_slice(&0u32.to_le_bytes());
    oversized.resize(8 + oversized_bits, 0);
    assert!(Bloom::decode(&oversized).is_none());
}

#[test]
fn bloom_capacity_clamps_huge_requests_before_allocation() {
    let bloom = Bloom::with_capacity(usize::MAX);
    let expected_bits = BLOOM_CAPACITY * BLOOM_BITS_PER_ELEMENT;
    assert_eq!(bloom.bytes().len(), expected_bits.div_ceil(8));
    assert_eq!(bloom.count(), 0);
}

fn encoded_minimal_bloom_bucket(epoch: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + 8 + 8);
    out.extend_from_slice(&epoch.to_le_bytes());
    out.extend_from_slice(&64u32.to_le_bytes()); // minimum valid m (8 bytes of bits)
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&[0u8; 8]);
    out
}

#[test]
fn seen_index_decode_rejects_more_than_the_normative_bucket_horizon() {
    let count = FILTER_BUCKETS + 1;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&(count as u32).to_le_bytes());
    for epoch in 0..count as u64 {
        bytes.extend_from_slice(&encoded_minimal_bloom_bucket(epoch));
    }
    bytes.extend_from_slice(&0u32.to_le_bytes()); // recent_count
    assert!(SeenIndex::decode(&bytes).is_none());
}

#[test]
fn receipt_filter_decode_rejects_more_than_the_normative_bucket_horizon() {
    let count = FILTER_BUCKETS + 1;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&(count as u32).to_le_bytes());
    for epoch in 0..count as u64 {
        bytes.extend_from_slice(&encoded_minimal_bloom_bucket(epoch));
    }
    bytes.extend_from_slice(&0u32.to_le_bytes()); // exact_count
    assert!(ReceiptFilter::decode(&bytes).is_none());
}

#[test]
fn receipt_filter_decode_rejects_exact_count_without_remaining_entries() {
    // Header only: exact_count claims one 24-byte entry, but no entry bytes
    // remain. The count must be rejected before the parse loop starts.
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&0u32.to_le_bytes()); // bucket_count
    bytes.extend_from_slice(&1u32.to_le_bytes()); // exact_count
    assert!(ReceiptFilter::decode(&bytes).is_none());
}
