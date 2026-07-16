//! Diagnostics / trace schema (doc 12 §7.7, AUDIT F14): the answer to "why
//! didn't this deliver?" for a black-box DTN. Off by default; the node records
//! per-bundle journey events and the CLI's `inspect` consumes them.

use mmp_types::ids::{Id16, NodeId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JourneyEvent {
    Created,
    Stored,
    HandedOff { peer: NodeId, budget: u8 },
    ReceiptSeen,
    Purged,
    Expired,
    Evicted,
}

#[derive(Debug, Clone)]
pub struct JourneyRecord {
    pub bundle: Id16,
    pub event: JourneyEvent,
    pub ts: u64,
}

#[derive(Default)]
pub struct DiagLog {
    enabled: bool,
    records: Vec<JourneyRecord>,
    pub clamps: u64,
    pub drops: u64,
}

impl DiagLog {
    pub fn new(enabled: bool) -> DiagLog {
        DiagLog { enabled, records: Vec::new(), clamps: 0, drops: 0 }
    }

    pub fn log(&mut self, bundle: Id16, event: JourneyEvent, ts: u64) {
        if self.enabled {
            self.records.push(JourneyRecord { bundle, event, ts });
        }
    }

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
