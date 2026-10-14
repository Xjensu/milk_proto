# Golden-vector conformance record

Status: engineering record; current requirements are in
[`docs/spec/conformance.md`](../../spec/conformance.md).

The committed vector catalogue covers positive and negative cases for beacons,
envelopes, bundles, claims, frames, receipts, prekey agreement, RBSR
fingerprints, and coverage metadata. The Rust generator constructs deterministic
objects from public fixtures. The conformance reader parses committed bytes
independently of the production decoder and checks canonical re-encoding,
strict rejection, signature domains, lifetime handling, and size limits.

The catalogue is checked by `mmp-cli` tests. Every entry has an identifier,
artifact name, source path, and evidence label in
`implementation/mesh-milk/vectors/coverage.json`. Any source path moved during
the documentation reboot was updated in the manifest and the reader's source
prefix check; wire bytes and vector payloads were not changed.

Remaining evidence gaps are independent non-Rust readers, sanitizer fuzz runs,
physical BLE interoperability, mobile platform validation, and external
cryptographic review. These gates are tracked in
[`docs/internal/handoff/release-gates.md`](../handoff/release-gates.md).
