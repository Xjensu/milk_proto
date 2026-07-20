# Implementation Audit

Rust source baseline / parent HEAD before this audit artifact was created:
`1c6470d313803458c4d0ecb8f83cc21389becc24`, inspected 2026-07-20. Subsequent
Task 5 commits add or revise only `course/audit/implementation.md`; they do not
change the audited Rust tree under `implementation/mesh-milk/`. This document
does not attempt to permanently name its own containing HEAD, because its own
commit necessarily changes that HEAD. It does not describe the user-owned,
possibly dirty checkout at `C:\Users\Asus\Desktop\milk_proto`, and no source
from that checkout was used as evidence. No additional implementation document
was read to resolve a boundary; this audit relies on manifests, Rust sources,
vectors, fuzz sources, and executable commands named below.

“Covered” below means a named source/test exercises a behavior. It does not
mean the behavior is correct, interoperable, secure in deployment, or complete.
Confidence is High only for a fact directly observed in this committed source
and, where stated, exercised by the named local test; Medium when source-only;
Low where the code itself labels a planned or stubbed feature.

## Build and test baseline

The required commands were run in this isolated worktree on 2026-07-20, with
the existing lockfile and without dependency edits.

| Command | Result | Counts / notes |
|---|---|---|
| `cargo test --manifest-path implementation/mesh-milk/Cargo.toml --workspace --all-targets --locked` | exit 0 | 111 passed, 0 failed, 1 ignored, 0 measured, 0 filtered out. The ignored test is `mmp_sim::tests::hundred_node_mesh_stress`; there were no failure names. This independently matches the orchestrator pre-task baseline of 111 executed and 1 ignored. |
| `cargo clippy --manifest-path implementation/mesh-milk/Cargo.toml --workspace --all-targets --locked -- -D warnings` | exit 0 | no warnings promoted to errors; the final repeat completed in 15.71 s after waiting for the build-directory lock. |
| `cargo run --manifest-path implementation/mesh-milk/Cargo.toml --locked -p layer-check` | exit 0 | `layer-check OK`: seven designated lower layers were clean of the banned package set and of `std::time` / `tokio::` source tokens. |

The test command validates unit/property/integration scenarios compiled by the
workspace, not BLE hardware, generated Kotlin/Swift bindings, embedded targets,
cargo-fuzz execution, production storage durability, or a standards-independent
decoder. In particular, a passing `--all-targets` run does not execute the four
`cargo-fuzz` harnesses, which are a separate workspace under
`implementation/mesh-milk/fuzz/Cargo.toml`.

## Workspace dependency map

`cargo metadata --manifest-path implementation/mesh-milk/Cargo.toml --locked
--no-deps --format-version 1` identified 17 workspace members: the 16 crates
under `crates/` and the `layer-check` tool. `fuzz/` is deliberately a separate
workspace, not a metadata workspace member.

```text
mmp-types (wire types, #![no_std])
  ├─ mmp-crypto (suite 0x01; std optional)
  │    ├─ mmp-core (sync/ingest; std optional)
  │    │    ├─ mmp-router (routing; std optional)
  │    │    ├─ mmp-store (std snapshot store)
  │    │    └─ mmp-node (std orchestration)
  │    │         ├─ mmp-cli, mmp-ffi, mmp-sim
  │    │         └─ mmp-node-embassy (manifest depends on core/router/store/TAL; source stub)
  │    └─ mmp-store / mmp-node
  ├─ mmp-tal (TAL event vocabulary; std optional)
  │    ├─ mmp-tal-btleplug, mmp-tal-peripheral, mmp-tal-mobile, mmp-tal-embedded
  │    └─ mmp-node, mmp-cli, mmp-sim, mmp-ffi
  └─ mmp-router / mmp-cli / mmp-ffi / mmp-sim

mmp-rs-code is an independent no_std Reed–Solomon crate.
layer-check is an independent std binary that inspects lower-layer dependency trees and sources.
```

The diagram is a manifest/source dependency summary, not a proof that the
declared architectural inversion holds in every build configuration. The tool
classifies these seven as lower layers: `mmp-types`, `mmp-crypto`, `mmp-core`,
`mmp-router`, `mmp-rs-code`, `mmp-store`, and `mmp-tal`
(`implementation/mesh-milk/tools/layer-check/src/main.rs`). It calls `cargo
tree` for transitive normal dependencies and source-scans `src/` for the two
banned tokens. Its successful run is High-confidence evidence for that limited
rule; it is not a general architecture or `no_std` certification.

## Crate inventory

All members returned by metadata appear here. “Specs” names the implementation
mapping stated in manifest/module comments; it is not a conformance verdict.

| Member | Responsibility and public seam | Workspace dependencies | std / no_std posture observed | Specs | Tests, vectors, and observed gap |
|---|---|---|---|---|---|
| `mmp-types` | Canonical wire vocabulary: `Bundle`, `Frame`, `Hello`, IDs, TLVs, beacon, claims, manifests, payloads, auth certificates (`crates/mmp-types/src/lib.rs`). | none | explicit `#![no_std]`, `alloc`. | 03, 08, 10, 16, 18 | 21 unit/property tests across `tests.rs` and `authcert.rs`, including canonical negatives and decoder no-panic properties. No golden JSON vectors. |
| `mmp-crypto` | Suite-0x01 identity/sign/verify, Noise, sealed/prekey/ratchet/group/receipt/Merkle APIs (`crates/mmp-crypto/src/lib.rs`). | `mmp-types` | `#![no_std]` when `std` feature is off; default feature enables `snow` Noise. | 03, 08, 10, 16 | 12 tests in `src/tests.rs`; no external vector corpus or independent crypto interoperability test. |
| `mmp-core` | Sans-I/O ingest, sync structures, `BundleStore`, `SessionHost`, and std-gated `SessionEngine` (`crates/mmp-core/src/lib.rs`). | `mmp-types`, `mmp-crypto` with crypto defaults disabled | conditional `no_std`; default `std` exposes session engine. | 03, 06, 07, 11, 12, 18 | 17 tests, including full two-engine in-memory sync. Blob availability is advisory and friendship/proxy frames are ignored in `src/session.rs:314-320`. |
| `mmp-router` | Spray/focus handoff, transfer/eviction ordering, utility and origin token buckets (`crates/mmp-router/src/lib.rs`). | `mmp-types`, no-default-feature `mmp-core` | conditional `no_std`; manifest has empty `std` and `alloc` features. | 05, 06, 12, 18 | 8 tests cover formula/ordering/bucket properties; no mobility calibration or adversarial-scale measurement. |
| `mmp-store` | `SnapshotStore`: checksummed file snapshot behind `BundleStore`, plus claim and opaque group-blob staging (`crates/mmp-store/src/lib.rs`). | `mmp-types`, `mmp-crypto`, `mmp-core` | source uses `std::fs`, `std::io`, and paths; no `no_std` attribute. Manifest reserves a future flash log but does not implement it. | 04, 12, 18 | 5 persistence/corruption tests. Snapshot write is temp plus remove/rename; its power-loss/Windows semantics are not fault-injected here. |
| `mmp-tal` | TAL event vocabulary (`TalEvent`, `LinkQuality`, `LinkId`) and in-memory `MemLink`, not a trait object transport (`crates/mmp-tal/src/lib.rs`). | `mmp-types` | conditional `no_std`, `alloc`; default feature set is empty. | 03, 09, 12 | no dedicated `#[test]` scenarios; `MemLink` is used as a seam, not radio evidence. |
| `mmp-node` | std node API: events, direct/group/announcement operations, sessions, quota/receipt/claim glue (`crates/mmp-node/src/lib.rs`). | types, crypto, core, router, store, TAL | std: uses `std::collections` and `rand::StdRng`. | 04, 05, 06, 08, 11, 12, 18 | 13 total unit-module tests: 11 in `tests::` and 2 in `authority_bucket::tests`. In-memory contacts prove protocol paths, not a real scheduler/radio. |
| `mmp-cli` | Desktop/debug binary: key generation, inspect, in-memory demo, BLE central `connect`/`send`, and peripheral `listen` (`crates/mmp-cli/src/main.rs`). | node, core, store, TAL, btleplug TAL, peripheral TAL, types | std/Tokio binary by manifest and source. | 03, 04, 09, 12 | 0 unit tests. CLI control flow and physical BLE commands have no automated result in this baseline. |
| `mmp-tal-btleplug` | Desktop BLE central adapter, UUID constants, events, chunking (`crates/mmp-tal-btleplug/src/lib.rs`, `framing.rs`). | TAL, types | std/Tokio/btleplug/futures by manifest/source. | 09, 12 | 7 deterministic unit tests (link ID and chunks); no device or cross-platform connection test. |
| `mmp-tal-peripheral` | PC peripheral/GATT server/advertiser; Linux/BlueZ implementation, Windows/macOS unsupported API-compatible stubs (`src/lib.rs`, `linux.rs`, `unsupported.rs`). | TAL, types; Linux `bluer`; dev-only central TAL | std/Tokio; target-specific Linux backend. | 09, 12 | Host `x86_64-pc-windows-msvc` selected `unsupported.rs`; 15 shared framing/stub/UUID/link/re-advertise tests ran. `linux.rs`/BlueZ was not compiled or tested in this native baseline, and no Bluetooth daemon/hardware scenario ran. |
| `mmp-tal-mobile` | Re-exports TAL types and `att_chunk(mtu)` for native Kotlin/Swift radio ownership (`crates/mmp-tal-mobile/src/lib.rs`). | TAL | no `#![no_std]` declaration; manifest has no std feature gate. | 09, 12 | 0 tests; no Rust phone radio implementation or generated binding test. |
| `mmp-tal-embedded` | Placeholder nRF52/ESP32 TAL re-export. | TAL | explicit `#![no_std]`. | 09, 12 | 0 tests; source explicitly says target TAL is out of MVP scope. |
| `mmp-node-embassy` | Placeholder embedded executor shell. | core, router, store, TAL | explicit `#![no_std]`; source contains only module documentation and the attribute. | 12 | 0 tests; no executor/API behavior exists at this HEAD. |
| `mmp-ffi` | UniFFI-facing `MmpNode`, FFI events/errors/tier/state, persistent shared-store adapter; cdylib/staticlib/lib (`crates/mmp-ffi/src/lib.rs`, `store.rs`). | node, core, store, TAL, types | std, `Mutex`/`Arc`, UniFFI; mobile bindings binary is opt-in `cli`. | 04, 12 | 7 Rust-surface tests: in-memory delivery, argument errors, restart persistence, claim/group restore, beacon behavior. Kotlin/Swift generation and app calls are not tested. |
| `mmp-sim` | Deterministic virtual-time, in-memory meeting simulator (`crates/mmp-sim/src/lib.rs`). | types, core, node, TAL | std (`HashSet`, test `Instant`). | 05, 12, 13 | 3 active tests and 1 ignored 100-node stress. It does not model radios, BLE contention, battery, OS scheduling, movement, loss, or adversarial behavior. |
| `mmp-rs-code` | GF(256) Reed–Solomon `encode`/`decode` with k<=64/n<=255 (`crates/mmp-rs-code/src/lib.rs`). | none | conditional `no_std`, `alloc`; default features empty. | 08, 12 | 3 unit tests. It is not a dependency of node/core and does not prove integrated blob transfer; deliberately corrupted parity can return wrong data, with integrity delegated to Merkle. |
| `layer-check` | Metadata workspace tool: dependency/source token gate (`tools/layer-check/src/main.rs`). | none | std binary (`std::process`, filesystem). | 12 | 0 unit tests; explicit run passed. It checks seven designated lower layers, not every member or target. |

The source-level `no_std` claims in this table have deliberately been stated
only where the attribute/feature gate was observed. Passing the default native
workspace build is not evidence that all feature-minimal or embedded targets
build; no cross compilation was requested or performed.

## Public protocol seams

The implementation exposes several useful, narrow seams. Their presence is
evidence of structure, not evidence of end-to-end conformance.

- Wire boundary — `mmp_types::Bundle::decode_canonical`, `Frame::decode_payload`,
  `read_envelope`, strict `Reader`, typed TLVs and `Beacon::decode`
  (`crates/mmp-types/src/{bundle,frame,wire,tlv,beacon}.rs`). Tests explicitly
  cover trailing bytes, reserved flags, TLV order/criticality, oversized envelope
  prefix, lifetime-zeroed IDs, frame round trips, and byte mutation
  (`crates/mmp-types/src/tests.rs`). Confidence: High for those exercised inputs.

- Verification and key boundary — `Identity::sign`, strict `verify`, Noise XX,
  sealed-box, prekey, ratchet, sender-key, receipt, and Merkle APIs
  (`crates/mmp-crypto/src/{identity,noise,sealed,prekey,ratchet,group,receipt,merkle}.rs`).
  `sign_verify_and_cross_domain_rejects`, `noise_xx_handshake_agrees_and_binds`,
  `ratchet_out_of_order_within_window`, and `merkle_path_verifies_and_corruption_fails`
  exercise representative success/negative cases. Confidence: High only for those
  scenarios; no test demonstrates a third-party implementation interoperating.

- Storage/host boundary — `BundleStore` and `SessionHost` in
  `crates/mmp-core/src/{store,session}.rs`. The session engine requests IDs,
  summaries, handoff bytes/budget, ingest, wants, and peer admission from a
  host. `full_session_syncs_two_stores` uses a test host and validates a two-sided
  in-memory run. `SnapshotStore` is one std implementation; `MemStore` is the
  reference/simulation store. Confidence: High for the tested in-memory path;
  Medium for crash/restart behavior limited to snapshot tests.

- Transport boundary — `mmp_tal::TalEvent` and `Node::{on_tal_event,poll_send}`
  (`crates/mmp-tal/src/lib.rs`, `crates/mmp-node/src/lib.rs:534-590`). The node
  remains host-driven by a caller-supplied monotonic `tick` value. The central
  desktop TAL and Linux peripheral TAL map real adapters to that vocabulary, but
  their automated tests cover framing/IDs/constants rather than hardware contact.

- Application/mobile boundary — `mmp_ffi::MmpNode` maps byte vectors and events
  over UniFFI (`crates/mmp-ffi/src/lib.rs`); the tests drive the exact Rust FFI
  methods for a two-node contact. `mmp-tal-mobile` is intentionally only shared
  types plus ATT chunk arithmetic, leaving Android/iOS scanning, advertising and
  GATT to native hosts. Confidence: High for Rust-wrapper inputs; Low for mobile
  platform execution.

- Policy boundary — router functions consume `BundleMeta` and local utility;
  authority verification is in `AuthorityStore` while rate admission is in
  `mmp-node::AuthorityBuckets` (`crates/mmp-core/src/authority.rs`,
  `crates/mmp-node/src/authority_bucket.rs`). The split is testable and has
  explicit tests for pinning, caps, sticky revocation, and per-subject budgets.

## Test and vector inventory

The active test total is 111. The useful grouping, based on the test output and
source locations, is: `mmp-types` 21; `mmp-crypto` 12; `mmp-core` 17;
`mmp-router` 8; `mmp-store` 5; `mmp-node` 13 (11 `tests::` plus 2
`authority_bucket::tests`);
`mmp-ffi` 7; `mmp-sim` 3 active plus 1 ignored; `mmp-rs-code` 3;
`mmp-tal-btleplug` 7; `mmp-tal-peripheral` 15; `mmp-cli`, `mmp-tal`,
`mmp-tal-mobile`, `mmp-tal-embedded`, `mmp-node-embassy`, and `layer-check`
each 0. This is a count of tests run, not a coverage measurement.

Representative executable evidence includes:

- wire/parser: `mmp-types/src/tests.rs::{trailing_byte_rejected,
  tlv_out_of_order_rejected,unknown_critical_tlv_rejected,
  envelope_oversize_prefix_rejected,mutate_any_byte,frame_decoder_never_panics,
  bundle_decoder_never_panics}`;
- ingest/sync: `mmp-core/src/tests.rs::{ingest_stores_verified_and_drops_bad_signature,
  seen_index_defeats_lifetime_resurrection,receipt_filter_hint_never_purges,
  rbsr_converges_on_disjoint_and_overlapping_sets,full_session_syncs_two_stores,
  handshake_timeout_closes_session}`;
- node behavior: `mmp-node/src/tests.rs::{direct_message_delivery_and_receipt,
  three_node_store_carry_forward,group_create_invite_and_message,
  blocked_peer_is_silently_denied,valid_attached_cert_elevates_priority_and_renders_official,
  revocation_gossip_clamps_a_previously_elevated_subject_and_stays_sticky}`;
- persistence/FFI: `mmp-store/src/lib.rs::{snapshot_roundtrip_and_corruption_drop,
  truncated_file_loads_with_no_records_and_no_panic,
  tampered_claim_signature_is_dropped_even_with_a_valid_checksum}` and
  `mmp-ffi/src/tests.rs::{state_survives_restart,
  claims_and_group_membership_survive_restart}`.

`implementation/mesh-milk/vectors/README.md` was read. It explicitly says the
directory contains no vectors at Stage 0; an inventory found **zero JSON files**
under `implementation/mesh-milk/vectors/`. Thus there are no actual vector names,
bytes, semantics, or independent decoder results to report. The README plans
fixed-key vectors for every frame/bundle plus negative cross-domain,
non-canonical/overlarge-S, lifetime-reset, and budget-clamp cases. Those plans
are not executable evidence at this HEAD. Confidence: High for absence; Low for
the unmaterialized plan.

`implementation/mesh-milk/fuzz/Cargo.toml` describes a standalone
`cargo-fuzz` workspace with four targets, all present and read:

| Target | Entry point / intended semantics | Evidence limit |
|---|---|---|
| `fuzz_beacon` | `Beacon::decode`, unauthenticated 24-byte broadcast parser. | harness exists; no fuzz run or corpus evidence here. |
| `fuzz_envelope` | `read_envelope` then `Frame::decode_payload`, including pre-auth framing. | harness exists; no fuzz run or corpus evidence here. |
| `fuzz_bundle` | `Bundle::decode_canonical` before signature verification. | harness exists; no fuzz run or corpus evidence here. |
| `fuzz_hs` | `NoiseHandshake::responder(...).advance()` on arbitrary first handshake bytes. | harness exists; no fuzz run or corpus evidence here. |

The fuzz README says there is no curated seed corpus and that sanitizer-backed
fuzzing is practically Linux/macOS/CI work; the required workspace test/clippy
commands neither build nor execute cargo-fuzz. Treat “no panic/no OOM” in the
README as the target invariant, not as a demonstrated result.

## Simulation capabilities

`mmp-sim` is a deterministic, discrete-event test harness rather than a radio
or battery simulator. `Sim::new(n, seed)` derives reproducible node identities;
`run_random_meetings` chooses uniform distinct pairs and virtual gaps; `meet`
opens `TalEvent::LinkUp`, pumps up to 300 cycles, closes the link, advances a
virtual clock, and counts tracked sends/receipts (`crates/mmp-sim/src/lib.rs`).

Actual committed scenarios/seeds are:

- `ten_node_random_mesh_delivers_and_receipts`: 10 nodes, seed
  `0xC0FF_EE12_3456_789A`, one introduced pair, 100 random 1–5 minute meetings
  plus five direct meetings; asserts one receive, one delivery receipt, ratio
  1.0 and quota bound.
- `thirty_node_mesh_delivers_ci`: 30 nodes, seed `0x27CB_609F_7C74_6C69`, five
  disjoint introduced pairs, five messages and 400 random meetings; asserts
  ratio >=0.8 and the conservative quota predicate.
- `hundred_node_mesh_stress`: ignored by default, 100 nodes, seed
  `0x1000_0000_0000_0000`, ten pairs, ten messages, 8,000 random meetings;
  asserts ratio >=0.9 and quota predicate if invoked separately.
- `rng_is_deterministic_for_a_fixed_seed`: checks 50 PRNG draws for seed 42.

The default run demonstrated the 10/30-node tests, not the ignored 100-node
test. `quota_ok()` bounds storage as `store_len * MAX_BUNDLE`, so it is a
conservative externally observable condition rather than a direct byte-accounting
assertion. There is no topology/mobility distribution, loss/MTU/goodput model,
collision model, energy model, OS behavior, adversary, repeated randomized seed
sweep, latency distribution, or 1k/10k scenario. Therefore the simulator is
High-confidence evidence for its fixed deterministic test paths, not evidence
for the project’s scale, battery, or real-world delivery targets.

## Mobile and embedded boundaries

Mobile has a Rust FFI/control boundary but not a shipped Rust BLE backend.
`mmp-ffi` exposes a cdylib/staticlib/lib and serializes calls through `MmpNode`;
the caller supplies `link_up`, `link_data`, `link_down`, `tick`, and polls
outbound frames/events. `mmp-tal-mobile/src/lib.rs` says Android/iOS native
Kotlin/Swift own scanning, advertising, and GATT; it only exports shared TAL
types and `att_chunk`. The FFI source tests never invoke generated Kotlin/Swift
bindings, as their own module comment states.

Embedded is deliberately incomplete: `mmp-tal-embedded/src/lib.rs` is a
`#![no_std]` re-export and `mmp-node-embassy/src/lib.rs` contains no executable
orchestration beyond `#![no_std]`. No nRF52, ESP32, Embassy, flash-log, or
`thumbv7em-none-eabihf` build/test was performed. The toolchain target listed in
`implementation/mesh-milk/rust-toolchain.toml` is therefore intent, not build
evidence.

Desktop transport has a stronger source implementation boundary but still limited
execution evidence. `mmp-tal-btleplug` is a Tokio/btleplug central adapter.
For `mmp-tal-peripheral`, the native host was `x86_64-pc-windows-msvc`, so
`unsupported.rs`, not the cfg-gated `linux.rs`/BlueZ backend, was selected.
The workspace native test therefore evidences only shared framing/stub paths;
it did not compile or test Linux/BlueZ, advertise, or connect to a physical
device or daemon.

## Specification-conformance evidence

The following maps implementation to specification areas without promoting
presence to proof.

| Specification area | Committed implementation evidence | Confidence and limit |
|---|---|---|
| 03 / 10 canonical bundle and frame grammar | `mmp-types` canonical codec, strict reader/TLV validation and 18 codec tests; `mmp-core::ingest` uses decode before verification/storage. | High for named parser/test cases; no golden vectors or independent implementation. |
| 03 / 04 verify-before-store, bounded lifetime and dedup | `mmp-core/src/ingest.rs`, `SeenIndex`, `BundleMeta`; tested by bad-signature, lifetime clamp and resurrection tests. | High for in-memory test cases; durable crash and all malformed cases are not exhaustively proven. |
| 05 routing and quota | router’s spray/focus, local utility, eviction/transfer order, origin bucket functions and eight unit tests; node integration validates a three-node carry-forward. | Medium: formula/path tests exist, but no empirical routing/scale validation. |
| 06 anti-entropy, claims, receipt and group state | RBSR, LWW claims, DAG, receipt Bloom/exact state, OR-Set; convergence/filter tests and full session sync. | Medium: representative properties are tested, but no large/adversarial reconciliation study and no D4 range reconciliation. |
| 08 crypto, groups and blobs | Suite primitives and tests for signature domain separation, Noise, sealed/prekey, ratchet, sender keys, receipt MAC, Merkle; standalone RS tests. | Medium: local test evidence only; no audit/interoperability; RS/Merkle is not connected to a node blob transfer. |
| 09 transport framing | TAL event seam, central/peripheral UUID/framing tests and ATT calculations. | Medium for byte utilities; Low for actual BLE behavior because no hardware test ran. |
| 11 session states/timers | `SessionEngine` phases, protocol-error closing, injected monotonic time; full sync and handshake timeout tests. | Medium: normal/tested paths are covered; malformed phase combinations and runtime radio timing are not exhaustively evidenced. |
| 12 crate/layer plan | manifest dependency shape and successful `layer-check` on seven lower layers. | Medium: bounded mechanical check; does not verify all target features or platform builds. |
| 16 file transfer extension | `FileManifest`, payload tags, symbol TLV, Merkle and RS codecs parse/round-trip locally. | Low for transfer conformance: node/session has no integrated symbol transfer; BlobReq/BlobHave are advisory. |
| 18 authority announcements | AuthCert/announcement/revocation codecs, authority store, rate buckets, node events; pin/cap/revocation/elevation tests. | Medium: in-memory scenarios cover selected rules, while root distribution is deferred and authority/bucket state is RAM-only. |

## Implementation-only behavior

Several behaviors are visible in code but should not silently become normative:

- `mmp-store` uses `MMPSNAP2`, record-kind tags, BLAKE3 record checksums, and
  temp/remove/rename snapshot persistence (`crates/mmp-store/src/lib.rs`). The
  snapshot format and its default repair behavior are implementation decisions.
- `mmp-node` derives deterministic `StdRng` and beacon salt from the identity
  seed, makes a fail-closed authority wall clock default to zero, and holds
  authority/token-bucket state only in RAM (`crates/mmp-node/src/lib.rs` and
  `authority_bucket.rs`). These choices influence restart behavior but are not
  demonstrated across live deployments.
- `mmp-sim` uses xorshift64* and a 300-pump-cycle contact limit; it counts only
  tracked message events. These make deterministic CI scenarios possible but
  are not a contact/radio model.
- `mmp-rs-code::decode` intentionally treats corruption as an erasure-coding
  problem and may yield wrong output rather than detect it; its own test names
  this (`corrupted_parity_yields_wrong_data_not_panic`). Merkle verification is
  expected to provide integrity, but this head contains no integrated path.
- `mmp-tal-peripheral` chooses target OS cfg tables rather than a cargo feature
  for Linux/Windows/macOS selection. The unsupported platform errors and UUID
  identity are concrete adapter policy, not a claim of portable peripheral BLE.

## Gaps and teaching implications

The largest audit-relevant gaps are explicit rather than hidden:

- No JSON golden vectors and no independent decoder cross-check, despite the
  vector README’s planned coverage.
- Fuzz harnesses exist but have no committed corpus and were not run; build/test
  success is not fuzz evidence.
- No hardware BLE/mobile execution evidence; Android/iOS integration, embedded
  TAL, Embassy orchestration, Windows/macOS peripheral roles, and flash storage
  are not implemented or not exercised here.
- Blob/file transfer is only partly represented by codecs: session treats blob
  availability as advisory, and `mmp-rs-code` has no node/core dependency edge.
  Friendship/proxy frame types parse but the session explicitly ignores them.
- The simulator tests deterministic small/medium fixed seeds; the 100-node case
  is ignored, and no 1k/10k, energy, OS, loss, adversarial, or statistical
  validation is present.
- Passing local tests/clippy and layer-check are valuable regression signals but
  do not substitute for cryptographic review, interoperability vectors,
  target-feature builds, persistence fault injection, or measurement against
  latency/battery/scale requirements.

For teaching, use this implementation as a map of seams and falsifiable local
claims: trace one bundle from canonical decode through signature/ingest, store,
RBSR and handoff; contrast exact receipts with Bloom hints; then identify which
claims are only code comments, deterministic tests, simulator assumptions, or
unimplemented surfaces. The right conclusion from the green baseline is that
the named executable scenarios currently pass in this isolated checkout—not
that the protocol is complete or production-ready.
