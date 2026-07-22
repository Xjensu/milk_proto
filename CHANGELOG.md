# Changelog

All notable changes to the mesh_milk (MMP/1) specification suite and its reference
implementation are recorded here, newest first. This file is normative: specification
docs 01–18 cite it as the audit-history record; `implementation/mesh-milk/vectors/README.md`
cites it for golden-vector requirements added after rev. A.

**Provenance note (reconstructed 2026-07-16, backlog item E0-S2):** this file did not
exist in the repository even though every spec document from 01 onward referenced it
normatively. It has been reconstructed from `other/AUDIT.md` (rev-A) and from the
self-description of docs 16–18 and their cross-references (rev-B), per the acceptance
criteria in `docs/AGILE_BACKLOG.md` §E0-S2. No entry below states anything the cited
source document does not itself say; where a detail could not be recovered, it is
omitted rather than invented.

---

## [Documentation] 2026-07-22 — E4-S4 stale backlog text corrected

### Fixed
- **`docs/AGILE_BACKLOG.md` §A3 item 4 and the E4-S4 story both still said "default: don't persist
  ratchet state"** — a decision `specification/08-crypto-spec.md` §5.3 reversed on 2026-07-16 (persist,
  keystore-wrapped). `docs/ROADMAP_TO_PRODUCTION.md` already carried the correction (its own addendum,
  same date); only the backlog text was stale and self-contradicting its own cited source.
- **E4-S4's real status, verified against code rather than assumed:** claims and group-state
  persistence are done (`mmp-store`'s `MMPSNAP2` format). The ratchet-persistence *decision* is done.
  Two sub-items are genuinely still open: SeenIndex/ReceiptFilter persistence (zero hits in
  `mmp-store` — pure Rust, agent-buildable now) and the ratchet persistence *wiring*
  (`mmp_crypto::ratchet::bump_epoch` exists but has zero call sites in `mmp-node` — the bump-epoch
  wiring itself is pure Rust and agent-buildable; the keystore-wrapped encryption needs native
  Android/iOS/Windows platform code, Epic 4's job, not the Rust workspace's).

---

## [Implementation] 2026-07-22 — E3-S3 GATT UUID reconciliation + Epic 3 marker sweep

### Added
- **Single source of truth for the dev GATT UUIDs (E3-S3, doc 09 §5.1):** `DEV_SERVICE_UUID`,
  `DEV_TX_CHARACTERISTIC_UUID`, `DEV_RX_CHARACTERISTIC_UUID` now live once, in `mmp-tal` (the
  shared base crate both TAL implementations already depended on). `mmp-tal-btleplug` re-exports
  them directly; `mmp-tal-peripheral` re-exports the service UUID and const-aliases its own
  perspective-renamed characteristic constants to the same canonical values. `MOBILE_INTEGRATION.md`
  §4.2 now quotes the actual UUID values against the named Rust source, not by name only.

### Fixed
- **Duplicated UUID literals, found while implementing E3-S3.** `mmp-tal-btleplug` and
  `mmp-tal-peripheral` each carried their own independent `pub const` copy of the same three UUID
  strings, cross-checked only by a unit test asserting the two literals matched — two sources that
  happen to agree is not one source. Removed the now-vacuous cross-check test and the
  `mmp-tal-btleplug` dev-dependency it existed for.
- **Stale backlog markers for E3-S1/E3-S2.** Both stories (`mmp-tal-peripheral` crate, `mmp-cli`
  radio wiring) were fully shipped — verified against code (chunk-arithmetic/LinkId-determinism
  tests, `connect`/`send`/`listen` subcommands) — but carried no `DONE` marker in
  `docs/AGILE_BACKLOG.md`, the same staleness class as the earlier E5-S3/E5-S7 gaps. Corrected.

---

## [Documentation] 2026-07-22 — E6-S4 external security review package

### Added
- **`docs/SECURITY_REVIEW_DOSSIER.md`:** scopes the review to OQ-10's three non-standard crypto
  compositions (channel binding doc 08 §3.2, serverless prekeys §4.2, opportunistic ratchet §5.2);
  a crypto inventory with exact `Cargo.lock`-resolved crate versions (`ed25519-dalek` 2.2.0,
  `x25519-dalek` 2.0.1, `chacha20poly1305` 0.10.1, `blake3` 1.8.5, `snow` 0.9.6); doc 07's full
  10-entry residual risk register; honest fuzz status (CI runs a 2-minute smoke test per target,
  not a hardening run; no seed corpus yet; a documented Windows toolchain gap, not a code defect);
  the 9 golden vectors and what they cover; and a "known gaps" section (clippy not yet CI-gated, no
  MSRV pin, `AuthorityStore` has no durable persistence, `mmp-sim` has no clustered-mobility model)
  so the external reviewer isn't spent rediscovering already-tracked items.

This is a compilation of already-shipped, already-verified material — no new analysis, no new
code. The review itself remains external, per the story's own scope.

---

## [Implementation] 2026-07-22 — E6-S3 deployment-profile drills (D-8/D-9)

### Added
- **`Sim::run_conductor_round` (E6-S3, doc 17 §3.2):** a scheduled, deterministic meeting sequence
  (conductor visits each stop in order) — the train profile's actual contact topology, which the
  existing uniform-random `run_random_meetings` cannot express. `Sim::is_delivered(id)`: per-message
  delivery detail beneath `delivery_ratio()`'s aggregate.
- **`mmp-sim::tests_deployment`** (`#[ignore]`d): `train_profile_a_misdeployment_drill` (D-8) —
  100% far-carriage telemetry arrival with a per-carriage anchor rail vs. 0% with only a single
  lead-carriage anchor, exceeding doc 17 §8's ≥50%-degradation band. `campus_profile_delivery_drill`
  (D-9) — delivery ratio 1.0 within 13.9 simulated minutes, clearing the 0.85-within-15-min band.

### Fixed (found mid-drill, not a pre-existing bug)
- **D-8's first version measured the wrong thing.** It tracked round-trip delivery receipts and got
  stuck at ratio 0.000 through 10 rounds; tracing individual contacts showed the conductor reliably
  received every carriage's telemetry within round 0, and the stall was 8 receipts minted back-to-back
  hitting the conductor's own per-origin admission bucket — a real, separate rate-limiting mechanism,
  not this drill's subject. Doc 17 §8's actual D-8 band is one-way arrival at the conductor
  (`Event::MessageReceived`), not a round trip; fixed by measuring that directly.

### Deferred, stated honestly (not faked)
- The misdeployed scenario's 60% contact-drop rate (brief walking-past pass vs. persistent
  same-carriage co-presence) is a stated modeling assumption, not a measured figure.
- Neither drill wires up real AuthCert/class-5 announcements end-to-end — telemetry/announcement
  propagation is modeled with ordinary class-1 direct bundles, since the conductor-round latency these
  drills measure is a session-mechanism property common to every bundle class.

---

## [Specification] 2026-07-22 — E1-S6 registry sweeps

### Added
- **doc 10** §4.6: pointer-only row for class-5 announcements (owned by doc 18 §3). §9: docs 16–18
  added to the cross-references list.
- **doc 13** new §3.1: three ⚠-tagged workload rows — album-thumbnail eager-fetch (doc 16 §5/§10),
  full-file pull (doc 16 §1/§10), AuthCert/announcement traffic (doc 18 §2/§3) — quantifying the
  "rounding error against the message-plane baseline" claims those docs already made in prose,
  rather than asserting them for the first time here.
- **doc 14** §1: F23/F24, pointer rows for doc 18 §7's A6 (stolen operator root key) and A7
  (declared-priority queue-jump), mirroring the existing F21/F22 pointer-row pattern. §3: D-8/D-9
  pointer rows for doc 17 §8's train/campus deployment drills (doc 17 §12 already expected these to
  exist here).

### Fixed
- **Terminology drift:** `specification/research/open-questions.md` OQ-17's Round-2-bis annotation
  said "issuer-signed kill-cert" — retired in favor of the field's actual name, `REVOCATION_LIST`
  (claim field `0x08`, doc 18 §6), matching the shipped code and doc 18's own terminology.

This unblocks **E6-S3** (deployment-profile presets, doc 17), the only backlog item that depended
on this story.

---

## [Implementation] 2026-07-22 — E6-S1 1k-node scale validation

### Added
- **`mmp-sim` latency and store-growth instrumentation (E6-S1, doc 13 §3/§5):** `Sim` gained
  `delivery_latencies_ms()`/`latency_percentile_ms(p)` (per-tracked-message send→delivery samples,
  nearest-rank percentile) and `store_high_water_bytes()` (running max, not an end-of-run snapshot).
  New `mmp-sim::tests_scale::thousand_node_mesh_scale_validation` (`#[ignore]`, ~19 min under
  `--release`): 1,000 nodes, 20 disjoint pairs, 500k uniform-random meetings.

### Finding (recorded, not silently worked around)
- **Uniform-random contact mixing measurably dilutes with N.** N=100/30 CI scenarios reach delivery
  ratio 1.0; the first N=1,000 attempt at a realistic 5–20 s contact cadence couldn't mix enough
  pairs within `LIFETIME_MESSAGE_MS` (72 h) at all; compressing the gap to ~250 ms fit the TTL but
  only reached ratio 0.45, and at that density the simulated clock no longer represents real
  elapsed time — its p50 (≈434 min) / p90 (≈683 min) are not comparable to doc 13 §5's real-minutes
  SLAW band. Quota/store-growth cost claims (doc 13 §3) DO hold, measured: 208 KiB high-water across
  1,000 nodes. Recorded in `specification/13-performance-model.md` §5/§6 and
  `specification/research/open-questions.md` OQ-4 as a partial answer — the cost/quota side closes,
  the SLAW-specific latency question stays open pending a clustered-mobility contact model.

### Deferred, stated honestly (not faked)
- A SLAW or other clustered-mobility contact model for `mmp-sim` — a simulation-model project of its
  own; without it, doc 13 §5's same-district latency band cannot be validly tested at N=1k at all.
- The N=10,000 tier of doc 13 §3's network-load table.

---

## [Implementation] 2026-07-22 — E6-S2 cipher-agility exercise

### Added
- **HELLO cipher-suite negotiation is now enforced (E6-S2, doc 03 §8):** `mmp-crypto::noise`
  gained `SUPPORTED_SUITES = &[0x01]`; `NoiseHandshake::advance()` now checks, the moment either
  side decrypts the peer's `Hello` payload, that the peer's advertised `suites` list intersects
  the local build's supported set, refusing (existing `CryptoError::Noise`, no new variant) if
  not. `mmp-core`'s session engine already closed cleanly on any handshake error, so no change was
  needed above the crypto layer. Full write-up in `docs/IMPLEMENTATION_HANDBOOK.md` §4.11.

### Fixed
- **Silent cipher-suite mismatch acceptance, pre-existing since Phase A.** `Hello.suites` has
  carried an advertised suite list on the wire from the start, but nothing ever read it —
  `NoiseHandshake::initiator`/`responder` accepted a `Hello` only to embed it in the encrypted
  payload, never to check it. Two nodes advertising disjoint suite sets would have completed the
  handshake anyway, contradicting doc 03 §8's "cipher suites: 1-byte registry pinned in the
  handshake." No wire format change — the field already existed; only the read side was missing.

### Deferred, stated honestly (not faked)
- Suite `0x02` (hybrid X25519+ML-KEM) itself — blocked on OQ-11 (no standardized hybrid-Noise
  construction to implement against yet).
- A real multi-suite *selection* policy — moot with exactly one real suite; "prefer highest
  mutually-supported ID" is the obvious default whenever `0x02` lands, but unverified today.
- Surfacing the specific refusal reason to the host/FFI layer — every handshake failure is silent
  to `SessionHost` today by design; this one is not treated differently.

---

## [Implementation] 2026-07-18 — E5-S1 blob plane / file transfer

### Added
- **File transfer over the blob plane (E5-S1, handbook §4.4, doc 16):** `Node::send_file`/
  `fetch_file`/`cancel_fetch`/`file_progress`, `Event::FileOffered`/`FileReady`/`FileFailed`,
  mirrored at the FFI boundary (`docs/API_REFERENCE.md` §5.12). Class-4 symbol bundles carry
  `u16 path_len ‖ MerklePath wire bytes ‖ symbol bytes` (`mmp-types::payload::SymbolPayload`);
  positioning stays exclusively in the existing `Ext::SymbolInfo` TLV — zero new wire allocations,
  per doc 16's own promise. `mmp-crypto::merkle` gained the `MerklePath` wire codec
  (`encode`/`decode`/`leaf_index`) and `compute_root`; a symbol only counts toward reassembly if
  its Merkle path both proves membership under the manifest root AND proves the claimed position
  (`leaf_index() == sym_no`) — closing a forged-position replay hole a membership-only check would
  leave open. `mmp-node::blob` implements the doc 11 §6 per-root FSM (`Wanted → Fetching →
  Assembled`, or `Abandoned`); symbols are never buffered beyond one stripe (≤ 64 KiB) — they live
  in the ordinary bundle store, indexed by a RAM-only `root → sym_no → BundleID` map pruned on
  expiry/eviction. Three new `SessionHost` seam methods (`on_blob_have`/`on_blob_req`/
  `blob_frames`), default no-op exactly like the shipped friendship seam, turn the previously
  advisory-only BLOB_REQ/BLOB_HAVE frames into real dispatch. Dedup by content address: resending
  identical bytes mints zero new class-4 bundles. Full design record (decisions D1–D8) and the
  residual list (airtime sub-budget, group sharing, thumbnail auto-fetch, streaming sender,
  origin-targeted BLOB_REQ) are in `docs/IMPLEMENTATION_HANDBOOK.md` §4.4's SHIPPED note.

### Fixed
- **`mmp-core/src/rbsr.rs` `on_ids`, pre-existing bug found while building this story's acceptance
  test.** The terminal-list reply to a peer's small-range echo unconditionally listed the full
  local ID set for that range with no size check, correct at every previously-tested scale but
  capable of producing a real envelope over `MAX_ENVELOPE_PAYLOAD` for a domain with hundreds of
  items — exactly what a file's class-4 symbol set is. The oversize check only fires as a
  `debug_assert!` at encode time, so a release build emitted the bad envelope; the receiver's
  `read_envelope` then rejected it outright, silently killing the session
  (`WireError::Oversize` → `protocol_error`) right as the largest domain's reconciliation was about
  to pay off. Fixed by giving `on_ids` the same size discipline `on_range` already has: split via
  the existing `split_ranges`/`RBSR_B_F` machinery instead of listing once the local set exceeds
  `RBSR_T_LIST`.

### Deferred, stated honestly (not faked)
- Airtime sub-budget (doc 11 §8 D3 / doc 16 §8, both ≤40%) — no per-domain accounting exists
  anywhere in the session engine; substituted with domain ordering + BULK priority + a fixed
  per-contact serve cap.
- Group file sharing — `deliver_group` discards the apptype byte needed to route a manifest inside
  a group message.
- Thumbnail auto-fetch (doc 16 §5.1's role-gated eager tier) — nothing auto-downloads yet.
- Streaming sender — `send_file` takes the whole buffer in host RAM (≤ 8 MiB); disk-streaming
  encode is follow-up work.
- Origin-targeted BLOB_REQ emission and BLOB_HAVE gossip beyond active assemblies — receive-side is
  wired for interoperability; MVP requesters only ever emit BLOB_HAVE.

### Errata candidate
- Doc 10 §4.5's class-4 payload-layout prose ("RS symbol bytes," positioning elsewhere) is
  superseded by this story's D1 placement decision (the Merkle path lives in the payload itself,
  ahead of the symbol bytes) — a path TLV would have required a new wire allocation doc 16
  explicitly promises against. Doc 11 §6's `REJECTED` state is unreachable in this implementation
  (a manifest failing `verify_strict` never creates an assembly at all) and its "blacklist sender"
  step is diagnostic-only (`bad_symbols` counter, no policy yet) — both are residuals, not gaps.
  Doc 16 §8 vs. doc 05 §8's bucket-throughput arithmetic for relayed files was never reconciled
  against each other; this story's acceptance test surfaced the gap concretely rather than papering
  over it (see `mmp-sim/src/tests_blob.rs`'s own interpretation-call documentation for the exact
  router interaction: a bundle's spray budget is spent on every hand-off *attempt*, not on confirmed
  receipt, so any nonzero loss on a non-destination relay hop strands most of a multi-hundred-symbol
  transfer within two attempts regardless of how many further contacts run — a real, pre-existing
  `mmp-router` characteristic, distinct from the `rbsr.rs` bug above).

---

## [Implementation] 2026-07-17 — E5-S4 golden test vectors

### Added
- **Golden test vectors (E5-S4, handbook §4.7, doc 10 §6.4):** a `mmp-cli vectors` generator
  (`crates/mmp-cli/src/vectors.rs`) emitting fixed-seed, RNG-free, wall-clock-free JSON vectors under
  `implementation/mesh-milk/vectors/` — bundle (canonical bytes + BundleID + `mmp1/bundle` signature),
  beacon codec, nickname claim, receipt MAC, envelope framing, RBSR fingerprint, plus negatives:
  cross-signing-domain rejection (AUDIT F4, incl. the rev-B `mmp1/manifest`/`mmp1/authcert`/`mmp1/announce`
  tags), lifetime-reset→identical-BundleID (F1), and budget>L_MAX clamp (F3). The consuming test
  (`vectors::tests`) drift-guards the committed files against current codec output AND independently
  re-verifies each invariant. Every expected byte comes from the real `mmp-types`/`mmp-crypto` codecs,
  never hand-authored. No private key/ratchet/MAC-key material appears in any vector.

### Deferred, stated honestly (not faked)
- The X3DH-agreement vector (needs deterministic-RNG plumbing) and the overlarge-S/non-canonical
  Ed25519 negative (needs a crafted malleable signature) are follow-ups; both properties are already
  covered by `mmp-crypto` unit tests (`prekey_agreement_matches_both_sides`,
  `sign_verify_and_cross_domain_rejects`). See `vectors/README.md`.

### Errata candidate
- Some spec/doc text refers to `specification/test-vectors/*.json`; the vectors have a single home at
  `implementation/mesh-milk/vectors/` (where `vectors/README.md` and the generator/test resolve them).
  The `specification/` path mention should be reconciled to the one location, not duplicated.

---

## [Implementation] 2026-07-17 — E5-S3 identity revocation, E5-S2 friendship buffering, E5-S7 adversarial sims

### Added
- **Identity revocation & succession (E5-S3, handbook §4.6, doc 06 §4.1):** new claim
  field `0x09 revocation` (`revoked_key ‖ reason ‖ optional successor ‖ successor_countersig`).
  `mmp-types::claim::Revocation` codec; `mmp-core::ClaimStore` tombstone-merge semantics — a
  stored revocation is immutable and beats any other claim by that owner regardless of seq;
  `revocation()`/`successor()` queries (the latter verifies the succession counter-signature under
  `mmp1/claim` with the successor's own key, a failed countersig leaves the revocation standing);
  `mmp-node` refuses sessions from and use of stored prekeys of a revoked key, emits
  `Event::PeerRevoked{peer, successor}`; FFI `revoke_identity` + `FfiEvent::PeerRevoked`. Petname
  migration stays manual (the event is the only hook). Field `0x09` chosen to avoid the `0x07`
  AUTHCERT / `0x08` REVOCATION_LIST collision (distinct mechanism: identity vs. authority-grant).
- **Friendship / proxy buffering (E5-S2, handbook §4.5, doc 04 §4):** counter-signed friendship
  claim (field `0x05`, body `friend ‖ since ‖ countersig` under `mmp1/friend`); `FRIEND_OFFER`/
  `FRIEND_ACK`/`PROXY_SUMMARY` frame bodies made load-bearing (four new `SessionHost` seam methods,
  all default no-op so existing hosts are unaffected); a patron pins bundles destined to a
  dependent up to `FRIEND_QUOTA` (16 MiB), exempt from origin-bucket charging and from eviction,
  unpinned+GC'd on the dependent's end-to-end receipt; `FRIENDSHIP_TTL_MS` (24 h) expiry;
  `MAX_PATRONS` (3). FFI `request_friendship`/`allow_friendship_from` + `FfiEvent::FriendshipEstablished`.
- **Adversarial simulation suite (E5-S7, handbook §4.8, doc 14 §1):** `mmp-sim::adversary::Adversary`
  raw-frame node wrapper and five scenarios — `replay_storm_delivers_exactly_once` (F21),
  `flood_origin_is_throttled_but_others_deliver` (F6), `quota_abuse_degrades_gracefully` (F4),
  `eclipse_gains_nothing` (F22), `bloom_poison_never_deletes` (F5) — each asserting a protocol
  observable, not an internal counter. `mmp-node::set_store_quota` added as a test/sim-only seam so
  quota-pressure scenarios run at CI scale instead of requiring a ~2000-identity Sybil swarm.

### Fixed
- The quota-abuse scenario originally shipped `#[ignore]`d because a tier-sized (128 MiB) quota was
  unreachable cheaply, and it asserted "eviction happened" without asserting its own precondition
  ("store exceeded quota") — masking that far fewer bundles were admitted than its burst-math
  assumed. Reworked around `set_store_quota` with an adaptive Sybil loop and an explicit
  precondition assertion; now runs in CI, un-ignored.

### Known limitation, stated honestly
- `MAX_PATRONS = 3` permits multiple concurrent patrons, but `ClaimStore` is a one-slot-per-
  `(owner, field)` LWW register, so only the last-published `0x05` friendship claim is wire-visible
  (the proxy-hint routing input). Buffering itself works with all patrons; only the gossiped claim
  is single-valued. Recorded in `docs/IMPLEMENTATION_HANDBOOK.md` §4.5 — resolving it needs a
  wire-format change, out of E5-S2's scope.

---

## [Implementation] 2026-07-16 — E1-S3 threat-catalog consolidation, E5-S6 fuzz targets

### Added
- **doc 07 §3/§7 (E1-S3):** ten new pointer/residual entries consolidating doc 18 §7's A1/A4/A5/A6/A7
  and doc 16 §9.1's garbage-manifest exhaustion into the main attack→defense matrix and residual
  risk register; §9 cross-references extended to docs 16/18. Item 8 of the residual register
  (stolen operator root key) is flagged as the register's one **unbounded** entry, unlike every
  other bounded item — stated plainly rather than smoothed over.
- **`implementation/mesh-milk/fuzz/` (E5-S6):** four `cargo-fuzz` targets (`fuzz_beacon`,
  `fuzz_envelope`, `fuzz_bundle`, `fuzz_hs`) per `docs/IMPLEMENTATION_HANDBOOK.md` §4.7, in its own
  `[workspace]` so the main build is unaffected (verified). CI gained `fuzz-build` (every push,
  build-only) and `fuzz-run` (manual `workflow_dispatch`, 2-minute smoke run per target).

### Known limitation, stated honestly
Full sanitizer-linked `cargo fuzz build` was verified to compile cleanly under plain `rustc` on the
Windows implementation box, but the final ASan-instrumented link step fails there
(`clang_rt.asan_dynamic_runtime_thunk` missing; `-s none` still fails on unresolved `__sancov_pcs`
symbols) — a Windows toolchain gap libFuzzer is known to have, not a defect in the four target
files. The CI `fuzz-build` job (Linux) is the actual verification and has not yet been observed
passing, since the branch is uncommitted (see E0-S1). No seed corpus is checked in yet (deferred,
`fuzz/README.md`).

---

## [Implementation] 2026-07-16 — E5-S5 authority announcements shipped (core mechanism)

### Added
- `mmp-types/src/authcert.rs`: `AuthCert`, `AnnouncementBody`, `RevocationList`/`RevEntry`
  wire codecs (doc 18 §2/§3/§6) and `scope_id_of` (doc 18 §4).
- `mmp-core/src/authority.rs` (`AuthorityStore`): AuthCert verification against a pinned
  root, sticky revocation union, and the `effective_class5_priority` clamp combinator
  (doc 18 §5.2/§6).
- `mmp-node/src/authority_bucket.rs` (`AuthorityBuckets`): per-authority token bucket
  (doc 18 §5.3), RAM-only by design (see `docs/IMPLEMENTATION_HANDBOOK.md` §4.10 for the
  persistence-decision rationale — a different call from the ratchet decision below,
  not an inconsistency).
- `mmp-node`/`mmp-ffi` public surface: `set_wall_clock`, `pin_authority_root`,
  `subscribe_scope`, `publish_announcement`, `Event::Announcement` /
  `FfiEvent::Announcement` (see `docs/API_REFERENCE.md` §5.9).
- `BundleMeta.effective_priority` (`mmp-core/src/store.rs`), split from the existing
  `priority` field so the doc 18 §5.2.2 requirement — RBSR partitioning (when it's
  eventually built) keys on the declared byte, never the receiver-local clamp — holds
  by construction rather than by discipline.

### Design note
This story's design (the ingest-pipeline integration point, the `BundleMeta` field
split, the token-bucket persistence call) was delegated to a Fable-class agent per
`docs/AGILE_BACKLOG.md`'s own assignment table (security-sensitive/cross-cutting design).
Full writeup in `docs/IMPLEMENTATION_HANDBOOK.md` §4.10, including what was explicitly
deferred (RBSR partitioning itself, D3 admission weighting, gossiped-root trust,
`AuthorityStore` durable persistence).

### Errata candidate (doc 18, not yet applied)
- **§2 and §3, wire-vs-preimage `suite` byte.** Doc 18 §2's `AuthCert` preimage and
  §3's `AnnouncementBody` preimage both include a leading `suite:u8` (stated
  explicitly for §2's cipher-agility rationale), but §3's payload-schema *struct
  listing* omits `suite` as a carried field — only §2's does list it. A verifier
  cannot reconstruct a preimage it doesn't know how to read back off the wire, so
  the implementation carries `suite` as a wire byte for both objects, consistently.
  Recorded here rather than edited into doc 18 directly — an orchestrator/spec-owner
  should confirm and fold this into doc 18 §3's struct listing in a future revision.

---

## [Amendment] 2026-07-16 — doc 08 §5.3 ratchet-persistence decision

### Changed
- **doc 08 §5.3 (new subsection).** Reverses the implementation handbook's earlier
  tentative "don't persist ratchet state for v1" default. New normative rule: on
  phones/PC, conversation ratchet state **SHALL be persisted** across process
  restart, and **only** under a platform-keystore-wrapped encryption key; ratchet
  state MUST NOT be written to storage outside that wrapping. Core/`no_std` targets
  do not persist ratchet state (unchanged for that tier).
  - Rationale (doc 08 §5.3): a restarted party's `Conv` has no cheap X3DH fallback,
    and `open_async`'s deterministic tiebreak can silently wedge a conversation
    without persisted state.
  - A latent hazard was flagged in the same pass: `Conv` currently retains `CK₀`
    across restart, which must never be persisted; flagged normatively in doc 08 §5.3.
  - Mandatory forced epoch-bump on first restored contact after a reload.
  - Tracked in `docs/ROADMAP_TO_PRODUCTION.md` Phase B as "Phase B / Stage 1c" —
    `bump_epoch` exists in `mmp-crypto` but is not yet wired into `mmp-node`; the
    wiring plus platform Keystore/Keychain integration is the remaining work.
  - PCS reasoning updated: doc 08 §5.2's "PCS between meetings" and §6.1's
    "PCS within a group epoch" window now explicitly extends across a restart when
    state is restored — healed, as always, at the next live session (doc 08 §11).

---

## [rev-B] 2026-07-03 — extension phase: docs 16–18

Drafted 2026-07-02, marked **FINAL** 2026-07-03. Adds three extension documents on
top of the frozen v1 wire format (docs 01–15, unmodified): **16** (file transfer),
**17** (deployment profiles), **18** (authority, announcements, priority admission).
Reviewed by an external multi-model council across Round 1, Round 2, and Round 2-bis
— disposition IDs are recorded in `COUNCIL-REVIEW.md`; this entry records only the
resulting wire/spec changes.

### Added
- **doc 16 — File transfer application layer.** Signed `FileManifest` application
  object (`mmp1/manifest` signing domain), carried inside existing class 1/2 message
  bodies via a one-byte `apptype` tag (0x01) — zero new bundle classes, zero new
  critical TLVs. Two-tier blob binding (full file `file_root`, optional thumbnail
  `thumb_root`) reusing the existing BLOB_REQ/BLOB_HAVE plane and the doc 11 §6
  per-root FSM unchanged. Constants: `FILE_MAX_SIZE = 8 MiB`, `ALBUM_MAX_FILES = 16`,
  `THUMB_MAX = 32 KiB ⚠` (doc 16 §10).
- **doc 17 — Deployment profiles.** Named, versioned tuning vectors over doc 03 §9's
  ⚠ constants: Train Profile A (infrastructure-assisted, anchor rail + operator
  authority), Train Profile B (pure ad-hoc), and a campus/building profile. Zero
  wire-format change by construction (doc 17 §1.2). New drills D-8 (Train Profile A
  end-to-end, incl. a misdeployment check) and D-9 (campus/building) feeding
  `mmp-sim` scenarios S-8/S-9.
- **doc 18 — Authority, announcements, and priority admission.** New bundle
  `class = 5` (announcement); new claim field `0x07 AuthCert` (corrects an earlier
  working assumption that `0x06` was free — doc 06 §4.1 already allocates `0x06` to
  `group_epoch_anchor`, doc 18 §2.1); new claim field `0x08 REVOCATION_LIST`
  (issuer-owned whole-state value, doc 18 §6); two new signing-domain tags,
  `mmp1/authcert` and `mmp1/announce`; two new optional-range TLVs, `0x81
  AUTHCERT_ATTACH` and `0x82 SCOPE_INFO`. A formally bounded per-receiver token
  bucket (doc 18 §5.3–§5.4) limits elevated-priority admission per AuthCert to
  `B + β·T ≤ β·(T+1)` distinct bundles (the §5.4(c) lemma). No new frame type, no
  new RBSR domain, no new HELLO capability bit (doc 18 §3–§4).
- **doc 15 §6 — extension-phase research items** R16–R21 and open questions
  OQ-14–OQ-18, arising from the docs 16–18 work and the council review.

### Golden-vector requirement (new, per `implementation/mesh-milk/vectors/README.md`)
- The cross-signing-domain negative-vector set (AUDIT F4, rev-A) is extended with
  the three new rev-B tags: a signature minted under `mmp1/manifest`,
  `mmp1/authcert`, or `mmp1/announce` and presented under another tag MUST be
  rejected. Stated twice normatively: doc 10 §3.1 (tag table) and doc 12 §6 rule 6
  (per `docs/AGILE_BACKLOG.md` E1-S4).

### Known gaps left open by this revision (not fixed here; tracked as backlog stories)
- Docs 03/06/07/10/11/12/13/14 (the "registry" docs) were not back-patched against
  docs 16–18 at rev-B time — see `docs/AGILE_BACKLOG.md` Epic 1 (E1-S1…S6) for the
  itemized reconciliation.
- **Errata-candidate (not yet applied):** doc 03 §5.2 phase 3's RBSR partition
  order and doc 05 §6's TRANSFER sort both key on the wire-visible *declared*
  `priority` byte with no admission concept — any origin can self-declare priority
  0/1 on junk and receive earlier-in-session service with no AuthCert anywhere in
  the path (doc 18 §5.2.2, abuse row A7, disposition CR2bis-11). The clamp cannot
  close this because it is receiver-local and per-receiver-divergent by construction
  (CR2bis-3). The structural fix — an admission-aware sort key in doc 05 §6 — is
  filed here as an errata-candidate for a future maintenance revision of the frozen
  v1 suite; it is bounded (priority buys order, never volume) and not applied as
  part of rev-B.

---

## [rev-A] 2026-06-11 — post-audit hardening

Formal adversarial audit of the complete v1 specification suite (docs 01–15),
conducted 2026-06-11 as a meta-document (`other/AUDIT.md`); no spec document was
modified by the audit itself. The patches below are the audit's proposed changes as
subsequently applied to docs 03–14 (each of which now carries the banner "revised
2026-06-11 post-audit (`AUDIT.md`, `CHANGELOG.md`)"). Severity and finding IDs are
`other/AUDIT.md`'s own (F1–F14); two (F6, F12) were resolved as "verdict: keep, no
change" and have no entry here.

### Fixed
- **F1 (MED–HIGH) — Lifetime-resurrection replay.** doc 03 §4.3/§11, doc 07 #17.
  A captor could reset a bundle's `lifetime_ms` to class-max and re-inject it past
  the dedup horizon while the signature still verified (replay-of-genuine-traffic).
  Fix: relays persist BundleID in SeenIndex for class-max duration **from first
  sight**, regardless of any later lifetime reset (implemented: `mmp-core`
  `seen_index_defeats_lifetime_resurrection` test). Doc 07 finding #17 rewritten to
  state the real residual (bounded by SeenIndex-from-first-sight + app-layer
  ratchet/seq index, not by lifetime alone). Cost: no new bytes required beyond the
  existing header (an origin-monotonic `created_seq` was considered and folded into
  the SeenIndex-from-first-sight approach instead).
- **F2 (HIGH) — Verify-before-forward vs. unmodeled claim propagation.** doc 03
  §3.2/§4.1, doc 10 §3. A relay carrying a 16-byte NodeID `origin` cannot verify a
  bundle's signature without the origin's 32-byte Ed25519 public key, which the
  suite never guaranteed was locally available. Fix: the origin's public key rides
  **inline** (32 B) in every relayed bundle rather than the derived NodeID alone,
  making verify-before-forward unconditional (implemented: `mmp-types` bundle
  layout; NodeID remains derivable as `BLAKE3(pubkey)[0..16]`). Cost: +16 B/bundle
  (~5% on a 340 B message).
- **F3 (HIGH reasoning gap) — Copy-budget inflation via honest relays.** doc 03
  §4.5, doc 05 §2/§11. The claim that inflated spray budgets only cost "the
  attacker's own airtime" is false for *relayed* bundles: a malicious relay can
  grant budget=32 to many honest downstream peers, spending their airtime, not its
  own. Fix: granted spray budget is **clamped to `L_MAX` on receive**, mirroring the
  existing lifetime clamp (implemented: `mmp-router`, golden-vector negative
  required per doc 12 §7.1(d)); the §4.5/§11 security argument rewritten to cite
  per-origin token buckets (doc 05 §8) as the actual bound.
- **F4 (MED, breaking) — No global signing-domain separation.** doc 06 §4.1, doc 08
  §3.2, doc 10 §3. Only `ID_PROOF` was domain-tagged; bundles/claims were signed
  over raw bytes, an inconsistency and a cross-protocol-reuse risk. Fix: one global
  scheme — every Ed25519 signature is over
  `BLAKE3("mmp1 " ‖ context_tag ‖ 0x00 ‖ body)` with distinct tags (`bundle`,
  `claim`, `chanbind`, `friendship`, `prekey`, `blobroot`, later extended by rev-B's
  `manifest`, `authcert`, `announce`). Implemented in `mmp-types`/`domain.rs`.
  Sequenced before the first golden vector, per doc 12 §10's build order.
- **F5 (LOW–MED) — OR-Set unbounded growth.** doc 06 §8. No tag/tombstone GC was
  specified for group membership OR-Sets. Fix: tags/tombstones older than the
  latest mutually-acknowledged epoch anchor may be compacted, keyed to that
  anchor's membership BLAKE3 root as the compaction checkpoint (implemented:
  `mmp-core` `orset_converges_and_compacts` test).
- **F7 (MED) — Burst-scan energy/OS-fragility.** doc 04 §5, doc 09 §3.2, doc 13
  §1.2. Burst-scan-on-contact (30 s) is Android-foreground-service-only in practice
  and cannot sustain on iOS background. Fix: reclassified as Android-FG-service-only
  policy; iOS background uses opportunistic-on-app-open only; the k_sys planning
  band raised to [4,12] pending OQ-13.
- **F8a (MED, privacy) — Beacon-summary cross-rotation linkage.** doc 09 §3.3, doc
  03 §5.1. The quasi-stable 14-byte rotating beacon summary let a continuous
  co-present scanner link ephem_ids across the 15-minute rotation via slowly
  changing Bloom fragments. Fix: re-salt the summary fragment per ephem_id epoch.
- **F8b (MED, hygiene) — Beacon-summary insufficiency at scale.** doc 04 §6, doc 05
  §7.2, doc 13. Documented explicitly: beacon-summary-driven offer-suppression is a
  ≤~100-node optimization; at 1k+ the 96-bit fragment is FP-saturated and contact
  selection falls back to novelty + RSSI + tier. No mechanism change; a
  documentation-only fix against over-trusting beacon dedup.
- **F9 (LOW) — Ed25519 signature malleability.** doc 08 §10, doc 12 §7. Fix:
  mandate canonical-S verification (`verify_strict`, reject S ≥ L per RFC 8032
  §5.1.7) plus a golden vector asserting a non-canonical-S variant is rejected
  (implemented: `mmp-crypto`, all verifiers use `verify_strict` only).
- **F10 (LOW) — Unbounded reassembly buffer.** doc 09 §5.1, doc 10 §7. A malicious
  peer could stream a length prefix of 4096 and dribble bytes to pin a session
  buffer. Fix: reassembly buffer is pre-allocated at `MAX_BUNDLE` (4096), a
  reassembly timeout is specified, and an over-long length prefix is
  ERROR+CLOSE (no growth).
- **F11 (MED, unsafe-direction) — Bloom filter as purge authority.** doc 06 §6.3.
  A Bloom ReceiptFilter false positive purged a **not-yet-delivered** copy — unsafe
  in the wrong direction. Fix: purge only on exact D0 receipt match; the Bloom
  ReceiptFilter becomes a suppression **hint** everywhere, never purge authority
  (implemented: `mmp-core` `receipt_filter_hint_never_purges` test, the Hint/Purge
  type asymmetry).
- **F13 (MED, deployment) — T2 relay availability assumed, not guaranteed.** doc 04
  §5, doc 14. Added an explicit precondition: T2 relay availability is
  OEM-battery-policy-dependent and may silently degrade to T1; extended drill D-7
  for OEM background-kill matrices.
- **F14 (LOW, under-specified) — No diagnostics/trace schema.** doc 12 (new §7.1
  item 7). Added a per-bundle journey log (received/stored/offered/handed-off/
  expired with peer + timestamp) and a contact log, gated behind a `diag` build
  flag, off by default, never on the Core tier (implemented: `mmp-node/diag.rs`).

### Not changed (audited, verdict: keep)
- F6 — sender-keys vs. MLS trade-off: verified technically correct against RFC
  9420; residual (removal latency = partition-healing time) surfaced in UX copy
  rather than the protocol.
- F12 — per-data-type CRDT primitive selection: verified correct as designed,
  except for F11 (Bloom-as-purge, above) and F5 (OR-Set GC, above).

### Carried forward, not fixed by this revision (deployment risks, not protocol
defects; tracked in `specification/research/open-questions.md`)
- A1/A2/A5 — contact-rate/mobility assumptions, Android foreground-service
  survival, iOS overflow-area scan persistence: field-measurement questions, not
  spec defects.
- F6b — prekey suppression (an active relay withholding fresh prekey claims):
  folded into OQ-10's external-crypto-review scope.
