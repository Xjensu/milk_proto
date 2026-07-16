//! Global Ed25519 signing-domain separation (doc 10 §3.1, AUDIT F4).
//!
//! The ONLY place domain tags exist. Every signature in MMP/1 is computed over
//! `dom_tag ‖ object_bytes`; the tag set is prefix-free (each tag ends 0x01).
//! Signing or verifying without [`signed_preimage`] must be impossible outside
//! `mmp-crypto` — that crate's `sign`/`verify` take a `SigDomain`, never raw bytes.

use alloc::vec::Vec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SigDomain {
    Bundle,
    Claim,
    Friend,
    Prekey,
    BlobRoot,
    ChanBind,
    Manifest,
    AuthCert,
    Announce,
    /// Class-2 member signature under the member's *group* signature key
    /// (doc 10 §4.2 defines the field; the registry assigns no tag —
    /// implementation-assigned, prefix-freeness preserved).
    GroupMsg,
}

impl SigDomain {
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
