# Protocol model

Status: normative MMP/1 source

An MMP node has five logical planes:

1. Discovery emits and receives BLE beacons.
2. The transport abstraction creates and closes links.
3. The authenticated session performs inventory reconciliation and transfer.
4. The durable store retains verified bundles, claims, receipts, and local indexes.
5. The router chooses which retained data to offer within local quotas.

The normal contact sequence is:

```text
beacon hint -> link admission -> HELLO -> Noise XX -> encrypted ID_PROOF
-> CREDIT -> SUMMARY -> RBSR reconciliation -> WANT/BUDGET -> BUNDLE or blob frames
-> receipt handling -> CLOSE
```

The sequence is restartable. A node MUST treat a link loss as a partial contact, preserve successfully verified durable state, and discard uncommitted session state. Transfers are idempotent by BundleID and symbol identity.

The first implementation milestone is a physical two-node exchange of one encrypted direct message and its receipt. The next milestone adds a relay that stores the bundle while the origin and destination are not simultaneously present. Neither milestone requires groups, file transfer, or authority announcements.

Discovery and data have separate trust boundaries. Discovery data MAY be stale or forged. After a link is admitted, all state that can cause storage, forwarding, deletion, or identity attribution MUST come from authenticated session frames or a valid signed bundle.

Unknown frame types, malformed lengths, reserved bits, invalid canonical encodings, failed signatures, and failed AEAD checks are errors. A session MUST fail closed and MUST NOT retain attacker-controlled partial state after an error.
