# mmp-rs-code

`no_std` systematic Reed-Solomon over GF(256) (doc 08 §8) for the blob (file
transfer) plane: source symbols plus parity, recoverable from any `k` of `n` symbols,
corrupted-parity-yields-wrong-data-not-panic by construction. The OQ-7 decode-speed
benchmark on nRF52-class silicon (specification/research/open-questions.md,
confidence Medium, "believed trivial, not yet benchmarked") belongs here.

**Layering:** depends on nothing else in the workspace — a standalone leaf crate.
Consumed by `mmp-node`'s blob state machine.

See `specification/16-file-transfer.md` (referenced as doc 16 elsewhere in this repo).

## Host benchmark (not hardware evidence)

Run the deterministic decode workload with:

```text
cargo run --locked -p mmp-rs-code --example decode_bench --release
```

Set `MMP_RS_BENCH_ITERS` to change the sample count (the default is five).
The workload checks the recovered bytes for every sample and reports minimum,
median, and maximum host elapsed time for the file-stripe (`k=32`, `n=40`)
and maximum-`k` (`k=64`, `n=72`) profiles, each with 2,048-byte symbols. Each
profile also prints `input_fingerprint`, a stable non-cryptographic 64-bit
fingerprint of the exact `(symbol_index, symbol_bytes)` sequence passed to
`decode`; record it with the timing so a changed workload cannot be mistaken
for a comparable run. The fingerprint is the wrapping recurrence
`h = h * 1_099_511_628_211 + value`, starting at zero, over each symbol's
index, byte length, and bytes in order. It uses only the existing crate and
standard-library timing in the example; no benchmark dependency is added.

These numbers are diagnostic and cannot establish the OQ-7 acceptance gate.
That gate still requires a linked nRF52-class image, measured flash/RAM, and a
target-silicon Reed--Solomon decode run. The current CI `thumbv7em` check and
rlib budgets are dependency/size-regression checks, not linked-image evidence.
