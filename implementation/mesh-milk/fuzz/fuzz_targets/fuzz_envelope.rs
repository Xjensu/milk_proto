//! Doc 12 §7.2 target 2. `read_envelope` runs before any authentication (it's
//! the length-prefix framing every session frame arrives inside); when it
//! yields a `(type, payload)` pair, `Frame::decode_payload` is the next
//! pre-auth step for frame types that precede identity proof (HELLO, HS,
//! ID_PROOF itself). Chaining both mirrors the real parse loop. Invariant: no
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
