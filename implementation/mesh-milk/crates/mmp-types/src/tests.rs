//! Codec exit tests (HANDOFF §2.1): round-trips, canonical-form negatives,
//! lifetime-reset ID stability, byte-mutation property.

extern crate std;

use alloc::vec;
use alloc::vec::Vec;

use crate::beacon::Beacon;
use crate::bundle::*;
use crate::claim::{Claim, PrekeyBatch};
use crate::frame::*;
use crate::ids::{Id16, Key32, Sig64};
use crate::manifest::{FileEntry, FileManifest};
use crate::payload::*;
use crate::tlv::Ext;
use crate::WireError;

fn sample_bundle() -> Bundle {
    Bundle {
        class: class::DIRECT,
        flags: FLAG_HAS_PARENTS | FLAG_WANTS_RECEIPT,
        origin: Key32([7; 32]),
        dest: Id16([9; 16]),
        lifetime_ms: 259_200_000,
        priority: 1,
        exts: vec![Ext::Parents(vec![Id16([1; 16]), Id16([2; 16])]), Ext::GroupEpoch(3)],
        payload: vec![0xAA; 100],
        signature: Sig64([0x55; 64]),
    }
}

#[test]
fn bundle_roundtrip() {
    let b = sample_bundle();
    let bytes = b.encode();
    let d = Bundle::decode_canonical(&bytes).unwrap();
    assert_eq!(b, d);
    assert_eq!(b.id(), d.id());
}

#[test]
fn lifetime_reset_yields_identical_id_and_signing_message() {
    // AUDIT F1 golden negative: a relay that resets only lifetime_ms produces
    // an IDENTICAL BundleID and signature preimage.
    let mut a = sample_bundle();
    let id_a = a.id();
    let msg_a = a.signing_message();
    a.lifetime_ms = crate::consts::LIFETIME_MESSAGE_MS; // "reset to class max"
    assert_eq!(a.id(), id_a);
    assert_eq!(a.signing_message(), msg_a);
    // But the encoded wire bytes DO differ (live lifetime is carried).
}

#[test]
fn trailing_byte_rejected() {
    let mut bytes = sample_bundle().encode();
    bytes.push(0);
    assert_eq!(Bundle::decode_canonical(&bytes), Err(WireError::Trailing));
}

#[test]
fn tlv_out_of_order_rejected() {
    // GroupEpoch (0x02) before Parents (0x01): encode manually.
    let mut b = sample_bundle();
    b.exts = vec![Ext::GroupEpoch(3), Ext::Parents(vec![Id16([1; 16])])];
    // encode_exts debug-asserts order; build the wire form by hand instead.
    let good = Bundle {
        exts: vec![Ext::Parents(vec![Id16([1; 16])]), Ext::GroupEpoch(3)],
        ..sample_bundle()
    };
    let mut bytes = good.encode();
    // Swap the two TLVs in place: Parents(1) = 1+2+16 = 19 bytes, GroupEpoch = 1+2+2 = 5.
    let ext_start = 58;
    let mut swapped = Vec::new();
    swapped.extend_from_slice(&bytes[ext_start + 19..ext_start + 24]); // epoch tlv
    swapped.extend_from_slice(&bytes[ext_start..ext_start + 19]); // parents tlv
    bytes[ext_start..ext_start + 24].copy_from_slice(&swapped);
    assert_eq!(Bundle::decode_canonical(&bytes), Err(WireError::TlvOrder));
}

#[test]
fn unknown_critical_tlv_rejected() {
    let b = Bundle { exts: vec![], flags: 0, ..sample_bundle() };
    let mut bytes = b.encode();
    // Splice a TLV 0x05 (unknown critical) of length 0 into the ext region.
    let ins = [0x05u8, 0, 0];
    bytes.splice(58..58, ins);
    let new_ext_len = 3u16.to_le_bytes();
    bytes[56..58].copy_from_slice(&new_ext_len);
    assert_eq!(Bundle::decode_canonical(&bytes), Err(WireError::TlvUnknownCritical));
}

#[test]
fn reserved_flag_bits_rejected() {
    let b = Bundle { flags: 0, exts: vec![], ..sample_bundle() };
    let mut bytes = b.encode();
    bytes[1] = 0x80;
    assert_eq!(Bundle::decode_canonical(&bytes), Err(WireError::ReservedBits));
}

#[test]
fn parents_flag_must_mirror_tlv() {
    let b = Bundle { flags: 0, ..sample_bundle() }; // TLV present, flag absent
    let bytes = b.encode();
    assert_eq!(Bundle::decode_canonical(&bytes), Err(WireError::BadValue("has_parents flag")));
}

#[test]
fn frames_roundtrip() {
    let frames = vec![
        Frame::Hs(vec![1, 2, 3]),
        Frame::IdProof { identity: Key32([1; 32]), sig: Sig64([2; 64]) },
        Frame::Credit(8),
        Frame::Summary(Summary {
            receipt_buckets: vec![SummaryBucket { bucket_epoch: 7, bloom: vec![1, 2] }],
            claim_digest: vec![ClaimDigestEntry { node_prefix: 5, field: 1, seq: 9 }],
            group_hints: vec![Id16([3; 16])],
            want_heads: vec![WantHeads { conv: Id16([4; 16]), heads: vec![Id16([5; 16])] }],
        }),
        Frame::ReconInit {
            domain: 0,
            window: 1,
            range: Range { lo: Id16::MIN, hi: Id16::MAX, count: 42, fp: 0xdead_beef },
        },
        Frame::ReconRange {
            domain: 3,
            ranges: vec![Range { lo: Id16([1; 16]), hi: Id16([2; 16]), count: 1, fp: 2 }],
        },
        Frame::ReconIds { domain: 1, lo: Id16::MIN, hi: Id16::MAX, ids: vec![Id16([6; 16])] },
        Frame::Want(vec![Id16([7; 16])]),
        Frame::Bundle(vec![0xAB; 40]),
        Frame::Budget { id: Id16([8; 16]), granted: 16 },
        Frame::BlobReq { root: Id16([9; 16]), block: 2, bitmap: vec![0xFF] },
        Frame::BlobHave { root: Id16([9; 16]), block: 2, bitmap: vec![0x0F] },
        Frame::Error { code: errcode::UNSUPPORTED, detail: vec![] },
        Frame::Close,
    ];
    for f in frames {
        let wire = f.encode();
        let (typ, payload, consumed) = read_envelope(&wire).unwrap().unwrap();
        assert_eq!(consumed, wire.len());
        assert_eq!(Frame::decode_payload(typ, payload).unwrap(), f);
    }
}

#[test]
fn friend_frame_bodies_roundtrip() {
    let offer = FriendOfferBody { since: 1_700_000_000 };
    assert_eq!(FriendOfferBody::decode(&offer.encode()).unwrap(), offer);

    let ack = FriendAckBody { countersig: Sig64([9; 64]) };
    assert_eq!(FriendAckBody::decode(&ack.encode()).unwrap(), ack);

    let summary = ProxySummaryBody { dependent: Id16([4; 16]), bloom: vec![1, 2, 3] };
    assert_eq!(ProxySummaryBody::decode(&summary.encode()).unwrap(), summary);
}

#[test]
fn friend_frame_bodies_reject_trailing_bytes() {
    let mut bytes = FriendOfferBody { since: 1 }.encode();
    bytes.push(0);
    assert_eq!(FriendOfferBody::decode(&bytes), Err(WireError::Trailing));

    let mut bytes = FriendAckBody { countersig: Sig64::ZERO }.encode();
    bytes.push(0);
    assert_eq!(FriendAckBody::decode(&bytes), Err(WireError::Trailing));

    let mut bytes = ProxySummaryBody { dependent: Id16([1; 16]), bloom: vec![] }.encode();
    bytes.push(0);
    assert_eq!(ProxySummaryBody::decode(&bytes), Err(WireError::Trailing));
}

#[test]
fn envelope_oversize_prefix_rejected() {
    // AUDIT F10: a length prefix over the cap must be an error, not a grow.
    let mut wire = Vec::new();
    wire.extend_from_slice(&5000u16.to_le_bytes());
    wire.push(0x20);
    assert_eq!(read_envelope(&wire), Err(WireError::Oversize));
}

#[test]
fn beacon_roundtrip() {
    let b = Beacon {
        ver: 1,
        tier: 2,
        battery: 3,
        ephem_id: [7; 8],
        busy: true,
        epoch: 0x55,
        frag_structure: 2,
        frag_offset: 0x1ABC,
        fragment: [9; 12],
    };
    let wire = b.encode();
    assert_eq!(Beacon::decode(&wire).unwrap(), b);
    assert_eq!(Beacon::decode(&wire[..23]), Err(WireError::Truncated));
}

#[test]
fn claim_and_prekey_roundtrip() {
    let batch = PrekeyBatch {
        signed_prekey: Key32([1; 32]),
        spk_sig: Sig64([2; 64]),
        prekeys: vec![Key32([3; 32]), Key32([4; 32])],
    };
    let body = batch.encode();
    assert_eq!(PrekeyBatch::decode(&body).unwrap(), batch);
    let c = Claim {
        owner: Key32([5; 32]),
        field: crate::claim::field::PREKEY_BATCH,
        seq: 12,
        body,
        sig: Sig64([6; 64]),
    };
    let wire = c.encode();
    assert_eq!(Claim::decode(&wire).unwrap(), c);
}

#[test]
fn payload_schemas_roundtrip() {
    let d = DirectPayload { conv: Id16([1; 16]), epoch: 2, index: 3, ct: vec![4; 20] };
    assert_eq!(DirectPayload::decode(&d.encode()).unwrap(), d);
    let a = AsyncPayload::Prekey {
        prekey_id: 5,
        sender_xpk: Key32([8; 32]),
        eph: Key32([6; 32]),
        ct: vec![7; 10],
    };
    assert_eq!(AsyncPayload::decode(&a.encode()).unwrap(), a);
    let g = GroupPayload {
        gid: Id16([1; 16]),
        epoch: 4,
        member: Id16([2; 16]),
        chain_index: 9,
        ct: vec![8; 30],
        member_sig: Sig64([9; 64]),
    };
    assert_eq!(GroupPayload::decode(&g.encode()).unwrap(), g);
    let rp = ReceiptPayload { delivered: Id16([3; 16]), mac: [0xCD; 16] };
    assert_eq!(ReceiptPayload::decode(&rp.encode()).unwrap(), rp);
}

/// E5-S1 decision D1: `u16 path_len ‖ path ‖ symbol` — decode requires an
/// EXACT match (`2 + path_len + SYMBOL_SIZE`), not merely "at least".
#[test]
fn symbol_payload_roundtrip_and_strict_length() {
    let sp = SymbolPayload { path: vec![0xAB; 40], symbol: vec![0x11; crate::consts::SYMBOL_SIZE] };
    let wire = sp.encode();
    assert_eq!(wire.len(), 2 + 40 + crate::consts::SYMBOL_SIZE);
    assert_eq!(SymbolPayload::decode(&wire).unwrap(), sp);

    // Trailing byte beyond the exact symbol length is rejected (crate-wide
    // strict-decode discipline — `Reader::finish`).
    let mut trailing = wire.clone();
    trailing.push(0);
    assert_eq!(SymbolPayload::decode(&trailing), Err(WireError::Trailing));

    // Truncated symbol (short by one byte) is rejected, not zero-padded.
    let short = &wire[..wire.len() - 1];
    assert_eq!(SymbolPayload::decode(short), Err(WireError::Truncated));

    // Empty input.
    assert_eq!(SymbolPayload::decode(&[]), Err(WireError::Truncated));
}

#[test]
fn manifest_roundtrip_and_caps() {
    let m = FileManifest {
        suite: 1,
        sharer: Key32([1; 32]),
        entries: vec![FileEntry {
            name: b"photo.jpg".to_vec(),
            mime: b"image/jpeg".to_vec(),
            size: 123_456,
            file_root: Id16([2; 16]),
            rs_k: 32,
            rs_n: 48,
            full_hash: [3; 32],
            thumb_root: Some(Id16([4; 16])),
        }],
        sig: Sig64([5; 64]),
    };
    let wire = m.encode();
    assert_eq!(FileManifest::decode(&wire).unwrap(), m);
    // count = 0 rejected
    let mut bad = wire.clone();
    bad[33] = 0;
    assert!(FileManifest::decode(&bad).is_err());
}

mod props {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// Mutate any byte of an encoded bundle ⇒ decode rejects, or the
        /// decoded bundle differs, or only the relay-mutable lifetime moved
        /// (in which case the BundleID must be unchanged).
        #[test]
        fn mutate_any_byte(idx in 0usize..300, val in 0u8..=255) {
            let b = sample_bundle();
            let mut bytes = b.encode();
            let idx = idx % bytes.len();
            if bytes[idx] == val { return Ok(()); }
            bytes[idx] = val;
            match Bundle::decode_canonical(&bytes) {
                Err(_) => {}
                Ok(d) => {
                    let lifetime_only = (50..55).contains(&idx);
                    if lifetime_only {
                        prop_assert_eq!(d.id(), b.id());
                    } else {
                        prop_assert_ne!(d, b);
                    }
                }
            }
        }

        #[test]
        fn frame_decoder_never_panics(typ in 0u8..=255, data in proptest::collection::vec(any::<u8>(), 0..200)) {
            let _ = Frame::decode_payload(typ, &data);
        }

        #[test]
        fn bundle_decoder_never_panics(data in proptest::collection::vec(any::<u8>(), 0..300)) {
            let _ = Bundle::decode_canonical(&data);
        }
    }
}
