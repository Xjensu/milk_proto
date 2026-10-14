# mmp-store

`BundleStore` implementations. Today: `SnapshotStore`, a checksummed atomic-write
persistence layer (bundles, claims, group state, `SeenIndex`/`ReceiptFilter`) with
corrupt-record skip-on-load — no panic is reachable from a corrupted or truncated
file (handbook §0 rule 9). The default `std` feature exposes that filesystem API;
`--no-default-features` is a dependency-fence profile with no store API while the
fixed-capacity embedded backend is being designed. A flash-log-oriented `no_std`
store for the embedded tier is reserved but not yet implemented.

Feature behavior is deliberate: `mmp-store`'s `std` feature forwards
`mmp-core/std` and `mmp-crypto/std`, while both path dependencies are declared
with `default-features = false`. This keeps host defaults unchanged and prevents
future embedded builds from inheriting the filesystem/Noise host graph silently.

**Layering:** depends on `mmp-types`, `mmp-crypto` (to re-verify claim signatures on
load), and `mmp-core`. Consumed by `mmp-node` and `mmp-ffi`.

See `docs/implementation/README.md` §4.3.
