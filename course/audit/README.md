# Материалы аудита курса

Этот каталог фиксирует доказательную базу курса для ревизии, указанной в
[baseline.md](baseline.md). Все перечисленные артефакты существуют и являются
входом в review фундамента; это не означает, что уроки, лабораторные,
starter code или assessment rubrics уже реализованы.

- [baseline.md](baseline.md) — зафиксированная исходная ревизия.
- [spec-core.md](spec-core.md) — аудит ядра спецификации 01–06.
- [security-wire.md](security-wire.md) — аудит безопасности, wire-форматов и Rust blueprint 07–12.
- [performance-extensions.md](performance-extensions.md) — аудит производительности, отказов, расширений и research provenance 13–18.
- [implementation.md](implementation.md) — классификация реализации и исполнимых доказательств.
- [traceability.csv](traceability.csv) — трассировка 82 требований.
- [discrepancies.md](discrepancies.md) — журнал семи расхождений.
- [coverage.csv](coverage.csv) — матрица 27 компетенций и их planned/gap статусов.

## Критерий завершения

Аудит завершён, когда прочитаны все 18 спецификаций и исследовательские
файлы, классифицированы все crate, все критичные для курса требования
сопоставлены либо помечены unmapped, а конфликты перенесены в
`discrepancies.md`.

## Foundation gate — 2026-07-20

### Базовые ревизии и область проверки

- **Specification/Rust source baseline:** `a21d84bbe31db060264730d28eb978f69349bf20`.
  Сравнение `a21d84b..73830b7 -- specification implementation` не показало
  изменений: specification и Rust-дерево, к которым относятся выводы, не
  менялись после baseline.
- **Audit-documentation review HEAD:** `73830b771c4a9c0944bba1573e26feb07ebdb3ad`
  перед этой записью gate. Он является развивающейся документацией аудита, а не
  новой ревизией specification/Rust baseline; последующие коммиты документации
  должны указывать собственный HEAD.
- **Среда:** изолированная чистая worktree
  `C:\Users\Asus\Desktop\milk_proto\.worktrees\course-foundation-audit` на
  ветке `codex/course-foundation-audit`. Исходная пользовательская checkout
  `C:\Users\Asus\Desktop\milk_proto` была грязной и не была скопирована,
  изменена или проверена; выводы не относятся к её незакоммиченным файлам.
- **Toolchain record:** `stable`, target `thumbv7em-none-eabihf`, буквально из
  `implementation/mesh-milk/rust-toolchain.toml`. Внешние URL в этом gate заново
  не запрашивались: даты обращения и ограничения остаются у каждой записи
  [source register](../references/source-register.md).

### Checklist against the approved design

| Design section | Concrete foundation evidence | Gate result |
|---|---|---|
| §3, source-of-truth audit | [baseline](baseline.md) records 18 specifications, three research files, 16 crates and the source baseline; [spec-core](spec-core.md), [security-wire](security-wire.md), [performance-extensions](performance-extensions.md), and [implementation](implementation.md) record the audit; [traceability.csv](traceability.csv) provides 82 requirement rows. | PASS for audit foundation; not a claim that all implementation evidence exists. |
| §7, artifact structure | Required foundation artifacts exist: course entry point, roadmap, 27 competencies, glossary, 140-entry source register, five audit narratives, traceability, discrepancies, coverage, and this index. Learner-facing lessons/labs/projects/instructor materials are deliberately not yet created. | PASS for the pre-authoring foundation only. |
| §9, evidence and citations | [source register](../references/source-register.md) has 140 unique `SRC-*` entries and 140 one-to-one bibliography aliases; audit narratives distinguish normative text, model/design inputs, confidence, limitations, and evidence gaps. `DISC-003` preserves the unresolved `F-A6` join instead of inventing a source. | PASS with the explicit teach-as-gap exception. |
| §13, course QA | CSV imports and unique IDs pass; competency references and traceability make 82 unique reverse joins in both directions; local Markdown links pass; prerequisite graph has 27 nodes, no unknown prerequisite and no cycle; mandatory hours sum to 160; source aliases are one-to-one. The lexical scan found only three factual audit statements about actual incomplete implementation surfaces. | PASS for documentary QA; simulations, hardware, dry run, and published-material QA have not occurred. |
| §14, versioning and updates | This section records separate source baseline and audit-documentation HEADs, review date, toolchain, source-register dates, and known discrepancies. A material future source change requires a migration note and traceability update rather than silently rewriting course meaning. | PASS for the foundation record. |

### Current audit accounting

- Artifacts required for this gate: **14/14** present.
- Traceability: **82** CSV rows, unique requirement IDs, and **82/82** unique
  competency-to-trace reverse joins.
- Discrepancies: **7**. Their permitted classifications are **block**
  (`DISC-001`, `DISC-002`, `DISC-004`–`DISC-007`) and **teach-as-gap**
  (`DISC-003`); no unsupported classification appears. A `block` must remain in
  affected coverage evidence, while `teach-as-gap` becomes an explicit lesson
  about provenance rather than a false conformance claim.
- **Documented plan correction:** Task 6 Step 4 previously compared the count
  of `conflict`/`unknown` rows with the count of `DISC-*` headings. That check
  was invalid because several rows can correctly share one root cause. The
  approved structured-reference check instead requires every such row to carry
  one or more `discrepancy_ids=DISC-NNN` values in `notes`, and verifies that
  every referenced ID exists in [discrepancies.md](discrepancies.md). It passes:
  **8** `conflict`/`unknown` rows and **11** `+block` rows resolve only to the
  **7** existing discrepancies. This is a plan-check correction, not a change
  to traceability status, discrepancy classification, or ledger cardinality.
- Competencies: **27** total — **22 planned**, **5 gap**, **0 covered**.
  The five gaps and their causes remain exactly those in
  [coverage.csv](coverage.csv) and the reviewer dependency map in
  [ROADMAP.md](../ROADMAP.md): `PHY-02` (`DISC-002`, `DISC-007`), `NET-03`
  (`DISC-001`, `DISC-006`), `RST-04` (`DISC-004`, `DISC-005`, `DISC-006`),
  `SIM-03` (`DISC-002`, `DISC-007`), and `DES-03` (`DISC-002`, `DISC-004`,
  `DISC-005`, `DISC-006`, `DISC-007`).
- Planned gate evidence counts: **A=4, B=12, C=13, D=5, E=10**. These are
  planned pre-release evidence mappings, not completed learner assessments.
- Source register: **140** entries.

### Re-run evidence

All commands below were run in the isolated worktree on **2026-07-20** against
the source baseline described above.

| Exact command | Result | Observed count / limit |
|---|---|---|
| `cargo test --manifest-path implementation/mesh-milk/Cargo.toml --workspace --all-targets --locked` | exit 0 | 111 passed, 0 failed, 1 ignored, 0 measured, 0 filtered out. The ignored test is the 100-node simulator stress case. |
| `cargo clippy --manifest-path implementation/mesh-milk/Cargo.toml --workspace --all-targets --locked -- -D warnings` | exit 0 | No warnings promoted to errors. |
| `cargo run --manifest-path implementation/mesh-milk/Cargo.toml --locked -p layer-check` | exit 0 | Seven designated lower-layer crates passed the banned-dependency and `std::time`/`tokio::` token checks. This is not a target-matrix or hardware check. |

### Gate conclusion

**DONE_WITH_CONCERNS:** foundation audit artifacts are complete enough for user
review and for follow-on lesson-authoring plans. The course itself is **not
implemented**: there are no published lessons, labs, starter code, projects or
assessment rubrics, and no competency is marked covered. The five coverage gaps
and blocked evidence remain prerequisites for the affected planned gates.

Nothing in this gate validates real BLE contacts, Android/iOS behavior,
Linux/Windows transport conformance, nRF52/ESP32 or other embedded targets,
whole-phone battery use, or OEM/platform behavior. Green native Rust tests,
Clippy, and layer-check are limited executable evidence; they must not be
represented as mobile, embedded, hardware-BLE, production-readiness, or full
release validation.
