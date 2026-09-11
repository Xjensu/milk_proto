# Engineering change record

This record lists durable technical changes without workflow history.

- Canonical bundle decoding, lifetime normalization, strict Ed25519 verification, and negative vectors were added to the Rust reference implementation.
- Noise XX sessions now bind the identity proof to the transcript and carry `ID_PROOF` in the encrypted transport.
- Direct first-contact sealed and prekey paths, forward ratchets, receipts, RBSR, binary Spray-and-Wait, Focus forwarding, per-origin quotas, and atomic persistence are implemented and tested.
- Authority certificate parsing and receiver-local priority clamping exist as an extension.
- Deterministic simulator sidecars cover energy algebra, contact schedules, loss-injected traces, and bounded resource metrics; they do not provide device energy or calibrated mobility evidence.
- Mobile FFI and SDK-free coordinator seams exist. Native Android/iOS behavior and physical BLE exchanges remain open.
- The current documentation reboot consolidates normative behavior under `docs/spec/`, separates extensions and research, and records implementation gaps explicitly.
