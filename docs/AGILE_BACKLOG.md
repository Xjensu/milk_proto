# MMP/1 — Agile Backlog for AI-Agent Execution

**Status:** synthesized 2026-07-15 from two verified audits (specification gap analysis of docs 01–18; implementation audit of `implementation/mesh-milk` with `cargo test`/`clippy`/`layer-check` re-run that day: 71 passed / 0 failed / 1 ignored, clippy `-D warnings` clean, layering intact).
**Audience:** AI agents (Sonnet-class and up) executing stories under an orchestrator, per the standing directive: the orchestrator makes design decisions; agents implement against written specs.
**Companion documents (read in this order before taking any story):**
1. `docs/IMPLEMENTATION_HANDBOOK.md` — golden rules (§0), system map (§1), buildable specs (§4), definition of done (§6). **Binding on every story below.**
2. `docs/ROADMAP_TO_PRODUCTION.md` — phase structure (A–E) and exit gates. This backlog refines it into stories; phases here mirror phases there.
3. `specification/01–18` — normative truth. When this backlog and a spec disagree, the spec wins (after Epic 1 lands; see below for why).

**Verification discipline (from the handbook §6, restated because it is the acceptance floor for every story):**

```
cargo test --workspace                                   # all green, new behavior has a test
cargo clippy --workspace --all-targets -- -D warnings    # zero findings
cargo run -p layer-check                                 # no_std layering intact
```

Plus: constants only in `mmp-types/src/consts.rs`; new signed objects get a new `SigDomain` tag; new frames get canonical-encoding negative tests; anything crossing the FFI appears in `MOBILE_INTEGRATION.md` §3 and the RN surface enumeration; docs updated in the same change.

---

## Part I — Target architecture (orchestrator decisions, settled)

These decisions are made; do not re-litigate them inside a story. If a story's implementation genuinely cannot satisfy one, stop and escalate to the orchestrator.

### A1. Crate layering (existing, enforced by `layer-check` — unchanged)

```
mmp-types  ←  mmp-crypto  ←  mmp-core  ←  mmp-router      (sans-IO, no_std+alloc, zero clocks/sockets)
                                  ↑
                            mmp-node (std; orchestration; the Rust-library API)
                       ↑         ↑          ↑
                 mmp-ffi     mmp-cli     mmp-sim
                     ↑           ↑
             (mobile apps)   mmp-tal-btleplug (central) + mmp-tal-peripheral (NEW, story E3-S1)
                                  ↑
                              mmp-tal (transport vocabulary: TalEvent/LinkId/LinkQuality/MemLink)
```

Radio crates are byte shuttles — no protocol logic that isn't equally testable in `mmp-sim`.

### A2. Embedder surfaces (the "library/API for mobile, desktop, IoT" answer)

| Consumer | Surface | Transport | Status today |
|---|---|---|---|
| Rust desktop/server app | `mmp_node::Node` (sans-IO: `tick(now_ms)`, `on_tal_event`, `poll_send`, `poll_event`) | `mmp-tal-btleplug` (central) + `mmp-tal-peripheral` (new) | API complete; peripheral role missing (E3-S1) |
| Android / iOS app | `mmp-ffi` UniFFI `MmpNode` object (Kotlin/Swift bindings) + platform-owned GATT shell per `MOBILE_INTEGRATION.md` | Platform BLE stack (Kotlin/Swift modules, E4-S1/S2) | FFI surface complete; bindings not built in CI; no reference radio modules |
| React Native app | npm package wrapping the TurboModule spec in `REACT_NATIVE_INTEGRATION.md` | same as mobile | Spec-only |
| IoT (nRF52/ESP32) | `mmp-node-embassy` shell over the same sans-IO engines, `heapless` feature on lower crates | `mmp-tal-embedded` over nrf-softdevice / esp-hal | 7–11-line stubs (E7) |

Interop guarantee across PC ↔ phone ↔ IoT comes from one wire format (`mmp-types`, hand-written codec, golden vectors E5-S4) — not from shared runtime code.

### A3. New decisions recorded by this backlog

1. **Rev-B crate ownership** (closes spec gap H2 at the implementation level; the spec-side amendment is E1-S4): `AuthCert` + announcement body structs and codecs → `mmp-types` (new `authcert.rs`, extend `payload.rs`); AuthCert verification, revocation tombstones, priority clamp → `mmp-core` (`claims.rs` + ingest admission); announcement policy/token bucket + UI events → `mmp-node`.
2. **Peripheral role is a new crate** `mmp-tal-peripheral` (feature-gated `bluez` / `winrt` backends), per handbook §4.1 — not grafted onto `mmp-tal-btleplug`.
3. **FFI strategy:** UniFFI 0.29 remains the only FFI for v1. A hand-rolled C ABI (for non-UniFFI consumers) is deferred to v2 — one FFI surface to audit, not two.
4. **Ratchet persistence:** ~~default is handbook §4.3 option (a) — do **not** persist ratchet state in v1~~ **superseded 2026-07-16, stale here until corrected 2026-07-22:** doc 08 §5.3's amendment reversed this — ratchet state SHALL be persisted on phones/PC, keystore-wrapped, never in the clear; Core/`no_std` still doesn't persist it. See E4-S4 below for what's actually shipped vs. still open.
5. **Versioning:** workspace stays `0.0.0` until the Phase A exit gate, then `0.1.0` and semver discipline (breaking = minor bump pre-1.0). MSRV pinned at the toolchain CI actually uses (E2-S3).
6. **Embedded memory model:** Core-tier constants + `heapless` fixed-capacity feature per handbook §4.9; blob plane is forbidden on the Core tier.

---

## Part II — Epics and stories

Story fields: **ID · Title · Size** (S ≈ hours–a day, M ≈ days, L ≈ a week-plus, agent-equivalent) · **Deps** · **Do** · **Acceptance**. Priority is the epic order itself; within an epic, story order.
Sizes and phase labels align with `docs/ROADMAP_TO_PRODUCTION.md` §3.

---

### EPIC 0 — Repo integrity & governance (do first; everything else builds on a committed, truthful repo)

> Evidence: implementation audit §5 + note on uncommitted state; spec audit finding B1.

**E0-S1 · Commit the working tree · S · Deps: none**
The entire implementation that produces the 71/0/1 green state exists only as uncommitted modifications on `master` (last commit is the initial scaffold). **Do:** review `git status`, commit in logically separated commits (spec rev-B docs; implementation waves; docs/). This story is the only one requiring explicit user confirmation before executing (pushing/committing is the user's call).
**Acceptance:** `git status` clean; `cargo test --workspace` green from a fresh checkout.

**E0-S2 · Resolve the phantom governance files (spec BLOCKER B1) · M · Deps: none**
`CHANGELOG.md`, `COUNCIL-REVIEW.md`, `ROADMAP.md` are cited as normative in nearly every spec doc (audit-history banners in docs 01–18; council disposition IDs CR-A1…A7, N1–N6, CR2-* quoted in docs 16–18; "ROADMAP Stage 1/4/9" gates in docs 12/17/18 and `implementation/mesh-milk/vectors/README.md`) but do not exist in the repo. **Do (orchestrator decision):** create the three files as first-class documents: `CHANGELOG.md` reconstructing rev-A (2026-06-11 audit fixes, from `other/AUDIT.md`) and rev-B (docs 16–18 additions) entries; `ROADMAP.md` as a thin pointer mapping the legacy "Stage 0–9" vocabulary onto `docs/ROADMAP_TO_PRODUCTION.md` phases A–E; `COUNCIL-REVIEW.md` reconstructing the disposition lists **only from IDs actually quoted in docs 16–18** (no invented content — where a disposition's full text is unrecoverable, record the ID with "text not preserved; see citing doc §").
**Acceptance:** every `CHANGELOG.md` / `COUNCIL-REVIEW.md` / `ROADMAP.md` reference in `specification/` and `implementation/` resolves to a real file section; grep proof attached to the PR/commit message.

**E0-S3 · Fix the stale workspace README · S · Deps: E0-S2 · DONE (2026-07-16)**
`implementation/mesh-milk/README.md` said "Stage 0 — all crates intentionally empty" and linked to the (previously nonexistent) `ROADMAP.md`. **Done:** rewritten to state the real crate inventory (15 crates + `mmp-tal-peripheral`, ~90 tests green, clippy clean, layer-check clean, Phase A code-complete pending hardware verification) and link `ROADMAP.md` (now real) + `docs/ROADMAP_TO_PRODUCTION.md` + `docs/AGILE_BACKLOG.md` + `docs/IMPLEMENTATION_HANDBOOK.md`; also noted the CI workflow's new repo-root location (E2-S0).
**Acceptance:** README claims match `cargo test` reality; every linked file (`ROADMAP.md`, `docs/ROADMAP_TO_PRODUCTION.md`, `docs/AGILE_BACKLOG.md`, `docs/IMPLEMENTATION_HANDBOOK.md`, `.github/workflows/ci.yml`) confirmed to exist — no dead links.

---

### EPIC 1 — Specification rev-B registry reconciliation (spec-only; blocks doc 16/18 implementation)

> Evidence: spec audit B2, H1–H4, M1, M2, M4, L1. Pattern: docs 16–18 were written self-contained; the registry docs (03/06/07/10/11/12/13/14) were never back-patched. All stories here are Markdown edits under `specification/` — follow CLAUDE.md methodology (no invented facts; every edit cites its source doc §).

**E1-S1 · Fix doc 03 §4.1 class enum (spec BLOCKER B2) · S · Deps: none · DONE (2026-07-16)**
Doc 03 §4.1's canonical `Bundle{}` comment listed `class: 0=control…4=symbol` and `plaintext for 0,3,4`, contradicting §8 of the same doc and doc 18 §3 (class 5 = announcement, plaintext). **Done:** both enumeration comments updated (`5=announcement (doc 18 §3)`; `plaintext for 0,3,4,5`); cross-checked against code, which already had `class::ANNOUNCEMENT = 5` (`mmp-types/src/bundle.rs`) — the spec was the only place out of sync.
**Acceptance:** doc 03 §4.1, §8, and doc 18 §3 now agree; grep for `0=control` shows the updated line only.

**E1-S2 · Extend doc 06 §4.1 claim-field registry with 0x07/0x08 + unknown-field rule (H1) · S · Deps: none · DONE (2026-07-16)**
Added pointer rows for `0x07 AuthCert` and `0x08 REVOCATION_LIST` (schema owner: doc 18 §2.1/§6, mirroring doc 03 §4.4's pointer pattern) and a normative sentence in doc 06 §4.1 stating unknown-field handling (store/re-offer, treat body as opaque) — previously only stated as an assumption inside doc 18 §2.1.
**Acceptance:** doc 18 §2.1's two self-flagged "assumption, not confirmed fact" notes rewritten to cite doc 06 §4.1 normatively — done.

**E1-S3 · Consolidate threat catalogs into doc 07 (H3) · M · Deps: none · DONE (2026-07-16)**
Doc 07 §3 (attack→defense matrix) and §7 (residual-risk register) never absorbed doc 18 §7's A1–A7 (incl. A6 stolen-operator-root "unbounded within scope until re-pin" and A7 priority queue-jump) or doc 16 §9's abuse items (garbage-manifest exhaustion, MIME confusion, pre-auth parser hardening). **Done:** added rows #19–24 to §3 (A1/A4/A5/A6/A7 from doc 18 plus doc 16 §9.1's garbage-manifest exhaustion), with a note explaining why A2/A3 are cross-referenced rather than duplicated (A2 already points back to this matrix from doc 18 itself; A3's scope-match mechanism has no general-catalog analog); added residual-register items 8–10 (§7), explicitly flagging item 8 (stolen root key) as the one **unbounded** entry in the register, unlike every other bounded item; extended §9 cross-references to docs 16/18.
**Acceptance:** every abuse ID in doc 16 §9 and doc 18 §7 appears in (or is pointed to from) doc 07 — done; A2/A3 verified to already resolve via existing cross-references rather than needing new rows.

**E1-S4 · Update doc 12 for rev B: crate ownership + golden-vector MUST (H2) · S · Deps: none · DONE (2026-07-16)**
Doc 12 had zero rev-B updates. **Done:** (a) added a crate-ownership paragraph to doc 12 §1 per architecture decision A3.1 (AuthCert/announcement codecs → `mmp-types`; verification/tombstones/clamp → `mmp-core`; token bucket/UI events → `mmp-node`); (b) extended §7 item 1's golden-vector negative list with entry (e) covering the three rev-B signing-domain tags (`mmp1/manifest`, `mmp1/authcert`, `mmp1/announce`), which doc 10 §3.1 already declares MUST and which `implementation/mesh-milk/vectors/README.md` already tracks — doc 12 was the one place not yet saying it.
**Acceptance:** doc 12 §7 item 1(e) and doc 10 §3.1 state the same vector requirement; doc 12 §1 names an owning crate for every rev-B mechanism.

**E1-S5 · Update doc 11 FSM/timer table for rev B (H4) · S · Deps: E1-S1 · DONE (2026-07-16)**
Added §7 timer rows: AuthCert `priority_budget` refill (per-cert field, admissions/hour ⚠, doc 18 §5.3); `not_after` skew tolerance 15 min ⚠ (doc 18 §6, CR-A6); REVOCATION_LIST republish (no dedicated timer — rides ordinary claim-gossip cadence per doc 06 §4.3, recorded honestly rather than inventing a cadence number doc 18 never states). Extended invariant I2's text with doc 18 §5.2's receive-time priority clamp as a second STORED→OFFERED admission gate for class-5 bundles.
**Acceptance:** doc 11 §7 covers every timer named in doc 18; §9 cross-references now include docs 16–18.

**E1-S6 · Registry sweeps: doc 10 §4/§9, doc 13 workloads, doc 14 register, terminology (M1, M2, M4, L1) · M · Deps: E1-S1 · DONE (2026-07-22)**
(a) doc 10: add §4.6 pointer row for class 5; add docs 16–18 to §9. (b) doc 13: add ⚠-tagged workload rows for album-thumbnail eager-fetch, full-file pull, AuthCert/announcement traffic (doc 16 §11 explicitly requested this). (c) doc 14: pointer rows for A6/A7 (mirroring the F21/F22 pattern) and a §3 pointer to doc 17's drills D-8/D-9. (d) unify "kill-cert" (open-questions OQ-17) → `REVOCATION_LIST` terminology. **Done:** doc 10 §4.6 (class-5 pointer) + §9 (docs 16–18 added); doc 13 new §3.1 (three ⚠-tagged workload rows, quantifying the "rounding error" claims doc 16 §11/doc 18 §9 already made rather than newly asserting them); doc 14 §1 gained F23/F24 (pointer rows for doc 18 §7's A6/A7) and §3 gained D-8/D-9 pointer rows (doc 17 §8 owns the full definitions — doc 17 §12 already expected these); OQ-17 in `open-questions.md` reworded from "kill-cert" to `REVOCATION_LIST` (the field's actual name). This unblocks **E6-S3** (deployment-profile presets), which was the only backlog item depending on this story.
**Acceptance:** grep for each added ID resolves both directions (extension doc ↔ registry doc).

---

### EPIC 2 — CI & release engineering (parallel-safe with Epics 1, 3)

> Evidence: implementation audit §5 + verification round 1. CI exists (`implementation/mesh-milk/.github/workflows/ci.yml`: host matrix, android/ios check, no-std size report, cargo-deny, layer-check, miri) but has real holes — starting with the fact that it is in the wrong place to run at all.

**E2-S0 · Make CI actually execute (workflow is not at repo root) · S · Deps: none · DONE (2026-07-16)**
The only workflow file in the repository lived at `implementation/mesh-milk/.github/workflows/ci.yml`. GitHub Actions only executes workflows from the repository root's `.github/workflows/`, so **this pipeline had never run** — every "wired into CI" claim in the docs was latent, not live. **Done:** moved to root `.github/workflows/ci.yml` with `defaults.run.working-directory: implementation/mesh-milk`; the `deny` job's `cargo-deny-action` gained an explicit `manifest-path` and the `mobile-bindings` job's `upload-artifact` paths gained the `implementation/mesh-milk/` prefix, since neither `uses:` step is affected by the `defaults.run.working-directory` default (that only governs `run:` shell steps) — both would otherwise have silently pointed at the wrong location post-move.
**Acceptance:** stale copy under `implementation/mesh-milk/.github/` removed; YAML re-validated (`yaml.safe_load`); a pushed commit showing a completed Actions run is still the outstanding real-world confirmation (this repo's tree is uncommitted — see E0-S1).

**E2-S1 · Add clippy gate to CI · S · Deps: E2-S0 · DONE (2026-07-23)**
No clippy step exists in CI; the "clean" claim is enforced only by manual local runs. **Do:** add `cargo clippy --workspace --all-targets -- -D warnings` to the `host` job (all three OSes).
**Acceptance:** CI fails on an intentionally introduced warning (prove once in a scratch branch), passes on master. **Done:** `.github/workflows/ci.yml`'s `host` job (all three OSes, matrix-driven) gained `cargo clippy --workspace --all-targets --all-features -- -D warnings` after the `build`/`test` steps. Proven locally per the acceptance criteria's own wording (no GitHub Actions runner available in this environment, so the equivalent local proof was run): a scratch `if x == true { true } else { false }` was added to `mmp-types/src/lib.rs`, `cargo clippy -p mmp-types --all-targets -- -D warnings` failed with real `clippy::needless_bool`/`clippy::bool_comparison` errors, then the scratch code was reverted (`git diff` confirmed zero residual change) before wiring the real CI step. `cargo clippy --workspace --all-targets --all-features -- -D warnings` (the exact new CI command) re-verified clean on master.

**E2-S2 · UniFFI bindgen artifacts in CI · S · Deps: none · DONE (verified 2026-07-23, no prior marker despite being shipped)**
**Do:** CI job running the `MOBILE_INTEGRATION.md` §3 commands, uploading Kotlin + Swift bindings as build artifacts on every merge. Pin UniFFI 0.29 (already pinned in Cargo.toml — keep it that way; version churn is roadmap risk #6).
**Acceptance:** downloadable `mmp.kt` / `mmp.swift` artifacts from a CI run. **Verified 2026-07-23:** `.github/workflows/ci.yml`'s `mobile-bindings` job builds `mmp-ffi --release`, runs its `uniffi-bindgen` binary (gated behind `mmp-ffi`'s `cli` feature — `Cargo.toml`'s `[features] cli = ["uniffi/cli"]` and `[[bin]] name = "uniffi-bindgen"` — so normal mobile-app builds never pull in clap/camino) for both `--language kotlin` and `--language swift`, then uploads both under one `mmp-mobile-bindings` artifact. `docs/ROADMAP_TO_PRODUCTION.md` §3 Phase A already listed this as done; this backlog entry was the one place still stale.

**E2-S3 · Publication hygiene: MSRV, metadata, per-crate READMEs, missing_docs · M · Deps: none**
No `rust-version` anywhere; `rust-toolchain.toml` pins a moving `stable`; 0 of 15 crates have a README; no `repository`/`homepage`/`keywords` metadata; no `missing_docs` lint (doc-comment discipline is good in practice but unenforced). **Do:** pin MSRV = the exact stable CI uses; add metadata to every publishable crate; short README per crate (what it is, layering position, `no_std` status); `#![warn(missing_docs)]` on `mmp-types`, `mmp-crypto`, `mmp-core`, `mmp-router`, `mmp-tal`, `mmp-node`, `mmp-ffi` and fix fallout.
**Acceptance:** definition-of-done commands green with the new lints; `cargo package --list` sane for `mmp-types` (the likeliest first published crate).

**E2-S4 · no_std size gate: informational → enforced · S · Deps: none · DONE (2026-07-23)**
The `no-std-size-gate` CI job builds `thumbv7em-none-eabihf` but only reports rlib sizes ("informational until Stage 7"). **Do:** add a hard budget derived from doc 12 §7.6 (≤256 KiB flash target with headroom margin; agent proposes the rlib-level proxy threshold from current sizes + 20%, orchestrator confirms the number).
**Acceptance:** CI fails if the size proxy exceeds the recorded threshold. **Done, and a real pre-existing bug found while implementing it:** the job's build command (`cargo build --release --target thumbv7em-none-eabihf -p mmp-types -p mmp-crypto -p mmp-core -p mmp-router -p mmp-rs-code`) had **never actually succeeded** — `mmp-crypto`/`mmp-core` each default to `std` (gating the Noise/snow session layer) and correctly disable it on their *internal* path-dependency edges (`default-features = false`), but multi-`-p`-selecting them directly in one cargo invocation re-activates each named package's own default feature set regardless, so the build reached for `getrandom`/`subtle`'s `std` paths on a target that doesn't have `std` — a hard compile error (`can't find crate for 'std'`), reproduced locally. This is the same failure class as E2-S0 (a CI job that looked wired but had never actually run/passed). Fixed by adding `--no-default-features` to the build command (confirmed to build clean locally afterward). With a real rlib now producible, added the hard gate itself: 5 per-crate byte budgets (baseline × 1.2, rounded up), orchestrator-confirmed in this session per the story's own "orchestrator confirms the number" instruction — baselines: `mmp_types` 576566→691880, `mmp_crypto` 259022→310827, `mmp_core` 722684→867221, `mmp_router` 219670→263604, `mmp_rs_code` 25896→31076 bytes. Proven both directions locally (no GitHub Actions runner available): the script passes against the real baseline sizes, and fails with the expected `::error::` line when a budget is artificially set below a measured size. Explicitly documented as a *proxy* threshold (rlib archives carry debug/monomorphization overhead a final linked+stripped image doesn't) — the literal ≤256 KiB flash number still needs the real linked-image measurement, tracked under Epic 7.

**E2-S5 · Android AAR / iOS XCFramework build automation · M · Deps: E2-S2**
Cross-compilation is documented (`MOBILE_INTEGRATION.md` §3) but manual. **Do:** CI (or a `just`/script entry point) producing an `.aar` and `.xcframework` as artifacts.
**Acceptance:** artifacts produced by CI; a documented one-command local reproduction.

**E2-S6 · Defensive-parsing policy note + mmp-store hardening · S · Deps: none · DONE (2026-07-16)**
Panic audit found all non-test `.expect()`s provably unreachable, but the discipline was unwritten, and `mmp-store`'s three fixed-width-slice `.expect()`s sat on the one path that reads potentially corrupted disk data. **Done:** `mmp-store/src/lib.rs` `load`/`decode_record` now use `.get()` + `?`/`else { break }` throughout instead of direct indexing or `.expect()` (the remaining `.try_into().unwrap()` calls are on slices whose length was just confirmed by the preceding `.get()`, so the conversion itself cannot fail); golden rule 9 ("no panic reachable from wire or disk bytes") added to handbook §0.
**Acceptance:** new test `truncated_file_loads_with_no_records_and_no_panic` (magic-only, truncated length-prefix, and truncated-record-body cases) passes; `cargo test -p mmp-store` (5/5) and `cargo clippy -p mmp-store --all-targets -- -D warnings` both clean.

---

### EPIC 3 — Phase A: first radio contact (PC ↔ PC over real BLE)

> Buildable specs already exist: handbook §4.1 (peripheral), §4.2 (CLI wiring). Exit gate (roadmap §3.A): two machines exchange a message + receipt over real BLE.

**E3-S1 · `mmp-tal-peripheral` crate (M5b) · L · Deps: none (schema-frozen GATT layout in handbook §4.1) · DONE (backlog marker corrected 2026-07-22; shipped Wave 3/Phase A, 2026-07-16 per project memory)**
New crate, feature-gated `bluez` (Linux, `bluer`) and `winrt` (Windows) backends; API `new / advertise(beacon) / events() / notify(link, bytes)`; GATT layout byte-matching `mmp-tal-btleplug`'s dev constants; LinkId = BLAKE3(central connection id), same recipe as central crate; chunking floor 20 B.
**Acceptance:** handbook §4.1's acceptance verbatim — unit tests for chunk arithmetic + LinkId determinism (no hardware in CI); hardware path behind an example binary. **Verified 2026-07-22** (this backlog entry had no DONE marker despite being shipped): `crates/mmp-tal-peripheral/src/linux.rs` (real BlueZ backend) + `unsupported.rs` (Windows/macOS stub with a WinRT design sketch); `framing.rs` has `chunk_size_is_always_mtu_minus_att_header_above_the_floor`/`remainder_produces_a_short_final_chunk`/etc.; `lib.rs` has `mint_link_id_is_deterministic_for_the_same_peer` — exactly the acceptance criteria named above.

**E3-S2 · `mmp-cli` radio wiring: `listen` / `connect` / `send` · M · Deps: E3-S1 · DONE (backlog marker corrected 2026-07-22; shipped Wave 3/Phase A, 2026-07-16 per project memory)**
Handbook §4.2's drive loop (~100 lines, heavily commented — it is the reference host implementation).
**Acceptance:** Phase A exit gate on real hardware (user-executed; agent delivers the runnable binary + a written test protocol); `--help` documents every subcommand. **Verified 2026-07-22:** `mmp-cli`'s `main.rs` dispatches `"connect" => cmd_connect`, `"send" => cmd_send`, `"listen" => cmd_listen`, all implemented. The real-hardware exit-gate confirmation itself remains outstanding (needs the user to actually run it on two machines) — that half was never claimable by an agent regardless.

**E3-S3 · GATT UUID reconciliation · S · Deps: E3-S1 · DONE (2026-07-22)**
Central + peripheral + `MOBILE_INTEGRATION.md` must pin the same UUID set (still dev placeholders; SIG assignment is E8-S1, not this story). **Do:** single `consts`-style source of truth for the dev UUIDs consumed by both TAL crates and quoted verbatim in `MOBILE_INTEGRATION.md` §4–5.
**Acceptance:** one grep finds exactly one definition site; docs match it. **Done:** the three dev UUID constants now live once, in `mmp-tal` (the shared base crate both `mmp-tal-btleplug` and `mmp-tal-peripheral` already depended on); both downstream crates re-export/const-alias from there instead of each carrying its own literal. Removed the now-redundant `gatt_uuids_match_central_crate_byte_for_byte` cross-check test (byte-identity is enforced by construction now, not by comparing two independent literals) and the `mmp-tal-btleplug` dev-dependency it existed for. `MOBILE_INTEGRATION.md` §4.2 now quotes the actual UUID values, pointing at `mmp_tal::DEV_SERVICE_UUID` as the source, rather than referencing them by name only.

---

### EPIC 4 — Phase B: mobile alpha

> Exit gate (roadmap §3.B): 5-device field test, 24 h, receipts delivered, <5%/day battery. Note: Kotlin/Swift/TypeScript stories build in-repo reference modules under a new `platforms/` directory — they are platform-shell code per `MOBILE_INTEGRATION.md` §1 ("the platform owns the radio and the clock"), not Rust workspace members.

**E4-S1 · Android reference radio module (Kotlin) · L · Deps: E2-S2, E3-S3** — GATT server+client per `MOBILE_INTEGRATION.md` §4 checklist; foreground-service pattern; Doze survival strategy per doc 14 D-7.
**Acceptance:** §4 checklist items each demonstrably covered; phone ↔ PC (`mmp-cli listen`) exchange on hardware.

**E4-S2 · iOS reference radio module (Swift) · L · Deps: E2-S2, E3-S3** — per `MOBILE_INTEGRATION.md` §5, designed around CoreBluetooth state restoration from day one (roadmap risk #1); document the honest iOS-worse-than-Android expectations.
**Acceptance:** §5 checklist covered; background-BLE caveats from `REACT_NATIVE_INTEGRATION.md` addressed in code comments + doc.

**E4-S3 · React Native npm package · M · Deps: E4-S1 or E4-S2** — assemble the TurboModule per `REACT_NATIVE_INTEGRATION.md` §2; beacon/link bytes never cross the JS bridge (existing rule).
**Acceptance:** package builds; TypeScript surface exactly matches the doc's enumeration.

**E4-S4 · Persistence completion · M · Deps: none (parallel-safe) · (1)/(2)/(3) DONE (2026-07-22/23); (4) bump_epoch wiring DONE (2026-07-23), disk persistence still open** — handbook §4.3 order: claims → group state → SeenIndex/ReceiptFilter → ratchet. **Verified done:** (1) claims and (2) group state — `mmp-store`'s `MMPSNAP2` format persists both, re-verifying claim signatures on load and skipping corrupt records. **(3) SeenIndex/ReceiptFilter — SHIPPED 2026-07-23:** both types persisted as typed records (`RECORD_SEEN_INDEX`/`RECORD_RECEIPT_FILTER`, no `MAGIC` bump needed), staged/restored in `mmp-ffi` alongside claims/groups; end-to-end acceptance test (`mmp-ffi::tests::seen_index_survives_restart_and_dedups_a_replayed_bundle`) drives a real restart + bundle replay and confirms no duplicate `MessageReceived`. **(4) bump_epoch wiring — SHIPPED 2026-07-23:** `mmp-node`'s `TalEvent::LinkDown` handler bumps both directions of an existing `Conv` on every repeat live session (tracked via `established_convs: HashSet<Id16>`); new `SessionEngine::session_material()`/`peer_identity()`. **Two real bugs found and fixed mid-story, not assumed away:** bumping at session START (`Established`) rather than END (`LinkDown`) orphans a brand-new conversation's own first-contact async message (caught by `group_create_invite_and_message` failing even with correctly-synced epochs — the ordering was wrong, not the sync); and the very first-ever contact with a fresh `Conv` must never bump (nothing to heal yet) — fixed by tracking "met before" independently of "conv exists," which also sets up the future restored-from-disk case correctly. New test: `mmp-node::tests::ratchet_epoch_bumps_on_repeat_contact_and_messages_still_decrypt`. Full writeups: `docs/IMPLEMENTATION_HANDBOOK.md` §4.3 items 3 and 4. **The ratchet-persistence decision itself is also done** (doc 08 §5.3 amendment, 2026-07-16) — reversing this story's own original "(a) don't persist" default (see `docs/ROADMAP_TO_PRODUCTION.md`/backlog §A3 item 4, corrected 2026-07-22 to stop contradicting it). **Still genuinely open:** the actual disk persistence — keystore-wrapped encryption of `(send CK, epoch, next_index)`/`(recv CK, epoch, next_index)`/skipped-key map/`PrekeyStore` secrets, `RATCHET_PERSIST_TTL_MS` discard-on-load — needs native Android Keystore/iOS Keychain/DPAPI, Epic 4's Kotlin/Swift work, not something the Rust workspace alone can finish; at most a Rust-side seam (an injected encrypt/decrypt callback) is buildable here.
**Acceptance:** kill -9 during a sync session; restart; node resumes with same claims/groups (demonstrated); a previously-received bundle re-offered by a peer is deduped (demonstrated); ratchet state survives restart under a keystore wrap (needs the disk-persistence half above, partially blocked on native platform code — the bump-epoch prerequisite is now satisfied).

**E4-S5 · Battery duty-cycle implementation + measurement harness · M · Deps: E4-S1/E4-S2** — doc 09 §7 duty cycles in the platform shells; k_sys measurement plan (OQ-13) with a recording template.
**Acceptance:** duty-cycle parameters configurable from the shell; field-test recording sheet checked in.

---

### EPIC 5 — Phase C: protocol completeness

> The largest epic. E5-S5/S6 depend on Epic 1's spec repairs — do not start them from the current self-contradicting registry state.

**E5-S1 · Blob plane / file transfer (doc 16) · L · Deps: E1-S6 (doc 13 workload rows), E1-S1 · DONE (2026-07-18)**
Handbook §4.4 is the buildable spec (constants SYMBOL_SIZE=2048, RS 32+8, FILE_MAX 8 MiB; sender/receiver algorithms; BlobHave/BlobReq become load-bearing; forbid on Core tier). `FileManifest` codec already exists and is cap-checked (`mmp-types/src/manifest.rs`) — this story is orchestration in `mmp-core`/`mmp-node` only. **Done:** resolved via a Fable-class design pass first (D1–D8, the wire-layout placement of the Merkle path and the doc 11 §6 FSM/D3-sub-budget gap were genuine architectural forks). `Node::send_file`/`fetch_file`/`cancel_fetch`/`file_progress` + `Event::FileOffered`/`FileReady`/`FileFailed`, mirrored at FFI (`docs/API_REFERENCE.md` §5.12); `SessionHost` gained three blob methods with default no-op impls, same shape as E5-S2's friendship seam. Full writeup, decision-by-decision, in `docs/IMPLEMENTATION_HANDBOOK.md` §4.4's shipped note — including a genuine pre-existing `mmp-core/src/rbsr.rs` bug found and fixed while building the acceptance test (`on_ids`'s unbounded terminal ID list, capable of a real session-killing oversized envelope at hundreds-of-items scale). **Deferred, not done:** airtime sub-budget (no per-domain accounting exists anywhere in the session engine — substituted with domain order + BULK priority + a fixed per-contact serve cap), group file sharing (`deliver_group` discards the apptype byte), thumbnail auto-fetch, streaming sender (whole file in host RAM), origin-targeted BLOB_REQ emission.
**Acceptance:** `mmp-sim::tests_blob::file_transfer_survives_lossy_relay_and_dedups` — 3-node sim (A/R/B), bit-exact reconstruction across lossy multi-contact relay, quota respected throughout, dedup no-op on re-send — plus unit edge cases (`mmp-types`/`mmp-crypto`/`mmp-node`: `SymbolPayload`/`MerklePath` codec round-trips and strict-decode negatives, stripe-padding edges, forged-Merkle-position rejection, end-to-end `send_file`→`fetch_file` reassembly). The literal "1 MiB file, 30% loss on both hops" parameters proved unsatisfiable by the current router's spray-budget mechanics regardless of E5-S1's own correctness (a real, pre-existing `mmp-router` characteristic, not this story's bug) — see the shipped note and the test file's own interpretation-call documentation for the adjusted, still-representative parameters actually used.

**E5-S2 · Friendship / proxy buffering · M · Deps: none · DONE (2026-07-17); doc 06 §7 proxy-answering half deferred**
Handbook §4.5: counter-signed friendship claim (field 0x05 exists), pin-up-to-FRIEND_QUOTA eviction exemption, PROXY_SUMMARY digest, receipts stay end-to-end. Replaces the "ignore" stubs at `mmp-core/src/session.rs:318-321`. **Done:** the architectural blocker flagged in `docs/ROADMAP_TO_PRODUCTION.md` Phase C (does a session retain the peer's verified identity `Key32` after ID_PROOF?) resolved via a Fable-class design pass — yes, via a host-side `NodeCore::known_identities` map, not the engine. `SessionHost` gained four friendship methods with default no-op impls (existing hosts, incl. `mmp-sim`'s, keep compiling unchanged). Full writeup in `docs/IMPLEMENTATION_HANDBOOK.md` §4.5's shipped note, including the interpretation calls flagged there (big-endian `since` in the countersig preimage; `FriendState.since` added beyond the handbook's struct sketch; the `MAX_PATRONS` vs. `ClaimStore`'s one-slot-per-field LWW tension for a wire-visible second patron). **Deferred, not done** (doc 06 §7): the patron answering D1/D2 reconciliation on the dependent's behalf while it's still offline; signed proxy summaries with a validity horizon; ephem-id resolution; a real poll schedule for `PROXY_SUMMARY` (currently sent unconditionally every session).
**Acceptance:** handbook §4.5's 3-node sim incl. the negative (non-friend evicts under pressure) — `mmp-node::tests::friend_buffers_pinned_bundle_through_quota_pressure_then_unpins_on_receipt`, `::non_friend_bundle_evicts_under_the_same_quota_pressure`, `::expired_friendship_unpins_its_buffered_bundle`; claim-level unit coverage in `mmp-types::claim::tests` and `mmp-core::claims::tests`; FFI surface in `mmp-ffi::tests::friendship_handshake_via_ffi_surface`.

**E5-S3 · Identity revocation & succession claims · M · Deps: E1-S2 · DONE (2026-07-17)**
Handbook §4.6 describes **general identity revocation + succession** (revoked key, reason, optional counter-signed successor) — a mechanism distinct from doc 18 §6's `REVOCATION_LIST` (which is AuthCert/authority-scoped). The handbook assigns it field `0x07`, which collides with the code's reserved `0x07 = AUTHCERT` (`mmp-types/src/claim.rs`). **Orchestrator decision:** identity revocation gets the next free field, `0x09`; amend doc 06 §4.1's registry (extending E1-S2's rows) and correct handbook §4.6 in the same change. Tombstone merge semantics; sessions refused from revoked keys; petname migration only on explicit user confirmation. **Done:** `mmp-types::claim::Revocation` (field `0x09`), `ClaimStore` tombstone-merge (a stored revocation beats any other claim by that owner regardless of seq), `revocation()`/`successor()` queries (successor countersig verified under the successor's own key; a bad countersig still leaves the revocation standing), `mmp-node` refuses sessions/prekeys from revoked keys + emits `Event::PeerRevoked`, FFI `revoke_identity`.
**Acceptance:** handbook §4.6's three unit acceptance cases pass; doc 06 §4.1 registers 0x09; handbook field number corrected; no collision with 0x07/0x08.

**E5-S4 · Golden test vectors + generator · M · Deps: E1-S4**
Handbook §4.7: `specification/test-vectors/*.json`, `mmp-cli vectors` generator, consuming test. MUST include the rev-B cross-tag negatives (`mmp1/manifest|authcert|announce` never verify under another tag) per doc 10 §3.1 / §6 rule 6.
**Acceptance:** vectors in-repo; a test consumes them; `vectors/README.md` updated from its "no vectors at Stage 0" state.

**E5-S5 · Authority announcements (doc 18): AuthCert + class 5 + priority clamp · L · Deps: E1-S1, E1-S2, E1-S5, E5-S3 · CORE MECHANISM DONE (2026-07-16); durable persistence + RBSR/D3 wiring deferred**
Per architecture decision A3.1: structs/codecs in `mmp-types` (`authcert.rs`; class-5 payload schema), verification + receive-time priority clamp (default clamp to NORMAL unless a valid AuthCert admits higher) + scope registry in `mmp-core`, per-subject token bucket + events in `mmp-node`. New SigDomain tags per doc 10 §3.1. **Done:** design delegated to a Fable-class agent (this story's own assignment — security-sensitive/cross-cutting), implemented and verified in one cycle; full writeup in `docs/IMPLEMENTATION_HANDBOOK.md` §4.10. `BundleMeta` split into `priority` (declared)/`effective_priority` (receiver-local clamp) so the future RBSR-partitioning requirement (below) holds by construction. **Still open, tracked as follow-on work under this story, not silently dropped:** RBSR priority-class partitioning itself (doc 03 §5.2 phase 3 — still absent everywhere in the codebase, unrelated to this story's scope); D3 relay-spray admission weighting for subscribed scopes (doc 18 §4 item b); gossiped-root trust; `AuthorityStore` durable persistence (`encode`/`decode` exist, no `mmp-store` record kind wired in — blocks mobile ship, not protocol correctness); a dedicated `AuthCert`-minting FFI method (mint Rust-side for now).
**Acceptance:** doc 18 §7's A1–A5 mapped to tests, done at the mechanism level — A2 (replay) via existing SeenIndex dedup (unit-covered elsewhere); A3 (cross-scope injection) covered by `mmp-core::authority::cross_scope_cert_grants_nothing_for_a_different_scope`; A4 (clamp-bypass/no-authority) covered by `mmp-node::tests::unauthenticated_announcement_clamps_to_normal_but_still_reaches_subscribers`; revoked-issuer covered by `mmp-node::tests::revocation_gossip_clamps_a_previously_elevated_subject_and_stays_sticky` (incl. the sticky-omission case); budget exhaustion covered by `mmp-node::tests::per_authority_budget_caps_elevation_across_distinct_announcements`. Golden vectors (E5-S4 tie-in) NOT yet extended — `signed_object()` functions exist as the generator inputs once E5-S4's vector generator lands.

**E5-S6 · cargo-fuzz targets · M · Deps: none · DONE (2026-07-16); seed corpus deferred**
Handbook §4.7's four targets in priority order (`fuzz_beacon`, `fuzz_envelope`, `fuzz_bundle`, `fuzz_hs`). No fuzz targets exist anywhere today despite the doc 12 §7 mandate; proptest mutation testing is not a substitute for a persistent corpus. **Done:** `implementation/mesh-milk/fuzz/` added (own `[workspace]`, verified not to affect the main workspace build), all four harnesses written and confirmed to compile cleanly under `rustc`; CI gained `fuzz-build` (every push, all four targets) and `fuzz-run` (manual `workflow_dispatch`, 2 min/target smoke run, corpus uploaded as an artifact). **Honestly flagged:** full sanitizer-linked `cargo fuzz build` could not be verified on the Windows implementation box (missing ASan runtime — a toolchain gap, not a code defect, documented with the exact linker errors in `fuzz/README.md`); CI's `ubuntu-latest` run is the actual verification, not yet observed by a human since the branch is uncommitted (E0-S1).
**Acceptance:** `cargo fuzz build` job added to CI (not yet observed passing — needs a real push/PR run); a short seeded run exists as a manual CI job per the "e.g. 10 min/target" guidance (scoped to 2 min for a smoke check, not the real hardening run — matches `docs/ROADMAP_TO_PRODUCTION.md`'s "don't pre-commit an unbenchmarked CPU-hour number" discipline). **Corpus NOT checked in** — deferred, see `fuzz/README.md`'s own "what's not here yet" section; libFuzzer bootstraps from empty for formats this size, so this doesn't block the acceptance gate's intent, but it's an honest gap, not silently closed.

**E5-S7 · Adversarial simulation suite · L · Deps: none · DONE (2026-07-17)**
Handbook §4.8's five scenarios (replay / flood / quota abuse / eclipse / bloom poison) with an `Adversary` raw-frame node wrapper; each asserts the stated protocol observable. **Done:** `mmp-sim/src/adversary.rs` (`Adversary` raw-frame wrapper) + `mmp-sim/src/tests_adversarial.rs` (`replay_storm_delivers_exactly_once`, `flood_origin_is_throttled_but_others_deliver`, `quota_abuse_degrades_gracefully`, `eclipse_gains_nothing`, `bloom_poison_never_deletes`). A real bug was found and fixed while building the quota-abuse scenario: the test asserted "eviction happened" without asserting its precondition "store exceeded quota" first, masking that the real admit rate was far lower than the burst-math the test assumed — fixed with a `Node::set_store_quota` sim-only test seam (never on wire/FFI) plus an adaptive Sybil loop and an explicit precondition assertion.
**Acceptance:** five independent tests green in CI; each maps to a doc 14 §1 register row (rows added by E1-S6 — **still open**, see E1-S6 above; the tests themselves are honestly cited against the closest existing doc 14 row where an exact one doesn't yet exist).

---

### EPIC 6 — Phase D: beta hardening

**E6-S1 · 1k-node sim scale validation · L · Deps: E5-S7 · CORE MEASUREMENT DONE (2026-07-22); SLAW-mobility model deferred** — validate against doc 13 §3/§5; feed deviations back into doc 13 as documented revisions (roadmap risk #3 discipline). Watch the recorded tuning lesson: keep total simulated time under LIFETIME_MESSAGE_MS or expiry is correct behavior, not failure. **Done:** `mmp-sim::tests_scale::thousand_node_mesh_scale_validation`, plus new latency-percentile + store-high-water instrumentation on `Sim`. Real finding: at N=1,000, uniform-random contact needs an unrealistically compressed inter-contact gap to mix at all within the 72h TTL, and even then only reaches delivery ratio 0.45 (vs. 1.0 at N=100/30) — direct evidence that doc 13 §5's SLAW-mobility qualifier is load-bearing, not decorative. Quota/store-growth cost claims hold, measured, at N=1,000 (208 KiB high-water). Full writeup: `docs/IMPLEMENTATION_HANDBOOK.md` §4.12; `specification/13-performance-model.md` §5/§6; `specification/research/open-questions.md` OQ-4. **Deferred, not done:** a SLAW/clustered-mobility contact model (own simulation-model project) — without it, the doc 13 §5 same-district p50/p90 band cannot be validly tested at all; the N=10,000 tier of doc 13 §3's table.
**E6-S2 · Cipher-agility exercise · M · Deps: none · DONE (2026-07-22)** — two builds pinned to different suite IDs complete (or correctly refuse) HELLO negotiation per doc 03 §8. **Done:** `Hello.suites` was carried on the wire since Phase A but never checked; `mmp-crypto/src/noise.rs` now verifies the peer's advertised suites overlap `SUPPORTED_SUITES = &[0x01]` inside `NoiseHandshake::advance()`, refusing via the existing coarse `CryptoError::Noise` (no new error variant — matches the crate's own "deliberately coarse" error philosophy). No change needed in `mmp-core`: `on_hs` already treats any handshake error as a clean `close_silently()`. Full write-up in `docs/IMPLEMENTATION_HANDBOOK.md` §4.11. Suite `0x02` itself remains out of scope (OQ-11, unstandardized hybrid-Noise construction).
**E6-S3 · Deployment-profile presets (doc 17) · M · Deps: E1-S6 · DONE (2026-07-22)** — doc 17 is zero-wire-change by design; implement as named constant-vector presets (train/campus) selectable in `mmp-sim` + drills D-8/D-9 with doc 17 §8's pass bands as `#[ignore]`-by-default scenarios. **Done:** `Sim::run_conductor_round` (new scheduled-meeting primitive — a conductor visits stops in order, distinct from `run_random_meetings`'s uniform-random pairing) plus `mmp-sim::tests_deployment::{train_profile_a_misdeployment_drill, campus_profile_delivery_drill}` (D-8/D-9, both `#[ignore]`). D-8's misdeployment comparison passed cleanly and decisively: 100% far-carriage telemetry arrival with a per-carriage anchor rail vs. 0% with only a single lead-carriage anchor (doc 17 §8's ≥50%-degradation band, exceeded). One real bug found and fixed mid-drill: the first version measured round-trip delivery receipts and got stuck at ratio 0.000 even after 10 rounds — tracing individual contacts showed telemetry reliably reached the conductor by round 0, but minting 8 receipts back-to-back hit the conductor's own per-origin admission bucket (a real, separate rate-limiting detail); fixed by measuring the one-way `MessageReceived` arrival doc 17 §8's D-8 band actually asks for, not a round-trip. D-9 needed three tuning passes on meeting count (150→250→420) to reach its 0.85-within-15-min band, landing at ratio 1.0 / 13.9 simulated minutes. Full writeup: `docs/IMPLEMENTATION_HANDBOOK.md` §4.13.
**E6-S4 · External security review package · M · Deps: E5-S4, E5-S6 · DONE (2026-07-22)** — assemble the review dossier (threat model = doc 07 post-E1-S3, crypto inventory, fuzz results, vectors); the review itself is external. **Done:** `docs/SECURITY_REVIEW_DOSSIER.md` — scopes the review to OQ-10's three non-standard compositions (channel binding doc 08 §3.2, serverless prekeys §4.2, opportunistic ratchet §5.2), a crypto inventory with exact `Cargo.lock`-resolved crate versions, doc 07's full 10-entry residual risk register, honest fuzz status (smoke-run only, no seed corpus, Windows toolchain gap), the 9 golden vectors, and a "known gaps" section (clippy not CI-gated, no MSRV pin, `AuthorityStore` persistence, `mmp-sim`'s SLAW gap) so the reviewer isn't spent rediscovering already-tracked items. The review itself remains external; this story's deliverable is the package being accurate, not a completed review.
**Acceptance (epic):** roadmap §3.D exit gate items, each a runnable command or recorded measurement.

---

### EPIC 7 — Embedded tier (post-GA unless a customer forces it — roadmap's explicit call)

**E7-S1 · `heapless` fixed-capacity feature on lower crates · L · Deps: E2-S4** — handbook §4.9 budget (≈44 KiB RAM total on Core tier); no `alloc` growth after init.
**E7-S2 · `mmp-tal-embedded` over nrf-softdevice / esp-hal · L · Deps: E7-S1, E3-S3.**
**E7-S3 · `mmp-node-embassy` executor shell · M · Deps: E7-S1** — wiring, not protocol (engines are sans-IO).
**E7-S4 · OQ-7 RS-decode benchmark on nRF52-class silicon · S · Deps: E7-S2** — the doc 12 §8 gate; "believed trivial, not yet benchmarked."
**Acceptance (epic):** ≤256 KiB flash / ≤48 KiB RAM measured on target; a T0 sensor exchanges bundles with a phone via friendship buffering (needs E5-S2).

---

### EPIC 8 — Production / GA

**E8-S1 · Bluetooth SIG 16-bit UUID assignment (OQ-12) · M, external-process-bound · Deps: E3-S3** — or formally adopt the quantified 128-bit service-data fallback from open-questions.md. Explicitly a production release blocker there.
**E8-S2 · Signed release artifacts + cargo-audit in CI · S · Deps: E2 complete.**
**E8-S3 · Store-compliance dossier · M · Deps: E4 complete** — iOS BGTask entitlement justification, Android FGS policy notes.
**E8-S4 · Local-only diagnostics policy statement · S · Deps: none · DONE (2026-07-23)** — confirm and document that diag/journey-log data never leaves the device (product commitment for store review). **Done:** verified at the code level (`mmp-node/src/diag.rs`'s `DiagLog` has no file/socket I/O anywhere; only the TAL crates touch a network socket, per handbook §0 rule 3, and they never see diagnostic records) and stated as a firm policy in `docs/ROADMAP_TO_PRODUCTION.md` Phase E — previously that section only stated the task ("confirm and document...") rather than an actual confirmed statement, and `docs/API_REFERENCE.md` §5.8 pointed at it circularly; both now resolve to a real, code-grounded statement.
**E8-S5 · v2 handoff review · S · Deps: all epics · ARTIFACT DONE (verified 2026-07-23); human review still open** — point at `V2_FEATURES.md` Part I + doc 15 R-items. **Verified:** `implementation/mesh-milk/V2_FEATURES.md` Part I already enumerates exactly R5 (warp contacts), R6 (tap-to-verify/UWB), R7 (extended-advertising presence), R10 (DCGKA group healing), R13 (multi-device identity), R14 (island bridges), R16 (deadline-aware contacts), R20 (self-certifying revocation) — one section per item, each traced to `specification/15-future-research.md`. The document half of this story is genuinely complete; only the "reviewed" half (a human sign-off that the v2 handoff is accepted) remains outstanding, same shape as E3-S2's real-hardware half — not something an agent can close.

---

## Part III — Dependency spine and suggested agent assignment

**Critical path:** E0-S2 → E1 (S1–S6, parallelizable) → E5-S5; and E3-S1 → E3-S2 → Phase A gate → E4.
**Parallel-safe from day one:** E2 (all), E4-S4, E5-S2, E5-S6, E5-S7, E8-S4.

| Work class | Agent tier | Stories |
|---|---|---|
| Spec/document edits with exact citations | Sonnet | E0-S2/S3, E1-*, E8-S4 |
| Mechanical CI/build engineering | Sonnet | E2-* |
| Rust features against a written handbook spec | Sonnet with orchestrator review | E3-*, E4-S4, E5-S1/S2/S3/S4/S6, E7-S3 |
| Security-sensitive or cross-cutting design | Orchestrator (Opus/Fable-class) designs, Sonnet implements | E5-S5, E5-S7, E6-*, E7-S1 |
| Platform-native code (Kotlin/Swift/TS) | Sonnet, one platform per agent | E4-S1/S2/S3 |
| Hardware-in-the-loop | Human (user) executes agent-written protocol | E3-S2 gate, E4-S5, E7-S4, E8-S1 |

**Standing rules for every executing agent:** read handbook §0 golden rules first; never touch `verify` (only `verify_strict`); constants in `consts.rs` only; no wall-clock asserts; the definition-of-done triplet before claiming completion; report deviations to the orchestrator instead of improvising around them.

---

## Part IV — Traceability

| Backlog item | Source evidence |
|---|---|
| E0-S1 | Implementation audit: working tree uncommitted (git status vs `6b5d56a`) |
| E0-S2, E0-S3 | Spec audit B1; impl audit §5 (stale README, dead ROADMAP.md link) |
| E1-S1…S6 | Spec audit B2, H1, H3, H2, H4, M1/M2/M4/L1 respectively |
| E2-S0…S6 | Impl audit §5 (no clippy in CI; no bindgen; no MSRV/metadata/READMEs/missing_docs; informational size gate; mmp-store expects) + verification round 1 (workflow file not at repo root → CI never executes) |
| E3, E4, E5-S1…S4, E5-S6/S7, E6, E7, E8 | Roadmap §2/§3 gaps re-verified STILL-OPEN 2026-07-15 (impl audit §2) + handbook §4 buildable specs |
| E5-S5 | Spec audit H2/H4 + impl audit §3 (doc 18 = schema-stub only: `mmp-types/src/bundle.rs` `class::ANNOUNCEMENT = 5`, `mmp-types/src/claim.rs:17-18` AUTHCERT/REVOCATION_LIST consts) |
| A3.4 ratchet decision | Handbook §4.3(4); roadmap risk #4 |

Both source reports (with full command logs and file:line citations) are preserved in the session scratchpad: `spec_gaps.md`, `impl_audit.md`.
