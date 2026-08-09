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
use alloc::vec::Vec;

use mmp_types::authcert::{AuthCert, RevocationList, CAP_ANNOUNCE, CAP_PRIORITY_EMERGENCY};
use mmp_types::bundle::prio;
use mmp_types::consts::NOT_AFTER_SKEW_S;
use mmp_types::domain::SigDomain;
use mmp_types::ids::{Id16, Key32};
use mmp_types::wire::{BufExt, Reader};

const AUTH_SNAPSHOT_VERSION: u8 = 1;
const MAX_AUTH_SUBSCRIPTIONS: usize = 256;
const MAX_AUTH_ROOTS: usize = 256;
const MAX_AUTH_CERTS: usize = 512;
const MAX_AUTH_REVOKED: usize = 512;
const AUTH_CERT_ENCODED_LEN: usize = 152;

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
    /// The receiver's local certificate map is at capacity. Existing
    /// `(subject, scope)` entries may still be refreshed.
    Capacity,
}

/// Local authority-state admission failure. This is not a wire rejection:
/// the affected scope is quarantined when its sticky revocation union cannot
/// retain another verified entry without evicting an older one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevocationMergeError {
    /// The sticky revocation map reached its local capacity. The affected
    /// scope is quarantined instead of dropping or evicting an entry.
    Capacity,
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

    /// Encode the non-secret authority admission state for a host snapshot.
    ///
    /// This is deliberately a private persistence format, not a wire object:
    /// local subscriptions, pinned roots, verified certificates, and sticky
    /// revocations are receiver-local state. Counts and certificate lengths
    /// are bounded so a hostile or corrupted snapshot cannot cause an
    /// unbounded allocation on restore. The maps are `BTree*`, therefore the
    /// output is deterministic and stable for checksummed snapshots.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.push(AUTH_SNAPSHOT_VERSION);
        out.put_u16(self.subscribed.len() as u16);
        for scope in &self.subscribed {
            out.extend_from_slice(&scope.0);
        }

        out.put_u16(self.roots.len() as u16);
        for (scope, (issuer, trust)) in &self.roots {
            out.extend_from_slice(&scope.0);
            out.extend_from_slice(&issuer.0);
            out.push(match trust {
                RootTrust::Pinned => 0,
            });
        }

        out.put_u16(self.certs.len() as u16);
        for cert in self.certs.values() {
            let encoded = cert.encode();
            out.put_u16(encoded.len() as u16);
            out.extend_from_slice(&encoded);
        }

        out.put_u16(self.revoked.len() as u16);
        for ((subject, scope), not_after) in &self.revoked {
            out.extend_from_slice(&subject.0);
            out.extend_from_slice(&scope.0);
            out.put_u32(*not_after);
        }
        out
    }

    /// Decode a bounded authority snapshot. Cryptographic certificates are
    /// verified again against the restored pinned root; malformed, unknown,
    /// or duplicate records reject the complete snapshot so the caller can
    /// fail closed to an empty authority store. Revocation tombstones may
    /// outlive a quarantined root so that explicit re-pinning cannot silently
    /// erase previously observed revocations.
    pub fn decode(bytes: &[u8]) -> Option<AuthorityStore> {
        let mut r = Reader::new(bytes);
        if r.u8().ok()? != AUTH_SNAPSHOT_VERSION {
            return None;
        }

        let count = |r: &mut Reader<'_>, max: usize, min_bytes: usize| -> Option<usize> {
            let n = r.u16().ok()? as usize;
            if n > max || n.checked_mul(min_bytes)? > r.remaining() {
                return None;
            }
            Some(n)
        };

        let mut store = AuthorityStore::new();
        let subscriptions = count(&mut r, MAX_AUTH_SUBSCRIPTIONS, 16)?;
        for _ in 0..subscriptions {
            let scope = r.id16().ok()?;
            if !store.subscribed.insert(scope) {
                return None;
            }
        }

        let roots = count(&mut r, MAX_AUTH_ROOTS, 49)?;
        for _ in 0..roots {
            let scope = r.id16().ok()?;
            let issuer = r.key32().ok()?;
            if r.u8().ok()? != 0 {
                return None;
            }
            if store.roots.insert(scope, (issuer, RootTrust::Pinned)).is_some() {
                return None;
            }
        }

        let certs = count(&mut r, MAX_AUTH_CERTS, 2 + AUTH_CERT_ENCODED_LEN)?;
        for _ in 0..certs {
            let len = r.u16().ok()? as usize;
            if len != AUTH_CERT_ENCODED_LEN {
                return None;
            }
            let cert = AuthCert::decode(r.take(len).ok()?).ok()?;
            if cert.suite != 0x01
                || store.roots.get(&cert.scope).map(|(issuer, _)| issuer) != Some(&cert.issuer)
                || mmp_crypto::verify(SigDomain::AuthCert, &cert.issuer, &cert.signed_object(), &cert.sig).is_err()
            {
                return None;
            }
            if store.certs.insert((cert.subject, cert.scope), cert).is_some() {
                return None;
            }
        }

        let revoked = count(&mut r, MAX_AUTH_REVOKED, 52)?;
        for _ in 0..revoked {
            let subject = r.key32().ok()?;
            let scope = r.id16().ok()?;
            let not_after = r.u32().ok()?;
            if store.revoked.insert((subject, scope), not_after).is_some() {
                return None;
            }
        }
        r.finish().ok()?;
        Some(store)
    }

    /// Doc 18 §4: a local filter only, never transmitted.
    pub fn subscribe(&mut self, scope: Id16) {
        // Local operator policy still needs a hard bound in the no_std+alloc
        // profile. Re-subscribing an existing scope remains a no-op.
        if self.subscribed.contains(&scope) || self.subscribed.len() < MAX_AUTH_SUBSCRIPTIONS {
            self.subscribed.insert(scope);
        }
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
        // Re-pinning an existing scope is allowed; a new scope is ignored once
        // the local trust-root budget is full. This is fail-closed: it cannot
        // widen authority.
        if self.roots.contains_key(&scope) || self.roots.len() < MAX_AUTH_ROOTS {
            self.roots.insert(scope, (issuer, RootTrust::Pinned));
        }
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
        if !self.certs.contains_key(&(cert.subject, cert.scope)) && self.certs.len() >= MAX_AUTH_CERTS {
            return Err(CertReject::Capacity);
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

    /// Quarantine one scope when its sticky revocation ledger is exhausted.
    /// Silently dropping a new revocation would be fail-open: a certificate
    /// whose tombstone could not be retained might still elevate priority.
    /// Removing the local root and all cached certs preserves the security
    /// direction at the cost of requiring an explicit re-pin after recovery.
    fn quarantine_scope(&mut self, scope: &Id16) {
        self.roots.remove(scope);
        self.certs.retain(|(_, cert_scope), _| cert_scope != scope);
    }

    /// Doc 18 §6 issuer-signed revocation, "revoked wins, sticky": insert-or-
    /// max each entry's `not_after`, never remove on a shorter/absent later
    /// list. `issuer` must be the trusted root for an entry's scope — an
    /// issuer cannot revoke into a scope it doesn't own (mirrors §5.2's
    /// cross-scope rule for grants).
    pub fn merge_revocation_list(&mut self, issuer: &Key32, list: &RevocationList) {
        let _ = self.try_merge_revocation_list(issuer, list);
    }

    /// Fallible revocation-list merge for callers that need an explicit local
    /// capacity signal. The legacy [`AuthorityStore::merge_revocation_list`]
    /// wrapper remains source-compatible and still quarantines affected scopes
    /// when its result is ignored.
    pub fn try_merge_revocation_list(
        &mut self,
        issuer: &Key32,
        list: &RevocationList,
    ) -> Result<(), RevocationMergeError> {
        // Unknown suites remain opaque claim bodies and must never be
        // interpreted by the v1 authority store.
        if list.suite != 0x01 {
            return Ok(());
        }

        // Preflight unique verified keys before mutating the sticky union. A
        // list that would overflow is rejected atomically; affected scopes
        // are quarantined instead of silently dropping a revocation or
        // evicting an older one.
        let mut pending = BTreeSet::new();
        for entry in &list.entries {
            if self.roots.get(&entry.scope).map(|(i, _)| i) == Some(issuer)
                && !self.revoked.contains_key(&(entry.subject, entry.scope))
            {
                pending.insert((entry.subject, entry.scope));
            }
        }
        if self
            .revoked
            .len()
            .checked_add(pending.len())
            .is_none_or(|total| total > MAX_AUTH_REVOKED)
        {
            let affected_scopes: BTreeSet<Id16> = pending.into_iter().map(|(_, scope)| scope).collect();
            for scope in &affected_scopes {
                self.quarantine_scope(scope);
            }
            return Err(RevocationMergeError::Capacity);
        }

        for entry in &list.entries {
            if self.roots.get(&entry.scope).map(|(i, _)| i) != Some(issuer) {
                continue;
            }
            let key = (entry.subject, entry.scope);
            let slot = self.revoked.entry(key).or_insert(0);
            if entry.not_after > *slot {
                *slot = entry.not_after;
            }
        }
        Ok(())
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
        self.revoked.retain(|_, not_after| now_unix_s <= not_after.saturating_add(NOT_AFTER_SKEW_S));
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
    fn unknown_revocation_suite_is_opaque_and_not_applied() {
        let issuer = Identity::from_seed(&[1; 32]);
        let subject = Key32([2; 32]);
        let scope = Id16([3; 16]);
        let mut store = AuthorityStore::new();
        store.pin_root(scope, issuer.public());
        let cert = signed_cert(&issuer, subject, scope, CAP_ANNOUNCE, 1000);
        store.consider_cert(&cert, 0).unwrap();

        // Unknown suites are retained for opaque relay/storage, but v1 must
        // not interpret their entries as v1 revocations.
        let future = RevocationList {
            suite: 0x02,
            entries: vec![mmp_types::authcert::RevEntry { subject, scope, not_after: 999 }],
        };
        store.merge_revocation_list(&issuer.public(), &future);
        assert!(store.grant_for(&subject, &scope, prio::OPERATIONAL, 0).is_some());
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

    #[test]
    fn authority_snapshot_roundtrip_preserves_subscription_cert_and_revocation() {
        let issuer = Identity::from_seed(&[7; 32]);
        let subject = Key32([8; 32]);
        let scope = Id16([9; 16]);
        let mut store = AuthorityStore::new();
        store.pin_root(scope, issuer.public());
        store.subscribe(scope);
        let cert = signed_cert(&issuer, subject, scope, CAP_ANNOUNCE, 10_000);
        store.consider_cert(&cert, 0).unwrap();
        let revoke = RevocationList {
            suite: 1,
            entries: vec![mmp_types::authcert::RevEntry { subject, scope, not_after: 9_999 }],
        };
        store.merge_revocation_list(&issuer.public(), &revoke);

        let restored = AuthorityStore::decode(&store.encode()).expect("valid authority snapshot");
        assert!(restored.is_subscribed(&scope));
        assert!(restored.grant_for(&subject, &scope, prio::OPERATIONAL, 0).is_none());
        assert!(!restored.official(&subject, &scope, 0));
    }

    #[test]
    fn authority_snapshot_rejects_trailing_unknown_version_and_bad_signature() {
        let issuer = Identity::from_seed(&[10; 32]);
        let subject = Key32([11; 32]);
        let scope = Id16([12; 16]);
        let mut store = AuthorityStore::new();
        store.pin_root(scope, issuer.public());
        store.consider_cert(&signed_cert(&issuer, subject, scope, CAP_ANNOUNCE, 10_000), 0).unwrap();
        let encoded = store.encode();

        let mut trailing = encoded.clone();
        trailing.push(0);
        assert!(AuthorityStore::decode(&trailing).is_none());

        let mut unknown_version = encoded.clone();
        unknown_version[0] = 0xff;
        assert!(AuthorityStore::decode(&unknown_version).is_none());

        let mut bad_signature = encoded;
        *bad_signature.last_mut().unwrap() ^= 0x01;
        assert!(AuthorityStore::decode(&bad_signature).is_none());
    }

    #[test]
    fn authority_snapshot_rejects_hostile_counts_before_allocation() {
        // version + four u16 counts, with the first count above the bounded
        // snapshot limit. The decoder must fail closed without attempting a
        // proportional allocation from untrusted disk bytes.
        let bytes = [1u8, 0xff, 0xff, 0, 0, 0, 0, 0, 0];
        assert!(AuthorityStore::decode(&bytes).is_none());
    }

    #[test]
    fn local_subscription_and_root_state_fail_closed_at_capacity() {
        let mut store = AuthorityStore::new();
        for n in 0..MAX_AUTH_SUBSCRIPTIONS {
            let mut scope = [0u8; 16];
            scope[..8].copy_from_slice(&(n as u64).to_le_bytes());
            store.subscribe(Id16(scope));
        }
        let extra_scope = Id16([0xee; 16]);
        store.subscribe(extra_scope);
        assert!(!store.is_subscribed(&extra_scope));

        let mut root_store = AuthorityStore::new();
        for n in 0..MAX_AUTH_ROOTS {
            let mut scope = [0u8; 16];
            scope[..8].copy_from_slice(&(n as u64).to_le_bytes());
            let mut issuer = [0u8; 32];
            issuer[..8].copy_from_slice(&(n as u64).to_le_bytes());
            root_store.pin_root(Id16(scope), Key32(issuer));
        }
        let extra_issuer = Identity::from_seed(&[0xabu8; 32]);
        let extra_cert = signed_cert(&extra_issuer, Key32([0xacu8; 32]), extra_scope, CAP_ANNOUNCE, 1000);
        root_store.pin_root(extra_scope, extra_issuer.public());
        assert_eq!(root_store.consider_cert(&extra_cert, 0), Err(CertReject::UntrustedIssuer));
    }

    #[test]
    fn certificate_admission_rejects_new_entries_at_capacity() {
        let issuer = Identity::from_seed(&[0xadu8; 32]);
        let scope = Id16([0xadu8; 16]);
        let mut store = AuthorityStore::new();
        store.pin_root(scope, issuer.public());

        for n in 0..MAX_AUTH_CERTS {
            let mut subject = [0u8; 32];
            subject[..8].copy_from_slice(&(n as u64).to_le_bytes());
            let cert = signed_cert(&issuer, Key32(subject), scope, CAP_ANNOUNCE, 10_000);
            assert_eq!(store.consider_cert(&cert, 0), Ok(()));
        }

        let mut subject = [0u8; 32];
        subject[..8].copy_from_slice(&(MAX_AUTH_CERTS as u64).to_le_bytes());
        let cert = signed_cert(&issuer, Key32(subject), scope, CAP_ANNOUNCE, 10_000);
        assert_eq!(store.consider_cert(&cert, 0), Err(CertReject::Capacity));
    }

    #[test]
    fn revocation_capacity_quarantines_scope_instead_of_dropping_tombstone() {
        let issuer = Identity::from_seed(&[0xb1u8; 32]);
        let filler_scope = Id16([0xb2u8; 16]);
        let target_scope = Id16([0xb3u8; 16]);
        let target_subject = Key32([0xb4u8; 32]);
        let retained_revoked_subject = Key32([0xb5u8; 32]);
        let filler_subject = Key32([0xb6u8; 32]);
        let mut store = AuthorityStore::new();
        store.pin_root(filler_scope, issuer.public());
        store.pin_root(target_scope, issuer.public());

        let target_cert = signed_cert(&issuer, target_subject, target_scope, CAP_ANNOUNCE, 10_000);
        let retained_revoked_cert = signed_cert(&issuer, retained_revoked_subject, target_scope, CAP_ANNOUNCE, 10_000);
        let filler_cert = signed_cert(&issuer, filler_subject, filler_scope, CAP_ANNOUNCE, 10_000);
        store.consider_cert(&target_cert, 0).unwrap();
        store.consider_cert(&retained_revoked_cert, 0).unwrap();
        store.consider_cert(&filler_cert, 0).unwrap();
        assert!(store.grant_for(&target_subject, &target_scope, prio::OPERATIONAL, 0).is_some());
        assert!(store.grant_for(&filler_subject, &filler_scope, prio::OPERATIONAL, 0).is_some());

        store.merge_revocation_list(
            &issuer.public(),
            &RevocationList {
                suite: 1,
                entries: vec![mmp_types::authcert::RevEntry {
                    subject: retained_revoked_subject,
                    scope: target_scope,
                    not_after: 10_000,
                }],
            },
        );

        for n in 0..(MAX_AUTH_REVOKED - 1) {
            let mut subject = [0u8; 32];
            subject[..8].copy_from_slice(&(n as u64).to_le_bytes());
            store.merge_revocation_list(
                &issuer.public(),
                &RevocationList {
                    suite: 1,
                    entries: vec![mmp_types::authcert::RevEntry {
                        subject: Key32(subject),
                        scope: filler_scope,
                        not_after: 10_000,
                    }],
                },
            );
        }
        assert_eq!(store.revoked.len(), MAX_AUTH_REVOKED);

        assert_eq!(
            store.try_merge_revocation_list(
                &issuer.public(),
                &RevocationList {
                    suite: 1,
                    entries: vec![mmp_types::authcert::RevEntry {
                        subject: target_subject,
                        scope: target_scope,
                        not_after: 10_000,
                    }],
                },
            ),
            Err(RevocationMergeError::Capacity)
        );

        assert!(store.grant_for(&target_subject, &target_scope, prio::OPERATIONAL, 0).is_none());
        assert_eq!(store.consider_cert(&target_cert, 0), Err(CertReject::UntrustedIssuer));
        assert!(store.grant_for(&filler_subject, &filler_scope, prio::OPERATIONAL, 0).is_some());
        assert_eq!(store.revoked.len(), MAX_AUTH_REVOKED);

        let mut restored = AuthorityStore::decode(&store.encode()).expect("quarantined snapshot remains valid");
        assert_eq!(restored.consider_cert(&target_cert, 0), Err(CertReject::UntrustedIssuer));
        assert_eq!(restored.consider_cert(&retained_revoked_cert, 0), Err(CertReject::UntrustedIssuer));

        let mut recovered = restored;
        recovered.pin_root(target_scope, issuer.public());
        assert_eq!(recovered.consider_cert(&retained_revoked_cert, 0), Err(CertReject::Revoked));
    }

    #[test]
    fn expire_prunes_revocations_after_their_own_horizon() {
        let issuer = Identity::from_seed(&[0xaeu8; 32]);
        let subject = Key32([0xafu8; 32]);
        let scope = Id16([0xb0u8; 16]);
        let mut store = AuthorityStore::new();
        store.pin_root(scope, issuer.public());
        let cert = signed_cert(&issuer, subject, scope, CAP_ANNOUNCE, 10_000);
        store.consider_cert(&cert, 0).unwrap();
        let revoke = RevocationList {
            suite: 1,
            entries: vec![mmp_types::authcert::RevEntry { subject, scope, not_after: 1000 }],
        };
        store.merge_revocation_list(&issuer.public(), &revoke);
        assert!(store.grant_for(&subject, &scope, prio::OPERATIONAL, 1000).is_none());

        store.expire(1000 + NOT_AFTER_SKEW_S + 1);
        assert!(store.grant_for(&subject, &scope, prio::OPERATIONAL, 1000 + NOT_AFTER_SKEW_S + 1).is_some());
    }
}
