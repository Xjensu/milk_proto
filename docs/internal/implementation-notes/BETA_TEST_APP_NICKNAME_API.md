# Beta-test app nickname query

Status: additive host API note, 2026-08-12.

`mmp-ffi::MmpNode::nickname_of(peer)` exposes the display nickname already
held in the node's verified claim store:

```text
nickname_of(peer_id: Vec<u8>) -> Result<Option<String>, FfiError>
```

The peer identifier must be exactly 16 bytes. `Some` is returned only when a
locally stored nickname claim decodes as UTF-8; `None` means that no usable
claim is currently known. It does not imply that the peer is offline or that
the peer has no nickname globally. A malformed identifier is rejected with
`BadArg` before the node state is accessed.

This is a read-only projection of existing `mmp-node::Node::nickname_of` state.
It does not add a wire field, perform a lookup, or change trust decisions.
Hosts should treat the result as a display hint and continue to identify peers
by their stable NodeID. Raw claim bytes, signatures, and key material remain
outside the returned value.

Native Kotlin/Swift bindings must be regenerated from the UniFFI definition
before calling this method from a platform app. No platform radio, background
execution, persistence, or delivery guarantee is implied by this API.
