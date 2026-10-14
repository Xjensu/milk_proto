//! Doc 12 §7.2 target 4 (std-gated — the Noise session layer needs
//! `mmp-crypto`'s `std` feature, on by default). The responder's first
//! `advance()` call processes an incoming Noise XX message from a peer with
//! no prior authentication — attacker-controlled bytes in the fullest sense,
//! pre-identity. `identity`/`hello` are fixed so all fuzzer entropy lands on
//! `data`, the actual wire input; the fixed values are internal to this
//! target and carry no secrecy requirement. Invariant: no panic, no OOM —
//! malformed input is `Err(CryptoError::Noise)`, never a panic.

#![no_main]

use libfuzzer_sys::fuzz_target;
use mmp_crypto::{Identity, NoiseHandshake};
use mmp_types::frame::Hello;

fuzz_target!(|data: &[u8]| {
    let identity = Identity::from_seed(&[7u8; 32]);
    let hello = Hello { ver: 1, suites: vec![0x01], tier: 1, flags: 0, credits: 8, psm: 0 };
    if let Ok(mut hs) = NoiseHandshake::responder(&identity, &hello) {
        let _ = hs.advance(data);
    }
});
