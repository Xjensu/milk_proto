//! SeenIndex (doc 03 §4.2, AUDIT F1): resurrection-resistant dedup.
//!
//! Entries persist for the bundle's CLASS-MAXIMUM lifetime measured FROM FIRST
//! SIGHT — never from the (relay-mutable) current lifetime. Time-bucketed
//! Blooms cover the 72 h horizon; an exact ring of the newest IDs keeps false
//! positives off the hot path for recent traffic (doc 04 §8).

use alloc::collections::{BTreeSet, VecDeque};
use alloc::vec::Vec;

use mmp_types::consts::{class_max_lifetime_ms, BLOOM_CAPACITY, FILTER_BUCKETS, FILTER_BUCKET_MS};
use mmp_types::ids::Id16;

use crate::bloom::{Bloom, INTERNAL_SALT, MIN_ENCODED_BYTES};

const EXACT_RING: usize = 1024;

/// `Clone` so `mmp-node`/`mmp-store` can stage a snapshot for persistence
/// (E4-S4, handbook §4.3 step 3) without borrowing the live index.
#[derive(Clone)]
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
    /// Builds an empty index: no buckets, no recent-ring entries.
    pub fn new() -> SeenIndex {
        SeenIndex {
            buckets: VecDeque::new(),
            recent: VecDeque::new(),
            recent_set: BTreeSet::new(),
        }
    }

    /// Record first sight of `id` at monotonic `now_ms` for `class`.
    pub fn insert(&mut self, id: &Id16, class: u8, now_ms: u64) {
        let expiry = now_ms.saturating_add(class_max_lifetime_ms(class));
        let epoch = expiry / FILTER_BUCKET_MS;
        let bloom = match self.buckets.iter_mut().find(|(e, _)| *e == epoch) {
            Some((_, b)) => b,
            None => {
                let pos = self.buckets.partition_point(|(e, _)| *e < epoch);
                self.buckets
                    .insert(pos, (epoch, Bloom::with_capacity(BLOOM_CAPACITY)));
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

    /// Whether `id` has already been seen: an exact hit in the recent ring,
    /// or a (possibly false-positive) hit in any time-bucketed Bloom.
    pub fn contains(&self, id: &Id16) -> bool {
        self.recent_set.contains(id)
            || self
                .buckets
                .iter()
                .any(|(_, b)| b.contains(id, &INTERNAL_SALT))
    }

    /// Drop buckets whose entire expiry window has passed. Monotonic clock only.
    pub fn rotate(&mut self, now_ms: u64) {
        while let Some((epoch, _)) = self.buckets.front() {
            if epoch.saturating_add(1).saturating_mul(FILTER_BUCKET_MS) <= now_ms {
                self.buckets.pop_front();
            } else {
                break;
            }
        }
    }

    /// Persistence wire form (E4-S4, handbook §4.3 step 3): `bucket_count:u32
    /// LE ‖ buckets× (epoch:u64 LE, Bloom::encode()) ‖ recent_count:u32 LE ‖
    /// recent× Id16`. `recent_set` is not persisted — it's rebuilt from
    /// `recent` on decode, since it's purely a derived fast-lookup index.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(self.buckets.len() as u32).to_le_bytes());
        for (epoch, bloom) in &self.buckets {
            out.extend_from_slice(&epoch.to_le_bytes());
            out.extend_from_slice(&bloom.encode());
        }
        out.extend_from_slice(&(self.recent.len() as u32).to_le_bytes());
        for id in &self.recent {
            out.extend_from_slice(&id.0);
        }
        out
    }

    /// Strict decode: `None` on any truncation or trailing bytes — a
    /// malformed persisted index is dropped exactly like any other corrupt
    /// disk record (mmp-store never trusts it, never panics on it).
    pub fn decode(bytes: &[u8]) -> Option<SeenIndex> {
        let mut pos = 0usize;
        let count_end = pos.checked_add(4)?;
        let bucket_count = u32::from_le_bytes(bytes.get(pos..count_end)?.try_into().ok()?) as usize;
        pos = count_end;
        // A valid index rotates at most FILTER_BUCKETS windows.  Also reject
        // impossible counts before reserving attacker-controlled capacity:
        // each bucket needs an epoch plus the minimum encoded Bloom.
        const MIN_BUCKET_BYTES: usize = 8 + MIN_ENCODED_BYTES;
        if bucket_count > FILTER_BUCKETS
            || bucket_count > bytes.len().saturating_sub(pos) / MIN_BUCKET_BYTES
        {
            return None;
        }
        let mut buckets = VecDeque::with_capacity(bucket_count);
        for _ in 0..bucket_count {
            let epoch = u64::from_le_bytes(bytes.get(pos..pos + 8)?.try_into().ok()?);
            pos += 8;
            let (bloom, consumed) = Bloom::decode_prefix(bytes.get(pos..)?)?;
            pos += consumed;
            buckets.push_back((epoch, bloom));
        }
        let count_end = pos.checked_add(4)?;
        let recent_count = u32::from_le_bytes(bytes.get(pos..count_end)?.try_into().ok()?) as usize;
        pos = count_end;
        if recent_count > EXACT_RING {
            return None; // more entries than the ring can ever legitimately hold
        }
        if recent_count > bytes.len().saturating_sub(pos) / 16 {
            return None;
        }
        let mut recent = VecDeque::with_capacity(recent_count);
        let mut recent_set = BTreeSet::new();
        for _ in 0..recent_count {
            let mut a = [0u8; 16];
            a.copy_from_slice(bytes.get(pos..pos + 16)?);
            pos += 16;
            let id = Id16(a);
            recent.push_back(id);
            recent_set.insert(id);
        }
        if pos != bytes.len() {
            return None; // trailing bytes
        }
        Some(SeenIndex {
            buckets,
            recent,
            recent_set,
        })
    }
}
