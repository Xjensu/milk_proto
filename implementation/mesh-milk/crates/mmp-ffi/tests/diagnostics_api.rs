use mmp_ffi::{FfiTier, MmpNode};

#[test]
fn diagnostics_can_be_disabled_without_changing_the_node_snapshot() {
    let node = MmpNode::new(vec![0x73; 32], FfiTier::Leaf, None)
        .expect("valid in-memory node construction");

    assert!(!node.status().diagnostics_enabled);
    node.enable_diagnostics();
    assert!(node.status().diagnostics_enabled);

    node.disable_diagnostics();
    let status = node.status();
    assert!(!status.diagnostics_enabled);
    assert_eq!(status.tier, FfiTier::Leaf);
    assert!(!status.persistent_store);
}
