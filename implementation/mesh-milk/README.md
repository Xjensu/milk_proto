# mesh-milk — MMP/1 reference implementation

This directory contains the Rust workspace for the **mesh_milk (MMP/1)**
peer-to-peer BLE mesh protocol. The normative specification is in
`../../docs/spec/`; extension mechanisms are in `../../docs/extensions/`.
The crate layout is described in `../../docs/implementation/rust-layout.md`.

Current engineering status is recorded in
`../../docs/internal/handoff/current-state.md` and the release gates in
`../../docs/internal/handoff/release-gates.md`. The repository document map is
in `../../docs/README.md`.

The workspace contains protocol, host, transport, and embedded-provider crates.
`mmp-embedded-provider` provides a host-testable two-slot journal boundary;
`mmp-tal-peripheral` implements the peripheral-role BLE path. The `vectors/`
directory contains committed wire and conformance vectors, and
`tools/layer-check` checks crate layering.

The layering rule is `mmp-types → mmp-crypto → mmp-core → mmp-router`; these
crates use `#![no_std]` with optional `alloc`. Crates below `mmp-node` MUST NOT
depend on Tokio, standard-library time, or a transport implementation. CI and
`cargo run -p layer-check` enforce this rule.

## Verify locally

For focused changes, run the package-scoped helper:

```sh
python scripts/fast_check.py --package mmp-node
python scripts/fast_check.py --package mmp-types --package mmp-crypto
python scripts/fast_check.py --python
```

For a release-style local matrix:

```sh
cargo build --workspace --all-features
cargo test --workspace --all-features
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p layer-check
cargo build --release --target thumbv7em-none-eabihf --no-default-features \
  -p mmp-types -p mmp-crypto -p mmp-core -p mmp-router -p mmp-rs-code
```

Wire behavior is defined in `../../docs/spec/bundles.md` and
`../../docs/spec/sessions.md`; signature domains are defined in
`../../docs/spec/cryptography.md`. Ed25519 verification uses strict
verification. Mobile radio integration and physical BLE interoperability
remain release gates.
