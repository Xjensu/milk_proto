use mmp_types::bundle::{class, Bundle};
use mmp_types::ids::{Id16, Key32, Sig64};

fn bundle(lifetime_ms: u64, dest: u8, priority: u8, payload: Vec<u8>) -> Bundle {
    Bundle {
        class: class::DIRECT,
        flags: 0,
        origin: Key32([0x11; 32]),
        dest: Id16([dest; 16]),
        lifetime_ms,
        priority,
        exts: Vec::new(),
        payload,
        signature: Sig64::ZERO,
    }
}

#[test]
fn canonical_aead_ad_excludes_payload_signature_and_live_lifetime() {
    let base = bundle(72_000, 0x22, 1, vec![0xaa, 0xbb, 0xcc]);
    let mut changed_lifetime = base.clone();
    changed_lifetime.lifetime_ms = 1;
    assert_eq!(
        base.canonical_aead_ad(),
        changed_lifetime.canonical_aead_ad()
    );

    let mut changed_payload = base.clone();
    changed_payload.payload = vec![0xdd, 0xee, 0xff];
    assert_eq!(
        base.canonical_aead_ad(),
        changed_payload.canonical_aead_ad()
    );

    let mut changed_signature = base.clone();
    changed_signature.signature = Sig64([0x77; 64]);
    assert_eq!(
        base.canonical_aead_ad(),
        changed_signature.canonical_aead_ad()
    );
}

#[test]
fn canonical_aead_ad_binds_routing_header_fields() {
    let base = bundle(72_000, 0x22, 1, vec![0xaa]);
    let mut changed_dest = base.clone();
    changed_dest.dest = Id16([0x33; 16]);
    assert_ne!(base.canonical_aead_ad(), changed_dest.canonical_aead_ad());

    let mut changed_priority = base.clone();
    changed_priority.priority = 2;
    assert_ne!(
        base.canonical_aead_ad(),
        changed_priority.canonical_aead_ad()
    );
}

#[test]
fn canonical_aead_ad_binds_payload_length_without_binding_payload_bytes() {
    let short = bundle(72_000, 0x22, 1, vec![0xaa]);
    let long = bundle(72_000, 0x22, 1, vec![0xaa, 0xbb]);

    assert_ne!(short.canonical_aead_ad(), long.canonical_aead_ad());

    let mut same_length = short.clone();
    same_length.payload = vec![0xcc];
    assert_eq!(short.canonical_aead_ad(), same_length.canonical_aead_ad());
}
