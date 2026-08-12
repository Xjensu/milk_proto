# E7-S1 transactional API decision

Status: design gate, 2026-08-09. No Rust API, wire format, or specification
document was changed by this review.

## Decision

Do **not** add `TransactionalBundleStore`, `StoreReservation`, or the proposed
error enums to `mmp-core` yet. The design is directionally correct, but adding
only the trait/types before a provider exists would create a public dead API and
would not give `NodeCore` a usable capability path.

The existing `BundleStore` and `BundleStore::try_put` surfaces remain the only
supported behavior during this gate. The detailed reservation contract lives in
`docs/E7_FIXED_STORE_DESIGN.md` as an implementation target, not as a frozen
Rust API.

## Evidence

### The current owner is a trait object

`NodeCore` and the FFI adapter own stores as `Box<dyn BundleStore + Send>`, while
`mmp_core::ingest` accepts `&mut dyn BundleStore`. An extension trait is not
discoverable through either path without another public capability seam or a
downcast contract. Adding a trait that no production caller can invoke would
not protect own-origin writes and could give the false impression that E7 is
implemented.

### The proposed token shape is not implementable as written

The design sketch uses an opaque `StoreReservation` with private fields. A
provider implemented in `mmp-store` or a board crate cannot construct that value
unless `mmp-core` exposes a constructor, which would make the provider depend on
an undocumented token-generation policy. Making the fields public solves
construction but permits arbitrary local token forgery and weakens the stated
provider-local ownership invariant. A provider-associated reservation type is a
better candidate, but associated types do not fit the current `dyn BundleStore`
path without an adapter.

### Commit semantics are not proven by a declaration

There is no fixed-capacity provider yet to demonstrate replacement deltas,
outstanding reservations, abort idempotence, multi-bundle atomicity, or recovery
after an uncertain commit. `SnapshotStore` also has an in-memory `put` boundary
and a later snapshot `persist` boundary; it cannot honestly implement an
all-or-none own-origin reservation by merely delegating to either one.

Finally, `send_file` needs an aggregate batch transaction, while pairwise and
group sends need candidate ratchet state. A trait declaration without those
producer tests would leave the highest-risk paths unchanged.

## Safe next step

Before exposing a public mmp-core capability, implement a provider-side
prototype behind a test-only or private module with a concrete token model and
the following tests:

1. exact-fill, replacement-delta, outstanding-reservation, abort, stale-token,
   wrong-ID, and wrong-length cases;
2. all-or-none commit and recovery after simulated provider failure;
3. receive-side `try_put` mapping to `DropReason::StoreRejected`;
4. pairwise, group, and file producer rejection tests proving no ratchet,
   pending, DAG, or blob-index state changes;
5. a target-specific capacity policy and no_std+alloc/no-allocator decision.

Then choose one of two public shapes:

* an object-safe capability method with a provider-issued opaque handle and an
  explicit `NodeCore` capability adapter; or
* a provider-associated reservation type with a generic `NodeCore`/adapter path
  that does not pretend the current `dyn BundleStore` can discover it.

The selected shape must be implemented by at least one independent production
provider before it is considered an extensibility contract. Until then,
`MemStore`/`SnapshotStore` may use the in-tree reservation methods, while
legacy providers retain the `try_put` fallback and must not be treated as
transactional.

## Verification update (2026-08-12)

The repository now contains an additive reservation seam directly on
`BundleStore`: opaque single-bundle and atomic-batch reservations are exercised
by `MemStore` and forwarded by `SnapshotStore`. The provider review found the
tested arithmetic and stale-generation checks sound, but the opaque token
fields remain private to `mmp-core`; an independent production backend cannot
implement the capability without a future adapter or provider-issued handle.

The slice-based `NodeCore::send_file` path uses the atomic batch capability when
available and publishes symbols, indexes, SeenIndex entries, and the manifest
only after a successful commit. The reader-backed path intentionally remains a
bounded-memory legacy path: it re-reads one stripe at a time and may retain a
durable accepted symbol prefix if a provider rejects a later symbol. A focused
regression test records that this prefix is indexed only after durable writes,
while the manifest conversation and delivery state remain unpublished.

Making reader sends all-or-none with the current borrowed-entry batch API would
require retaining every encoded symbol until commit (up to the full 8 MiB file
limit and its Reed–Solomon expansion). That memory trade-off is not justified
for mobile or IoT targets. Do not reserve capacity before reading the complete
source, and do not fall back to single writes after a non-`Unsupported` batch
failure. The next design gate is a bounded streaming/staged provider capability
or an explicit product choice to keep reader sends resumable-prefix semantics.

## Scope and acceptance boundary

This decision is implementation-only. It does not alter protocol messages,
cryptographic state, routing, or specification status. The E7 gate remains open
until a provider, producer rollback/batch behavior, host tests, target checks,
and named-board HIL evidence satisfy `docs/E7_FIXED_STORE_DESIGN.md`.

## Prototype evidence (2026-08-10)

The first safe implementation slice is now committed as `b5200f1`
(`test(store): prototype fixed-capacity reservations`). The
integration-test-only provider in
`crates/mmp-store/tests/fixed_transactional_provider.rs` keeps its reservation
token private and exercises exact-fill admission, replacement-delta accounting,
outstanding reservations, same-ID busy rejection, idempotent abort, stale or
mismatched tokens, simulated unavailable commit, and bounded receive-side
`try_put`. Five focused tests and strict Clippy pass.

This prototype intentionally does **not** add a public capability trait or
claim that `SnapshotStore` is transactional. The producer audit confirms why:
The staged pairwise and group-ciphertext paths now commit candidate state only
after `try_store_own`, while group distributions/rekeys and the file plane still
span multiple provider writes. File symbols use the safer interim order
`try_store_own` then holder-index mutation, but an accepted prefix can remain
after a later symbol or manifest failure. A reservation API without staged
producer state would make capacity failures worse by leaving cryptographic and
blob state advanced.

`SnapshotLimits` also bounds the complete snapshot image only. It does not yet
provide slot-count, per-bundle, metadata, auxiliary-budget, or crash-journal
reservation semantics. Wrapping `try_put` or estimating free bytes would
therefore be an invalid production implementation. The next E7 step is a
reviewed provider policy plus producer-specific prepare/commit paths for group
distributions/rekeys and the aggregate file/manifest operation; the current
single-bundle pairwise paths are already covered by candidate staging.
