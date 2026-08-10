//! Claims: single-writer LWW registers (doc 06 §4).
//!
//! Higher `seq` wins; equal `seq` + different body resolves deterministically
//! (smaller BLAKE3(body)) AND raises a compromise signal (doc 07 §6.4).
//! Signature verification happens at ingest — the store only merges verified
//! claims.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use mmp_types::claim::{
    field, friendship_signed_object, succession_signed_object, Claim, EpochAnchor, Friendship, PrekeyBatch,
    Revocation,
};
use mmp_types::consts::LIFETIME_CLAIM_MS;
use mmp_types::domain::SigDomain;
use mmp_types::frame::ClaimDigestEntry;
use mmp_types::ids::{node_id_of, Key32, NodeId};

/// Outcome of merging one incoming claim into a [`ClaimStore`] (doc 06 §4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimMerge {
    /// New or newer — adopted.
    Adopted,
    /// Lower or equal seq, identical body — nothing to do.
    Stale,
    /// Equal seq, different body: deterministic winner chosen; surface a
    /// CompromiseWarning for `owner` (doc 06 §4.2).
    Conflict {
        /// Whether the incoming claim (rather than the one already stored) won the tie-break.
        adopted: bool,
    },
    /// A structurally invalid recognized claim body (for example a malformed
    /// `REVOCATION` or an over-cap/non-UTF-8 nickname). Never stored;
    /// distinct from `Stale` because nothing was already on file to keep.
    Rejected,
}

struct Stored {
    claim: Claim,
    received_at: u64,
    /// ContactGraph peers' claims never expire (doc 06 §4.2).
    pinned: bool,
}

/// Per-(owner, field) single-writer LWW claim registry (doc 06 §4).
#[derive(Default)]
pub struct ClaimStore {
    map: BTreeMap<(NodeId, u8), Stored>,
}

impl ClaimStore {
    /// An empty claim store.
    pub fn new() -> ClaimStore {
        ClaimStore::default()
    }

    /// Validate a claim body using the schemas implemented by this Rust
    /// release, without mutating a store.  Authority pointer fields remain
    /// opaque by design (doc 06 §4.1 / doc 18); their owner signature is the
    /// only v1 relay-side requirement.  The Friendship body follows the
    /// current Rust/handbook schema; the document-10 alternate schema remains
    /// a tracked conformance discrepancy and is not silently normalized here.
    pub fn validate_claim(claim: &Claim) -> bool {
        if claim.validate_known_body().is_err() {
            return false;
        }
        match claim.field {
            field::EXCHANGE_KEY => claim.body.len() == 32,
            field::PREKEY_BATCH => {
                let Ok(batch) = PrekeyBatch::decode(&claim.body) else { return false };
                mmp_crypto::verify(SigDomain::Prekey, &claim.owner, &batch.signed_object(), &batch.spk_sig).is_ok()
            }
            field::FRIENDSHIP => {
                let Ok(body) = Friendship::decode(&claim.body) else { return false };
                let preimage = friendship_signed_object(&claim.owner, &body.friend, body.since);
                mmp_crypto::verify(SigDomain::Friend, &body.friend, &preimage, &body.countersig).is_ok()
            }
            field::GROUP_EPOCH_ANCHOR => EpochAnchor::decode(&claim.body).is_ok(),
            field::REVOCATION => {
                let Ok(body) = Revocation::decode(&claim.body) else { return false };
                body.revoked_key == claim.owner
            }
            // AVATAR_ROOT has no complete body codec in the frozen suite;
            // AUTHCERT and REVOCATION_LIST are explicit store-don't-interpret
            // pointers.  Do not guess a new schema at this boundary.
            field::AVATAR_ROOT | field::AUTHCERT | field::REVOCATION_LIST => true,
            _ => true,
        }
    }

    /// Merge a signature-verified claim.
    ///
    /// Tombstone rule (doc 06 §4.6 / handbook §4.6): once a `REVOCATION`
    /// claim is stored for an owner, it is immutable, and it beats ANY other
    /// claim by that same owner from then on — regardless of seq, including a
    /// second revocation attempt. This short-circuits before the ordinary
    /// per-field LWW logic below, which is otherwise unchanged.
    pub fn merge(&mut self, claim: Claim, now_ms: u64, pinned: bool) -> ClaimMerge {
        let node = node_id_of(&claim.owner);

        if self.map.contains_key(&(node, field::REVOCATION)) {
            return ClaimMerge::Stale;
        }

        // `Claim::decode` applies the same check to wire input.  Repeat the
        // complete implemented-schema validation at the state boundary
        // because callers may construct a Claim directly (out-of-band
        // bootstrap, tests, or persistence adapters). Unknown and authority
        // pointer fields remain opaque and are intentionally accepted here.
        if !Self::validate_claim(&claim) {
            return ClaimMerge::Rejected;
        }

        let key = (node, claim.field);
        match self.map.get_mut(&key) {
            None => {
                self.map.insert(key, Stored { claim, received_at: now_ms, pinned });
                ClaimMerge::Adopted
            }
            Some(cur) => {
                cur.pinned |= pinned;
                if claim.seq > cur.claim.seq {
                    cur.claim = claim;
                    cur.received_at = now_ms;
                    ClaimMerge::Adopted
                } else if claim.seq < cur.claim.seq || claim.body == cur.claim.body {
                    ClaimMerge::Stale
                } else {
                    // Equal seq, different body: smaller BLAKE3(body) wins.
                    let ours = blake3::hash(&cur.claim.body);
                    let theirs = blake3::hash(&claim.body);
                    let adopted = theirs.as_bytes() < ours.as_bytes();
                    if adopted {
                        cur.claim = claim;
                        cur.received_at = now_ms;
                    }
                    ClaimMerge::Conflict { adopted }
                }
            }
        }
    }

    /// The currently-stored claim for `(node, field)`, if any.
    pub fn get(&self, node: &NodeId, field: u8) -> Option<&Claim> {
        self.map.get(&(*node, field)).map(|s| &s.claim)
    }

    /// The revocation tombstone for `owner`, if any (doc 06 §4.6 / handbook
    /// §4.6). Decoded fresh from the stored claim body each call — cheap
    /// (fixed-size, no allocation) and keeps `Stored` from needing a second,
    /// redundant decoded copy.
    pub fn revocation(&self, owner: &NodeId) -> Option<Revocation> {
        self.map.get(&(*owner, field::REVOCATION)).and_then(|s| Revocation::decode(&s.claim.body).ok())
    }

    /// The trusted successor for `owner`'s revocation, if the revocation
    /// names one AND its counter-signature verifies — the successor signing
    /// `revoked_key ‖ successor` (`succession_signed_object`) under
    /// `mmp1/claim` with its OWN key (distinct from the outer claim's owner
    /// signature by the revoked identity). `None` covers "no revocation",
    /// "no successor named", and "countersig present but invalid" alike —
    /// callers that need to distinguish those should inspect
    /// [`ClaimStore::revocation`] directly. A failed countersig never
    /// invalidates the revocation itself (handbook §4.6: "succession
    /// countersig failure => succession ignored, revocation stands").
    pub fn successor(&self, owner: &NodeId) -> Option<Key32> {
        let rev = self.revocation(owner)?;
        let successor = rev.successor?;
        let sig = rev.successor_countersig?;
        let preimage = succession_signed_object(&rev.revoked_key, &successor);
        mmp_crypto::verify(SigDomain::Claim, &successor, &preimage, &sig).ok()?;
        Some(successor)
    }

    /// Marks every stored claim for `node` as pinned (a verified contact),
    /// exempting them from [`ClaimStore::expire`].
    pub fn pin(&mut self, node: &NodeId) {
        for ((n, _), s) in self.map.iter_mut() {
            if n == node {
                s.pinned = true;
            }
        }
    }

    /// Claim digest for SUMMARY, scoped to the given nodes (doc 06 §4.3 —
    /// non-WoT sessions digest only the two session identities).
    pub fn digest_for(&self, nodes: &[NodeId]) -> Vec<ClaimDigestEntry> {
        let mut out = Vec::new();
        for node in nodes {
            for ((n, field), s) in self.map.range((*node, 0)..=(*node, 0xff)) {
                debug_assert_eq!(n, node);
                out.push(ClaimDigestEntry {
                    node_prefix: u32::from_le_bytes([node.0[0], node.0[1], node.0[2], node.0[3]]),
                    field: *field,
                    seq: s.claim.seq,
                });
            }
        }
        out
    }

    /// Strangers' claims expire at 7 d; pinned (contact) claims are retained.
    pub fn expire(&mut self, now_ms: u64) {
        self.map.retain(|_, s| s.pinned || s.received_at.saturating_add(LIFETIME_CLAIM_MS) > now_ms);
    }

    /// All currently-stored claims, in `(owner, field)` order.
    pub fn iter(&self) -> impl Iterator<Item = &Claim> {
        self.map.values().map(|s| &s.claim)
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use mmp_crypto::Identity;
    use mmp_types::domain::SigDomain;
    use mmp_types::ids::Sig64;

    use super::*;

    fn identity(seed: u8) -> Identity {
        Identity::from_seed(&[seed; 32])
    }

    fn signed_claim(owner: &Identity, field_id: u8, seq: u32, body: Vec<u8>) -> Claim {
        let mut c = Claim { owner: owner.public(), field: field_id, seq, body, sig: Sig64::ZERO };
        c.sig = owner.sign(SigDomain::Claim, &c.signed_object());
        c
    }

    fn revoke(owner: &Identity, seq: u32) -> Claim {
        let body = Revocation { revoked_key: owner.public(), reason: 1, successor: None, successor_countersig: None };
        signed_claim(owner, field::REVOCATION, seq, body.encode())
    }

    /// Handbook §4.6 acceptance case 1: a revocation beats a higher-seq claim
    /// for any other field, even one merged afterward.
    #[test]
    fn revocation_beats_a_higher_seq_nickname_claim() {
        let alice = identity(1);
        let mut store = ClaimStore::new();
        let nick = signed_claim(&alice, field::NICKNAME, 5, b"alice".to_vec());
        assert_eq!(store.merge(nick, 0, false), ClaimMerge::Adopted);

        assert_eq!(store.merge(revoke(&alice, 1), 0, false), ClaimMerge::Adopted);

        // A LATER, higher-seq nickname claim must be ignored — the tombstone wins.
        let later_nick = signed_claim(&alice, field::NICKNAME, 99, b"alicia".to_vec());
        assert_eq!(store.merge(later_nick, 0, false), ClaimMerge::Stale);

        let n = node_id_of(&alice.public());
        assert_eq!(store.get(&n, field::NICKNAME).unwrap().body, b"alice");
        assert!(store.revocation(&n).is_some());
    }

    /// Handbook §4.6 acceptance case 3 (succession half): a bad countersig
    /// means `successor()` yields `None`, but the revocation itself stands.
    #[test]
    fn succession_countersig_failure_leaves_revocation_but_no_successor() {
        let alice = identity(1);
        let bob = identity(2); // named as successor but never actually signs
        let mallory = identity(3); // signs instead, under the wrong key

        let mut store = ClaimStore::new();
        let bad_sig =
            mallory.sign(SigDomain::Claim, &succession_signed_object(&alice.public(), &bob.public()));
        let body = Revocation {
            revoked_key: alice.public(),
            reason: 0,
            successor: Some(bob.public()),
            successor_countersig: Some(bad_sig),
        };
        let rev = signed_claim(&alice, field::REVOCATION, 1, body.encode());
        assert_eq!(store.merge(rev, 0, false), ClaimMerge::Adopted);

        let n = node_id_of(&alice.public());
        assert!(store.revocation(&n).is_some(), "revocation stands");
        assert!(store.successor(&n).is_none(), "bad countersig => no trusted successor");
    }

    /// A correctly counter-signed succession DOES resolve — the mirror image
    /// of the failure case above, so `successor()`'s happy path is covered.
    #[test]
    fn valid_succession_countersig_resolves_the_successor() {
        let alice = identity(1);
        let bob = identity(2);

        let mut store = ClaimStore::new();
        let sig =
            bob.sign(SigDomain::Claim, &succession_signed_object(&alice.public(), &bob.public()));
        let body = Revocation {
            revoked_key: alice.public(),
            reason: 0,
            successor: Some(bob.public()),
            successor_countersig: Some(sig),
        };
        let rev = signed_claim(&alice, field::REVOCATION, 1, body.encode());
        assert_eq!(store.merge(rev, 0, false), ClaimMerge::Adopted);

        let n = node_id_of(&alice.public());
        assert_eq!(store.successor(&n), Some(bob.public()));
    }

    /// Handbook §4.6 acceptance case: session-relevant claims (exchange key,
    /// prekey batch — anything) arriving after a revocation are rejected
    /// outright, not merely out-scored.
    #[test]
    fn revoked_owners_later_claims_of_any_field_are_rejected() {
        let alice = identity(1);
        let mut store = ClaimStore::new();
        assert_eq!(store.merge(revoke(&alice, 1), 0, false), ClaimMerge::Adopted);

        let exchange = signed_claim(&alice, field::EXCHANGE_KEY, 1, vec![7u8; 32]);
        assert_eq!(store.merge(exchange, 0, false), ClaimMerge::Stale);

        let n = node_id_of(&alice.public());
        assert!(store.get(&n, field::EXCHANGE_KEY).is_none());
    }

    /// A revocation is immutable once stored — a second revocation attempt
    /// (even a differently-shaped one) for the same owner never displaces it.
    #[test]
    fn a_second_revocation_for_the_same_owner_is_ignored() {
        let alice = identity(1);
        let mut store = ClaimStore::new();
        assert_eq!(store.merge(revoke(&alice, 1), 0, false), ClaimMerge::Adopted);
        assert_eq!(store.merge(revoke(&alice, 2), 0, false), ClaimMerge::Stale);
    }

    /// Handbook §4.5 acceptance: a friendship claim with a valid patron
    /// countersig is adopted.
    #[test]
    fn friendship_claim_with_valid_countersig_is_adopted() {
        let alice = identity(1); // dependent
        let bob = identity(2); // patron
        let since = 1_000;
        let preimage = mmp_types::claim::friendship_signed_object(&alice.public(), &bob.public(), since);
        let countersig = bob.sign(SigDomain::Friend, &preimage);
        let body = mmp_types::claim::Friendship { friend: bob.public(), since, countersig };
        let claim = signed_claim(&alice, field::FRIENDSHIP, 1, body.encode());

        let mut store = ClaimStore::new();
        assert_eq!(store.merge(claim, 0, false), ClaimMerge::Adopted);
    }

    /// A friendship claim whose countersig doesn't verify (wrong signer, or
    /// signed over a different preimage) is rejected outright — the "patron"
    /// never actually agreed.
    #[test]
    fn friendship_claim_with_bad_countersig_is_rejected() {
        let alice = identity(1); // dependent
        let bob = identity(2); // claimed patron
        let mallory = identity(3); // actually signs — not bob
        let since = 1_000;
        let preimage = mmp_types::claim::friendship_signed_object(&alice.public(), &bob.public(), since);
        let bad_countersig = mallory.sign(SigDomain::Friend, &preimage);
        let body = mmp_types::claim::Friendship { friend: bob.public(), since, countersig: bad_countersig };
        let claim = signed_claim(&alice, field::FRIENDSHIP, 1, body.encode());

        let mut store = ClaimStore::new();
        assert_eq!(store.merge(claim, 0, false), ClaimMerge::Rejected);
        let n = node_id_of(&alice.public());
        assert!(store.get(&n, field::FRIENDSHIP).is_none());
    }

    /// The `revoked_key` inside the body must match the claim's own `owner` —
    /// alice cannot sign a claim purporting to revoke someone else's key.
    #[test]
    fn revocation_with_owner_mismatch_is_rejected() {
        let alice = identity(1);
        let carol = identity(4);
        let mut store = ClaimStore::new();
        let body =
            Revocation { revoked_key: carol.public(), reason: 0, successor: None, successor_countersig: None };
        let rev = signed_claim(&alice, field::REVOCATION, 1, body.encode());
        assert_eq!(store.merge(rev, 0, false), ClaimMerge::Rejected);

        let n = node_id_of(&alice.public());
        assert!(store.revocation(&n).is_none());
    }

    #[test]
    fn known_nickname_body_bounds_are_enforced_at_merge() {
        let alice = identity(5);
        let mut store = ClaimStore::new();

        let oversized = signed_claim(
            &alice,
            field::NICKNAME,
            1,
            vec![b'x'; mmp_types::consts::NICKNAME_MAX + 1],
        );
        assert_eq!(store.merge(oversized, 0, false), ClaimMerge::Rejected);

        let non_utf8 = signed_claim(&alice, field::NICKNAME, 2, vec![0xff]);
        assert_eq!(store.merge(non_utf8, 0, false), ClaimMerge::Rejected);

        let node = node_id_of(&alice.public());
        assert!(store.get(&node, field::NICKNAME).is_none());
    }

    #[test]
    fn unknown_claim_fields_remain_opaque_at_merge() {
        let alice = identity(6);
        let mut store = ClaimStore::new();
        let claim = signed_claim(
            &alice,
            0xf0,
            1,
            vec![0xff; mmp_types::consts::NICKNAME_MAX + 1],
        );
        assert_eq!(store.merge(claim, 0, false), ClaimMerge::Adopted);
    }

    #[test]
    fn prekey_claim_with_bad_signed_prekey_signature_is_rejected() {
        let alice = identity(7);
        let mut batch = mmp_types::claim::PrekeyBatch {
            signed_prekey: Key32([8; 32]),
            spk_sig: Sig64::ZERO,
            prekeys: vec![Key32([9; 32])],
        };
        batch.spk_sig = alice.sign(SigDomain::Prekey, &batch.signed_object());
        batch.spk_sig.0[0] ^= 0x80;
        let claim = signed_claim(&alice, field::PREKEY_BATCH, 1, batch.encode());

        let mut store = ClaimStore::new();
        assert_eq!(store.merge(claim, 0, false), ClaimMerge::Rejected);
    }
}
