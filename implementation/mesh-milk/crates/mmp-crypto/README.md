# mmp-crypto

Cipher suite 0x01 (doc 08): Ed25519 (`verify_strict` only — canonical-S, AUDIT F9),
X25519, ChaCha20-Poly1305, BLAKE3, Noise XX with ID_PROOF channel binding, X3DH-style
serverless prekeys, the opportunistic double-ratchet-lite, sender-keys groups, and
sealed boxes. Secrets zeroize on drop; RNG only via an injected `CryptoRngCore`.

**Layering:** depends only on `mmp-types`. `#![cfg_attr(not(feature = "std"), no_std)]`
— the `std` feature (on by default) gates the `snow`-backed Noise session layer;
everything else is `no_std`+`alloc`. Downstream crates that need the embedded profile
depend on this crate with `default-features = false`.

See `docs/implementation/README.md` §2.4 and `docs/spec/cryptography.md`.
