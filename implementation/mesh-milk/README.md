# mesh-milk — MMP/1 reference implementation

Rust workspace for the **mesh_milk (MMP/1)** peer-to-peer BLE mesh protocol.
The specification suite lives in `../../specification/` (docs 01–15); this
workspace implements it per `../../specification/12-rust-blueprint.md`.

**Where the project actually is:** the roadmap and backlog are the
authoritative status trackers, not this file —
`../../docs/ROADMAP_TO_PRODUCTION.md` (phases A–E, with per-phase gap
analysis and exit gates) and `../../docs/AGILE_BACKLOG.md` (story-level
execution plan). `../../ROADMAP.md` is a thin pointer mapping the legacy
"Stage N" vocabulary some spec docs still cite onto that phase structure.
`../../docs/IMPLEMENTATION_HANDBOOK.md` has the golden rules (§0) binding on
every change and the buildable specs for remaining work (§4).

As of 2026-07-16 the workspace is well past the original "Stage 0 scaffold"
point this README used to describe: all 15 crates plus `mmp-tal-peripheral`
have working implementations, `cargo test --workspace` is green (~90 tests
across the workspace, 1 intentionally `#[ignore]`d 100-node stress scenario),
`cargo clippy --workspace --all-targets -- -D warnings` is clean, and
`layer-check` passes. Phase A ("first radio contact") is code-complete and
unit-tested; its exit gate — a real two-machine BLE exchange — is the one
item that needs physical hardware to verify. See
`../../docs/ROADMAP_TO_PRODUCTION.md` §1 for the full per-crate breakdown and
§2 for what's still open.

## Layout (normative: doc 12 §1)

15 crates under `crates/` (plus `mmp-tal-peripheral`, added for Phase A's
peripheral-role BLE) + `vectors/` (committed golden wire/conformance vectors
and `coverage.json`, see `vectors/README.md`) + `tools/layer-check` (the CI
layer-dependency test).

Layering rule: `mmp-types ← mmp-crypto ← mmp-core ← mmp-router` are `#![no_std]`
(alloc-optional via the `alloc` feature); nothing below `mmp-node` may depend on
tokio, std time, or any TAL implementation. Enforced by `cargo run -p layer-check`
and CI (`../../.github/workflows/ci.yml` — note the workflow lives at the repo
root, not under this directory, because that's the only place GitHub Actions
discovers workflow files).

## Verify locally

```sh
cargo build --workspace --all-features
cargo test  --workspace --all-features
cargo clippy --workspace --all-targets -- -D warnings
cargo run   -p layer-check
cargo build --release --target thumbv7em-none-eabihf \
  -p mmp-types -p mmp-crypto -p mmp-core -p mmp-router -p mmp-rs-code
```

Wire truth is doc 10; every signature is domain-separated (doc 10 §3.1);
Ed25519 verification is `verify_strict` only (AUDIT F9). License is
provisional (`MIT OR Apache-2.0`) pending a project decision.
