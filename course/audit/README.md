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

### Provenance stages and scope

- **Historical Specification/Rust source baseline:**
  `a21d84bbe31db060264730d28eb978f69349bf20`.
  Сравнение `a21d84b..73830b7 -- specification implementation` не показало
  изменений: specification и Rust-дерево, к которым относятся выводы, не
  менялись после baseline.
- **Historical pre-gate audit input:**
  `73830b771c4a9c0944bba1573e26feb07ebdb3ad`.
- **Historical foundation-gate record:**
  `b506323d7c0f662e9548913672326fb81b1da7eb`.
- **Historical validation-plan correction:**
  `2eedbb244b3c61d75320b9c1b6acd3740756c24a`.
  These are literal stages, not a claim that one of them is the current commit
  containing a later correction.
- **Authoritative current path revisions:** do not create an impossible
  self-reference in this document. For any later correction, obtain the current
  gate-record and plan revisions from the paths themselves:

  ```powershell
  git log -1 --format=%H -- course/audit/README.md
  git log -1 --format=%H -- docs/superpowers/plans/2026-07-20-course-foundation-audit.md
  ```

  These symbolic path revisions are authoritative after the historical stages
  above; they distinguish evolving audit documentation from the fixed source
  baseline.
- **Среда:** изолированная чистая worktree
  `C:\Users\Asus\Desktop\milk_proto\.worktrees\course-foundation-audit` на
  ветке `codex/course-foundation-audit`. Исходная пользовательская checkout
  `C:\Users\Asus\Desktop\milk_proto` была грязной и не была скопирована,
  изменена или проверена; выводы не относятся к её незакоммиченным файлам.
- **Toolchain record (captured 2026-07-20):** `stable`, target
  `thumbv7em-none-eabihf`, literally from
  `implementation/mesh-milk/rust-toolchain.toml`; `rustc -Vv` resolved to:

  ```text
  rustc 1.96.1 (31fca3adb 2026-06-26)
  binary: rustc
  commit-hash: 31fca3adb283cc9dfd56b49cdee9a96eb9c96ffd
  commit-date: 2026-06-26
  host: x86_64-pc-windows-msvc
  release: 1.96.1
  LLVM version: 22.1.2
  ```

  Reproduce with `rustc -Vv`. External URLs were not re-fetched in this gate:
  dates of access and limitations remain with each [source register](../references/source-register.md)
  entry.

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

### Reproducibility commands

Run the following PowerShell commands from the repository root. They are
deliberately inline rather than a new helper script, so a later reviewer can
inspect precisely what the gate asserts.

```powershell
$ErrorActionPreference = 'Stop'
$required = @(
  'course/README.md','course/ROADMAP.md','course/competencies.md',
  'course/glossary/terms.md','course/references/source-register.md',
  'course/audit/baseline.md','course/audit/spec-core.md',
  'course/audit/security-wire.md','course/audit/performance-extensions.md',
  'course/audit/implementation.md','course/audit/traceability.csv',
  'course/audit/discrepancies.md','course/audit/coverage.csv','course/audit/README.md'
)
$required | ForEach-Object { if (-not (Test-Path -LiteralPath $_)) { throw "Missing artifact: $_" } }
if ($required.Count -ne 14) { throw "Required-artifact list changed: $($required.Count)" }

$traceHeader = 'requirement_id,spec_location,requirement_summary,implementation_location,evidence_location,status,confidence,course_treatment,notes'
$coverageHeader = 'competency_id,planned_module,mini_milk_evidence,capstone_evidence,assessment_gate,status'
if ((Get-Content -LiteralPath course/audit/traceability.csv -First 1) -ne $traceHeader) { throw 'Unexpected traceability CSV header' }
if ((Get-Content -LiteralPath course/audit/coverage.csv -First 1) -ne $coverageHeader) { throw 'Unexpected coverage CSV header' }
$trace = Import-Csv course/audit/traceability.csv
$coverage = Import-Csv course/audit/coverage.csv
if ($trace.Count -ne 82 -or $coverage.Count -ne 27) { throw "Unexpected CSV counts: trace=$($trace.Count), coverage=$($coverage.Count)" }
if (($trace.requirement_id | Sort-Object -Unique).Count -ne 82) { throw 'Duplicate traceability ID' }
if (($coverage.competency_id | Sort-Object -Unique).Count -ne 27) { throw 'Duplicate coverage competency ID' }

$competencies = Get-Content -Raw -Encoding utf8 course/competencies.md
$competencyTraceIds = @([regex]::Matches($competencies, '(?m)^- Traceability: (.+)$') |
  ForEach-Object { $_.Groups[1].Value -split ',\s*' | ForEach-Object { $_.Trim().TrimEnd('.') } } |
  Sort-Object -Unique)
$traceIds = @($trace.requirement_id | Sort-Object -Unique)
$missingTraceIds = @($competencyTraceIds | Where-Object { $_ -notin $traceIds })
$unreferencedTraceIds = @($traceIds | Where-Object { $_ -notin $competencyTraceIds })
if ($competencyTraceIds.Count -ne 82 -or $missingTraceIds.Count -ne 0 -or $unreferencedTraceIds.Count -ne 0) { throw 'Traceability reverse join is not 82/82' }

$sources = Get-Content -Raw -Encoding utf8 course/references/source-register.md
$sourceEntries = [regex]::Matches($sources, '(?m)^## SRC-\d{4} —').Count
$aliases = @([regex]::Matches($sources, '(?m)^- Supports: (.+)$') |
  ForEach-Object { [regex]::Matches($_.Groups[1].Value, '\[([^\]]+)\]') | ForEach-Object { $_.Groups[1].Value } })
$confidences = @([regex]::Matches($sources, '(?m)^- Confidence: (High|Medium|Low)$') | ForEach-Object { $_.Groups[1].Value })
if ($sourceEntries -ne 140 -or $aliases.Count -ne 140 -or ($aliases | Group-Object | Where-Object Count -ne 1).Count -ne 0) { throw 'Source aliases are not one-to-one' }
if ($confidences.Count -ne 140 -or ($confidences | Where-Object { $_ -notin @('High','Medium','Low') }).Count -ne 0) { throw 'Source confidence field is not scalar High/Medium/Low for every entry' }
```

```powershell
$competencyIds = @([regex]::Matches($competencies, '(?m)^## ([A-Z]+-\d{2}) —') | ForEach-Object { $_.Groups[1].Value })
$edges = @{}
foreach ($match in [regex]::Matches($competencies, '(?ms)^## ([A-Z]+-\d{2}) —.*?^- Предпосылки: (.+?)$')) {
  $edges[$match.Groups[1].Value] = @($match.Groups[2].Value -split ',\s*' | Where-Object { $_ -ne 'нет' })
}
$unknownPrerequisites = @($edges.Values | ForEach-Object { $_ } | Where-Object { $_ -notin $competencyIds })
$visitState = @{}; $cycle = $false
function Visit-Prerequisite([string]$id) {
  if ($visitState[$id] -eq 1) { $script:cycle = $true; return }
  if ($visitState[$id] -eq 2) { return }
  $visitState[$id] = 1
  foreach ($prerequisite in $edges[$id]) { Visit-Prerequisite $prerequisite }
  $visitState[$id] = 2
}
$competencyIds | ForEach-Object { Visit-Prerequisite $_ }
if ($competencyIds.Count -ne 27 -or $unknownPrerequisites.Count -ne 0 -or $cycle) { throw 'Prerequisite DAG check failed' }
if (($coverage | Where-Object status -eq 'planned').Count -ne 22 -or ($coverage | Where-Object status -eq 'gap').Count -ne 5 -or ($coverage | Where-Object status -eq 'covered').Count -ne 0) { throw 'Coverage planned/gap/covered counts changed' }
$gateCounts = @{}; foreach ($gate in 'A','B','C','D','E') { $gateCounts[$gate] = @($coverage | Where-Object { $_.assessment_gate -split '\+' -contains $gate }).Count }
if ($gateCounts.A -ne 4 -or $gateCounts.B -ne 12 -or $gateCounts.C -ne 13 -or $gateCounts.D -ne 5 -or $gateCounts.E -ne 10) { throw 'Coverage gate counts changed' }
```

The exact strict discrepancy parser is also Task 6 Step 4 of the
[approved foundation plan](../../docs/superpowers/plans/2026-07-20-course-foundation-audit.md).
Run this copy to validate the current ledger, including its deterministic
negative cases:

```powershell
$discrepancyText = Get-Content -Raw -Encoding utf8 course/audit/discrepancies.md
$discrepancyIds = @([regex]::Matches($discrepancyText, '(?m)^## (DISC-\d{3})\s+—') | ForEach-Object { $_.Groups[1].Value })
if ($discrepancyIds.Count -ne 7 -or $discrepancyIds.Count -ne ($discrepancyIds | Sort-Object -Unique).Count) { throw 'Unexpected or duplicate discrepancy headings' }
function Get-DiscrepancyIds([string]$notes, [string]$label) {
  $fields = @($notes -split ';' | ForEach-Object { $_.Trim() })
  $discrepancyFields = @($fields | Where-Object { $_.StartsWith('discrepancy_ids=', [System.StringComparison]::Ordinal) })
  if ($discrepancyFields.Count -ne 1) { throw "$label must contain exactly one semicolon-delimited discrepancy_ids= field" }
  $value = $discrepancyFields[0].Substring('discrepancy_ids='.Length)
  if ($value -notmatch '^DISC-\d{3}(,DISC-\d{3})*$') { throw "$label has malformed discrepancy_ids value: $value" }
  $ids = @($value -split ',')
  if ($ids.Count -ne ($ids | Sort-Object -Unique).Count) { throw "$label repeats a discrepancy ID" }
  return $ids
}
$applicableRows = @($trace | Where-Object { $_.status -in @('conflict','unknown') -or ($_.course_treatment -split '\+' -contains 'block') } | Sort-Object requirement_id -Unique)
$referencedIds = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::Ordinal)
foreach ($row in $applicableRows) {
  $rowIds = @(Get-DiscrepancyIds ([string]$row.notes) "trace row $($row.requirement_id)")
  $missing = @($rowIds | Where-Object { $_ -notin $discrepancyIds })
  if ($missing.Count -gt 0) { throw "Trace row $($row.requirement_id) references missing IDs: $($missing -join ', ')" }
  foreach ($id in $rowIds) { $null = $referencedIds.Add($id) }
}
if (@($discrepancyIds | Where-Object { -not $referencedIds.Contains($_) }).Count -ne 0) { throw 'A discrepancy heading has no applicable trace row' }
foreach ($case in @{ malformed_suffix='x; discrepancy_ids=DISC-001-suffix'; spaced_list='x; discrepancy_ids=DISC-001, DISC-002'; duplicate_id='x; discrepancy_ids=DISC-001,DISC-001' }.GetEnumerator()) {
  $rejected = $false; try { $null = Get-DiscrepancyIds $case.Value "negative $($case.Key)" } catch { $rejected = $true }
  if (-not $rejected) { throw "Negative parser case accepted: $($case.Key)" }
}
if (@($trace | Where-Object { $_.status -in @('conflict','unknown') }).Count -ne 8 -or @($trace | Where-Object { $_.course_treatment -split '\+' -contains 'block' }).Count -ne 11) { throw 'Applicable discrepancy-row counts changed' }
```

```powershell
$badLinks = @()
Get-ChildItem -Recurse -File course -Filter *.md | ForEach-Object {
  $file = $_; $body = Get-Content -Raw -Encoding utf8 $file.FullName
  foreach ($match in [regex]::Matches($body, '\]\(([^)]+)\)')) {
    $target = $match.Groups[1].Value
    if ($target -match '^(https?://|mailto:|#)') { continue }
    $path = ($target -split '#')[0]
    if ($path -and -not (Test-Path (Join-Path $file.DirectoryName $path))) { $badLinks += "$($file.FullName):$target" }
  }
}
if ($badLinks.Count -ne 0) { throw "Broken local links: $($badLinks -join '; ')" }
$roadmap = Get-Content -Raw -Encoding utf8 course/ROADMAP.md
$hours = @([regex]::Matches($roadmap, '(?m)^\| (?:0\. Entry/tools|1\. Physics|2\. Network foundations|3\. Rust I|4\. DTN/routing|5\. Synchronization|6\. Security/crypto|7\. Rust II/robustness|Capstone) \| (\d+) \|') | ForEach-Object { [int]$_.Groups[1].Value })
if ($hours.Count -ne 9 -or ($hours | Measure-Object -Sum).Sum -ne 160) { throw 'Mandatory-hour arithmetic changed' }
$emptyMaterial = @(rg -n 'T[B]D|T[O]DO|F[I]XME|X[X]X|f[i]ll in|implement l[a]ter|add appropr[i]ate|write t[e]sts for' course)
if ($emptyMaterial.Count -ne 0) { throw "Unfilled course-material markers: $($emptyMaterial -join '; ')" }
$documentedImplementationLimits = @(rg -n -i ('place' + 'holder') course)
if ($documentedImplementationLimits.Count -ne 3 -or @($documentedImplementationLimits | Where-Object { $_ -notmatch '^course\\audit\\(?:discrepancies|implementation)\.md:' }).Count -ne 0) { throw 'Unexpected documented-incomplete-implementation label outside the three evidence records' }
git diff --check -- course docs/superpowers/plans/2026-07-20-course-foundation-audit.md
```

```powershell
rustc -Vv
cargo test --manifest-path implementation/mesh-milk/Cargo.toml --workspace --all-targets --locked
cargo clippy --manifest-path implementation/mesh-milk/Cargo.toml --workspace --all-targets --locked -- -D warnings
cargo run --manifest-path implementation/mesh-milk/Cargo.toml --locked -p layer-check
```

Expected current results: 14 artifacts; trace/coverage `82/27`; reverse joins
`82/82`; 140 source entries, aliases, and scalar confidences; 27-node acyclic
prerequisite graph; planned/gap/covered `22/5/0`; gates `A4 B12 C13 D5 E10`;
seven discrepancies referenced by eight conflict/unknown and 11 `+block` rows;
160 mandatory hours; three documented incomplete-implementation labels; and
native Rust results of 111 passed, one ignored, zero failed.

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
