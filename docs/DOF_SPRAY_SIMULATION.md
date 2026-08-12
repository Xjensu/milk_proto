# DoF Spray Simulation Slice

**Status:** research-only simulator model; not part of the MMP/1 wire protocol.

`mmp-sim::dof` provides a bounded experiment surface for the MMP/Φ fountain/DoF
hypothesis. It reuses the existing `mmp-rs-code` Reed–Solomon implementation
and does not alter `mmp-node`, `mmp-router`, bundle classes, or transport
behavior.

## Model

- `DofLedger` accepts at most `k` distinct encoded symbol indices.
- Repeated identical symbols are ignored; conflicting bytes for one index fail
  closed.
- Index, length, parameter, and post-completion violations do not mutate the
  ledger.
- `DofContactPlanner` allocates ascending indices once, from one bounded total
  budget across contacts.
- Reconstruction is delegated to `mmp-rs-code`; the model assumes symbols belong
  to one common Reed–Solomon block.

The model reports rank, useful symbols, retained bytes, completion, and planned
symbol budget. It is not evidence that DoF Spray improves delivery, latency,
airtime, or battery use.

## Verification

```text
cargo test -p mmp-sim dof --lib --locked
cargo clippy -p mmp-sim --lib --all-features --locked -- -D warnings
```

The focused suite covers duplicate suppression, conflicting duplicates,
bounded rank, malformed inputs, deterministic contact allocation, and a lossy
multi-contact reconstruction using the existing Reed–Solomon decoder.

## Evidence boundary

This slice is a mechanism test, not an MMP/Φ result. Confirmation requires a
paired baseline/candidate experiment with identical contact traces, loss model,
payloads, seeds, and separately reported delivery, latency, bytes, airtime, and
energy distributions. No production strategy may consume this module until
those measurements and an explicit architecture review exist.
