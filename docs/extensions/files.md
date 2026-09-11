# File transfer extension

The file extension uses a signed `FileManifest` and class-4 symbol bundles. The current manifest carries suite, sharer key, entries, file root, Reed-Solomon parameters, full hash, flags, and an optional thumbnail root. The reference profile caps one file at 8 MiB, one album at 16 files, names at 255 bytes, MIME strings at 64 bytes, thumbnails at 32 KiB, and symbols at 2048 bytes. A stripe has 32 data and 8 parity symbols.

The required confidentiality pipeline is:

```text
file plaintext -> file-level authenticated encryption -> ciphertext
    -> Reed-Solomon symbols -> Merkle-authenticated symbols -> relay storage
```

The file key and nonce derivation, manifest binding, and symbol associated data MUST be specified together before this requirement is called interoperable. The reference implementation currently encrypts the manifest inside the direct-message channel but applies Reed-Solomon and Merkle processing to plaintext file data. Relay nodes can therefore read file symbols. That is an implementation gap, not a permitted interpretation of this extension.

Every received symbol MUST pass canonical decode, origin signature verification, manifest binding, Reed-Solomon parameter checks, and Merkle membership verification before storage. A relay MAY reconstruct a stripe from verified symbols, but it MUST NOT serve a symbol that has not passed those checks. The current proof form is a depth-prefixed Merkle path followed by a fixed 2048-byte symbol.

The extension has a pull-oriented state machine and a seven-day local assembly-interest TTL. It does not provide a delivery guarantee. Loss, quota, and contact scheduling remain local policy decisions.
