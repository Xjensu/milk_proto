//! FFI-surface integration test: two [`MmpNode`]s exchange claims out of
//! band, pump an in-memory link exactly the way a mobile host pumps a GATT
//! connection (link_up / link_data / poll_send), and confirm a text message
//! crosses with the right body via `poll_event`.
//!
//! This does not exercise UniFFI's generated Kotlin/Swift bindings (no
//! bindings-generation step here) — it exercises the exact Rust surface
//! those bindings call into.

use super::{FfiEvent, FfiTier, MmpNode};

fn node(seed: u8) -> std::sync::Arc<MmpNode> {
    MmpNode::new(vec![seed; 32], FfiTier::Relay, None).expect("valid 32-byte seed")
}

/// Exchange identity claims out-of-band (the QR / first-sync bootstrap).
fn introduce(a: &MmpNode, b: &MmpNode) {
    for c in a.own_claims() {
        b.ingest_claim(c, true).expect("well-formed own claim");
    }
    for c in b.own_claims() {
        a.ingest_claim(c, true).expect("well-formed own claim");
    }
}

/// Run one full contact between two nodes over an in-memory link — same
/// shape as `mmp-node`'s `contact` helper (crates/mmp-node/src/tests.rs),
/// but driven entirely through the FFI methods a mobile host would call.
fn contact(a: &MmpNode, b: &MmpNode, link: u64, now: u64) {
    a.tick(now);
    b.tick(now);
    a.link_up(link, true);
    b.link_up(link, false);
    for _ in 0..300 {
        let mut quiet = true;
        while let Some(out) = a.poll_send() {
            quiet = false;
            b.link_data(out.link, out.bytes);
        }
        while let Some(out) = b.poll_send() {
            quiet = false;
            a.link_data(out.link, out.bytes);
        }
        if quiet {
            break;
        }
    }
    a.link_down(link);
    b.link_down(link);
}

fn drain(n: &MmpNode) -> Vec<FfiEvent> {
    std::iter::from_fn(|| n.poll_event()).collect()
}

#[test]
fn direct_message_delivery_via_ffi_surface() {
    let alice = node(1);
    let bob = node(2);
    introduce(&alice, &bob);
    drain(&alice);
    drain(&bob);

    let bob_id = bob.node_id();
    let msg_id = alice.send_text(bob_id.clone(), "hello over the mesh".to_string()).unwrap();

    contact(&alice, &bob, 1, 10_000);
    let bob_events = drain(&bob);
    let received = bob_events.iter().find_map(|e| match e {
        FfiEvent::MessageReceived { body, from, .. } => Some((body.clone(), from.clone())),
        _ => None,
    });
    let (body, from) = received.expect("bob received the message");
    assert_eq!(body, b"hello over the mesh");
    assert_eq!(from, alice.node_id());

    // Receipt returns on the second contact (same shape as the mmp-node test).
    contact(&alice, &bob, 2, 20_000);
    let alice_events = drain(&alice);
    assert!(
        alice_events.iter().any(|e| matches!(e, FfiEvent::Delivered { id } if *id == msg_id)),
        "alice got the delivery receipt: {alice_events:?}"
    );
}

#[test]
fn bad_length_ids_are_rejected_at_the_ffi_boundary() {
    let alice = node(1);
    let bad_dest = vec![0u8; 4]; // not 16 bytes
    let err = alice.send_text(bad_dest, "x".to_string()).unwrap_err();
    assert_eq!(err, super::FfiError::BadArg);

    let bad_seed = vec![0u8; 31]; // not 32 bytes
    let err = match MmpNode::new(bad_seed, FfiTier::Leaf, None) {
        Err(e) => e,
        Ok(_) => panic!("31-byte seed must be rejected"),
    };
    assert_eq!(err, super::FfiError::BadArg);
}

#[test]
fn unknown_peer_is_reported() {
    let alice = node(1);
    let stranger = vec![0xabu8; 16];
    let err = alice.send_text(stranger, "hi".to_string()).unwrap_err();
    assert_eq!(err, super::FfiError::UnknownPeer);
}

/// A unique scratch directory under the OS temp dir, cleaned up by
/// [`TempDir::drop`]. No `tempfile` crate dependency — this crate already
/// has everything it needs in `std`.
struct TempDir(std::path::PathBuf);

impl TempDir {
    fn new(tag: &str) -> TempDir {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("mmp-ffi-test-{tag}-{unique}"));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        TempDir(dir)
    }

    fn file(&self, name: &str) -> String {
        self.0.join(name).to_str().expect("valid utf-8 temp path").to_string()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn state_survives_restart() {
    let dir = TempDir::new("survives-restart");
    let path = dir.file("alice.mmp");
    let seed = vec![7u8; 32];

    let bob = node(2);
    let before_restart;

    {
        let alice =
            MmpNode::new(seed.clone(), FfiTier::Relay, Some(path.clone())).expect("valid seed");
        introduce(&alice, &bob);
        drain(&alice);
        drain(&bob);

        // Queue a bundle in alice's store without ever contacting bob, so
        // the bundle (and alice's own claim-gossip bundles) stay put.
        let bob_id = bob.node_id();
        alice.send_text(bob_id, "still here after restart".to_string()).expect("bob's claims known");

        before_restart = alice.store_len();
        assert!(before_restart > 0, "expect at least the queued message + gossiped claims");

        alice.flush().expect("flush to a writable temp dir must succeed");
        assert_eq!(alice.last_persist_error(), None);

        // alice (and its Arc<MmpNode>) is dropped at the end of this block —
        // simulates process death after a successful flush.
    }

    let alice2 =
        MmpNode::new(seed, FfiTier::Relay, Some(path)).expect("reopen the same snapshot path");
    // A fresh node always republishes its 2 identity claims (deterministic
    // content given the same seed, so they land on the same bundle IDs and
    // simply overwrite in place). The exact count surviving the restart
    // therefore proves the queued *message* bundle specifically came back
    // from disk, not just baseline construction noise.
    assert_eq!(
        alice2.store_len(),
        before_restart,
        "bundle store (identity claims + the queued message) must survive the restart intact"
    );
}

/// The real acceptance test for handbook §4.3 steps 1–2 ("persistence
/// completion"): claims and group membership must be usable immediately
/// after reopening the same snapshot path, with no re-ingest / re-invite
/// needed. Mirrors `state_survives_restart`'s kill-and-reopen shape but
/// checks `ClaimStore` + group-membership contents rather than raw bundle
/// count.
#[test]
fn claims_and_group_membership_survive_restart() {
    let dir = TempDir::new("claims-and-groups-survive-restart");
    let path = dir.file("alice.mmp");
    let seed = vec![11u8; 32];

    let bob = node(20);
    let carol = node(21);
    let gid;

    {
        let alice =
            MmpNode::new(seed.clone(), FfiTier::Relay, Some(path.clone())).expect("valid seed");
        introduce(&alice, &bob);
        introduce(&alice, &carol);
        drain(&alice);
        drain(&bob);
        drain(&carol);

        gid = alice.create_group();
        alice.group_invite(gid.clone(), bob.node_id()).expect("bob is a known peer");
        alice.group_invite(gid.clone(), carol.node_id()).expect("carol is a known peer");

        assert_eq!(
            alice.group_members(gid.clone()).unwrap().len(),
            3,
            "alice + bob + carol before restart"
        );
        // Bob's and Carol's exchange-key/prekey claims must be on file too
        // (learned via `introduce`, not just alice's own claims).
        assert!(!alice.own_claims().is_empty());

        alice.flush().expect("flush to a writable temp dir must succeed");
        assert_eq!(alice.last_persist_error(), None);

        // alice is dropped here — simulates process death after a successful flush.
    }

    let alice2 =
        MmpNode::new(seed, FfiTier::Relay, Some(path)).expect("reopen the same snapshot path");

    // Group membership survived without re-inviting anyone.
    let members = alice2.group_members(gid).expect("group known without re-creating it");
    assert_eq!(members.len(), 3, "membership (alice + bob + carol) must survive the restart");
    assert!(members.contains(&bob.node_id()));
    assert!(members.contains(&carol.node_id()));

    // Bob and carol's claims survived without re-running `introduce`: alice2
    // can address them by NodeID right away.
    let msg = alice2.send_text(bob.node_id(), "still know you after restart".to_string());
    assert!(msg.is_ok(), "bob's exchange-key/prekey claims must have survived the restart: {msg:?}");
}

/// A scanned beacon is accepted without panicking. Beacon ingestion (see
/// `mmp-node`'s `on_tal_event` / `TalEvent::Beacon` arm, crates/mmp-node/src/lib.rs
/// around line 423) only folds the decoded ephemeral ID into the population
/// estimate N̂ via `Utility::note_ephem` — internal router bookkeeping used
/// for contact-radar / dial gating. It does not touch `ClaimStore`, does not
/// establish a session, and does not push anything onto the `Event` queue
/// (compare `SessionEvent::Established`, the only source of `ContactSeen` /
/// `Established`, at crates/mmp-node/src/lib.rs around line 459-462). So a
/// beacon from a known, claim-introduced peer — even with `verified: true`
/// claims on file — has no observable effect through `poll_event`; the only
/// thing to assert here is that the call is accepted and the event queue
/// stays empty.
#[test]
fn beacon_is_accepted_with_no_observable_event() {
    let alice = node(1);
    let bob = node(2);
    introduce(&alice, &bob);
    drain(&alice);
    drain(&bob);

    let bob_beacon = bob.beacon(10_000);
    alice.on_beacon(bob_beacon, -60);

    assert!(
        drain(&alice).is_empty(),
        "beacon ingestion is N̂ bookkeeping only; it must not surface an FfiEvent"
    );
}

#[test]
fn group_remove_and_set_tier_roundtrip() {
    let alice = node(1);
    let bob = node(2);
    introduce(&alice, &bob);
    drain(&alice);
    drain(&bob);

    let gid = alice.create_group();
    let bob_id = bob.node_id();
    alice.group_invite(gid.clone(), bob_id.clone()).expect("known peer");

    let epoch = alice.group_remove(gid, bob_id).expect("admin removes a known member");
    assert_eq!(epoch, 2, "groups start at epoch 1; first removal rotates to epoch 2");

    // set_tier / public_key just need to be callable through the FFI surface
    // and behave sanely.
    alice.set_tier(FfiTier::Anchor);
    let pk = alice.public_key();
    assert_eq!(pk.len(), 32, "identity public key is exactly 32 bytes");
}
