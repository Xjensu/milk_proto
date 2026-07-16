//! Locally-observed utility (doc 05 §3–§4): PRoPHET-style predictability
//! WITHOUT transitivity, social tie from the local ContactGraph, and the
//! never-exchanged population estimate N̂.
//!
//! Bounded state IS the 10k-survivability answer: P-table ≤ 256 LRU,
//! ContactGraph ≤ 4096. Everything else: U = 0 → pure spray, the provably-OK
//! floor.

use alloc::collections::BTreeMap;
use alloc::collections::BTreeSet;
use alloc::collections::VecDeque;

use mmp_types::consts::{CONTACT_GRAPH_CAP, GAMMA_PER_HOUR, P_INIT, P_TABLE_CAP, SIGMA_TIE};
use mmp_types::ids::NodeId;

const DAY_MS: u64 = 24 * 3600 * 1000;
/// EWMA α = 0.25/day (doc 05 §3).
const EWMA_ALPHA_PER_DAY: f32 = 0.25;
/// Collision discount for unresolved ephem_ids.
const EPHEM_DISCOUNT: f32 = 0.75;

#[derive(Debug, Clone, Copy)]
struct Predict {
    p: f32,
    updated: u64,
    last_used: u64,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ContactStats {
    pub encounters: u32,
    pub total_duration_ms: u64,
    pub last_seen: u64,
    pub verified: bool,
}

pub struct UtilityState {
    p_table: BTreeMap<NodeId, Predict>,
    contacts: BTreeMap<NodeId, ContactStats>,
    /// 24 h sliding windows for N̂.
    resolved_seen: VecDeque<(u64, NodeId)>,
    ephem_seen: VecDeque<(u64, [u8; 8])>,
    n_hat: f32,
    n_hat_updated: u64,
}

impl Default for UtilityState {
    fn default() -> Self {
        Self::new()
    }
}

impl UtilityState {
    pub fn new() -> UtilityState {
        UtilityState {
            p_table: BTreeMap::new(),
            contacts: BTreeMap::new(),
            resolved_seen: VecDeque::new(),
            ephem_seen: VecDeque::new(),
            n_hat: 0.0,
            n_hat_updated: 0,
        }
    }

    /// Record a completed contact with `peer` (doc 05 §4):
    /// `P ← P + (1−P)·P_init`, plus ContactGraph stats and the N̂ window.
    pub fn note_contact(&mut self, peer: &NodeId, duration_ms: u64, now_ms: u64) {
        let p = self.aged_p(peer, now_ms);
        let entry = Predict { p: p + (1.0 - p) * P_INIT, updated: now_ms, last_used: now_ms };
        self.p_table.insert(*peer, entry);
        self.lru_evict_p();

        let stats = self.contacts.entry(*peer).or_default();
        stats.encounters += 1;
        stats.total_duration_ms += duration_ms;
        stats.last_seen = now_ms;
        if self.contacts.len() > CONTACT_GRAPH_CAP {
            // Evict the longest-unseen unverified stranger.
            if let Some(victim) = self
                .contacts
                .iter()
                .filter(|(_, s)| !s.verified)
                .min_by_key(|(_, s)| s.last_seen)
                .map(|(n, _)| *n)
            {
                self.contacts.remove(&victim);
            }
        }
        self.resolved_seen.push_back((now_ms, *peer));
        self.trim_windows(now_ms);
    }

    pub fn note_ephem(&mut self, ephem: [u8; 8], now_ms: u64) {
        self.ephem_seen.push_back((now_ms, ephem));
        self.trim_windows(now_ms);
    }

    pub fn mark_verified(&mut self, peer: &NodeId) {
        self.contacts.entry(*peer).or_default().verified = true;
    }

    pub fn contact(&self, peer: &NodeId) -> Option<&ContactStats> {
        self.contacts.get(peer)
    }

    /// `U(d) = max(P(d), σ·tie(d))` — OUR utility toward `dest` (doc 05 §4).
    pub fn utility(&mut self, dest: &NodeId, now_ms: u64) -> f32 {
        let p = self.aged_p(dest, now_ms);
        if let Some(e) = self.p_table.get_mut(dest) {
            e.p = p;
            e.updated = now_ms;
            e.last_used = now_ms;
        }
        p.max(SIGMA_TIE * self.tie(dest, now_ms))
    }

    /// Social tie ∈ [0,1] from the local ContactGraph only.
    fn tie(&self, dest: &NodeId, now_ms: u64) -> f32 {
        let Some(s) = self.contacts.get(dest) else { return 0.0 };
        // Frequency (saturates at ~16 encounters), recency (fades over a week),
        // and a verified-contact bonus — all locally measured or consented.
        // A verified-but-never-encountered contact (QR bootstrap) has no radio
        // history: frequency and recency are zero, only the bonus remains —
        // otherwise `last_seen == 0` reads as "just seen" and the origin
        // overrates itself as a carrier, refusing every spray hand-off.
        let base = if s.encounters == 0 {
            0.0
        } else {
            let freq = (s.encounters as f32 / 16.0).min(1.0);
            let age_days = now_ms.saturating_sub(s.last_seen) as f32 / DAY_MS as f32;
            let recency = (1.0 - age_days / 7.0).clamp(0.0, 1.0);
            0.5 * freq + 0.5 * recency
        };
        if s.verified {
            (base + 0.25).min(1.0)
        } else {
            base
        }
    }

    /// PRoPHET aging: `P ← P·γ^hours` since last update.
    fn aged_p(&self, peer: &NodeId, now_ms: u64) -> f32 {
        match self.p_table.get(peer) {
            None => 0.0,
            Some(e) => {
                let hours = now_ms.saturating_sub(e.updated) as f32 / 3_600_000.0;
                e.p * libm_powf(GAMMA_PER_HOUR, hours)
            }
        }
    }

    fn lru_evict_p(&mut self) {
        while self.p_table.len() > P_TABLE_CAP {
            if let Some(victim) = self.p_table.iter().min_by_key(|(_, e)| e.last_used).map(|(n, _)| *n)
            {
                self.p_table.remove(&victim);
            }
        }
    }

    /// N̂: distinct resolved peers + 0.75 × distinct unresolved ephem_ids over
    /// 24 h, EWMA-smoothed. NEVER exchanged with peers (poisoning vector).
    pub fn n_hat(&mut self, now_ms: u64) -> f32 {
        self.trim_windows(now_ms);
        let resolved: BTreeSet<&NodeId> = self.resolved_seen.iter().map(|(_, n)| n).collect();
        let ephems: BTreeSet<&[u8; 8]> = self.ephem_seen.iter().map(|(_, e)| e).collect();
        let raw = resolved.len() as f32 + EPHEM_DISCOUNT * ephems.len() as f32;
        let elapsed_days =
            now_ms.saturating_sub(self.n_hat_updated) as f32 / DAY_MS as f32;
        let alpha = (EWMA_ALPHA_PER_DAY * elapsed_days).clamp(0.05, 1.0);
        self.n_hat = if self.n_hat == 0.0 { raw } else { self.n_hat + alpha * (raw - self.n_hat) };
        self.n_hat_updated = now_ms;
        self.n_hat
    }

    fn trim_windows(&mut self, now_ms: u64) {
        let horizon = now_ms.saturating_sub(DAY_MS);
        while self.resolved_seen.front().is_some_and(|(t, _)| *t < horizon) {
            self.resolved_seen.pop_front();
        }
        while self.ephem_seen.front().is_some_and(|(t, _)| *t < horizon) {
            self.ephem_seen.pop_front();
        }
    }
}

/// `powf` without std (no_std float pow via exp2/log2 boils down to repeated
/// squaring for our small exponents; hours are small and γ ∈ (0,1)).
fn libm_powf(base: f32, exp: f32) -> f32 {
    if exp <= 0.0 {
        return 1.0;
    }
    // γ^h = e^(h·ln γ); a 6-term series is plenty at our precision needs.
    let ln_gamma = ln_f32(base);
    exp_f32(exp * ln_gamma)
}

fn ln_f32(x: f32) -> f32 {
    // ln via atanh series around 1 — accurate for γ ≈ 0.98 and general x > 0.
    let mut v = x;
    let mut k = 0i32;
    while v > 1.5 {
        v /= 2.0;
        k += 1;
    }
    while v < 0.75 {
        v *= 2.0;
        k -= 1;
    }
    let t = (v - 1.0) / (v + 1.0);
    let t2 = t * t;
    let series = t * (1.0 + t2 * (1.0 / 3.0 + t2 * (1.0 / 5.0 + t2 / 7.0)));
    2.0 * series + k as f32 * core::f32::consts::LN_2
}

fn exp_f32(x: f32) -> f32 {
    // Range-reduce to [-ln2/2, ln2/2] then a 6-term Taylor series.
    let k = (x / core::f32::consts::LN_2 + if x >= 0.0 { 0.5 } else { -0.5 }) as i32;
    let r = x - k as f32 * core::f32::consts::LN_2;
    let mut term = 1.0f32;
    let mut sum = 1.0f32;
    for i in 1..7 {
        term *= r / i as f32;
        sum += term;
    }
    // 2^k scaling.
    let mut result = sum;
    if k > 0 {
        for _ in 0..k {
            result *= 2.0;
        }
    } else {
        for _ in 0..(-k) {
            result /= 2.0;
        }
    }
    result
}
