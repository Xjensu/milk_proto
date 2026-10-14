# Design review findings

The former review records identified the following engineering findings. They are stated here without process narration.

| Finding | Evidence | Disposition |
|---|---|---|
| `ID_PROOF` had conflicting placement descriptions | `mmp-crypto/src/noise.rs`, `mmp-core/src/session.rs`, former message-format text | Canonical behavior is encrypted transport; publish an erratum and independent vector |
| Receiver buckets do not provide global Byzantine copy conservation | `mmp-router/src/quota.rs`, copy-budget analysis | Keep local clamps and quotas; narrow the invariant |
| AEAD associated data must zero mutable lifetime | `Bundle::canonical_aead_ad()`, conformance vectors | Canonical implementation boundary is documented in `spec/bundles.md` |
| Group future epochs lack authenticated administrator control | group code and control audit | Keep extension partial; define anchor, floor, merge, and late-join rules |
| File symbols are relay-visible plaintext | `mmp-node/src/blob.rs`, `mmp-types/src/manifest.rs` | Require encrypt-before-erasure-coding; preserve implementation gap |
| BLE, battery, and large-scale claims lack physical or calibrated evidence | implementation-state report and simulator tests | Keep as release/research gates |
| Claim grammar differs for Friendship and avatar roots | Rust claim parser versus former text | Resolve through reviewed erratum before interop vectors |

These findings do not authorize silent wire or implementation changes.
