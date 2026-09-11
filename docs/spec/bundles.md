# Bundles and canonical wire format

Status: normative MMP/1 source

The bundle is the unit of storage and forwarding. All integer fields are little-endian unless stated otherwise. A bundle is encoded as:

```text
ver_class:u8
flags:u8
origin:32
dest:16
lifetime_ms:u40
priority:u8
extensions_len:u16
extensions
payload_len:u16
payload
signature:64
```

`ver_class` contains wire version 1 in the high nibble and class 0 through 5 in the low nibble. The maximum encoded size is 4096 bytes. Extensions are ascending TLVs (`type:u8`, `length:u16`, `value`) and the `HAS_PARENTS` flag MUST match the presence of the Parents TLV. Reserved flag bits and unknown critical TLVs MUST be rejected.

Classes are control (0), direct message (1), group message (2), claim (3), symbol (4), and authority announcement (5). Class 5 is an extension and does not block the core milestone.

`lifetime_ms` is remaining lifetime. A relay MAY decrement it according to monotonic elapsed time and MUST discard an expired bundle. The field is zeroed when computing the BundleID and origin signature. This prevents a relay's lifetime update from changing identity or signature input. Receive-side maximums are six hours for class 0, 72 hours for classes 1, 2, 4, and unknown classes, and seven days for class 3. A receiver MUST clamp or reject a larger value according to the class policy.

`BundleID = BLAKE3(canonical_unsigned_bytes_with_zeroed_lifetime)[0..16]`. The signature domain is `mmp1/bundle` with the same zeroed canonical bytes. A receiver MUST decode canonically, verify the origin signature with strict Ed25519 checks, verify the NodeID binding, check lifetime and quota policy, and only then store or forward.

For encrypted class-1 and class-2 payloads, associated data is the canonical header through `payload_len`, with lifetime zeroed. Payload and trailing signature are excluded. The implementation boundary is `Bundle::canonical_aead_ad()`.

The current reference layout and negative cases are pinned by `bundle.json`, `bundles.json`, `bundle_tlv_negative.json`, `reserved_bits_negative.json`, `lifetime_reset_negative.json`, and `noncanonical_s_negative.json` in the vector directory.
