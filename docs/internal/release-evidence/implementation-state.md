# Implementation state

This neutral copy records the 2026-09-11 checkout evidence. The detailed
subsystem reconciliation is in [`reconciliation-notes.md`](reconciliation-notes.md).

## Subsystems

| Area | State | Evidence or open gate |
|---|---|---|
| Bundle codec | implemented | canonical decoder and vectors |
| Identity/claims | partial | claim validation; Friendship/avatar grammar open |
| Noise and ID_PROOF | partial | encrypted channel binding; external review open |
| Direct crypto/ratchet | implemented in core | sealed/prekey first contact and ratchet tests |
| Session FSM | implemented | frame, timer, credit, RBSR, transfer tests |
| Routing | implemented for MMP/1 | binary spray, Focus, utility, quotas |
| Receipts | implemented | keyed MAC and filter tests |
| BLE | partial | codec and adapters; zero physical sessions |
| Groups | partial | sender keys; epoch authorization open |
| Files/blobs | partial | RS/Merkle and fetch; plaintext symbols visible to relays |
| Friendship | partial | offer/ack and buffer; offline proxy reconciliation open |
| Authority | extension partial | local cert and clamp; root learning and full RBSR integration open |
| Simulator | baseline implemented | calibrated mobility and scale gates open |

## Build and test evidence

- `cargo build --workspace --all-features --locked`: passed on Windows.
- `cargo test --workspace --all-targets --all-features --locked -- --test-threads=1`: Windows MSVC PDB linking failed; the `mmp-node` tests passed with `RUSTFLAGS=-C debuginfo=0`.
- Combined successful Rust split: 898 passed, 0 failed, 6 ignored.
- Python suite: 261 passed, 10 skipped for platform or artifact conditions.
- Layer check and deterministic fuzz seed validation: passed.
- Sanitizer-linked fuzzing, native mobile builds, physical BLE, and embedded hardware: not completed in this environment.

The repository remains a functional prototype. The evidence does not establish production readiness.
