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

use crate::bloom::{Bloom, INTERNAL_SALT, MIN_ENCODED_BYTES};

// The implementation handbook's Standard-tier RAM table budgets the exact
// receipt map at 8,192 entries (~448 KiB with BTreeMap node overhead).  Keep
// that local resource bound explicit here: receipt admission is authenticated,
// but a peer can still present many valid receipts over time.  Exceeding the
// bound may delay an optional purge (the Bloom remains only a hint), never
// authorize an unsafe deletion.
pub(crate) const EXACT_RECEIPT_CAP: usize = BLOOM_CAPACITY;

/// Hint deprioritizes offers; Purge is the only deletion authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuppressSignal {
    /// No evidence the bundle was delivered — offer it normally.
    None,
    /// A Bloom hit: possibly-false-positive evidence of delivery —
    /// deprioritize offering it, but never delete it on this alone.
    Hint,
    /// An exact, verified D0 receipt was seen for this bundle — the only
    /// signal authorized to actually delete it.
    Purge,
}

/// `Clone` so `mmp-node`/`mmp-store` can stage a snapshot for persistence
/// (E4-S4, handbook §4.3 step 3) without borrowing the live filter.
#[derive(Clone)]
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
    /// Builds an empty filter: no buckets, no exact receipts.
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

        // Do not let the exact-purge index grow without bound.  Replacing an
        // existing receipt refreshes its expiry and does not consume a slot.
        // When the local cap is full, discard the entry with the earliest
        // expiry (Id16 breaks equal-expiry ties deterministically).  Losing an
        // exact entry can only retain a bundle longer: the Bloom hint is still
        // present, and no Bloom path is allowed to purge.
        if !self.exact.contains_key(delivered) && self.exact.len() >= EXACT_RECEIPT_CAP {
            if let Some((oldest, _)) = self
                .exact
                .iter()
                .min_by_key(|(id, expiry)| (**expiry, **id))
                .map(|(id, expiry)| (*id, *expiry))
            {
                let _ = self.exact.remove(&oldest);
            }
        }
        self.exact.insert(*delivered, now_ms.saturating_add(LIFETIME_MESSAGE_MS));
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

    /// Persistence wire form (E4-S4, handbook §4.3 step 3): `bucket_count:u32
    /// LE ‖ buckets× (epoch:u64 LE, Bloom::encode()) ‖ exact_count:u32 LE ‖
    /// exact× (Id16, expiry:u64 LE)`.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(self.buckets.len() as u32).to_le_bytes());
        for (epoch, bloom) in &self.buckets {
            out.extend_from_slice(&epoch.to_le_bytes());
            out.extend_from_slice(&bloom.encode());
        }
        out.extend_from_slice(&(self.exact.len() as u32).to_le_bytes());
        for (id, expiry) in &self.exact {
            out.extend_from_slice(&id.0);
            out.extend_from_slice(&expiry.to_le_bytes());
        }
        out
    }

    /// Strict decode: `None` on any truncation or trailing bytes — dropped
    /// exactly like any other corrupt disk record, never panics on it.
    pub fn decode(bytes: &[u8]) -> Option<ReceiptFilter> {
        let mut pos = 0usize;
        let count_end = pos.checked_add(4)?;
        let bucket_count = u32::from_le_bytes(bytes.get(pos..count_end)?.try_into().ok()?) as usize;
        pos = count_end;
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
        let exact_count = u32::from_le_bytes(bytes.get(pos..count_end)?.try_into().ok()?) as usize;
        pos = count_end;
        // Each exact entry is a 16-byte ID plus an 8-byte expiry.  Check the
        // count against the remaining bytes before entering the loop so a
        // hostile snapshot cannot force an unbounded parse.
        if exact_count > EXACT_RECEIPT_CAP
            || exact_count > bytes.len().saturating_sub(pos) / 24
        {
            return None;
        }
        let mut exact = BTreeMap::new();
        for _ in 0..exact_count {
            let mut a = [0u8; 16];
            a.copy_from_slice(bytes.get(pos..pos + 16)?);
            pos += 16;
            let expiry = u64::from_le_bytes(bytes.get(pos..pos + 8)?.try_into().ok()?);
            pos += 8;
            exact.insert(Id16(a), expiry);
        }
        if pos != bytes.len() {
            return None; // trailing bytes
        }
        Some(ReceiptFilter { buckets, exact })
    }
}
