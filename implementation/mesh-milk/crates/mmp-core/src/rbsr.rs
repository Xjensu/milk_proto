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

/// `FP(range) = BLAKE3(xor_fold(ids) ‖ count)[0..8]` (doc 06 §3): a
/// commutative (order-independent) fingerprint of a set of IDs, used to
/// compare a range against a peer's without exchanging the full ID list.
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
    /// The wire domain byte this reconciler is running RBSR for.
    pub domain: u8,
    /// Ranges we already sent our full ID list for.
    listed: BTreeSet<(Id16, Id16)>,
    /// Canonical descriptors for requests emitted by this side that are
    /// still awaiting a peer response. Keeping descriptors rather than only
    /// a scalar count prevents partitioned responses from leaving a parent
    /// range permanently open and rejects unrelated echoes.
    pending: BTreeSet<(Id16, Id16, u32, u64)>,
    /// Unresolved ranges we have in flight (drives domain completion).
    pub open: usize,
    /// IDs the peer has that we lack (drained by the session into WANT).
    pub missing: BTreeSet<Id16>,
}

/// The reconciler asks the host for its sorted IDs per range through this seam.
pub trait IdSource {
    /// The host's sorted local IDs for `domain` within `[lo, hi]` inclusive.
    fn ids_in(&mut self, domain: u8, lo: &Id16, hi: &Id16) -> Vec<Id16>;
    /// Whether the host already holds `id` (used to turn a peer's ID list
    /// into WANTs for the ones we lack).
    fn has(&mut self, id: &Id16) -> bool;
}

impl Reconciler {
    /// Builds a fresh reconciler for `domain`: nothing listed, no ranges
    /// open, nothing known missing yet.
    pub fn new(domain: u8) -> Reconciler {
        Reconciler {
            domain,
            listed: BTreeSet::new(),
            pending: BTreeSet::new(),
            open: 0,
            missing: BTreeSet::new(),
        }
    }

    /// Initiator: open the domain with the full-range fingerprint.
    pub fn init_frame(&mut self, src: &mut impl IdSource) -> Frame {
        let ids = src.ids_in(self.domain, &Id16::MIN, &Id16::MAX);
        let range = Range {
            lo: Id16::MIN,
            hi: Id16::MAX,
            count: ids.len() as u32,
            fp: fingerprint(&ids),
        };
        self.track(&range);
        Frame::ReconInit {
            domain: self.domain,
            window: 0,
            range,
        }
    }

    /// Whether every range opened for this domain has been resolved.
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
            self.resolve_exact(range);
            return Vec::new();
        }
        self.resolve_containing(&range.lo, &range.hi);
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
        self.track_all(&ranges);
        alloc::vec![Frame::ReconRange { domain: self.domain, ranges }]
    }

    /// Handle a peer's terminal ID list for `[lo, hi]`.
    ///
    /// Pre-existing bug fixed here (found during E5-S1 blob-plane
    /// acceptance testing, not part of that story's own design): the peer
    /// reaching a terminal list only proves THEIR side of `[lo, hi]` is small
    /// (`on_range`'s own size check is an OR over both sides, by design —
    /// either side alone being small ends that side's recursion). It says
    /// nothing about how many IDs WE hold in that same range. Unconditionally
    /// echoing `ours` back here — the previous behavior — could dump
    /// thousands of IDs into a single `Frame::ReconIds` (16 B/ID), silently
    /// exceeding `MAX_FRAME_PAYLOAD`: `debug_assert!` in `encode_payload`/
    /// `write_envelope` catches this in debug builds only, so a release build
    /// would emit a real oversized envelope, and the receiver's
    /// `read_envelope` rejects it outright (`WireError::Oversize`) — an
    /// immediate, silent session kill (`protocol_error`) with NO retry, right
    /// as the largest domain's reconciliation was about to pay off. A file
    /// share's class-4 domain is exactly the case that triggers this: a
    /// fresh holder starts at 0 items, so `on_range` terminates on ITS side
    /// after one hop, and the sender's hundreds of symbol IDs used to be
    /// dumped in one shot right here. Fix: mirror `on_range`'s own
    /// size discipline — split rather than list when OUR side exceeds
    /// `RBSR_T_LIST`, recursing (via the same `split_ranges`/`B_F` machinery)
    /// until every leaf is wire-safe.
    pub fn on_ids(&mut self, lo: &Id16, hi: &Id16, ids: &[Id16], src: &mut impl IdSource) -> Vec<Frame> {
        self.resolve_bounds(lo, hi);
        for id in ids {
            if !src.has(id) {
                self.missing.insert(*id);
            }
        }
        if self.listed.contains(&(*lo, *hi)) {
            return Vec::new();
        }
        let ours = src.ids_in(self.domain, lo, hi);
        if ours.len() > RBSR_T_LIST {
            let mut ranges = Vec::with_capacity(RBSR_B_F);
            for (rlo, rhi, sub) in split_ranges(lo, hi, &ours, RBSR_B_F) {
                ranges.push(Range { lo: rlo, hi: rhi, count: sub.len() as u32, fp: fingerprint(&sub) });
            }
            self.track_all(&ranges);
            return alloc::vec![Frame::ReconRange { domain: self.domain, ranges }];
        }
        self.list_frames(lo, hi, ours)
    }

    fn key(range: &Range) -> (Id16, Id16, u32, u64) {
        (range.lo, range.hi, range.count, range.fp)
    }

    fn refresh_open(&mut self) {
        self.open = self.pending.len();
    }

    fn track(&mut self, range: &Range) {
        self.pending.insert(Self::key(range));
        self.refresh_open();
    }

    fn track_all(&mut self, ranges: &[Range]) {
        for range in ranges {
            self.pending.insert(Self::key(range));
        }
        self.refresh_open();
    }

    fn resolve_exact(&mut self, range: &Range) -> bool {
        let removed = self.pending.remove(&Self::key(range));
        self.refresh_open();
        removed
    }

    fn resolve_bounds(&mut self, lo: &Id16, hi: &Id16) -> bool {
        let key = self.pending.iter().find(|(pending_lo, pending_hi, _, _)| pending_lo == lo && pending_hi == hi).copied();
        let removed = key.is_some_and(|key| self.pending.remove(&key));
        self.refresh_open();
        removed
    }

    fn resolve_containing(&mut self, lo: &Id16, hi: &Id16) -> bool {
        let mut best: Option<(Id16, Id16, u32, u64)> = None;
        for key in &self.pending {
            if key.0 <= *lo && *hi <= key.1 {
                let better = best.is_none_or(|old| key.0 > old.0 || (key.0 == old.0 && key.1 < old.1));
                if better {
                    best = Some(*key);
                }
            }
        }
        let removed = best.is_some_and(|key| self.pending.remove(&key));
        self.refresh_open();
        removed
    }

    fn list_frames(&mut self, lo: &Id16, hi: &Id16, ids: Vec<Id16>) -> Vec<Frame> {
        self.listed.insert((*lo, *hi));
        // A list both offers our IDs and (on the peer) resolves this range.
        alloc::vec![Frame::ReconIds { domain: self.domain, lo: *lo, hi: *hi, ids }]
    }
}
