# MMP/1 — Roadmap to Production

Audience: engineers and reviewers who need to know what mesh_milk's Rust implementation actually does today, what's missing before it can ship to real users on real radios, and in what order the remaining work should be attacked. Companion documents: `implementation/mesh-milk/MOBILE_INTEGRATION.md` (integration seam contract), `implementation/mesh-milk/REACT_NATIVE_INTEGRATION.md`, `implementation/mesh-milk/V2_FEATURES.md` (protocol-level v2 features and hidden v1 powers), `docs/USER_FEATURES.md` (product-facing feature catalog derived from this roadmap).

Every claim below was checked against the workspace at `implementation/mesh-milk` on 2026-07-11: `cargo test --workspace` (71 passed, 1 ignored, 0 failed), `cargo clippy --workspace --all-targets -- -D warnings` (clean), and direct inspection of the named source files. Where inspection turned up something the initial gap analysis got wrong, this document follows the code and flags the correction inline.

**Addendum, 2026-07-16:** the workspace has grown substantially since the 07-11 baseline above (Phase A radio work, persistence, and now E5-S5 authority announcements — see `docs/AGILE_BACKLOG.md` and `docs/IMPLEMENTATION_HANDBOOK.md` §4.10 for the latest). `cargo test --workspace` currently passes **111 tests, 0 failed, 1 ignored**; `clippy -D warnings` and `layer-check` remain clean. The per-crate table below is not fully re-verified line-by-line as of this addendum — treat the crate descriptions as directionally accurate and the newest work (mmp-types §authcert, mmp-core §authority, mmp-node §authority_bucket/announcements) as the most current addition on top of it.

**Addendum, 2026-08-08:** E5-S4 is now committed (`b07a0b7`, `c34069b`, `66786fb` and follow-up documentation): deterministic envelope, bundle, beacon, HELLO, frame-payload, negative, and cross-domain artifacts are under `implementation/mesh-milk/vectors/`, with independent readers and `coverage.json`. The CLI snapshot workflow was corrected in `a73f747`: `connect` now restores persisted claims/groups/replay/receipt state, and exit-time persistence stages the same state before writing; a restart regression test covers this path. The serial workspace matrix passes, `mmp-cli` has 26 tests, clippy is clean, and `layer-check` passes. A real two-machine BLE exchange remains unverified because this Windows host has no usable Linux/BlueZ peripheral path; see `docs/MANUAL_INTEROP_TESTING.md` and `docs/E5-S4_CONFORMANCE_HANDOFF.md` for the reproducible procedure and evidence boundary.

**Current checkpoint, 2026-08-08 (continuation after `0343cf5`):** The historical counts and
commit list above are not the current verification record. Since that addendum,
the implementation has added strict outbound/inbound frame caps, sender-key
provenance guards, bounded hostile-count and file-manifest parsing, bounded
Bloom/SeenIndex/ReceiptFilter/OR-Set/GroupState snapshot decoding, and bounded
SessionEngine RX reassembly, and tier-derived concurrent session admission
(Leaf/Relay/Anchor = 1/2/8, with silent duplicate/quota denial). The
normative HELLO/CREDIT frame-flow-control path is now enforced with bounded
deferred frames and atomic bundle handoff deferral. The node outbound queue is
also bounded per admitted session (eight data-credit frames plus four control
reserve frames), with tier-derived global frame/byte limits and affected-link
atomic teardown on overflow. The central `mmp-tal-btleplug` event ingress is
also bounded to a 2048-event public FIFO fed by a 64-record normal control
ledger, a bounded 64-entry emergency `LinkDown` tombstone lane, and a
1,984-record data lane. Producers never await the public receiver:
data overflow retires only the affected link, while control overflow fails
closed after live state removal. The Linux peripheral API now applies the same
two-lane policy;
central per-link TX chunk serialization and monotonic generation fencing are
enforced for central reconnect, teardown, adapter disconnect, and RX EOF. The
Linux peripheral now has direction-specific endpoint tokens and generation
fencing for reader/writer replacement and teardown. Both backends sweep links
on stream termination and bound external disconnect awaits outside lifecycle
locks; peripheral retired endpoint identities are capped at 256 per direction
and fail closed rather than evicting history. Reserved control capacity,
bounded emergency teardown, and overflow retirement are implemented;
Linux/BlueZ build and hardware validation remain open. The latest
serial workspace matrix passes with zero failures; the current handoff records the exact per-crate counts and
ignored simulation cases in `docs/E5-S4_CONFORMANCE_HANDOFF.md`. The
specification remains unchanged. The real Linux/BlueZ two-host exchange and a
successful remote CI artifact run are still unverified; mobile remains an
FFI/API bridge until native Android/iOS consuming radio shells exist. Treat
the per-crate table below and the older addenda as historical unless a row is
explicitly refreshed by a current command run.

The current continuation also adds an executable encrypted-`ID_PROOF` boundary
regression in `mmp-core` and a Byzantine double-grant simulation that records
the receiver-local, rather than global, copy-budget invariant. The focused
`mmp-core` suite now passes 42 unit tests; `mmp-sim` passes 10 active tests and
4 intentionally ignored deployment/scale cases.

The `a4ecfb8` transport lifecycle slice added 21 Windows-target peripheral
tests (including endpoint-token replacement and stale opposite-direction
callbacks) in addition to the 12 central lifecycle tests. Follow-up
`0343cf5` adds four peripheral ingress tests and four central ingress tests,
including full-data-lane control delivery, stale-stream rejection,
replacement ordering, and nonblocking enqueue. The continuation adds
emergency-down ordering, explicit retired-link draining, endpoint-cap
fail-closed handling, and EOF-triggered sweeps. Workspace tests, workspace
clippy, and `layer-check` pass; direct Linux/BlueZ execution remains a
hardware/sysroot gate, and the specification directory remains unchanged.

**Addendum, 2026-08-10:** the continuation after the 2026-08-08 checkpoint
landed four bounded correctness slices without changing `specification/`:
`25fcc62` exposes typed self-revocation failure through the FFI and fixes the
existing successor wrapper's `Ok(empty)` ambiguity; `b9f39cd` documents and
tests zero-mutation `StorePutError::Capacity` replacement rejection;
`b794180` locks host CI dependency resolution; and the earlier file/claim
producer slices add destination-safe deduplication and manifest-only retry
coverage. Focused evidence is 34 passing `mmp-ffi` tests, 136 passing
`mmp-node` tests, four bounded snapshot-provider tests, 42 mobile source and
artifact checks with four expected skips, and clean focused Clippy. Generated
Kotlin/Swift regeneration, native Android/iOS runtime gates, Linux/BlueZ
two-host exchange, and hardware battery measurements remain open. Existing
dirty user-owned files and provisional candidates were not staged.

---

## 1. Where we are (implemented and verified)

| Crate | What it does | Evidence |
|---|---|---|
| `mmp-types` | Canonical hand-written wire codec — no serde on the wire, ever. `Bundle` canonicalizes with a **zeroed lifetime field** before signing/hashing (audit finding F1: signing the live lifetime breaks resurrection-resistant dedup and cross-implementation interop). Domain-separated signatures per audit finding F4 (`domain.rs`, doc 10 §3.1). Strict TLV parsing (unknown-critical rejected, out-of-order rejected, reserved bits rejected, trailing bytes rejected). All session frames (HELLO/ID_PROOF/SUMMARY/RBSR/BUNDLE/BUDGET/BLOB_REQ/BLOB_HAVE/FRIEND_OFFER/FRIEND_ACK/PROXY_SUMMARY/ERROR/CLOSE). 24-byte beacon. Claims and file-manifest payload schemas. | 16 tests (`crates/mmp-types/src/tests.rs`) incl. negative vectors (`reserved_flag_bits_rejected`, `unknown_critical_tlv_rejected`, `tlv_out_of_order_rejected`, `trailing_byte_rejected`, `envelope_oversize_prefix_rejected`) and proptest mutation fuzzing (`props::mutate_any_byte`, `props::frame_decoder_never_panics`, `props::bundle_decoder_never_panics`). |
| `mmp-crypto` | Ed25519 `verify_strict` only (F9 — permissive verify accepts malleable-S signatures). Noise XX handshake with ID_PROOF channel binding (anti-unknown-key-share). Sealed box. X3DH-adapted serverless prekeys. Ratchet-lite with a skipped-message-key window of 512. Group sender keys with committed AEAD. Receipt MAC with constant-time comparison. BLAKE3 Merkle tree, arity 4. | 12 tests (`crates/mmp-crypto/src/tests.rs`): `sign_verify_and_cross_domain_rejects`, `noise_xx_handshake_agrees_and_binds`, `id_proof_from_wrong_identity_rejected`, `sealed_box_roundtrip`, `prekey_agreement_matches_both_sides`, `prekey_batch_forged_signature_rejected`, `ratchet_out_of_order_within_window`, `ratchet_epoch_bump_stays_in_sync`, `sender_keys_group_roundtrip`, `receipt_mac_roundtrip`, `merkle_path_verifies_and_corruption_fails`, `conversation_id_is_commutative`. |
| `mmp-core` | Ingest pipeline in doc 03 §4.1.2 order (verify-before-store, verify-before-forward, unconditionally). Resurrection-resistant `SeenIndex` (entries keyed on class-max lifetime **from first sight**, not the peer's claimed lifetime — F1's other half). `ReceiptFilter` with the Hint/Purge type asymmetry (F11: a Bloom hit only deprioritizes, never deletes; exact D0 receipts purge) plus the wire mini-filter Bloom fold (doc 06 §9, `bloom.rs`). Claims LWW merge with deterministic tie-break and conflict flagging. Sans-IO `SessionEngine` implementing the doc 11 §3 FSM. RBSR reconciliation (doc 06 §3). Conversation DAG (`dag.rs`). Group OR-Set with epoch-anchor GC (F5 — tombstone compaction only past a mutually-acknowledged epoch anchor). | 10 tests (`crates/mmp-core/src/tests.rs`): `ingest_stores_verified_and_drops_bad_signature`, `seen_index_defeats_lifetime_resurrection`, `lifetime_clamped_on_receive`, `receipt_filter_hint_never_purges`, `claims_lww_merge_is_deterministic_and_flags_conflicts`, `handshake_timeout_closes_session`, `full_session_syncs_two_stores`, `rbsr_converges_on_disjoint_and_overlapping_sets`, `fingerprint_is_order_and_content_sensitive`, `orset_converges_and_compacts`. |
| `mmp-router` | Binary Spray-and-Wait with Focus (doc 05 §2). Locally-observed utility, PRoPHET-style but explicitly **no transitivity** (a deliberate poisoning-resistance choice, not an oversight — `utility.rs`). Population estimate N̂ (doc 05 §3). Per-origin token buckets with a ×4 web-of-trust multiplier (`quota.rs`). Eviction and transfer ordering. **Recorded lesson:** a sentinel-zero bug in the copy-budget `tie()` comparator was found and fixed during implementation — kept in `handoff_rules`/`spray_budget_is_conserved_over_arbitrary_trees` test coverage as a regression guard. | 7 tests (`crates/mmp-router/src/tests.rs`): `utility_grows_with_contacts_and_ages`, `origin_budget_tracks_population`, `token_bucket_bounds_an_origin`, `transfer_order_is_value_dense`, `eviction_protects_own_originals_and_prefers_confirmed`, `handoff_rules`, `spray_budget_is_conserved_over_arbitrary_trees`. |
| `mmp-node` | Full orchestration: session lifecycle, claims gossip, direct messaging (X3DH first contact + ratchet), groups (pairwise own-key invite/remove/epoch rotation with authenticated sender-key provenance and stale/duplicate rejection), receipts with origin-only purge gate, beacons with ephemeral-ID rotation, tier ladder (doc 11 §2), diagnostics journey log (`diag.rs`, the mechanism behind V2_FEATURES.md Part II §B). | 8 integration tests (`crates/mmp-node/src/lib.rs`): `beacon_rotates_with_slot`, `blocked_peer_is_silently_denied`, `spray_budget_flows_and_message_marked_in_mesh`, `direct_message_delivery_and_receipt`, **`three_node_store_carry_forward`** (store-carry-forward with receipt back-propagation across 3 nodes), `group_create_invite_and_message`, `sender_key_member_must_match_authenticated_origin`, and `removed_member_cannot_reintroduce_stale_sender_key`. |
| `mmp-store` | `SnapshotStore` — checksummed atomic-write persistence, corrupt-record skip on load. | 1 test: `snapshot_roundtrip_and_corruption_drop`. |
| `mmp-rs-code` | `no_std` GF(256) systematic Reed-Solomon. | 3 tests: `systematic_and_recovers_from_any_k`, `duplicate_index_rejected`, `corrupted_parity_yields_wrong_data_not_panic`. |
| `mmp-tal` + `mmp-tal-btleplug` + `mmp-tal-peripheral` | TAL vocabulary (`mmp-tal`, 0 tests — a trait/type crate). Central-role BLE in `mmp-tal-btleplug`: scan, dial, GATT write/notify shuttle, chunking, `LinkId` minting. **Peripheral role (M5b) is implemented in code** in the sibling crate `mmp-tal-peripheral`: Linux backend on `bluer` (BlueZ D-Bus GATT server), Windows/macOS are `PeriphError::Unsupported` stubs with a concrete design sketch for the WinRT path. The current stream-token and bounded-ingress hardening is hardware-independent tested on Windows; the Linux build/runtime and real BlueZ exchange remain environment- and hardware-gated. The development service and characteristic UUIDs have one source of truth in `mmp-tal`; the peripheral crate re-exports them with the central-role inversion made explicit. `mmp-cli` exposes both roles (`connect`=central, `listen`=peripheral). | `mmp-tal-btleplug`: 15 focused tests (including generation fencing, bounded ingress, and control-saturation retirement). `mmp-tal-peripheral`: 27 Windows-target tests (including stream fencing, bounded ingress, emergency-down ordering, retired-link draining, and the retired-endpoint cap); Linux/BlueZ execution is not current evidence. |
| `mmp-ffi` | Complete UniFFI 0.29 surface: constructor takes seed + tier + optional `SnapshotStore` path, transport injection including `on_beacon`, polling (`poll_send`/`poll_event`), groups, claims, `flush()`/persistence policy. Secrets never cross the FFI boundary outward (the FFI type carries public keys, ciphertexts, and events only). | 6 tests: `bad_length_ids_are_rejected_at_the_ffi_boundary`, `unknown_peer_is_reported`, `beacon_is_accepted_with_no_observable_event`, `group_remove_and_set_tier_roundtrip`, `state_survives_restart`, `direct_message_delivery_via_ffi_surface`. |
| `mmp-cli` | `keygen` / `demo` / `inspect` subcommands, plus **`connect`** (central-role drive loop over `mmp-tal-btleplug`) and **`listen`** (peripheral-role drive loop over `mmp-tal-peripheral`) and `send` (queue-and-wait-for-delivery over `connect`'s loop). Both radio subcommands verified error-clean on hosts without working BLE hardware/adapters (graceful exit 2, no panics); the actual two-machine radio exchange (Phase A's exit gate) still needs a real multi-host BLE test, not yet run. | Hand-rolled `std::env` arg parsing; exercised by the `demo` path plus manual no-hardware error-path runs of `connect`/`send`/`listen`, no dedicated unit tests (radio subcommands are inherently interactive/hardware-driven). |
| `mmp-sim` | 10-node and 30-node meshes run in CI; adversarial replay, flood, quota, eclipse, Bloom-poison, and Byzantine double-grant scenarios are active; deployment/scale drills remain opt-in. | 10 active + 4 ignored: deterministic/random delivery, six adversarial scenarios, file transfer, and four ignored deployment/scale cases. |
| `mmp-tal-embedded`, `mmp-node-embassy` | Stubs only (11 and 7 lines respectively) — `no_std`-clean placeholders. `mmp-tal-embedded`'s doc comment records this as a deliberate scope decision ("keep the `no_std` discipline, ship no embedded TAL") with the acceptance gate spelled out: ≤256 KiB flash / ≤48 KiB RAM (doc 12 §7.6) plus the OQ-7 Reed-Solomon decode benchmark. | 0 tests — nothing to test yet. |
| Docs | `MOBILE_INTEGRATION.md`, `REACT_NATIVE_INTEGRATION.md`, `V2_FEATURES.md` all present at `implementation/mesh-milk/`. | — |
| Workspace-wide | **Historical baseline: 71 tests pass, 0 fail, 1 ignored.** The current checkpoint is recorded in the dated addendum above and in `docs/E5-S4_CONFORMANCE_HANDOFF.md`; this row is retained as the original audit baseline, not as current evidence. | Historical 2026-07-11 run. |

**Note on the gap-analysis outline vs. the code:** every specific technical claim in the original outline (F1/F4/F5/F9/F11 audit findings, the 71-test total, per-crate test counts, the Spray-and-Wait+Focus/N̂/token-bucket router design, the peripheral-role gap, the OQ-7/OQ-12 embedded and BLE gates, the FRIEND_*/BlobReq/BlobHave stub status, doc-section cross-references) checked out exactly against the source. The one place worth flagging: doc 07 §8 is titled "Persona review checkpoints," not a dedicated fuzzing section — but its Skeptical Professor entry is precisely where the fuzzing mandate is recorded ("doc 12 mandates fuzzing of all pre-auth parsers"), so citing doc 07 §8 for the fuzz mandate is accurate, just indirect (the actual mandate text lives in doc 12 §7). This document cites it as doc 07 §8 (mandate origin) / doc 12 §7 (mandate text) throughout.

---

## 2. Gap analysis (what remains)

**(a) Protocol completeness**

**Current-status correction (2026-08-09):** the three historical bullets below
predate the shipped `mmp-node` blob/friendship/revocation work. The current
implementation has class-4 symbol transfer plus `BLOB_HAVE`/receive-side
`BLOB_REQ` serving, friend-offer/ack buffering with advisory proxy summaries,
and generalized field-`0x09` revocation/succession handling. Remaining gaps
are the deferred origin-targeted `BLOB_REQ` scheduler, offline patron proxy
reconciliation, and native keystore persistence; see the Phase C/B entries
and the implementation handoffs for the precise boundaries.

- Blob plane: manifests, Reed–Solomon symbols, `BLOB_HAVE`, and receive-side `BLOB_REQ`/`BLOB_HAVE` serving are integrated in `mmp-node` (see `Node::fetch_file`, `blob_frames`, and `blob_serve_candidates`). The remaining MVP limitation is deliberate: requesters emit `BLOB_HAVE` only; an origin-targeted `BLOB_REQ` scheduler, per-domain airtime accounting, and streaming symbol production remain deferred. See doc 16 (file transfer).
- Friendship/proxy buffering: `FRIEND_OFFER`/`FRIEND_ACK`, friend-quota pinning, and advisory `PROXY_SUMMARY` hints are implemented in `mmp-node` and exercised by positive/negative tests. The deferred pieces are offline patron reconciliation, signed proxy-summary validity horizons, ephem-ID resolution, and a real poll schedule; see doc 06 §7.
- Key revocation/succession claims: the generalized field `0x09` claim is implemented in `mmp-types`, `mmp-core`, `mmp-node`, and the FFI surface, including verified successor countersignatures and session/send refusal after revocation. The remaining work is native persistence/keystore integration, not the wire claim itself.
- Full state persistence beyond bundles: `SnapshotStore` persists the bundle table, verified claims, group blobs, `SeenIndex`, and `ReceiptFilter`; the CLI and FFI hosts restore and stage those records across restarts. Sender-key secrets/distributions and session-ratchet state remain non-persistent by design. Ratchet persistence still needs the explicit platform-keystore/PCS design described in Phase B; it must not be inferred from the already-shipped non-secret snapshot records.

**(b) Transports**
- M5b: PC peripheral role. **Implemented for Linux/BlueZ** in `mmp-tal-peripheral` and wired to `mmp-cli listen`; Windows/macOS remain explicit unsupported stubs. The real two-machine BLE exchange is still the Phase A hardware exit gate.
- Native mobile GATT server + client reference modules (Kotlin/Swift) per `MOBILE_INTEGRATION.md` §4–§5 — the guide specifies the contract; the reference modules themselves aren't in this repository.
- GATT UUID pinning + Bluetooth SIG 16-bit UUID assignment (OQ-12, specification/research/open-questions.md — explicitly called a "production release blocker" there). Current state, verified in code: `mmp-tal-btleplug` uses development-placeholder 128-bit UUIDs for both TX/RX characteristics and the service, and its doc comment states plainly that these placeholders "must match whatever the mobile GATT server publishes; that doc does not yet pin concrete values either, so reconciling the two is a prerequisite of the first real PC↔phone contact."
- MTU negotiation: `btleplug` 0.11.7 exposes no portable MTU query on any backend (verified against the crate's `api` surface per the module doc), so every link falls back to the conservative 20-byte chunk (23-byte ATT floor − 3-byte header) for its whole lifetime. The 244/247-byte target from doc 09 §4 is "aspirational until a portable MTU signal exists."
- Background scan/advertise duty cycles vs. battery budget: doc 09 §7 (Coexistence and airtime fairness) and doc 13 §1 (phone energy budget, <5%/day target) specify the model; no platform-side duty-cycle implementation exists yet (that work is inherently platform-shell code, per `MOBILE_INTEGRATION.md` §1's "the platform owns the radio and the clock" rule).

**(c) Packaging**
- UniFFI bindgen artifacts (Kotlin/Swift) are produced by the `mobile-bindings` CI job and uploaded together; a downstream Kotlin/Swift consumer compile and a successful remote Actions run remain unverified. `MOBILE_INTEGRATION.md` §3 documents the local reproduction.
- Android AAR / iOS XCFramework build mechanisms and CI jobs exist (`MOBILE_INTEGRATION.md` §3), but no successful remote Actions run or downstream consumer smoke compile is evidenced on the current unpushed branch.
- RN npm package (`REACT_NATIVE_INTEGRATION.md` specifies the TypeScript API and native module shape) is not published.
- `mmp-cli` radio wiring is implemented (`connect`, `listen`, and `send`), but
  no dedicated hardware-integration test runs in CI: only the in-memory
  `demo` path is deterministic, and the real two-host exchange remains
  unverified.

**(d) Hardening**
- `cargo-fuzz` on pre-auth parsers (beacon, handshake first message) is a mandate recorded in doc 07 §8 (attributed there to doc 12). Four fuzz targets now exist under `implementation/mesh-milk/fuzz/`; a deterministic seed recipe is checked, materialized, and byte-verified in CI from committed vectors. The generated seed files and libFuzzer-discovered corpus are intentionally not persisted; a measured CPU-hour run remains open. `mmp-types` also has proptest-based mutation testing (`props::*`), which is complementary rather than a replacement for the reproducible seed recipe or a hardening run.
- Doc 10 §6.4 test vectors, independent readers, and a coverage manifest are now committed under `implementation/mesh-milk/vectors/`; a real two-host radio interop run remains outstanding.
- Adversarial simulations per doc 14 §3 (Implementation drills, must-pass before v1.0): replay, flood, quota abuse, eclipse, Bloom-poison, and Byzantine double-grant scenarios are active in `mmp-sim`; the double-grant result is recorded as a receiver-local invariant in `docs/COPY_BUDGET_SECURITY_INVARIANT.md`, while trace-driven optimization measurements remain open.
- 1k-node scale validation against doc 13 targets exists in `mmp-sim`; the remaining gap is a calibrated clustered/SLAW-like mobility model for validating the same-district latency bands.
- External security review: not started (pre-requisite: the above hardening steps, per usual practice).

**(e) Embedded tier**
- `mmp-tal-embedded` + `mmp-node-embassy` (nRF52/ESP32) are 11- and 7-line stubs. Acceptance criteria are already written into the code comment: ≤256 KiB flash / ≤48 KiB RAM (doc 12 §7.6) and the OQ-7 GF(256) decode benchmark on nRF52-class MCUs at k≤64 (specification/research/open-questions.md, confidence Medium, "believed trivial, not yet benchmarked").

---

## 3. Phased plan to production

No calendar dates — this repo doesn't fake schedules (see CLAUDE.md methodology). Sizes are relative: S = days, M = one to a few weeks of focused work, L = a substantial multi-week effort, all in single-engineer-equivalent terms.

### Phase A — "First radio contact"
- ~~M5b: PC peripheral role~~ **IMPLEMENTED; Linux/BlueZ verification OPEN** — `mmp-tal-peripheral` contains the Linux (`bluer`/BlueZ) backend, while Windows/macOS remain explicit stubs with a documented WinRT design sketch. Hardware-independent lifecycle and ingress tests pass on the Windows target; the Linux target compile and real BlueZ exchange still require a Linux/pkg-config environment and physical adapters. **[L]**
- ~~`mmp-cli` radio wiring~~ **DONE** — `connect` (central), `listen` (peripheral), `send` (queue + wait-for-delivery) all wired against the real radio crates; verified error-clean without hardware, not yet verified against a real two-machine exchange. **[M]**
- ~~GATT UUID reconciliation~~ **DONE** — the development UUIDs now have one source of truth in `mmp-tal`, with the peripheral role re-exporting the central-facing values and documenting the role inversion; the SIG-assigned UUID remains a production blocker per OQ-12. **[M]**
- ~~UniFFI bindings (Kotlin + Swift) generated in CI~~ **CI-WIRED, RUN UNVERIFIED** — `mmp-ffi` ships a `uniffi-bindgen` binary gated behind a `cli` feature and the `mobile-bindings` job defines the artifact upload. A current remote Actions run and downstream Kotlin/Swift consumer smoke compile are still required. **[S]**

**Exit gate:** two PCs (or a PC + a phone dev-kit build) exchange a message and a receipt over real BLE — **still unverified**. The host-side roles, CLI commands, and matching development UUIDs are implemented; Windows-target tests and workspace checks pass, but the Linux peripheral build/runtime and real multi-host exchange remain environment- and hardware-gated. CI artifact production is wired but its successful remote run remains unverified. See `docs/MANUAL_INTEROP_TESTING.md` for the exact procedure and evidence checklist.

**A defect found and fixed along the way:** the CI `host` job's `ubuntu-latest` leg would have failed the moment `mmp-tal-peripheral` landed, because `bluer` links against `libdbus` and the stock runner image doesn't carry its dev headers. Fixed by installing `libdbus-1-dev pkg-config` before the build step on that leg.

### Phase B — "Mobile alpha"
- Android reference radio module (GATT server + client) per `MOBILE_INTEGRATION.md` §4 checklist. **[L]**
- iOS reference radio module per `MOBILE_INTEGRATION.md` §5 checklist, including the background-BLE caveats called out in `REACT_NATIVE_INTEGRATION.md` §"iOS background-BLE caveats". **[L]**
- RN package assembled from the TurboModule spec in `REACT_NATIVE_INTEGRATION.md` §2. **[M]**
- ~~Persistence completion: claims and group state~~ **DONE** — `SnapshotStore` persists claims (re-verified on load), group membership (OR-Set + epoch), `SeenIndex`, `ReceiptFilter`, validated authority admission state, and the explicit blocklist under the versioned `MMPSNAP2` record format. Both CLI and FFI restart paths restore and stage the same non-secret state; sender-key secrets/distributions and all ratchet state remain excluded.
- **Ratchet-persistence-vs-PCS decision: DONE, reverses this document's earlier assumption.** doc 08 §5.3 (new, 2026-07-16) concludes the "don't persist" default was wrong: a restarted party's `Conv` has no cheap X3DH fallback, and `open_async`'s deterministic tiebreak can silently wedge a conversation. Recommendation: persist ratchet state under a **platform-keystore-wrapped key** (mobile only), never persist `CK₀` (a latent hazard found in `Conv` — it currently retains `ck0`, flagged normatively), 72 h staleness discard on load, mandatory forced epoch-bump on first restored contact (`bump_epoch` exists in `mmp-crypto` but is not yet wired into `mmp-node` — that wiring plus the keystore integration itself is the concrete remaining work, tracked as **Phase B / Stage 1c**, naturally bundled with the native Android/iOS reference modules below since it needs real Keystore/Keychain APIs to implement, not just Rust). **[M]**
- Battery duty-cycle implementation on both platforms per doc 09 §7 and the <5%/day target in doc 13 §1. **[M]**

**Exit gate:** a 5-device field test delivers messages with receipts over a 24-hour window within the battery budget (<5%/day background, doc 13 §1). This is a measured field result, not a simulated one — real devices, real Doze/App Refresh behavior.

### Phase C — "Protocol completeness"
- Blob plane follow-up: class-4 symbol bundles and `BLOB_HAVE`/receive-side `BLOB_REQ` serving are shipped; make origin-targeted `BLOB_REQ` scheduling, per-domain airtime accounting, and streaming symbol production load-bearing instead of advisory. **[M]**
- ~~Friendship/proxy: implement the buffering agent~~ **DONE** (2026-07-17, `docs/AGILE_BACKLOG.md` E5-S2) — the architectural blocker this note flagged (does a session retain the peer's verified identity `Key32` after ID_PROOF?) went through the recommended Fable-class design pass first, same as E5-S5: resolved as "yes, via a host-side `NodeCore::known_identities` map, not the engine," with `SessionHost` gaining four friendship methods behind default no-op impls so nothing else (incl. `mmp-sim`) had to change. Buffer/pin-up-to-FRIEND_QUOTA/PROXY_SUMMARY/receipts-stay-end-to-end all implemented and tested (positive + negative 3-node scenarios, TTL expiry, FFI surface) — full writeup in `docs/IMPLEMENTATION_HANDBOOK.md` §4.5. **Still open, deferred per doc 06 §7:** the patron answering reconciliation on the dependent's behalf while it's still offline (this MVP requires the dependent to actually connect), signed proxy summaries with a validity horizon, ephem-id resolution, and a real `PROXY_SUMMARY` poll schedule. **[M → remaining pieces M, tracked as v-next]**
- ~~Authority announcements: AuthCert + class 5 + priority clamp~~ **CORE MECHANISM + NON-SECRET PERSISTENCE DONE** (2026-07-16/2026-08-08, `docs/AGILE_BACKLOG.md` E5-S5) — verification, sticky revocation-list handling (doc 18 §6, the AuthCert-scoped kind), the receiver-local priority clamp, the per-authority token bucket, validated `AuthorityStore` snapshots, and a dedicated validated FFI mint helper are implemented and tested end-to-end (`mmp-types::authcert`, `mmp-core::authority`, `mmp-node::authority_bucket`, FFI/CLI persistence). Still open: RBSR priority-class partitioning itself (the item below is a prerequisite this story deliberately didn't block on), D3 admission weighting for subscribed scopes, gossiped-root trust, and platform-keystore-backed ratchet persistence. The mint helper signs only with the local identity and never auto-pins or adopts the certificate. **[L → remaining pieces M]**
- General identity revocation/succession claims (field `0x09`, doc 18 §6 generalized) are implemented and tested end-to-end, including verified successor countersignatures and refusal to initiate/send to revoked peers. Native keystore persistence and user-mediated successor trust remain platform/product work. **[DONE; remaining pieces M]**
- ~~Fuzz targets~~ **DONE** (2026-07-16, `docs/AGILE_BACKLOG.md` E5-S6) — all four `cargo-fuzz` targets (`fuzz_beacon`/`fuzz_envelope`/`fuzz_bundle`/`fuzz_hs`) implemented, CI gained `fuzz-build` (every push) and `fuzz-run` (manual, 2 min/target smoke run). A deterministic seed recipe was added on 2026-08-09 and is checked, materialized, and verified in CI from the committed vectors; generated seed files and libFuzzer-discovered inputs remain intentionally untracked. Still open: running to a defined CPU-hour target with zero crashes (needs a measured baseline first, per this doc's own no-pre-commit discipline). **[M → remaining pieces S]**
- Adversarial sim suite: replay, flood, quota-abuse, eclipse, Bloom-poison, and Byzantine double-grant scenarios are active in `mmp-sim`, tracing to doc 14 §1's failure register and passing the doc 14 §3 implementation drills. **[L]**

**Exit gate:** interop test vectors published in the repo; fuzzers run N CPU-hours (N set once a baseline throughput is measured — do not pre-commit to a number not yet benchmarked) with zero crashes; doc 14 §3 drill scenarios pass in `mmp-sim`.

### Phase D — "Beta hardening"
- External crypto/security review of `mmp-crypto` and the session/handshake code, plus fixes arising from it. **[L]** — review **package assembled** (E6-S4, 2026-07-22): `docs/SECURITY_REVIEW_DOSSIER.md` scopes OQ-10's three non-standard compositions, a versioned crypto inventory, doc 07's residual risk register, honest fuzz/vector status. The review itself, and any fixes it produces, remain outstanding.
- ~~1k-node `mmp-sim` scale validation against the doc 13 performance model (§3 network load vs. scale, §5 latency).~~ **CORE MEASUREMENT DONE (E6-S1, 2026-07-22)** — see `docs/AGILE_BACKLOG.md` E6-S1 and `docs/IMPLEMENTATION_HANDBOOK.md` §4.12. §3 quota/store-growth claims measured and hold at N=1,000; §5's SLAW-band latency claim explains-its-deviation rather than closes — a clustered-mobility contact model remains a real gap in `mmp-sim`.
- ~~Versioning/cipher-agility exercise: run an actual HELLO negotiation between two builds pinned to different suite IDs, confirm doc 03 §8 (versioning and agility) behaves as specified.~~ **DONE (E6-S2, 2026-07-22)** — see `docs/AGILE_BACKLOG.md` E6-S2 and `docs/IMPLEMENTATION_HANDBOOK.md` §4.11. Suite `0x02` itself remains blocked on OQ-11.
- Performance/battery measurements on real devices (resolves OQ-13, the k_sys whole-phone energy multiplier, per specification/research/open-questions.md). **[M]**
- Docs freeze.

**Exit gate:** review findings closed or explicitly risk-accepted with sign-off; 1k-node sim meets or explains deviation from doc 13 targets; HELLO negotiation demonstrated between two suite versions; on-device battery numbers recorded for ≥3 device classes.

### Phase E — "Production / GA"
- Bluetooth SIG 16-bit UUID assignment (resolves OQ-12) or the validated 128-bit service-data fallback quantified in open-questions.md. **[M, but largely external-process-bound]**
- Release engineering: signed artifacts, MSRV policy, `cargo-deny`/`cargo-audit` wired into CI (the workspace already ships a `deny.toml`; verify it runs in CI, not just locally). **[S]**
- App-store background-mode compliance notes (iOS BGTask entitlement justification, Android foreground-service policy) written up for store submission. **[M]**
- ~~Operational diagnostics policy~~ **DONE (E8-S4, 2026-07-23):** confirmed and stated as a product commitment, not just an implementation fact — `DiagLog` (`implementation/mesh-milk/crates/mmp-node/src/diag.rs`, doc 12 §7.7) is a pure in-memory `Vec<JourneyRecord>` with no file or socket I/O; its only output, `dump()`, returns a `String` for local display/export. Per the crate-layering rule (`docs/IMPLEMENTATION_HANDBOOK.md` §0 rule 3), only the TAL crates touch a network socket at all, and they carry protocol wire bytes only — there is no code path from `DiagLog` to any transport. **Policy statement:** diagnostics are off by default; once enabled, journey-log data never leaves the device automatically — it surfaces only via `mmp-cli inspect` or the FFI's `diagDump()` for in-app UI or a bug report the user explicitly chooses to share. No telemetry of any kind is transmitted by mesh_milk. **[S]**
- v2 handoff: point to `V2_FEATURES.md` Part I (R5/R6/R7/R10/R13/R14/R16/R20) and doc 15's R-items for what comes after GA.

**Exit gate:** signed release artifacts produced by CI; SIG UUID assigned or fallback formally adopted; store compliance docs written; v2 handoff document exists and is reviewed.

### Proposal-derived research and conformance follow-up

The supplied MMP/Φ dialogue is recorded and audited in
`docs/PROPOSAL_AUDIT_MMP_PHI.md`. It does not change the MMP/1 wire contract.
The next work is ordered as: (1) resolve the encrypted `ID_PROOF` wording and
copy-budget security invariant; the implementation-side ID_PROOF erratum is
recorded in `docs/ID_PROOF_ERRATUM.md`; canonical AEAD associated data is now aligned
in the Rust class-1/2 callers and documented in
`docs/AEAD_AD_CONFORMANCE.md`; (2) finish transport-level admission/close
feedback and Linux/BlueZ evidence; the Rust close-action seam is now
implemented and hardware/mobile wiring remains open; (3) measure
EDR duty-cycle points, deadline scheduling, and evidence-weighted routing in
`mmp-sim` against the current binary-spray baseline; (4) prototype mobility
vouchers and fountain/DoF coding only behind research feature flags. Any claim
of a 2× improvement requires delivery, latency, airtime, radio energy, and
whole-phone energy measurements with confidence intervals.

---

## 4. Risk register

| # | Risk | Mitigation |
|---|---|---|
| 1 | **iOS background BLE restrictions.** Apple's background BLE model (state restoration, no reliable background scanning for undiscovered peripherals) is the single biggest product risk — it can silently cap real-world mesh density on iOS regardless of protocol quality. | Design the iOS reference module around documented state-restoration APIs from day one (Phase B); do not treat this as a later optimization. Budget for a materially worse iOS experience than Android and say so honestly in product docs. |
| 2 | **Android OEM scan throttling.** Many OEM battery managers throttle or kill background BLE scanning outside stock AOSP behavior, unpredictably across vendors. | Foreground-service pattern (already the plan per `MOBILE_INTEGRATION.md` §4.1) plus a device-compatibility matrix built from Phase B field-test data; treat OEM quirks as an ongoing maintenance cost, not a one-time fix. |
| 3 | **BLE goodput below the doc 13 model.** The model assumes MTU negotiation that `btleplug` currently cannot deliver on desktop, and mobile GATT servers may negotiate differently than assumed. | Measure real goodput per platform in Phase A/B before trusting doc 13 projections; feed measured numbers back into doc 13 as a documented revision, not silently. |
| 4 | **Ratchet-persistence vs. PCS tension.** Persisting ratchet state for restart-survival (a user-visible convenience) directly trades against post-compromise security guarantees the crypto design currently provides for free via drop-and-redownload. | Do not persist ratchet state without an explicit written design (Phase B item) reviewed against the doc 07 threat model; consider forward-secure state wipe on read as a middle ground. |
| 5 | **GATT interop between stacks.** `btleplug` (desktop), Android's native `BluetoothGatt`, and iOS's `CoreBluetooth` each have different quirks (write-without-response batching, notification coalescing, characteristic discovery timing) that can produce silent data corruption or stalls that unit tests can't catch. | Phase A's UUID reconciliation step is the first real interop test; keep an integration test matrix (`MOBILE_INTEGRATION.md` §9) growing as new platform pairs are added. |
| 6 | **UniFFI version churn.** UniFFI is a fast-moving dependency (workspace pins 0.29); a breaking UniFFI release could invalidate generated bindings or CI tooling. | Pin the version explicitly (already done via `Cargo.toml`), track UniFFI release notes before bumping, keep binding-generation in CI so breakage is caught immediately rather than at integration time. |
| 7 | **Single-implementation protocol ossification.** With only one Rust implementation, ambiguities in the spec silently resolve to "whatever this code does," which is invisible until a second implementation (or a hostile fuzzer) exposes it. | Doc 10 §6.4 test vectors and a published interop suite (Phase C) are the direct mitigation — treat vector publication as a gate, not a nice-to-have. |
| 8 | **Store-quota abuse in the wild.** The router's per-origin token buckets and WoT multiplier are validated in `mmp-router`'s unit tests against synthetic scenarios, but real adversarial behavior (Sybil floods, targeted quota exhaustion against a specific origin) hasn't been simulated. | Phase C's adversarial sim suite (flood/quota-abuse scenarios) is the direct mitigation; do not consider quota logic production-ready until those sims pass. |

---

## 5. Verification discipline

Carried forward from the implementation work to date, and binding on every phase above:

- Every phase gate is a runnable command or a measurable field result — never a subjective "looks done."
- No wall-clock asserts in tests or sims; all timing is driven by the caller-supplied `now_ms`, consistent with the sans-IO design (`MOBILE_INTEGRATION.md` §1–§2).
- Gates are protocol observables only (delivery ratio, receipt MAC verification, quota headroom) — not implementation-detail assertions that would break on a valid refactor.
- `layer-check` runs in CI and must stay green; it is what enforces the `no_std` layering that makes the embedded tier (Phase-independent groundwork) possible at all.
- `mmp-sim` is the protocol-truth instrument. Radio crates (`mmp-tal-btleplug`, the future mobile GATT modules) are byte shuttles only — they must never contain protocol logic that isn't equally expressible and testable in the simulator.
