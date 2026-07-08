# mesh-milk — MMP/1 reference implementation

Rust workspace for the **mesh_milk (MMP/1)** peer-to-peer BLE mesh protocol.
The specification suite lives in `../../specification/` (docs 01–18);
this workspace implements it per `../../specification/12-rust-blueprint.md` and the staged,
gated plan in `../../ROADMAP.md`.

**Current stage: 0 — workspace scaffold & CI skeleton.** All crates are
intentionally empty; the layering rules are mechanical from the first commit.

## Layout (normative: doc 12 §1)

15 crates under `crates/` + `vectors/` (golden test vectors, populated at Stage 1)
+ `tools/layer-check` (the CI layer-dependency test).

Layering rule: `mmp-types ← mmp-crypto ← mmp-core ← mmp-router` are `#![no_std]`
(alloc-optional via the `alloc` feature); nothing below `mmp-node` may depend on
tokio, std time, or any TAL implementation. Enforced by `cargo run -p layer-check`
and CI.

## Verify locally

```sh
cargo build --workspace --all-features
cargo test  --workspace --all-features
cargo run   -p layer-check
cargo build --release --target thumbv7em-none-eabihf \
  -p mmp-types -p mmp-crypto -p mmp-core -p mmp-router -p mmp-rs-code
```

## Stage gates (ROADMAP)

Stage 1 (`mmp-types` codec + golden vectors + fuzzing) may begin only when
Stage 0's exit criteria hold: empty crates compile on all six CI targets;
layer test and size gate run in CI. Wire truth is doc 10; every signature is
domain-separated (doc 10 §3.1); Ed25519 verification is `verify_strict` only
(AUDIT F9). License is provisional (`MIT OR Apache-2.0`) pending a project decision.
