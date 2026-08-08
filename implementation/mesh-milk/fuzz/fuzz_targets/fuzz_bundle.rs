//! Doc 12 §7.2 target 3. `Bundle::decode_canonical` is the entry point for
//! every bundle a relay ever sees, from any peer, before signature
//! verification (doc 03 §4.1.2 step 1 — canonical decode precedes
//! `verify_strict`). Invariant: no panic, no OOM, only `Ok`/`Err` — this is
//! the same property `mmp-types`' `props::bundle_decoder_never_panics`
//! proptest checks via mutation, but a persistent corpus explores far more of
//! the input space than single-byte mutations of one seed bundle.

#![no_main]

use libfuzzer_sys::fuzz_target;
use mmp_types::bundle::id_of_encoded;
use mmp_types::Bundle;

fuzz_target!(|data: &[u8]| {
    let _ = Bundle::decode_canonical(data);
    let _ = id_of_encoded(data);
});
