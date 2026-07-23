# mmp-core

Bundle core, session FSM, and sync engine (docs 03/06/11): the verify-before-store /
verify-before-forward ingest pipeline, resurrection-resistant `SeenIndex`, the
Hint/Purge `ReceiptFilter`, claims LWW merge, the sans-IO `SessionEngine`
(Handshake → Binding → Established → Closed), RBSR set reconciliation, the
conversation DAG, group OR-Set membership, and authority-announcement handling.

**Layering:** depends on `mmp-types` and `mmp-crypto` (`default-features = false`).
Sans-IO throughout — no sockets, no timers, no clock of its own; time enters only as
a caller-supplied `now_ms`. `#![cfg_attr(not(feature = "std"), no_std)]`, `std`
forwards to `mmp-crypto/std` for the Noise-backed session layer.

See `docs/IMPLEMENTATION_HANDBOOK.md` §2 and `specification/03-protocol-spec.md` /
`specification/06-sync-spec.md` / `specification/11-state-machines.md`.
