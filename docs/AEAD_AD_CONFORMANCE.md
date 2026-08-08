# AEAD Associated-Data Conformance Note

Status: implementation alignment; the frozen specification is unchanged.

## Finding

The supplied MMP/Φ review correctly identified a real gap between the normative
message-format rule and the previous Rust path. `specification/10-message-format.md`
§4.1 requires class-1 ciphertext to use the bundle header as AEAD associated
data, and its security review clarifies that the same lifetime-zeroed canonical
form is used. The previous implementation used only `conv_id` for direct/async
messages and `gid` for group messages. Outer Ed25519/member signatures limited
the immediate exploitability, but header fields such as destination, flags,
priority, extensions, and payload length were not independently committed.

## Executable boundary

`mmp-types::Bundle::canonical_aead_ad()` now returns exactly:

```text
fixed bundle header + canonical TLVs + payload_len(u16)
```

The payload and outer signature are excluded. The five mutable lifetime bytes
are zeroed before returning, just as for `BundleID` and the bundle signature.
This keeps relay lifetime decrementing compatible with end-to-end decryption
while binding all other header fields.

## Callers

- class-1 direct and async-first-contact send/open paths use the canonical AD;
- class-2 group sender-key encryption and decryption use the same outer-header
  AD, while retaining the existing group context commitment and member
  signature;
- the old low-level group crypto helper remains available for tests and other
  callers that explicitly provide their own AD.

Senders construct a same-length placeholder ciphertext first so `payload_len`
is present when the AD is computed; the real ciphertext is then inserted and
the outer signature is generated over the normal lifetime-zeroed bundle form.

## Verification

`mmp-types/tests/aead_ad.rs` proves that live lifetime, payload, and signature
changes do not enter AD, while destination and priority changes do. Existing
node direct/group delivery tests and the three-node store-carry-forward test
exercise the end-to-end callers, including relay lifetime mutation. No wire
specification file was edited.

The repository's opaque direct-message vector remains intentionally opaque: it
does not contain key material or an AEAD transcript. A future independent
interoperability vector should include the canonical AD bytes and ciphertext
once the project decides to publish a stable test key fixture.
