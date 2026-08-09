# E7-S1 fixed-capacity store handoff

Status: design blocker identified (2026-08-09). No protocol specification,
wire encoding, `mmp-types`, mobile adapter, or existing `SnapshotStore` behavior
was changed in this audit.

## Decision

Do not add a guessed `FixedStore` implementation yet. The current
`mmp_core::BundleStore` contract cannot represent bounded-storage rejection
without lying to the node, and its read-side API allocates. A small array-backed
store would therefore either violate the existing node semantics or still
require a heap allocator. The correct next step is an explicit API seam and
capacity decision, followed by implementation and target tests.

## Evidence from the current tree

The target dependency closure itself is healthy:

```text
cargo check -p mmp-node-embassy -p mmp-tal-embedded \
  --target thumbv7em-none-eabihf --no-default-features
Finished `dev` profile
```

The persistence crate now has an explicit dependency-fence profile. Its
no-default-features target check passes, but that profile intentionally exports
no store API until the fixed-capacity backend is designed:

```text
cargo check -p mmp-store --target thumbv7em-none-eabihf --no-default-features
Finished `dev` profile
```

The default `std` feature still provides the filesystem `SnapshotStore`; its
`mmp-core` and `mmp-crypto` path dependencies are now explicitly default-off
and are re-enabled only by `mmp-store/std`. This is compile hygiene, not an
embedded persistence implementation.

The lower protocol API also demonstrates why a store-only patch is not enough:

| API seam | Current contract | Consequence for a no-alloc store |
| --- | --- | --- |
| `BundleStore::put` | returns `()` | a full store cannot report rejection; `ingest` then returns `Stored` and `Node` records delivery/state as if bytes were durable |
| `BundleStore::get` | returns `Option<(BundleMeta, Vec<u8>)>` | reading a bundle allocates |
| `ids_range` / `all_meta` / `expire` | return `Vec` | reconciliation, quota, and expiry require allocation |
| `MemStore` | `BTreeMap<Id16, (BundleMeta, Vec<u8>)>` | the reference implementation is heap-backed |
| `mmp-types` / `mmp-crypto` | wire, AEAD, Noise, ratchet, and group APIs use `Vec`/`BTreeMap` | the current `#![no_std]` profile is `no_std + alloc`, not a no-allocator profile |

The most dangerous case is an oversized or full store: silently dropping from
`put` would make `ingest` return `Disposition::Stored` and may deliver a local
message even though the durable record was never admitted.

## Required API work before implementation

1. Add an explicit write outcome (for example `try_put` returning
   `Stored`/`Replaced`/`CapacityRejected`) and thread capacity rejection through
   `ingest` as a distinct `DropReason`. Existing `SnapshotStore` behavior must
   remain unchanged; its implementation can always return `Stored` or an I/O
   error at its persistence boundary.
2. Decide whether the embedded profile is `no_std + alloc` or genuinely
   allocator-free. For the former, a bounded store can own a fixed number of
   heap allocations and still use the current read API. For the latter, add
   caller-provided output buffers or callback iteration for `get`, range IDs,
   metadata, and expiry. This is a public Rust API change, not a wire change.
3. Define capacity from a named board/resource budget: maximum live bundles,
   maximum bytes per bundle, metadata/output counts, claims/group state, and
   allocator/stack reservation. Do not invent constants from the protocol's
   network quota; storage capacity is a board policy.
4. Add the fixed-capacity embedded module behind the existing dependency-fence
   profile. The default `std` snapshot module and its explicit feature wiring
   are now in place, but no embedded store API is exposed yet.
5. Add host tests for replacement, capacity rejection, expiry, sorted range
   output, and no-state-change on rejected writes. Then run target checks and a
   linked board image before claiming E7-S1.

## External acceptance gate

No host-only test can close the remaining E7 gate. A future run needs a named
nRF52832/nRF52840 (or exact ESP32 variant), linker script and release profile,
stripped image/map output, static-RAM accounting, and a power/reset fixture.
The HIL matrix must include authenticated phone-to-sensor exchange, full-store
rejection, packet loss, reset, power-cycle recovery, and Reed–Solomon decode
measurements. Until those artifacts exist, the target check above is only a
dependency-closure proof, not evidence of a functioning embedded store.

## Host acceptance-double evidence (2026-08-09)

Commit `0ab41c8` adds an integration-test-only fixed-capacity acceptance
double at
`implementation/mesh-milk/crates/mmp-store/tests/transactional_prototype.rs`.
It exercises exact-fill and replacement-delta accounting, outstanding
reservation/abort behavior, stale or mismatched commit tokens, failed-commit
state preservation, and recovery of the last durable generation. The
reproducible command is:

```text
cargo test -p mmp-store --test transactional_prototype --all-features
```

The six tests pass locally and the command is now a named CI step
(`test embedded transactional store acceptance double (E7-S1)`). This is a
provider-design preflight only: the test double is not exposed as a public
store implementation, does not choose board capacities, and does not close
the fixed-capacity backend, linked-image, or hardware-in-the-loop gates above.

## Current API checkpoint (2026-08-09)

The original blocker list above predates the receive-side bounded-write seam.
The current tree now contains `mmp_core::BundleStore::try_put`,
`StorePutOutcome`, and `StorePutError::Capacity`; `mmp_core::ingest` maps a
rejected write to `DropReason::StoreRejected` without reporting the bundle as
stored. `mmp-store`'s bounded `SnapshotStore::try_put` accounts for complete
snapshot framing and replacement deltas, with host coverage in
`tests/bounded_snapshot.rs`.

This checkpoint is deliberately narrower than the proposed transactional
producer API: `try_put` is a receive-side admission result and does not reserve
space while an own-origin ratchet or file operation is being prepared. The
no-default-features profile still exports no flash-backed provider, and no
board capacity or allocator policy is selected here. Future work must not
interpret the current target check or acceptance double as closing those
remaining E7-S1 requirements.

## Lower-layer bounded-state increment (2026-08-09)

The E7 audit found one concrete unbounded state path below the provider seam:
`mmp_core::ReceiptFilter` kept its exact-purge `BTreeMap` without a runtime
bound, even though the implementation handbook's Standard-tier RAM table
budgets that map at 8,192 entries. The implementation now binds the local map
to the existing `BLOOM_CAPACITY` value (8,192), rejects persisted exact counts
above that bound before allocating, and evicts the earliest-expiring exact
entry when a new authenticated receipt arrives at a full map. Equal-expiry
ties are deterministic by `Id16`.

This is intentionally conservative: the corresponding Bloom bit remains a
hint, so evicting an exact entry can only delay an optional purge; it cannot
cause an undelivered bundle to be deleted. The bound is local state only and
does not alter receipt wire encoding, RBSR, quotas, or bundle semantics. The
focused tests cover full-capacity insertion, deterministic eviction, and
hostile persisted counts. This change reduces an attacker- or workload-driven
allocation path in the shared no-std+alloc core, but it does not select a Core
tier capacity, provide a flash backend, or establish allocator-free operation.

Evidence:

```text
cargo test -p mmp-core --lib receipt_filter -- --test-threads=1
5 passed
```

The remaining E7 decision is unchanged: a board-specific fixed-capacity
provider must still define allocator/flash policy, producer reservation and
rollback semantics, and linked-image/HIL acceptance before the embedded gate
can be closed.

## Authority-state runtime bounds (2026-08-09)

The same audit checked `mmp_core::AuthorityStore` against its snapshot limits.
Those limits were already enforced by `decode`, but live mutation paths could
otherwise exceed them between snapshots. The implementation now:

- bounds local subscriptions and pinned roots at 256 entries, ignoring a new
  entry at capacity (an intentionally fail-closed local policy); and
- rejects a new `(subject, scope)` certificate once the 512-entry map is full
  with `CertReject::Capacity`, while allowing an existing entry to refresh.

`AuthorityStore::expire` now also prunes sticky revocations after each entry's
`not_after + skew` horizon, matching its existing documentation and allowing
expired state to release capacity. No wire object or snapshot version changed.

The revocation union remains an explicit design blocker: silently dropping a
new revocation at the 512-entry limit could fail open, while evicting an older
revocation is also unsafe. The current method therefore remains unbounded until
an API-compatible, fail-closed overflow policy (for example an authority-disable
state with producer-visible error) is designed. This is not papered over by the
runtime bounds above.

Focused evidence:

```text
cargo test -p mmp-core --lib authority -- --test-threads=1
14 passed

cargo check -p mmp-core -p mmp-node-embassy \
  --target thumbv7em-none-eabihf --no-default-features
Finished `dev` profile
```
