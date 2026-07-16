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

#[test]
fn conversation_id_is_commutative() {
    let a = Key32([1; 32]);
    let b = Key32([2; 32]);
    assert_eq!(conversation_id(&a, &b), conversation_id(&b, &a));
    assert_ne!(conversation_id(&a, &b), conversation_id(&a, &Key32([3; 32])));
}
