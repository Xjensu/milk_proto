# Rust architecture

The workspace layers are:

```text
mmp-types       wire types, codecs, constants, vectors
mmp-crypto      identity, Noise, AEAD, ratchets, Merkle proofs
mmp-core        sessions, ingest, claims, reconciliation, indexes
mmp-router      utility, spray/focus, quotas, bounded routing state
mmp-store       durable snapshots and recovery
mmp-rs-code     Reed-Solomon coding
mmp-tal*        transport abstraction and platform adapters
mmp-node        node orchestration, blobs, groups, friendship, authority
mmp-ffi         mobile bindings
mmp-sim         deterministic simulation and research sidecars
mmp-cli         command-line tools and vector generation
```

Lower layers do not depend on platform adapters. `no_std` builds cover core codec, crypto, core, router, and Reed-Solomon crates. Platform adapters provide BLE and persistence facilities through explicit traits.
