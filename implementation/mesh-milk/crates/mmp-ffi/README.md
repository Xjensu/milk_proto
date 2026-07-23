# mmp-ffi

The UniFFI 0.29 binding surface consumed by mobile apps (Kotlin/Android, Swift/iOS,
and — through those — React Native): a `MmpNode` object wrapping `mmp-node`'s `Node`,
transport injection (`on_beacon`, wire bytes in/out), event polling, groups, claims,
friendship, blob transfer, and snapshot persistence. Secrets never cross this
boundary outward — only public keys, ciphertexts, IDs, and events do.

**Layering:** depends on `mmp-node`, `mmp-core`, `mmp-store`, `mmp-tal`, `mmp-types`.
`crate-type = ["cdylib", "staticlib", "lib"]` — cdylib for Android, staticlib for the
iOS XCFramework, plain `lib` for Rust-native consumers and this crate's own tests.
The `uniffi-bindgen` CLI binary (Kotlin/Swift codegen) is gated behind the opt-in
`cli` feature so ordinary mobile-app builds never pull in `clap`/`camino`.

See `docs/API_REFERENCE.md` and `implementation/mesh-milk/MOBILE_INTEGRATION.md`.
