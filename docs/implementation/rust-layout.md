# Code-to-specification map

| Protocol area | Main implementation | Evidence |
|---|---|---|
| Bundle and TLV | `mmp-types/src/bundle.rs`, `tlv.rs` | bundle and negative vectors |
| Claims and identity | `mmp-types/src/claim.rs`, `mmp-crypto/src/identity.rs`, `mmp-core/src/claims.rs` | claim vectors and claim tests |
| Noise and first contact | `mmp-crypto/src/noise.rs`, `prekey.rs`, `sealed.rs` | handshake and prekey tests |
| Ratchet and receipts | `mmp-crypto/src/ratchet.rs`, `receipt.rs` | crypto tests |
| Session frames | `mmp-types/src/frame.rs`, `mmp-core/src/session.rs` | frames and session tests |
| RBSR and indexes | `mmp-core/src/rbsr.rs`, `seen.rs`, `receiptfilter.rs` | RBSR and persistence tests |
| Routing | `mmp-router/src/lib.rs`, `utility.rs`, `quota.rs` | routing invariant tests |
| BLE | `mmp-tal-btleplug`, `mmp-tal-peripheral`, `mmp-tal-mobile` | codec tests; no physical evidence |
| Groups | `mmp-crypto/src/group.rs`, `mmp-node/src/group.rs` | group tests; control gap open |
| Blobs/files | `mmp-node/src/blob.rs`, `mmp-types/src/manifest.rs`, `mmp-crypto/src/merkle.rs`, `mmp-rs-code` | symbol tests; confidentiality gap open |
| Authority | `mmp-types/src/authcert.rs`, `mmp-core/src/authority.rs`, `mmp-node/src/authority_bucket.rs` | local validation tests |

The current subsystem status and wire layouts are maintained in [`../internal/release-evidence/implementation-state.md`](../internal/release-evidence/implementation-state.md).
