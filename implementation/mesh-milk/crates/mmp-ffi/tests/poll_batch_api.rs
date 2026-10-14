use mmp_ffi::{FfiEvent, FfiTier, MmpNode, OutFrame};

fn node(seed: u8, tier: FfiTier) -> std::sync::Arc<MmpNode> {
    MmpNode::new(vec![seed; 32], tier, None).expect("valid seed")
}

fn drain_singly(node: &MmpNode) -> Vec<OutFrame> {
    std::iter::from_fn(|| node.poll_send()).collect()
}

#[test]
fn send_batch_preserves_fifo_and_matches_single_item_polling() {
    let batched = node(201, FfiTier::Relay);
    let singly = node(201, FfiTier::Relay);

    batched.link_up(7, true);
    singly.link_up(7, true);

    assert!(batched.poll_send_batch(0).is_empty());
    let first = batched.poll_send_batch(1);
    assert!(first.len() <= 1);
    let mut rest = batched.poll_send_batch(64);
    assert!(rest.len() <= 64);

    let expected = drain_singly(&singly);
    let mut observed = first;
    observed.append(&mut rest);
    assert_eq!(observed.len(), expected.len());
    assert_eq!(
        observed.iter().map(|frame| frame.link).collect::<Vec<_>>(),
        expected.iter().map(|frame| frame.link).collect::<Vec<_>>(),
        "batch and single-item polling must preserve link FIFO"
    );

    // A request above the hard bound is clamped rather than allocating an
    // unbounded bridge result. The node's outbox is empty at this point.
    assert!(batched.poll_send_batch(u32::MAX).is_empty());
}

#[test]
fn event_batch_preserves_fifo_and_zero_limit_is_noop() {
    let node = node(202, FfiTier::Relay);
    while node.poll_event().is_some() {}
    node.set_tier(FfiTier::Leaf);
    node.set_tier(FfiTier::Relay);
    node.set_tier(FfiTier::Anchor);

    assert!(node.poll_event_batch(0).is_empty());
    let events = node.poll_event_batch(2);
    assert_eq!(events.len(), 2);
    assert!(matches!(
        events[0],
        FfiEvent::TierChanged {
            tier: FfiTier::Leaf
        }
    ));
    assert!(matches!(
        events[1],
        FfiEvent::TierChanged {
            tier: FfiTier::Relay
        }
    ));

    let remaining = node.poll_event_batch(u32::MAX);
    assert_eq!(remaining.len(), 1);
    assert!(matches!(
        remaining[0],
        FfiEvent::TierChanged {
            tier: FfiTier::Anchor
        }
    ));
    assert!(node.poll_event().is_none());
}

#[test]
fn event_batch_clamps_large_requests_to_the_hard_bound() {
    let node = node(204, FfiTier::Relay);
    while node.poll_event().is_some() {}
    for index in 0..100 {
        node.set_tier(if index % 2 == 0 {
            FfiTier::Leaf
        } else {
            FfiTier::Relay
        });
    }

    let first = node.poll_event_batch(u32::MAX);
    assert_eq!(first.len(), 64, "the bridge batch must be hard bounded");
    let remaining = node.poll_event_batch(u32::MAX);
    assert_eq!(remaining.len(), 36);
    assert!(node.poll_event().is_none());
}

#[test]
fn close_batch_preserves_fifo_and_is_hard_bounded() {
    let node = node(203, FfiTier::Leaf);
    node.link_up(1, true);
    while node.poll_send().is_some() {}

    // One admitted session leaves later links denied and queued for local
    // transport teardown. The node's close queue is larger than the FFI
    // batch bound, so this also exercises a multi-call drain.
    for link in 2..=65 {
        node.link_up(link, true);
    }

    assert!(node.poll_close_batch(0).is_empty());
    let first = node.poll_close_batch(3);
    assert_eq!(first, vec![2, 3, 4]);
    let rest = node.poll_close_batch(u32::MAX);
    assert_eq!(rest.len(), 61);
    assert_eq!(rest.first(), Some(&5));
    assert_eq!(rest.last(), Some(&65));
    assert!(node.poll_close().is_none());
}
