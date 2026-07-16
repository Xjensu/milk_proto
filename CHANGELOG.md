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
