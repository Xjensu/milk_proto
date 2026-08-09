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
