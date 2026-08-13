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
floor region. Tests cover canonical bytes, every-prefix truncation, trailing
data, invalid magic, reserved generations, output bounds, generation mismatch,
and a journal write/validated-scan round trip.

## Security and crash-recovery boundary

This is not a durable anti-rollback implementation. The flash-journal checksum
is unkeyed corruption detection, not authentication. The codec does not couple
the floor journal atomically to the snapshot journal. A board adapter must
therefore select only fully validated records, fail closed on `M != F`, reject
rollback attempts, and specify the independent rollback domain that protects
the floor region. It must not enable own-origin producer counters or claim
crash-atomic ratchet/SeenIndex continuity from this primitive alone.

The adversarial review found no defect in the bounded codec itself. It did find
that separate floor and snapshot regions have old/old, new-floor/old-snapshot,
old-floor/new-snapshot, and new/new power-loss states. Cross-region recovery
and exact-generation equality remain an implementation task, not a property
of the codec.

## Next implementation gate

Implement a named-board floor region and a host fault-injection model before
connecting producers. The gate must prove exact-generation selection,
fail-closed mismatch handling, torn-write recovery, duplicate-generation
handling, and no counter reuse after an indeterminate write. Keep the MMP/1
specification and wire format unchanged.
