# Specification reboot report

Date: 2026-09-11  
Branch: `docs/spec-reboot`

## 1. Repository changes

Added:

- `docs/spec/` with one normative source per MMP/1 concept;
- `docs/extensions/`, `docs/research/`, `docs/implementation/`, and `docs/deployment/`;
- `docs/internal/` with handoffs, decisions, audits, conformance records, and release evidence;
- the root README, internal README, status manifest, and this report.

The added file sets are `docs/spec/*.md`, `docs/extensions/*.md`,
`docs/research/*.md`, `docs/implementation/*.md`, `docs/deployment/*.md`, and
the new records under `docs/internal/`.

Moved:

- former `specification/` and `course/` content was consolidated into normative, research, implementation, and internal records;
- engineering handoffs, conformance notes, audit records, implementation notes, and release records were placed under their internal categories.

The moved file sets include the former root `docs/*HANDOFF*.md`, audit and
conformance records, mobile/API guides, test guides, and deployment records.

Deleted:

- AI/process directories and root instruction files listed in section 4;
- obsolete numbered publication entry points after durable technical content was migrated.

The removed file sets are `specification/01` through `specification/18`,
`specification/research/*`, `course/*`, `docs/superpowers/*`, and the root
workflow files listed below.

## 2. Specification changes

| Area | Old documentation | New specification | Implementation status | Interoperability/security impact |
|---|---|---|---|---|
| File transfer | Plaintext symbols were described as a usable relay path. | File bytes MUST be encrypted before erasure coding and Merkle authentication. | Current implementation does not satisfy this. | File-extension interoperability is blocked until the cryptographic binding is specified and implemented. |
| Authority | Class-5 authority material appeared beside core protocol text. | Authority is an optional extension. | Local certificate code exists. | The direct-message milestone is independent of authority review. |
| ID_PROOF | Placement was described both before and after the channel. | HELLO and Noise records are pre-transport; ID_PROOF is the first encrypted record. | Current session engine matches. | Removes parser ambiguity and preserves channel binding. |
| Copy budget | Wording implied global conservation. | Honest local binary splitting conserves a budget; receiver clamps and local buckets are limits. | Current router matches the narrower rule. | Avoids a false Byzantine guarantee. |
| Clocks | No shared clock was conflated with no wall clock. | Monotonic time ages bundles; receiver-local wall time checks authority expiry. | Current code matches. | Prevents clock rollback from changing causal behavior. |
| Maturity | Numbered documents used final or production language. | Evidence and open gates are stated explicitly. | Functional prototype. | Prevents unsupported release claims. |

## 3. Implementation mismatches

- File symbols are relay-visible plaintext; encrypt-before-erasure-coding is not implemented.
- Group future-epoch distributions lack the final authenticated administrator anchor and bounded retention rule.
- Friendship and avatar claim bodies are not settled for independent implementations.
- RBSR priority partitions and authority admission weighting are absent.
- Offline Friendship proxy reconciliation and signed validity horizons are incomplete.
- Native Android/iOS BLE paths, physical BLE sessions, mobile keystore persistence, and embedded hardware recovery are unverified.
- No independent cryptographic review, sanitizer fuzz campaign, or calibrated 1,000/10,000-node mobility evidence is recorded.

## 4. Deleted AI/process artefacts

The following process-only paths were removed from the working tree after their engineering content was classified or summarized:

- `AGENTS.md`, `CLAUDE.md`, and `COUNCIL-REVIEW.md`;
- root `ROADMAP.md` and `CHANGELOG.md` as duplicated process/history entry points;
- `docs/superpowers/` plans and specifications;
- `.claude/`, `.serena/`, `.superpowers/`, `.worktrees/`, and `other/` local tooling or duplicate worktree directories;
- obsolete numbered specification and course entry points after their durable technical content was migrated.

## 5. Migrated engineering knowledge

- Implementation state and build evidence: `docs/internal/release-evidence/implementation-state.md`.
- Wire and security findings: `docs/internal/audits/design-review-findings.md`, `docs/internal/audits/`, and `docs/internal/decisions/open-protocol-decisions.md`.
- MMP/1 energy and contact-window model: `docs/research/contact-window-scheduling.md` and `docs/internal/handoff/MMP_PHI_EDR_HANDOFF.md`.
- Release gates and integration sequence: `docs/internal/handoff/release-gates.md` and `docs/internal/handoff/current-state.md`.
- Prior art and source URLs: `docs/research/bibliography.md`.

## 6. Open blockers

Core: physical direct-message and relay demonstrations, independent vector reader, full fuzz evidence, external cryptographic review, and persistence/platform validation.

Extensions: file encryption pipeline, group control, claim grammar, Friendship completion, and authority-root behavior.

Research validation: battery, contact-window scheduling, clustered mobility, and scale measurements.

Deployment validation: Linux/BlueZ runtime, Android, iOS, embedded hardware, production UUIDs, and mobile keystore/keychain behavior.

## 7. Validation commands

- `git diff --check` — passed after link and prose repair.
- `python implementation/mesh-milk/scripts/check_spec_status.py --repo . --manifest docs/spec/status-manifest.json` — passed.
- `python implementation/mesh-milk/scripts/check_release_governance.py --repo .` — passed.
- Repository-wide relative Markdown link scan — no missing links.
- `python -m unittest discover -s scripts/tests -p 'test_*.py'` from `implementation/mesh-milk` — 261 passed, 10 skipped.
- `RUSTFLAGS=-C debuginfo=0 cargo test --locked -p mmp-types -p mmp-crypto -p mmp-core -p mmp-router --all-targets -- --test-threads=1` — passed.
- `RUSTFLAGS=-C debuginfo=0 cargo test --locked -p mmp-node --lib -- --test-threads=1` — passed (198 tests).
- `cargo build --workspace --all-features --locked` — passed on Windows in the recorded implementation run.
- `cargo test --workspace --all-targets --all-features --locked -- --test-threads=1` — full run remains subject to the known MSVC PDB linker limit; the split commands above pass with debug info disabled.
