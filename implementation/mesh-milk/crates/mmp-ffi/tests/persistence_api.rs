use mmp_ffi::{FfiError, FfiTier, MmpNode};

#[test]
fn file_backed_flush_reports_success_without_secret_state() {
    let dir = TempDir::new("flush-success");
    let node = MmpNode::new(vec![0x61; 32], FfiTier::Relay, Some(dir.file("node.mmp")))
        .expect("valid file-backed node");

    let status = node.status();
    assert!(status.persistent_store);
    assert!(node.capabilities().persistent_store);

    node.flush().expect("writable snapshot must flush");
    assert_eq!(node.last_persist_error(), None);
}

#[test]
fn bounded_store_flush_reports_provider_failure() {
    let dir = TempDir::new("flush-failure");
    let node =
        MmpNode::new_with_store_limit(vec![0x62; 32], FfiTier::Relay, dir.file("bounded.mmp"), 1)
            .expect("valid bounded provider configuration");

    assert_eq!(node.flush(), Err(FfiError::Internal));
    assert!(node.last_persist_error().is_some());
}

struct TempDir(std::path::PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("mmp-ffi-persistence-{tag}-{unique}"));
        std::fs::create_dir_all(&path).expect("create test temp directory");
        Self(path)
    }

    fn file(&self, name: &str) -> String {
        self.0
            .join(name)
            .to_str()
            .expect("temporary path must be valid UTF-8")
            .to_owned()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
