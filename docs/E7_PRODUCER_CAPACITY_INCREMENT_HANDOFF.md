# E7 producer-capacity increment handoff

Status: bounded implementation increment; no wire or specification change.

## What changed

This continuation adds narrow uses of the existing receive-side
`BundleStore::try_put` seam:

- `NodeCore::publish_announcement` now stores through `try_store_own`.
- The sealed first-contact fallback (`send_sealed`) now stores through the
  same helper.
- The valid-prekey first-contact branch of `send_app` now builds a candidate
  conversation and sender ratchet, stores the complete bundle through
  `try_store_own`, and commits conversation/pending/DAG state only after the
  provider accepts it.

`try_store_own` inserts `SeenIndex` state and logs `JourneyEvent::Created` only
after the provider accepts the complete encoded bundle. A capacity rejection
maps to the existing local `SendError::ResourceLimit` and leaves the store,
seen index, diagnostics, conversation map, and pending-first-contact map
unchanged. Announcement and sealed producers perform no cryptographic state
mutation before the write; the prekey first-contact producer keeps its
conversation and sender-chain changes in local candidates until the write
commits. This narrow scope is safe without a new reservation/commit API.

The CLI `SharedSnapshot` adapter now delegates `try_put` instead of inheriting
the default infallible trait implementation. This prevents a future bounded
CLI snapshot configuration from silently bypassing `SnapshotStore`'s complete-
image admission check.

The FFI `set_nickname` method now propagates `Node::try_set_nickname` errors
through the existing throwing error surface. A post-revocation nickname no
longer returns success while publishing nothing.

## Evidence

From `implementation/mesh-milk`:

```powershell
cargo test -p mmp-node --lib -- --test-threads=1
cargo test -p mmp-cli --all-targets -- --test-threads=1
cargo test -p mmp-ffi --all-targets -- --test-threads=1
```

The focused runs passed with 120 node tests, 37 CLI tests, and 30 FFI tests.
New regressions cover:

1. sealed first-contact capacity rejection;
2. announcement capacity rejection;
3. bounded `SharedSnapshot::try_put` propagation; and
4. FFI nickname rejection after self-revocation; and
5. valid-prekey first-contact rejection with unchanged ratchet/conversation
   state and a successful retry after provider recovery.

## Explicit non-claims

This is not a fixed-capacity producer transaction and does not close E7-S1.
The following paths still use legacy or non-transactional producer behavior
and require separate staging work:

- constructor bootstrap claims keep a local discovery view when persistence is
  rejected, while legacy claim-returning wrappers swallow the provider error;
- receipts and other fire-and-forget auxiliary bundles;
- established-conversation direct `send_app` (its ratchet still advances
  before the legacy store path);
- group sender-key messages and distributions; and
- file symbols and manifests.

`Node::new` retains fixed bootstrap claims in its local discovery view even
when their provider write is rejected, but cannot report that persistence
failure through its source-compatible constructor. A complete producer fix
still needs provider reservation/commit semantics and a batch transaction for
file symbols. Do not replace all `store_own` calls with `try_put` until those
invariants are tested.

## Next implementation order

1. Stage the established-conversation direct `send_app` branch and prove
   sender-chain, DAG, SeenIndex, delivery, and event state are unchanged on
   provider rejection.
2. Design and review an additive provider reservation/commit capability;
   include outstanding reservations, replacement deltas, failed-commit
   recovery, and restart semantics.
3. Add candidate-state or batch semantics for group and file producers.
4. Select a board-specific allocator/flash policy, then implement the
   `no_std` provider and linked-image/HIL gates.

## Adversarial review

### Security auditor

**Potential flaw:** `try_put` is only a bounded write result; it is not an
atomic reservation across concurrent producers or auxiliary snapshot records.

**Resolution:** this increment is limited to producers with no pre-write
cryptographic state mutation, and the handoff makes no global quota claim.

### Distributed-systems reviewer

**Potential flaw:** a failed own-origin write could still be retried with a
different ephemeral sealed key, creating duplicate user-visible messages.

**Resolution:** the current API reports local resource failure; callers must
retry according to host policy. Exactly-once user semantics remain outside this
increment and require durable producer journals or idempotency policy.

### Performance engineer

**Potential flaw:** complete-image admission can reject a bundle because of
auxiliary snapshot state even when bundle bytes alone fit.

**Resolution:** that is intentional fail-closed behavior of
`SnapshotStore::try_put`; the CLI adapter now preserves the result instead of
discarding the capacity signal.

### Skeptical professor

**Potential flaw:** passing host tests could be mistaken for a board-backed
fixed-capacity implementation.

**Resolution:** no embedded backend, allocator policy, linked image, or HIL
evidence is claimed here.

No file under `specification/` was modified.
