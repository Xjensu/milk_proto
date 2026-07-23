# mmp-node

Full `std` orchestration (doc 04, doc 11 §1–§2): the `Node` API — session lifecycle,
claims gossip, direct messaging (X3DH first contact + ratchet), groups (invite/
remove/epoch rotation), friendship/proxy buffering, the blob (file transfer) state
machine, receipts, beacon rotation, the tier ladder (T0 sensor .. T3 anchor), and the
opt-in local-only diagnostics journey log.

**Layering:** depends on `mmp-types`, `mmp-crypto`, `mmp-core`, `mmp-router`,
`mmp-store`, `mmp-tal`, `mmp-rs-code`. This is the crate boundary where `std` becomes
unconditional — everything below is sans-IO/`no_std`-capable; everything at or above
this layer owns real time, storage, and (via a TAL implementation) a radio.
`mmp-ffi`, `mmp-cli`, and `mmp-sim` all consume this crate's public API rather than
reaching into the lower layers directly.

See `docs/IMPLEMENTATION_HANDBOOK.md` §1.1 and `specification/04-architecture.md`.
