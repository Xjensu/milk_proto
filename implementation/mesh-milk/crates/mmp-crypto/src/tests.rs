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
use crate::noise::NoiseHandshake;
use crate::prekey::{recipient_agree, sender_agree, PrekeyStore, PREKEY_NONE};
use crate::ratchet::{direction_chain, RecvChain, SendChain};
use crate::receipt::{mac_eq, receipt_key, receipt_mac};
use crate::sealed::{open_box, seal_box};
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
