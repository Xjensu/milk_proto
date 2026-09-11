# MMP documentation

MMP is a delay-tolerant, peer-to-peer messaging protocol for intermittent BLE contacts. A node discovers nearby peers with a small beacon, establishes an authenticated encrypted session, exchanges inventory and data, and stores signed bundles for later forwarding. The protocol does not require an Internet service or a continuously connected overlay.

The normative MMP/1 specification is in [`spec/`](spec/index.md). Extensions are separated in [`extensions/`](extensions/README.md). Research notes describe rationale and experiments; they do not add wire requirements. Implementation and deployment notes describe the Rust workspace and the evidence needed for release.

## Repository layout

- [`spec/`](spec/index.md): one normative source for MMP/1.
- [`extensions/`](extensions/README.md): files, groups, friendship, and authority mechanisms outside the minimum direct-message milestone.
- [`research/`](research/README.md): prior art, models, hypotheses, failure analysis, and open questions.
- [`implementation/`](implementation/README.md): crate boundaries, integration guidance, and test strategy.
- [`deployment/`](deployment/README.md): profiles and field validation procedures.
- [`internal/`](internal/README.md): engineering handoffs, decisions, audits, conformance records, and release evidence.
- [`../implementation/mesh-milk/`](../implementation/mesh-milk/README.md): Rust workspace and committed vectors.

## Current maturity

The Rust workspace is a functional prototype. Canonical bundle and frame codecs, cryptographic routines, in-memory sessions, routing, persistence, and deterministic simulation have automated coverage. The minimum physical milestone remains open: no two-device BLE exchange with an end-to-end receipt has been recorded in this checkout. Android and iOS native radio integration, embedded hardware recovery, independent cryptographic review, whole-device energy measurements, calibrated large-scale mobility, and independent implementation interoperability remain release gates.

## Build and test

From `implementation/mesh-milk`:

```text
cargo build --workspace --all-features --locked
cargo test --workspace --all-targets --all-features --locked -- --test-threads=1
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo run -p mmp-cli -- vectors
```

The current Windows test evidence and environment failures are recorded in [`internal/release-evidence/implementation-state.md`](internal/release-evidence/implementation-state.md). The vector catalogue is in [`implementation/mesh-milk/vectors/README.md`](../implementation/mesh-milk/vectors/README.md).

## Known limits

File symbols are currently relay-visible plaintext; the specification requires file-level encryption before erasure coding and records the implementation gap. Group control accepts some future-epoch paths without the final authenticated administrator rule. Receiver-local copy quotas do not provide global Byzantine spend-once conservation. BLE mobile paths have no physical Android or iOS evidence. Battery and 1,000/10,000-node claims remain experimental. Authority certificates and announcements are extensions and do not block direct-message MMP/1.

Contributions should begin with [`spec/conformance.md`](spec/conformance.md), the implementation-state report, and the relevant decision record in `internal/decisions/`.
