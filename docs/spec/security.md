# Security model and limits

Status: normative MMP/1 source

The primary adversary is local and active: it can inject, replay, reorder, suppress, and selectively forward radio traffic; observe BLE metadata; operate many pseudonymous identities; and read relay-visible bundle metadata. It cannot break Ed25519, X25519, ChaCha20-Poly1305, or BLAKE3 under their stated assumptions. RF jamming, compromised operating systems, and a stolen identity private key are outside the protocol's prevention boundary.

The main security invariants are:

- A receiver verifies canonical encoding, origin signature, and local policy before store or forward.
- Origin key material is carried inline, so verification does not depend on prior claim gossip.
- Mutable lifetime is excluded from BundleID, signature input, and encrypted associated data.
- Session identity is bound to the Noise transcript by an encrypted ID_PROOF.
- Relay nodes cannot open direct-message ciphertext.
- Local copy clamps, quotas, and token buckets limit receiver admission; they do not create global Byzantine conservation.
- Bloom and beacon data are hints and cannot authorize a durable action.

Pseudonymity is distinct from anonymity. Contact timing, approximate presence, destination fields, origin keys, and bundle size remain metadata. First-contact trust may still be vulnerable to a key-substitution or social-engineering attack until users verify a key out of band. Opportunistic ratchet recovery occurs only when peers meet. Group epoch authorization, file symbol confidentiality, and external review of the non-standard cryptographic compositions remain open.

An implementation MUST expose the validation status of its platform and test evidence. It MUST NOT describe the MMP/1 prototype as production-ready without physical BLE, platform keystore, cross-implementation, and security-review evidence.
