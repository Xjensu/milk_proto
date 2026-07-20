# Protocol Engineering Course Foundation and Audit Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Establish a revision-pinned, evidence-backed foundation for the Russian-language protocol-engineering course before any lessons are authored.

**Architecture:** Treat the specification, research notes, Rust workspace, tests, and vectors as separate evidence layers. Record findings in small audit files, reconcile them into traceability and competency matrices, then expose a minimal `course/` navigation shell whose claims point back to those matrices.

**Tech Stack:** Markdown, CSV, Git, ripgrep (`rg`), PowerShell, Cargo metadata and Cargo test commands; existing Rust workspace under `implementation/mesh-milk`.

---

## Scope and follow-on plans

This plan implements only the prerequisite foundation. It does not write lessons or project implementations. Completion unlocks separate plans for:

1. physical/network foundations and adaptive mathematics;
2. integrated Rust track;
3. DTN, routing, synchronization, security, and cryptography modules;
4. `mini_milk` simulator and labs;
5. assessment system and instructor materials;
6. capstone package and full-course verification.

## File map

| Path | Responsibility |
|---|---|
| `course/README.md` | Entry point, audience, outcomes, prerequisites, and navigation |
| `course/ROADMAP.md` | 160-hour core and optional-depth sequence |
| `course/competencies.md` | Stable competency IDs and measurable graduation outcomes |
| `course/glossary/terms.md` | Shared Russian/English protocol vocabulary |
| `course/references/source-register.md` | Course-local source register and evidence policy |
| `course/audit/baseline.md` | Git revision, toolchain, inventory, and audit method |
| `course/audit/spec-core.md` | Findings for specification documents 01–06 |
| `course/audit/security-wire.md` | Findings for specification documents 07–12 |
| `course/audit/performance-extensions.md` | Findings for documents 13–18 and research files |
| `course/audit/implementation.md` | Rust workspace architecture, conformance evidence, and gaps |
| `course/audit/traceability.csv` | Machine-readable requirement-to-code-to-test mapping |
| `course/audit/discrepancies.md` | Explicit unresolved conflicts and their teaching treatment |
| `course/audit/coverage.csv` | Competency-to-module/gate coverage matrix |
| `course/audit/README.md` | Audit navigation and completion criteria |

### Task 1: Pin the baseline and inventory the source material

**Files:**
- Create: `course/audit/baseline.md`
- Create: `course/audit/README.md`

- [ ] **Step 1: Capture the exact repository and Rust baselines**

Run from `C:\Users\Asus\Desktop\milk_proto`:

```powershell
git rev-parse HEAD
git status --short
Get-Content -LiteralPath implementation/mesh-milk/rust-toolchain.toml
cargo metadata --manifest-path implementation/mesh-milk/Cargo.toml --no-deps --format-version 1
```

Expected: one Git SHA, the existing dirty-worktree inventory, the pinned Rust toolchain, and JSON listing every workspace crate. Do not clean, reset, stage, or modify pre-existing worktree changes.

- [ ] **Step 2: Inventory all specification and evidence files**

Run:

```powershell
rg --files specification | Sort-Object
rg --files implementation/mesh-milk/crates implementation/mesh-milk/vectors | Sort-Object
```

Expected: specification documents 01–18, three research Markdown files, Rust crate sources, and test vectors.

- [ ] **Step 3: Write `course/audit/baseline.md`**

Use these exact sections:

```markdown
# Audit Baseline

## Revision
## Working-tree caveat
## Rust toolchain
## Specification inventory
## Implementation inventory
## Evidence layers
## Audit method
## Completion rule
```

Record the SHA and observed toolchain literally. Under `Working-tree caveat`, state that audit observations include unstaged/untracked user work visible on the audit date and must not be mistaken for the committed SHA alone.

- [ ] **Step 4: Write `course/audit/README.md`**

Link to every audit artifact in the file map and define completion as: all 18 specification documents read; research files read; all workspace crates classified; every course-critical normative requirement mapped or explicitly marked unmapped; every conflict copied to `discrepancies.md`.

- [ ] **Step 5: Verify and commit the baseline**

Run:

```powershell
rg -n "^## (Revision|Working-tree caveat|Rust toolchain|Specification inventory|Implementation inventory|Evidence layers|Audit method|Completion rule)$" course/audit/baseline.md
git diff --check -- course/audit/baseline.md course/audit/README.md
git add -- course/audit/baseline.md course/audit/README.md
git commit -m "docs(course): pin audit baseline"
```

Expected: eight headings, no whitespace errors, and a commit containing only the two audit files.

### Task 2: Audit the protocol core, architecture, routing, and synchronization

**Files:**
- Create: `course/audit/spec-core.md`
- Read: `specification/01-executive-summary.md`
- Read: `specification/02-research-report.md`
- Read: `specification/03-protocol-spec.md`
- Read: `specification/04-architecture.md`
- Read: `specification/05-routing-spec.md`
- Read: `specification/06-sync-spec.md`

- [ ] **Step 1: Read documents 01–06 completely**

Read each file from first line to EOF. For each document, separately extract normative requirements (`MUST`, `MUST NOT`, `SHOULD`, fixed constants), design rationale, alternatives, provisional values, persona-review concerns, and referenced research identifiers.

- [ ] **Step 2: Write one audit section per document**

Use the following repeated structure in `course/audit/spec-core.md`, replacing `NN` and the title with each real document number and title:

```markdown
## NN — Title

### Purpose
### Normative invariants
### Physical or distributed-systems model
### Design choices and alternatives
### Provisional or uncertain claims
### Dependencies and cross-references
### Teaching implications
### Potential flaw / Alternative / Resolution
```

The `Teaching implications` subsection must distinguish prerequisite concepts from mesh_milk-specific decisions. Do not resolve uncertainty by inference; carry it to Task 6.

- [ ] **Step 3: Verify coverage**

Run:

```powershell
1..6 | ForEach-Object { $id = $_.ToString('00'); if (-not (Select-String -LiteralPath course/audit/spec-core.md -Pattern "^## $id —" -Quiet)) { throw "Missing document $id" } }
rg -n "^### (Purpose|Normative invariants|Physical or distributed-systems model|Design choices and alternatives|Provisional or uncertain claims|Dependencies and cross-references|Teaching implications|Potential flaw / Alternative / Resolution)$" course/audit/spec-core.md
git diff --check -- course/audit/spec-core.md
```

Expected: all six document headings and eight audit subsections per document.

- [ ] **Step 4: Commit the core audit**

```powershell
git add -- course/audit/spec-core.md
git commit -m "docs(course): audit protocol core"
```

Expected: a commit containing only `spec-core.md`.

### Task 3: Audit security, cryptography, transport, wire format, state machines, and Rust blueprint

**Files:**
- Create: `course/audit/security-wire.md`
- Read: `specification/07-security-spec.md`
- Read: `specification/08-crypto-spec.md`
- Read: `specification/09-transport-ble.md`
- Read: `specification/10-message-format.md`
- Read: `specification/11-state-machines.md`
- Read: `specification/12-rust-blueprint.md`

- [ ] **Step 1: Read documents 07–12 completely**

Extract the adversary model, security invariants, cryptographic assumptions, metadata exposure, BLE platform constraints, canonical encoding rules, parser rejection rules, state transitions, timeout semantics, public crate seams, and mandatory testing requirements.

- [ ] **Step 2: Write `course/audit/security-wire.md` using the Task 2 section contract**

For document 08, additionally record whether a construction is a standard primitive, a composition, or a project-specific protocol. For documents 09–10, record each size/airtime dependency. For document 11, list unreachable, terminal, and error states. For document 12, map proposed crates to observed crates without claiming conformance yet.

- [ ] **Step 3: Verify coverage and dangerous ambiguity markers**

```powershell
7..12 | ForEach-Object { $id = $_.ToString('00'); if (-not (Select-String -LiteralPath course/audit/security-wire.md -Pattern "^## $id —" -Quiet)) { throw "Missing document $id" } }
rg -n "provisional|uncertain|not implemented|unverified|residual" course/audit/security-wire.md
git diff --check -- course/audit/security-wire.md
```

Expected: all six documents represented and all unresolved security/wire claims stated explicitly rather than silently normalized.

- [ ] **Step 4: Commit the security/wire audit**

```powershell
git add -- course/audit/security-wire.md
git commit -m "docs(course): audit security wire and Rust blueprint"
```

### Task 4: Audit performance, failures, extensions, and research provenance

**Files:**
- Create: `course/audit/performance-extensions.md`
- Create: `course/references/source-register.md`
- Read: `specification/13-performance-model.md`
- Read: `specification/14-failure-analysis.md`
- Read: `specification/15-future-research.md`
- Read: `specification/16-file-transfer.md`
- Read: `specification/17-deployment-profiles.md`
- Read: `specification/18-authority-announcements.md`
- Read: `specification/research/bibliography.md`
- Read: `specification/research/systems-comparison.md`
- Read: `specification/research/open-questions.md`

- [ ] **Step 1: Read documents 13–18 and all three research files completely**

For every numeric constant, record its units, derivation, evidence level, dependencies, and whether it is validated, provisional, simulated, or merely a design target.

- [ ] **Step 2: Write the six document sections**

Use the Task 2 audit contract in `course/audit/performance-extensions.md`. Add these final sections:

```markdown
## Open-question ledger
## Constant-validation ledger
## Extension placement in the course
## Research gaps requiring fresh web verification
```

Classify documents 16–18 as required core, elective, or case study at the concept level; do not omit them from traceability.

- [ ] **Step 3: Build the course source register from fetched URLs**

Create `course/references/source-register.md` with this entry contract:

```markdown
## SRC-0001 — Short title

- URL:
- Source class:
- Accessed:
- Supports:
- Confidence: High | Medium | Low
- Evidence basis:
- Limitations:
```

Copy only URLs actually present in the project bibliography or freshly opened during later research. Number entries consecutively from `SRC-0001` in source-register order; never reuse an ID. Assign these stable course IDs without changing existing bibliography identifiers. Do not cite from memory.

- [ ] **Step 4: Validate the audit and register**

```powershell
13..18 | ForEach-Object { $id = $_.ToString('00'); if (-not (Select-String -LiteralPath course/audit/performance-extensions.md -Pattern "^## $id —" -Quiet)) { throw "Missing document $id" } }
rg -n "^## (Open-question ledger|Constant-validation ledger|Extension placement in the course|Research gaps requiring fresh web verification)$" course/audit/performance-extensions.md
rg -n "^- (URL|Source class|Accessed|Supports|Confidence|Evidence basis|Limitations):" course/references/source-register.md
git diff --check -- course/audit/performance-extensions.md course/references/source-register.md
```

- [ ] **Step 5: Commit the performance and provenance audit**

```powershell
git add -- course/audit/performance-extensions.md course/references/source-register.md
git commit -m "docs(course): audit performance extensions and sources"
```

### Task 5: Audit the Rust implementation and executable evidence

**Files:**
- Create: `course/audit/implementation.md`
- Read: `implementation/mesh-milk/Cargo.toml`
- Read: every `implementation/mesh-milk/crates/*/Cargo.toml`
- Read: public modules and tests under `implementation/mesh-milk/crates/*/src/`
- Read: `implementation/mesh-milk/vectors/README.md`
- Read: every JSON file under `implementation/mesh-milk/vectors/`
- Read: `implementation/mesh-milk/fuzz/README.md`

- [ ] **Step 1: Establish the executable baseline without modifying dependencies**

```powershell
cargo test --manifest-path implementation/mesh-milk/Cargo.toml --workspace --all-targets --locked
cargo clippy --manifest-path implementation/mesh-milk/Cargo.toml --workspace --all-targets --locked -- -D warnings
```

Expected: record the exact pass/fail result, failing test or lint names, and whether failures arise from pre-existing working-tree changes. A failure is audit evidence, not authorization to fix implementation code.

- [ ] **Step 2: Classify every crate**

For each workspace member, record responsibility, public API, dependencies on other workspace crates, `std`/`no_std` posture, relevant specification documents, tests, vectors, and observed gaps.

- [ ] **Step 3: Write `course/audit/implementation.md`**

Use these sections:

```markdown
# Implementation Audit

## Build and test baseline
## Workspace dependency map
## Crate inventory
## Public protocol seams
## Test and vector inventory
## Simulation capabilities
## Mobile and embedded boundaries
## Specification-conformance evidence
## Implementation-only behavior
## Gaps and teaching implications
```

Never equate code presence with correctness; state the supporting test/vector evidence.

- [ ] **Step 4: Verify every workspace member is mentioned**

```powershell
$metadata = cargo metadata --manifest-path implementation/mesh-milk/Cargo.toml --no-deps --format-version 1 | ConvertFrom-Json
$workspacePackages = @($metadata.packages | Where-Object { $_.id -in $metadata.workspace_members })
if ($workspacePackages.Count -ne $metadata.workspace_members.Count) { throw "Workspace package resolution mismatch: packages=$($workspacePackages.Count), members=$($metadata.workspace_members.Count)" }
$workspaceNames = @($workspacePackages.name)
if (($workspaceNames | Sort-Object -Unique).Count -ne $workspaceNames.Count) { throw 'Duplicate workspace package name' }
$workspaceNames | ForEach-Object {
  $name = $_
  if (-not (Select-String -LiteralPath course/audit/implementation.md -SimpleMatch $name -Quiet)) { throw "Missing crate $name" }
}
git diff --check -- course/audit/implementation.md
```

Expected: all 17 workspace packages resolve from the modern Cargo PackageId
values, all 17 exact package names are present, no names are duplicated, and
there are no whitespace errors.

- [ ] **Step 5: Commit the implementation audit**

```powershell
git add -- course/audit/implementation.md
git commit -m "docs(course): audit Rust implementation"
```

### Task 6: Build traceability and discrepancy ledgers

**Files:**
- Create: `course/audit/traceability.csv`
- Create: `course/audit/discrepancies.md`
- Read: `course/audit/spec-core.md`
- Read: `course/audit/security-wire.md`
- Read: `course/audit/performance-extensions.md`
- Read: `course/audit/implementation.md`

- [ ] **Step 1: Create the traceability schema**

The first line of `course/audit/traceability.csv` must be:

```csv
requirement_id,spec_location,requirement_summary,implementation_location,evidence_location,status,confidence,course_treatment,notes
```

Allowed `status`: `implemented`, `partial`, `specified-only`, `implementation-only`, `conflict`, `unknown`. Allowed `confidence`: `High`, `Medium`, `Low`. Quote CSV fields containing commas.

- [ ] **Step 2: Populate all course-critical requirements**

Include identity, bundle lifecycle, plane separation, TAL contract, routing budget, quotas, reconciliation, DAG/ordering, receipts, group membership, adversary model, handshake, pairwise/group messaging, beacon and bulk framing, canonical parsing, every FSM, storage, simulation, file transfer, deployment profiles, and authority announcements.

- [ ] **Step 3: Write the discrepancy ledger**

Each `course/audit/discrepancies.md` entry must use:

```markdown
## DISC-001 — Short title

- Specification:
- Implementation:
- Evidence:
- Impact:
- Course treatment: block | teach-as-gap | elective | exclude
- Confidence: High | Medium | Low
- Resolution owner:
```

Number subsequent entries consecutively (`DISC-002`, `DISC-003`, and so on) in discovery order. `Resolution owner` identifies a role, such as protocol maintainer or security reviewer; it must not invent a person or deadline.

- [ ] **Step 4: Mechanically validate controlled values**

```powershell
$rows = Import-Csv course/audit/traceability.csv
$allowedStatus = @('implemented','partial','specified-only','implementation-only','conflict','unknown')
$allowedConfidence = @('High','Medium','Low')
$rows | ForEach-Object {
  if ($_.status -notin $allowedStatus) { throw "Bad status: $($_.status)" }
  if ($_.confidence -notin $allowedConfidence) { throw "Bad confidence: $($_.confidence)" }
  if (-not $_.requirement_id -or -not $_.spec_location -or -not $_.requirement_summary) { throw "Incomplete row: $($_.requirement_id)" }
}
$discrepancyText = Get-Content -Raw -Encoding utf8 course/audit/discrepancies.md
$discrepancyIds = @([regex]::Matches($discrepancyText, '(?m)^## (DISC-\d{3})\s+—') | ForEach-Object { $_.Groups[1].Value })
if ($discrepancyIds.Count -ne ($discrepancyIds | Sort-Object -Unique).Count) { throw 'Duplicate discrepancy heading ID' }
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
function Assert-DiscrepancyReferences([object[]]$rowsToCheck, [string]$label, [System.Collections.Generic.HashSet[string]]$referencedIds) {
  foreach ($row in $rowsToCheck) {
    $rowIds = @(Get-DiscrepancyIds ([string]$row.notes) "$label row $($row.requirement_id)")
    $missingIds = @($rowIds | Where-Object { $_ -notin $discrepancyIds })
    if ($missingIds.Count -gt 0) { throw "$label row $($row.requirement_id) references missing discrepancy IDs: $($missingIds -join ', ')" }
    foreach ($id in $rowIds) { $null = $referencedIds.Add($id) }
  }
}
$flaggedRows = @($rows | Where-Object { $_.status -in @('conflict','unknown') })
$blockRows = @($rows | Where-Object { $_.course_treatment -split '\+' -contains 'block' })
$referencedIds = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::Ordinal)
Assert-DiscrepancyReferences $flaggedRows 'Conflict/unknown' $referencedIds
Assert-DiscrepancyReferences $blockRows '+block' $referencedIds
$unreferencedIds = @($discrepancyIds | Where-Object { -not $referencedIds.Contains($_) })
if ($unreferencedIds.Count -gt 0) { throw "Discrepancy headings without an applicable trace row: $($unreferencedIds -join ', ')" }
$negativeNotes = @{
  malformed_suffix = 'context; discrepancy_ids=DISC-001-suffix'
  spaced_list = 'context; discrepancy_ids=DISC-001, DISC-002'
  duplicate_id = 'context; discrepancy_ids=DISC-001,DISC-001'
}
foreach ($case in $negativeNotes.GetEnumerator()) {
  $rejected = $false
  try { $null = Get-DiscrepancyIds $case.Value "negative case $($case.Key)" } catch { $rejected = $true }
  if (-not $rejected) { throw "Negative parser case was accepted: $($case.Key)" }
}
git diff --check -- course/audit/traceability.csv course/audit/discrepancies.md
```

Expected: all controlled values accepted; every conflict/unknown row and every
`+block` row has exactly one semicolon-delimited `discrepancy_ids=` field whose
comma-separated IDs are strict, unique, and present in the ledger. Every ledger
heading is referenced by at least one applicable trace row. The negative cases
prove rejection of a suffix, a space-separated list, and a duplicate ID. This is
a many-rows-to-one-root-cause relation, so the number of flagged rows need not
equal the number of discrepancy headings.

- [ ] **Step 5: Commit traceability**

```powershell
git add -- course/audit/traceability.csv course/audit/discrepancies.md
git commit -m "docs(course): add protocol traceability ledger"
```

### Task 7: Define the shared vocabulary and graduation competencies

**Files:**
- Create: `course/glossary/terms.md`
- Create: `course/competencies.md`
- Create: `course/audit/coverage.csv`

- [ ] **Step 1: Write the bilingual glossary**

Each entry in `course/glossary/terms.md` uses:

```markdown
## Русский термин (`English term`)

- Кратко:
- Точно:
- Не путать с:
- В mesh_milk:
```

At minimum include: protocol, service, interface, frame, packet, bundle, payload, beacon, contact, encounter, DTN, store-and-forward, routing, forwarding, reconciliation, causal order, DAG, identity, authentication, authorization, confidentiality, integrity, availability, threat model, adversary, invariant, state machine, canonical encoding, airtime, link budget, duty cycle, copy budget, TTL, quota, receipt, epoch, and transport adaptation layer.

- [ ] **Step 2: Define competency IDs**

Organize `course/competencies.md` into these stable domains:

```text
PHY — physical and energy constraints
NET — networking and wire protocols
DTN — disruption tolerance and routing
SYN — synchronization and distributed state
SEC — security engineering and applied cryptography
RST — Rust protocol implementation
SIM — modeling, simulation, and measurement
DES — requirements, architecture, trade-offs, and review
```

Each competency must have an ID such as `PHY-01`, an observable action, required evidence, and graduation level from 1 (`explain`) through 5 (`defend`). Avoid verbs such as “understand” that cannot be directly assessed.

- [ ] **Step 3: Create the coverage schema and initial rows**

The first line of `course/audit/coverage.csv` must be:

```csv
competency_id,planned_module,mini_milk_evidence,capstone_evidence,assessment_gate,status
```

Allowed `status`: `planned`, `covered`, `gap`. At this foundation stage every competency is `planned` or `gap`; none is `covered` before lesson and assessment artifacts exist.

- [ ] **Step 4: Validate IDs and complete coverage**

```powershell
$competencyIds = Select-String -LiteralPath course/competencies.md -Pattern '^## ([A-Z]{3}-\d{2}) —' | ForEach-Object { $_.Matches[0].Groups[1].Value }
$coverage = Import-Csv course/audit/coverage.csv
if ($competencyIds.Count -ne ($competencyIds | Sort-Object -Unique).Count) { throw 'Duplicate competency IDs' }
$competencyIds | ForEach-Object { if ($_ -notin $coverage.competency_id) { throw "Missing coverage row: $_" } }
if ($coverage.status | Where-Object { $_ -notin @('planned','covered','gap') }) { throw 'Invalid coverage status' }
git diff --check -- course/glossary/terms.md course/competencies.md course/audit/coverage.csv
```

- [ ] **Step 5: Commit vocabulary and competencies**

```powershell
git add -- course/glossary/terms.md course/competencies.md course/audit/coverage.csv
git commit -m "docs(course): define vocabulary and competencies"
```

### Task 8: Create the learner-facing course shell and roadmap

**Files:**
- Create: `course/README.md`
- Create: `course/ROADMAP.md`

- [ ] **Step 1: Write the learner entry point**

`course/README.md` must contain:

```markdown
# Проектирование mesh/DTN-протоколов: от нуля до собственного протокола

## Для кого этот курс
## Что вы сможете построить
## Что курс не обещает
## Входная диагностика
## Как устроено обучение
## Две проектные работы
## Как оценивается прогресс
## Требования к компьютеру
## Как начать
```

State explicitly: no prior Rust required; school mathematics is sufficient for the core; all mandatory work runs on one ordinary PC; real BLE/mobile/embedded validation is outside the first release.

- [ ] **Step 2: Write the 160-hour roadmap**

`course/ROADMAP.md` must preserve the approved budget:

| Block | Core hours |
|---|---:|
| 0. Entry and tools | 8 |
| 1. Physics | 15 |
| 2. Network foundations | 15 |
| 3. Rust I | 17 |
| 4. DTN and routing | 20 |
| 5. Synchronization | 18 |
| 6. Security and cryptography | 20 |
| 7. Rust II and robustness | 17 |
| Capstone | 30 |

Explain that approximately 30 hours of `mini_milk` work are embedded in blocks 2–7 and not added again. Optional mathematical/research depth is capped at approximately 40 additional hours.

- [ ] **Step 3: Link only existing destinations**

Link `README.md` to `ROADMAP.md`, `competencies.md`, `glossary/terms.md`, and `audit/README.md`. Do not add dead links for future module files; describe them as planned text until they exist.

- [ ] **Step 4: Verify promises against the approved design**

```powershell
rg -n "160|Rust|школьн|обычн.*ПК|mini_milk|capstone|BLE" course/README.md course/ROADMAP.md
git diff --check -- course/README.md course/ROADMAP.md
```

Expected: every major learner-facing constraint is explicit and there are no whitespace errors.

- [ ] **Step 5: Commit the course shell**

```powershell
git add -- course/README.md course/ROADMAP.md
git commit -m "docs(course): add course roadmap and entry point"
```

### Task 9: Run the foundation quality gate and prepare the next plans

**Files:**
- Modify: `course/audit/README.md`
- Modify: `course/audit/coverage.csv`
- Modify: `course/audit/discrepancies.md`

- [ ] **Step 1: Run repository-local structural checks**

```powershell
$required = @(
  'course/README.md','course/ROADMAP.md','course/competencies.md',
  'course/glossary/terms.md','course/references/source-register.md',
  'course/audit/baseline.md','course/audit/spec-core.md',
  'course/audit/security-wire.md','course/audit/performance-extensions.md',
  'course/audit/implementation.md','course/audit/traceability.csv',
  'course/audit/discrepancies.md','course/audit/coverage.csv','course/audit/README.md'
)
$required | ForEach-Object { if (-not (Test-Path -LiteralPath $_)) { throw "Missing $_" } }
git diff --check
```

Expected: all 14 foundation artifacts exist. `git diff --check` may expose unrelated pre-existing worktree warnings; distinguish them from course-file warnings and do not edit unrelated files.

- [ ] **Step 2: Re-run executable evidence checks**

```powershell
cargo test --manifest-path implementation/mesh-milk/Cargo.toml --workspace --all-targets --locked
```

Expected: record the current result in `course/audit/README.md` with date and exact command. If it differs from Task 5, add a discrepancy rather than silently replacing history.

- [ ] **Step 3: Perform the design-spec coverage review**

Read `docs/superpowers/specs/2026-07-20-protocol-engineering-course-design.md` from start to EOF. In `course/audit/README.md`, add a `## Foundation gate` checklist covering sections 3, 7, 9, 13, and 14 of the design. Each item links to concrete foundation evidence.

- [ ] **Step 4: Classify blockers for lesson planning**

Every unresolved discrepancy must be one of:

- `block`: no lesson may assert a resolved behavior;
- `teach-as-gap`: safe to teach as an explicit open engineering problem;
- `elective`: affects only optional material;
- `exclude`: outside first-release scope with rationale.

Update `coverage.csv` to `gap` wherever a `block` discrepancy prevents a planned competency from being assessed.

- [ ] **Step 5: Run the final placeholder and consistency scan**

```powershell
rg -n "T[B]D|T[O]DO|F[I]XME|X[X]X|f[i]ll in|implement l[a]ter|add appropr[i]ate|write t[e]sts for" course
$trace = Import-Csv course/audit/traceability.csv
$coverage = Import-Csv course/audit/coverage.csv
if (-not $trace) { throw 'Traceability matrix is empty' }
if (-not $coverage) { throw 'Coverage matrix is empty' }
git diff --check -- course
```

Expected: no placeholders, non-empty matrices, and no whitespace errors in `course/`.

- [ ] **Step 6: Commit the completed foundation gate**

```powershell
git add -- course/audit/README.md course/audit/coverage.csv course/audit/discrepancies.md
git commit -m "docs(course): pass foundation audit gate"
```

- [ ] **Step 7: Stop and request review**

Report the exact Git revision audited, Cargo test/clippy results, count of traceability rows, count of `block` discrepancies, count of competency gaps, and links to `course/audit/README.md`, `course/competencies.md`, and `course/README.md`. Do not begin lesson-authoring plans until the user approves the foundation gate.
