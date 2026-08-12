use mmp_ffi::{FfiError, FfiTier, MmpNode};

#[test]
fn group_application_api_rejects_reserved_tags_and_accepts_custom_tags() {
    let node = MmpNode::new(vec![0x61; 32], FfiTier::Relay, None).expect("valid in-memory node");
    let gid = node.create_group();

    assert_eq!(
        node.group_send_body(gid.clone(), 0x02, Vec::new()),
        Err(FfiError::BadArg),
        "sender-key control tags must use the dedicated group distribution path"
    );
    assert!(node
        .group_send_body(gid.clone(), 0x01, b"manifest".to_vec())
        .is_ok());
    assert!(node
        .group_send_body(gid, 0x42, b"beta payload".to_vec())
        .is_ok());
}
