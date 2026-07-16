# fuzz/ — cargo-fuzz targets (doc 07 §8 / doc 12 §7 mandate, backlog E5-S6)

Four targets, in priority order (pre-auth parsers first — the ones that run before any identity
is proven, so their input is fully attacker-controlled with no cost to the attacker):

1. `fuzz_beacon` → `Beacon::decode` — unauthenticated broadcast, 24 B fixed input.
2. `fuzz_envelope` → `read_envelope` + `Frame::decode_payload` — the session-frame length-prefix
   framing plus payload decode for pre-auth frame types (HELLO, HS, ID_PROOF).
3. `fuzz_bundle` → `Bundle::decode_canonical` — every bundle any relay ever sees, before
   `verify_strict` (doc 03 §4.1.2 step 1).
4. `fuzz_hs` → `NoiseHandshake::responder().advance()` — the first message from an unauthenticated
   peer in a live Noise XX handshake.

Invariant for all four: no panic, no OOM. The parsers already pre-validate lengths before any
allocation proportional to unvalidated input (doc 10 §6.4/AUDIT F10) — anything the fuzzer finds
that violates this is a real bug in that class.

## Running

```sh
cargo install cargo-fuzz --locked   # once
cd fuzz
cargo +nightly fuzz build fuzz_beacon      # or fuzz_envelope / fuzz_bundle / fuzz_hs
cargo +nightly fuzz run fuzz_bundle -- -max_total_time=120   # short local run
```

**Linux/macOS only in practice.** libFuzzer needs a sanitizer runtime (ASan/SanitizerCoverage);
on `ubuntu-latest` (this repo's CI) that ships with the standard clang toolchain and just works. On
a Windows dev box you will likely hit a linker error (`clang_rt.asan_dynamic_runtime_thunk` missing,
or `__start___sancov_pcs` unresolved even with `-s none`) — this was verified 2026-07-16: all four
harnesses compile cleanly under plain `rustc` on Windows, so the *code* is correct; only the final
sanitizer-instrumented link step needs a toolchain component Windows doesn't ship by default. Not a
project bug — develop/debug target logic with `cargo check`/`cargo build` (no `cargo fuzz` prefix)
on Windows, and use CI's `fuzz-build`/`fuzz-run` jobs (or a Linux/WSL box) to actually fuzz.

## CI

- `fuzz-build` (every push): `cargo fuzz build` for all four targets — a cheap "still compiles and
  links" gate, not a fuzzing run.
- `fuzz-run` (manual `workflow_dispatch` only, not on every push): a 2-minute smoke run per target,
  corpus uploaded as a build artifact. This is a smoke check, not the real hardening run — doc 12
  §7's "N CPU-hours once a baseline throughput is measured" is deliberately not pre-committed here
  (see `docs/ROADMAP_TO_PRODUCTION.md` Phase C exit gate, same discipline).

## What's NOT here yet

**No curated seed corpus.** `corpus/` is `.gitignore`d (libFuzzer's own discovered-input directory,
regenerated per run) and no hand-picked "interesting" seed inputs (a maximal-length beacon, a
bundle with every TLV type, a truncated Noise message, …) have been committed. libFuzzer bootstraps
fine from an empty corpus for formats this size, so this isn't blocking, but a deliberately-curated
seed set would likely find issues faster — tracked as follow-on work under E5-S6, not silently
dropped.
