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

## Beta harness observability

Groups expose both the text convenience method and `group_send_body`, which
retains the encrypted application-type byte in the corresponding
`FfiEvent::GroupMessage` event. This is an API extension over the existing
group payload and does not change the frozen MMP/1 grammar.

`MmpNode::status()` returns a coarse, secret-free snapshot containing the
selected tier, persistence availability, diagnostics opt-in, and the latest
accepted monotonic tick. `MmpNode::metrics()` returns local store, callback
backpressure, and diagnostics counters; persistence failures remain available
through the existing `last_persist_error()` query. These snapshots
contain no peer/session identifiers, wire bytes, provider handles, or key
material; hosts decide whether to display or explicitly export them.

Diagnostics recording is opt-in. `enable_diagnostics()` starts a local journey
window and `disable_diagnostics()` stops recording new events while retaining
the already collected dump for inspection.

The original `link_up(link, initiator)` remains available and uses the
conservative BLE default. Hosts with a measured transport can call
`link_up_with_quality(link, initiator, FfiLinkQuality { mtu, goodput_bps })`;
the values are validated locally and are scheduling hints only. Native adapter
bindings must regenerate from the UniFFI definition before using this method.
