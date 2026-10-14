# Authenticated sessions and frames

Status: normative MMP/1 source

The session uses `Noise_XX_25519_ChaChaPoly_BLAKE2s` for the authenticated transport handshake. The only pre-transport records are HELLO and Noise handshake records. After Noise establishes transport encryption, the first encrypted application record is `ID_PROOF`.

`ID_PROOF` carries the identity signature over the Noise handshake hash, both identity public keys, and the `mmp1/chanbind` domain tag. A peer MUST verify this record before accepting inventory or application frames. Older message-format text described `ID_PROOF` as a pre-channel parser input; the implementation and crypto rule resolve that ambiguity in favor of the encrypted record. The erratum is recorded in [`internal/decisions/open-protocol-decisions.md`](../internal/decisions/open-protocol-decisions.md).

The session envelope is:

```text
length:u16
type:u8
payload[length]
```

The payload is limited to 4096 bytes. Registered frames are HS, ID_PROOF, CREDIT, SUMMARY, RECON_INIT, RECON_RANGE, RECON_IDS, WANT, BUNDLE, BUDGET, BLOB_REQ, BLOB_HAVE, FRIEND_OFFER, FRIEND_ACK, PROXY_SUMMARY, ERROR, and CLOSE. Unknown types or impossible lengths produce `ERROR(unsupported)` followed by `CLOSE`.

After binding, each direction has a conversation root and a symmetric forward ratchet. Message keys are derived from the chain key; the chain advances after successful encryption or decryption. The skipped-key map is bounded by the configured window (512 in Standard policy, 32 in Core policy). A live Noise contact mixes fresh session material into the conversation epoch. This provides post-compromise recovery on contact, not between contacts.

Direct payload forms are:

```text
direct: conv:16, epoch:u16, index:u32, ciphertext:varbytes
async sealed: mode:0x00, ephemeral_x25519:32, ciphertext:varbytes
async prekey: mode:0x01, prekey_id:u16, sender_x25519:32,
             ephemeral_x25519:32, ciphertext:varbytes
```

The first-contact sealed and prekey paths establish the conversation root. A relay forwards their outer signed bundles but cannot open the ciphertext. Receipts are separate signed control bundles containing the delivered BundleID and a keyed MAC that only the conversation participants can verify.

Credits and budgets are session-local flow control. They are not global authorization. A sender MUST respect the peer's advertised receive credit and MUST NOT treat a lost session as evidence that data was accepted.
