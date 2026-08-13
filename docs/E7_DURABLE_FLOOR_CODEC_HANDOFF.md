# E7 durable generation-floor codec handoff

Status: **FIXED** for the bounded codec; **DEFERRED-WITH-REASON** for
board-owned durable floor persistence and producer checkpoint integration
(2026-08-13).

## Delivered

`mmp-flash-journal` now exposes `GenerationFloorRecord`, a provider-neutral,
allocation-free structural codec with these exact rules:

```text
payload = ASCII "GFL1" || generation_le_u64
generation != 0 and generation != u64::MAX
payload length == 12 bytes
outer journal generation == inner payload generation
```

The API provides `encode`, `decode`, and `validate_record`. It is suitable for
passing to `scan_first_validated` and for a board adapter that owns a separate
floor region. The embedded provider now adds
`recover_with_exact_durable_floor`, and the nRF52840 profile exposes the
corresponding target-only wrapper. Exact recovery accepts only an equal,
non-zero generation, or the explicit first-install `0/0` case; both floor-ahead
and snapshot-ahead mismatches fail closed. Tests cover canonical bytes,
every-prefix truncation, trailing data, invalid magic, reserved generations,
output bounds, generation mismatch, journal integration, and exact-floor
recovery cases.

## Security and crash-recovery boundary

This is not a durable anti-rollback implementation. Exact recovery is a
comparison gate, not a transaction across two regions. The flash-journal checksum
is unkeyed corruption detection, not authentication. The codec does not couple
the floor journal atomically to the snapshot journal. A board adapter must
therefore select only fully validated records, fail closed on `M != F`, reject
rollback attempts, and specify the independent rollback domain that protects
the floor region. It must not enable own-origin producer counters or claim
crash-atomic ratchet/SeenIndex continuity from this primitive alone.

The adversarial review found no defect in the bounded codec or exact comparison
seam. It did find
that separate floor and snapshot regions have old/old, new-floor/old-snapshot,
old-floor/new-snapshot, and new/new power-loss states. The provider comparison
seam now rejects cross-region generation mismatch, but it does not resolve
those power-loss states or make equality a property of the codec.

The host provider test matrix now models a separate two-slot GFL1 authority. It
covers all four ordering states, missing/uninitialized authority, body/checksum/
marker corruption, duplicate generations, malformed higher records, and
inner/outer generation mismatch. Invalid authority is never converted to the
scalar floor `0`. The old/old case intentionally passes equality to document
the residual limitation: jointly rolled-back regions require an independent
rollback-resistant authority and are not detected by this comparison seam.

## Next implementation gate

The host fault-injection matrix is complete. The next gate is a named-board
floor region plus hardware/power-loss fault evidence before connecting
producers. That gate must prove exact-generation selection, fail-closed
mismatch handling, torn-write recovery, duplicate-generation handling, and no
counter reuse after an indeterminate write. Keep the MMP/1 specification and
wire format unchanged.

The nRF52840 profile now supplies an address-parametric
`DurableFloorConfig`/`validate_with_floor_region` geometry gate. It requires two
independently erase-aligned floor slots, checks their padded GFL1 capacity and
disjointness from the snapshot region, and remains explicit that linker
reservation, authentication, and power-loss atomicity are still external
hardware gates.
