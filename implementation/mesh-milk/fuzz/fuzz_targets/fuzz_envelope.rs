//! Doc 12 §7.2 target 2. `read_envelope` runs before any authentication (it's
//! the length-prefix framing every session frame arrives inside); when it
//! yields a `(type, payload)` pair, `Frame::decode_payload` hardens the payload
//! grammar. HELLO and HS are pre-proof inputs; ID_PROOF is deliberately fuzzed
//! as a decoded payload even though the live protocol carries it encrypted as
//! the first Noise transport record. Chaining both mirrors the parser boundary.
//! Invariant: no
//! panic, no OOM — a length prefix exceeding `MAX_ENVELOPE_PAYLOAD` is
//! `Err(Oversize)`, never an unbounded allocation (AUDIT F10 class).

#![no_main]

use libfuzzer_sys::fuzz_target;
use mmp_types::frame::{read_envelope, Frame};

fuzz_target!(|data: &[u8]| {
    if let Ok(Some((typ, payload, _consumed))) = read_envelope(data) {
        let _ = Frame::decode_payload(typ, payload);
    }
});
