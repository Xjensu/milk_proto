//! Authority/announcement admission state (doc 18) — AuthCert verification,
//! sticky revocation, and scope subscription. Sans-IO: `now_unix_s` is the
//! receiver's own wall clock, passed in by the caller exactly like `now_ms`
//! elsewhere in this crate (doc 18 §6.3 — a DTN has no shared clock).
//!
//! The per-authority *token bucket* (doc 18 §5.3) deliberately does NOT live
//! here — it's rate-limiting policy state owned by `mmp-node` (architecture
//! decision A3.1), mirroring `mmp-router`'s per-origin buckets. This module
//! only answers "is this cert valid and what does it authorize," not "has the
//! bucket got room."

use alloc::collections::{BTreeMap, BTreeSet};

use mmp_types::authcert::{AuthCert, RevocationList, CAP_ANNOUNCE, CAP_PRIORITY_EMERGENCY};
use mmp_types::bundle::prio;
use mmp_types::consts::NOT_AFTER_SKEW_S;
use mmp_types::domain::SigDomain;
use mmp_types::ids::{Id16, Key32};

/// Doc 18 §2 verification failure reasons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CertReject {
    /// `cert.suite` isn't the one supported suite (`0x01`).
    BadSuite,
    /// The `mmp1/authcert` signature over `cert.signed_object()` doesn't
    /// verify under the pinned issuer key.
    BadSignature,
    /// No pinned root for the cert's scope, or the cert's `issuer` doesn't
    /// match the one pinned for it.
    UntrustedIssuer,
    /// `now_unix_s` is past `cert.not_after` (plus `NOT_AFTER_SKEW_S`).
    Expired,
    /// The `(subject, scope)` pair is on the sticky revocation list as of
    /// `now_unix_s`.
    Revoked,
}

/// Doc 18 §2.2: two trust levels for an operator root. Only `Pinned` is wired
/// in this slice (gossiped-root learning is deferred, E5-S5 scope call) — the
/// variant is reserved so the enum doesn't need to change when it lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootTrust {
    /// The root was pinned locally (e.g. via a QR-ceremony) rather than
    /// learned from gossip.
    Pinned,
}

/// Verification + sticky-revocation + scope-subscription state (doc 18).
/// `no_std`-clean: nothing here forecloses an eventual Core-tier build.
#[derive(Default)]
pub struct AuthorityStore {
    subscribed: BTreeSet<Id16>,
    roots: BTreeMap<Id16, (Key32, RootTrust)>,
    certs: BTreeMap<(Key32, Id16), AuthCert>,
    /// Sticky union (doc 18 §6, "revoked wins, sticky"): (subject, scope) →
    /// the latest `not_after` ever seen from a verified revocation entry.
    /// Only grows (via `merge_revocation_list`) or prunes past its own
    /// expiry (`expire`) — never shrinks because a later list omitted it.
    revoked: BTreeMap<(Key32, Id16), u32>,
}

impl AuthorityStore {
    /// Builds an empty store: no subscriptions, no pinned roots, no certs,
    /// no revocations.
    pub fn new() -> AuthorityStore {
        AuthorityStore::default()
    }

    /// Doc 18 §4: a local filter only, never transmitted.
    pub fn subscribe(&mut self, scope: Id16) {
        self.subscribed.insert(scope);
    }

    /// Reverses [`AuthorityStore::subscribe`]: `scope` no longer passes the
    /// local subscription filter.
    pub fn unsubscribe(&mut self, scope: &Id16) {
        self.subscribed.remove(scope);
    }

    /// Whether `scope` currently passes the local subscription filter set by
    /// [`AuthorityStore::subscribe`]/[`AuthorityStore::unsubscribe`].
    pub fn is_subscribed(&self, scope: &Id16) -> bool {
        self.subscribed.contains(scope)
    }

    /// Pin a trusted issuer for `scope` (doc 18 §2.2 QR-ceremony entry point).
    pub fn pin_root(&mut self, scope: Id16, issuer: Key32) {
        self.roots.insert(scope, (issuer, RootTrust::Pinned));
    }

    fn revoked_at(&self, subject: &Key32, scope: &Id16, now_unix_s: u32) -> bool {
        self.revoked
            .get(&(*subject, *scope))
            .is_some_and(|&not_after| now_unix_s <= not_after.saturating_add(NOT_AFTER_SKEW_S))
    }

    /// Doc 18 §2 verification: suite, a trusted root for the cert's scope
    /// whose key matches `issuer`, the `mmp1/authcert` signature, unexpired
    /// (±skew), and not revoked. On success the cert is adopted (keyed by
    /// `(subject, scope)` — a later verified cert for the same pair replaces
    /// it, matching the singleton-slot claim-gossip semantics it usually
    /// arrives through, doc 18 §2.1).
    pub fn consider_cert(&mut self, cert: &AuthCert, now_unix_s: u32) -> Result<(), CertReject> {
        if cert.suite != 0x01 {
            return Err(CertReject::BadSuite);
        }
        let Some(&(issuer, _)) = self.roots.get(&cert.scope) else {
            return Err(CertReject::UntrustedIssuer);
        };
        if issuer != cert.issuer {
            return Err(CertReject::UntrustedIssuer);
        }
        if mmp_crypto::verify(SigDomain::AuthCert, &cert.issuer, &cert.signed_object(), &cert.sig).is_err() {
            return Err(CertReject::BadSignature);
        }
        if now_unix_s > cert.not_after.saturating_add(NOT_AFTER_SKEW_S) {
            return Err(CertReject::Expired);
        }
        if self.revoked_at(&cert.subject, &cert.scope, now_unix_s) {
            return Err(CertReject::Revoked);
        }
        self.certs.insert((cert.subject, cert.scope), *cert);
        Ok(())
    }

    /// Doc 18 §6 subject-signed tombstone path: drop every cert this subject
    /// holds (retirement/device-loss only — NOT a defense against key theft,
    /// per the doc's own explicit caveat; that's what revocation lists are for).
    pub fn drop_subject_certs(&mut self, subject: &Key32) {
        self.certs.retain(|(s, _), _| s != subject);
    }

    /// Doc 18 §6 issuer-signed revocation, "revoked wins, sticky": insert-or-
    /// max each entry's `not_after`, never remove on a shorter/absent later
    /// list. `issuer` must be the trusted root for an entry's scope — an
    /// issuer cannot revoke into a scope it doesn't own (mirrors §5.2's
    /// cross-scope rule for grants).
    pub fn merge_revocation_list(&mut self, issuer: &Key32, list: &RevocationList) {
        for entry in &list.entries {
            if self.roots.get(&entry.scope).map(|(i, _)| i) != Some(issuer) {
                continue;
            }
            let slot = self.revoked.entry((entry.subject, entry.scope)).or_insert(0);
            if entry.not_after > *slot {
                *slot = entry.not_after;
            }
        }
    }

    /// Doc 18 §5.2 elevation predicate, minus the token bucket (that's
    /// `mmp-node`'s `AuthorityBuckets`): a verified, unexpired, unrevoked cert
    /// for `(subject, scope)` whose `caps` admit the `declared` priority.
    /// Doc 18 §2's caps comment is explicit: EMERGENCY (0) needs BOTH
    /// `CAP_ANNOUNCE` and `CAP_PRIORITY_EMERGENCY`; OPERATIONAL (1) needs only
    /// `CAP_ANNOUNCE`.
    pub fn grant_for(&self, subject: &Key32, scope: &Id16, declared: u8, now_unix_s: u32) -> Option<&AuthCert> {
        let cert = self.certs.get(&(*subject, *scope))?;
        if now_unix_s > cert.not_after.saturating_add(NOT_AFTER_SKEW_S) {
            return None;
        }
        if self.revoked_at(subject, scope, now_unix_s) {
            return None;
        }
        let needed = if declared == prio::EMERGENCY { CAP_ANNOUNCE | CAP_PRIORITY_EMERGENCY } else { CAP_ANNOUNCE };
        if cert.caps & needed != needed {
            return None;
        }
        Some(cert)
    }

    /// Doc 18 §5.2.1 item 2 rendering predicate: "rendering is a pure
    /// function of (announcement, cert) with no anti-abuse state" — no
    /// bucket, independent of declared priority.
    pub fn official(&self, subject: &Key32, scope: &Id16, now_unix_s: u32) -> bool {
        self.certs.get(&(*subject, *scope)).is_some_and(|cert| {
            now_unix_s <= cert.not_after.saturating_add(NOT_AFTER_SKEW_S)
                && !self.revoked_at(subject, scope, now_unix_s)
                && cert.caps & CAP_ANNOUNCE != 0
        })
    }

    /// Prune certs past their own `not_after` + skew (issuer-prune mirror of
    /// doc 18 §6's REVLIST_MAX discipline — bounded state).
    pub fn expire(&mut self, now_unix_s: u32) {
        self.certs.retain(|_, c| now_unix_s <= c.not_after.saturating_add(NOT_AFTER_SKEW_S));
    }
}

/// Doc 18 §5.2: the effective receiver-local priority for a class-5 bundle.
/// `bucket_admit` is called at most once, and only when every other
/// condition already holds — callers should wire it to a token-bucket
/// `admit()` so a bucket is never charged for a bundle that wouldn't elevate
/// anyway. Priorities ≥ NORMAL pass through unclamped (nothing to elevate).
pub fn effective_class5_priority(
    declared: u8,
    subscribed: bool,
    grant: Option<&AuthCert>,
    bucket_admit: impl FnOnce(&AuthCert) -> bool,
) -> u8 {
    if declared >= prio::NORMAL {
        return declared;
    }
    match grant {
        Some(cert) if subscribed && bucket_admit(cert) => declared,
        _ => prio::NORMAL,
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use mmp_crypto::Identity;
    use mmp_types::authcert::AnnouncementBody;

    use super::*;

    fn signed_cert(issuer: &Identity, subject: Key32, scope: Id16, caps: u8, not_after: u32) -> AuthCert {
        let mut cert = AuthCert {
            suite: 1,
            subject,
            scope,
            caps,
            priority_budget: 10,
            not_after,
            issuer: issuer.public(),
            sig: mmp_types::ids::Sig64::ZERO,
        };
        cert.sig = issuer.sign(SigDomain::AuthCert, &cert.signed_object());
        cert
    }

    #[test]
    fn untrusted_issuer_without_pinned_root_is_rejected() {
        let issuer = Identity::from_seed(&[1; 32]);
        let subject = Key32([2; 32]);
        let scope = Id16([3; 16]);
        let cert = signed_cert(&issuer, subject, scope, CAP_ANNOUNCE, 1000);
        let mut store = AuthorityStore::new();
        // No pin_root call for `scope` — cert must not verify.
        assert_eq!(store.consider_cert(&cert, 0), Err(CertReject::UntrustedIssuer));
    }

    #[test]
    fn pinned_root_admits_and_grants_within_caps() {
        let issuer = Identity::from_seed(&[1; 32]);
        let subject = Key32([2; 32]);
        let scope = Id16([3; 16]);
        let mut store = AuthorityStore::new();
        store.pin_root(scope, issuer.public());
        let cert = signed_cert(&issuer, subject, scope, CAP_ANNOUNCE, 1000);
        assert_eq!(store.consider_cert(&cert, 0), Ok(()));
        // OPERATIONAL (1) needs only CAP_ANNOUNCE.
        assert!(store.grant_for(&subject, &scope, prio::OPERATIONAL, 0).is_some());
        // EMERGENCY (0) additionally needs CAP_PRIORITY_EMERGENCY, absent here.
        assert!(store.grant_for(&subject, &scope, prio::EMERGENCY, 0).is_none());
        assert!(store.official(&subject, &scope, 0));
    }

    #[test]
    fn cross_scope_cert_grants_nothing_for_a_different_scope() {
        let issuer = Identity::from_seed(&[1; 32]);
        let subject = Key32([2; 32]);
        let scope_x = Id16([3; 16]);
        let scope_y = Id16([4; 16]);
        let mut store = AuthorityStore::new();
        store.pin_root(scope_x, issuer.public());
        let cert = signed_cert(&issuer, subject, scope_x, CAP_ANNOUNCE, 1000);
        store.consider_cert(&cert, 0).unwrap();
        assert!(store.grant_for(&subject, &scope_y, prio::OPERATIONAL, 0).is_none());
    }

    #[test]
    fn expired_cert_is_rejected_beyond_skew_and_grants_nothing() {
        let issuer = Identity::from_seed(&[1; 32]);
        let subject = Key32([2; 32]);
        let scope = Id16([3; 16]);
        let mut store = AuthorityStore::new();
        store.pin_root(scope, issuer.public());
        let cert = signed_cert(&issuer, subject, scope, CAP_ANNOUNCE, 1000);
        assert_eq!(store.consider_cert(&cert, 1000 + NOT_AFTER_SKEW_S), Ok(()));
        assert_eq!(store.consider_cert(&cert, 1000 + NOT_AFTER_SKEW_S + 1), Err(CertReject::Expired));
    }

    #[test]
    fn revocation_is_sticky_across_a_later_list_that_omits_the_entry() {
        let issuer = Identity::from_seed(&[1; 32]);
        let subject = Key32([2; 32]);
        let scope = Id16([3; 16]);
        let mut store = AuthorityStore::new();
        store.pin_root(scope, issuer.public());
        let cert = signed_cert(&issuer, subject, scope, CAP_ANNOUNCE, 1000);
        store.consider_cert(&cert, 0).unwrap();
        assert!(store.grant_for(&subject, &scope, prio::OPERATIONAL, 0).is_some());

        let revoke = RevocationList {
            suite: 1,
            entries: vec![mmp_types::authcert::RevEntry { subject, scope, not_after: 999 }],
        };
        store.merge_revocation_list(&issuer.public(), &revoke);
        assert!(store.grant_for(&subject, &scope, prio::OPERATIONAL, 0).is_none());

        // A later list that simply omits the entry must NOT un-revoke it.
        let later_empty = RevocationList { suite: 1, entries: vec![] };
        store.merge_revocation_list(&issuer.public(), &later_empty);
        assert!(store.grant_for(&subject, &scope, prio::OPERATIONAL, 0).is_none(), "sticky: omission must not restore");
    }

    #[test]
    fn effective_class5_priority_clamps_without_a_grant_and_elevates_with_one() {
        let issuer = Identity::from_seed(&[1; 32]);
        let subject = Key32([2; 32]);
        let scope = Id16([3; 16]);
        let cert = signed_cert(&issuer, subject, scope, CAP_ANNOUNCE, 1000);

        assert_eq!(effective_class5_priority(prio::OPERATIONAL, true, None, |_| true), prio::NORMAL);
        assert_eq!(effective_class5_priority(prio::OPERATIONAL, false, Some(&cert), |_| true), prio::NORMAL);
        assert_eq!(effective_class5_priority(prio::OPERATIONAL, true, Some(&cert), |_| false), prio::NORMAL);
        assert_eq!(effective_class5_priority(prio::OPERATIONAL, true, Some(&cert), |_| true), prio::OPERATIONAL);
        // BULK (3) is already ≥ NORMAL: passes through regardless.
        assert_eq!(effective_class5_priority(prio::BULK, false, None, |_| true), prio::BULK);
    }

    #[test]
    fn announcement_body_signed_object_is_reused_by_the_authority_layer_unmodified() {
        // Sanity: authority.rs doesn't touch AnnouncementBody signing at all —
        // that's mmp-node's job — but confirms the import compiles cleanly
        // against the type this module's callers will pass around.
        let body = AnnouncementBody {
            suite: 1,
            ann_type: 0,
            scope: Id16([1; 16]),
            not_after: 0,
            text: alloc::vec::Vec::new(),
            sig: mmp_types::ids::Sig64::ZERO,
        };
        assert!(body.signed_object().starts_with(&[1]));
    }
}
