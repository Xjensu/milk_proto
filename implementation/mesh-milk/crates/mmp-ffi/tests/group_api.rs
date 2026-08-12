use std::sync::Arc;

use mmp_ffi::{FfiError, FfiEvent, FfiTier, MmpNode};
use mmp_types::consts::MAX_BUNDLE;

fn introduce(a: &MmpNode, b: &MmpNode) {
    for claim in a.own_claims() {
        b.ingest_claim(claim, true).expect("well-formed claim");
    }
    for claim in b.own_claims() {
        a.ingest_claim(claim, true).expect("well-formed claim");
    }
}

fn drain(node: &MmpNode) -> Vec<FfiEvent> {
    std::iter::from_fn(|| node.poll_event()).collect()
}

fn contact(a: &Arc<MmpNode>, b: &Arc<MmpNode>, link: u64, now: u64) {
    a.tick(now);
    b.tick(now);
    a.link_up(link, true);
    b.link_up(link, false);
    for _ in 0..300 {
        let mut quiet = true;
        while let Some(frame) = a.poll_send() {
            quiet = false;
            b.link_data(frame.link, frame.bytes);
        }
        while let Some(frame) = b.poll_send() {
            quiet = false;
            a.link_data(frame.link, frame.bytes);
        }
        if quiet {
            break;
        }
    }
    a.link_down(link);
    b.link_down(link);
}

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

#[test]
fn oversized_group_body_is_rejected_before_queue_or_group_mutation() {
    let node = MmpNode::new(vec![0x64; 32], FfiTier::Relay, None).expect("valid in-memory node");
    let gid = node.create_group();
    let members_before = node.group_members(gid.clone()).expect("created group is known");
    let store_before = node.store_len();
    drain(&node);

    assert!(node.poll_send().is_none());
    assert!(node.poll_event().is_none());

    assert_eq!(
        node.group_send_body(gid.clone(), 0x42, vec![0u8; MAX_BUNDLE]),
        Err(FfiError::BadArg),
        "the FFI boundary must reject a payload at the frozen bundle cap"
    );

    assert_eq!(node.store_len(), store_before, "rejected input must not enter the store");
    assert!(node.poll_send().is_none(), "rejected input must not queue transport work");
    assert!(node.poll_event().is_none(), "rejected input must not emit an application event");
    assert_eq!(
        node.group_members(gid.clone()).expect("group remains known"),
        members_before,
        "rejected input must not mutate group membership state"
    );

    assert!(
        node.group_send_body(gid, 0x42, b"retry after rejection".to_vec())
            .is_ok(),
        "a valid retry must remain usable after rejected input"
    );
    assert_eq!(node.store_len(), store_before + 1, "only the valid retry should be stored");
}

#[test]
fn group_application_event_preserves_type_tag_at_ffi_boundary() {
    let alice = MmpNode::new(vec![0x62; 32], FfiTier::Relay, None).expect("valid Alice");
    let bob = MmpNode::new(vec![0x63; 32], FfiTier::Relay, None).expect("valid Bob");
    introduce(&alice, &bob);
    drain(&alice);
    drain(&bob);

    let gid = alice.create_group();
    alice.group_invite(gid.clone(), bob.node_id()).expect("group invite");
    contact(&alice, &bob, 90, 900_000);
    drain(&alice);
    drain(&bob);

    let payload = b"group beta payload".to_vec();
    alice
        .group_send_body(gid.clone(), 0x42, payload.clone())
        .expect("custom group body");
    contact(&alice, &bob, 91, 910_000);

    assert!(drain(&bob).into_iter().any(|event| matches!(
        event,
        FfiEvent::GroupMessage { gid: event_gid, apptype, body, .. }
            if event_gid == gid && apptype == 0x42 && body == payload
    )));
}
