# mmp-types

Canonical hand-written wire codec for MMP/1 (doc 10): ids, frames, bundles, claims,
the beacon, and the file-manifest payload schema. No serde on the wire, ever — one
valid byte representation of every value; parsers reject trailing bytes, overlong
lengths, and non-ascending/reserved TLV bits.

**Layering:** the bottom of the stack. `#![no_std]` + `alloc`, zero dependencies on
any other `mmp-*` crate, exactly one external dependency (`blake3`). Everything else
in the workspace depends on this crate, directly or transitively.

See `docs/implementation/README.md` §2.1 and `docs/spec/bundles.md`.
