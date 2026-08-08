use mmp_types::bundle::id_of_encoded;
use mmp_types::consts::MAX_BUNDLE;
use mmp_types::ids::{Id16, Key32, Sig64};
use mmp_types::{class, Bundle, WireError};

fn sample_bundle() -> Bundle {
    Bundle {
        class: class::DIRECT,
        flags: 0,
        origin: Key32([7; 32]),
        dest: Id16([9; 16]),
        lifetime_ms: 259_200_000,
        priority: 1,
        exts: Vec::new(),
        payload: vec![0xAA; 100],
        signature: Sig64([0x55; 64]),
    }
}

#[test]
fn valid_encoded_bundle_id_matches_structural_id() {
    let bundle = sample_bundle();
    assert_eq!(id_of_encoded(&bundle.encode()), Ok(bundle.id()));
}

#[test]
fn truncated_encoded_bundle_is_rejected_without_panicking() {
    for length in [0usize, 1, 63, 64, 118, 119, 123] {
        assert_eq!(id_of_encoded(&vec![0u8; length]), Err(WireError::Truncated), "length {length}");
    }
}

#[test]
fn oversized_encoded_bundle_is_rejected_before_copying() {
    assert_eq!(id_of_encoded(&vec![0u8; MAX_BUNDLE + 1]), Err(WireError::Oversize));
}
