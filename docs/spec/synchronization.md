# Synchronization and durable indexes

Status: normative MMP/1 source

MMP reconciles inventories with a bounded range-based set reconciliation procedure (RBSR). A domain is identified by class and destination scope. The exchange starts with a SUMMARY hint, then RECON_INIT, RECON_RANGE, and either RECON_IDS or a terminal list. A range record is:

```text
lo:id16, hi:id16, count:u32 little-endian, fingerprint:u64 little-endian
```

The fingerprint is a BLAKE3-derived fold of the IDs and the count. A reply flag is carried in the high bit of the domain. The terminal threshold is 24 entries and the subdivision factor is 16 in the current reference policy. These are bounded implementation parameters, not delivery guarantees.

SUMMARY data is suppress-only hint data. Bloom false positives may suppress an offer for a contact, but they MUST NOT authorize deletion or storage. Exact BundleIDs and receipt MACs drive durable decisions.

`SeenIndex` prevents repeated admission of the same BundleID over the retention horizon. `ReceiptFilter` tracks delivered IDs in twelve six-hour buckets. Both structures rotate monotonically and are persisted with checksums. A loader MUST reject malformed, duplicate singleton, stale, or unverifiable records rather than selecting attacker-controlled state.

The durable store MUST apply the same validation used on live ingest during startup rebuild. A symbol is servable only after its canonical payload, file metadata, origin signature, and Merkle membership have been verified. Storage quotas are enforced before accepting new bytes; a failed persistence transaction MUST preserve the last complete snapshot.

Claim state uses sequence ordering rather than wall-clock ordering. Causal parent IDs in the Parents TLV form a compact Merkle-DAG hint for application ordering. Missing parents do not block relay of a valid bundle; applications request or reconcile them separately.

An implementation MUST make duplicate transfer safe. Replaying a valid bundle or symbol can consume local CPU and radio but cannot create a second durable object or bypass origin, size, lifetime, quota, or Merkle checks.
