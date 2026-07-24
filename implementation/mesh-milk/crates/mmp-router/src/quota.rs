//! Per-origin token buckets (doc 05 §8): 32 KiB/h, burst 128 KiB, WoT ×4.
//! Sybil cost: each new key starts at default; socially-close traffic wins
//! under contention, strangers still pass at base rate.

use alloc::collections::BTreeMap;

use mmp_types::consts::{ORIGIN_BUCKET_BURST, ORIGIN_BUCKET_RATE_PER_H, WOT_MULTIPLIER};
use mmp_types::ids::NodeId;

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
        bucket.level = (bucket.level + elapsed * ORIGIN_BUCKET_RATE_PER_H * mult / 3600).min(burst);
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
        self.map.retain(|_, b| now_ms.saturating_sub(b.last_refill) < idle_ms);
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
