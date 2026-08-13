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

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;

use mmp_types::consts::{RBSR_B_F, RBSR_T_LIST};
use mmp_types::frame::{Frame, Range};
use mmp_types::ids::Id16;

/// High bit of the wire `domain` byte marks a resolution echo.
pub const DOMAIN_REPLY_FLAG: u8 = 0x80;

/// Bound the per-parent interval ledger used to prevent a single nested range
/// from prematurely resolving a pending parent. Honest binary partitions use
/// at most `RBSR_B_F` children; excess untrusted fragments are ignored until
/// the session times out rather than growing memory without limit.
const MAX_COVERAGE_INTERVALS_PER_PARENT: usize = RBSR_B_F * 4;
/// Bound the per-session range ledger.  A peer controls the `(count, fp)`
/// fields of every descriptor, so using the wire descriptor as a map key
/// without a cap would let an authenticated peer grow `pending` for the whole
/// session.  The cap is deliberately above the leaf count needed for the
/// supported 10k-node/BundleID-scale inventory (`10_000 / RBSR_T_LIST < 512`)
/// while keeping one reconciler's metadata bounded.
const MAX_PENDING_RANGES: usize = 1024;
/// Bound the set of ranges already answered with a terminal ID list.  The
/// descriptor bounds are attacker-controlled and the set otherwise survives
/// for the complete contact.
const MAX_LISTED_RANGES: usize = 4096;
/// Bound IDs collected from untrusted RECON_IDS frames until the session drains
/// them into WANT frames.  This is a defensive backstop for a host that is
/// temporarily unable to emit those frames.
const MAX_MISSING_IDS: usize = 4096;

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
    u64::from_le_bytes(
        out.as_bytes()[..8]
            .try_into()
            .expect("blake3 output ≥ 8 bytes"),
    )
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

/// Byte-wise increment (saturating at MAX), used to check inclusive interval
/// coverage without converting identifiers to an integer type.
fn id_inc(id: &Id16) -> Id16 {
    let mut b = id.0;
    for i in (0..16).rev() {
        if b[i] < 0xff {
            b[i] += 1;
            return Id16(b);
        }
        b[i] = 0;
    }
    Id16::MAX
}

/// Whether sorted inclusive intervals cover an entire parent span. Intervals
/// are stored in a `BTreeSet`, so the scan is deterministic and naturally
/// coalesces overlap while rejecting gaps.
fn interval_set_covers(parent: (Id16, Id16), intervals: &BTreeSet<(Id16, Id16)>) -> bool {
    let (parent_lo, parent_hi) = parent;
    let mut cursor = parent_lo;
    for (lo, hi) in intervals {
        if *hi < cursor {
            continue;
        }
        if *lo > cursor {
            return false;
        }
        if *hi >= parent_hi {
            return true;
        }
        cursor = id_inc(hi);
    }
    cursor > parent_hi
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

type RangeKey = (Id16, Id16, u32, u64);

struct PendingState {
    /// Child intervals observed for this outstanding range.
    coverage: BTreeSet<(Id16, Id16)>,
    /// A terminal `RECON_IDS` response was emitted for this range and the
    /// peer must still answer with its corresponding list or split ranges.
    await_terminal: bool,
}

/// One domain's reconciliation state on one side of a session.
pub struct Reconciler {
    /// The wire domain byte this reconciler is running RBSR for.
    pub domain: u8,
    /// Ranges we already sent our full ID list for.
    listed: BTreeSet<(Id16, Id16)>,
    /// Canonical descriptors for requests emitted by this side that are
    /// still awaiting a peer response. Each descriptor carries bounded
    /// partition coverage and terminal-response state, preventing a single
    /// nested range from resolving a parent prematurely.
    pending: BTreeMap<RangeKey, PendingState>,
    /// Unresolved ranges we have in flight (drives domain completion).
    pub open: usize,
    /// IDs the peer has that we lack (drained by the session into WANT).
    pub missing: BTreeSet<Id16>,
    /// Set when an untrusted range/list would exceed one of the bounded
    /// ledgers.  The session treats this as a protocol resource violation and
    /// closes; silently dropping a child would leave both peers with a false
    /// reconciliation result.
    exhausted: bool,
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
            pending: BTreeMap::new(),
            open: 0,
            missing: BTreeSet::new(),
            exhausted: false,
        }
    }

    /// Whether a peer has driven one of the bounded RBSR ledgers past its
    /// safe capacity.  A session must fail closed rather than continue with a
    /// partial reconciliation state.
    pub fn resource_exhausted(&self) -> bool {
        self.exhausted
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
    pub fn on_range(
        &mut self,
        range: &Range,
        is_reply: bool,
        src: &mut impl IdSource,
    ) -> Vec<Frame> {
        if range.lo > range.hi {
            return Vec::new();
        }
        let ids = src.ids_in(self.domain, &range.lo, &range.hi);
        let our_fp = fingerprint(&ids);
        if is_reply {
            // Echo: a matching descriptor resolves the non-terminal range we
            // issued; a differing echo shouldn't happen (peers only echo
            // matches), and is ignored by the exact-key lookup.
            // A terminal-list obligation is not a range we emitted: it was
            // created after we sent RECON_IDS and must be completed by the
            // peer's corresponding list/split response.  Do not let a
            // forged RECON_RANGE echo shortcut that obligation and make the
            // session advance with an unresolved set difference.
            self.resolve_exact(range);
            return Vec::new();
        }
        // A terminal ID-list response is a distinct obligation: the peer
        // must answer the list with its own list or a split range.  A
        // non-reply RECON_RANGE carrying the exact terminal descriptor is
        // therefore malformed.  Treating it as an ordinary matching range
        // would let an authenticated peer retire the terminal obligation via
        // `note_range_response`, advancing the domain without ever proving
        // the peer's set.  Fail closed; this is state validation only and
        // does not add a wire field or alter valid transcripts.
        if self
            .pending
            .get(&Self::key(range))
            .is_some_and(|state| state.await_terminal)
        {
            self.exhausted = true;
            return Vec::new();
        }
        let has_obligation = self.containing_key(&range.lo, &range.hi).is_some();
        if our_fp == range.fp && ids.len() as u32 == range.count {
            self.note_range_response(&range.lo, &range.hi);
            // Match → echo so the asker resolves this range.
            return alloc::vec![Frame::ReconRange {
                domain: self.domain | DOMAIN_REPLY_FLAG,
                ranges: alloc::vec![Range {
                    lo: range.lo,
                    hi: range.hi,
                    count: ids.len() as u32,
                    fp: our_fp
                }],
            }];
        }
        // The response carries OUR IDs. A small peer-advertised count does
        // not make a large local set wire-safe: listing `ids` here could
        // exceed MAX_FRAME_PAYLOAD and kill the session. Split whenever our
        // own set is larger than the terminal-list threshold.
        if ids.len() <= RBSR_T_LIST {
            self.note_range_response(&range.lo, &range.hi);
            if has_obligation {
                self.track_terminal(&Range {
                    lo: range.lo,
                    hi: range.hi,
                    count: ids.len() as u32,
                    fp: our_fp,
                });
            }
            return self.list_frames(&range.lo, &range.hi, ids);
        }
        // Mismatch, local side exceeds the terminal threshold: reply with
        // OUR count-balanced splits, even if the peer advertised a small set.
        self.note_range_response(&range.lo, &range.hi);
        let mut ranges = Vec::with_capacity(RBSR_B_F);
        for (lo, hi, sub) in split_ranges(&range.lo, &range.hi, &ids, RBSR_B_F) {
            ranges.push(Range {
                lo,
                hi,
                count: sub.len() as u32,
                fp: fingerprint(&sub),
            });
        }
        self.track_all(&ranges);
        alloc::vec![Frame::ReconRange {
            domain: self.domain,
            ranges
        }]
    }

    /// Handle a peer's terminal ID list for `[lo, hi]`.
    ///
    /// Pre-existing bug fixed here (found during E5-S1 blob-plane
    /// acceptance testing, not part of that story's own design): the peer
    /// reaching a terminal list only proves THEIR side of `[lo, hi]` is small.
    /// The responder must independently keep its own terminal list within the
    /// wire threshold; a peer's claimed count is not a payload-size guarantee.
    /// Unconditionally
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
    pub fn on_ids(
        &mut self,
        lo: &Id16,
        hi: &Id16,
        ids: &[Id16],
        src: &mut impl IdSource,
    ) -> Vec<Frame> {
        if lo > hi || ids.iter().any(|id| *id < *lo || *id > *hi) {
            return Vec::new();
        }
        self.resolve_bounds(lo, hi);
        for id in ids {
            if !src.has(id) {
                if !self.missing.contains(id) && self.missing.len() >= MAX_MISSING_IDS {
                    self.exhausted = true;
                    break;
                }
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
                ranges.push(Range {
                    lo: rlo,
                    hi: rhi,
                    count: sub.len() as u32,
                    fp: fingerprint(&sub),
                });
            }
            self.track_all(&ranges);
            return alloc::vec![Frame::ReconRange {
                domain: self.domain,
                ranges
            }];
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
        if self.pending.contains_key(&Self::key(range)) {
            return;
        }
        if self.pending.len() >= MAX_PENDING_RANGES {
            self.exhausted = true;
            return;
        }
        self.pending
            .entry(Self::key(range))
            .or_insert(PendingState {
                coverage: BTreeSet::new(),
                await_terminal: false,
            });
        self.refresh_open();
    }

    fn track_all(&mut self, ranges: &[Range]) {
        for range in ranges {
            self.track(range);
            if self.exhausted {
                break;
            }
        }
        self.refresh_open();
    }

    fn track_terminal(&mut self, range: &Range) {
        if !self.pending.contains_key(&Self::key(range)) && self.pending.len() >= MAX_PENDING_RANGES
        {
            self.exhausted = true;
            return;
        }
        let state = self
            .pending
            .entry(Self::key(range))
            .or_insert(PendingState {
                coverage: BTreeSet::new(),
                await_terminal: false,
            });
        state.await_terminal = true;
        self.refresh_open();
    }

    fn resolve_exact(&mut self, range: &Range) -> bool {
        let key = Self::key(range);
        // `await_terminal` means this descriptor was emitted as RECON_IDS,
        // not as a range.  A reply-flagged RECON_RANGE cannot satisfy it.
        let removable = self
            .pending
            .get(&key)
            .is_some_and(|state| !state.await_terminal);
        let removed = removable && self.pending.remove(&key).is_some();
        self.refresh_open();
        removed
    }

    fn resolve_bounds(&mut self, lo: &Id16, hi: &Id16) -> bool {
        let key = self
            .pending
            .iter()
            .filter(|(key, _state)| key.0 == *lo && key.1 == *hi)
            .max_by_key(|(_, state)| state.await_terminal)
            .map(|(key, _)| *key);
        let removed = key.is_some_and(|key| self.pending.remove(&key).is_some());
        self.refresh_open();
        removed
    }

    fn containing_key(&self, lo: &Id16, hi: &Id16) -> Option<RangeKey> {
        let mut best: Option<(RangeKey, bool)> = None;
        for (key, state) in &self.pending {
            if key.0 <= *lo && *hi <= key.1 {
                let better = best.is_none_or(|(old, old_terminal)| {
                    key.0 > old.0
                        || (key.0 == old.0 && key.1 < old.1)
                        || (key.0 == old.0
                            && key.1 == old.1
                            && state.await_terminal
                            && !old_terminal)
                });
                if better {
                    best = Some((*key, state.await_terminal));
                }
            }
        }
        best.map(|(key, _)| key)
    }

    /// Record one response interval for the innermost pending obligation.
    /// Returns the matched key, if any. A parent is retired only after the
    /// union covers the full inclusive span and no terminal response remains.
    fn note_range_response(&mut self, lo: &Id16, hi: &Id16) -> Option<RangeKey> {
        let key = self.containing_key(lo, hi)?;
        let state = self.pending.get_mut(&key)?;
        state.await_terminal = false;
        if !state.coverage.contains(&(*lo, *hi)) {
            if state.coverage.len() >= MAX_COVERAGE_INTERVALS_PER_PARENT {
                self.refresh_open();
                return Some(key);
            }
            state.coverage.insert((*lo, *hi));
        }
        let complete = interval_set_covers((key.0, key.1), &state.coverage);
        if complete && !state.await_terminal {
            self.pending.remove(&key);
        }
        self.refresh_open();
        Some(key)
    }

    fn list_frames(&mut self, lo: &Id16, hi: &Id16, ids: Vec<Id16>) -> Vec<Frame> {
        if !self.listed.contains(&(*lo, *hi)) && self.listed.len() >= MAX_LISTED_RANGES {
            self.exhausted = true;
            return Vec::new();
        }
        self.listed.insert((*lo, *hi));
        // A list both offers our IDs and (on the peer) resolves this range.
        alloc::vec![Frame::ReconIds {
            domain: self.domain,
            lo: *lo,
            hi: *hi,
            ids
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FixedIds {
        ids: Vec<Id16>,
    }

    impl IdSource for FixedIds {
        fn ids_in(&mut self, _domain: u8, lo: &Id16, hi: &Id16) -> Vec<Id16> {
            self.ids
                .iter()
                .copied()
                .filter(|id| *id >= *lo && *id <= *hi)
                .collect()
        }

        fn has(&mut self, _id: &Id16) -> bool {
            false
        }
    }

    fn id_with_tail(value: u16) -> Id16 {
        let mut bytes = [0x80; 16];
        bytes[14..].copy_from_slice(&value.to_be_bytes());
        Id16(bytes)
    }

    fn high_bound(value: u16) -> Id16 {
        let mut bytes = [0xff; 16];
        bytes[14..].copy_from_slice(&value.to_be_bytes());
        Id16(bytes)
    }

    #[test]
    fn hostile_range_descriptors_trip_pending_cap_without_growth() {
        let mut source = FixedIds {
            ids: (0..25).map(id_with_tail).collect(),
        };
        let mut recon = Reconciler::new(0);

        // Every descriptor covers the same 25 local IDs but carries a unique
        // upper bound.  A peer can send these as valid, in-cap frames; before
        // the bound each one opened another 16-child split in `pending`.
        for value in 0..=u16::MAX {
            let range = Range {
                lo: Id16::MIN,
                hi: high_bound(value),
                count: 0,
                fp: 0,
            };
            recon.on_range(&range, false, &mut source);
            if recon.resource_exhausted() {
                break;
            }
        }

        assert!(recon.resource_exhausted());
        assert!(recon.pending.len() <= MAX_PENDING_RANGES);
    }

    #[test]
    fn hostile_terminal_ranges_and_missing_ids_are_bounded() {
        let mut source = FixedIds { ids: Vec::new() };
        let mut recon = Reconciler::new(0);

        // Small local ranges take the terminal-list path.  Distinct bounds
        // would otherwise make `listed` grow for the life of the session.
        for value in 0..=u16::MAX {
            let range = Range {
                lo: Id16::MIN,
                hi: high_bound(value),
                count: 1,
                fp: 1,
            };
            recon.on_range(&range, false, &mut source);
            if recon.resource_exhausted() {
                break;
            }
        }
        assert!(recon.resource_exhausted());
        assert!(recon.listed.len() <= MAX_LISTED_RANGES);

        let mut recon = Reconciler::new(0);
        let ids: Vec<Id16> = (0..=MAX_MISSING_IDS as u16).map(id_with_tail).collect();
        let range = Range {
            lo: Id16::MIN,
            hi: Id16::MAX,
            count: ids.len() as u32,
            fp: 0,
        };
        recon.on_ids(&range.lo, &range.hi, &ids, &mut source);
        assert!(recon.resource_exhausted());
        assert!(recon.missing.len() <= MAX_MISSING_IDS);
    }

    #[test]
    fn terminal_list_obligation_cannot_be_resolved_by_range_echo() {
        let id = id_with_tail(7);
        let mut source = FixedIds { ids: vec![id] };
        let mut recon = Reconciler::new(3);
        let _ = recon.init_frame(&mut source);

        // A mismatched child with a small local set emits RECON_IDS and
        // records a terminal-list obligation in addition to the parent
        // coverage.  A RECON_RANGE echo is not a valid response to that
        // terminal-list exchange and must not retire the obligation.
        let child = Range {
            lo: id,
            hi: id,
            count: 0,
            fp: 0,
        };
        let frames = recon.on_range(&child, false, &mut source);
        assert!(matches!(frames.first(), Some(Frame::ReconIds { .. })));
        assert_eq!(
            recon.open, 2,
            "parent and terminal-list obligations are pending"
        );

        let terminal = Range {
            lo: id,
            hi: id,
            count: 1,
            fp: fingerprint(&[id]),
        };
        assert!(recon.on_range(&terminal, true, &mut source).is_empty());
        assert_eq!(
            recon.open, 2,
            "a range echo must not shortcut RECON_IDS completion"
        );
    }

    #[test]
    fn terminal_list_obligation_rejects_matching_non_reply_range() {
        let id = id_with_tail(8);
        let mut source = FixedIds { ids: vec![id] };
        let mut recon = Reconciler::new(3);
        let _ = recon.init_frame(&mut source);

        // This creates the terminal-list child obligation for [id, id].
        let child = Range {
            lo: id,
            hi: id,
            count: 0,
            fp: 0,
        };
        let frames = recon.on_range(&child, false, &mut source);
        assert!(matches!(frames.first(), Some(Frame::ReconIds { .. })));
        assert_eq!(recon.open, 2);

        // A non-reply range with the exact terminal descriptor is not the
        // required peer ID-list/split response.  It must fail closed rather
        // than retiring the obligation and allowing domain advancement.
        let matching = Range {
            lo: id,
            hi: id,
            count: 1,
            fp: fingerprint(&[id]),
        };
        assert!(recon.on_range(&matching, false, &mut source).is_empty());
        assert!(recon.resource_exhausted());
        assert_eq!(
            recon.open, 2,
            "malformed range must not retire terminal state"
        );
    }

    #[test]
    fn d3_transcript_keeps_one_wire_domain_without_priority_partition() {
        // The frozen rev-A executable behavior carries every relay-spray item
        // (including class-5 announcements) under D3.  Declared priority is
        // intentionally not a second wire key until the protocol erratum
        // defines one.  This test locks the safe boundary: every RBSR frame
        // produced by a D3 reconciler keeps base domain 3, with only the
        // existing reply flag allowed to vary.
        let mut source = FixedIds {
            ids: (0..40).map(id_with_tail).collect(),
        };
        let mut recon = Reconciler::new(3);
        let init = recon.init_frame(&mut source);
        assert_eq!(recon_frame_domain(&init), Some(3));

        // Force the split path so the assertion covers generated RECON_RANGE
        // children rather than only the initial full-range frame.
        let mismatched = Range {
            lo: Id16::MIN,
            hi: Id16::MAX,
            count: 0,
            fp: 0,
        };
        let replies = recon.on_range(&mismatched, false, &mut source);
        assert!(!replies.is_empty());
        assert!(replies.iter().all(|frame| {
            recon_frame_domain(frame).is_some_and(|domain| domain & !DOMAIN_REPLY_FLAG == 3)
        }));

        // The terminal-list and echo branches must preserve the same base
        // domain as the split branch.  This is deliberately a transcript
        // invariant only: declared priority is not a key until an approved
        // erratum defines a partition identifier and legacy behavior.
        let mut empty = FixedIds { ids: Vec::new() };
        let mut terminal = Reconciler::new(3);
        let terminal_range = Range {
            lo: Id16::MIN,
            hi: Id16::MAX,
            count: 1,
            fp: 1,
        };
        let terminal_frames = terminal.on_range(&terminal_range, false, &mut empty);
        assert!(terminal_frames.iter().all(|frame| {
            recon_frame_domain(frame).is_some_and(|domain| domain & !DOMAIN_REPLY_FLAG == 3)
        }));

        let mut echo_source = FixedIds { ids: Vec::new() };
        let mut echo = Reconciler::new(3);
        let _ = echo.init_frame(&mut echo_source);
        let matching = Range {
            lo: Id16::MIN,
            hi: Id16::MAX,
            count: 0,
            fp: fingerprint(&[]),
        };
        let echo_frames = echo.on_range(&matching, false, &mut echo_source);
        assert!(echo_frames.iter().all(|frame| {
            recon_frame_domain(frame).is_some_and(|domain| domain & !DOMAIN_REPLY_FLAG == 3)
        }));
    }

    fn recon_frame_domain(frame: &Frame) -> Option<u8> {
        match frame {
            Frame::ReconInit { domain, .. }
            | Frame::ReconRange { domain, .. }
            | Frame::ReconIds { domain, .. } => Some(*domain),
            _ => None,
        }
    }
}
