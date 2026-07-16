//! Range-Based Set Reconciliation over BundleIDs (doc 06 §3, Negentropy-adapted).
//!
//! `FP(range) = BLAKE3(xor_fold(ids) ‖ count)[0..8]`. Stateless between
//! sessions. Mismatched ranges with ≤ T_LIST elements exchange full ID lists;
//! larger ranges split into B_f count-balanced sub-ranges.
//!
//! Message discipline (implementation of the doc 06 §3.2 alternation):
//! - A received range whose fp matches ours is answered with an ECHO (same
//!   bounds, our fp) carrying the reply flag, so the asker resolves it.
//! - A received echo (reply flag) is never answered — no ping-pong.
//! - A mismatched range is answered with our ID list (small) or our splits.
//! - An ID list for a range is answered with our own list at most once per
//!   range (`listed` set), and always yields WANTs for the IDs we lack.

use alloc::collections::BTreeSet;
use alloc::vec::Vec;

use mmp_types::consts::{RBSR_B_F, RBSR_T_LIST};
use mmp_types::frame::{Frame, Range};
use mmp_types::ids::Id16;

/// High bit of the wire `domain` byte marks a resolution echo.
pub const DOMAIN_REPLY_FLAG: u8 = 0x80;

pub fn fingerprint(ids: &[Id16]) -> u64 {
    let mut fold = [0u8; 16];
    for id in ids {
        for (f, b) in fold.iter_mut().zip(id.0.iter()) {
            *f ^= b;
        }
    }
    let mut h = blake3::Hasher::new();
    h.update(&fold);
    h.update(&(ids.len() as u32).to_le_bytes());
    let out = h.finalize();
    u64::from_le_bytes(out.as_bytes()[..8].try_into().expect("blake3 output ≥ 8 bytes"))
}

/// Byte-wise decrement (saturating at MIN) — used to make exclusive split
/// boundaries inclusive.
fn id_dec(id: &Id16) -> Id16 {
    let mut b = id.0;
    for i in (0..16).rev() {
        if b[i] > 0 {
            b[i] -= 1;
            return Id16(b);
        }
        b[i] = 0xff;
    }
    Id16::MIN
}

/// Split `[lo, hi]` into ≤ `parts` count-balanced sub-ranges of `ids`
/// (ids are our sorted local elements within the range).
fn split_ranges(lo: &Id16, hi: &Id16, ids: &[Id16], parts: usize) -> Vec<(Id16, Id16, Vec<Id16>)> {
    let n = ids.len();
    let mut out = Vec::new();
    let mut start_bound = *lo;
    let mut start_idx = 0usize;
    for p in 1..=parts {
        let end_idx = n * p / parts;
        if end_idx <= start_idx && p != parts {
            continue;
        }
        let end_bound = if p == parts {
            *hi
        } else {
            // Boundary = first element of the next chunk, exclusive → dec.
            id_dec(&ids[end_idx])
        };
        if end_bound < start_bound {
            continue;
        }
        out.push((start_bound, end_bound, ids[start_idx..end_idx].to_vec()));
        if p != parts {
            start_bound = ids[end_idx];
        }
        start_idx = end_idx;
    }
    out
}

/// One domain's reconciliation state on one side of a session.
pub struct Reconciler {
    pub domain: u8,
    /// Ranges we already sent our full ID list for.
    listed: BTreeSet<(Id16, Id16)>,
    /// Unresolved ranges we have in flight (drives domain completion).
    pub open: usize,
    /// IDs the peer has that we lack (drained by the session into WANT).
    pub missing: BTreeSet<Id16>,
}

/// The reconciler asks the host for its sorted IDs per range through this seam.
pub trait IdSource {
    fn ids_in(&mut self, domain: u8, lo: &Id16, hi: &Id16) -> Vec<Id16>;
    fn has(&mut self, id: &Id16) -> bool;
}

impl Reconciler {
    pub fn new(domain: u8) -> Reconciler {
        Reconciler { domain, listed: BTreeSet::new(), open: 0, missing: BTreeSet::new() }
    }

    /// Initiator: open the domain with the full-range fingerprint.
    pub fn init_frame(&mut self, src: &mut impl IdSource) -> Frame {
        let ids = src.ids_in(self.domain, &Id16::MIN, &Id16::MAX);
        self.open = 1;
        Frame::ReconInit {
            domain: self.domain,
            window: 0,
            range: Range { lo: Id16::MIN, hi: Id16::MAX, count: ids.len() as u32, fp: fingerprint(&ids) },
        }
    }

    pub fn is_done(&self) -> bool {
        self.open == 0
    }

    /// Handle a peer range (from RECON_INIT or RECON_RANGE). `is_reply` = the
    /// peer's echo resolving one of OUR ranges.
    pub fn on_range(&mut self, range: &Range, is_reply: bool, src: &mut impl IdSource) -> Vec<Frame> {
        let ids = src.ids_in(self.domain, &range.lo, &range.hi);
        let our_fp = fingerprint(&ids);
        if is_reply {
            // Echo: fp equality resolves; a differing echo shouldn't happen
            // (peers only echo matches) — treat as resolved anyway.
            self.open = self.open.saturating_sub(1);
            return Vec::new();
        }
        if our_fp == range.fp && ids.len() as u32 == range.count {
            // Match → echo so the asker resolves this range.
            return alloc::vec![Frame::ReconRange {
                domain: self.domain | DOMAIN_REPLY_FLAG,
                ranges: alloc::vec![Range { lo: range.lo, hi: range.hi, count: ids.len() as u32, fp: our_fp }],
            }];
        }
        if ids.len() <= RBSR_T_LIST || range.count as usize <= RBSR_T_LIST {
            return self.list_frames(&range.lo, &range.hi, ids);
        }
        // Mismatch, both sides big: reply with OUR count-balanced splits.
        let mut ranges = Vec::with_capacity(RBSR_B_F);
        for (lo, hi, sub) in split_ranges(&range.lo, &range.hi, &ids, RBSR_B_F) {
            ranges.push(Range { lo, hi, count: sub.len() as u32, fp: fingerprint(&sub) });
        }
        self.open += ranges.len();
        alloc::vec![Frame::ReconRange { domain: self.domain, ranges }]
    }

    /// Handle a peer's terminal ID list for `[lo, hi]`.
    pub fn on_ids(&mut self, lo: &Id16, hi: &Id16, ids: &[Id16], src: &mut impl IdSource) -> Vec<Frame> {
        self.open = self.open.saturating_sub(1);
        for id in ids {
            if !src.has(id) {
                self.missing.insert(*id);
            }
        }
        if self.listed.contains(&(*lo, *hi)) {
            return Vec::new();
        }
        let ours = src.ids_in(self.domain, lo, hi);
        self.list_frames(lo, hi, ours)
    }

    fn list_frames(&mut self, lo: &Id16, hi: &Id16, ids: Vec<Id16>) -> Vec<Frame> {
        self.listed.insert((*lo, *hi));
        // A list both offers our IDs and (on the peer) resolves this range.
        alloc::vec![Frame::ReconIds { domain: self.domain, lo: *lo, hi: *hi, ids }]
    }
}
