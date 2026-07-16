//! Identity = Ed25519 keypair (doc 03 §3, doc 08 §2). Keys never rotate.
//!
//! ALL signature creation/verification in the system flows through
//! [`Identity::sign`] / [`verify`], which force a [`SigDomain`] — signing raw
//! bytes is unrepresentable outside this crate (doc 10 §3.1, AUDIT F4).

use ed25519_dalek::{Signature, SigningKey, VerifyingKey};
use mmp_types::domain::{signed_preimage, SigDomain};
use mmp_types::ids::{node_id_of, Key32, NodeId, Sig64};
use zeroize::Zeroizing;

use crate::error::CryptoError;

/// Local identity: Ed25519 signing key + derived X25519 exchange secret.
///
/// The exchange secret is derived deterministically from the identity seed
/// (`blake3::derive_key`) so a restored seed restores the full identity; it is
/// published as the exchange-key claim (doc 03 §3.3) and doubles as the Noise
/// XX static (doc 08 §2).
///
/// `Clone` lets the std node hold one copy for its session engines and one for
/// its delivery path without a borrow conflict; secrets still zeroize on drop.
#[derive(Clone)]
pub struct Identity {
    signing: SigningKey,
    exchange: x25519_dalek::StaticSecret,
    public: Key32,
    exchange_public: Key32,
    node_id: NodeId,
}

impl Identity {
    pub fn from_seed(seed: &[u8; 32]) -> Identity {
        let signing = SigningKey::from_bytes(seed);
        let x_seed = Zeroizing::new(blake3::derive_key("mmp1 exchange-x25519 v1", seed));
        let exchange = x25519_dalek::StaticSecret::from(*x_seed);
        let public = Key32(signing.verifying_key().to_bytes());
        let exchange_public = Key32(x25519_dalek::PublicKey::from(&exchange).to_bytes());
        let node_id = node_id_of(&public);
        Identity { signing, exchange, public, exchange_public, node_id }
    }

    pub fn public(&self) -> Key32 {
        self.public
    }

    pub fn exchange_public(&self) -> Key32 {
        self.exchange_public
    }

    pub fn node_id(&self) -> NodeId {
        self.node_id
    }

    /// Domain-separated Ed25519 signature (doc 10 §3.1).
    pub fn sign(&self, dom: SigDomain, object: &[u8]) -> Sig64 {
        use ed25519_dalek::Signer;
        let msg = signed_preimage(dom, object);
        Sig64(self.signing.sign(&msg).to_bytes())
    }

    /// X25519 agreement with the identity's static exchange secret.
    pub fn exchange_dh(&self, their: &Key32) -> [u8; 32] {
        let pk = x25519_dalek::PublicKey::from(their.0);
        self.exchange.diffie_hellman(&pk).to_bytes()
    }

    /// Raw exchange secret bytes — for the Noise static only. Zeroizing wrapper
    /// keeps the copy from outliving its use.
    pub(crate) fn exchange_secret_bytes(&self) -> Zeroizing<[u8; 32]> {
        Zeroizing::new(self.exchange.to_bytes())
    }
}

/// Strict (canonical-S, RFC 8032 §5.1.7) domain-separated verification.
///
/// This wraps `ed25519-dalek` so the permissive `verify` can never be called
/// (AUDIT F9); clippy `disallowed-methods` backs this up in CI.
pub fn verify(dom: SigDomain, pk: &Key32, object: &[u8], sig: &Sig64) -> Result<(), CryptoError> {
    let vk = VerifyingKey::from_bytes(&pk.0).map_err(|_| CryptoError::BadKey)?;
    let msg = signed_preimage(dom, object);
    vk.verify_strict(&msg, &Signature::from_bytes(&sig.0))
        .map_err(|_| CryptoError::BadSignature)
}

/// A random-looking Ed25519 group signature keypair for class-2 sender keys
/// (distinct from the identity key — limits cross-context linkage, doc 08 §6.2).
pub fn signing_key_from_seed(seed: &[u8; 32]) -> SigningKey {
    SigningKey::from_bytes(seed)
}
