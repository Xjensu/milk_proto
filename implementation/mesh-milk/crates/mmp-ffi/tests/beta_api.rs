use mmp_ffi::{FfiCapability, FfiTier, MmpNode};

#[test]
fn beta_snapshot_is_versioned_secret_free_and_tracks_monotonic_tick() {
    let node = MmpNode::new(vec![0x42; 32], FfiTier::Leaf, None)
        .expect("valid in-memory node construction");

    let before = node.beta_snapshot();
    assert_eq!(before.schema_version, 1);
    assert_eq!(before.status.tier, FfiTier::Leaf);
    assert!(!before.status.persistent_store);
    assert_eq!(before.status.last_tick_ms, 0);
    // Node construction may retain its own protocol bootstrap records; the
    // beta contract reports the local count and does not promise an empty store.
    assert_eq!(
        before.capabilities.capabilities,
        vec![
            FfiCapability::DirectMessaging,
            FfiCapability::GroupMessaging,
            FfiCapability::FileTransfer,
            FfiCapability::Announcements,
            FfiCapability::Diagnostics,
            FfiCapability::TransportInjection,
        ]
    );

    node.tick(4_000);
    let after = node.beta_snapshot();
    assert_eq!(after.schema_version, before.schema_version);
    assert_eq!(after.status.last_tick_ms, 4_000);
    assert_eq!(after.status.tier, before.status.tier);
    assert_eq!(after.capabilities, before.capabilities);
    assert_eq!(after.metrics.store_len, before.metrics.store_len);

    // The report exposes no seed, private material, transport bytes, or
    // persistence path; its typed API contains only the bounded records above.
    assert_eq!(node.node_id().len(), 16);
}
