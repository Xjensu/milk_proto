//! Authority certificates, announcement bodies, and revocation lists (doc 18).
//!
//! All three are plain signed wire objects, same family as `Claim`/`PrekeyBatch`
//! (`claim.rs`) — encode/decode + a `signed_object()` for the caller to run
//! through `mmp_crypto::verify`/`Identity::sign` under the matching `SigDomain`.
//! No verification logic lives here — that's `mmp-core::authority` (stateful:
//! trusted roots, sticky revocation) and `mmp-node` (per-authority token
//! bucket) per architecture decision A3.1.

use alloc::vec::Vec;

use crate::consts::{ANNOUNCE_TEXT_MAX, REVLIST_MAX};
use crate::err::WireError;
use crate::ids::{Id16, Key32, Sig64};
use crate::wire::{BufExt, Reader};

/// Bit 0: MAY announce (admits declared priority ≤ OPERATIONAL/1). Bit 1:
/// MAY declare EMERGENCY/0 — doc 18 §2's `caps` comment states this plainly:
/// "absent bit1 caps the subject at prio 1 (OPERATIONAL)", i.e. EMERGENCY
/// requires BOTH bits. Bits 2-7 reserved.
pub const CAP_ANNOUNCE: u8 = 0b01;
/// See [`CAP_ANNOUNCE`] — the second capability bit, required (together with
/// `CAP_ANNOUNCE`) to declare EMERGENCY/0 priority.
pub const CAP_PRIORITY_EMERGENCY: u8 = 0b10;

/// Doc 18 §2. Wire: `suite(1) subject(32) scope(16) caps(1) priority_budget(2)
/// not_after(4) issuer(32) sig(64)` = 152 B.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthCert {
    /// Cipher-agility selector (doc 08 §9 precedent) — 0x01 for v1. Every
    /// field here is fixed-width, but the byte is carried anyway so a future
    /// suite-0x02 cert can never be confused with a suite-0x01 one under the
    /// same domain tag (doc 18 §2).
    pub suite: u8,
    /// Conductor/staff identity being granted authority.
    pub subject: Key32,
    /// scope_id (`scope_id_of`) this cert is valid for.
    pub scope: Id16,
    /// Bitfield of `CAP_*` constants granted to `subject`.
    pub caps: u8,
    /// Elevated-priority admissions per hour (doc 18 §5.2/§5.3). ⚠
    pub priority_budget: u16,
    /// Wall-clock expiry, checked against the receiver's own local clock
    /// (doc 18 §6.3 — no shared clock in a DTN).
    pub not_after: u32,
    /// Operator root (or delegate) that signed this grant.
    pub issuer: Key32,
    /// Issuer's signature, `mmp1/authcert` scheme, over [`AuthCert::signed_object`].
    pub sig: Sig64,
}

impl AuthCert {
    /// Encode to the fixed 152-byte wire form (see module doc for layout).
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(152);
        out.push(self.suite);
        out.extend_from_slice(&self.subject.0);
        out.extend_from_slice(&self.scope.0);
        out.push(self.caps);
        out.put_u16(self.priority_budget);
        out.put_u32(self.not_after);
        out.extend_from_slice(&self.issuer.0);
        out.extend_from_slice(&self.sig.0);
        out
    }

    /// Strict decode of the wire form produced by [`AuthCert::encode`].
    pub fn decode(bytes: &[u8]) -> Result<AuthCert, WireError> {
        let mut r = Reader::new(bytes);
        let cert = AuthCert {
            suite: r.u8()?,
            subject: r.key32()?,
            scope: r.id16()?,
            caps: r.u8()?,
            priority_budget: r.u16()?,
            not_after: r.u32()?,
            issuer: r.key32()?,
            sig: r.sig64()?,
        };
        r.finish()?;
        Ok(cert)
    }

    /// The `object_bytes` under `SigDomain::AuthCert` (doc 18 §2, CR-S4):
    /// `suite ‖ subject ‖ scope ‖ caps ‖ priority_budget ‖ not_after ‖ issuer`
    /// — everything in [`AuthCert::encode`] except the trailing signature.
    pub fn signed_object(&self) -> Vec<u8> {
        let mut o = self.encode();
        o.truncate(o.len() - 64);
        o
    }
}

/// Doc 18 §3. Payload of a class-5 (ANNOUNCEMENT) bundle.
///
/// Wire carries `suite` even though doc 18's payload-schema listing omits it
/// (see the doc's own §2 rationale for why AuthCert carries it: a future
/// suite-0x02 preimage must not be confusable with suite-0x01 under the same
/// tag). The signed preimage requires `suite`, so a verifier must be able to
/// read it back off the wire — this is a spec-precision gap in the listing,
/// not a design choice; recorded as a `docs/internal/release-evidence/changelog.md` errata candidate rather
/// than silently "fixed" in the doc.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnouncementBody {
    /// Cipher-agility selector; see [`AuthCert::suite`] for the same rationale.
    pub suite: u8,
    /// 0 delay, 1 platform_change, 2 safety_emergency, 3.. reserved.
    pub ann_type: u8,
    /// Redundant with the bundle's `dest` (doc 18 §3) — explicit here for
    /// standalone re-verification after the wrapping bundle has expired.
    pub scope: Id16,
    /// Announcement's own display-validity, wall clock — may be shorter than
    /// the bundle's `lifetime_ms`.
    pub not_after: u32,
    /// UTF-8, length-delimited. `ANNOUNCE_TEXT_MAX` is a local decode-safety
    /// bound, not a protocol cap (doc 18 §3 states none).
    pub text: Vec<u8>,
    /// Inner signature, `mmp1/announce` scheme, by the authority SUBJECT key
    /// (doc 18 §3: outlasts the TTL-bounded bundle store for re-verification).
    pub sig: Sig64,
}

impl AnnouncementBody {
    /// Encode to `suite ‖ ann_type ‖ scope ‖ not_after ‖ text(varbytes) ‖
    /// sig` — note this WIRE order differs from [`AnnouncementBody::signed_object`]'s.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(1 + 1 + 16 + 4 + 2 + self.text.len() + 64);
        out.push(self.suite);
        out.push(self.ann_type);
        out.extend_from_slice(&self.scope.0);
        out.put_u32(self.not_after);
        out.put_var(&self.text);
        out.extend_from_slice(&self.sig.0);
        out
    }

    /// Strict decode of the wire form produced by [`AnnouncementBody::encode`];
    /// `text` is capped at [`ANNOUNCE_TEXT_MAX`].
    pub fn decode(bytes: &[u8]) -> Result<AnnouncementBody, WireError> {
        let mut r = Reader::new(bytes);
        let body = AnnouncementBody {
            suite: r.u8()?,
            ann_type: r.u8()?,
            scope: r.id16()?,
            not_after: r.u32()?,
            text: r.varbytes(ANNOUNCE_TEXT_MAX)?.to_vec(),
            sig: r.sig64()?,
        };
        r.finish()?;
        Ok(body)
    }

    /// The `object_bytes` under `SigDomain::Announce` (doc 18 §3, CR-S4).
    /// **Preimage field order is NOT wire order** — normative as written:
    /// `suite ‖ scope ‖ ann_type ‖ not_after ‖ len(text):u16 ‖ text`, whereas
    /// the wire puts `ann_type` before `scope`. Do not "harmonize" the two;
    /// doing so would silently change every signature.
    pub fn signed_object(&self) -> Vec<u8> {
        let mut o = Vec::with_capacity(1 + 16 + 1 + 4 + 2 + self.text.len());
        o.push(self.suite);
        o.extend_from_slice(&self.scope.0);
        o.push(self.ann_type);
        o.put_u32(self.not_after);
        o.put_var(&self.text);
        o
    }
}

/// One entry of a REVOCATION_LIST body (doc 18 §6). 52 B.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RevEntry {
    /// The AuthCert `subject` this entry revokes.
    pub subject: Key32,
    /// The AuthCert `scope` this entry revokes within.
    pub scope: Id16,
    /// The revoked cert's `not_after` — identifies WHICH grant is revoked
    /// (a subject may hold multiple certs for the same scope over time).
    pub not_after: u32,
}

/// Claim-field `0x08` body (doc 18 §6): the issuer's entire current
/// revocation state as one value (never per-subject slots — doc 06 §4's
/// singleton-LWW-register hazard, doc 18 §6 CR2bis-10). No inner signature:
/// authenticated by the claim wrapper's ordinary `mmp1/claim` owner signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevocationList {
    /// Cipher-agility selector; see [`AuthCert::suite`] for the same rationale.
    pub suite: u8,
    /// The issuer's currently-revoked grants, capped at [`REVLIST_MAX`].
    pub entries: Vec<RevEntry>,
}

impl RevocationList {
    /// Encode to `suite ‖ count(u8) ‖ entries…`.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(2 + 52 * self.entries.len());
        out.push(self.suite);
        out.push(self.entries.len() as u8);
        for e in &self.entries {
            out.extend_from_slice(&e.subject.0);
            out.extend_from_slice(&e.scope.0);
            out.put_u32(e.not_after);
        }
        out
    }

    /// Strict decode; rejects an entry `count` exceeding [`REVLIST_MAX`].
    pub fn decode(bytes: &[u8]) -> Result<RevocationList, WireError> {
        let mut r = Reader::new(bytes);
        let suite = r.u8()?;
        let count = r.u8()? as usize;
        if count > REVLIST_MAX {
            return Err(WireError::Oversize);
        }
        const REV_ENTRY_ENCODED_LEN: usize = 32 + 16 + 4;
        if count > r.remaining() / REV_ENTRY_ENCODED_LEN {
            return Err(WireError::Truncated);
        }
        let mut entries = Vec::with_capacity(count);
        for _ in 0..count {
            entries.push(RevEntry {
                subject: r.key32()?,
                scope: r.id16()?,
                not_after: r.u32()?,
            });
        }
        r.finish()?;
        Ok(RevocationList { suite, entries })
    }
}

/// `scope_id = BLAKE3("mmp1/scope" ‖ scope_name)[0..16]` (doc 18 §4) — a
/// deterministic, self-certifying identifier from a human-readable name,
/// requiring no registrar. Well-known grammars: `train/<operator>/<service>`,
/// `campus/<org>`.
pub fn scope_id_of(scope_name: &[u8]) -> Id16 {
    let mut input = Vec::with_capacity(10 + scope_name.len());
    input.extend_from_slice(b"mmp1/scope");
    input.extend_from_slice(scope_name);
    let hash = blake3::hash(&input);
    let mut id = [0u8; 16];
    id.copy_from_slice(&hash.as_bytes()[..16]);
    Id16(id)
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use super::*;

    fn cert() -> AuthCert {
        AuthCert {
            suite: 1,
            subject: Key32([1; 32]),
            scope: Id16([2; 16]),
            caps: CAP_ANNOUNCE | CAP_PRIORITY_EMERGENCY,
            priority_budget: 12,
            not_after: 1_000_000,
            issuer: Key32([3; 32]),
            sig: Sig64([4; 64]),
        }
    }

    #[test]
    fn authcert_roundtrip_is_152_bytes_and_preimage_excludes_sig() {
        let c = cert();
        let bytes = c.encode();
        assert_eq!(bytes.len(), 152);
        assert_eq!(AuthCert::decode(&bytes).unwrap(), c);
        assert_eq!(c.signed_object().len(), 152 - 64);
        assert_eq!(&c.signed_object()[..], &bytes[..88]);
    }

    #[test]
    fn authcert_decode_rejects_trailing_bytes() {
        let mut bytes = cert().encode();
        bytes.push(0);
        assert_eq!(AuthCert::decode(&bytes), Err(WireError::Trailing));
    }

    fn announcement() -> AnnouncementBody {
        AnnouncementBody {
            suite: 1,
            ann_type: 2,
            scope: Id16([9; 16]),
            not_after: 500,
            text: b"platform 3 delayed".to_vec(),
            sig: Sig64([5; 64]),
        }
    }

    #[test]
    fn announcement_roundtrip_and_preimage_order_differs_from_wire() {
        let a = announcement();
        let bytes = a.encode();
        assert_eq!(AnnouncementBody::decode(&bytes).unwrap(), a);
        // Wire: suite, ann_type, scope...  Preimage: suite, scope, ann_type...
        assert_eq!(bytes[1], a.ann_type);
        assert_eq!(&bytes[2..18], &a.scope.0);
        let pre = a.signed_object();
        assert_eq!(&pre[1..17], &a.scope.0);
        assert_eq!(pre[17], a.ann_type);
    }

    #[test]
    fn revocation_list_roundtrip_and_cap_enforced() {
        let list = RevocationList {
            suite: 1,
            entries: vec![
                RevEntry {
                    subject: Key32([1; 32]),
                    scope: Id16([2; 16]),
                    not_after: 10,
                },
                RevEntry {
                    subject: Key32([3; 32]),
                    scope: Id16([4; 16]),
                    not_after: 20,
                },
            ],
        };
        let bytes = list.encode();
        assert_eq!(RevocationList::decode(&bytes).unwrap(), list);

        // count byte says more entries than REVLIST_MAX allows.
        let mut oversized = vec![1u8, (REVLIST_MAX + 1) as u8];
        oversized.resize(2 + 52 * (REVLIST_MAX + 1), 0);
        assert_eq!(RevocationList::decode(&oversized), Err(WireError::Oversize));
    }

    #[test]
    fn revocation_list_rejects_count_before_allocating_entries() {
        // The count is valid by itself, but no fixed-width RevEntry follows.
        // Decode must reject before reserving REVLIST_MAX entries.
        assert_eq!(
            RevocationList::decode(&[1, REVLIST_MAX as u8]),
            Err(WireError::Truncated)
        );
    }

    #[test]
    fn scope_id_is_deterministic_and_distinct_per_name() {
        let a = scope_id_of(b"train/nationalrail/1234");
        let b = scope_id_of(b"train/nationalrail/1234");
        let c = scope_id_of(b"campus/example");
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
