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

`put` cannot report a full fixed-capacity store. `try_put` is sufficient for
receive-side admission, but it does not reserve capacity while a producer is
encrypting a message or preparing a multi-bundle file. Replacing `put` with
`try_put` in `NodeCore::store_own` today would leave pairwise/group ratchets,
conversation state, and file indexes advanced when the provider rejects the
write.

The next seam must therefore be additive. Existing implementations and callers
continue to compile and retain their current behavior. A provider opts into
transactional writes only after it can satisfy the invariants below.

## 2. Proposed additive API

Add a separate provider capability rather than changing `BundleStore` in place.
The initial shape is intentionally single-bundle; a batch extension is required
for file symbols and other operations that create several bundles.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StoreReservation {
    // Private fields. The value is provider-local and cannot be serialized or
    // constructed from wire input.
    _private: (u64, u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreReserveError {
    /// The requested replacement/insert does not fit the configured policy.
    Capacity,
    /// `meta.size` or the requested byte length is not representable or is
    /// inconsistent with the provider's limits.
    TooLarge,
    /// The provider cannot issue another reservation until an outstanding
    /// operation is completed.
    Busy,
    /// The provider is unavailable without changing durable state.
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreCommitError {
    /// The token, bundle ID, or byte length does not match the reservation.
    ReservationMismatch,
    /// The provider could not complete the write and guarantees that no
    /// partial bundle became visible.
    Capacity,
    /// The provider failed before publishing a complete commit. The caller
    /// must not reuse candidate cryptographic state until recovery resolves it.
    Unavailable,
    /// The candidate bytes or metadata failed provider-local validation.
    Invalid,
}

pub trait TransactionalBundleStore: BundleStore {
    /// Reserve the exact final encoded byte length. Existing IDs reserve only
    /// the replacement delta, not a second copy of the old bytes.
    fn reserve_bundle(
        &mut self,
        meta: BundleMeta,
        byte_len: u32,
    ) -> Result<StoreReservation, StoreReserveError>;

    /// Atomically publish one reserved bundle. Success consumes the token;
    /// failure leaves no new bytes or metadata visible.
    fn commit_bundle(
        &mut self,
        reservation: StoreReservation,
        meta: BundleMeta,
        bytes: &[u8],
    ) -> Result<StorePutOutcome, StoreCommitError>;

    /// Idempotently release an uncommitted reservation. It must not remove a
    /// previously committed bundle when called with an already-consumed token.
    fn abort_bundle(&mut self, reservation: StoreReservation);
}
```

The exact Rust spelling can change during implementation, but the following
properties are part of the contract:

* `StoreReservation` is opaque, process-local, non-wire state. It is not a
  capability that a peer can request or forge, and it is not persisted across a
  restart.
* `reserve_bundle` accounts for the final `byte_len`, metadata overhead, and
  replacement delta. It includes all currently outstanding reservations in the
  capacity calculation.
* `commit_bundle` checks the token, ID, declared size, and actual `bytes.len()`
  before publishing. It never evicts an existing own-origin bundle as a hidden
  side effect.
* A successful commit consumes the reservation exactly once. A failed commit
  either leaves the reservation abortable or returns an explicit unavailable
  outcome; it must never report failure after publishing a partial bundle.
* A provider that cannot prove these properties must continue using the legacy
  `put`/`try_put` surface and must not implement this capability by estimation.

### 2.1 Batch extension

`send_file` creates symbols and a manifest. Per-bundle reservations are not
enough to make that operation all-or-none. The follow-up batch capability should
use the same opaque-token model:

```rust
trait TransactionalBundleBatch: TransactionalBundleStore {
    type Batch;

    fn begin_batch(&mut self) -> Result<Self::Batch, StoreReserveError>;
    fn reserve_in_batch(
        &mut self,
        batch: &mut Self::Batch,
        meta: BundleMeta,
        byte_len: u32,
    ) -> Result<(), StoreReserveError>;
    fn commit_batch(
        &mut self,
        batch: Self::Batch,
        writes: &[(&BundleMeta, &[u8])],
    ) -> Result<Vec<StorePutOutcome>, StoreCommitError>;
    fn abort_batch(&mut self, batch: Self::Batch);
}
```

The batch is provider-local and may be implemented with a fixed array on an
embedded target. It must preflight the aggregate replacement delta before any
write becomes visible. The batch API is not required to land in the first
single-bundle implementation, but `send_file` must not pretend that repeated
single commits are transactional.

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

No producer should call `commit_bundle` until all candidate state is staged:

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
* `TransactionalBundleStore` is an opt-in capability. `NodeCore` should detect
  it only after each producer has an explicit staged-state path; there is no
  safe generic downcast from `dyn BundleStore`.

## 6. Acceptance plan

### Host/provider tests

Implement a deterministic fixed store test double before a board backend. It
must cover:

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
   and pairwise direct messages. Do not touch group/file sends until their
   rollback or batch path exists.
5. Add the batch capability for file symbols, then wire the embedded backend.
6. Run host, target, and hardware acceptance suites before claiming E7-S1.

Stop and return to design review if any provider cannot prove all-or-none commit,
if capacity arithmetic needs a wire-visible change, or if a producer would need
to reuse an advanced ratchet index after an uncertain commit result.

