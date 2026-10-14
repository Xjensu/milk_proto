//! Diagnostics / trace schema (doc 12 §7.7, AUDIT F14): the answer to "why
//! didn't this deliver?" for a black-box DTN. Off by default; the node records
//! per-bundle journey events and the CLI's `inspect` consumes them.

use mmp_types::ids::{Id16, NodeId};

/// Maximum number of journey records retained by one local diagnostics
/// window. Diagnostics are deliberately opt-in, but enabling them must not
/// turn a long-running node into an unbounded in-memory log sink.
pub const MAX_RECORDS: usize = 4096;

/// One lifecycle step a bundle passed through, as logged by `NodeCore`'s
/// various call sites (`try_store_own`, `on_bundle`, `enforce_quota`, `tick`,
/// `load_for_handoff`, `handle_receipt`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JourneyEvent {
    /// Minted and accepted by the local provider (`NodeCore::try_store_own`).
    Created,
    /// Admitted into the store from the wire (`SessionHost::on_bundle`).
    Stored,
    /// Handed to a peer during a session, with the spray budget granted to
    /// them (`SessionHost::load_for_handoff`).
    HandedOff {
        /// The peer the bundle was handed off to.
        peer: NodeId,
        /// The spray budget granted to that peer for this copy.
        budget: u8,
    },
    /// A delivery receipt referencing this bundle was seen (as carrier or
    /// origin, `NodeCore::handle_receipt`).
    ReceiptSeen,
    /// Removed from the store because a verified receipt proved delivery
    /// (`NodeCore::handle_receipt`).
    Purged,
    /// Removed from the store because its `lifetime_ms` lapsed (`Node::tick`).
    Expired,
    /// Removed from the store by quota pressure (`NodeCore::enforce_quota`).
    Evicted,
}

/// One recorded `JourneyEvent` for one bundle, timestamped on the monotonic
/// clock (`NodeCore::now`).
#[derive(Debug, Clone)]
pub struct JourneyRecord {
    /// The bundle this event happened to.
    pub bundle: Id16,
    /// What happened.
    pub event: JourneyEvent,
    /// Monotonic timestamp (ms) the event was logged at.
    pub ts: u64,
}

/// Opt-in, local-only bookkeeping of per-bundle lifecycle events — a bounded
/// in-memory `Vec<JourneyRecord>` with no file or network I/O of its own.
/// Disabled by default (`Node::new`); `Node::enable_diagnostics` turns it on
/// so `records`/`dump` have something to answer "why didn't this deliver?"
/// with. Once [`MAX_RECORDS`] is reached, later records are ignored until the
/// window is replaced or diagnostics are disabled. Never persisted and never
/// transmitted.
#[derive(Default)]
pub struct DiagLog {
    enabled: bool,
    records: Vec<JourneyRecord>,
    /// Count of class-5 announcements whose priority was clamped down from
    /// what was declared (doc 18 §5.2), tallied regardless of `enabled`.
    pub clamps: u64,
    /// Count of bundles rejected by `ingest`'s admission gate, tallied
    /// regardless of `enabled`.
    pub drops: u64,
}

impl DiagLog {
    /// A fresh, empty log; `enabled` gates whether `log` actually records
    /// anything (the `clamps`/`drops` counters always accrue either way).
    pub fn new(enabled: bool) -> DiagLog {
        DiagLog {
            enabled,
            records: Vec::new(),
            clamps: 0,
            drops: 0,
        }
    }

    /// Stop recording new journey events while retaining records already
    /// collected for the current local inspection window.
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// Append one journey event for `bundle`, if the log is enabled — a
    /// silent no-op otherwise.
    pub fn log(&mut self, bundle: Id16, event: JourneyEvent, ts: u64) {
        if self.enabled && self.records.len() < MAX_RECORDS {
            self.records.push(JourneyRecord { bundle, event, ts });
        }
    }

    /// Every recorded event, in the order it was logged.
    pub fn records(&self) -> &[JourneyRecord] {
        &self.records
    }

    /// Line-delimited dump for `mmp-cli inspect`.
    pub fn dump(&self) -> String {
        use std::fmt::Write;
        let mut s = String::new();
        for r in &self.records {
            let _ = writeln!(s, "{:>10} {:02x?} {:?}", r.ts, &r.bundle.0[..4], r.event);
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::{DiagLog, JourneyEvent, MAX_RECORDS};
    use mmp_types::ids::Id16;

    #[test]
    fn disabling_keeps_existing_records_but_stops_new_events() {
        let mut log = DiagLog::new(true);
        let bundle = Id16([7; 16]);
        log.log(bundle, JourneyEvent::Created, 1);
        assert_eq!(log.records().len(), 1);

        log.disable();
        log.log(bundle, JourneyEvent::Stored, 2);

        assert_eq!(log.records().len(), 1);
        assert_eq!(log.records()[0].event, JourneyEvent::Created);
    }

    #[test]
    fn recording_window_is_bounded_without_replacing_earlier_events() {
        let mut log = DiagLog::new(true);
        let bundle = Id16([8; 16]);
        for ts in 0..(MAX_RECORDS as u64 + 1) {
            log.log(bundle, JourneyEvent::Created, ts);
        }

        assert_eq!(log.records().len(), MAX_RECORDS);
        assert_eq!(log.records().first().map(|record| record.ts), Some(0));
        assert_eq!(
            log.records().last().map(|record| record.ts),
            Some((MAX_RECORDS - 1) as u64)
        );
    }
}
