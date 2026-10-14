//! Per-authority token bucket (doc 18 §5.3) — meters elevated-priority
//! admissions per AuthCert.subject, independent of relay path. Structural
//! twin of `mmp-router::quota::OriginBuckets` (per-origin, milli-unit
//! integer refill) but on the node-policy plane: this counts *admission
//! events*, not relay bytes, and the rate comes from a cert the node holds
//! rather than a fixed protocol constant.
//!
//! **Deliberately RAM-only — not persisted.** The doc 08 §5.3 amendment
//! persisted ratchet state because its loss destroys availability (in-flight
//! messages) and the state is key material needing keystore wrapping. Bucket
//! state is the opposite on both axes: no secrets (a snapshot reveals only
//! that some announcements were admitted), and its loss costs only bounded
//! over-admission — a restart re-grants at most one full burst per subject,
//! the same "accounting, not prevention" residual doc 18 §5.4(b) already
//! treats as first-class (a colluding receiver can zero its own bucket at
//! will; a restarting honest one briefly resembles that). Matches
//! `OriginBuckets`, which has also never persisted.

use std::collections::BTreeMap;

use mmp_types::ids::Key32;

/// Maximum number of live authority-subject buckets retained by one node.
/// Subject keys are attacker-controlled once an authority has issued grants,
/// so the metering map needs a hard bound independent of bundle/storage
/// quotas. New subjects fail closed when the bound is reached.
const AUTHORITY_BUCKET_CAP: usize = 4096;

struct Bucket {
    /// Milli-admissions, to keep refill integer-exact at ms granularity.
    level_milli: u64,
    last_refill: u64,
    /// The cert's `priority_budget` this bucket was last sized against
    /// (admissions/hour). A renewal at a different rate adopts the new rate
    /// but does not refill beyond it — conservative under the doc 18 §5.4(c)
    /// per-cert bound (capacity B ≤ β).
    rate_per_h: u16,
}

/// Per-subject token buckets metering elevated-priority announcement
/// admissions (doc 18 §5.3) — one [`Bucket`] per AuthCert `subject`, created
/// lazily and refilled on every [`AuthorityBuckets::admit`] call.
#[derive(Default)]
pub struct AuthorityBuckets {
    map: BTreeMap<Key32, Bucket>,
}

impl AuthorityBuckets {
    /// An empty bucket map — every subject starts unseen and gets its bucket
    /// on first [`AuthorityBuckets::admit`] call.
    pub fn new() -> AuthorityBuckets {
        AuthorityBuckets::default()
    }

    /// Try to admit one elevated-priority announcement for `subject` under a
    /// cert granting `budget_per_h` (β admissions/hour). New subjects start
    /// full (capacity B = β·1000 milli). No dedup logic here: the caller
    /// (`handle_announcement`) only reaches this once per distinct BundleID —
    /// `ingest`'s SeenIndex already turned replays into `Disposition::Duplicate`
    /// before dispatch, which is exactly the doc 18 §5.4 proof's step (2).
    pub fn admit(&mut self, subject: &Key32, budget_per_h: u16, now_ms: u64) -> bool {
        if !self.map.contains_key(subject) && self.map.len() >= AUTHORITY_BUCKET_CAP {
            return false;
        }
        let capacity = u64::from(budget_per_h) * 1000;
        let bucket = self.map.entry(*subject).or_insert(Bucket {
            level_milli: capacity,
            last_refill: now_ms,
            rate_per_h: budget_per_h,
        });
        if bucket.rate_per_h != budget_per_h {
            bucket.rate_per_h = budget_per_h;
            bucket.level_milli = bucket.level_milli.min(capacity);
        }
        // Refill: rate/h (admissions) → milli-admissions per ms, same formula
        // as OriginBuckets' bytes/h → milli-bytes (mmp-router/src/quota.rs).
        let elapsed = now_ms.saturating_sub(bucket.last_refill);
        let refill = elapsed.saturating_mul(u64::from(bucket.rate_per_h)) / 3600;
        bucket.level_milli = bucket.level_milli.saturating_add(refill).min(capacity);
        bucket.last_refill = now_ms;
        if bucket.level_milli >= 1000 {
            bucket.level_milli -= 1000;
            true
        } else {
            false
        }
    }

    /// Drop idle buckets (bounded state — same discipline as `OriginBuckets`).
    pub fn compact(&mut self, now_ms: u64, idle_ms: u64) {
        self.map
            .retain(|_, b| now_ms.saturating_sub(b.last_refill) < idle_ms);
    }

    /// Number of subjects with a live (not yet compacted) bucket.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// True iff no subject currently has a bucket.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admits_up_to_budget_then_refills_after_an_hour() {
        let mut b = AuthorityBuckets::new();
        let subject = Key32([1; 32]);
        assert!(b.admit(&subject, 2, 0));
        assert!(b.admit(&subject, 2, 0));
        assert!(
            !b.admit(&subject, 2, 0),
            "third admission within the same instant must be refused"
        );
        assert!(
            b.admit(&subject, 2, 3_600_000),
            "an hour later, refilled to budget"
        );
    }

    #[test]
    fn distinct_subjects_have_independent_buckets() {
        let mut b = AuthorityBuckets::new();
        let s1 = Key32([1; 32]);
        let s2 = Key32([2; 32]);
        assert!(b.admit(&s1, 1, 0));
        assert!(!b.admit(&s1, 1, 0));
        assert!(
            b.admit(&s2, 1, 0),
            "s2's bucket is unaffected by s1 exhausting theirs"
        );
    }

    #[test]
    fn fresh_subjects_are_rejected_after_capacity_is_reached() {
        let mut b = AuthorityBuckets::new();
        for i in 0..AUTHORITY_BUCKET_CAP {
            let mut bytes = [0u8; 32];
            bytes[..8].copy_from_slice(&(i as u64).to_le_bytes());
            assert!(b.admit(&Key32(bytes), 1, 0));
        }
        assert_eq!(b.len(), AUTHORITY_BUCKET_CAP);
        assert!(!b.admit(&Key32([0xff; 32]), 1, 0));
    }

    #[test]
    fn idle_compaction_releases_authority_buckets() {
        let mut b = AuthorityBuckets::new();
        let subject = Key32([7; 32]);
        assert!(b.admit(&subject, 1, 0));
        assert_eq!(b.len(), 1);
        b.compact(100, 100);
        assert!(b.is_empty());
    }

    #[test]
    fn huge_clock_jump_saturates_refill_without_overflow() {
        let mut b = AuthorityBuckets::new();
        let subject = Key32([8; 32]);

        assert!(b.admit(&subject, 1, 0));
        assert!(b.admit(&subject, 1, u64::MAX));
        assert!(!b.admit(&subject, 1, u64::MAX));
    }
}
