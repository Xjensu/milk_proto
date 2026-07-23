//! Fixed-size Bloom filter over `Id16` with BLAKE3-derived indices
//! (doc 06 §6.1: k = 7, ~10 bits/element for ≈1 % FP at capacity).

use alloc::vec;
use alloc::vec::Vec;

use mmp_types::consts::{BLOOM_BITS_PER_ELEMENT, BLOOM_K};
use mmp_types::ids::Id16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bloom {
    bits: Vec<u8>,
    m: u32,
    count: u32,
}

impl Bloom {
    pub fn with_capacity(capacity: usize) -> Bloom {
        let m_bits = (capacity * BLOOM_BITS_PER_ELEMENT).max(64);
        Bloom { bits: vec![0; m_bits.div_ceil(8)], m: m_bits as u32, count: 0 }
    }

    /// `salt` keys the index derivation — beacon fragments re-salt per ephem
    /// epoch (AUDIT F8a); internal filters pass a fixed salt.
    fn indices(&self, id: &Id16, salt: &[u8; 32]) -> [u32; BLOOM_K] {
        let hash = blake3::keyed_hash(salt, &id.0);
        let h = hash.as_bytes();
        let mut idx = [0u32; BLOOM_K];
        for (i, slot) in idx.iter_mut().enumerate() {
            let o = i * 4;
            *slot = u32::from_le_bytes([h[o], h[o + 1], h[o + 2], h[o + 3]]) % self.m;
        }
        idx
    }

    pub fn insert(&mut self, id: &Id16, salt: &[u8; 32]) {
        for i in self.indices(id, salt) {
            self.bits[(i / 8) as usize] |= 1 << (i % 8);
        }
        self.count += 1;
    }

    pub fn contains(&self, id: &Id16, salt: &[u8; 32]) -> bool {
        self.indices(id, salt)
            .iter()
            .all(|&i| self.bits[(i / 8) as usize] & (1 << (i % 8)) != 0)
    }

    /// Lossy OR-fold to at most `max_bytes` by repeated halving — the wire
    /// mini-filter (doc 06 §9). Because each halving keeps `m′ | m`, a query
    /// index `h mod m` reduces to `h mod m′`: folding can never lose a member,
    /// it only raises the false-positive rate — safe for a Hint (AUDIT F11).
    pub fn folded(&self, max_bytes: usize) -> Vec<u8> {
        let mut out = self.bits.clone();
        while out.len() > max_bytes && out.len().is_multiple_of(2) {
            let half = out.len() / 2;
            for i in 0..half {
                out[i] |= out[i + half];
            }
            out.truncate(half);
        }
        out
    }

    /// OR a peer's same-window bucket into ours (doc 06 §6.3). A folded
    /// mini-filter (length dividing ours) is tiled across the full array,
    /// which preserves every peer positive under our larger modulus; any
    /// other dimension is silently ignored (old/foreign peer).
    pub fn union(&mut self, other_bits: &[u8]) {
        let n = other_bits.len();
        if n == 0 || !self.bits.len().is_multiple_of(n) {
            return;
        }
        for (i, a) in self.bits.iter_mut().enumerate() {
            *a |= other_bits[i % n];
        }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bits
    }

    pub fn count(&self) -> u32 {
        self.count
    }

    /// Persistence wire form (E4-S4 SeenIndex/ReceiptFilter persistence):
    /// `m:u32 LE ‖ count:u32 LE ‖ bits (m.div_ceil(8) bytes)` — `bits.len()`
    /// is derivable from `m`, so it isn't stored twice.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(8 + self.bits.len());
        out.extend_from_slice(&self.m.to_le_bytes());
        out.extend_from_slice(&self.count.to_le_bytes());
        out.extend_from_slice(&self.bits);
        out
    }

    /// Strict decode: `None` on truncation or trailing bytes — a malformed
    /// persisted filter is exactly as untrusted as a corrupt disk record
    /// (mmp-store drops it, never panics on it).
    pub fn decode(bytes: &[u8]) -> Option<Bloom> {
        let (bloom, consumed) = Self::decode_prefix(bytes)?;
        if consumed != bytes.len() {
            return None; // trailing bytes: not a lone encoded Bloom
        }
        Some(bloom)
    }

    /// Like [`Bloom::decode`], but for embedding a `Bloom` inside a larger
    /// stream (`SeenIndex`/`ReceiptFilter` persistence, both of which hold a
    /// list of `(epoch, Bloom)` buckets): returns how many bytes were
    /// consumed so the caller can keep reading past it, rather than
    /// requiring the whole remaining buffer to be exactly one `Bloom`.
    pub fn decode_prefix(bytes: &[u8]) -> Option<(Bloom, usize)> {
        let m = u32::from_le_bytes(bytes.get(0..4)?.try_into().ok()?);
        let count = u32::from_le_bytes(bytes.get(4..8)?.try_into().ok()?);
        let bits_len = (m as usize).div_ceil(8);
        let bits = bytes.get(8..8 + bits_len)?;
        Some((Bloom { bits: bits.to_vec(), m, count }, 8 + bits_len))
    }
}

/// Fixed salt for filters that are never broadcast (SeenIndex, ReceiptFilter
/// internals) — salting there defends nothing, so a constant keeps them cheap.
pub const INTERNAL_SALT: [u8; 32] = *b"mmp1 internal bloom salt v1\0\0\0\0\0";
