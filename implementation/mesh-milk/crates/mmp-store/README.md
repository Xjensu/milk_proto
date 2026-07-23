# mmp-store

`BundleStore` implementations. Today: `SnapshotStore`, a checksummed atomic-write
persistence layer (bundles, claims, group state, `SeenIndex`/`ReceiptFilter`) with
corrupt-record skip-on-load — no panic is reachable from a corrupted or truncated
file (handbook §0 rule 9). A flash-log-oriented `no_std` store for the embedded tier
is reserved but not yet implemented; this crate is `std`-only today.

**Layering:** depends on `mmp-types`, `mmp-crypto` (to re-verify claim signatures on
load), and `mmp-core`. Consumed by `mmp-node` and `mmp-ffi`.

See `docs/IMPLEMENTATION_HANDBOOK.md` §4.3.
