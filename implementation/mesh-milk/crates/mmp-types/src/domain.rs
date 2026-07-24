//! Global Ed25519 signing-domain separation (doc 10 §3.1, AUDIT F4).
//!
//! The ONLY place domain tags exist. Every signature in MMP/1 is computed over
//! `dom_tag ‖ object_bytes`; the tag set is prefix-free (each tag ends 0x01).
//! Signing or verifying without [`signed_preimage`] must be impossible outside
//! `mmp-crypto` — that crate's `sign`/`verify` take a `SigDomain`, never raw bytes.

use alloc::vec::Vec;

/// The registry of Ed25519 signing domains (doc 10 §3.1). Each variant names
/// one class of signed object and maps ([`SigDomain::tag`]) to a distinct,
/// prefix-free byte tag prepended to that object's bytes before signing —
/// so a signature valid for one object type can never be replayed as valid
/// for another.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SigDomain {
    /// [`crate::bundle::Bundle`] signatures, over the bundle's lifetime-zeroed
    /// canonical bytes.
    Bundle,
    /// [`crate::claim::Claim`] signatures, over `owner ‖ field ‖ seq ‖ body`.
    Claim,
    /// Friendship countersig, by the patron, over
    /// [`crate::claim::friendship_signed_object`].
    Friend,
    /// [`crate::claim::PrekeyBatch`] signatures, over `signed_prekey ‖ count ‖ prekeys…`.
    Prekey,
    /// Blob Merkle-root signatures (doc 16 blob plane).
    BlobRoot,
    /// Channel-binding signatures.
    ChanBind,
    /// [`crate::manifest::FileManifest`] signatures, over `suite ‖ sharer ‖ count ‖ entries`.
    Manifest,
    /// [`crate::authcert::AuthCert`] signatures, over [`crate::authcert::AuthCert::signed_object`].
    AuthCert,
    /// [`crate::authcert::AnnouncementBody`] signatures, over
    /// [`crate::authcert::AnnouncementBody::signed_object`].
    Announce,
    /// Class-2 member signature under the member's *group* signature key
    /// (doc 10 §4.2 defines the field; the registry assigns no tag —
    /// implementation-assigned, prefix-freeness preserved).
    GroupMsg,
}

impl SigDomain {
    /// The domain's prefix-free wire tag, e.g. `b"mmp1/bundle\x01"` — always
    /// ends in `0x01` so no tag can be a prefix of another.
    pub const fn tag(self) -> &'static [u8] {
        match self {
            SigDomain::Bundle => b"mmp1/bundle\x01",
            SigDomain::Claim => b"mmp1/claim\x01",
            SigDomain::Friend => b"mmp1/friend\x01",
            SigDomain::Prekey => b"mmp1/prekey\x01",
            SigDomain::BlobRoot => b"mmp1/blobroot\x01",
            SigDomain::ChanBind => b"mmp1/chanbind\x01",
            SigDomain::Manifest => b"mmp1/manifest\x01",
            SigDomain::AuthCert => b"mmp1/authcert\x01",
            SigDomain::Announce => b"mmp1/announce\x01",
            SigDomain::GroupMsg => b"mmp1/groupmsg\x01",
        }
    }
}

/// `dom_tag ‖ object_bytes` — the exact Ed25519 message (doc 10 §3.1).
pub fn signed_preimage(dom: SigDomain, object: &[u8]) -> Vec<u8> {
    let tag = dom.tag();
    let mut out = Vec::with_capacity(tag.len() + object.len());
    out.extend_from_slice(tag);
    out.extend_from_slice(object);
    out
}
