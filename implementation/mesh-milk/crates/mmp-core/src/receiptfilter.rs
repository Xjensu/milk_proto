//! ReceiptFilter (doc 06 §6): rotating Bloom buckets + an exact receipt set.
//!
//! The asymmetry is load-bearing and visible in the types (AUDIT F11):
//! a Bloom hit is a [`SuppressSignal::Hint`] — deprioritize offering, NEVER
//! delete; only an exact, verified D0 receipt is [`SuppressSignal::Purge`].

use alloc::collections::{BTreeMap, VecDeque};
use alloc::vec::Vec;

use mmp_types::consts::{
    BLOOM_CAPACITY, BLOOM_SUMMARY_MAX, FILTER_BUCKETS, FILTER_BUCKET_MS, LIFETIME_MESSAGE_MS,
};
use mmp_types::frame::SummaryBucket;
use mmp_types::ids::Id16;

use crate::bloom::{Bloom, INTERNAL_SALT};

/// Hint deprioritizes offers; Purge is the only deletion authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuppressSignal {
    None,
    Hint,
    Purge,
}

pub struct ReceiptFilter {
    /// (bucket_epoch, bloom), ascending; epoch = receipt-time / 6 h.
    buckets: VecDeque<(u64, Bloom)>,
    /// Exact verified receipts → expiry of relevance (delivered bundle's max life).
    exact: BTreeMap<Id16, u64>,
}

impl Default for ReceiptFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl ReceiptFilter {
    pub fn new() -> ReceiptFilter {
        ReceiptFilter { buckets: VecDeque::new(), exact: BTreeMap::new() }
    }

    /// Fold a VERIFIED receipt (origin-signed class-0, doc 05 §7.2) for
    /// `delivered` observed at `now_ms`.
    pub fn note_verified_receipt(&mut self, delivered: &Id16, now_ms: u64) {
        let epoch = now_ms / FILTER_BUCKET_MS;
        let bloom = match self.buckets.iter_mut().find(|(e, _)| *e == epoch) {
            Some((_, b)) => b,
            None => {
                let pos = self.buckets.partition_point(|(e, _)| *e < epoch);
                self.buckets.insert(pos, (epoch, Bloom::with_capacity(BLOOM_CAPACITY)));
                &mut self.buckets[pos].1
            }
        };
        bloom.insert(delivered, &INTERNAL_SALT);
        self.exact.insert(*delivered, now_ms + LIFETIME_MESSAGE_MS);
    }

    /// OR a peer's bucket into ours — caller MUST have authenticated the
    /// session (doc 06 §6.3). Peer epochs are wire-u16 (mod 2¹⁶ of ours).
    pub fn union_from_peer(&mut self, peer_epoch: u16, bloom_bytes: &[u8], now_ms: u64) {
        let base = now_ms / FILTER_BUCKET_MS;
        // Resolve the u16 wire epoch against our recent windows.
        let Some(epoch) = (0..FILTER_BUCKETS as u64)
            .map(|back| base.saturating_sub(back))
            .find(|e| (*e & 0xffff) as u16 == peer_epoch)
        else {
            return; // too old / clock-skewed: ignore, it's only a hint
        };
        let bloom = match self.buckets.iter_mut().find(|(e, _)| *e == epoch) {
            Some((_, b)) => b,
            None => {
                let pos = self.buckets.partition_point(|(e, _)| *e < epoch);
                self.buckets.insert(pos, (epoch, Bloom::with_capacity(BLOOM_CAPACITY)));
                &mut self.buckets[pos].1
            }
        };
        bloom.union(bloom_bytes);
    }

    /// What may we do about a stored bundle `id`?
    pub fn signal(&self, id: &Id16) -> SuppressSignal {
        if self.exact.contains_key(id) {
            SuppressSignal::Purge
        } else if self.buckets.iter().any(|(_, b)| b.contains(id, &INTERNAL_SALT)) {
            SuppressSignal::Hint
        } else {
            SuppressSignal::None
        }
    }

    /// Newest `n` buckets for SUMMARY, folded to the wire mini-filter size
    /// (doc 06 §6.3 newest-2 default; doc 06 §9 mini-filter mandate).
    pub fn summary_buckets(&self, n: usize) -> Vec<SummaryBucket> {
        self.buckets
            .iter()
            .rev()
            .take(n)
            .map(|(e, b)| SummaryBucket {
                bucket_epoch: (*e & 0xffff) as u16,
                bloom: b.folded(BLOOM_SUMMARY_MAX),
            })
            .collect()
    }

    /// Age out buckets past the retention horizon and expired exact entries.
    pub fn rotate(&mut self, now_ms: u64) {
        let horizon = (now_ms / FILTER_BUCKET_MS).saturating_sub(FILTER_BUCKETS as u64);
        while let Some((epoch, _)) = self.buckets.front() {
            if *epoch < horizon {
                self.buckets.pop_front();
            } else {
                break;
            }
        }
        self.exact.retain(|_, expiry| *expiry > now_ms);
    }
}
