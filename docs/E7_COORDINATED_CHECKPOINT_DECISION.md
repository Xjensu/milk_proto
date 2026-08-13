# E7 coordinated checkpoint decision

Status: **DEFERRED-WITH-REASON** (2026-08-13). No MMP/1 wire field or
specification document is changed by this decision.

## Decision

Do not ship the provisional `MCP1` bundle-plus-auxiliary container as a
producer checkpoint API. A bounded concatenation with magic, version, and
lengths is useful parser scaffolding, but it is not sufficient to preserve
replay, ratchet, or producer continuity across a crash.

The current production-safe boundary remains:

- `mmp-embedded-provider::JournaledFixedStore::commit_snapshot` for verified
  FBS1 bundle snapshots;
- `recover_with_durable_floor` for comparison against a separately persisted
  floor, or `recover_with_exact_durable_floor` when the caller requires exact
  checkpoint equality; and
- receive-only operation until auxiliary replay/cryptographic state shares the
  same durable commit boundary.

## Bounded floor-record primitive (2026-08-13)

`mmp-flash-journal` now provides a provider-neutral, allocation-free
`GenerationFloorRecord` codec for a board-owned floor region. Its canonical
payload is `GFL1 || generation_le_u64`; it rejects reserved generations,
malformed lengths, invalid markers, and any mismatch between the payload
generation and the enclosing journal generation. The codec is a structural
validation seam only. The journal checksum is unkeyed corruption detection,
not authentication, and the codec does not couple a floor write atomically to
the snapshot journal. A board adapter must therefore keep the floor in a
separately specified crash-safe region, select only fully validated records,
and fail closed on cross-region generation mismatches. The exact provider
helper enforces that equality after snapshot validation, but it does not write
or authenticate the floor. Until those policies are implemented and
fault-tested, this primitive must not be used to claim durable anti-rollback or
producer checkpoint continuity.

## Evidence and threat model

An attacker who can alter local journal bytes can recompute the unkeyed journal
checksum. Without an authenticated binding, valid auxiliary bytes can be
spliced onto another bundle snapshot or replayed under a different generation.
That can make durable bundle state disagree with SeenIndex, sender counters,
ratchet state, or pending producer state. The journal checksum detects random
corruption; it is not an authenticity mechanism.

The current `mmp-node::SnapshotVault` already defines the correct higher-level
shape for host/mobile secret state: an authenticated envelope, trusted clock,
provider-owned wrapped key, restore lock, and durable generation CAS. It is a
`std`/allocation-dependent seam and cannot be copied into the embedded crate
as-is. The embedded provider must receive already bounded, authenticated opaque
bytes and must not depend on the private `SnapshotVault` implementation.

## Required future envelope

Before producer integration, an additive provider-local envelope must include:

```text
MCP1
version
reserved flags (must be zero)
auxiliary schema/kind
inner generation
bundle snapshot length
auxiliary length
body digest
FBS1 snapshot bytes
authenticated/encrypted auxiliary bytes
```

The decoder must borrow caller-owned slices, reject unknown schema and flags,
zero/oversized lengths, truncation, trailing bytes, digest mismatch, and an
inner generation that differs from the outer journal generation. The outer
provider must validate FBS1 before publication and must never fall back to an
older generation after a selected snapshot fails validation.

The auxiliary authenticated-data context must commit at least:

```text
checkpoint domain || auxiliary schema || generation || digest(FBS1)
```

An unkeyed digest alone is not enough against an active local storage attacker.
Empty auxiliary state may be allowed only for an explicitly receive-only
schema; it must not imply producer continuity.

## Capacity and verification gates

The existing secret-state aggregate can reach 256 KiB and uses allocation,
while the embedded journal parser is bounded to 64 KiB and the board profile
has smaller slot limits. A bounded auxiliary schema and exact board capacity
must therefore be selected before implementation. Required tests are:

1. exact golden bytes and canonical roundtrip;
2. every-prefix truncation, trailing bytes, unknown schema, reserved flags,
   zero/overflow lengths, and output immutability;
3. inner/outer generation mismatch and durable-floor rollback;
4. replaying valid auxiliary bytes onto a different FBS1 snapshot;
5. restart and torn body/checksum/marker recovery;
6. provider rejection and indeterminate-write tests proving no counter reuse;
7. Cortex-M no-alloc checks and a named-board capacity/static-RAM report.

Until these gates are met, the implementation must not claim exactly-once
producer semantics, crash-atomic ratchets, or durable auxiliary-state
continuity. This is a deliberate security closure, not a protocol limitation
being hidden from the user.
