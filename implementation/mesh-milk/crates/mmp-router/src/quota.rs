//! Per-origin token buckets (doc 05 §8): 32 KiB/h, burst 128 KiB, WoT ×4.
//! Sybil cost: each new key starts at default; socially-close traffic wins
//! under contention, strangers still pass at base rate.

use alloc::collections::BTreeMap;

use mmp_types::consts::{ORIGIN_BUCKET_BURST, ORIGIN_BUCKET_RATE_PER_H, WOT_MULTIPLIER};
use mmp_types::ids::NodeId;

/// Maximum number of live origin buckets retained by one node. A hostile
/// peer can mint fresh pseudonymous origins, so the map itself must have a
/// hard bound in addition to the per-origin byte rate. Once full, unknown
/// origins fail closed while existing buckets continue to refill normally.
const ORIGIN_BUCKET_CAP: usize = 4096;

struct Bucket {
    /// Milli-bytes to keep refill integer-exact at ms granularity.
    level: u64,
    last_refill: u64,
    wot: bool,
}

/// One token bucket per origin, keyed by the origin's `NodeId` (doc 05 §8).
/// Bounded by `compact` so state stays flat regardless of how many distinct
/// origins are ever seen.
#[derive(Default)]
pub struct OriginBuckets {
    map: BTreeMap<NodeId, Bucket>,
}

impl OriginBuckets {
    /// Empty bucket set — no origin has been seen yet.
    pub fn new() -> OriginBuckets {
        OriginBuckets::default()
    }

    /// Mark `origin` as web-of-trust (or not), applying the `WOT_MULTIPLIER`
    /// to its rate/burst from the next `admit` call on. No-op if `origin`
    /// has no bucket yet (first `admit` call establishes one).
    pub fn set_wot(&mut self, origin: &NodeId, wot: bool) {
        if let Some(b) = self.map.get_mut(origin) {
            b.wot = wot;
        }
    }

    /// Try to admit `size` bytes from `origin` at monotonic `now_ms`.
    pub fn admit(&mut self, origin: &NodeId, size: u32, now_ms: u64, wot: bool) -> bool {
        if !self.map.contains_key(origin) && self.map.len() >= ORIGIN_BUCKET_CAP {
            return false;
        }
        let initial_mult = if wot { WOT_MULTIPLIER } else { 1 };
        let bucket = self.map.entry(*origin).or_insert(Bucket {
            level: ORIGIN_BUCKET_BURST * 1000 * initial_mult,
            last_refill: now_ms,
            wot,
        });
        bucket.wot |= wot;
        let mult = if bucket.wot { WOT_MULTIPLIER } else { 1 };
        let burst = ORIGIN_BUCKET_BURST * 1000 * mult;
        // Refill: rate/h → milli-bytes per ms.
        let elapsed = now_ms.saturating_sub(bucket.last_refill);
        let refill = elapsed
            .saturating_mul(ORIGIN_BUCKET_RATE_PER_H)
            .saturating_mul(mult)
            / 3600;
        bucket.level = bucket.level.saturating_add(refill).min(burst);
        bucket.last_refill = now_ms;

        let cost = u64::from(size) * 1000;
        if bucket.level >= cost {
            bucket.level -= cost;
            true
        } else {
            false
        }
    }

    /// Drop idle buckets (bounded state at 10k scale).
    pub fn compact(&mut self, now_ms: u64, idle_ms: u64) {
        self.map
            .retain(|_, b| now_ms.saturating_sub(b.last_refill) < idle_ms);
    }

    /// Number of distinct origins currently tracked.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// True if no origin has a live bucket (nothing seen, or all compacted).
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmp_types::ids::Id16;

    #[test]
    fn fresh_origins_are_rejected_after_capacity_is_reached() {
        let mut b = OriginBuckets::new();
        for i in 0..ORIGIN_BUCKET_CAP {
            let mut bytes = [0u8; 16];
            bytes[..8].copy_from_slice(&(i as u64).to_le_bytes());
            assert!(b.admit(&Id16(bytes), 1, 0, false));
        }
        assert_eq!(b.len(), ORIGIN_BUCKET_CAP);

        let fresh = Id16([0xff; 16]);
        assert!(!b.admit(&fresh, 1, 0, false));
        // Existing origins remain serviceable while the map is full.
        let mut first_bytes = [0u8; 16];
        first_bytes[..8].copy_from_slice(&0u64.to_le_bytes());
        assert!(b.admit(&Id16(first_bytes), 1, 0, false));
    }

    #[test]
    fn idle_compaction_releases_origin_buckets() {
        let mut b = OriginBuckets::new();
        let origin = Id16([7; 16]);
        assert!(b.admit(&origin, 1, 0, false));
        assert_eq!(b.len(), 1);
        b.compact(100, 100);
        assert!(b.is_empty());
    }

    #[test]
    fn huge_clock_jump_saturates_refill_without_overflow() {
        let mut b = OriginBuckets::new();
        let origin = Id16([8; 16]);

        assert!(b.admit(&origin, 1, 0, false));
        assert!(b.admit(&origin, ORIGIN_BUCKET_BURST as u32, u64::MAX, false));
        assert!(!b.admit(&origin, 1, u64::MAX, false));
    }
}
