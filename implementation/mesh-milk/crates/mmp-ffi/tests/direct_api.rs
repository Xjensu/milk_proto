use std::sync::Arc;

use mmp_ffi::{FfiError, FfiEvent, FfiTier, MmpNode};
use mmp_types::consts::MAX_BUNDLE;

fn node(seed: u8) -> Arc<MmpNode> {
    MmpNode::new(vec![seed; 32], FfiTier::Relay, None).expect("valid in-memory node")
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

fn contact(
    a: &Arc<MmpNode>,
    b: &Arc<MmpNode>,
    link: u64,
    now: u64,
) -> (Vec<FfiEvent>, Vec<FfiEvent>) {
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

    (drain(a), drain(b))
}

fn received_message(events: Vec<FfiEvent>) -> (Vec<u8>, Vec<u8>, Vec<u8>, u8) {
    events
        .into_iter()
        .find_map(|event| match event {
            FfiEvent::MessageReceived {
                id,
                from,
                body,
                apptype,
                ..
            } => Some((id, from, body, apptype)),
            _ => None,
        })
        .expect("recipient emitted MessageReceived")
}

#[test]
fn direct_text_and_body_round_trip_after_reconnect() {
    let alice = node(101);
    let bob = node(102);
    introduce(&alice, &bob);
    drain(&alice);
    drain(&bob);

    // Establish the authenticated link before queuing application data. The
    // second contact is the reconnect that carries the first direct message.
    let (alice_events, bob_events) = contact(&alice, &bob, 700, 10_000);
    assert!(alice_events.iter().any(|event| matches!(
        event,
        FfiEvent::Established { peer } if *peer == bob.node_id()
    )));
    assert!(bob_events.iter().any(|event| matches!(
        event,
        FfiEvent::Established { peer } if *peer == alice.node_id()
    )));

    let text_id = alice
        .send_text(bob.node_id(), "text over the FFI".to_string())
        .expect("known peer accepts text");
    let (_, bob_events) = contact(&alice, &bob, 701, 20_000);
    let (id, from, body, apptype) = received_message(bob_events);
    assert_eq!(id, text_id);
    assert_eq!(from, alice.node_id());
    assert_eq!(body, b"text over the FFI");
    assert_eq!(apptype, 0, "text uses the built-in text body tag");

    // The first direct message is still awaiting its authenticated receipt;
    // a second body must be rejected without queuing work or an event.
    let pending_payload = b"must wait for receipt".to_vec();
    assert_eq!(
        alice.send_body(bob.node_id(), 0x42, pending_payload),
        Err(FfiError::ConversationPending)
    );
    assert!(
        alice.poll_send().is_none(),
        "pending body must not queue transport work"
    );
    assert!(
        alice.poll_event().is_none(),
        "pending body must not emit an event"
    );

    // A receipt contact closes the first-contact transaction before another
    // application body is admitted by the direct-message API.
    let (alice_events, _) = contact(&alice, &bob, 702, 30_000);
    assert!(alice_events.into_iter().any(|event| matches!(
        event,
        FfiEvent::Delivered { id } if id == text_id
    )));

    let payload = b"application body over the FFI".to_vec();
    let body_id = alice
        .send_body(bob.node_id(), 0x42, payload.clone())
        .expect("known peer accepts an application body");
    let (_, bob_events) = contact(&alice, &bob, 703, 40_000);
    let (id, from, body, apptype) = received_message(bob_events);
    assert_eq!(id, body_id);
    assert_eq!(from, alice.node_id());
    assert_eq!(body, payload);
    assert_eq!(apptype, 0x42);
}

#[test]
fn direct_api_rejects_unknown_or_oversized_input_without_work() {
    let alice = node(103);
    let bob = node(104);
    introduce(&alice, &bob);
    drain(&alice);
    drain(&bob);

    let stranger = vec![0xa5; 16];
    assert_eq!(
        alice.send_text(stranger, "not routable".to_string()),
        Err(FfiError::UnknownPeer)
    );
    assert!(
        alice.poll_send().is_none(),
        "unknown peer must not queue transport work"
    );
    assert!(
        alice.poll_event().is_none(),
        "unknown peer must not emit an event"
    );

    let oversized = "x".repeat(MAX_BUNDLE);
    assert_eq!(
        alice.send_text(bob.node_id(), oversized),
        Err(FfiError::BadArg)
    );
    assert!(
        alice.poll_send().is_none(),
        "oversized input must not queue transport work"
    );
    assert!(
        alice.poll_event().is_none(),
        "oversized input must not emit an event"
    );
}
