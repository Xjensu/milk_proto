//! Node integration tests (HANDOFF M3): two in-memory nodes exchange direct +
//! group messages, receipts purge; a 3-node chain relays store-carry-forward.

use mmp_core::store::{BundleMeta, SlotState};
use mmp_core::MemStore;
use mmp_tal::TalEvent;
use mmp_types::frame::MAX_ENVELOPE_PAYLOAD;
use mmp_types::authcert::AuthCert;
use mmp_types::bundle::{class, prio, Bundle};
use mmp_types::consts::{FRIENDSHIP_TTL_MS, MAX_BUNDLE};
use mmp_types::domain::SigDomain;
use mmp_types::ids::{Id16, Key32, NodeId, Sig64};
use mmp_types::payload::{apptype, AsyncPayload};

use crate::{DeliveryState, Event, Node, SendError, Tier};

fn node(seed: u8) -> Node {
    Node::new([seed; 32], Box::new(MemStore::new()), Tier::Relay)
}

/// Exchange identity claims out-of-band (the QR / first-sync bootstrap).
fn introduce(a: &mut Node, b: &mut Node) {
    for c in a.own_claims() {
        b.ingest_claim(c, true);
    }
    for c in b.own_claims() {
        a.ingest_claim(c, true);
    }
}

/// Bootstrap only the identity and exchange-key claims. This intentionally
/// omits PREKEY_BATCH so first-contact tests exercise the normative sealed-box
/// baseline rather than the optional X3DH upgrade.
fn introduce_exchange_only(a: &mut Node, b: &mut Node) {
    for c in a.own_claims() {
        if c.field == mmp_types::claim::field::EXCHANGE_KEY {
            b.ingest_claim(c, true);
        }
    }
    for c in b.own_claims() {
        if c.field == mmp_types::claim::field::EXCHANGE_KEY {
            a.ingest_claim(c, true);
        }
    }
}

/// Run one full contact between two nodes over an in-memory link.
fn contact(a: &mut Node, b: &mut Node, link: u64, now: u64) {
    a.tick(now);
    b.tick(now);
    a.on_tal_event(TalEvent::LinkUp { link, initiator: true, quality: Default::default() });
    b.on_tal_event(TalEvent::LinkUp { link, initiator: false, quality: Default::default() });
    for _ in 0..300 {
        let mut quiet = true;
        while let Some((l, bytes)) = a.poll_send() {
            quiet = false;
            b.on_tal_event(TalEvent::LinkData { link: l, bytes });
        }
        while let Some((l, bytes)) = b.poll_send() {
            quiet = false;
            a.on_tal_event(TalEvent::LinkData { link: l, bytes });
        }
        if quiet {
            break;
        }
    }
    a.on_tal_event(TalEvent::LinkDown { link });
    b.on_tal_event(TalEvent::LinkDown { link });
}

fn drain(n: &mut Node) -> Vec<Event> {
    std::iter::from_fn(|| n.poll_event()).collect()
}

fn link_up(n: &mut Node, link: u64) {
    n.on_tal_event(TalEvent::LinkUp {
        link,
        initiator: true,
        quality: Default::default(),
    });
}

#[test]
fn concurrent_session_admission_caps_leaf_relay_anchor() {
    for (seed, tier, cap) in [
        (101, Tier::Leaf, 1usize),
        (102, Tier::Relay, 2usize),
        (103, Tier::Anchor, 8usize),
    ] {
        let mut n = Node::new([seed; 32], Box::new(MemStore::new()), tier);
        for link in 0..cap {
            link_up(&mut n, link as u64);
        }
        assert_eq!(n.sessions.len(), cap, "tier {tier:?} accepted sessions");
        let queued_before_rejection = n.outbox.len();

        link_up(&mut n, 10_000);

        assert_eq!(
            n.sessions.len(),
            cap,
            "tier {tier:?} must retain its admitted sessions"
        );
        assert_eq!(
            n.outbox.len(),
            queued_before_rejection,
            "quota denial must not queue handshake output"
        );
        assert_eq!(n.poll_close(), Some(10_000), "denied physical link must be closed by host");
        assert_eq!(n.poll_close(), None);
    }
}

#[test]
fn duplicate_link_up_preserves_existing_session_and_output() {
    let mut n = Node::new([104; 32], Box::new(MemStore::new()), Tier::Relay);
    link_up(&mut n, 7);
    let queued_after_first = n.outbox.len();

    link_up(&mut n, 7);

    assert_eq!(n.sessions.len(), 1);
    assert_eq!(
        n.outbox.len(),
        queued_after_first,
        "duplicate LinkUp must not replace the engine or queue another handshake"
    );
    assert_eq!(n.poll_close(), None, "duplicate LinkUp must not request a close");
}

#[test]
fn close_action_queue_is_bounded_and_reports_saturation() {
    let mut n = Node::new([110; 32], Box::new(MemStore::new()), Tier::Leaf);
    link_up(&mut n, 1);
    for link in 2..=66 {
        link_up(&mut n, link);
    }

    let mut retained = 0;
    while n.poll_close().is_some() {
        retained += 1;
    }
    assert_eq!(retained, 64);
    assert_eq!(n.close_actions_dropped(), 1);
}

#[test]
fn outbound_queue_overflow_clears_only_the_affected_link() {
    let mut n = Node::new([107; 32], Box::new(MemStore::new()), Tier::Leaf);
    let unrelated = 99;
    for _ in 0..1_000 {
        n.outbox.push_back((unrelated, vec![0u8; MAX_ENVELOPE_PAYLOAD]));
    }

    link_up(&mut n, 1);

    assert!(!n.sessions.contains_key(&1), "overflow must terminate the affected session");
    assert!(!n.outbox.iter().any(|(link, _)| *link == 1), "affected-link frames must be cleared atomically");
    assert!(n.outbox.iter().any(|(link, _)| *link == unrelated), "unrelated queued traffic must survive");
    assert_eq!(n.poll_close(), Some(1));
}

#[test]
fn outbound_queue_per_link_cap_is_enforced() {
    let mut n = Node::new([108; 32], Box::new(MemStore::new()), Tier::Leaf);
    for _ in 0..12 {
        n.outbox.push_back((1, vec![0u8; MAX_ENVELOPE_PAYLOAD]));
    }

    link_up(&mut n, 1);

    assert!(!n.sessions.contains_key(&1));
    assert!(!n.outbox.iter().any(|(link, _)| *link == 1));
    assert_eq!(n.poll_close(), Some(1));
}

#[test]
fn outbound_queue_byte_cap_is_enforced_before_frame_cap() {
    let mut n = Node::new([109; 32], Box::new(MemStore::new()), Tier::Leaf);
    for _ in 0..11 {
        n.outbox.push_back((1, vec![0u8; MAX_ENVELOPE_PAYLOAD + 500]));
    }

    link_up(&mut n, 1);

    assert!(!n.sessions.contains_key(&1));
    assert!(!n.outbox.iter().any(|(link, _)| *link == 1));
    assert_eq!(n.poll_close(), Some(1));
}

#[test]
fn session_slot_releases_after_link_down() {
    let mut n = Node::new([105; 32], Box::new(MemStore::new()), Tier::Leaf);
    link_up(&mut n, 1);
    while n.poll_send().is_some() {}
    link_up(&mut n, 2);
    assert_eq!(n.sessions.len(), 1, "Leaf quota is one session");

    n.outbox.push_back((1, vec![0x55]));
    n.on_tal_event(TalEvent::LinkDown { link: 1 });
    assert!(n.sessions.is_empty());
    assert!(!n.outbox.iter().any(|(link, _)| *link == 1), "LinkDown must purge stale queued frames");

    link_up(&mut n, 2);
    assert_eq!(n.sessions.len(), 1, "LinkDown must release the admission slot");
}

#[test]
fn session_slot_releases_when_engine_closes() {
    let mut n = Node::new([106; 32], Box::new(MemStore::new()), Tier::Leaf);
    link_up(&mut n, 1);
    while n.poll_send().is_some() {}

    let oversized_declared_len = (MAX_ENVELOPE_PAYLOAD + 1) as u16;
    let mut malformed = vec![0u8; 3];
    malformed[..2].copy_from_slice(&oversized_declared_len.to_le_bytes());
    n.on_tal_event(TalEvent::LinkData {
        link: 1,
        bytes: malformed,
    });
    assert!(
        n.sessions.is_empty(),
        "a closed engine must be removed before a replacement is admitted"
    );
    assert_eq!(n.poll_close(), Some(1), "local session closure must reach the transport host");

    link_up(&mut n, 2);
    assert_eq!(n.sessions.len(), 1, "Closed must release the admission slot");
}

#[test]
fn direct_message_delivery_and_receipt() {
    let mut alice = node(1);
    let mut bob = node(2);
    introduce(&mut alice, &mut bob);
    drain(&mut alice);
    drain(&mut bob);

    let msg_id = alice.send_text(&bob.node_id(), "hello over the mesh").unwrap();
    assert_eq!(alice.message_state(&msg_id), Some(DeliveryState::Queued));

    // One contact: message crosses, receipt comes back in the same session
    // (receipt is created during Bob's ingest, then Bob's WANT-driven transfer
    // has already happened — so the receipt returns on the SECOND contact).
    contact(&mut alice, &mut bob, 1, 10_000);
    let bob_events = drain(&mut bob);
    let received = bob_events.iter().find_map(|e| match e {
        Event::MessageReceived { body, from, .. } => Some((body.clone(), *from)),
        _ => None,
    });
    let (body, from) = received.expect("bob received the message");
    assert_eq!(body, b"hello over the mesh");
    assert_eq!(from, alice.node_id());

    contact(&mut alice, &mut bob, 2, 20_000);
    let alice_events = drain(&mut alice);
    assert!(
        alice_events.iter().any(|e| matches!(e, Event::Delivered { id } if *id == msg_id)),
        "alice got the delivery receipt: {alice_events:?}"
    );
    assert_eq!(alice.message_state(&msg_id), Some(DeliveryState::Delivered));
}

#[test]
fn sealed_first_contact_delivers_and_returns_receipt_without_prekeys() {
    let mut alice = node(41);
    let mut bob = node(42);
    introduce_exchange_only(&mut alice, &mut bob);
    drain(&mut alice);
    drain(&mut bob);

    let msg_id = alice.send_text(&bob.node_id(), "sealed baseline").unwrap();
    let (_, raw) = alice.core.store.get(&msg_id).expect("sealed bundle stored");
    let bundle = Bundle::decode_canonical(&raw).unwrap();
    assert!(matches!(
        AsyncPayload::decode(&bundle.payload).unwrap(),
        AsyncPayload::Sealed { .. }
    ));

    contact(&mut alice, &mut bob, 41, 10_000);
    let bob_events = drain(&mut bob);
    assert!(
        bob_events
            .iter()
            .any(|event| matches!(event, Event::MessageReceived { body, .. } if body == b"sealed baseline")),
        "sealed first-contact message was not delivered: {bob_events:?}"
    );

    contact(&mut alice, &mut bob, 42, 20_000);
    let alice_events = drain(&mut alice);
    assert!(
        alice_events.iter().any(|event| matches!(event, Event::Delivered { id } if *id == msg_id)),
        "sealed first-contact receipt was not delivered: {alice_events:?}"
    );
}

#[test]
fn malformed_prekey_async_does_not_commit_ratchet_state() {
    let mut alice = node(43);
    let mut bob = node(44);
    introduce(&mut alice, &mut bob);
    let msg_id = alice.send_text(&bob.node_id(), "candidate state").unwrap();
    let (_, raw) = alice.core.store.get(&msg_id).expect("prekey bundle stored");
    let bundle = Bundle::decode_canonical(&raw).unwrap();
    let conv_id = mmp_crypto::conversation_id(&alice.core.public, &bob.core.public);

    let mut forged = bundle.clone();
    let AsyncPayload::Prekey { prekey_id, sender_xpk, eph, mut ct } =
        AsyncPayload::decode(&forged.payload).unwrap()
    else {
        panic!("full-claim bootstrap should use prekey async form");
    };
    ct[0] ^= 0x80;
    forged.payload = AsyncPayload::Prekey { prekey_id, sender_xpk, eph, ct }.encode();
    forged.signature = alice.identity.sign(SigDomain::Bundle, &forged.canonical_zeroed());

    assert!(bob.core.open_async(&forged, &conv_id, &alice.core.public).is_none());
    assert!(!bob.core.convs.contains_key(&conv_id), "failed AEAD must not commit Conv");
    assert!(bob.core.open_async(&bundle, &conv_id, &alice.core.public).is_some());
    assert!(bob.core.convs.contains_key(&conv_id), "valid async must still establish Conv");
}

#[test]
fn sealed_async_cannot_replace_established_conversation() {
    let mut alice = node(45);
    let mut bob = node(46);
    introduce_exchange_only(&mut alice, &mut bob);
    let first = alice.send_text(&bob.node_id(), "first root").unwrap();
    contact(&mut alice, &mut bob, 45, 10_000);
    drain(&mut alice);
    drain(&mut bob);

    let conv_id = mmp_crypto::conversation_id(&alice.core.public, &bob.core.public);
    let original_root = bob.core.convs.get(&conv_id).expect("conversation established").ck0;
    assert!(bob.core.established_convs.contains(&conv_id));

    let second = alice
        .core
        .send_sealed(&bob.node_id(), &bob.core.public, conv_id, apptype::TEXT, b"stale root")
        .unwrap();
    assert_ne!(first, second);
    let (_, raw) = alice.core.store.get(&second).unwrap();
    let sealed = Bundle::decode_canonical(&raw).unwrap();
    assert!(bob.core.open_async(&sealed, &conv_id, &alice.core.public).is_none());
    assert_eq!(bob.core.convs.get(&conv_id).unwrap().ck0, original_root);
}

#[test]
fn oversized_prekey_first_send_does_not_poison_async_state() {
    let mut alice = node(47);
    let mut bob = node(48);
    introduce(&mut alice, &mut bob);

    let oversized = "x".repeat(MAX_BUNDLE);
    assert_eq!(alice.send_text(&bob.node_id(), &oversized), Err(SendError::TooLarge));

    let msg_id = alice.send_text(&bob.node_id(), "retry after rejection").unwrap();
    let (_, raw) = alice.core.store.get(&msg_id).unwrap();
    let bundle = Bundle::decode_canonical(&raw).unwrap();
    assert!(matches!(
        AsyncPayload::decode(&bundle.payload).unwrap(),
        AsyncPayload::Prekey { .. }
    ));

    contact(&mut alice, &mut bob, 47, 10_000);
    let bob_events = drain(&mut bob);
    assert!(
        bob_events
            .iter()
            .any(|event| matches!(event, Event::MessageReceived { body, .. } if body == b"retry after rejection")),
        "retry must decrypt after rejected oversized first send: {bob_events:?}"
    );
}

#[test]
fn second_first_contact_send_is_explicitly_rejected_until_receipt() {
    let mut alice = node(49);
    let mut bob = node(50);
    introduce_exchange_only(&mut alice, &mut bob);

    let first = alice.send_text(&bob.node_id(), "first sealed root").unwrap();
    assert_eq!(
        alice.send_text(&bob.node_id(), "must wait"),
        Err(SendError::ConversationPending)
    );

    contact(&mut alice, &mut bob, 49, 10_000);
    assert!(drain(&mut bob).iter().any(|event| {
        matches!(event, Event::MessageReceived { id, body, .. } if *id == first && body == b"first sealed root")
    }));
    assert_eq!(
        alice.send_text(&bob.node_id(), "still waiting for receipt"),
        Err(SendError::ConversationPending)
    );

    contact(&mut alice, &mut bob, 50, 20_000);
    assert!(drain(&mut alice).iter().any(|event| matches!(event, Event::Delivered { id } if *id == first)));
    let second = alice.send_text(&bob.node_id(), "now direct").unwrap();
    let (_, raw) = alice.core.store.get(&second).unwrap();
    let bundle = Bundle::decode_canonical(&raw).unwrap();
    assert!(mmp_types::payload::DirectPayload::decode(&bundle.payload).is_ok());
}

/// E4-S4 bump_epoch wiring (doc 08 §5.2/§5.3): the very first live session
/// with a brand-new conversation must NOT bump (nothing to heal yet — the
/// bug this drill caught: bumping too early, before that first session's own
/// async message is even processed, desyncs the two sides); every REPEAT
/// contact after that must bump both sides symmetrically, and messages sent
/// under the bumped epoch must still decrypt correctly.
#[test]
fn ratchet_epoch_bumps_on_repeat_contact_and_messages_still_decrypt() {
    let mut alice = node(1);
    let mut bob = node(2);
    introduce(&mut alice, &mut bob);
    drain(&mut alice);
    drain(&mut bob);

    // Contact 1: brand-new Conv (async X3DH, first-ever contact) — no bump.
    alice.send_text(&bob.node_id(), "first").unwrap();
    contact(&mut alice, &mut bob, 1, 10_000);
    drain(&mut alice);
    let bob_events = drain(&mut bob);
    assert!(
        bob_events.iter().any(|e| matches!(e, Event::MessageReceived { body, .. } if body == b"first")),
        "the first message must cross before any epoch bump: {bob_events:?}"
    );
    assert_eq!(
        alice.conv_epoch(&bob.node_id()),
        Some((0, 0)),
        "first-ever contact must not bump the epoch"
    );
    assert_eq!(bob.conv_epoch(&alice.node_id()), Some((0, 0)));

    // Contact 2: both sides have now met before — bump fires, symmetrically.
    contact(&mut alice, &mut bob, 2, 20_000);
    drain(&mut alice);
    drain(&mut bob);
    assert_eq!(
        alice.conv_epoch(&bob.node_id()),
        Some((1, 1)),
        "a repeat contact must bump the epoch"
    );
    assert_eq!(
        bob.conv_epoch(&alice.node_id()),
        Some((1, 1)),
        "both sides must bump in lockstep, or decryption breaks"
    );

    // A message sent AFTER the bump must still cross correctly under epoch 1.
    alice.send_text(&bob.node_id(), "second, after the bump").unwrap();
    contact(&mut alice, &mut bob, 3, 30_000);
    let bob_events2 = drain(&mut bob);
    assert!(
        bob_events2
            .iter()
            .any(|e| matches!(e, Event::MessageReceived { body, .. } if body == b"second, after the bump")),
        "a message encrypted under the bumped epoch must still decrypt: {bob_events2:?}"
    );
    assert_eq!(alice.conv_epoch(&bob.node_id()), Some((2, 2)), "the third contact bumps again");
}

#[test]
fn three_node_store_carry_forward() {
    // A meets R (relay), then R meets B — the DTN path over space AND time.
    let mut a = node(1);
    let mut r = node(3);
    let mut b = node(2);
    introduce(&mut a, &mut b); // A knows B's claims (met before / QR)

    let msg_id = a.send_text(&b.node_id(), "carried across time").unwrap();

    contact(&mut a, &mut r, 1, 10_000); // A → R (spray hand-off)
    drain(&mut a);
    drain(&mut r);
    assert!(r.store_len() > 0, "relay carries bundles");

    contact(&mut r, &mut b, 2, 60_000); // R → B (delivery)
    let b_events = drain(&mut b);
    assert!(
        b_events
            .iter()
            .any(|e| matches!(e, Event::MessageReceived { body, .. } if body == b"carried across time")),
        "B received via relay: {b_events:?}"
    );

    // Receipt back-propagates: B → R → A.
    contact(&mut r, &mut b, 3, 70_000);
    contact(&mut a, &mut r, 4, 80_000);
    drain(&mut r);
    let a_events = drain(&mut a);
    assert!(
        a_events.iter().any(|e| matches!(e, Event::Delivered { id } if *id == msg_id)),
        "receipt reached the origin via the relay: {a_events:?}"
    );
}

#[test]
fn spray_budget_flows_and_message_marked_in_mesh() {
    let mut a = node(1);
    let mut r = node(3);
    let mut b = node(2);
    introduce(&mut a, &mut b);
    let id = a.send_text(&b.node_id(), "x").unwrap();
    contact(&mut a, &mut r, 1, 10_000);
    let evs = drain(&mut a);
    assert!(
        evs.iter().any(|e| matches!(
            e,
            Event::DeliveryProgress { id: i, state: DeliveryState::InMesh } if *i == id
        )),
        "own message flips to InMesh after first hand-off: {evs:?}"
    );
}

#[test]
fn sender_key_member_must_match_authenticated_origin() {
    let mut alice = node(1);
    let mut bob = node(2);
    introduce(&mut alice, &mut bob);

    let gid = alice.create_group();
    let dist = alice.core.groups.get(&gid).unwrap().local.dist().encode();
    let mut forged = Vec::with_capacity(16 + dist.len());
    forged.extend_from_slice(&bob.node_id().0);
    forged.extend_from_slice(&dist);
    alice.core.send_app(&bob.node_id(), apptype::SENDER_KEY, &forged).unwrap();

    contact(&mut alice, &mut bob, 91, 910_000);
    let events = drain(&mut bob);
    assert!(!bob.core.groups.contains_key(&gid), "forged sender-key must not auto-adopt a group");
    assert!(!events.iter().any(|event| matches!(event, Event::GroupInvite { gid: g, .. } if *g == gid)));
}

#[test]
fn removed_member_cannot_reintroduce_stale_sender_key() {
    let mut alice = node(11);
    let mut bob = node(12);
    introduce(&mut alice, &mut bob);

    let gid = alice.create_group();
    alice.group_invite(&gid, &bob.node_id()).unwrap();
    contact(&mut alice, &mut bob, 92, 920_000);
    drain(&mut alice);
    drain(&mut bob);

    let old_dist = bob.core.groups.get(&gid).unwrap().local.dist().encode();
    alice.group_remove(&gid, &bob.node_id()).unwrap();
    let mut stale = Vec::with_capacity(16 + old_dist.len());
    stale.extend_from_slice(&bob.node_id().0);
    stale.extend_from_slice(&old_dist);
    bob.core.send_app(&alice.node_id(), apptype::SENDER_KEY, &stale).unwrap();

    contact(&mut bob, &mut alice, 93, 930_000);
    assert!(!alice.group_members(&gid).contains(&bob.node_id()));
}

#[test]
fn group_create_invite_and_message() {
    let mut alice = node(1);
    let mut bob = node(2);
    let mut carol = node(4);
    introduce(&mut alice, &mut bob);
    introduce(&mut alice, &mut carol);
    introduce(&mut bob, &mut carol);

    let gid = alice.create_group();
    alice.group_invite(&gid, &bob.node_id()).unwrap();
    alice.group_invite(&gid, &carol.node_id()).unwrap();

    // Dist messages travel as ordinary class-1 bundles: one contact each.
    contact(&mut alice, &mut bob, 1, 10_000);
    contact(&mut alice, &mut carol, 2, 20_000);
    // Each member distributes only its own key. No sender-key provenance is
    // inferred from an admin relay.
    contact(&mut alice, &mut carol, 3, 30_000);
    contact(&mut alice, &mut bob, 4, 40_000);

    let bob_events = drain(&mut bob);
    assert!(
        bob_events.iter().any(|e| matches!(e, Event::GroupInvite { gid: g, .. } if *g == gid)),
        "bob saw the invite: {bob_events:?}"
    );

    // Alice posts; Bob and Carol read it after sync.
    alice.group_send(&gid, "hello group").unwrap();
    contact(&mut alice, &mut bob, 5, 50_000);
    contact(&mut alice, &mut carol, 6, 60_000);
    let b_events = drain(&mut bob);
    let c_events = drain(&mut carol);
    for (who, evs) in [("bob", &b_events), ("carol", &c_events)] {
        assert!(
            evs.iter().any(
                |e| matches!(e, Event::GroupMessage { gid: g, body, .. } if *g == gid && body == b"hello group")
            ),
            "{who} read the group message: {evs:?}"
        );
    }

    // Removal rotates the epoch; the removed member reads nothing new.
    alice.group_remove(&gid, &carol.node_id()).unwrap();
    contact(&mut alice, &mut bob, 7, 70_000);
    alice.group_send(&gid, "carol is out").unwrap();
    contact(&mut alice, &mut bob, 8, 80_000);
    contact(&mut alice, &mut carol, 9, 90_000);
    let b2: Vec<Event> = drain(&mut bob);
    let c2: Vec<Event> = drain(&mut carol);
    assert!(
        b2.iter()
            .any(|e| matches!(e, Event::GroupMessage { body, .. } if body == b"carol is out")),
        "bob reads the new epoch: {b2:?}"
    );
    assert!(
        !c2.iter()
            .any(|e| matches!(e, Event::GroupMessage { body, .. } if body == b"carol is out")),
        "carol must NOT read the new epoch: {c2:?}"
    );
}

#[test]
fn blocked_peer_is_silently_denied() {
    let mut alice = node(1);
    let mut bob = node(2);
    introduce(&mut alice, &mut bob);
    alice.block(bob.node_id());
    contact(&mut alice, &mut bob, 1, 10_000);
    let evs = drain(&mut alice);
    assert!(
        !evs.iter().any(|e| matches!(e, Event::Established { .. })),
        "no session with a blocked peer: {evs:?}"
    );
}

/// Handbook §4.6 acceptance case: "session attempt from a revoked key is
/// silently denied." Mirrors `blocked_peer_is_silently_denied`'s shape exactly
/// — a revocation reaches Alice the same way a QR-ceremony claim would (here,
/// out-of-band via `own_claims`/`ingest_claim`, same as `introduce`), and the
/// next contact attempt must not establish.
#[test]
fn revoked_peer_session_is_silently_denied() {
    let mut alice = node(1);
    let mut bob = node(2);
    introduce(&mut alice, &mut bob);
    drain(&mut alice);
    drain(&mut bob);

    bob.revoke_identity(0);
    for c in bob.own_claims() {
        alice.ingest_claim(c, true);
    }
    drain(&mut alice);
    drain(&mut bob);

    contact(&mut alice, &mut bob, 1, 10_000);
    let evs = drain(&mut alice);
    assert!(
        !evs.iter().any(|e| matches!(e, Event::Established { .. })),
        "no session with a revoked peer: {evs:?}"
    );
}

/// Handbook §4.6: a prekey batch already on file for a peer must not be used
/// to initiate X3DH once that peer is revoked — even though the tombstone
/// merge rule only blocks NEW claims, not ones already stored before the
/// revocation arrived.
#[test]
fn revoked_peers_stored_prekey_batch_is_not_used_to_initiate() {
    let mut alice = node(1);
    let mut bob = node(2);
    introduce(&mut alice, &mut bob); // alice already holds bob's prekey batch
    drain(&mut alice);
    drain(&mut bob);

    bob.revoke_identity(0);
    for c in bob.own_claims() {
        alice.ingest_claim(c, true);
    }
    drain(&mut alice);

    let err = alice.send_text(&bob.node_id(), "too late").unwrap_err();
    assert_eq!(err, SendError::PeerRevoked);
}

/// A valid succession counter-signature surfaces on `Event::PeerRevoked`;
/// this exercises the full gossip path (not just `ClaimStore::successor`
/// directly, which `mmp-core`'s own tests already cover).
#[test]
fn peer_revoked_event_carries_the_verified_successor() {
    let mut alice = node(1);
    let mut bob = node(2);
    let carol = node(3); // bob's chosen successor
    introduce(&mut alice, &mut bob);
    drain(&mut alice);
    drain(&mut bob);

    // Sign the countersig with carol's own identity (same seed `node(3)` used
    // to construct her `Node` — deterministic, so this is genuinely "carol's
    // key," not a stand-in).
    let carol_identity = mmp_crypto::Identity::from_seed(&[3; 32]);
    let sig = carol_identity.sign(
        SigDomain::Claim,
        &mmp_types::claim::succession_signed_object(&bob.public_key(), &carol.public_key()),
    );
    let body = mmp_types::claim::Revocation {
        revoked_key: bob.public_key(),
        reason: 0,
        successor: Some(carol.public_key()),
        successor_countersig: Some(sig),
    };
    bob.publish_claim(mmp_types::claim::field::REVOCATION, 1, body.encode());
    for c in bob.own_claims() {
        alice.ingest_claim(c, true);
    }
    let evs = drain(&mut alice);
    assert!(
        evs.iter().any(|e| matches!(
            e,
            Event::PeerRevoked { peer, successor: Some(s) }
                if *peer == bob.node_id() && *s == carol.public_key()
        )),
        "expected a verified successor on PeerRevoked: {evs:?}"
    );
}

#[test]
fn revoke_identity_with_successor_publishes_signed_successor_claim() {
    let mut bob = node(2);
    let carol = node(3);
    let carol_identity = mmp_crypto::Identity::from_seed(&[3; 32]);
    let countersig = carol_identity.sign(
        SigDomain::Claim,
        &mmp_types::claim::succession_signed_object(&bob.public_key(), &carol.public_key()),
    );

    let claim_bytes = bob.revoke_identity_with_successor(7, carol.public_key(), countersig);
    let claim = mmp_types::claim::Claim::decode(&claim_bytes).expect("well-formed claim");
    assert_eq!(claim.field, mmp_types::claim::field::REVOCATION);
    assert_eq!(claim.owner, bob.public_key());
    let revocation = mmp_types::claim::Revocation::decode(&claim.body).expect("well-formed body");
    assert_eq!(revocation.revoked_key, bob.public_key());
    assert_eq!(revocation.reason, 7);
    assert_eq!(revocation.successor, Some(carol.public_key()));
    assert_eq!(revocation.successor_countersig, Some(countersig));
}

#[test]
fn beacon_rotates_with_slot() {
    let n = node(1);
    let b1 = n.beacon(0);
    let b2 = n.beacon(16 * 60 * 1000); // next 15-min slot
    let e1 = &b1[1..9];
    let e2 = &b2[1..9];
    assert_ne!(e1, e2, "ephem_id rotates across slots");
    let same_slot = n.beacon(1000);
    assert_eq!(&b1[1..9], &same_slot[1..9]);
}

/// Compile-time check: NodeId is what the public API trades in.
#[allow(dead_code)]
fn api_uses_nodeids(n: &Node) -> NodeId {
    n.node_id()
}

// ------------------------------------------------------- E5-S5: doc 18 -----

fn signed_cert(
    issuer: &mmp_crypto::Identity,
    subject: Key32,
    scope: Id16,
    caps: u8,
    priority_budget: u16,
    not_after: u32,
) -> AuthCert {
    let mut cert = AuthCert {
        suite: 1,
        subject,
        scope,
        caps,
        priority_budget,
        not_after,
        issuer: issuer.public(),
        sig: Sig64::ZERO,
    };
    cert.sig = issuer.sign(SigDomain::AuthCert, &cert.signed_object());
    cert
}

#[test]
fn unauthenticated_announcement_clamps_to_normal_but_still_reaches_subscribers() {
    let mut alice = node(1);
    let mut bob = node(2);
    let scope = bob.subscribe_scope(b"campus/example");
    // Bob subscribes but pins no root: alice's announcement carries no cert
    // and no issuer is trusted for this scope — must clamp, never drop.
    alice.publish_announcement(scope, 1, prio::OPERATIONAL, b"no authority yet", None).unwrap();

    contact(&mut alice, &mut bob, 1, 10_000);
    let evs = drain(&mut bob);
    let ann = evs.iter().find_map(|e| match e {
        Event::Announcement { effective_priority, official, text, .. } => {
            Some((*effective_priority, *official, text.clone()))
        }
        _ => None,
    });
    let (effective, official, text) = ann.expect("bob (subscribed) sees the announcement event");
    assert_eq!(text, b"no authority yet");
    assert_eq!(effective, prio::NORMAL, "clamped: no trusted root, no attached cert");
    assert!(!official);
}

#[test]
fn unsubscribed_scope_gets_no_announcement_event() {
    let mut alice = node(1);
    let mut bob = node(2);
    let scope = Id16([0x42; 16]); // bob never subscribes to this one
    alice.publish_announcement(scope, 0, prio::EMERGENCY, b"evacuate", None).unwrap();

    contact(&mut alice, &mut bob, 1, 10_000);
    let evs = drain(&mut bob);
    assert!(
        !evs.iter().any(|e| matches!(e, Event::Announcement { .. })),
        "unsubscribed scope must not surface an event, even though the bundle still relays: {evs:?}"
    );
}

#[test]
fn valid_attached_cert_elevates_priority_and_renders_official() {
    let mut alice = node(1);
    let mut bob = node(2);
    let authority = mmp_crypto::Identity::from_seed(&[9; 32]);
    let scope = bob.subscribe_scope(b"train/nationalrail/1234");
    bob.pin_authority_root(scope, authority.public());

    let cert = signed_cert(
        &authority,
        alice.public_key(),
        scope,
        mmp_types::authcert::CAP_ANNOUNCE,
        10,
        1_000_000_000,
    );
    bob.set_wall_clock(0);
    alice.publish_announcement(scope, 1, prio::OPERATIONAL, b"platform 3 delayed", Some(&cert)).unwrap();

    contact(&mut alice, &mut bob, 1, 10_000);
    let evs = drain(&mut bob);
    let ann = evs.iter().find_map(|e| match e {
        Event::Announcement { effective_priority, official, .. } => Some((*effective_priority, *official)),
        _ => None,
    });
    let (effective, official) = ann.expect("bob sees the elevated announcement");
    assert_eq!(effective, prio::OPERATIONAL, "valid cert + budget ⇒ declared priority honored");
    assert!(official, "a valid unrevoked cert with CAP_ANNOUNCE renders official regardless of the bucket");
}

#[test]
fn per_authority_budget_caps_elevation_across_distinct_announcements() {
    let mut alice = node(1);
    let mut bob = node(2);
    let authority = mmp_crypto::Identity::from_seed(&[9; 32]);
    let scope = bob.subscribe_scope(b"train/nationalrail/1234");
    bob.pin_authority_root(scope, authority.public());
    bob.set_wall_clock(0);

    // Budget = 2/hour: exactly two of three distinct announcements elevate.
    let cert =
        signed_cert(&authority, alice.public_key(), scope, mmp_types::authcert::CAP_ANNOUNCE, 2, 1_000_000_000);
    for text in [&b"delay one"[..], b"delay two", b"delay three"] {
        alice.publish_announcement(scope, 1, prio::OPERATIONAL, text, Some(&cert)).unwrap();
    }

    contact(&mut alice, &mut bob, 1, 10_000);
    let evs = drain(&mut bob);
    let elevated = evs
        .iter()
        .filter(|e| matches!(e, Event::Announcement { effective_priority, .. } if *effective_priority == prio::OPERATIONAL))
        .count();
    let clamped = evs
        .iter()
        .filter(|e| matches!(e, Event::Announcement { effective_priority, .. } if *effective_priority == prio::NORMAL))
        .count();
    assert_eq!((elevated, clamped), (2, 1), "budget=2 admits exactly two of three: {evs:?}");
}

#[test]
fn revocation_gossip_clamps_a_previously_elevated_subject_and_stays_sticky() {
    let mut alice = node(1);
    let mut bob = node(2);
    let authority = mmp_crypto::Identity::from_seed(&[9; 32]);
    let scope = bob.subscribe_scope(b"train/nationalrail/1234");
    bob.pin_authority_root(scope, authority.public());
    bob.set_wall_clock(0);

    let cert = signed_cert(
        &authority,
        alice.public_key(),
        scope,
        mmp_types::authcert::CAP_ANNOUNCE,
        10,
        1_000_000_000,
    );
    alice.publish_announcement(scope, 1, prio::OPERATIONAL, b"first, elevated", Some(&cert)).unwrap();
    contact(&mut alice, &mut bob, 1, 10_000);
    let evs = drain(&mut bob);
    assert!(
        evs.iter().any(|e| matches!(e, Event::Announcement { effective_priority, .. } if *effective_priority == prio::OPERATIONAL)),
        "sanity: first announcement elevates: {evs:?}"
    );

    // Issuer revokes alice for this scope via ordinary claim gossip (field 0x08).
    let revoke = mmp_types::authcert::RevocationList {
        suite: 1,
        entries: vec![mmp_types::authcert::RevEntry {
            subject: alice.public_key(),
            scope,
            not_after: 999_999_999,
        }],
    };
    let mut claim = mmp_types::claim::Claim {
        owner: authority.public(),
        field: mmp_types::claim::field::REVOCATION_LIST,
        seq: 1,
        body: revoke.encode(),
        sig: Sig64::ZERO,
    };
    claim.sig = authority.sign(SigDomain::Claim, &claim.signed_object());
    bob.ingest_claim(claim.clone(), true);

    alice.publish_announcement(scope, 1, prio::OPERATIONAL, b"second, revoked", Some(&cert)).unwrap();
    contact(&mut alice, &mut bob, 2, 20_000);
    let evs2 = drain(&mut bob);
    let ann2 = evs2.iter().find_map(|e| match e {
        Event::Announcement { text, effective_priority, .. } if text == b"second, revoked" => Some(*effective_priority),
        _ => None,
    });
    assert_eq!(ann2, Some(prio::NORMAL), "revoked subject clamps even with a still-valid, still-attached cert");

    // A later revocation list that OMITS the entry must not un-revoke it.
    let later_empty =
        mmp_types::authcert::RevocationList { suite: 1, entries: vec![] };
    let mut claim2 = mmp_types::claim::Claim {
        owner: authority.public(),
        field: mmp_types::claim::field::REVOCATION_LIST,
        seq: 2,
        body: later_empty.encode(),
        sig: Sig64::ZERO,
    };
    claim2.sig = authority.sign(SigDomain::Claim, &claim2.signed_object());
    bob.ingest_claim(claim2, true);

    alice.publish_announcement(scope, 1, prio::OPERATIONAL, b"third, still revoked", Some(&cert)).unwrap();
    contact(&mut alice, &mut bob, 3, 30_000);
    let evs3 = drain(&mut bob);
    let ann3 = evs3.iter().find_map(|e| match e {
        Event::Announcement { text, effective_priority, .. } if text == b"third, still revoked" => {
            Some(*effective_priority)
        }
        _ => None,
    });
    assert_eq!(ann3, Some(prio::NORMAL), "sticky: an omitting later list must not restore admission");
}

// -------------------------------------------------- E5-S2: friendship ------
// Handbook §4.5 acceptance. `n.core.<field>` reaches into private `NodeCore`
// state directly — this test module is a descendant of the crate root where
// `Node`/`NodeCore` are defined, so ordinary Rust privacy allows it; used
// here (a) to fabricate deterministic "junk" quota pressure without relying
// on the router's probabilistic spray timing, and (b) to assert on
// patron-side pin bookkeeping (`friends[dependent].buffered`/`quota_used`)
// that has no other public surface.

/// Drop a synthetic, unsigned bundle straight into `n`'s store — bypassing
/// ingest entirely. `enforce_quota`/eviction only look at `BundleMeta`, never
/// bundle content, so this is a faithful, deterministic stand-in for "some
/// unrelated relay traffic filled the store" without depending on the
/// router's probabilistic spray/utility timing to generate real pressure.
fn inject_junk(n: &mut Node, id: Id16, size: u32, now: u64) {
    let meta = BundleMeta {
        id,
        class: class::DIRECT,
        origin: Id16([0xEE; 16]),
        dest: Id16([0xEE; 16]), // an unrelated stranger — never anyone's friend
        priority: 1,
        effective_priority: 1,
        size,
        stored_at: now,
        expires_at: now + mmp_types::consts::LIFETIME_MESSAGE_MS,
        budget: 1,
        own_origin: false,
        slot: SlotState::Stored,
    };
    n.core.store.put(meta, &vec![0u8; size as usize]);
}

/// The handbook §4.5 positive acceptance case: A sends class-1 to phone B
/// while B is offline; F (B's patron) buffers it, pinned past its own
/// eviction pressure; B comes online, syncs from F, and B's receipt purges
/// (and unpins) F's copy.
#[test]
fn friend_buffers_pinned_bundle_through_quota_pressure_then_unpins_on_receipt() {
    let mut a = node(1); // sender
    let mut b = node(2); // dependent (offline phone)
    let mut f = node(3); // patron (always-on PC)
    introduce(&mut a, &mut b);
    drain(&mut a);
    drain(&mut b);

    // Friendship handshake: F whitelists B, B solicits F as patron, one
    // contact carries FRIEND_OFFER/FRIEND_ACK (piggybacked right after
    // SUMMARY, handbook §4.5) and mints B's field-0x05 claim.
    f.allow_friendship_from(b.public_key());
    b.request_friendship(f.public_key());
    contact(&mut b, &mut f, 1, 10_000);
    let b_events = drain(&mut b);
    let f_events = drain(&mut f);
    assert!(
        b_events.iter().any(
            |e| matches!(e, Event::FriendshipEstablished { peer, .. } if *peer == f.node_id())
        ),
        "dependent sees FriendshipEstablished: {b_events:?}"
    );
    assert!(
        f_events.iter().any(
            |e| matches!(e, Event::FriendshipEstablished { peer, .. } if *peer == b.node_id())
        ),
        "patron sees FriendshipEstablished: {f_events:?}"
    );

    // A sends to B while B is offline; the message relays onto F (ordinary
    // D3 spray path, same mechanics as `three_node_store_carry_forward`),
    // where F's friend-buffer admission pins it for B.
    let msg_id = a.send_text(&b.node_id(), "buffered while offline").unwrap();
    contact(&mut a, &mut f, 2, 20_000);
    drain(&mut a);
    drain(&mut f);
    assert!(f.core.store.meta(&msg_id).is_some(), "sanity: F carries the message");
    assert!(
        f.core.friends.get(&b.node_id()).unwrap().buffered.contains(&msg_id),
        "the message is pinned in F's friend-buffer bookkeeping"
    );

    // Quota pressure: shrink F's quota to the smallest possible value and
    // pile on junk relay traffic — anything NOT pinned must go.
    f.core.quota = 1;
    inject_junk(&mut f, Id16([0xA1; 16]), 2000, 20_000);
    inject_junk(&mut f, Id16([0xA2; 16]), 2000, 20_000);
    inject_junk(&mut f, Id16([0xA3; 16]), 2000, 20_000);
    f.tick(21_000);

    assert!(f.core.store.meta(&Id16([0xA1; 16])).is_none(), "junk #1 evicted under pressure");
    assert!(f.core.store.meta(&Id16([0xA2; 16])).is_none(), "junk #2 evicted under pressure");
    assert!(f.core.store.meta(&Id16([0xA3; 16])).is_none(), "junk #3 evicted under pressure");
    assert!(
        f.core.store.meta(&msg_id).is_some(),
        "the friend-pinned bundle survives even quota=1 eviction pressure"
    );

    // B comes online and syncs from F: the buffered message delivers.
    contact(&mut b, &mut f, 3, 30_000);
    let b_events = drain(&mut b);
    assert!(
        b_events.iter().any(
            |e| matches!(e, Event::MessageReceived { body, .. } if body == b"buffered while offline")
        ),
        "B received the buffered message via F: {b_events:?}"
    );

    // B's receipt (created during that ingest) propagates back to F on the
    // NEXT contact (same WANT-driven-ordering shape as
    // `direct_message_delivery_and_receipt`) — purging and unpinning F's copy.
    contact(&mut b, &mut f, 4, 40_000);
    drain(&mut b);
    drain(&mut f);

    assert!(f.core.store.meta(&msg_id).is_none(), "F's copy purged once B's receipt arrives");
    let state = f.core.friends.get(&b.node_id()).expect("friendship still on file");
    assert!(state.buffered.is_empty(), "unpinned: buffered set empty after purge");
    assert_eq!(state.quota_used, 0, "quota_used refunded after purge");
}

/// The handbook §4.5 negative acceptance case: under the same quota
/// pressure, a bundle carried for a NON-friend has no pin protection and
/// evicts like any other relay copy.
#[test]
fn non_friend_bundle_evicts_under_the_same_quota_pressure() {
    let mut a = node(1);
    let mut b = node(2);
    let mut f = node(3); // relay only — no friendship with b this time
    introduce(&mut a, &mut b);
    drain(&mut a);
    drain(&mut b);

    let msg_id = a.send_text(&b.node_id(), "not buffered").unwrap();
    contact(&mut a, &mut f, 1, 10_000);
    drain(&mut a);
    drain(&mut f);
    assert!(f.core.store.meta(&msg_id).is_some(), "sanity: F still carries a plain relay copy");
    assert!(f.core.friends.is_empty(), "sanity: no friendship exists at F");

    f.core.quota = 1;
    inject_junk(&mut f, Id16([0xB1; 16]), 2000, 10_000);
    inject_junk(&mut f, Id16([0xB2; 16]), 2000, 10_000);
    f.tick(11_000);

    assert!(
        f.core.store.meta(&msg_id).is_none(),
        "without a friendship, the relay copy is fully eviction-eligible under pressure"
    );
}

/// Handbook §4.5 expiry: a friendship past `FRIENDSHIP_TTL_MS` is pruned
/// lazily on `tick`, and its formerly-pinned bundles revert to ordinary
/// eviction candidacy (no renewal happened).
#[test]
fn expired_friendship_unpins_its_buffered_bundle() {
    let mut a = node(1);
    let mut b = node(2);
    let mut f = node(3);
    introduce(&mut a, &mut b);
    drain(&mut a);
    drain(&mut b);

    f.allow_friendship_from(b.public_key());
    b.request_friendship(f.public_key());
    contact(&mut b, &mut f, 1, 10_000);
    drain(&mut b);
    drain(&mut f);

    let msg_id = a.send_text(&b.node_id(), "will outlive the friendship").unwrap();
    contact(&mut a, &mut f, 2, 20_000);
    drain(&mut a);
    drain(&mut f);
    assert!(
        f.core.friends.get(&b.node_id()).unwrap().buffered.contains(&msg_id),
        "sanity: pinned before expiry"
    );

    // Advance well past the TTL (measured from the offer's `since`, ~10_000)
    // without ever renewing the friendship.
    let past_ttl = 10_000 + FRIENDSHIP_TTL_MS + 1_000;
    f.tick(past_ttl);
    assert!(
        !f.core.friends.contains_key(&b.node_id()),
        "lapsed friendship is pruned from `friends`"
    );

    // The formerly-pinned bundle is now an ordinary eviction candidate.
    f.core.quota = 1;
    f.tick(past_ttl + 1_000);
    assert!(
        f.core.store.meta(&msg_id).is_none(),
        "unpinned bundle evicts once the friendship has lapsed"
    );
}

// --------------------------------------------------- blob plane (E5-S1) --

/// Doc 16 / handbook §4.4 padding-edge cases: empty file, exactly one
/// stripe's worth, and a non-multiple length all round UP to a whole number
/// of stripes (never truncate, never zero for an empty file).
#[test]
fn stripes_for_size_padding_edges() {
    use crate::blob::stripes_for_size;
    use mmp_types::consts::STRIPE_DATA_BYTES;

    assert_eq!(stripes_for_size(0), 1, "empty file still gets one zero stripe");
    assert_eq!(stripes_for_size(1), 1, "one byte still needs a whole stripe");
    assert_eq!(stripes_for_size(STRIPE_DATA_BYTES as u32), 1, "exactly one stripe's worth");
    assert_eq!(
        stripes_for_size(STRIPE_DATA_BYTES as u32 + 1),
        2,
        "one byte over one stripe rounds up to a second"
    );
    assert_eq!(
        stripes_for_size((STRIPE_DATA_BYTES * 3) as u32 - 1),
        3,
        "non-multiple length rounds up, never down"
    );
}

/// E5-S1 acceptance (decision D8): `send_file` → the manifest arrives as a
/// verified `FileOffered` → `fetch_file` absorbs the already-delivered
/// symbols and reassembles bit-exact bytes on disk → a second `send_file` of
/// the identical bytes mints zero new class-4 bundles (dedup, decision D6).
#[test]
fn send_file_fetch_reassembles_bit_exact_and_dedups() {
    let mut a = node(1);
    let mut b = node(2);
    introduce(&mut a, &mut b);
    drain(&mut a);
    drain(&mut b);

    // One stripe's worth minus a bit, so this is a genuine non-multiple-length
    // single-stripe file (RS_DATA_K=32 data symbols + 8 parity = 40 total).
    let data: Vec<u8> = (0..3000u32).map(|i| (i % 251) as u8).collect();

    let dest_dir = std::env::temp_dir().join("mmp-node-blob-test");
    std::fs::create_dir_all(&dest_dir).expect("create temp dir");
    let dest_path = dest_dir.join(format!("recv-{}-{}.bin", std::process::id(), line!()));
    let _ = std::fs::remove_file(&dest_path);
    let part_path = {
        let mut name = dest_path.file_name().unwrap().to_os_string();
        name.push(".part");
        dest_path.with_file_name(name)
    };
    let _ = std::fs::remove_file(&part_path);

    let sent = a.send_file(&b.node_id(), "test.bin", "application/octet-stream", &data).unwrap();

    contact(&mut a, &mut b, 1, 10_000);
    drain(&mut a);
    let b_events = drain(&mut b);

    let offered = b_events
        .iter()
        .find_map(|e| match e {
            Event::FileOffered { entries, .. } => Some(entries.clone()),
            _ => None,
        })
        .expect("bob received the file offer");
    assert_eq!(offered.len(), 1);
    assert_eq!(offered[0].root, sent.root);
    assert_eq!(offered[0].size, data.len() as u32);

    b.fetch_file(&sent.root, &dest_path).unwrap();
    // Idempotent re-call while active.
    assert!(b.fetch_file(&sent.root, &dest_path).is_ok());

    let progress = b.file_progress(&sent.root).expect("assembly exists");
    assert_eq!(
        progress.state,
        crate::BlobState::Assembled,
        "every symbol was already delivered in the single contact above: {progress:?}"
    );

    let b_events2 = drain(&mut b);
    assert!(
        b_events2.iter().any(|e| matches!(e, Event::FileReady { root, .. } if *root == sent.root)),
        "FileReady fired: {b_events2:?}"
    );

    let written = std::fs::read(&dest_path).expect("assembled file exists");
    assert_eq!(written, data, "reassembled bytes are bit-exact");
    assert!(!part_path.exists(), ".part is renamed away on success");

    // Dedup by content address (decision D6): identical bytes ⇒ zero new
    // class-4 bundles minted, only a fresh manifest message.
    // The first-contact manifest is an async root-establishing bundle. Its
    // receipt is generated only after the first reconciliation pass, so a
    // subsequent application send must wait for one more contact rather than
    // racing a direct bundle ahead of the root.
    contact(&mut a, &mut b, 2, 20_000);
    drain(&mut a);
    drain(&mut b);

    let symbol_count = |n: &Node| {
        n.core.store.all_meta().iter().filter(|m| m.class == mmp_types::bundle::class::SYMBOL).count()
    };
    let before = symbol_count(&a);
    let resent = a.send_file(&b.node_id(), "test.bin", "application/octet-stream", &data).unwrap();
    let after = symbol_count(&a);
    assert_eq!(before, after, "identical content is deduped, no new symbol bundles minted");
    assert_eq!(resent.root, sent.root, "same content address");

    let _ = std::fs::remove_file(&dest_path);
    let _ = std::fs::remove_file(&part_path);
}

/// `Node::cancel_fetch` moves an active (not-yet-complete) assembly to
/// `Abandoned` and fires `FileFailed`. Uses a directly-injected offer (rather
/// than a real contact) so the assembly is deterministically still `Wanted`
/// (no symbols delivered) at the moment of cancellation — a real contact
/// tends to deliver everything and finish the file in one pass at this small
/// test scale, which would make cancellation a documented no-op instead
/// (`cancel_fetch` never un-assembles a completed file).
#[test]
fn cancel_fetch_abandons_and_fires_event() {
    let mut b = node(2);
    let root = mmp_types::ids::Id16([42; 16]);
    b.core.offers.insert(
        root,
        crate::blob::FileOfferInfo {
            size: 1000,
            full_hash: [0; 32],
            sharer: mmp_types::ids::Key32([1; 32]),
        },
    );

    let dest = std::env::temp_dir().join(format!("mmp-node-cancel-{}.bin", std::process::id()));
    b.fetch_file(&root, &dest).unwrap();
    assert_eq!(b.file_progress(&root).unwrap().state, crate::BlobState::Wanted);

    b.cancel_fetch(&root);
    assert_eq!(b.file_progress(&root).unwrap().state, crate::BlobState::Abandoned);
    let events = drain(&mut b);
    assert!(
        events.iter().any(|e| matches!(
            e,
            Event::FileFailed { root: r, reason } if *r == root && *reason == crate::FileFailReason::Abandoned
        )),
        "cancel fires FileFailed{{Abandoned}}: {events:?}"
    );

    let _ = std::fs::remove_file(&dest);
}
