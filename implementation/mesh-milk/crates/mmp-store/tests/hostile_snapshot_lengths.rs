use std::fs;

use mmp_core::store::BundleStore;
use mmp_store::SnapshotStore;

#[test]
fn hostile_length_prefix_is_ignored_without_panicking() {
    let path = std::env::temp_dir().join(format!(
        "mmp-store-hostile-length-{}-{}.mmp",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    let mut bytes = b"MMPSNAP2".to_vec();
    bytes.extend_from_slice(&u32::MAX.to_le_bytes());
    bytes.extend_from_slice(&[0xAA, 0xBB]);
    fs::write(&path, bytes).expect("write hostile fixture");

    let store = SnapshotStore::open(&path);
    assert_eq!(store.len(), 0);

    let _ = fs::remove_file(path);
}
