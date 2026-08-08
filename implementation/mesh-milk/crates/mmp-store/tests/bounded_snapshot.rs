use std::fs;

use mmp_core::store::{BundleMeta, BundleStore, SlotState};
use mmp_store::{SnapshotLimits, SnapshotStore};
use mmp_types::ids::Id16;

fn meta(id: u8) -> BundleMeta {
    BundleMeta {
        id: Id16([id; 16]),
        class: 1,
        origin: Id16([2; 16]),
        dest: Id16([3; 16]),
        priority: 1,
        effective_priority: 1,
        size: 5,
        stored_at: 10,
        expires_at: 1000,
        budget: 4,
        own_origin: true,
        slot: SlotState::Stored,
    }
}

#[test]
fn bounded_open_rejects_before_read_and_never_overwrites_primary() {
    let path = std::env::temp_dir().join(format!("mmp-store-bounded-{}.mmp", std::process::id()));
    let temp = path.with_extension("tmp");
    let _ = fs::remove_file(&path);
    let _ = fs::remove_file(&temp);

    let mut writer = SnapshotStore::open(&path);
    writer.put(meta(1), b"hello");
    writer.persist().expect("unbounded writer persists fixture");
    let image = fs::read(&path).expect("fixture image");
    assert!(image.len() > 8);

    let accepted = SnapshotStore::open_with_limits(&path, SnapshotLimits::new(image.len() as u64));
    assert!(!accepted.load_rejected());
    assert!(accepted.get(&Id16([1; 16])).is_some());

    // A smaller candidate must not shadow an oversized primary: the primary
    // path remains authoritative even when the host refuses to load it.
    fs::write(&temp, b"MMPSNAP2").expect("candidate fixture");
    let rejected = SnapshotStore::open_with_limits(&path, SnapshotLimits::new((image.len() - 1) as u64));
    assert!(rejected.load_rejected());
    assert!(rejected.get(&Id16([1; 16])).is_none());
    let error = rejected.persist().expect_err("rejected image must not be replaced");
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    assert_eq!(fs::read(&path).expect("primary remains intact"), image);

    let _ = fs::remove_file(path);
    let _ = fs::remove_file(temp);
}

#[test]
fn bounded_persist_removes_oversized_temp_image() {
    let path = std::env::temp_dir().join(format!("mmp-store-bounded-write-{}.mmp", std::process::id()));
    let temp = path.with_extension("tmp");
    let _ = fs::remove_file(&path);
    let _ = fs::remove_file(&temp);

    let mut store = SnapshotStore::open_with_limits(&path, SnapshotLimits::new(8));
    store.put(meta(2), b"hello");
    let error = store.persist().expect_err("fixture exceeds the complete-file limit");
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    assert!(!path.exists());
    assert!(!temp.exists());

    let _ = fs::remove_file(path);
    let _ = fs::remove_file(temp);
}
