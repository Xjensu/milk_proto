# E7-S1 fixed-capacity store: reservation and commit seam

Status: implementation design, 2026-08-09. This document is deliberately
limited to the Rust storage and node integration boundary. It does not change
the MMP wire format, cryptographic primitives, or any file in `specification/`.

## 1. Problem statement

The current `mmp_core::BundleStore` API remains the compatibility surface:

```rust
fn put(&mut self, meta: BundleMeta, bytes: &[u8]);
fn try_put(
    &mut self,
    meta: BundleMeta,
    bytes: &[u8],
) -> Result<StorePutOutcome, StorePutError>;
```

`put` cannot report a full fixed-capacity store. The additive `try_put` seam now
handles receive-side admission, while the current `BundleStore` trait also
contains provider-neutral batch and reservation methods for producers. The
seams are implemented by the host `SnapshotStore` and the in-memory reference
store, but no producer has yet staged every cryptographic/index state transition
around them. Calling a reservation method from an own-origin path without
staging state would still leave pairwise/group ratchets, conversation state, or
file indexes advanced when a provider rejects a write.

## 2. Implemented additive API (current tree)

The transactional seam is additive on `mmp_core::BundleStore`; there is no
second `TransactionalBundleStore` trait to implement. The current methods are:

```rust
fn try_put_batch(
    &mut self,
    entries: &[StoreBatchEntry<'_>],
) -> Result<Vec<StorePutOutcome>, StoreBatchError>;
fn try_reserve(&mut self, meta: BundleMeta)
    -> Result<StoreReservation, StoreReserveError>;
fn try_reserve_batch(&mut self, metas: &[BundleMeta])
    -> Result<Vec<StoreReservation>, StoreReserveError>;
fn try_reserve_batch_atomic(&mut self, metas: &[BundleMeta])
    -> Result<StoreBatchReservation, StoreReserveError>;
fn commit_reserved(
    &mut self,
    reservation: &StoreReservation,
    meta: BundleMeta,
    bytes: &[u8],
) -> Result<StorePutOutcome, StoreCommitError>;
fn commit_reserved_batch(
    &mut self,
    batch: &StoreBatchReservation,
    entries: &[StoreBatchEntry<'_>],
) -> Result<Vec<StorePutOutcome>, StoreCommitError>;
fn abort_reservation(&mut self, reservation: &StoreReservation)
    -> Result<(), StoreAbortError>;
fn abort_reserved_batch(&mut self, batch: &StoreBatchReservation)
    -> Result<(), StoreAbortError>;
```

`StoreReservation` and `StoreBatchReservation` are provider-local,
runtime-only tokens. Their fields are private; `from_parts` exists for external
provider adapters, which must still validate the provider namespace, nonce,
ID, declared size, and pending state before commit or abort. Tokens are not
serialized and are deliberately absent from `SnapshotStore` persistence.

The implemented contract is:

* `StoreReservation` is opaque, process-local, non-wire state. It is not a
  capability that a peer can request or forge, and it is not persisted across a
  restart.
* `try_reserve` accounts for `BundleMeta::size`, metadata overhead, replacement
  delta, and all currently outstanding reservations. The commit path then
  checks the actual `bytes.len()` against that declaration.
* `commit_reserved` checks the token, ID, declared size, and actual
  `bytes.len()` before publishing. It never evicts an existing own-origin
  bundle as a hidden side effect.
* A successful commit consumes the reservation exactly once. A failed commit
  either leaves the reservation abortable or returns an explicit unavailable
  outcome; it must never report failure after publishing a partial bundle.
* `try_put_batch` validates the complete request and publishes it atomically at
  the provider-call working-set level; it makes no crash-recovery or
  unknown-outcome claim.
* `try_reserve_batch` preflights several independent reservations. Each token
  can be committed or aborted separately, so this is not an all-or-none write.
* `try_reserve_batch_atomic` creates one ordered all-or-nothing batch token;
  `commit_reserved_batch` validates count, order, IDs, declared sizes, and
  capacity before any entry becomes visible.
* A provider that cannot prove these properties must leave the methods at their
  fail-closed defaults and use the legacy `put`/`try_put` surface. The current
  host implementation proves provider-call atomicity only; it is not a
  fixed-capacity no-`alloc` flash provider.

### 2.1 Batch semantics

`send_file` creates symbols and a manifest. Per-bundle reservations are not
enough to make that operation all-or-none, so the implemented atomic batch
capability uses the same opaque-token model:

```rust
let batch = store.try_reserve_batch_atomic(&metas)?;
let outcomes = store.commit_reserved_batch(&batch, &entries)?;
store.abort_reserved_batch(&batch)?; // idempotent if commit did not consume it
```

The provider must preflight the aggregate replacement delta before any write
becomes visible. `try_reserve_batch` and repeated `commit_reserved` calls remain
valid for independently recoverable records, but file producers use the
atomic form when the provider advertises it. The host implementation wires
both slice-backed and reader-backed file publication to this capability; a
provider returning `Unsupported` deliberately retains the legacy
accepted-prefix fallback. A fixed-capacity board backend and crash-recovery
integration are still open.

## 3. Capacity policy inputs

Capacity is a board/provider decision, not a protocol constant. A fixed store
configuration must name at least:

| Input | Required meaning |
| --- | --- |
| `max_live_bundles` | Maximum number of bundle slots, including reserved slots. |
| `max_bundle_bytes` | Maximum encoded bundle accepted by one slot. |
| `max_bundle_bytes_total` | Aggregate encoded payload budget for live bundles. |
| `max_reserved_slots` / `max_reserved_bytes` | Space withheld from concurrent reservations. |
| `metadata_bytes_per_slot` | Fixed header/index cost on the target backend. |
| `auxiliary_bytes` | Claims, group blobs, SeenIndex, ReceiptFilter, authority, and blocklist budget, if stored by that backend. |
| `own_origin_policy` | Own originals are pinned until expiry; they are never silently evicted to admit a relay bundle. |
| `eviction_policy` | Which non-own classes may be evicted, and whether eviction happens before or outside a reservation. |

The provider must perform arithmetic with checked or saturating operations and
reject an unrepresentable request. The reservation check is:

```text
projected_used = used_bytes
               - bytes_of_existing_id_if_replaced
               + requested_bytes
               + metadata_delta
               + outstanding_reservations

accept iff:
    requested_bytes <= max_bundle_bytes
    live_slots_after <= max_live_bundles
    projected_used + auxiliary_bytes <= max_bundle_bytes_total
```

The actual formula may include a backend-specific index term, but it must be
deterministic and documented. A `total_bytes()` observation followed by a later
`put()` is not a reservation: another writer, an eviction pass, or an auxiliary
snapshot update can change the available space in between.

## 4. Commit and rollback invariants

### Provider invariants

1. A rejected reservation changes no bundle bytes, metadata, slot count, or
   accounting counters.
2. An aborted reservation releases exactly its reserved delta. Aborting twice is
   harmless, and aborting a consumed token cannot delete the committed bundle.
3. A successful commit makes bytes and metadata visible together, with
   `meta.size == bytes.len()` and the existing `BundleMeta.id` binding intact.
4. A failed commit leaves the pre-commit store state unchanged. A crash-recovery
   implementation must restore either that state or the complete new commit,
   never a mixture of records and counters.
5. A reservation is not durable cryptographic state. Restart discards in-flight
   tokens and reopens the last complete provider generation.

### Ingest invariants

The existing `mmp_core::ingest` order remains unchanged. It verifies the bundle,
inserts the ID into `SeenIndex` on first sight, then calls `try_put`. A capacity
rejection still returns `Disposition::Dropped(DropReason::StoreRejected)` and
does not deliver the bundle. Keeping the `SeenIndex` entry is intentional: it
prevents replay loops even when this node had no room to retain the bytes.

### Own-origin producer invariants

No producer should call `commit_reserved` or `commit_reserved_batch` until all
candidate state is staged:

| Producer | Candidate state that must remain unpublished until commit |
| --- | --- |
| `send_sealed` | Conversation root, DAG entry, pending first-contact marker, and `Created`/delivery event. |
| `send_app` | Pairwise send-chain key/index, pending async marker, conversation DAG, sent-message tracking, and event. |
| `send_group` | Local sender-key message index/chain and any queued distribution state. |
| `send_file` | Every symbol, blob index entry, manifest, and delivery bookkeeping; use the batch extension. |
| `publish_announcement` | SeenIndex/created event and returned ID must follow a successful commit. |
| `gossip_claim` / `emit_receipt` | Their current fire-and-forget semantics must be made explicit before capacity errors are surfaced; do not silently convert them to a partial transaction. |

For pairwise chains, the existing `SendChain::snapshot`/`restore` seam can be a
temporary implementation aid. It does not solve group keys or multi-record file
operations. The preferred long-term API is a `prepare`/`commit` candidate state
that never mutates the live chain before storage succeeds.

## 5. Error handling and recovery

`StoreReserveError::Capacity` is local admission information. It must not be
encoded in a bundle or exposed as a protocol-level rejection. `StoreCommitError`
is likewise local; callers may retry only after the provider says the token was
not consumed. On an unavailable/unknown outcome, the producer must retain the
candidate state in a local retry journal or discard it without reusing a
cryptographic index. It must not report `Created` or `Stored` merely because
encryption completed.

The legacy APIs remain valid during migration:

* `BundleStore::put` keeps its current infallible behavior for `MemStore` and
  existing host callers.
* `BundleStore::try_put` remains the receive-side bounded seam and maps capacity
  to `DropReason::StoreRejected`.
* The reservation and batch methods are opt-in capabilities on `BundleStore`.
  Their default implementations return `Unsupported`, so `NodeCore` should
  invoke them only after each producer has an explicit staged-state path; there
  is no safe generic downcast from `dyn BundleStore`.

## 6. Acceptance plan

### Host/provider tests

The host implementation and acceptance double already cover the first provider
preflight. A future fixed-capacity backend must retain these tests and add:

1. Exact-fill insert, one-byte-over capacity rejection, and replacement using
   the delta rather than double-counting old bytes.
2. Outstanding reservations consume capacity; abort releases it; duplicate,
   stale, mismatched-ID, and wrong-length tokens fail closed.
3. Rejected reserve and failed commit leave bytes, metadata, slot count, and
   accounting unchanged.
4. Own-origin bundles are not evicted to admit a relay bundle; expired bundles
   may be reclaimed only by the named policy.
5. `ingest` maps full-store rejection to `StoreRejected`, keeps first-sight
   SeenIndex state, and never emits a local delivery event.
6. Snapshot/journal recovery exposes either the old or complete new generation
   after simulated write, flush, rename, and restart failures.

### Producer tests

For each own-origin producer, inject reservation rejection and assert that the
candidate state is unchanged: pairwise chain epoch/index, group sender-key
index, conversation/DAG/pending markers, sent-message state, blob indexes, and
event queue. Add a successful path that proves all of those become visible only
after commit. File tests must exercise aggregate batch rejection and no partial
symbol/index residue.

### Embedded acceptance gate

The host suite is necessary but not sufficient. The target gate requires:

```text
cargo check -p mmp-node-embassy -p mmp-tal-embedded \
  --target thumbv7em-none-eabihf --no-default-features

cargo check -p mmp-store --target thumbv7em-none-eabihf --no-default-features
```

Then build a named nRF52/ESP32 release image and record static RAM, flash, and
stack use. Hardware-in-the-loop must cover authenticated phone-to-sensor
exchange, full-store rejection, packet loss, reset/power-cycle recovery, and
Reed–Solomon decode. A target dependency check alone is not evidence that the
store is usable on a board.

## 7. Implementation order and stop conditions

1. Review and freeze the additive capability and board policy inputs.
2. Implement a host fixed store and deterministic transaction test double;
   leave the legacy `BundleStore` path unchanged.
3. Integrate receive-side `try_put` and verify `StoreRejected` behavior.
4. Add candidate-state APIs producer by producer, starting with `send_sealed`
   and pairwise direct messages. The current implementation has a reviewed
   interim exception for file symbols (`try_store_own` before holder-index
   mutation) that provides resumable partial inventory only; do not claim
   strict aggregate group/file atomicity until their rollback or batch path
   exists.
5. Add the batch capability for file symbols, then wire the embedded backend.
6. Run host, target, and hardware acceptance suites before claiming E7-S1.

Stop and return to design review if any provider cannot prove all-or-none commit,
if capacity arithmetic needs a wire-visible change, or if a producer would need
to reuse an advanced ratchet index after an uncertain commit result.
