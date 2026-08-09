//! Crypto exit tests (HANDOFF §2.2): domain separation negatives, handshake
//! agreement, prekey round-trip, ratchet reorder windows, Merkle corruption.

use alloc::vec;
use alloc::vec::Vec;

use mmp_types::domain::SigDomain;
use mmp_types::frame::Hello;
use mmp_types::ids::{Id16, Key32};

use crate::group::{LocalSenderKey, RemoteSenderKey, SenderKeyDist};
use crate::identity::{verify, Identity};
use crate::merkle;
use crate::noise::{NoiseHandshake, MAX_HANDSHAKE_MESSAGE};
use crate::prekey::{recipient_agree, sealed_ck0, sender_agree, PrekeyStore, PREKEY_NONE};
use crate::ratchet::{direction_chain, RecvChain, SendChain};
use crate::receipt::{mac_eq, receipt_key, receipt_mac};
use crate::sealed::{open_box, seal_box, seal_box_with_root};
use crate::{conversation_id, CryptoError};

fn id(seed: u8) -> Identity {
    Identity::from_seed(&[seed; 32])
}

fn rng() -> impl rand_core::CryptoRngCore {
    use rand::SeedableRng;
    rand::rngs::StdRng::seed_from_u64(42)
}

fn hello() -> Hello {
    Hello { ver: 1, suites: vec![0x01], tier: 2, flags: 0, credits: 8, psm: 0 }
}

fn hello_with_suites(suites: Vec<u8>) -> Hello {
    Hello { ver: 1, suites, tier: 2, flags: 0, credits: 8, psm: 0 }
}

#[test]
fn sign_verify_and_cross_domain_rejects() {
    let alice = id(1);
    let object = b"claim-bytes";
    let sig = alice.sign(SigDomain::Claim, object);
    verify(SigDomain::Claim, &alice.public(), object, &sig).unwrap();
    // AUDIT F4 golden negative: same bytes under a different tag must fail.
    assert_eq!(
        verify(SigDomain::Bundle, &alice.public(), object, &sig),
        Err(CryptoError::BadSignature)
    );
}

#[test]
fn noise_xx_handshake_agrees_and_binds() {
    let alice = id(1);
    let bob = id(2);
    let (msg1, mut a) = NoiseHandshake::initiator(&alice, &hello()).unwrap();
    let mut b = NoiseHandshake::responder(&bob, &hello()).unwrap();

    let out_b = b.advance(&msg1).unwrap();
    assert!(out_b.channel.is_none());
    let msg2 = out_b.reply.unwrap();

    let out_a = a.advance(&msg2).unwrap();
    let mut chan_a = out_a.channel.unwrap();
    let msg3 = out_a.reply.unwrap();

    let out_b = b.advance(&msg3).unwrap();
    assert!(out_b.reply.is_none());
    let mut chan_b = out_b.channel.unwrap();

    // Both sides learned the peer intro carried in the encrypted payloads.
    assert_eq!(chan_a.peer.identity, bob.public());
    assert_eq!(chan_b.peer.identity, alice.public());
    assert_eq!(chan_a.handshake_hash, chan_b.handshake_hash);

    // ID_PROOF both directions (doc 08 §3.2).
    let proof_a = chan_a.make_id_proof(&alice, true);
    chan_b.verify_id_proof(&bob.public(), &proof_a).unwrap();
    let proof_b = chan_b.make_id_proof(&bob, false);
    chan_a.verify_id_proof(&alice.public(), &proof_b).unwrap();

    // Data flows.
    let ct = chan_a.encrypt(b"ping").unwrap();
    assert_eq!(chan_b.decrypt(&ct).unwrap(), b"ping");
    let ct = chan_b.encrypt(b"pong").unwrap();
    assert_eq!(chan_a.decrypt(&ct).unwrap(), b"pong");
}

#[test]
fn noise_xx_rejects_oversized_pre_auth_input_without_poisoning_state() {
    let alice = id(1);
    let bob = id(2);
    let (msg1, _a) = NoiseHandshake::initiator(&alice, &hello()).unwrap();
    let mut b = NoiseHandshake::responder(&bob, &hello()).unwrap();

    // A hostile peer can reach this path before any identity is authenticated.
    // The fixed bound must be checked before Snow is called or the handshake
    // state is taken, and the normal first message must remain usable after a
    // rejected oversized attempt.
    let oversized = vec![0xa5; MAX_HANDSHAKE_MESSAGE + 1];
    assert!(matches!(b.advance(&oversized), Err(CryptoError::Noise)));
    let out_b = b.advance(&msg1).unwrap();
    assert!(out_b.reply.is_some());
    assert!(out_b.reply.as_ref().is_some_and(|m| m.len() <= MAX_HANDSHAKE_MESSAGE));
}

#[test]
fn noise_xx_ignores_nonempty_msg1_payload_until_encrypted_intro() {
    let alice = id(1);
    let bob = id(2);
    let secret = alice.exchange_secret_bytes();
    let params = crate::noise::NOISE_PARAMS.parse().expect("valid noise params");
    let mut raw_initiator = snow::Builder::new(params)
        .local_private_key(&secret[..])
        .build_initiator()
        .unwrap();
    let mut msg1 = vec![0u8; MAX_HANDSHAKE_MESSAGE];
    let n = raw_initiator.write_message(&[0xde, 0xad, 0xbe, 0xef], &mut msg1).unwrap();
    msg1.truncate(n);
    let mut b = NoiseHandshake::responder(&bob, &hello()).unwrap();

    // The cleartext payload is accepted by Noise but must not be parsed as a
    // provisional identity/HELLO.  A deliberately invalid payload therefore
    // still permits the encrypted handshake to complete.
    let msg2 = b.advance(&msg1).unwrap().reply.unwrap();
    let mut payload = vec![0u8; MAX_HANDSHAKE_MESSAGE];
    let n = raw_initiator.read_message(&msg2, &mut payload).unwrap();
    assert!(n >= 32);
    let mut msg3_payload = alice.public().0.to_vec();
    msg3_payload.extend_from_slice(&hello().encode());
    let n = raw_initiator.write_message(&msg3_payload, &mut payload).unwrap();
    payload.truncate(n);
    let out_b = b.advance(&payload).unwrap();
    let channel = out_b.channel.expect("encrypted msg3 completes handshake");
    assert_eq!(channel.peer.identity, alice.public());
}

#[test]
fn chanbind_object_pins_transcript_and_identity_order() {
    let h = [0x11; 32];
    let init = Key32([0x22; 32]);
    let resp = Key32([0x33; 32]);
    let object = crate::noise::chanbind_object(&h, &init, &resp);

    assert_eq!(object.len(), 96);
    assert_eq!(&object[..32], &h);
    assert_eq!(&object[32..64], &init.0);
    assert_eq!(&object[64..], &resp.0);
    assert_ne!(object, crate::noise::chanbind_object(&h, &resp, &init));
}

#[test]
fn noise_xx_handshake_refuses_disjoint_suite_from_initiator() {
    // E6-S2 cipher-agility exercise (doc 03 §8): a build advertising only a
    // suite id this side does not implement must be refused, not silently
    // accepted under whatever primitives happen to be compiled in.
    let alice = id(1);
    let bob = id(2);
    // msg1 (`-> e`) carries no payload; the initiator's HELLO only reaches
    // the responder inside msg3, so the refusal surfaces on that read.
    let (msg1, mut a) = NoiseHandshake::initiator(&alice, &hello_with_suites(vec![0x02])).unwrap();
    let mut b = NoiseHandshake::responder(&bob, &hello()).unwrap();
    let msg2 = b.advance(&msg1).unwrap().reply.unwrap();
    let msg3 = a.advance(&msg2).unwrap().reply.unwrap();
    assert!(matches!(b.advance(&msg3), Err(CryptoError::Noise)));
}

#[test]
fn noise_xx_handshake_refuses_disjoint_suite_from_responder() {
    let alice = id(1);
    let bob = id(2);
    let (msg1, mut a) = NoiseHandshake::initiator(&alice, &hello()).unwrap();
    let mut b = NoiseHandshake::responder(&bob, &hello_with_suites(vec![0x02])).unwrap();
    let msg2 = b.advance(&msg1).unwrap().reply.unwrap();
    assert!(matches!(a.advance(&msg2), Err(CryptoError::Noise)));
}

#[test]
fn noise_xx_handshake_accepts_overlapping_multi_suite_list() {
    // A future build may advertise several suites; overlap on just one
    // (0x01, the only real suite today) must still succeed.
    let alice = id(1);
    let bob = id(2);
    let (msg1, mut a) = NoiseHandshake::initiator(&alice, &hello_with_suites(vec![0x02, 0x01])).unwrap();
    let mut b = NoiseHandshake::responder(&bob, &hello()).unwrap();
    let msg2 = b.advance(&msg1).unwrap().reply.unwrap();
    let out_a = a.advance(&msg2).unwrap();
    assert!(out_a.channel.is_some());
}

#[test]
fn id_proof_from_wrong_identity_rejected() {
    let alice = id(1);
    let bob = id(2);
    let mallory = id(3);
    let (msg1, mut a) = NoiseHandshake::initiator(&alice, &hello()).unwrap();
    let mut b = NoiseHandshake::responder(&bob, &hello()).unwrap();
    let msg2 = b.advance(&msg1).unwrap().reply.unwrap();
    let out_a = a.advance(&msg2).unwrap();
    let chan_a = out_a.channel.unwrap();
    let chan_b = b.advance(&out_a.reply.unwrap()).unwrap().channel.unwrap();

    // Mallory signs with the right preimage but is not the claimed identity.
    let forged = chan_a.make_id_proof(&mallory, true);
    assert!(chan_b.verify_id_proof(&bob.public(), &forged).is_err());
}

#[test]
fn sealed_box_roundtrip() {
    let bob = id(2);
    let mut r = rng();
    let (eph, ct) = seal_box(&mut r, &bob.exchange_public(), b"ad", b"secret");
    assert_eq!(open_box(&bob, &eph, b"ad", &ct).unwrap(), b"secret");
    assert!(open_box(&bob, &eph, b"other-ad", &ct).is_err());
}

#[test]
fn sealed_box_root_matches_recipient_derivation() {
    let alice = id(1);
    let bob = id(2);
    let mut r = rng();
    let ad = b"sealed-header-ad";
    let body = b"first-contact";
    let (eph, ct, sender_root) =
        seal_box_with_root(&mut r, &bob.exchange_public(), &alice.public(), &bob.public(), ad, body);

    assert_eq!(open_box(&bob, &eph, ad, &ct).unwrap(), body);
    let dh = bob.exchange_dh(&eph);
    let recipient_root = sealed_ck0(&dh, &alice.public(), &bob.public());
    assert_eq!(*sender_root, *recipient_root);
}

#[test]
fn prekey_agreement_matches_both_sides() {
    let alice = id(1);
    let bob = id(2);
    let mut r = rng();
    let store = PrekeyStore::generate(&mut r, 4);
    let batch = store.batch(&bob);

    // With a one-time prekey.
    let sa = sender_agree(&mut r, &alice, &bob.public(), &batch, Some(2)).unwrap();
    let ck_b = recipient_agree(&store, &bob, &alice.public(), &alice.exchange_public(), &sa.eph_pub, sa.prekey_id)
        .unwrap();
    assert_eq!(*sa.ck0, *ck_b);

    // Signed-prekey-only fallback.
    let sa2 = sender_agree(&mut r, &alice, &bob.public(), &batch, None).unwrap();
    assert_eq!(sa2.prekey_id, PREKEY_NONE);
    let ck_b2 = recipient_agree(&store, &bob, &alice.public(), &alice.exchange_public(), &sa2.eph_pub, PREKEY_NONE)
        .unwrap();
    assert_eq!(*sa2.ck0, *ck_b2);
    assert_ne!(*sa.ck0, *sa2.ck0);
}

#[test]
fn prekey_and_sealed_open_reject_low_order_points() {
    let alice = id(1);
    let bob = id(2);
    let mut r = rng();
    let store = PrekeyStore::generate(&mut r, 2);
    let low = Key32([0; 32]);

    let mut low_batch = store.batch(&bob);
    low_batch.signed_prekey = low;
    low_batch.spk_sig = bob.sign(SigDomain::Prekey, &low_batch.signed_object());
    assert!(sender_agree(&mut r, &alice, &bob.public(), &low_batch, None).is_err());
    assert!(recipient_agree(&store, &bob, &alice.public(), &alice.exchange_public(), &low, PREKEY_NONE).is_err());
    assert!(recipient_agree(&store, &bob, &alice.public(), &low, &low, PREKEY_NONE).is_err());
    assert!(open_box(&bob, &low, b"ad", &[0; 16]).is_err());
}

#[test]
fn prekey_batch_forged_signature_rejected() {
    let alice = id(1);
    let bob = id(2);
    let mallory = id(3);
    let mut r = rng();
    let store = PrekeyStore::generate(&mut r, 2);
    let batch = store.batch(&mallory); // signed by the wrong identity
    assert!(sender_agree(&mut r, &alice, &bob.public(), &batch, None).is_err());
}

#[test]
fn ratchet_out_of_order_within_window() {
    let ck0 = [7u8; 32];
    let alice_pk = Key32([1; 32]);
    let seed = direction_chain(&ck0, &alice_pk);
    let mut send = SendChain::new(*seed, 0);
    let mut recv = RecvChain::new(*seed, 0, 16);

    let keys: Vec<_> = (0..6).map(|_| send.next()).collect();
    // Deliver 3, 0, 5, 1 out of order.
    for &i in &[3usize, 0, 5, 1] {
        let (ref k, e, idx) = keys[i];
        assert_eq!(*recv.key_for(e, idx).unwrap(), **k);
    }
    // Replay is rejected (key consumed).
    assert!(recv.key_for(0, 3).is_err());
    // Beyond window fails.
    assert!(recv.key_for(0, 1000).is_err());
}

#[test]
fn ratchet_epoch_bump_stays_in_sync() {
    let seed = [9u8; 32];
    let mut send = SendChain::new(seed, 0);
    let mut recv = RecvChain::new(seed, 0, 8);
    let material = [3u8; 32];
    send.bump_epoch(&material);
    recv.bump_epoch(&material);
    let (k, e, i) = send.next();
    assert_eq!(e, 1);
    assert_eq!(*recv.key_for(e, i).unwrap(), *k);
}

#[test]
fn send_chain_snapshot_rejects_counter_wrap_boundary() {
    let mut chain = SendChain::new([1; 32], 0);
    chain.next_index = u32::MAX;
    assert!(chain.snapshot().is_err());
    chain.next_index = 0;
    chain.epoch = u16::MAX;
    assert!(chain.snapshot().is_err());
}

#[test]
fn send_chain_snapshot_roundtrip_preserves_future_keys() {
    let mut original = SendChain::new([7; 32], 3);
    for _ in 0..5 {
        original.next();
    }

    let snapshot = original.snapshot().unwrap();
    let mut restored = SendChain::restore(&snapshot).expect("valid send snapshot");
    assert_eq!(restored.snapshot().unwrap().as_slice(), snapshot.as_slice());

    for _ in 0..4 {
        assert_eq!(original.next(), restored.next());
    }
}

#[test]
fn recv_chain_snapshot_roundtrip_preserves_skipped_keys() {
    let seed = [8; 32];
    let mut sender = SendChain::new(seed, 2);
    let keys: Vec<_> = (0..7).map(|_| sender.next()).collect();
    let mut original = RecvChain::new(seed, 2, 16);
    let (expected, epoch, index) = &keys[6];
    assert_eq!(*original.key_for(*epoch, *index).unwrap(), **expected);

    let snapshot = original.snapshot().unwrap();
    let mut restored = RecvChain::restore(&snapshot).expect("valid receive snapshot");
    assert_eq!(restored.snapshot().unwrap().as_slice(), snapshot.as_slice());

    for &i in &[2usize, 0, 5] {
        let (expected, epoch, index) = &keys[i];
        assert_eq!(*original.key_for(*epoch, *index).unwrap(), **expected);
        assert_eq!(*restored.key_for(*epoch, *index).unwrap(), **expected);
    }
}

#[test]
fn recv_chain_snapshot_rejects_trailing_duplicate_and_invalid_records() {
    let seed = [9; 32];
    let mut sender = SendChain::new(seed, 4);
    let keys: Vec<_> = (0..3).map(|_| sender.next()).collect();
    let mut receiver = RecvChain::new(seed, 4, 16);
    let (_, epoch, index) = &keys[1];
    receiver.key_for(*epoch, *index).unwrap();
    let snapshot = receiver.snapshot().unwrap();

    let mut trailing = snapshot.to_vec();
    trailing.push(0);
    assert!(RecvChain::restore(&trailing).is_err());

    // Header: version(1), CK(32), epoch(2), next_index(4), window(4),
    // skipped_count(2); each skipped record is epoch(2), index(4), key(32).
    const COUNT: usize = 43;
    const HEADER: usize = 45;
    const RECORD: usize = 38;
    let mut duplicate = snapshot.to_vec();
    duplicate[COUNT..COUNT + 2].copy_from_slice(&2u16.to_le_bytes());
    duplicate.extend_from_within(HEADER..HEADER + RECORD);
    assert!(RecvChain::restore(&duplicate).is_err());

    let mut ordered_receiver = RecvChain::new(seed, 4, 16);
    let (_, epoch, index) = &keys[2];
    ordered_receiver.key_for(*epoch, *index).unwrap();
    let ordered = ordered_receiver.snapshot().unwrap();
    let mut unsorted = ordered.to_vec();
    let first = unsorted[HEADER..HEADER + RECORD].to_vec();
    let second = unsorted[HEADER + RECORD..HEADER + RECORD * 2].to_vec();
    unsorted[HEADER..HEADER + RECORD].copy_from_slice(&second);
    unsorted[HEADER + RECORD..HEADER + RECORD * 2].copy_from_slice(&first);
    assert!(RecvChain::restore(&unsorted).is_err());

    let mut future_epoch = snapshot.to_vec();
    future_epoch[HEADER..HEADER + 2].copy_from_slice(&5u16.to_le_bytes());
    assert!(RecvChain::restore(&future_epoch).is_err());

    let mut current_future_index = snapshot.to_vec();
    current_future_index[HEADER + 2..HEADER + 6].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(RecvChain::restore(&current_future_index).is_err());

    let mut over_cap = snapshot.to_vec();
    over_cap[COUNT..COUNT + 2].copy_from_slice(&u16::MAX.to_le_bytes());
    assert!(RecvChain::restore(&over_cap).is_err());

    let huge_window = RecvChain::new(seed, 4, mmp_types::consts::RATCHET_WINDOW + 1);
    assert!(huge_window.snapshot().is_err());
}

#[test]
fn prekey_snapshot_roundtrip_preserves_private_batch() {
    let alice = id(10);
    let mut r = rng();
    let original = PrekeyStore::generate(&mut r, 4);
    let snapshot = original.snapshot().unwrap();
    let restored = PrekeyStore::restore(&snapshot).expect("valid prekey snapshot");
    assert_eq!(restored.snapshot().unwrap().as_slice(), snapshot.as_slice());
    assert_eq!(original.batch(&alice), restored.batch(&alice));

    let mut sender_rng = rng();
    let agreement = sender_agree(
        &mut sender_rng,
        &alice,
        &id(11).public(),
        &original.batch(&id(11)),
        Some(2),
    )
    .unwrap();
    let restored_ck = recipient_agree(
        &restored,
        &id(11),
        &alice.public(),
        &alice.exchange_public(),
        &agreement.eph_pub,
        agreement.prekey_id,
    )
    .unwrap();
    let original_ck = recipient_agree(
        &original,
        &id(11),
        &alice.public(),
        &alice.exchange_public(),
        &agreement.eph_pub,
        agreement.prekey_id,
    )
    .unwrap();
    assert_eq!(*restored_ck, *original_ck);
}

#[test]
fn prekey_snapshot_rejects_trailing_duplicate_and_oversize() {
    let mut r = rng();
    let original = PrekeyStore::generate(&mut r, 1);
    let snapshot = original.snapshot().unwrap();
    let mut trailing = snapshot.to_vec();
    trailing.push(0);
    assert!(PrekeyStore::restore(&trailing).is_err());

    // Header: version(1), SPK secret(32), count(2); each record is id(2) +
    // secret(32).
    const COUNT: usize = 33;
    const HEADER: usize = 35;
    const RECORD: usize = 34;
    let mut duplicate = snapshot.to_vec();
    duplicate[COUNT..COUNT + 2].copy_from_slice(&2u16.to_le_bytes());
    duplicate.extend_from_within(HEADER..HEADER + RECORD);
    assert!(PrekeyStore::restore(&duplicate).is_err());

    let mut gap = snapshot.to_vec();
    gap[HEADER..HEADER + 2].copy_from_slice(&1u16.to_le_bytes());
    assert!(PrekeyStore::restore(&gap).is_err());

    let mut over_cap = snapshot.to_vec();
    over_cap[COUNT..COUNT + 2].copy_from_slice(&u16::MAX.to_le_bytes());
    assert!(PrekeyStore::restore(&over_cap).is_err());

    let oversized = PrekeyStore::generate(&mut r, mmp_types::consts::PREKEY_BATCH + 1);
    assert!(oversized.snapshot().is_err());
}

#[test]
fn sender_keys_group_roundtrip() {
    let gid = Id16([4; 16]);
    let mut r = rng();
    let mut local = LocalSenderKey::generate(&mut r, gid, 1);
    let dist = local.dist().clone();
    let mut remote = RemoteSenderKey::from_dist(&dist, 32);

    let member = Id16([1; 16]);
    let p = local.encrypt(member, b"hdr", b"hello group");
    assert_eq!(remote.decrypt(&p, b"hdr").unwrap(), b"hello group");

    // Tampered signature rejected.
    let mut p2 = local.encrypt(member, b"hdr", b"x");
    p2.member_sig.0[0] ^= 1;
    assert!(remote.decrypt(&p2, b"hdr").is_err());

    // Dist survives the wire.
    assert_eq!(SenderKeyDist::decode(&dist.encode()).unwrap(), dist);
}

#[test]
fn receipt_mac_roundtrip() {
    let root = [5u8; 32];
    let key = receipt_key(&root);
    let delivered = Id16([8; 16]);
    let mac = receipt_mac(&key, &delivered);
    assert!(mac_eq(&mac, &receipt_mac(&key, &delivered)));
    assert!(!mac_eq(&mac, &receipt_mac(&key, &Id16([9; 16]))));
}

#[test]
fn merkle_path_verifies_and_corruption_fails() {
    let blocks: Vec<Vec<u8>> = (0..13u8).map(|i| vec![i; 100]).collect();
    let leaves: Vec<[u8; 32]> = blocks.iter().map(|b| merkle::hash_block(b)).collect();
    let root = merkle::merkle_root(&leaves);
    for (i, leaf) in leaves.iter().enumerate() {
        let path = merkle::merkle_path(&leaves, i);
        merkle::verify_path(&root, leaf, &path).unwrap();
    }
    // A corrupted symbol must fail against the same path.
    let path = merkle::merkle_path(&leaves, 5);
    let bad_leaf = merkle::hash_block(b"corrupted");
    assert!(merkle::verify_path(&root, &bad_leaf, &path).is_err());
}

/// E5-S1 decision D1: `MerklePath::encode`/`decode` round-trips, and
/// `leaf_index` reconstructs the exact index `merkle_path` was built for.
#[test]
fn merkle_path_wire_roundtrip_and_leaf_index() {
    let blocks: Vec<Vec<u8>> = (0..20u8).map(|i| vec![i; 16]).collect();
    let leaves: Vec<[u8; 32]> = blocks.iter().map(|b| merkle::hash_block(b)).collect();
    for i in [0usize, 1, 5, 13, 19] {
        let path = merkle::merkle_path(&leaves, i);
        let wire = path.encode();
        let decoded = merkle::MerklePath::decode(&wire).unwrap();
        assert_eq!(decoded, path);
        assert_eq!(decoded.leaf_index(), i as u64);
    }
}

/// Strict decode: trailing bytes, a depth over the cap, and an
/// out-of-range `pos`/`nsib` are all rejected (crate-wide canonical-decode
/// discipline).
#[test]
fn merkle_path_decode_rejects_malformed_input() {
    let blocks: Vec<Vec<u8>> = (0..5u8).map(|i| vec![i; 8]).collect();
    let leaves: Vec<[u8; 32]> = blocks.iter().map(|b| merkle::hash_block(b)).collect();
    let path = merkle::merkle_path(&leaves, 2);
    let wire = path.encode();

    let mut trailing = wire.clone();
    trailing.push(0);
    assert!(merkle::MerklePath::decode(&trailing).is_err());

    let mut over_depth = wire.clone();
    over_depth[0] = (merkle::MERKLE_PATH_MAX_LEVELS + 1) as u8;
    assert!(merkle::MerklePath::decode(&over_depth).is_err());

    // Truncated mid-level.
    assert!(merkle::MerklePath::decode(&wire[..wire.len() - 1]).is_err());
}

/// E5-S1 decision D1 check (b) — position binding: a syntactically valid
/// path that genuinely proves leaf `i` must be rejected when checked against
/// a DIFFERENT claimed `sym_no`, even though `compute_root` alone (a
/// membership-only check) would happily accept it. This is exactly the
/// forged-position replay hole the `leaf_index` check closes.
#[test]
fn forged_position_path_is_rejected_by_leaf_index_check() {
    let blocks: Vec<Vec<u8>> = (0..20u8).map(|i| vec![i; 16]).collect();
    let leaves: Vec<[u8; 32]> = blocks.iter().map(|b| merkle::hash_block(b)).collect();
    let root = merkle::merkle_root(&leaves);
    let true_index = 5usize;
    let claimed_sym_no = 9u64; // attacker relabels this symbol as index 9
    let path = merkle::merkle_path(&leaves, true_index);

    // Membership alone still verifies (the path IS valid for the leaf it
    // actually proves) — this is exactly why membership alone isn't enough.
    assert!(merkle::verify_path(&root, &leaves[true_index], &path).is_ok());
    // But the path does not prove the claimed position: this is the check
    // `mmp-node`'s `symbol_payload_verifies` (D1 check b) relies on.
    assert_ne!(path.leaf_index(), claimed_sym_no);
    assert_eq!(path.leaf_index(), true_index as u64);
}

#[test]
fn conversation_id_is_commutative() {
    let a = Key32([1; 32]);
    let b = Key32([2; 32]);
    assert_eq!(conversation_id(&a, &b), conversation_id(&b, &a));
    assert_ne!(conversation_id(&a, &b), conversation_id(&a, &Key32([3; 32])));
}
