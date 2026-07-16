//! Node integration tests (HANDOFF M3): two in-memory nodes exchange direct +
//! group messages, receipts purge; a 3-node chain relays store-carry-forward.

use mmp_core::MemStore;
use mmp_tal::TalEvent;
use mmp_types::authcert::AuthCert;
use mmp_types::bundle::prio;
use mmp_types::domain::SigDomain;
use mmp_types::ids::{Id16, Key32, NodeId, Sig64};

use crate::{DeliveryState, Event, Node, Tier};

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
    // Bob's reciprocal dist reached Alice; Alice (admin) forwards Bob's key to
    // Carol on the next contact.
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
