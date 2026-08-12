use mmp_ffi::{FfiError, FfiTier, MmpNode};

#[test]
fn nickname_query_returns_verified_claim_without_exposing_raw_claims() {
    let alice = MmpNode::new(vec![0x71; 32], FfiTier::Leaf, None).expect("valid Alice");
    let bob = MmpNode::new(vec![0x72; 32], FfiTier::Leaf, None).expect("valid Bob");

    alice
        .set_nickname("Alice beta".to_owned(), 1)
        .expect("nickname claim accepted");
    for claim in alice.own_claims() {
        bob.ingest_claim(claim, true).expect("Alice claim decodes");
    }

    assert_eq!(
        bob.nickname_of(alice.node_id()).expect("valid node id"),
        Some("Alice beta".to_owned())
    );
    assert_eq!(
        alice.nickname_of(alice.node_id()).expect("valid node id"),
        Some("Alice beta".to_owned())
    );
    assert_eq!(
        bob.nickname_of(vec![0xabu8; 16]).expect("valid unknown id"),
        None
    );
}

#[test]
fn nickname_query_rejects_malformed_peer_id_before_node_access() {
    let node = MmpNode::new(vec![0x73; 32], FfiTier::Leaf, None).expect("valid node");

    assert_eq!(node.nickname_of(vec![0u8; 15]), Err(FfiError::BadArg));
    assert_eq!(node.nickname_of(vec![0u8; 17]), Err(FfiError::BadArg));
}
