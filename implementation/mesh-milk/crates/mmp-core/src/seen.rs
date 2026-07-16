//! SeenIndex (doc 03 §4.2, AUDIT F1): resurrection-resistant dedup.
//!
//! Entries persist for the bundle's CLASS-MAXIMUM lifetime measured FROM FIRST
//! SIGHT — never from the (relay-mutable) current lifetime. Time-bucketed
//! Blooms cover the 72 h horizon; an exact ring of the newest IDs keeps false
//! positives off the hot path for recent traffic (doc 04 §8).

use alloc::collections::{BTreeSet, VecDeque};

use mmp_types::consts::{class_max_lifetime_ms, BLOOM_CAPACITY, FILTER_BUCKET_MS};
use mmp_types::ids::Id16;

use crate::bloom::{Bloom, INTERNAL_SALT};

const EXACT_RING: usize = 1024;

pub struct SeenIndex {
    /// (bucket_epoch, bloom): bucket e holds IDs whose seen-entry expires in
    /// [e·B, (e+1)·B). Ascending order.
    buckets: VecDeque<(u64, Bloom)>,
    recent: VecDeque<Id16>,
    recent_set: BTreeSet<Id16>,
}

impl Default for SeenIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl SeenIndex {
    pub fn new() -> SeenIndex {
        SeenIndex { buckets: VecDeque::new(), recent: VecDeque::new(), recent_set: BTreeSet::new() }
    }

    /// Record first sight of `id` at monotonic `now_ms` for `class`.
    pub fn insert(&mut self, id: &Id16, class: u8, now_ms: u64) {
        let expiry = now_ms + class_max_lifetime_ms(class);
        let epoch = expiry / FILTER_BUCKET_MS;
        let bloom = match self.buckets.iter_mut().find(|(e, _)| *e == epoch) {
            Some((_, b)) => b,
            None => {
                let pos = self.buckets.partition_point(|(e, _)| *e < epoch);
                self.buckets.insert(pos, (epoch, Bloom::with_capacity(BLOOM_CAPACITY)));
                &mut self.buckets[pos].1
            }
        };
        bloom.insert(id, &INTERNAL_SALT);

        if self.recent.len() >= EXACT_RING {
            if let Some(old) = self.recent.pop_front() {
                self.recent_set.remove(&old);
            }
        }
        self.recent.push_back(*id);
        self.recent_set.insert(*id);
    }

    pub fn contains(&self, id: &Id16) -> bool {
        self.recent_set.contains(id)
            || self.buckets.iter().any(|(_, b)| b.contains(id, &INTERNAL_SALT))
    }

    /// Drop buckets whose entire expiry window has passed. Monotonic clock only.
    pub fn rotate(&mut self, now_ms: u64) {
        while let Some((epoch, _)) = self.buckets.front() {
            if (epoch + 1) * FILTER_BUCKET_MS <= now_ms {
                self.buckets.pop_front();
            } else {
                break;
            }
        }
    }
}
