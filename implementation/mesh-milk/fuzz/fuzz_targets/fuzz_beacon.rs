//! Doc 12 §7.2 target 1 (priority order: pre-auth parsers first, doc 07 §8 /
//! doc 12 §7 mandate). Beacons are unauthenticated broadcast — the very first
//! bytes any node parses from a stranger. Invariant: no panic, no OOM, only
//! `Ok`/`Err` (24 B fixed input, so allocation-proportional-to-input is moot
//! here, but the decode path must still never panic on arbitrary bytes).

#![no_main]

use libfuzzer_sys::fuzz_target;
use mmp_types::Beacon;

fuzz_target!(|data: &[u8]| {
    let _ = Beacon::decode(data);
});
