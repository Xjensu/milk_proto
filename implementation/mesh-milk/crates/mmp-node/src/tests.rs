//! Node integration tests (HANDOFF M3): two in-memory nodes exchange direct +
//! group messages, receipts purge; a 3-node chain relays store-carry-forward.

use std::collections::{BTreeMap, HashSet};

use mmp_core::store::{BundleMeta, BundleStore, SlotState, StorePutError, StorePutOutcome};
use mmp_core::{IdSource, MemStore, SessionHost};
use mmp_crypto::{conversation_id, Identity};
use mmp_tal::TalEvent;
use mmp_types::frame::{ProxySummaryBody, FT_CLOSE, FT_ERROR, MAX_ENVELOPE_PAYLOAD};
use mmp_types::authcert::{AnnouncementBody, AuthCert};
use mmp_types::bundle::{class, prio, Bundle};
use mmp_types::consts::{
    BLOB_INTEREST_TTL_MS, FRIENDSHIP_TTL_MS, LIFETIME_CONTROL_MS, LIFETIME_MESSAGE_MS, MAX_BUNDLE,
    OFFER_MIN_LIFETIME_MS, RS_DATA_K, RS_TOTAL_N, SYMBOL_SIZE,
};
use mmp_types::domain::SigDomain;
use mmp_types::ids::{node_id_of, Id16, Key32, NodeId, Sig64};
use mmp_types::payload::{apptype, AsyncPayload, GroupPayload, ReceiptPayload, SymbolPayload};
use mmp_types::tlv::Ext;

use crate::{DeliveryState, Event, Node, SendError, SentMessage, Tier};

fn node(seed: u8) -> Node {
    Node::new([seed; 32], Box::new(MemStore::new()), Tier::Relay)
}

#[test]
fn persisted_handoff_revalidates_raw_bundle_before_mutation() {
    let signer = Identity::from_seed(&[241; 32]);
    let peer = Id16([242; 16]);
    let mut bundle = Bundle {
        class: class::DIRECT,
        flags: 0,
        origin: signer.public(),
        dest: peer,
        lifetime_ms: LIFETIME_MESSAGE_MS,
        priority: prio::NORMAL,
        exts: Vec::new(),
        payload: vec![apptype::TEXT, 1, 2, 3],
        signature: Sig64::ZERO,
    };
    bundle.signature = signer.sign(SigDomain::Bundle, &bundle.canonical_zeroed());
    let id = bundle.id();
    let mut tampered = bundle.encode();
    // Keep the bytes canonical but invalidate the origin signature. A
    // checksum-only snapshot can otherwise reintroduce this record after a
    // restart, and the old handoff path would slice its lifetime field before
    // discovering the problem.
    let payload_tail = tampered.len() - 65;
    tampered[payload_tail] ^= 1;

    let mut n = node(243);
    n.core.store = Box::new(MemStore::new());
    n.core.store.put(
        BundleMeta {
            id,
            class: bundle.class,
            origin: bundle.origin_node(),
            dest: bundle.dest,
            priority: bundle.priority,
            effective_priority: bundle.priority,
            size: tampered.len() as u32,
            stored_at: 0,
            expires_at: LIFETIME_MESSAGE_MS,
            budget: 1,
            own_origin: false,
            slot: SlotState::Stored,
        },
        &tampered,
    );

    assert!(SessionHost::load_for_handoff(&mut n.core, &peer, &id).is_none());
    assert!(n.core.store.meta(&id).is_none(), "invalid persisted bundle must be removed");
}

#[test]
fn startup_rebuild_rejects_signed_but_merkle_invalid_symbol() {
    let signer = Identity::from_seed(&[244; 32]);
    let root = Id16([245; 16]);
    let peer = Id16([246; 16]);
    let mut bundle = Bundle {
        class: class::SYMBOL,
        flags: 0,
        origin: signer.public(),
        dest: peer,
        lifetime_ms: LIFETIME_MESSAGE_MS,
        priority: prio::BULK,
        exts: vec![Ext::SymbolInfo {
            root,
            block: 0,
            sym: 0,
            k: RS_DATA_K as u8,
            n: RS_TOTAL_N as u8,
        }],
        // The shape is decodable and the bundle signature is valid, but the
        // path is not a proof for `root`; startup must not index it as
        // servable holder state.
        payload: SymbolPayload { path: vec![0], symbol: vec![0; SYMBOL_SIZE] }.encode(),
        signature: Sig64::ZERO,
    };
    bundle.signature = signer.sign(SigDomain::Bundle, &bundle.canonical_zeroed());
    let id = bundle.id();
    let bytes = bundle.encode();
    let meta = BundleMeta {
        id,
        class: bundle.class,
        origin: bundle.origin_node(),
        dest: bundle.dest,
        priority: bundle.priority,
        effective_priority: bundle.priority,
        size: bytes.len() as u32,
        stored_at: 0,
        expires_at: LIFETIME_MESSAGE_MS,
        budget: 1,
        own_origin: false,
        slot: SlotState::Stored,
    };
    let mut store = MemStore::new();
    store.put(meta, &bytes);

    let restored = Node::new([247; 32], Box::new(store), Tier::Relay);
    assert!(restored.core.store.meta(&id).is_none(), "invalid symbol must not survive startup");
    assert!(!restored.core.blob_index.contains_key(&root));
}

/// Provider fixture that refuses every durable write. This exercises the
/// node-level boundary after `mmp-core::ingest` has authenticated a bundle,
/// without making quota eviction or provider I/O part of the test.
struct CapacityRejectStore;

impl BundleStore for CapacityRejectStore {
    fn put(&mut self, _meta: BundleMeta, _bytes: &[u8]) {}

    fn try_put(
        &mut self,
        _meta: BundleMeta,
        _bytes: &[u8],
    ) -> Result<StorePutOutcome, StorePutError> {
        Err(StorePutError::Capacity)
    }

    fn get(&self, _id: &Id16) -> Option<(BundleMeta, Vec<u8>)> {
        None
    }

    fn meta(&self, _id: &Id16) -> Option<BundleMeta> {
        None
    }

    fn update_meta(&mut self, _meta: BundleMeta) {}

    fn remove(&mut self, _id: &Id16) -> bool {
        false
    }

    fn ids_range(&self, _lo: &Id16, _hi: &Id16) -> Vec<Id16> {
        Vec::new()
    }

    fn all_meta(&self) -> Vec<BundleMeta> {
        Vec::new()
    }

    fn total_bytes(&self) -> u64 {
        0
    }

    fn len(&self) -> usize {
        0
    }

    fn expire(&mut self, _now_ms: u64) -> Vec<Id16> {
        Vec::new()
    }
}

#[test]
fn tick_compacts_idle_admission_bucket_maps() {
    let mut n = node(250);
    let origin = Id16([7; 16]);
    let authority_subject = Key32([8; 32]);
    assert!(n.core.buckets.admit(&origin, 1, 0, false));
    assert!(n.core.auth_buckets.admit(&authority_subject, 1, 0));
    assert_eq!(n.core.buckets.len(), 1);
    assert_eq!(n.core.auth_buckets.len(), 1);

    n.tick(LIFETIME_MESSAGE_MS + 1);

    assert!(n.core.buckets.is_empty());
    assert!(n.core.auth_buckets.is_empty());
}

#[test]
fn tick_ignores_backward_monotonic_samples() {
    let mut n = node(201);

    n.tick(10_000);
    assert_eq!(n.core.now, 10_000);

    // Direct FFI/mobile callers use the same Node::tick path as MeshDrive.
    // A stale callback must not rewind expiry/session time or mutate state.
    n.tick(9_999);
    assert_eq!(n.core.now, 10_000);

    // Equal timestamps remain valid and later samples still advance.
    n.tick(10_000);
    assert_eq!(n.core.now, 10_000);
    n.tick(10_001);
    assert_eq!(n.core.now, 10_001);
}

#[test]
fn sync_view_keeps_current_base_domains_without_priority_partition() {
    let mut n = node(249);
    // Node::new publishes its own identity/prekey claims; replace the store
    // so this mapping assertion contains only the deliberately seeded records.
    n.core.store = Box::new(MemStore::new());
    let peer = Id16([0xF0; 16]);
    let other = Id16([0xF1; 16]);
    let origin = n.node_id();
    let entries = [
        (Id16([0x10; 16]), class::CONTROL, peer, 0, 0),
        (Id16([0x20; 16]), class::CLAIM, peer, 3, 3),
        (Id16([0x30; 16]), class::DIRECT, peer, 1, 1),
        (Id16([0x40; 16]), class::GROUP, other, 2, 2),
        (Id16([0x50; 16]), class::DIRECT, other, 0, 0),
        (Id16([0x60; 16]), class::SYMBOL, other, 3, 3),
        // A class-5 announcement may have a receiver-local effective priority
        // different from its signed byte, but remains in the existing D3
        // relay-spray domain until a partition decision is approved.
        (Id16([0x70; 16]), class::ANNOUNCEMENT, other, 0, 2),
        (Id16([0x71; 16]), class::ANNOUNCEMENT, other, 1, 2),
    ];
    for (id, bundle_class, dest, priority, effective_priority) in entries {
        n.core.store.put(
            BundleMeta {
                id,
                class: bundle_class,
                origin,
                dest,
                priority,
                effective_priority,
                size: 0,
                stored_at: 0,
                expires_at: OFFER_MIN_LIFETIME_MS + 1,
                budget: 1,
                own_origin: false,
                slot: SlotState::Stored,
            },
            &[],
        );
    }

    let view = SessionHost::ids(&mut n.core, &peer);
    assert_eq!(
        view.ids_in(0, &Id16::MIN, &Id16::MAX),
        vec![Id16([0x10; 16]), Id16([0x20; 16])],
    );
    assert_eq!(view.ids_in(1, &Id16::MIN, &Id16::MAX), vec![Id16([0x30; 16])]);
    assert_eq!(view.ids_in(2, &Id16::MIN, &Id16::MAX), vec![Id16([0x40; 16])]);
    assert_eq!(
        view.ids_in(3, &Id16::MIN, &Id16::MAX),
        vec![Id16([0x50; 16]), Id16([0x60; 16]), Id16([0x70; 16]), Id16([0x71; 16])],
    );
}

#[test]
fn want_handoffs_use_declared_transfer_order_without_priority_partition() {
    let mut n = node(250);
    n.core.store = Box::new(MemStore::new());
    let peer = Id16([0xD0; 16]);
    let other = Id16([0xD1; 16]);
    let origin = n.node_id();
    let entries = [
        // Deliberately arrive in reverse value order.  A local clamp on the
        // two announcements must not affect their declared-byte ordering.
        (Id16([0x05; 16]), class::ANNOUNCEMENT, other, 3, 3),
        (Id16([0x04; 16]), class::ANNOUNCEMENT, other, 1, 2),
        (Id16([0x03; 16]), class::ANNOUNCEMENT, other, 0, 2),
        (Id16([0x02; 16]), class::DIRECT, peer, 3, 3),
        (Id16([0x01; 16]), class::CONTROL, other, 3, 3),
    ];
    for (id, bundle_class, dest, priority, effective_priority) in entries {
        n.core.store.put(
            BundleMeta {
                id,
                class: bundle_class,
                origin,
                dest,
                priority,
                effective_priority,
                size: 0,
                stored_at: 0,
                expires_at: OFFER_MIN_LIFETIME_MS + 1,
                budget: 1,
                own_origin: false,
                slot: SlotState::Stored,
            },
            &[],
        );
    }

    let mut ids = vec![
        Id16([0x05; 16]),
        Id16([0x04; 16]),
        Id16([0x03; 16]),
        Id16([0x02; 16]),
        Id16([0x01; 16]),
        Id16([0xFF; 16]), // expired between RBSR and WANT: unknown locally
    ];
    SessionHost::order_handoffs(&mut n.core, &peer, &mut ids);

    assert_eq!(
        ids,
        vec![
            Id16([0x01; 16]), // control
            Id16([0x02; 16]), // destination here
            Id16([0x03; 16]), // declared emergency
            Id16([0x04; 16]), // declared operational, locally clamped
            Id16([0x05; 16]), // declared bulk
            Id16([0xFF; 16]), // unknown request remains harmlessly last
        ],
    );
}

#[test]
fn callback_event_queue_is_bounded_when_host_does_not_poll() {
    let mut n = node(251);
    while n.poll_event().is_some() {}
    for _ in 0..(super::EVENT_QUEUE_CAPACITY * 2) {
        n.core.enqueue_event(Event::TierChanged { tier: Tier::Relay });
    }

    assert_eq!(n.core.events.len(), super::EVENT_QUEUE_CAPACITY);
    assert_eq!(n.core.event_queue_dropped, super::EVENT_QUEUE_CAPACITY as u64);
}

#[test]
fn node_store_rejection_prevents_delivery_and_post_store_side_effects() {
    let mut alice = node(255);
    let mut bob = Node::new([254; 32], Box::new(CapacityRejectStore), Tier::Relay);
    introduce(&mut alice, &mut bob);
    drain(&mut alice);
    drain(&mut bob);

    let message_id = alice.send_text(&bob.node_id(), "provider capacity").unwrap();
    let (_, raw) = alice.core.store.get(&message_id).expect("source bundle stored");

    bob.core.on_bundle(&alice.node_id(), &raw);

    assert_eq!(bob.core.store.len(), 0, "provider rejection must not retain bytes");
    assert_eq!(bob.core.diag.drops, 1, "rejected ingest must be visible as a local drop");
    assert!(bob.core.seen.contains(&message_id), "first-sight dedup still prevents replay loops");
    assert!(
        !bob.core.events.iter().any(|event| matches!(event, Event::MessageReceived { id, .. } if *id == message_id)),
        "delivery must not run before a durable store accepts the bundle"
    );
}

#[test]
fn proxy_summary_hints_are_bounded_per_peer() {
    let mut n = node(252);
    let peer = Id16([0xabu8; 16]);
    for value in 0..(super::MAX_PROXY_HINTS_PER_PEER * 2) {
        let mut dependent = [0u8; 16];
        dependent[..8].copy_from_slice(&(value as u64).to_le_bytes());
        let body = ProxySummaryBody { dependent: Id16(dependent), bloom: Vec::new() }.encode();
        n.core.on_proxy_summary(&peer, &body);
    }

    assert_eq!(n.core.proxy_hints.get(&peer).map(HashSet::len), Some(super::MAX_PROXY_HINTS_PER_PEER));

    for value in 0..(super::MAX_PROXY_HINT_PEERS * 2) {
        let mut peer_id = [0u8; 16];
        peer_id[..8].copy_from_slice(&(value as u64).to_le_bytes());
        let body = ProxySummaryBody {
            dependent: Id16([0x55; 16]),
            bloom: Vec::new(),
        }
        .encode();
        n.core.on_proxy_summary(&Id16(peer_id), &body);
    }
    assert!(n.core.proxy_hints.len() <= super::MAX_PROXY_HINT_PEERS);
}

#[test]
fn blob_serve_counters_are_bounded_for_unseen_peers() {
    let mut n = node(253);
    let empty_root = Id16([0x33; 16]);
    for value in 0..(super::MAX_BLOB_SERVE_PEERS * 2) {
        let mut peer = [0u8; 16];
        peer[..8].copy_from_slice(&(value as u64).to_le_bytes());
        n.core.blob_serve_candidates(&Id16(peer), &empty_root, 0, &[0u8; 5], false);
    }
    assert!(n.core.blob_served.is_empty(), "empty requests must not consume peer slots");

    let root = Id16([0x44; 16]);
    n.core.blob_index.insert(root, BTreeMap::from([(0, Id16([0x66; 16]))]));
    for value in 0..(super::MAX_BLOB_SERVE_PEERS * 2) {
        let mut peer = [0u8; 16];
        peer[..8].copy_from_slice(&(value as u64).to_le_bytes());
        n.core.blob_serve_candidates(&Id16(peer), &root, 0, &[0u8; 5], false);
    }

    assert_eq!(n.core.blob_served.len(), super::MAX_BLOB_SERVE_PEERS);
}

/// A shape-valid class-4 bundle with a malformed Merkle payload must not be
/// retained in the holder index merely because no local fetch is active.  If
/// it were indexed first, a later BLOB_HAVE/BLOB_REQ could serve the bytes to
/// another peer without a verification pass.
#[test]
fn unsolicited_invalid_symbol_is_removed_before_indexing() {
    let sender = node(230);
    let mut receiver = node(231);
    let root = Id16([0x91; 16]);
    let mut bundle = Bundle {
        class: class::SYMBOL,
        flags: 0,
        origin: sender.identity.public(),
        dest: receiver.node_id(),
        lifetime_ms: LIFETIME_MESSAGE_MS,
        priority: prio::BULK,
        exts: vec![Ext::SymbolInfo { root, block: 0, sym: 0, k: 32, n: 40 }],
        // The payload has the required symbol length and a decodable path
        // envelope (depth zero), but the path cannot prove this random root.
        payload: SymbolPayload { path: vec![0], symbol: vec![0u8; SYMBOL_SIZE] }.encode(),
        signature: Sig64::ZERO,
    };
    bundle.signature = sender.identity.sign(SigDomain::Bundle, &bundle.canonical_zeroed());
    let id = bundle.id();

    receiver.core.on_bundle(&sender.node_id(), &bundle.encode());

    assert!(receiver.core.store.meta(&id).is_none(), "invalid symbol must not remain durable");
    assert!(!receiver.core.symbol_of.contains_key(&id), "invalid symbol must not enter reverse index");
    assert!(!receiver.core.blob_index.contains_key(&root), "invalid symbol must not enter holder index");
}

#[test]
fn active_invalid_symbol_is_counted_and_removed() {
    let sender = node(228);
    let mut receiver = node(229);
    let root = Id16([0x92; 16]);
    receiver.core.offers.insert(
        root,
        crate::blob::FileOfferInfo {
            size: 1_000,
            full_hash: [0; 32],
            sharer: sender.identity.public(),
        },
    );
    let dest = std::env::temp_dir().join(format!("mmp-node-invalid-symbol-{}.part", std::process::id()));
    receiver.core.fetch_file(&root, &dest).expect("cached offer admits fetch");

    let mut bundle = Bundle {
        class: class::SYMBOL,
        flags: 0,
        origin: sender.identity.public(),
        dest: receiver.node_id(),
        lifetime_ms: LIFETIME_MESSAGE_MS,
        priority: prio::BULK,
        exts: vec![Ext::SymbolInfo { root, block: 0, sym: 0, k: 32, n: 40 }],
        payload: SymbolPayload { path: vec![0], symbol: vec![0u8; SYMBOL_SIZE] }.encode(),
        signature: Sig64::ZERO,
    };
    bundle.signature = sender.identity.sign(SigDomain::Bundle, &bundle.canonical_zeroed());
    let id = bundle.id();
    receiver.core.on_bundle(&sender.node_id(), &bundle.encode());

    let assembly = receiver.core.assemblies.get(&root).expect("fetch remains visible");
    assert_eq!(assembly.bad_symbols, 1, "invalid symbol is attributable to the active fetch");
    assert!(receiver.core.store.meta(&id).is_none(), "invalid symbol must be removed from storage");
    assert!(!receiver.core.symbol_of.contains_key(&id));
    assert!(!receiver.core.blob_index.contains_key(&root));

    let _ = std::fs::remove_file(dest);
}

#[test]
fn blob_index_replacement_and_receipt_purge_keep_indexes_consistent() {
    let mut n = node(236);
    n.core.store = Box::new(MemStore::new());
    let root = Id16([0x41; 16]);
    let old_id = Id16([0x42; 16]);
    let new_id = Id16([0x43; 16]);
    let destination_key = Key32([0x44; 32]);
    let destination = node_id_of(&destination_key);
    let symbol_ext = Ext::SymbolInfo { root, block: 0, sym: 0, k: 32, n: 40 };
    let old_bundle = Bundle {
        class: class::SYMBOL,
        flags: 0,
        origin: Key32([0x45; 32]),
        dest: destination,
        lifetime_ms: LIFETIME_MESSAGE_MS,
        priority: prio::BULK,
        exts: vec![symbol_ext],
        payload: Vec::new(),
        signature: Sig64::ZERO,
    };
    let new_bundle = Bundle { origin: Key32([0x46; 32]), ..old_bundle.clone() };
    let meta = |id, origin| BundleMeta {
        id,
        class: class::SYMBOL,
        origin: node_id_of(&origin),
        dest: destination,
        priority: prio::BULK,
        effective_priority: prio::BULK,
        size: 0,
        stored_at: 0,
        expires_at: LIFETIME_MESSAGE_MS,
        budget: 1,
        own_origin: false,
        slot: SlotState::Stored,
    };

    n.core.store.put(meta(old_id, old_bundle.origin), &[]);
    n.core.index_symbol_bundle(&old_bundle, old_id);
    n.core.store.put(meta(new_id, new_bundle.origin), &[]);
    n.core.index_symbol_bundle(&new_bundle, new_id);
    assert!(!n.core.symbol_of.contains_key(&old_id));
    assert_eq!(n.core.blob_index.get(&root).and_then(|idx| idx.get(&0)), Some(&new_id));

    n.core.deindex_symbol(&old_id);
    assert_eq!(n.core.blob_index.get(&root).and_then(|idx| idx.get(&0)), Some(&new_id));

    let receipt = Bundle {
        class: class::CONTROL,
        flags: 0,
        origin: destination_key,
        dest: n.core.node_id,
        lifetime_ms: LIFETIME_CONTROL_MS,
        priority: prio::EMERGENCY,
        exts: Vec::new(),
        payload: ReceiptPayload { delivered: new_id, mac: [0; 16] }.encode(),
        signature: Sig64::ZERO,
    };
    n.core.handle_receipt(&receipt, false);
    assert!(n.core.store.meta(&new_id).is_none());
    assert!(!n.core.symbol_of.contains_key(&new_id));
    assert!(!n.core.blob_index.contains_key(&root));
}

#[test]
fn known_identity_cache_releases_after_contact_teardown() {
    let mut a = node(254);
    for (link, seed) in (0u64..16).zip(0u8..16) {
        let mut b = node(seed);
        contact(&mut a, &mut b, link, link + 1);
        assert!(
            a.core.known_identities.is_empty(),
            "identity cache must not retain peer {} after its session is down",
            seed
        );
    }
}

#[test]
fn known_identity_cache_survives_another_live_link() {
    let mut a = node(240);
    let mut b = node(241);
    establish(&mut a, &mut b, 1);
    establish(&mut a, &mut b, 2);

    let peer = b.node_id();
    assert_eq!(a.core.known_identities.len(), 1);
    a.on_tal_event(TalEvent::LinkDown { link: 1 });
    assert!(a.core.known_identities.contains_key(&peer));

    a.on_tal_event(TalEvent::LinkDown { link: 2 });
    assert!(a.core.known_identities.is_empty());
}

#[test]
fn new_conversation_contacts_are_rejected_at_local_state_cap() {
    let mut n = node(238);
    let identity = Key32([0xd1; 32]);
    let peer = Id16([0xe2; 16]);
    let conv_id = conversation_id(&n.core.public, &identity);

    for value in 0..super::MAX_CONVERSATION_STATE {
        let mut id = [0u8; 16];
        id[..8].copy_from_slice(&(value as u64).to_le_bytes());
        n.core.established_convs.insert(Id16(id));
    }
    // Make the assertion deterministic even if the generated conversation ID
    // happens to overlap one of the synthetic marker IDs above.
    n.core.established_convs.remove(&conv_id);
    while n.core.established_convs.len() < super::MAX_CONVERSATION_STATE {
        let mut id = [0xff; 16];
        id[..8].copy_from_slice(&(n.core.established_convs.len() as u64).to_le_bytes());
        n.core.established_convs.insert(Id16(id));
    }

    assert!(!n.core.accept_peer(&identity, &peer));
    assert!(!n.core.known_identities.contains_key(&peer));
    assert_eq!(n.core.established_convs.len(), super::MAX_CONVERSATION_STATE);
}

#[test]
fn outbound_conversation_cap_is_side_effect_free_for_text_and_file() {
    let mut n = node(235);
    let mut peer = node(234);
    introduce(&mut n, &mut peer);
    for value in 0..super::MAX_CONVERSATION_STATE {
        let mut id = [0u8; 16];
        id[..8].copy_from_slice(&(value as u64).to_le_bytes());
        n.core.established_convs.insert(Id16(id));
    }
    let before_total = n.store_len();
    let before_symbols = n.core.store.all_meta().iter().filter(|m| m.class == class::SYMBOL).count();

    assert_eq!(n.send_text(&peer.node_id(), "at capacity"), Err(SendError::ResourceLimit));
    assert_eq!(n.store_len(), before_total);
    assert_eq!(n.core.store.all_meta().iter().filter(|m| m.class == class::SYMBOL).count(), before_symbols);

    assert_eq!(
        n.send_file(&peer.node_id(), "at-capacity.bin", "application/octet-stream", b"payload"),
        Err(SendError::ResourceLimit)
    );
    assert_eq!(n.store_len(), before_total);
    assert_eq!(n.core.store.all_meta().iter().filter(|m| m.class == class::SYMBOL).count(), before_symbols);
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

/// Establish a session and drain its normal synchronization output, leaving
/// both physical links up for tests that exercise post-handshake close paths.
fn establish(a: &mut Node, b: &mut Node, link: u64) {
    a.tick(0);
    b.tick(0);
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
    let mut peer = Node::new([113; 32], Box::new(MemStore::new()), Tier::Leaf);
    establish(&mut n, &mut peer, 1);

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
    assert!(
        n.core.known_identities.is_empty(),
        "engine closure must release the peer identity before LinkDown"
    );
    assert_eq!(n.poll_close(), None, "physical close waits for terminal wire frames");
    let terminal_types: Vec<u8> = std::iter::from_fn(|| n.poll_send().map(|(_, bytes)| bytes[2])).collect();
    assert_eq!(terminal_types, vec![FT_ERROR, FT_CLOSE]);
    assert_eq!(n.poll_close(), Some(1), "local session closure must reach the transport host");

    link_up(&mut n, 2);
    assert_eq!(n.sessions.len(), 1, "Closed must release the admission slot");
}

#[test]
fn timer_close_flushes_close_before_physical_teardown() {
    let mut n = Node::new([111; 32], Box::new(MemStore::new()), Tier::Leaf);
    let mut peer = Node::new([114; 32], Box::new(MemStore::new()), Tier::Leaf);
    establish(&mut n, &mut peer, 1);

    n.tick(10_001); // SESSION_IDLE_MS + 1 in the frozen timer table.
    assert!(!n.sessions.contains_key(&1));
    assert_eq!(n.poll_close(), None, "CLOSE must be drained before teardown");
    let (link, bytes) = n.poll_send().expect("timer close frame");
    assert_eq!(link, 1);
    assert_eq!(bytes[2], FT_CLOSE);
    assert!(n.poll_send().is_none());
    assert_eq!(n.poll_close(), Some(1));
}

#[test]
fn terminal_close_preserves_unrelated_link_outbox() {
    let mut n = Node::new([112; 32], Box::new(MemStore::new()), Tier::Leaf);
    let mut peer = Node::new([115; 32], Box::new(MemStore::new()), Tier::Leaf);
    establish(&mut n, &mut peer, 1);
    n.outbox.push_back((99, vec![0, 0, 0x20, 0xaa]));

    let oversized_declared_len = (MAX_ENVELOPE_PAYLOAD + 1) as u16;
    let mut malformed = vec![0u8; 3];
    malformed[..2].copy_from_slice(&oversized_declared_len.to_le_bytes());
    n.on_tal_event(TalEvent::LinkData { link: 1, bytes: malformed });

    let queued: Vec<(u64, u8)> = std::iter::from_fn(|| n.poll_send().map(|(link, bytes)| (link, bytes[2]))).collect();
    assert_eq!(queued, vec![(99, 0x20), (1, FT_ERROR), (1, FT_CLOSE)]);
    assert_eq!(n.poll_close(), Some(1));
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
fn sent_history_evicts_terminal_entries_but_rejects_when_all_active() {
    let mut n = node(239);
    let conv = Id16([0x71; 16]);
    for value in 0..super::MAX_SENT_RUNTIME {
        let mut id = [0u8; 16];
        id[..8].copy_from_slice(&(value as u64).to_le_bytes());
        n.core.sent.insert(Id16(id), SentMessage { conv, state: DeliveryState::Delivered });
    }
    assert!(n.core.sent_capacity_available());
    n.core.record_sent(Id16([0xf1; 16]), conv).unwrap();
    assert_eq!(n.core.sent.len(), super::MAX_SENT_RUNTIME);
    assert_eq!(n.core.sent.get(&Id16([0xf1; 16])).map(|m| m.state), Some(DeliveryState::Queued));

    n.core.sent.clear();
    for value in 0..super::MAX_SENT_RUNTIME {
        let mut id = [0u8; 16];
        id[..8].copy_from_slice(&(value as u64).to_le_bytes());
        n.core.sent.insert(Id16(id), SentMessage { conv, state: DeliveryState::InMesh });
    }
    assert!(!n.core.sent_capacity_available());
    assert_eq!(n.core.record_sent(Id16([0xf2; 16]), conv), Err(SendError::ResourceLimit));
    assert!(!n.core.sent.contains_key(&Id16([0xf2; 16])));
}

#[test]
fn saturated_active_sent_history_rejects_before_bundle_creation() {
    let mut a = node(240);
    let mut b = node(241);
    introduce(&mut a, &mut b);
    let conv = conversation_id(&a.core.public, &b.core.public);
    for value in 0..super::MAX_SENT_RUNTIME {
        let mut id = [0u8; 16];
        id[..8].copy_from_slice(&(value as u64).to_le_bytes());
        a.core.sent.insert(Id16(id), SentMessage { conv, state: DeliveryState::InMesh });
    }
    let before_store = a.store_len();
    assert_eq!(a.send_text(&b.node_id(), "blocked"), Err(SendError::ResourceLimit));
    assert_eq!(a.store_len(), before_store);
    assert!(!a.core.convs.contains_key(&conv));
}

#[test]
fn direct_send_reports_ratchet_exhaustion_without_emitting_bundle() {
    let mut alice = node(1);
    let mut bob = node(2);
    introduce(&mut alice, &mut bob);
    drain(&mut alice);
    drain(&mut bob);

    // Establish the conversation and clear first-contact receipt gating.
    alice.send_text(&bob.node_id(), "bootstrap").unwrap();
    contact(&mut alice, &mut bob, 1, 10_000);
    drain(&mut alice);
    drain(&mut bob);
    contact(&mut alice, &mut bob, 2, 20_000);
    drain(&mut alice);
    drain(&mut bob);

    let conv_id = conversation_id(&alice.core.public, &bob.core.public);
    alice.core.convs.get_mut(&conv_id).unwrap().send.next_index = u32::MAX;

    // The terminal index is still valid once; the following attempt must
    // return the explicit public error and leave the durable store unchanged.
    alice.send_text(&bob.node_id(), "terminal index").unwrap();
    let stored_after_terminal = alice.store_len();
    assert_eq!(
        alice.send_text(&bob.node_id(), "after exhaustion"),
        Err(SendError::RatchetExhausted)
    );
    assert_eq!(alice.store_len(), stored_after_terminal);
}

#[test]
fn link_down_retires_conversation_at_epoch_exhaustion() {
    let mut alice = node(3);
    let mut bob = node(4);
    introduce(&mut alice, &mut bob);
    drain(&mut alice);
    drain(&mut bob);

    alice.send_text(&bob.node_id(), "bootstrap").unwrap();
    contact(&mut alice, &mut bob, 1, 10_000);
    drain(&mut alice);
    drain(&mut bob);
    contact(&mut alice, &mut bob, 2, 20_000);
    drain(&mut alice);
    drain(&mut bob);

    let conv_id = conversation_id(&alice.core.public, &bob.core.public);
    alice.core.convs.get_mut(&conv_id).unwrap().send.epoch = u16::MAX;
    bob.core.convs.get_mut(&conv_id).unwrap().send.epoch = u16::MAX;

    // Link teardown must not call the panic-compatible epoch wrapper. Both
    // peers fail closed by retiring the unusable conversation state.
    contact(&mut alice, &mut bob, 3, 30_000);
    assert!(!alice.core.convs.contains_key(&conv_id));
    assert!(!bob.core.convs.contains_key(&conv_id));
    assert!(!alice.core.established_convs.contains(&conv_id));
    assert!(!bob.core.established_convs.contains(&conv_id));
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
fn malformed_sealed_async_does_not_commit_ratchet_state() {
    let mut alice = node(51);
    let mut bob = node(52);
    introduce_exchange_only(&mut alice, &mut bob);
    let msg_id = alice.send_text(&bob.node_id(), "sealed candidate state").unwrap();
    let (_, raw) = alice.core.store.get(&msg_id).expect("sealed bundle stored");
    let bundle = Bundle::decode_canonical(&raw).unwrap();
    let conv_id = mmp_crypto::conversation_id(&alice.core.public, &bob.core.public);

    let mut forged = bundle.clone();
    let AsyncPayload::Sealed { eph, mut ct } = AsyncPayload::decode(&forged.payload).unwrap() else {
        panic!("exchange-only bootstrap should use sealed async form");
    };
    ct[0] ^= 0x80;
    forged.payload = AsyncPayload::Sealed { eph, ct }.encode();
    forged.signature = alice.identity.sign(SigDomain::Bundle, &forged.canonical_zeroed());

    assert!(bob.core.open_async(&forged, &conv_id, &alice.core.public).is_none());
    assert!(!bob.core.convs.contains_key(&conv_id), "failed AEAD must not commit Conv");
    assert!(!bob.core.pending_async.contains_key(&conv_id), "failed AEAD must not commit pending X3DH state");
    assert!(!bob.core.pending_first_contact.contains_key(&conv_id), "failed AEAD must not commit pending first-contact state");

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
    let original_receipt_key = *bob
        .core
        .convs
        .get(&conv_id)
        .expect("conversation established")
        .receipt_key;
    assert!(bob.core.established_convs.contains(&conv_id));

    let second = alice
        .core
        .send_sealed(&bob.node_id(), &bob.core.public, conv_id, apptype::TEXT, b"stale root")
        .unwrap();
    assert_ne!(first, second);
    let (_, raw) = alice.core.store.get(&second).unwrap();
    let sealed = Bundle::decode_canonical(&raw).unwrap();
    assert!(bob.core.open_async(&sealed, &conv_id, &alice.core.public).is_none());
    assert_eq!(
        *bob.core.convs.get(&conv_id).unwrap().receipt_key,
        original_receipt_key,
        "rejected sealed root must not replace receipt state"
    );
}

#[test]
fn oversized_prekey_first_send_does_not_poison_async_state() {
    let mut alice = node(47);
    let mut bob = node(48);
    introduce(&mut alice, &mut bob);

    let oversized = "x".repeat(MAX_BUNDLE);
    assert_eq!(alice.send_text(&bob.node_id(), &oversized), Err(SendError::TooLarge));
    let conv_id = mmp_crypto::conversation_id(&alice.core.public, &bob.core.public);
    assert!(!alice.core.convs.contains_key(&conv_id));
    assert!(!alice.core.pending_async.contains_key(&conv_id));
    assert!(!alice.core.pending_first_contact.contains_key(&conv_id));
    assert!(alice.core.sent.is_empty());

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

#[test]
fn send_file_pending_rejects_before_minting_new_symbols() {
    let mut alice = node(53);
    let mut bob = node(54);
    introduce_exchange_only(&mut alice, &mut bob);

    let first_data = vec![0x11; 3000];
    alice
        .send_file(&bob.node_id(), "first.bin", "application/octet-stream", &first_data)
        .expect("first file manifest establishes the pending root");
    let symbols_before = alice
        .core
        .store
        .all_meta()
        .iter()
        .filter(|meta| meta.class == mmp_types::bundle::class::SYMBOL)
        .count();

    let second_data = vec![0x22; 3000];
    assert_eq!(
        alice.send_file(&bob.node_id(), "second.bin", "application/octet-stream", &second_data),
        Err(SendError::ConversationPending)
    );
    let symbols_after = alice
        .core
        .store
        .all_meta()
        .iter()
        .filter(|meta| meta.class == mmp_types::bundle::class::SYMBOL)
        .count();
    assert_eq!(symbols_after, symbols_before, "rejected manifest must not mint symbols");
}

#[test]
fn send_file_revoked_peer_is_transactionally_side_effect_free() {
    let mut alice = node(55);
    let mut bob = node(56);
    introduce(&mut alice, &mut bob);
    drain(&mut alice);
    drain(&mut bob);

    bob.revoke_identity(0);
    for claim in bob.own_claims() {
        alice.ingest_claim(claim, true);
    }
    drain(&mut alice);

    let symbol_count_before = alice
        .core
        .store
        .all_meta()
        .iter()
        .filter(|meta| meta.class == mmp_types::bundle::class::SYMBOL)
        .count();
    let blob_index_before = alice.core.blob_index.clone();
    let symbol_of_before = alice.core.symbol_of.clone();

    assert_eq!(
        alice.send_file(&bob.node_id(), "revoked.bin", "application/octet-stream", &[0x5a; 3000]),
        Err(SendError::PeerRevoked)
    );
    let symbol_count_after = alice
        .core
        .store
        .all_meta()
        .iter()
        .filter(|meta| meta.class == mmp_types::bundle::class::SYMBOL)
        .count();
    assert_eq!(symbol_count_after, symbol_count_before, "rejected file must not mint symbol bundles");
    assert_eq!(alice.core.blob_index, blob_index_before, "rejected file must not index symbols");
    assert_eq!(alice.core.symbol_of, symbol_of_before, "rejected file must not populate reverse index");
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
fn network_group_bootstrap_is_bounded() {
    let mut alice = node(31);
    let mut bob = node(32);
    introduce(&mut alice, &mut bob);

    for i in 0..crate::group::MAX_GROUPS {
        let mut gid = [0u8; 16];
        gid[..8].copy_from_slice(&(i as u64).to_le_bytes());
        let gid = Id16(gid);
        let dist = mmp_crypto::group::LocalSenderKey::generate(&mut alice.core.rng, gid, 1).dist().clone();
        let mut body = Vec::with_capacity(16 + dist.encode().len());
        body.extend_from_slice(&alice.node_id().0);
        body.extend_from_slice(&dist.encode());
        bob.core.handle_sender_key(&alice.node_id(), &body);
    }
    assert_eq!(bob.core.groups.len(), crate::group::MAX_GROUPS);

    let gid = Id16([0xee; 16]);
    let dist = mmp_crypto::group::LocalSenderKey::generate(&mut alice.core.rng, gid, 1).dist().clone();
    let mut body = Vec::with_capacity(16 + dist.encode().len());
    body.extend_from_slice(&alice.node_id().0);
    body.extend_from_slice(&dist.encode());
    bob.core.handle_sender_key(&alice.node_id(), &body);
    assert!(!bob.core.groups.contains_key(&gid));

    let mut restored = node(33);
    for blob in bob.all_group_blobs() {
        assert!(restored.restore_group(&blob));
    }
    let alice_id = alice.node_id();
    let extra = crate::group::GroupState::create(&mut alice.core.rng, Id16([0xdd; 16]), alice_id, true);
    assert!(!restored.restore_group(&extra.encode()));
}

#[test]
fn non_admin_cannot_invite_into_an_immutable_v1_group() {
    let mut alice = node(21);
    let mut bob = node(22);
    let mut carol = node(23);
    introduce(&mut alice, &mut bob);
    introduce(&mut alice, &mut carol);
    introduce(&mut bob, &mut carol);

    let gid = alice.create_group();
    alice.group_invite(&gid, &bob.node_id()).unwrap();
    contact(&mut alice, &mut bob, 94, 940_000);
    drain(&mut alice);
    drain(&mut bob);

    assert_eq!(bob.group_invite(&gid, &carol.node_id()), Err(SendError::UnknownPeer));
    assert!(!bob.group_members(&gid).contains(&carol.node_id()));
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
    let rotated_epoch = alice.core.groups.get(&gid).unwrap().epoch;
    assert!(
        alice
            .core
            .sent_dists
            .iter()
            .all(|(sent_gid, _, sent_epoch)| *sent_gid != gid || *sent_epoch == rotated_epoch),
        "stale sender-key distribution markers must not survive an epoch rotation"
    );
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
fn rejected_group_invite_does_not_mutate_membership() {
    let mut alice = node(62);
    let gid = alice.create_group();
    let me = alice.node_id();
    let stranger = Id16([0xa5; 16]);

    assert_eq!(alice.group_invite(&gid, &stranger), Err(SendError::UnknownPeer));
    assert_eq!(alice.group_members(&gid), vec![me]);

    let mut bob = node(63);
    introduce(&mut alice, &mut bob);
    alice.send_text(&bob.node_id(), "first contact").expect("queue first-contact root");
    assert_eq!(
        alice.group_invite(&gid, &bob.node_id()),
        Err(SendError::ConversationPending)
    );
    assert_eq!(alice.group_members(&gid), vec![me]);
}

#[test]
fn oversized_restored_group_send_does_not_distribute_before_rejection() {
    let mut alice = node(64);
    let mut bob = node(65);
    introduce(&mut alice, &mut bob);
    let gid = alice.create_group();
    alice.group_invite(&gid, &bob.node_id()).expect("known peer");
    let blob = alice
        .all_group_blobs()
        .into_iter()
        .find(|candidate| candidate[..16] == gid.0)
        .expect("group snapshot");

    // Restore the same identity from the metadata-only snapshot. The fresh
    // local sender key sets needs_distribution, so a naive group_send would
    // queue a key distribution before discovering the oversized bundle.
    let mut restored = node(64);
    introduce(&mut restored, &mut bob);
    assert!(restored.restore_group(&blob));
    assert!(restored.core.groups.get(&gid).unwrap().needs_distribution());

    assert_eq!(
        restored.group_send(&gid, &"x".repeat(MAX_BUNDLE - 1)),
        Err(SendError::TooLarge)
    );
    assert!(restored.core.groups.get(&gid).unwrap().needs_distribution());
    assert!(restored.poll_send().is_none(), "rejected send must not queue a distribution");
}

#[test]
fn oversized_group_send_does_not_consume_sender_chain_index() {
    let mut alice = node(61);
    let gid = alice.create_group();

    assert_eq!(alice.group_send(&gid, &"x".repeat(MAX_BUNDLE)), Err(SendError::TooLarge));

    let id = alice.group_send(&gid, "after rejection").expect("valid group retry");
    let (_, raw) = alice.core.store.get(&id).expect("group bundle stored");
    let bundle = Bundle::decode_canonical(&raw).expect("well-formed group bundle");
    let payload = GroupPayload::decode(&bundle.payload).expect("well-formed group payload");
    assert_eq!(payload.chain_index, 0, "rejected send must not consume index 0");
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

#[test]
fn blocklist_snapshot_is_bounded_and_roundtrips_without_mutating_on_error() {
    let mut n = node(60);
    let blocked = Id16([61; 16]);
    n.block(blocked);
    let snapshot = n.blocklist_snapshot();

    let mut restored = node(62);
    assert!(restored.restore_blocklist(&snapshot));
    assert!(restored.core.blocklist.contains(&blocked));

    let before = restored.blocklist_snapshot();
    let mut malformed = snapshot.clone();
    malformed.push(0);
    assert!(!restored.restore_blocklist(&malformed));
    assert_eq!(restored.blocklist_snapshot(), before);

    let hostile_count = [1u8, 0xff, 0xff];
    assert!(!restored.restore_blocklist(&hostile_count));
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
fn announcement_rejects_invalid_priority_before_storing() {
    let mut alice = node(30);
    let scope = Id16([0x31; 16]);
    let before = alice.core.store.len();

    assert_eq!(
        alice.publish_announcement(scope, 1, prio::BULK + 1, b"invalid", None),
        Err(SendError::InvalidPriority)
    );
    assert_eq!(alice.core.store.len(), before, "invalid input must not create a bundle");
}

#[test]
fn unknown_announcement_suite_is_relay_only_and_not_rendered() {
    let alice = node(31);
    let mut bob = node(32);
    let scope = bob.subscribe_scope(b"future/announcement-suite");
    let mut body = AnnouncementBody {
        suite: 0x02,
        ann_type: 1,
        scope,
        not_after: 0,
        text: b"future grammar".to_vec(),
        sig: Sig64::ZERO,
    };
    body.sig = alice.core.identity.sign(SigDomain::Announce, &body.signed_object());
    let mut bundle = Bundle {
        class: mmp_types::bundle::class::ANNOUNCEMENT,
        flags: 0,
        origin: alice.public_key(),
        dest: scope,
        lifetime_ms: 60_000,
        priority: prio::OPERATIONAL,
        exts: Vec::new(),
        payload: body.encode(),
        signature: Sig64::ZERO,
    };
    bundle.signature = alice.core.identity.sign(SigDomain::Bundle, &bundle.canonical_zeroed());

    bob.core.handle_announcement(&bundle);
    assert!(!bob.core.events.iter().any(|event| matches!(event, Event::Announcement { .. })));
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
fn unclocked_authority_cert_fails_closed_even_when_future_dated() {
    let mut alice = node(13);
    let mut bob = node(14);
    let authority = mmp_crypto::Identity::from_seed(&[15; 32]);
    let scope = bob.subscribe_scope(b"clock/unset");
    bob.pin_authority_root(scope, authority.public());

    // A zero-initialized clock would incorrectly accept this certificate.
    // The readiness bit must keep the attached certificate unusable until a
    // host supplies an actual wall-clock reading.
    let cert = signed_cert(
        &authority,
        alice.public_key(),
        scope,
        mmp_types::authcert::CAP_ANNOUNCE,
        10,
        u32::MAX,
    );
    alice
        .publish_announcement(scope, 1, prio::OPERATIONAL, b"clock not ready", Some(&cert))
        .unwrap();

    contact(&mut alice, &mut bob, 13, 130_000);
    let ann = drain(&mut bob)
        .into_iter()
        .find_map(|event| match event {
            Event::Announcement { effective_priority, official, text, .. } => {
                Some((effective_priority, official, text))
            }
            _ => None,
        })
        .expect("subscribed node still surfaces the announcement");
    assert_eq!(ann.0, prio::NORMAL, "unclocked authority must not elevate");
    assert!(!ann.1, "unclocked authority must not render official");
    assert_eq!(ann.2, b"clock not ready");
}

#[test]
fn wall_clock_rollback_cannot_revalidate_expired_authority() {
    let mut alice = node(16);
    let mut bob = node(17);
    let authority = mmp_crypto::Identity::from_seed(&[18; 32]);
    let scope = bob.subscribe_scope(b"clock/rollback");
    bob.pin_authority_root(scope, authority.public());
    let cert = signed_cert(
        &authority,
        alice.public_key(),
        scope,
        mmp_types::authcert::CAP_ANNOUNCE,
        10,
        1_000,
    );

    bob.set_wall_clock(0);
    alice
        .publish_announcement(scope, 1, prio::OPERATIONAL, b"before rollback", Some(&cert))
        .unwrap();
    contact(&mut alice, &mut bob, 16, 160_000);
    let first = drain(&mut bob).into_iter().find_map(|event| match event {
        Event::Announcement { effective_priority, official, text, .. } if text == b"before rollback" => {
            Some((effective_priority, official))
        }
        _ => None,
    });
    assert_eq!(first, Some((prio::OPERATIONAL, true)));

    // Move past not_after + skew, then attempt to roll the receiver's clock
    // back. The lower sample must be ignored and the certificate stay expired.
    bob.set_wall_clock(2_000);
    bob.set_wall_clock(0);
    alice
        .publish_announcement(scope, 1, prio::OPERATIONAL, b"after rollback", Some(&cert))
        .unwrap();
    contact(&mut alice, &mut bob, 17, 170_000);
    let second = drain(&mut bob).into_iter().find_map(|event| match event {
        Event::Announcement { effective_priority, official, text, .. } if text == b"after rollback" => {
            Some((effective_priority, official))
        }
        _ => None,
    });
    assert_eq!(second, Some((prio::NORMAL, false)), "rollback must not restore authority");
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

/// Terminal blob progress is retained for one interest window, then only the
/// requester-side assembly is reclaimed. Partial holder symbols remain
/// indexed and therefore servable after that cleanup; an unrelated active
/// assembly must not be aged out by the same sweep.
#[test]
fn blob_terminal_gc_preserves_active_and_partial_holder_state() {
    let mut b = node(3);
    let active_root = Id16([43; 16]);
    let abandoned_root = Id16([44; 16]);
    for root in [active_root, abandoned_root] {
        b.core.offers.insert(
            root,
            crate::blob::FileOfferInfo {
                size: 1000,
                full_hash: [0; 32],
                sharer: Key32([1; 32]),
            },
        );
    }

    let active_dest = std::env::temp_dir().join(format!("mmp-node-active-{}.bin", std::process::id()));
    let abandoned_dest =
        std::env::temp_dir().join(format!("mmp-node-abandoned-{}.bin", std::process::id()));
    b.fetch_file(&active_root, &active_dest).unwrap();
    b.fetch_file(&abandoned_root, &abandoned_dest).unwrap();
    b.cancel_fetch(&abandoned_root);
    assert_eq!(
        b.core.assemblies.get(&abandoned_root).and_then(|a| a.terminal_at),
        Some(0)
    );

    // Model one verified partial symbol that the holder still serves even
    // after the requester gives up. No durable bytes are needed to exercise
    // the index-lifetime invariant here.
    let symbol_id = Id16([45; 16]);
    b.core
        .blob_index
        .insert(abandoned_root, BTreeMap::from([(0, symbol_id)]));
    b.core.symbol_of.insert(symbol_id, (abandoned_root, 0));

    b.tick(BLOB_INTEREST_TTL_MS - 1);
    assert_eq!(b.file_progress(&active_root).unwrap().state, crate::BlobState::Wanted);
    assert_eq!(b.file_progress(&abandoned_root).unwrap().state, crate::BlobState::Abandoned);
    assert!(b.core.blob_index.contains_key(&abandoned_root));

    // At the retention boundary the terminal record is gone, but the active
    // request is still within its original interest lifetime and the holder
    // index is untouched.
    b.tick(BLOB_INTEREST_TTL_MS);
    assert_eq!(b.file_progress(&active_root).unwrap().state, crate::BlobState::Wanted);
    assert!(b.file_progress(&abandoned_root).is_none());
    assert_eq!(b.core.blob_index.get(&abandoned_root).and_then(|idx| idx.get(&0)), Some(&symbol_id));
    assert_eq!(b.core.symbol_of.get(&symbol_id), Some(&(abandoned_root, 0)));

    // The active request transitions normally once its own TTL lapses and is
    // retained as terminal progress for a fresh window.
    b.tick(BLOB_INTEREST_TTL_MS + 1);
    assert_eq!(b.file_progress(&active_root).unwrap().state, crate::BlobState::Abandoned);
    assert_eq!(
        b.core.assemblies.get(&active_root).and_then(|a| a.terminal_at),
        Some(BLOB_INTEREST_TTL_MS + 1)
    );

    let _ = std::fs::remove_file(&active_dest);
    let _ = std::fs::remove_file(&abandoned_dest);
}
