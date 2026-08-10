use std::fs;

use mmp_core::store::{BundleMeta, BundleStore, SlotState, StorePutError, StorePutOutcome};
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

#[test]
fn bounded_try_put_accounts_for_replacement_delta_before_admission() {
    let stem = format!("mmp-store-bounded-admission-{}", std::process::id());
    let fixture_path = std::env::temp_dir().join(format!("{stem}-fixture.mmp"));
    let path = std::env::temp_dir().join(format!("{stem}.mmp"));
    let fixture_tmp = fixture_path.with_extension("tmp");
    let temp = path.with_extension("tmp");
    for file in [&fixture_path, &fixture_tmp, &path, &temp] {
        let _ = fs::remove_file(file);
    }

    // Establish the exact image size of one five-byte bundle. The bounded
    // store is then opened from that complete image so the replacement test
    // exercises an already-full working set rather than only an in-memory
    // estimate.
    let mut fixture = SnapshotStore::open(&fixture_path);
    fixture.put(meta(1), b"hello");
    fixture.persist().expect("fixture persists");
    let image = fs::read(&fixture_path).expect("fixture image");
    fs::write(&path, &image).expect("copy complete fixture image");

    // One extra byte is available: replacing the existing body with six
    // bytes must fit, while inserting even a one-byte second record must not
    // fit because it also carries framing, metadata, and a checksum.
    let mut bounded = SnapshotStore::open_with_limits(&path, SnapshotLimits::new(image.len() as u64 + 1));
    let mut replacement = meta(1);
    replacement.size = 6;
    assert_eq!(
        bounded.try_put(replacement, b"123456"),
        Ok(StorePutOutcome::Replaced),
        "replacement accounting must charge only the size delta"
    );

    let mut second = meta(2);
    second.size = 1;
    assert_eq!(
        bounded.try_put(second, b"x"),
        Err(StorePutError::Capacity),
        "a new record must include its complete framing overhead"
    );
    assert_eq!(bounded.len(), 1);
    assert_eq!(bounded.total_bytes(), 6);

    bounded.persist().expect("the exact-fill replacement remains durable");
    assert_eq!(fs::metadata(&path).unwrap().len(), image.len() as u64 + 1);
    let restored = SnapshotStore::open(&path);
    assert_eq!(restored.get(&replacement.id).unwrap().1, b"123456");
    assert!(restored.get(&second.id).is_none());

    for file in [&fixture_path, &fixture_tmp, &path, &temp] {
        let _ = fs::remove_file(file);
    }
}

#[test]
fn bounded_try_put_rejected_replacement_preserves_all_state() {
    let stem = format!("mmp-store-bounded-reject-replacement-{}", std::process::id());
    let fixture_path = std::env::temp_dir().join(format!("{stem}-fixture.mmp"));
    let path = std::env::temp_dir().join(format!("{stem}.mmp"));
    for file in [&fixture_path, &path] {
        let _ = fs::remove_file(file);
        let _ = fs::remove_file(file.with_extension("tmp"));
    }

    let mut fixture = SnapshotStore::open(&fixture_path);
    fixture.put(meta(1), b"hello");
    fixture.persist().expect("fixture persists");
    let image = fs::read(&fixture_path).expect("fixture image");
    fs::write(&path, &image).expect("copy complete fixture image");

    let mut bounded = SnapshotStore::open_with_limits(&path, SnapshotLimits::new(image.len() as u64));
    let id = Id16([1; 16]);
    let before_get = bounded.get(&id);
    let before_meta = bounded.all_meta();
    let before_ids = bounded.ids_range(&Id16::MIN, &Id16::MAX);
    let before_len = bounded.len();
    let before_total = bounded.total_bytes();

    let mut replacement = meta(1);
    replacement.size = 6;
    assert_eq!(
        bounded.try_put(replacement, b"123456"),
        Err(StorePutError::Capacity)
    );

    assert_eq!(bounded.get(&id), before_get);
    assert_eq!(bounded.all_meta(), before_meta);
    assert_eq!(bounded.ids_range(&Id16::MIN, &Id16::MAX), before_ids);
    assert_eq!(bounded.len(), before_len);
    assert_eq!(bounded.total_bytes(), before_total);

    for file in [&fixture_path, &path] {
        let _ = fs::remove_file(file);
        let _ = fs::remove_file(file.with_extension("tmp"));
    }
}
