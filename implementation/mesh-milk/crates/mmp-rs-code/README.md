# mmp-rs-code

`no_std` systematic Reed-Solomon over GF(256) (doc 08 §8) for the blob (file
transfer) plane: source symbols plus parity, recoverable from any `k` of `n` symbols,
corrupted-parity-yields-wrong-data-not-panic by construction. The OQ-7 decode-speed
benchmark on nRF52-class silicon (specification/research/open-questions.md,
confidence Medium, "believed trivial, not yet benchmarked") belongs here.

**Layering:** depends on nothing else in the workspace — a standalone leaf crate.
Consumed by `mmp-node`'s blob state machine.

See `specification/16-file-transfer.md` (referenced as doc 16 elsewhere in this repo).
