# MMP (mesh_milk)

MMP is a peer-to-peer, delay-tolerant messaging protocol for phones, PCs, and constrained devices. It uses BLE for discovery and contact, Noise XX for an authenticated encrypted session, signed bundles for store-carry-forward delivery, and bounded local routing state. Internet infrastructure is optional and outside the core protocol.

The first implementation milestone is a physical two-node exchange of an encrypted direct message and its receipt. The next milestone adds a relay contact between the origin and destination. The repository does not claim production readiness: physical BLE, mobile platform, embedded hardware, external cryptographic review, file confidentiality, group-control authorization, battery, and calibrated scale gates remain open.

## Documentation

- [`docs/spec/`](docs/spec/index.md) - normative MMP/1 protocol specification.
- [`docs/extensions/`](docs/extensions/README.md) - optional files, groups, friendship, and authority mechanisms.
- [`docs/research/`](docs/research/README.md) - prior art, models, experiments, failure analysis, and sources.
- [`docs/implementation/`](docs/implementation/README.md) - Rust architecture and platform integration.
- [`docs/deployment/`](docs/deployment/README.md) - deployment profiles and validation drills.
- [`docs/internal/`](docs/internal/README.md) - engineering state, decisions, audits, conformance, and release evidence.

## Build and test

```text
cd implementation/mesh-milk
cargo build --workspace --all-features --locked
cargo test --workspace --all-targets --all-features --locked -- --test-threads=1
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo run -p mmp-cli -- vectors
```

The implementation uses stable Rust with an MSRV declared in the workspace manifest. Platform-specific commands require the corresponding SDK and hardware. The current command results and environment limitations are recorded in [`docs/internal/release-evidence/implementation-state.md`](docs/internal/release-evidence/implementation-state.md).

## Implementation entry points

Start with `mmp-types` for canonical wire objects, `mmp-crypto` for identity and session cryptography, `mmp-core` for session and durable protocol state, `mmp-router` for bounded forwarding, and `mmp-node` for orchestration. Committed vectors are in `implementation/mesh-milk/vectors/`.

Contributions should update the relevant normative section, vectors, implementation tests, and the internal decision or release record when behavior is unresolved. Do not change wire or cryptographic semantics without a reviewed protocol decision.
