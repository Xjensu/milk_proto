//! FFI-surface integration test: two [`MmpNode`]s exchange claims out of
//! band, pump an in-memory link exactly the way a mobile host pumps a GATT
//! connection (link_up / link_data / poll_send), and confirm a text message
//! crosses with the right body via `poll_event`.
//!
//! This does not exercise UniFFI's generated Kotlin/Swift bindings (no
//! bindings-generation step here) — it exercises the exact Rust surface
//! those bindings call into.

use super::{FfiError, FfiEvent, FfiTier, MmpNode};
use mmp_core::store::BundleStore;
use mmp_store::SnapshotStore;
use mmp_types::consts::{LIFETIME_MESSAGE_MS, MAX_BUNDLE, NICKNAME_MAX};
use mmp_types::ids::Id16;
use std::sync::{Arc, Barrier};
use std::thread;

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
fn first_tick_anchors_persisted_message_deadline() {
    let dir = TempDir::new("first-tick-deadline");
    let path = dir.file("alice.mmp");
    let bob = node(22);
    let alice = MmpNode::new(vec![21u8; 32], FfiTier::Relay, Some(path.clone())).expect("valid seed");
    introduce(&alice, &bob);
    drain(&alice);
    drain(&bob);

    const NOW: u64 = 123_456;
    alice.tick(NOW);
    let message = alice.send_text(bob.node_id(), "clock anchored".to_string()).expect("known peer");
    alice.flush().expect("flush to a writable temp dir must succeed");

    let id = Id16(message.as_slice().try_into().expect("FFI returns a 16-byte BundleID"));
    let snapshot = SnapshotStore::open(path);
    let meta = snapshot
        .all_meta()
        .into_iter()
        .find(|candidate| candidate.id == id)
        .expect("queued message metadata is persisted");
    assert_eq!(meta.stored_at, NOW);
    assert_eq!(meta.expires_at - meta.stored_at, LIFETIME_MESSAGE_MS);

    alice.tick(NOW + 1);
    assert!(alice.store_len() > 0, "the first post-send tick must retain the message");
}

#[test]
fn backward_tick_is_ignored_at_ffi_boundary() {
    let dir = TempDir::new("backward-tick");
    let node = MmpNode::new(
        vec![71u8; 32],
        FfiTier::Leaf,
        Some(dir.file("node.mmp")),
    )
    .expect("valid seed");

    node.tick(10_000);
    node.tick(9_999);

    let observed = node
        .persist
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .last_seen_now_ms;
    assert_eq!(observed, 10_000, "stale native callback must not rewind FFI persistence time");
}

#[test]
fn in_memory_tick_still_advances_ffi_monotonic_guard() {
    let node = node(72);

    node.tick(10_000);
    node.tick(9_999);

    let observed = node
        .persist
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .last_seen_now_ms;
    assert_eq!(observed, 10_000, "in-memory hosts must retain the latest monotonic sample");
}

#[test]
fn concurrent_ticks_keep_the_largest_monotonic_sample() {
    let dir = TempDir::new("concurrent-ticks");
    let node = MmpNode::new(vec![72u8; 32], FfiTier::Leaf, Some(dir.file("node.mmp")))
        .expect("valid seed");
    let gate = Arc::new(Barrier::new(8));
    let mut workers = Vec::new();

    for offset in 1..=8u64 {
        let node = Arc::clone(&node);
        let gate = Arc::clone(&gate);
        workers.push(thread::spawn(move || {
            gate.wait();
            node.tick(offset * 1_000);
        }));
    }
    for worker in workers {
        worker.join().expect("tick worker must not panic");
    }

    let observed = node
        .persist
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .last_seen_now_ms;
    assert_eq!(observed, 8_000, "the largest sample must win despite callback races");
}

#[test]
fn first_contact_reordering_guard_is_visible_at_ffi_boundary() {
    let alice = node(11);
    let bob = node(12);
    introduce(&alice, &bob);
    drain(&alice);
    drain(&bob);

    let bob_id = bob.node_id();
    alice.send_text(bob_id.clone(), "first".to_string()).unwrap();
    assert_eq!(
        alice.send_text(bob_id, "second before root".to_string()),
        Err(FfiError::ConversationPending)
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
fn oversized_text_is_rejected_without_poisoning_first_contact_at_ffi_boundary() {
    let alice = node(3);
    let bob = node(4);
    introduce(&alice, &bob);
    drain(&alice);
    drain(&bob);

    // The first-contact path preflights the largest authenticated encoding
    // before allocating conversation state.  Keep this regression at the
    // mobile-facing API boundary: a host-controlled String at the bundle cap
    // must map to BadArg and must not leave a pending conversation behind.
    let oversized = "x".repeat(MAX_BUNDLE);
    assert_eq!(alice.send_text(bob.node_id(), oversized), Err(FfiError::BadArg));

    // A valid retry must still be accepted immediately; otherwise the
    // rejected caller input would have poisoned the first-contact state.
    assert!(alice.send_text(bob.node_id(), "retry after rejection".to_string()).is_ok());
}

#[test]
fn bounded_store_constructor_surfaces_persist_failure() {
    let dir = TempDir::new("bounded-store");
    let path = dir.file("bounded.mmp");
    let node = MmpNode::new_with_store_limit(vec![44u8; 32], FfiTier::Relay, path, 1)
        .expect("valid bounded store constructor arguments");
    node.tick(1);
    assert_eq!(node.flush(), Err(FfiError::Internal));
    assert!(node.last_persist_error().is_some(), "host must be able to surface the cap failure");
}

#[test]
fn transport_close_feedback_is_visible_at_the_ffi_boundary() {
    let node = MmpNode::new(vec![9u8; 32], FfiTier::Leaf, None).expect("valid seed");
    node.link_up(1, true);
    while node.poll_send().is_some() {}

    // Leaf admits one session. The second physical link is denied locally and
    // must be closed by the mobile transport host.
    node.link_up(2, true);
    assert_eq!(node.poll_close(), Some(2));
    assert_eq!(node.poll_close(), None);
}

#[test]
fn transport_close_saturation_is_visible_at_the_ffi_boundary() {
    let node = MmpNode::new(vec![10u8; 32], FfiTier::Leaf, None).expect("valid seed");
    node.link_up(1, true);
    while node.poll_send().is_some() {}

    // Keep the bounded close queue undrained: one admitted link plus 65 denied
    // links fills the 64-entry queue and drops exactly one close action.
    for link in 2..=66 {
        node.link_up(link, true);
    }
    assert_eq!(node.close_actions_dropped(), 1);
}

#[test]
fn callback_event_queue_saturation_is_visible_at_the_ffi_boundary() {
    let node = node(13);
    while node.poll_event().is_some() {}

    // `set_tier` is a host callback producer. Keep the queue undrained and
    // alternate tiers until the bounded mmp-node FIFO starts dropping the
    // newest event.
    for index in 0..4096 {
        node.set_tier(if index % 2 == 0 { FfiTier::Leaf } else { FfiTier::Relay });
    }

    assert!(node.event_queue_dropped() > 0);
}

#[test]
fn successor_revocation_forwards_and_validates_fixed_length_keys() {
    let bob = node(2);
    let successor = node(3);
    let successor_key = successor.public_key();
    let countersig = [0xa5u8; 64];

    let claim_bytes = bob
        .revoke_identity_with_successor(9, successor_key.clone(), countersig.to_vec())
        .expect("valid 32/64-byte inputs");
    let claim = mmp_types::claim::Claim::decode(&claim_bytes).expect("well-formed claim");
    let revocation = mmp_types::claim::Revocation::decode(&claim.body).expect("well-formed body");
    assert_eq!(revocation.revoked_key.0.to_vec(), bob.public_key());
    assert_eq!(revocation.reason, 9);
    assert_eq!(revocation.successor, Some(mmp_types::ids::Key32(successor_key.clone().try_into().unwrap())));
    assert_eq!(revocation.successor_countersig, Some(mmp_types::ids::Sig64(countersig)));

    assert_eq!(
        bob.revoke_identity_with_successor(9, vec![0u8; 31], countersig.to_vec()),
        Err(super::FfiError::BadArg)
    );
    assert_eq!(
        bob.revoke_identity_with_successor(9, successor_key, vec![0u8; 63]),
        Err(super::FfiError::BadArg)
    );
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
        alice.tick(100_000);

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

/// The real acceptance test for handbook §4.3 step 3 ("SeenIndex ...
/// persistence"): a bundle bob re-offers after alice restarts must not fire
/// `MessageReceived` a second time — proving the resurrection/replay dedup
/// window (AUDIT F1) survived the restart, not just that its bytes
/// round-trip in isolation (already covered directly in `mmp-store::tests`
/// and `mmp-core::tests`).
#[test]
fn authority_admission_survives_restart() {
    let dir = TempDir::new("authority-survives-restart");
    let path = dir.file("bob.mmp");
    let alice = node(41);
    let authority = mmp_crypto::Identity::from_seed(&[42; 32]);
    let cert = {
        let mut cert = mmp_types::authcert::AuthCert {
            suite: 1,
            subject: mmp_types::ids::Key32(alice.public_key().try_into().unwrap()),
            scope: mmp_types::authcert::scope_id_of(b"persisted-scope"),
            caps: mmp_types::authcert::CAP_ANNOUNCE,
            priority_budget: 10,
            not_after: 1_000_000,
            issuer: mmp_types::ids::Key32(authority.public().0),
            sig: mmp_types::ids::Sig64::ZERO,
        };
        cert.sig = authority.sign(mmp_types::domain::SigDomain::AuthCert, &cert.signed_object());
        cert.encode()
    };
    let scope = mmp_types::authcert::scope_id_of(b"persisted-scope").0.to_vec();

    let bob = MmpNode::new(vec![44; 32], FfiTier::Relay, Some(path.clone())).expect("valid seed");
    introduce(&alice, &bob);
    bob.subscribe_scope("persisted-scope".to_string());
    bob.pin_authority_root(scope.clone(), authority.public().0.to_vec()).unwrap();
    bob.set_wall_clock(100);

    alice
        .publish_announcement(scope.clone(), 1, 1, "before restart".to_string(), Some(cert.clone()))
        .unwrap();
    contact(&alice, &bob, 1, 10_000);
    let before = drain(&bob);
    assert!(before.iter().any(|event| matches!(
        event,
        FfiEvent::Announcement { effective_priority, official, .. }
            if *effective_priority == 1 && *official
    )), "the pinned authority should elevate before restart: {before:?}");

    bob.flush().expect("authority snapshot flush");
    drop(bob);

    let bob2 = MmpNode::new(vec![44; 32], FfiTier::Relay, Some(path)).expect("reopen snapshot");
    bob2.set_wall_clock(100);
    alice
        .publish_announcement(scope, 1, 1, "after restart".to_string(), Some(cert))
        .unwrap();
    contact(&alice, &bob2, 2, 20_000);
    let after = drain(&bob2);
    assert!(after.iter().any(|event| matches!(
        event,
        FfiEvent::Announcement { effective_priority, official, text, .. }
            if *effective_priority == 1 && *official && text == b"after restart"
    )), "authority admission should survive restart: {after:?}");
}

#[test]
fn mint_auth_cert_signs_locally_without_auto_trusting_the_issuer() {
    let issuer = node(45);
    let recipient = node(46);
    introduce(&issuer, &recipient);
    drain(&issuer);
    drain(&recipient);

    let scope = mmp_types::authcert::scope_id_of(b"minted-scope");
    issuer.set_wall_clock(1_000);
    let cert_bytes = issuer
        .mint_auth_cert(
            1,
            issuer.public_key(),
            scope.0.to_vec(),
            mmp_types::authcert::CAP_ANNOUNCE | mmp_types::authcert::CAP_PRIORITY_EMERGENCY,
            u16::MAX,
            2_000,
        )
        .expect("valid v1 certificate parameters");
    let cert = mmp_types::authcert::AuthCert::decode(&cert_bytes).expect("canonical AuthCert");
    assert_eq!(cert.suite, 1);
    assert_eq!(cert.subject.0.to_vec(), issuer.public_key());
    assert_eq!(cert.scope, scope);
    assert_eq!(cert.priority_budget, u16::MAX);
    assert_eq!(cert.not_after, 2_000);
    assert_eq!(cert.issuer.0.to_vec(), issuer.public_key());
    mmp_crypto::verify(
        mmp_types::domain::SigDomain::AuthCert,
        &cert.issuer,
        &cert.signed_object(),
        &cert.sig,
    )
    .expect("local identity signed the AuthCert");

    // Minting signs only; it does not pin the local identity as an authority
    // root. The recipient therefore keeps this validly signed announcement at
    // NORMAL and non-official until an explicit pinning ceremony occurs.
    recipient.subscribe_scope("minted-scope".to_string());
    recipient.set_wall_clock(1_000);
    issuer
        .publish_announcement(scope.0.to_vec(), 1, 1, "untrusted issuer".to_string(), Some(cert_bytes))
        .expect("announcement with attached cert");
    contact(&issuer, &recipient, 45, 45_000);
    let events = drain(&recipient);
    assert!(events.iter().any(|event| matches!(
        event,
        FfiEvent::Announcement { effective_priority, official, text, .. }
            if *effective_priority == 2 && !*official && text == b"untrusted issuer"
    )), "local minting must not auto-trust its issuer: {events:?}");
}

#[test]
fn mint_auth_cert_rejects_unknown_caps_and_expired_parameters() {
    let node = node(47);
    let scope = mmp_types::authcert::scope_id_of(b"mint-validation").0.to_vec();
    let subject = node.public_key();
    node.set_wall_clock(1_000);

    assert_eq!(
        node.mint_auth_cert(2, subject.clone(), scope.clone(), mmp_types::authcert::CAP_ANNOUNCE, 1, 2_000),
        Err(FfiError::BadArg),
        "only the frozen v1 AuthCert suite is mintable"
    );
    assert_eq!(
        node.mint_auth_cert(1, subject.clone(), scope.clone(), 0b100, 1, 2_000),
        Err(FfiError::BadArg),
        "reserved capability bits must not be minted"
    );
    assert_eq!(
        node.mint_auth_cert(1, subject.clone(), scope.clone(), mmp_types::authcert::CAP_PRIORITY_EMERGENCY, 1, 2_000),
        Err(FfiError::BadArg),
        "emergency capability requires announce capability"
    );
    assert_eq!(
        node.mint_auth_cert(1, subject.clone(), scope.clone(), mmp_types::authcert::CAP_ANNOUNCE, 1, 1_000),
        Err(FfiError::BadArg),
        "expiry at the issuer's current wall clock is invalid"
    );
    assert_eq!(
        node.mint_auth_cert(1, subject, scope, mmp_types::authcert::CAP_ANNOUNCE, 1, 0),
        Err(FfiError::BadArg),
        "zero expiry is not a usable certificate"
    );
}

#[test]
fn blocklist_survives_restart_and_denies_reintroduced_peer() {
    let dir = TempDir::new("blocklist-survives-restart");
    let path = dir.file("alice.mmp");
    let bob = node(51);
    let seed = vec![50; 32];

    {
        let alice = MmpNode::new(seed.clone(), FfiTier::Relay, Some(path.clone())).expect("valid seed");
        introduce(&alice, &bob);
        drain(&alice);
        drain(&bob);
        alice.block(bob.node_id()).unwrap();
        alice.flush().expect("blocklist snapshot flush");
    }

    let alice2 = MmpNode::new(seed, FfiTier::Relay, Some(path)).expect("reopen snapshot");
    contact(&alice2, &bob, 7, 70_000);
    let alice_events = drain(&alice2);
    assert!(
        !alice_events.iter().any(|event| matches!(event, FfiEvent::Established { .. })),
        "a persisted blocklist must deny a reintroduced peer: {alice_events:?}"
    );
}

#[test]
fn seen_index_survives_restart_and_dedups_a_replayed_bundle() {
    let dir = TempDir::new("seen-index-survives-restart");
    let path = dir.file("alice.mmp");
    let seed = vec![13u8; 32];

    let bob = node(30);
    let msg_id;

    {
        let alice =
            MmpNode::new(seed.clone(), FfiTier::Relay, Some(path.clone())).expect("valid seed");
        introduce(&alice, &bob);
        drain(&alice);
        drain(&bob);

        msg_id = bob
            .send_text(alice.node_id(), "replay me".to_string())
            .expect("alice's claims known");
        contact(&bob, &alice, 1, 1000);
        let alice_events = drain(&alice);
        assert!(
            alice_events
                .iter()
                .any(|e| matches!(e, FfiEvent::MessageReceived { id, .. } if *id == msg_id)),
            "alice must receive the message before restart"
        );

        alice.flush().expect("flush to a writable temp dir must succeed");
        assert_eq!(alice.last_persist_error(), None);
        // alice is dropped here — simulates process death after a successful flush.
    }

    let alice2 =
        MmpNode::new(seed, FfiTier::Relay, Some(path)).expect("reopen the same snapshot path");

    // Bob never purged the bundle (no receipt round-trip modeled here), so it
    // re-offers on a fresh contact exactly like an honest relay replaying
    // stale inventory would.
    contact(&bob, &alice2, 2, 2000);
    let alice2_events = drain(&alice2);
    assert!(
        !alice2_events
            .iter()
            .any(|e| matches!(e, FfiEvent::MessageReceived { id, .. } if *id == msg_id)),
        "SeenIndex must have survived restart: a replayed bundle must not fire MessageReceived twice"
    );
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

#[test]
fn group_remove_unknown_member_maps_to_unknown_peer() {
    let alice = node(4);
    let gid = alice.create_group();
    let unknown = vec![0xd5; 16];

    let err = alice.group_remove(gid, unknown).expect_err("unknown member must be rejected");
    assert_eq!(err, super::FfiError::UnknownPeer);
}

/// Handbook §4.5 (E5-S2), FFI surface: a friendship handshake driven purely
/// through `requestFriendship`/`allowFriendshipFrom` produces
/// `FfiEvent::FriendshipEstablished` on both sides after one contact.
#[test]
fn friendship_handshake_via_ffi_surface() {
    let bob = node(1); // dependent
    let carol = node(2); // patron
    introduce(&bob, &carol);
    drain(&bob);
    drain(&carol);

    carol.allow_friendship_from(bob.public_key()).expect("32-byte key");
    bob.request_friendship(carol.public_key()).expect("32-byte key");
    contact(&bob, &carol, 1, 10_000);

    let bob_events = drain(&bob);
    let carol_events = drain(&carol);
    assert!(
        bob_events.iter().any(|e| matches!(
            e,
            FfiEvent::FriendshipEstablished { peer, .. } if *peer == carol.node_id()
        )),
        "dependent sees FriendshipEstablished: {bob_events:?}"
    );
    assert!(
        carol_events.iter().any(|e| matches!(
            e,
            FfiEvent::FriendshipEstablished { peer, .. } if *peer == bob.node_id()
        )),
        "patron sees FriendshipEstablished: {carol_events:?}"
    );
    for events in [&bob_events, &carol_events] {
        let since = events.iter().find_map(|event| match event {
            FfiEvent::FriendshipEstablished { since, .. } => Some(*since),
            _ => None,
        });
        assert_eq!(since, Some(10_000), "friendship timestamp uses host monotonic milliseconds");
    }
}

/// `revoke_identity` (doc handbook §4.6, E5-S3): the returned bytes are a
/// well-formed `Claim` at field `0x09`; once the peer learns it (mirroring
/// how `introduce` delivers any claim out-of-band), it surfaces as
/// `FfiEvent::PeerRevoked` and further sends to the revoked peer fail with
/// `FfiError::UnknownPeer` (the coarse FFI mapping for `SendError::PeerRevoked`
/// — see `map_send_error`'s doc comment).
#[test]
fn revoke_identity_surfaces_as_peer_revoked_and_blocks_further_sends() {
    let alice = node(1);
    let bob = node(2);
    introduce(&alice, &bob);
    drain(&alice);
    drain(&bob);

    let bob_id = bob.node_id();
    // Sanity: alice can message bob before the revocation.
    alice.send_text(bob_id.clone(), "before".to_string()).expect("known peer, not yet revoked");

    let claim_bytes = bob.revoke_identity(3);
    let claim = mmp_types::claim::Claim::decode(&claim_bytes).expect("well-formed claim");
    assert_eq!(claim.field, mmp_types::claim::field::REVOCATION);
    assert_eq!(claim.owner.0.to_vec(), bob.public_key());

    for c in bob.own_claims() {
        alice.ingest_claim(c, true).expect("well-formed claim");
    }
    let evs = drain(&alice);
    assert!(
        evs.iter().any(|e| matches!(
            e,
            FfiEvent::PeerRevoked { peer, successor: None } if *peer == bob_id
        )),
        "expected PeerRevoked with no successor: {evs:?}"
    );

    let err = alice.send_text(bob_id, "after".to_string()).unwrap_err();
    assert_eq!(err, super::FfiError::UnknownPeer);
}

/// The generated mobile error surface intentionally folds local lifecycle
/// admission failures into `Internal` until a versioned FFI error-policy
/// change is approved.  Keep this policy explicit so adding a new
/// `SendError` variant cannot silently alter the Kotlin/Swift contract.
#[test]
fn ffi_maps_local_lifecycle_failures_to_internal() {
    assert_eq!(
        super::map_send_error(mmp_node::SendError::ResourceLimit),
        super::FfiError::Internal
    );
    assert_eq!(
        super::map_send_error(mmp_node::SendError::RatchetExhausted),
        super::FfiError::Internal
    );
}

#[test]
fn ffi_maps_invalid_priority_to_bad_argument() {
    assert_eq!(
        super::map_send_error(mmp_node::SendError::InvalidPriority),
        super::FfiError::BadArg
    );
}

#[test]
fn oversized_nickname_is_rejected_before_claim_or_gossip_side_effects() {
    let node = node(90);
    let claims_before = node.own_claims();
    let store_before = node.store_len();
    // Use ASCII here so the test proves the wire-byte boundary directly;
    // multi-byte UTF-8 names are covered by the same byte-length check.
    let oversized = "n".repeat(NICKNAME_MAX + 1);

    assert_eq!(node.set_nickname(oversized, 1), Err(FfiError::BadArg));
    assert_eq!(node.own_claims(), claims_before, "invalid input must not publish a claim");
    assert_eq!(node.store_len(), store_before, "invalid input must not enqueue gossip");
}

#[test]
fn revoked_identity_rejects_nickname_through_ffi_error_surface() {
    let node = node(91);

    let revocation = node.revoke_identity(7);
    assert!(!revocation.is_empty());
    let claims_after_revocation = node.own_claims();
    let store_after_revocation = node.store_len();
    assert_eq!(node.set_nickname("after-revocation".to_string(), 1), Err(FfiError::BadArg));
    assert_eq!(node.own_claims(), claims_after_revocation);
    assert_eq!(node.store_len(), store_after_revocation, "only the revocation gossip is stored");
}
