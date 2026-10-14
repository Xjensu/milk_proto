# Design basis

MMP combines established delay-tolerant networking with BLE transport constraints. RFC 9171 supplies the bundle and lifetime model. Spray-and-Wait and PRoPHET provide prior art for bounded replication and contact-based utility, but MMP does not claim PRoPHET's transitive history is cryptographically trustworthy. Noise XX, X3DH, Ed25519, X25519, ChaCha20-Poly1305, BLAKE3, Reed-Solomon, and Merkle proofs are used as documented primitives; the channel-binding and serverless prekey compositions require review.

The main engineering choices are:

- verify origin signatures before storage so relays do not need prior identity gossip;
- keep beacon data unauthenticated and cheap, then gate durable actions on the encrypted session;
- use bounded local quotas and copy budgets instead of epidemic flooding;
- use range reconciliation and suppress-only Bloom hints to keep short contacts bounded;
- separate monotonic bundle/session timing from receiver-local authority expiry;
- leave authority, groups, and file transfer outside the first direct-message milestone.

These choices address observed BLE background and metadata constraints. They do not prove delivery, anonymity, battery, or large-scale performance. Source identifiers and URLs are maintained in [`bibliography.md`](bibliography.md).
