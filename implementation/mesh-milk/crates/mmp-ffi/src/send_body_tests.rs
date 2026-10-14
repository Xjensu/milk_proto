//! Focused FFI regression tests for application-defined direct bodies.

use super::{FfiError, FfiEvent, FfiTier, MmpNode};

fn node(seed: u8) -> std::sync::Arc<MmpNode> {
    MmpNode::new(vec![seed; 32], FfiTier::Relay, None).expect("valid 32-byte seed")
}

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

fn contact(a: &MmpNode, b: &MmpNode, link: u64, now: u64) {
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
fn application_body_round_trips_and_reserved_tags_are_rejected() {
    let alice = node(91);
    let bob = node(92);
    introduce(&alice, &bob);
    drain(&alice);
    drain(&bob);

    let body = b"beta application payload".to_vec();
    assert_eq!(
        alice.send_body(bob.node_id(), 0x02, body.clone()),
        Err(FfiError::BadArg),
        "sender-key control tag must use its dedicated core path"
    );
    let message_id = alice
        .send_body(bob.node_id(), 0x42, body.clone())
        .expect("known peer accepts a bounded application body");

    contact(&alice, &bob, 1, 10_000);
    let received = drain(&bob)
        .into_iter()
        .find_map(|event| match event {
            FfiEvent::MessageReceived {
                id, body, apptype, ..
            } => Some((id, body, apptype)),
            _ => None,
        })
        .expect("application body event");

    assert_eq!(received.0, message_id);
    assert_eq!(received.1, body);
    assert_eq!(received.2, 0x42);
}
