# E7-S1 nRF52-class fixed-capacity provider contract

Status: implementation contract proposal, host-testable but not hardware
validated. This document does not modify `specification/`, the MMP/1 wire
format, cryptographic domains, or protocol limits.

## 1. Purpose and evidence boundary

The current repository has three useful pieces, but not a complete embedded
store:

* `mmp-core::BundleStore` is the host compatibility seam. Its required
  `Vec`-returning reads and infallible `put` cannot prove the handbook §4.9
  requirement of no allocation growth after initialization.
* `mmp-flash-journal` supplies allocation-free framing, checksums, scanning,
  and a provider-neutral aligned-I/O trait. It deliberately does not select
  board geometry or promise power-loss atomicity.
* `mmp-node-embassy` and `mmp-tal-embedded` are bounded queue shells; neither
  is a flash provider or a board backend.

Therefore this contract defines a separate embedded storage boundary and a
host reference model before any nRF52 HAL dependency is introduced. It does
not claim that the existing `BundleStore` trait can be implemented without an
allocator on an MCU.

## 2. Target profile

The first target is an **nRF52-class Cortex-M4 profile**. The exact board,
flash map, HAL, SoftDevice reservation, program unit, erase page, linker
script, and runner are deliberately parameters of the implementation ticket,
not invented facts in this contract.

The profile must publish, in one checked configuration record:

```text
FLASH_CAPACITY
STORE_REGION_OFFSET / STORE_REGION_BYTES
PROGRAM_UNIT_BYTES
ERASE_PAGE_BYTES
MAX_LIVE_BUNDLES
MAX_BUNDLE_BYTES = mmp_types::consts::MAX_BUNDLE
MAX_INDEX_ENTRIES = MAX_LIVE_BUNDLES
MAX_RESERVATIONS
MAX_BATCH_ENTRIES
MAX_SNAPSHOT_PAYLOAD <= mmp_flash_journal::MAX_PAYLOAD_LEN
```

Compile-time validation must reject zero values, non-aligned region boundaries,
`MAX_BUNDLE_BYTES` larger than the provider slot, and any configuration whose
metadata/index/journal overhead exceeds `STORE_REGION_BYTES`. It must also
prove that the worst-case complete snapshot (all live slots, index, generation
metadata, framing, alignment, and assigned auxiliary state) fits both
`MAX_SNAPSHOT_PAYLOAD` and the configured journal region.

The existing handbook budget remains the acceptance target: approximately
12 KiB protocol RAM plus a 32 KiB store, within 48 KiB static RAM. The 32 KiB
figure is not permission to allocate 32 KiB in addition to the stated RAM
budget; the linker map and static allocation report are authoritative.

## 3. No-allocation API boundary

The embedded provider must not implement the MCU path by boxing
`mmp-core::BundleStore`, because the compatibility trait requires allocating
methods (`get`, `ids_range`, `all_meta`, and `expire`) and `Vec`-returning batch
results. The implementation shall introduce an additive, no-alloc trait in an
embedded-facing crate or module, with caller-owned buffers and fixed-capacity
iterators. The exact Rust names are intentionally left to the implementation
PR, but the contract is:

```text
read_meta(id, &mut BundleMeta) -> NotFound | Ok
read_bundle(id, &mut [u8]) -> NotFound | OutputTooSmall(required) | Ok(len)
visit_ids(range, &mut dyn FnMut(Id16))
visit_meta(&mut dyn FnMut(BundleMeta))
expire_into(now, &mut [Id16]) -> OutputTooSmall(required) | Ok(count)
try_put / reserve / commit / abort
recover() -> Committed { generation } | Reusable | Indeterminate
```

`generation` identifies the complete recovered snapshot, not an individual
bundle ID; a committed snapshot may publish multiple bundle IDs and auxiliary
records together.

All result collections are caller-owned or fixed-capacity. No input-derived
length may drive an unbounded allocation. A host adapter may continue to
implement `BundleStore` by translating this boundary into `Vec` results; that
adapter is not evidence of embedded suitability.

`heapless` is a candidate dependency for fixed maps/queues, but it must be
evaluated against the workspace MSRV, license, `no_std` behavior, and actual
capacity semantics before it is added. A small protocol-specific index is
allowed only if those checks show that a general crate would add more risk
than the bounded local type.

## 4. Static storage and admission policy

The provider owns a fixed array of slot metadata and a fixed ID index. A slot
is either free, reserved, stored, or quarantined after an indeterminate write.
The index and slot table are bounded by `MAX_LIVE_BUNDLES`; they never grow in
response to a peer-controlled field.

`MAX_RESERVATIONS` and `MAX_BATCH_ENTRIES` are compile-time limits as well.
Outstanding reservations, batch entries, and quarantined slots count against
their respective limits before any peer-controlled request is accepted. The
provider must expose a bounded recovery operation that scans and classifies a
quarantined slot as committed or reusable; there is no unbounded quarantine
list and no silent manual deletion of an indeterminate record.

Admission arithmetic is checked and accounts for:

1. encoded bundle bytes (`meta.size`, then the actual byte length at commit);
2. per-slot metadata and index overhead;
3. journal framing, checksum, alignment padding, and generation headers;
4. all outstanding reservations and batch entries; and
5. quarantined/indeterminate slots plus auxiliary records explicitly assigned
   to this profile.

An accepted reservation must remain valid against concurrent local writers.
Duplicate IDs in one batch, an ID with an outstanding reservation, size
overflow, and replacement-generation mismatch are rejected without mutation.

The eviction policy is fixed and testable:

* own-origin records are never evicted before expiry;
* pinned friendship records are not eviction candidates;
* expired records may be reclaimed only by the configured expiry pass;
* among eligible relay records, use the existing `mmp-router` eviction order;
* if no eligible record exists, return local capacity failure rather than
  silently violating pinning or own-origin rules.

The profile must not enable the blob plane until its slot and RAM accounting
proves that a complete symbol transfer fits the named target budget.

## 5. Journal and commit state machine

The provider reuses `mmp-flash-journal` framing and scanning. The scanner is a
corruption/recovery primitive, not an authenticity mechanism. The provider
should use `scan_validated` to gate the selected highest-generation payload;
its validator must revalidate the complete snapshot with the existing
canonical decode, BundleID, signature, metadata binding, and class-specific
checks before any state enters the index. A validator failure is fail-closed;
the provider must not silently fall back to an older generation.

For the first implementation, **one journal payload is one complete bounded
provider snapshot**, not one bundle record. It contains the slot table, ID
index, live bundle bytes, generation metadata, and the auxiliary records
assigned to the selected profile. This is required because the current
scanner returns the highest valid record rather than an iterator over every
valid record. A per-slot append log would need a different scanner and a
cross-record recovery protocol; it is explicitly deferred.

One logical generation follows this sequence:

```text
FREE → BODY_WRITTEN → CHECKSUM_WRITTEN → COMMIT_MARKER_WRITTEN → VISIBLE
```

The board adapter must make the commit marker the final aligned program
operation. `encode()` returning a complete image is not itself a power-loss
guarantee; the adapter must split or stage the image so a marker cannot become
valid before its body and checksum. The implementation must document how
program-unit padding and erase alignment preserve this ordering.

After reset, scanning selects the highest valid generation and ignores
incomplete/corrupt candidates. A generation floor or equivalent monotonic
check must prevent rollback to an older image after a torn write. Generation
overflow is a fail-closed condition, not a wrapping counter.

## 6. Indeterminate write outcomes

An I/O error after programming has started is **indeterminate**, not ordinary
capacity failure. The provider contract must expose one of these outcomes to
its caller:

```text
Committed       — recovery proves the complete record is durable
NotCommitted    — recovery proves no new record is visible
Indeterminate   — recovery cannot yet distinguish the two
```

The current `StoreCommitError` enum has no indeterminate variant; an embedded
adapter must not silently map this condition to `Capacity` or call an ordinary
abort. Until recovery resolves it, the provider enters a quarantine state:
new reservations are rejected, no cryptographic counter is reused, and the
caller performs a fresh scan. A retry is allowed only after the result is
`NotCommitted`; a `Committed { generation }` result identifies the durable
complete snapshot, whose restored IDs are then enumerated from the provider;
an unresolved `Indeterminate` result requires operator/reset recovery according
to the board policy.

If implementation chooses to keep the existing trait unchanged, it must prove
that the provider internally performs this recovery before returning any
`Err`. A best-effort abort after an unknown flash write is not compliant.

## 7. Reservation lifecycle across reset

Reservation tokens remain runtime-only and are never serialized. On restart,
all in-memory reservations are discarded only after the journal has been
scanned and committed records have been reconstructed. Pending/quarantined
slots are reclaimed according to the generation state machine; they must not
reduce capacity forever or expose half-written bytes.

The provider must test reset/power loss immediately before and after every
body, checksum, marker, index, and generation-floor operation. The result must
be either the old complete generation or the complete new generation, never a
mixture of records, metadata, counters, and accounting.

## 8. Node-state boundary

`BundleStore` durability alone does not persist `SeenIndex`, receipt filters,
claims, groups, authority state, or pairwise/group ratchets. A bundle journal
must therefore not be advertised as crash-atomic for producer state.

Before wiring own-origin producers into this profile, choose and test one of
these explicit policies:

* **Coordinated checkpoint:** bundle records and all replay/cryptographic state
  needed to resume counters share one generation/commit boundary; or
* **Conservative receive-only profile:** the embedded node stores and forwards
  verified received bundles, but own-origin durable publication is disabled
  until a coordinated checkpoint exists. No sender counter may be reused after
  an uncertain commit.

The implementation must not claim exactly-once producer semantics by merely
placing `BundleStore` behind a flash journal.

## 9. Acceptance matrix

The host reference provider must pass, at minimum:

* exact-fill, one-byte-over, replacement-delta, duplicate, stale-token, and
  wrong-length tests;
* reservation rejection, abort idempotence, commit failure, and indeterminate
  outcome tests with byte/index/accounting snapshots;
* partial-write fault injection at every logical record boundary;
* restart selection of old versus complete new generations, including nested
  markers and generation rollback attempts;
* complete-snapshot recovery proves that every live slot and its index are
  restored together; a valid record containing only one bundle is rejected as
  an invalid provider snapshot;
* malformed/corrupt recovered bundles rejected before indexing; and
* exhaustion tests proving no peer-controlled input causes allocation or
  capacity-table growth.

The target gate then requires:

```text
cargo check -p <embedded-provider> -p mmp-node-embassy \
  --target thumbv7em-none-eabihf --no-default-features
```

followed by a named-board release image with linker map, static RAM/flash
measurement, RS-decode benchmark, reset/power-cycle matrix, and authenticated
phone-to-sensor exchange. Until those artifacts exist, this contract is a
design milestone, not an E7 completion claim.

## 10. Explicit non-claims and next implementation order

This document does not select an nRF52 HAL, SoftDevice reservation, linker
script, flash page count, radio schedule, or new wire field. It does not claim
that `mmp-flash-journal` provides authenticity or that the current
`mmp-core::BundleStore` satisfies no-alloc constraints.

Implementation order:

1. Select and record one exact board/HAL/toolchain and fill the profile
   geometry from its authoritative datasheet and linker map.
2. Implement the no-alloc trait and a host fixed-capacity reference provider.
   The current `mmp-embedded-provider::JournaledFixedStore` is the first
   two-slot reference slice and uses caller-owned buffers.
3. Add the indeterminate-outcome, bounded quarantine, and coordinated-
   checkpoint decisions to the provider API before connecting producers.
4. Reuse `mmp-flash-journal`, existing bundle verification, and existing
   eviction logic; avoid a second serializer or cryptographic primitive. The
   reference provider now exercises this composition, including strict
   head-record selection and validator-gated recovery.
5. Run the host fault matrix, Cortex-M compile, then hardware gates in order.

The MMP/1 specification remains frozen throughout these steps.

## 11. Design review record

### Alternative considered: implement `BundleStore` directly with `heapless`

This was rejected for the first implementation because the existing trait's
required allocating methods and dynamic `Box<dyn BundleStore>` ownership would
still expose an allocator-dependent contract. A heapless internal map would
reduce growth but would not remove those return types or define flash commit
outcomes. A dedicated embedded boundary plus an optional host adapter is the
smallest change that can honestly satisfy §4.9 without changing wire behavior.

### Alternative considered: treat every flash error as capacity failure

This was rejected because a program operation may have reached the commit
marker before reporting an error. The provider must resolve the record by
recovery or quarantine the profile; otherwise retrying can reuse a cryptographic
counter or corrupt accounting.

### Skeptical Professor

**Potential flaw:** a written contract could be mistaken for a measured board
implementation.

**Resolution:** exact board geometry, linked-image measurements, and HIL are
explicitly acceptance gates; all current values are labelled provisional.

### Security Auditor

**Potential flaw:** checksum recovery could be treated as authentication.

**Resolution:** every recovered bundle must pass the existing canonical ID,
signature, metadata, and class validation before indexing; the journal is not a
trust boundary.

### Performance Engineer

**Potential flaw:** static slot counts can fit flash while exceeding stack or
auxiliary-state RAM.

**Resolution:** the profile requires linker-map/static-RAM evidence and checked
metadata/index accounting before enabling a configuration.

### Distributed Systems Reviewer

**Potential flaw:** durable bundle bytes without SeenIndex or ratchet state can
create replay or counter-reuse after reset.

**Resolution:** producer integration is blocked until a coordinated checkpoint
exists, or the profile is explicitly receive-only; BundleStore durability alone
never claims exactly-once producer semantics.

### Battery Specialist

**Potential flaw:** a flash provider could meet capacity limits while causing
excessive erase/write energy.

**Resolution:** erase wear and power measurements belong to the named-board
acceptance matrix; no battery claim is inferred from this contract.

### Inventive Scientist

**Potential flaw:** board-specific optimization could silently fork protocol
semantics.

**Resolution:** reuse existing framing, verification, and eviction primitives;
all optimization is confined to local storage and must preserve the frozen
wire contract byte-for-byte.
