# mmp-router

Bounded replication (doc 05): Binary Spray-and-Wait + Focus, locally-observed PRoPHET-
style utility (deliberately **no transitivity** — a peer can never inflate your
opinion of a third party), per-origin token buckets with a web-of-trust multiplier,
and eviction/transfer ordering. No route computation exists here — only per-contact
decisions.

**Layering:** depends on `mmp-core` (`default-features = false`) and `mmp-types`.
`#![cfg_attr(not(feature = "std"), no_std)]`.

See `docs/implementation/README.md` §2.5 and `docs/spec/routing.md`.
