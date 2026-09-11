# Cryptographic profile

Status: normative MMP/1 source

The mandatory suite is:

| Function | Algorithm | Use |
|---|---|---|
| Identity signatures | Ed25519 with strict canonical-S verification | identity, claims, bundles, group members |
| Key agreement | X25519 | Noise, first contact, prekeys |
| AEAD | ChaCha20-Poly1305 IETF | direct and group ciphertext |
| Application hash/KDF | BLAKE3 | BundleID, domains, ratchets, receipts, Merkle nodes |
| Session handshake | Noise XX with ChaChaPoly and BLAKE2s | authenticated transport |

Implementations MUST use a CSPRNG, zeroize secret material when it leaves scope, and use the signing-domain function for every Ed25519 signature. `verify_strict` semantics are required; signatures with non-canonical S values MUST be rejected. The negative vector `noncanonical_s_negative.json` covers this rule.

The identity key signs the exchange key and prekey claims. First contact may use a sealed X25519 payload or an adapted prekey batch. Published one-time prekeys have no online server enforcing one-time use. A relay may replay-consume or suppress fresh prekey claims; this can reduce first-message forward secrecy toward the sealed baseline but does not enable forgery. The construction requires independent cryptographic review.

Conversation chains provide forward secrecy within an epoch. A live authenticated contact adds session material to the epoch. Phones and PCs may persist chain state only in platform-keystore-wrapped storage; Core targets keep it in RAM. The conversation root is not part of the persisted state. If the keystore boundary is unavailable, an implementation MUST decline to persist ratchet state.

Suite `0x02` reserves a future X25519 plus ML-KEM hybrid. It is not implemented. A node MUST reject a handshake that requires an unsupported suite and MUST NOT silently downgrade from a mutually required suite.

These are project-specific compositions of standard primitives. Their security properties are claims for review, not a substitute for an independent proof or audit.
