# Implementation documentation

The Rust workspace is under [`../../implementation/mesh-milk`](../../implementation/mesh-milk). It is organized by protocol responsibility rather than platform UI.

- [`architecture.md`](architecture.md) maps protocol layers to crates.
- [`rust-layout.md`](rust-layout.md) identifies the implementation of each normative subsystem.
- [`mobile-integration.md`](mobile-integration.md) records platform seams and unverified gates.
- [`test-strategy.md`](test-strategy.md) defines the evidence expected from codecs, crypto, state machines, and hardware.

The implementation follows current wire behavior where vectors and tested code select an interpretation. Where that behavior conflicts with an owner decision or a security requirement, the mismatch is recorded in [`../internal/decisions/open-protocol-decisions.md`](../internal/decisions/open-protocol-decisions.md) and in the release reboot report.
