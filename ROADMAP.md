# Roadmap — Pointer Document

**This is a thin pointer, not the roadmap itself.** The authoritative, current
production roadmap is `docs/ROADMAP_TO_PRODUCTION.md` (phases **A–E**, with a
per-phase "where we are / gap analysis / exit gate" structure kept in sync with
`cargo test`/`clippy`/`layer-check` runs). This file exists only because
specification docs 12, 15, 17, and 18 (and
`implementation/mesh-milk/{README.md,vectors/README.md}`) normatively cite a
legacy **"ROADMAP Stage N"** vocabulary that predates the phase-lettered structure
and had no file to resolve to (backlog item `docs/AGILE_BACKLOG.md` §E0-S2).

**Provenance note (reconstructed 2026-07-16).** No document in this repository ever
defined "Stage 0" through "Stage 9" as a complete, named list — the numbers are
used *in passing* by several documents to refer to points in the build order that
`specification/12-rust-blueprint.md` §10 lays out. The mapping below is built by
matching each cited Stage number against that build order and against
`docs/ROADMAP_TO_PRODUCTION.md`'s current phase content. Stages 0, 1, 4, 7, and 9
are directly attested by a citing document (cited below); Stages 2, 3, 5, 6, and 8
are **interpolated** from doc 12 §10's sequential build-order list for
completeness and are marked as such — no document was found that names them by
"Stage N" explicitly.

---

## Stage 0–9 → Phase A–E mapping

| Legacy stage | What it denotes | Attested by | Current phase (`docs/ROADMAP_TO_PRODUCTION.md`) |
|---|---|---|---|
| **Stage 0** | Workspace scaffold & CI skeleton; all crates intentionally empty | `implementation/mesh-milk/README.md` ("Current stage: 0") | Precedes Phase A — complete (workspace now populated; see `docs/AGILE_BACKLOG.md` E0-S3 to bring the README's own wording current) |
| **Stage 1** | `mmp-types` codec + golden vectors + fuzz harness (doc 12 §10 step 1) | `implementation/mesh-milk/README.md` ("Stage 1 … may begin only when Stage 0's exit criteria hold"); `implementation/mesh-milk/vectors/README.md` ("Populated at ROADMAP Stage 1") | Codec: done, precedes Phase A (§1 "Where we are," `mmp-types`). Vectors/fuzz corpus: still open — Phase C ("Test vectors … published; fuzz corpus stood up") |
| **Stage 2** *(interpolated)* | `mmp-crypto` (suite 0x01, sealed box/prekeys/ratchets) + property tests (doc 12 §10 step 2) | Not cited by "Stage N" anywhere; position inferred from doc 12 §10's step ordering | Done, precedes Phase A (§1 "Where we are," `mmp-crypto`) |
| **Stage 3** *(interpolated)* | `mmp-core` session+sync against in-memory TAL; `mmp-router` (doc 12 §10 step 3) | Not cited by "Stage N" anywhere; position inferred from doc 12 §10's step ordering | Done, precedes Phase A (§1 "Where we are," `mmp-core`/`mmp-router`) |
| **Stage 4** | `mmp-sim` de-⚠ pass on doc 13 constants — "gate for everything below" (doc 12 §10 step 4) | doc 15 §1 R1 ("hook into doc 15 R1's mmp-sim artifact and `ROADMAP.md` Stage 4 gate"); doc 17 §8 (drills D-8/D-9 → sim scenarios S-8/S-9 → same Stage 4 gate) | Partially done: 10/30-node CI scenarios pass; the full de-⚠ pass needs 1k-node validation → **Phase D** ("1k-node `mmp-sim` scale validation against the doc 13 performance model") |
| **Stage 5** *(interpolated)* | `mmp-store` persistence impl; `mmp-cli` PC node over `btleplug` — first real-radio contact (doc 12 §10 step 5) | Not cited by "Stage N" anywhere; position inferred from doc 12 §10's step ordering, but its content ("first real-radio contact") is an exact match for Phase A's exit gate wording | **Phase A** — "First radio contact" (two PCs exchange a message + receipt over real BLE) |
| **Stage 6** *(interpolated)* | `mmp-ffi` + Android FG-service app skeleton; iOS skeleton (doc 12 §10 step 6) | Not cited by "Stage N" anywhere; position inferred from doc 12 §10's step ordering | **Phase B** — "Mobile alpha" |
| **Stage 7** | `mmp-tal-embedded` nRF52 Core profile + OQ-7 RS bench + **size gate enforced** (doc 12 §10 step 7) | `docs/AGILE_BACKLOG.md` E2-S4 ("only reports rlib sizes ('informational until Stage 7')") | Embedded tier — not one of Phases A–E in `docs/ROADMAP_TO_PRODUCTION.md` (that document scopes A–E to the mobile/PC production path); corresponds to `docs/AGILE_BACKLOG.md` **Epic 7** (post-GA unless a customer forces it earlier) |
| **Stage 8** *(interpolated)* | External crypto review window (OQ-10) before any v1.0 tag (doc 12 §10 step 8) | Not cited by "Stage N" anywhere; position inferred from doc 12 §10's step ordering | **Phase D** — "External crypto/security review of `mmp-crypto` and the session/handshake code" |
| **Stage 9** | Human-expert external review of the **authority/announcement model** (doc 18) before implementation — added after doc 12's original 8-step build order, specific to the rev-B extension | doc 18 header ("ROADMAP Stage 9 still gates on a human-expert external review of the authority model before implementation") | Gates `docs/AGILE_BACKLOG.md` **E5-S5** (Authority announcements implementation, Epic 5 / **Phase C** — "Protocol completeness"); not a numbered step in doc 12 §10, which predates docs 16–18 |

---

## Reading notes

- **Stages 0–3 and 5–6 predate Phase A** in the sense that `docs/ROADMAP_TO_PRODUCTION.md`
  §1 ("Where we are") already records the corresponding crates (`mmp-types`,
  `mmp-crypto`, `mmp-core`, `mmp-router`) as implemented and tested — Phase A's own
  scope is specifically the *radio* milestone (Stage 5's "first real-radio contact"),
  not the engine work that came before it.
- **Stage 7 (embedded tier) has no A–E letter.** `docs/ROADMAP_TO_PRODUCTION.md`
  explicitly scopes phases A–E to the mobile/PC production path; the embedded tier
  is tracked separately as `docs/AGILE_BACKLOG.md` Epic 7, called out there as
  "post-GA unless a customer forces it" — this is a deliberate scoping decision, not
  an omission in this mapping.
- **Stage 9 is a rev-B addition.** It postdates `specification/12-rust-blueprint.md`
  §10's original 8-step list (which was written before docs 16–18 existed) and
  applies specifically to the authority-certificate/announcement mechanism doc 18
  introduces. It is not a renumbering of doc 12's step 8 (the general external
  crypto review) — the two are distinct gates for distinct review scopes.
- For what each phase's exit gate actually requires, its current gap analysis, and
  its risk register, read `docs/ROADMAP_TO_PRODUCTION.md` directly — this file
  intentionally does not duplicate that content.

## Cross-reference

`docs/AGILE_BACKLOG.md` (story-level execution plan refining the phases below),
`docs/IMPLEMENTATION_HANDBOOK.md` (golden rules and buildable specs),
`docs/ROADMAP_TO_PRODUCTION.md` (the phase A–E roadmap itself),
`specification/12-rust-blueprint.md` §10 (the original Stage-numbering source),
`CHANGELOG.md`, `COUNCIL-REVIEW.md`.
