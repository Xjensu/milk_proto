# Conformance and evidence

Status: normative MMP/1 source

Conformance has four layers.

1. **Wire codec.** Decode and re-encode every positive vector byte-for-byte. Reject oversize, truncated, reserved-bit, non-canonical, lifetime-reset, and cross-domain cases.
2. **Cryptography.** Verify strict Ed25519, Noise channel binding, AEAD associated data, ratchet advancement, first-contact forms, receipt MACs, and Merkle membership.
3. **State machine.** Exercise legal and illegal frame ordering, timer expiry, link loss, retry, credit exhaustion, duplicate transfer, and persistence recovery.
4. **Transport and deployment.** Run real BLE contacts, platform background and keystore tests, embedded flash recovery, and independent implementation interop.

Committed vectors live in `implementation/mesh-milk/vectors/`. `coverage.json` maps artifacts to codec and specification evidence. The vector generator is:

```text
cargo run -p mmp-cli -- vectors
```

Current automated evidence includes 898 Rust tests in the recorded split, 261 Python tests with 10 platform or artifact skips, deterministic fuzz seed validation, vector drift checks, and a layer check. No sanitizer fuzz campaign completed on Windows. No physical BLE session was recorded. The exact result and environment limitations are in [`../internal/release-evidence/implementation-state.md`](../internal/release-evidence/implementation-state.md).

A release candidate MUST publish the vector revision, test command, toolchain, platform, and retained artifact for every claimed gate. A simulation result MUST identify its mobility model, seed, workload, and confidence interval. A target, estimate, or research parameter MUST NOT be presented as an observed delivery or battery result.
