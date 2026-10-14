# Terminology and clocks

Status: normative MMP/1 source

**Identity key.** A 32-byte Ed25519 public key. It is the stable cryptographic identity of a node. Applications may assign a local petname; a petname is never a protocol identifier.

**NodeID.** The 16-byte truncated BLAKE3-derived identifier used in destinations, claims, and local indexes. The full origin key remains in every signed bundle so a receiver can verify before storing.

**BundleID.** The first 16 bytes of BLAKE3 over the canonical unsigned bundle with the five lifetime bytes set to zero. A relay MUST use this value for deduplication and MUST NOT derive it from mutable lifetime or the signature.

**Contact.** A period in which two nodes can exchange a session over a transport. Contact duration and radio quality are local observations.

**Beacon.** The 24-byte unauthenticated BLE discovery record. It may contain an ephemeral identifier and summary hint. It does not prove identity.

**Relay.** A node that stores or forwards a bundle for another node. A relay can read routing metadata and, for ordinary class-1 bundles, the outer signature. It cannot read direct-message ciphertext without the conversation key.

**Monotonic time.** An injected elapsed-time source used for bundle aging, session timers, backoff, and storage expiry. It MUST NOT move backwards.

**Wall time.** A receiver-local clock used only for authority certificate `not_after` checks. MMP has no shared clock. Wall-clock rollback or skew MUST NOT affect bundle lifetime or causal ordering.

**Core, Standard, Anchor.** Deployment tiers with different local storage and memory budgets. They do not change the wire grammar. A profile MAY lower local policy values, but it MUST remain within interoperability bounds.

**Conforming node.** An implementation that accepts only the canonical encodings and validation rules in this directory and passes the applicable vector and state-machine tests.
