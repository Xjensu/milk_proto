//! Group sender keys + epochs (doc 08 §6).
//!
//! Per (group, epoch, member): a chain key and a DISTINCT Ed25519 group
//! signature keypair (limits cross-context signature linkage). Distribution
//! happens over pairwise conversation channels; epoch++ on membership change
//! re-keys everyone (future secrecy vs removed members).

use alloc::vec;
use alloc::vec::Vec;

use ed25519_dalek::Signer;
use mmp_types::domain::{signed_preimage, SigDomain};
use mmp_types::err::WireError;
use mmp_types::ids::{Id16, Key32, NodeId, Sig64};
use mmp_types::payload::GroupPayload;
use mmp_types::wire::{BufExt, Reader};
use rand_core::CryptoRngCore;
use zeroize::Zeroizing;

use crate::aead;
use crate::error::CryptoError;
use crate::identity::verify;
use crate::ratchet::{RecvChain, SendChain};

/// Sender-key distribution record, sent inside the pairwise channel as
/// apptype 0x02 (relays never see it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SenderKeyDist {
    /// Group id this sender key belongs to.
    pub gid: Id16,
    /// Epoch this sender key was minted for.
    pub epoch: u16,
    /// Seed for the recipient's [`RecvChain`], mirroring the sender's chain.
    pub chain_seed: [u8; 32],
    /// Public half of the per-(group, epoch) group signature keypair.
    pub sig_pub: Key32,
}

impl SenderKeyDist {
    /// Wire encoding: `gid ‖ epoch ‖ chain_seed ‖ sig_pub`.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(16 + 2 + 32 + 32);
        out.extend_from_slice(&self.gid.0);
        out.put_u16(self.epoch);
        out.extend_from_slice(&self.chain_seed);
        out.extend_from_slice(&self.sig_pub.0);
        out
    }

    /// Decode a [`SenderKeyDist::encode`]d record; rejects trailing bytes.
    pub fn decode(bytes: &[u8]) -> Result<SenderKeyDist, WireError> {
        let mut r = Reader::new(bytes);
        let gid = r.id16()?;
        let epoch = r.u16()?;
        let mut chain_seed = [0u8; 32];
        chain_seed.copy_from_slice(r.take(32)?);
        let sig_pub = r.key32()?;
        r.finish()?;
        Ok(SenderKeyDist { gid, epoch, chain_seed, sig_pub })
    }
}

/// Our own sender key for (group, epoch).
pub struct LocalSenderKey {
    chain: SendChain,
    sig: ed25519_dalek::SigningKey,
    dist: SenderKeyDist,
}

impl LocalSenderKey {
    /// Mint a fresh sender key for (`gid`, `epoch`): a random chain seed and a
    /// new, identity-distinct Ed25519 group signing keypair.
    pub fn generate(rng: &mut impl CryptoRngCore, gid: Id16, epoch: u16) -> LocalSenderKey {
        let mut chain_seed = [0u8; 32];
        rng.fill_bytes(&mut chain_seed);
        let mut sig_seed = Zeroizing::new([0u8; 32]);
        rng.fill_bytes(&mut sig_seed[..]);
        let sig = ed25519_dalek::SigningKey::from_bytes(&sig_seed);
        let dist = SenderKeyDist {
            gid,
            epoch,
            chain_seed,
            sig_pub: Key32(sig.verifying_key().to_bytes()),
        };
        LocalSenderKey { chain: SendChain::new(chain_seed, epoch), sig, dist }
    }

    /// The distributable record (`gid`, `epoch`, `chain_seed`, `sig_pub`) to
    /// send to other members over their pairwise channels.
    pub fn dist(&self) -> &SenderKeyDist {
        &self.dist
    }

    /// The epoch this sender key was generated for.
    pub fn epoch(&self) -> u16 {
        self.dist.epoch
    }

    /// Encrypt + sign one group message body. `ad` binds the routing header
    /// (doc 10 §4.1 discipline applied to class 2).
    pub fn encrypt(&mut self, member: NodeId, ad: &[u8], body: &[u8]) -> GroupPayload {
        self.encrypt_with_ad(member, body, |_| ad.to_vec())
    }

    /// Fallible counterpart to [`Self::encrypt`].  The wrapper is retained
    /// for source compatibility, while production callers should prefer
    /// this method so ratchet exhaustion is an explicit error instead of a
    /// panic.
    pub fn try_encrypt(
        &mut self,
        member: NodeId,
        ad: &[u8],
        body: &[u8],
    ) -> Result<GroupPayload, CryptoError> {
        self.try_encrypt_with_ad(member, body, |_| ad.to_vec())
    }

    /// Encrypt + sign a group body after the caller has constructed the exact
    /// outer bundle header that will be used as AEAD associated data. The
    /// placeholder payload passed to `make_ad` has the final ciphertext length
    /// and all routing fields, so the resulting AD includes `payload_len`
    /// without consuming a second sender-key index.
    pub fn encrypt_with_ad<F>(&mut self, member: NodeId, body: &[u8], make_ad: F) -> GroupPayload
    where
        F: FnOnce(&GroupPayload) -> Vec<u8>,
    {
        self.try_encrypt_with_ad(member, body, make_ad)
            .expect("group sender-key ratchet exhausted")
    }

    /// Fallible variant of [`Self::encrypt_with_ad`].  The sender chain is
    /// advanced only after the caller's payload has passed its own size
    /// checks; exhaustion therefore leaves the group state unchanged and
    /// lets the node retire or re-key the group explicitly.
    pub fn try_encrypt_with_ad<F>(
        &mut self,
        member: NodeId,
        body: &[u8],
        make_ad: F,
    ) -> Result<GroupPayload, CryptoError>
    where
        F: FnOnce(&GroupPayload) -> Vec<u8>,
    {
        let (key, epoch, index) = self.chain.try_next()?;
        let mut p = GroupPayload {
            gid: self.dist.gid,
            epoch,
            member,
            chain_index: index,
            ct: vec![0u8; body.len() + 16],
            member_sig: Sig64::ZERO,
        };
        let ad = make_ad(&p);
        let bound = aead::committed_key(&key, &group_context(&self.dist.gid, epoch, &member, index));
        p.ct = aead::seal(&bound, &ad, body);
        let msg = signed_preimage(SigDomain::GroupMsg, &p.signed_object());
        p.member_sig = Sig64(self.sig.sign(&msg).to_bytes());
        Ok(p)
    }
}

/// A remote member's sender key for (group, epoch).
pub struct RemoteSenderKey {
    recv: RecvChain,
    sig_pub: Key32,
    gid: Id16,
}

impl RemoteSenderKey {
    /// Build the receive side from a peer's distributed [`SenderKeyDist`],
    /// tolerating reordering within `window` (see [`RecvChain::new`]).
    pub fn from_dist(dist: &SenderKeyDist, window: u32) -> RemoteSenderKey {
        RemoteSenderKey {
            recv: RecvChain::new(dist.chain_seed, dist.epoch, window),
            sig_pub: dist.sig_pub,
            gid: dist.gid,
        }
    }

    /// Verify the member signature, then decrypt.
    pub fn decrypt(&mut self, p: &GroupPayload, ad: &[u8]) -> Result<Vec<u8>, CryptoError> {
        verify(SigDomain::GroupMsg, &self.sig_pub, &p.signed_object(), &p.member_sig)?;
        let key = self.recv.key_for(p.epoch, p.chain_index)?;
        let bound =
            aead::committed_key(&key, &group_context(&self.gid, p.epoch, &p.member, p.chain_index));
        aead::open(&bound, ad, &p.ct)
    }
}

/// AEAD context commitment input (doc 16 §4.2 CR-S2 applied to groups).
fn group_context(gid: &Id16, epoch: u16, member: &NodeId, index: u32) -> Vec<u8> {
    let mut c = Vec::with_capacity(16 + 2 + 16 + 4);
    c.extend_from_slice(&gid.0);
    c.put_u16(epoch);
    c.extend_from_slice(&member.0);
    c.put_u32(index);
    c
}

#[cfg(test)]
mod tests {
    use super::LocalSenderKey;
    use crate::CryptoError;
    use mmp_types::ids::Id16;
    use rand::{rngs::StdRng, SeedableRng};

    #[test]
    fn fallible_group_encrypt_reports_ratchet_exhaustion() {
        let gid = Id16([0x42; 16]);
        let member = Id16([0x24; 16]);
        let mut rng = StdRng::seed_from_u64(7);
        let mut local = LocalSenderKey::generate(&mut rng, gid, 1);
        local.chain.next_index = u32::MAX;

        local
            .try_encrypt(member, b"ad", b"terminal")
            .expect("terminal index is valid once");
        assert_eq!(
            local.try_encrypt(member, b"ad", b"after"),
            Err(CryptoError::Ratchet)
        );
    }
}
