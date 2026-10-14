use std::sync::Arc;

use mmp_ffi::{FfiEvent, FfiTier, MmpNode};

fn node(seed: u8) -> Arc<MmpNode> {
    MmpNode::new(vec![seed; 32], FfiTier::Relay, None).expect("valid in-memory node")
}

fn introduce(a: &MmpNode, b: &MmpNode) {
    for claim in a.own_claims() {
        b.ingest_claim(claim, true).expect("valid claim from Alice");
    }
    for claim in b.own_claims() {
        a.ingest_claim(claim, true).expect("valid claim from Bob");
    }
}

fn contact(a: &MmpNode, b: &MmpNode, link: u64, now_ms: u64) -> Vec<FfiEvent> {
    a.tick(now_ms);
    b.tick(now_ms);
    a.link_up(link, true);
    b.link_up(link, false);

    for _ in 0..300 {
        let mut quiet = true;
        for frame in a.poll_send_batch(64) {
            quiet = false;
            b.link_data(frame.link, frame.bytes);
        }
        for frame in b.poll_send_batch(64) {
            quiet = false;
            a.link_data(frame.link, frame.bytes);
        }
        if quiet {
            break;
        }
    }

    a.link_down(link);
    b.link_down(link);
    std::iter::from_fn(|| b.poll_event()).collect()
}

#[test]
fn beta_app_workflow_covers_send_observe_status_and_restart() {
    let alice = node(0x51);
    let bob = node(0x52);
    introduce(&alice, &bob);
    while alice.poll_event().is_some() {}
    while bob.poll_event().is_some() {}

    // The first contact establishes the authenticated session. The app then
    // sends and observes a message through the same bounded bridge calls used
    // by the native drive loops.
    let _ = contact(&alice, &bob, 91, 1_000);
    let message_id = alice
        .send_text(bob.node_id(), "beta workflow".to_owned())
        .expect("known peer accepts text");
    let events = contact(&alice, &bob, 92, 2_000);
    assert!(events.iter().any(|event| matches!(
        event,
        FfiEvent::MessageReceived { id, body, .. }
            if *id == message_id && body == b"beta workflow"
    )));

    let snapshot = alice.beta_snapshot();
    assert_eq!(snapshot.schema_version, 1);
    assert_eq!(snapshot.status, alice.status());
    assert_eq!(snapshot.metrics, alice.metrics());
    assert_eq!(snapshot.status.last_tick_ms, 2_000);
    assert_eq!(snapshot.metrics.store_len, alice.store_len());

    // A beta host persists on lifecycle pause and reconstructs with the same
    // seed/path after a process restart. The secret seed never leaves the
    // host; only the public identity is compared across the restart.
    let dir = TempDir::new();
    let path = dir.file();
    let persisted =
        MmpNode::new(vec![0x61; 32], FfiTier::Relay, Some(path.clone())).expect("file-backed node");
    let public_key = persisted.public_key();
    persisted.tick(3_000);
    persisted.flush().expect("lifecycle flush succeeds");
    drop(persisted);

    let reopened = MmpNode::new(vec![0x61; 32], FfiTier::Relay, Some(path))
        .expect("restart reopens the validated snapshot");
    assert_eq!(reopened.public_key(), public_key);
    assert!(reopened.status().persistent_store);
    reopened.tick(3_001);
    assert_eq!(reopened.status().last_tick_ms, 3_001);
}

struct TempDir(std::path::PathBuf);

impl TempDir {
    fn new() -> Self {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("mmp-ffi-beta-app-{unique}"));
        std::fs::create_dir_all(&path).expect("create temporary directory");
        Self(path)
    }

    fn file(&self) -> String {
        self.0
            .join("node.mmp")
            .to_str()
            .expect("temporary path is UTF-8")
            .to_owned()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
