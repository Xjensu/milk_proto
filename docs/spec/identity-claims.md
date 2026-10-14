# Identity and claims

Status: normative MMP/1 source

An implementation MUST generate an Ed25519 identity key pair from a cryptographically secure random source. The public key is carried inline in every origin-signed bundle. The private key MUST be protected by the platform keystore or an equivalent local mechanism on phones and PCs. `no_std` targets use their hardware-protected storage boundary where available.

The NodeID derivation is stable for the identity key and is used for compact routing fields. A receiver MUST bind the inline origin key to the derived NodeID before accepting a destination match or claim update.

Claims are signed last-writer-wins records:

```text
owner:32-byte Ed25519 key
field:u8
sequence:u32 little-endian
body:variable bytes
signature:64-byte Ed25519 signature
```

The signing domain includes a context tag and the canonical claim bytes. A receiver MUST verify the signature, owner binding, field grammar, and sequence policy before merge. Equal or lower sequence values do not replace an adopted value. A revocation creates a local tombstone for the owner; local publication after an adopted revocation MUST fail without producing a signature or bundle.

Recognized bodies include exchange keys, signed prekeys, one-time prekey batches, nicknames, friendship records, group metadata, avatars, and revocations. A nickname is UTF-8 and at most 32 bytes. Unknown fields remain opaque and may be relayed when their outer claim is valid. The avatar body and the friendship body still have conformance decisions recorded in [`internal/conformance/open-claim-conformance.md`](../internal/conformance/open-claim-conformance.md); implementations MUST use the current vector grammar until an erratum is approved.

The protocol does not provide global nickname uniqueness, anonymity, or automatic key rotation. Applications SHOULD expose the identity key and local verification state to users.
