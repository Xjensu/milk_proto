# Copy-ready prompt: continue from the browser-verified administrator baseline

You are implementing the administrator workbench design system in `C:\Users\Asus\Desktop\milk_schedule`. This is authorization to implement the existing plan, not to write another plan. Work directly without sub-agents. Complete one verified batch at a time and continue through the plan. Do not stop after creating CSS files or passing syntax checks.

## Read and reconcile before editing

Read root `AGENTS.md`, `apps/schedule_web/AGENTS.md`, and the following files in `docs/design/admin-workbench-2026-09-13/`: the current opening/status table in `HANDOFF.md`, `BROWSER_AUDIT.md`, `DESIGN_SYSTEM.md`, `IMPLEMENTATION_PLAN.md`, and the complete `IMPLEMENTATION_PROMPT.md`. Read relevant `AUDIT.md` contract sections selectively. Before any API changes, read `apps/api_geteway/AGENTS.md` and actual routes/actions/contracts/repositories.

The browser audit is newer than the original source audit and supersedes its claims about missing CSS, no browser and a clean Rails worktree. The historical appended handoff includes partial implementation across several batches; it is not a completion certificate. Inspect current source rather than trusting the latest appended sentence. If the runtime changed again, record precise new evidence and reconcile the smallest discrepancy.

Inspect Git status independently in root, Rails and API. Preserve all existing changes, untracked files and tests. Do not reset, bulk-stage, stash away other work or commit without a request. No new tests or test files. Do not rebuild/restart unrelated services. Use the established runtime runbook for changes that need deployment; the audited Rails container had no application bind mounts, so editing host files alone did not change served code.

Use Playwright MCP for browser work. It successfully signed into the local administrator application during the audit. Reuse the authorized session if valid; otherwise use normal sign-in through authorized credentials. Do not run an account-creation snippet, mine old auth artifacts or put secrets in output/docs/screenshots. If access fails now, report the newly observed failure, not the historical unavailable-browser report.

## Product decisions are fixed

The UI is Russian; discussion and documentation are English. Preserve Rails ERB/Hotwire/Stimulus/importmap/Propshaft, the dark sidebar, Manrope/Unbounded and ink/velvet/almond identity. No React, Tailwind, shadcn, frontend rewrite, generic CRUD framework, new design kit or database ownership transfer.

Keep the recognizable Schedule, Workload, Teachers and Notice compositions. Resources, Calendar and Accounts demonstrate what needs repair; do not copy their permanent create forms, raw controls, long selects or overlapping editors. Unification means consistent visual roles and interaction behavior, not identical page layouts.

Never change table columns/order, header hierarchy, row meanings, grouping, axes, merged cells, totals or print semantics. Do not hide columns or convert rows to cards. Use a labelled, keyboard-reachable local scrolling region. The source audit did not find an external explicit table schema; preserve current structures and record genuine discrepancies.

Use existing `_admin_tokens.css`, `_admin_components.css`, `_admin_resources.css`, `_admin_calendar.css` and `_admin_accounts.css`. They already load. Find the owning selector and repair coverage/cascade. Do not append competing overrides or rename equivalent tokens solely to match a proposed name. Keep public UI unaffected.

Fixed visual roles: Unbounded 600 heading 24/32 desktop and 20/28 narrow; Manrope body/forms 14/20, tables 13/18, support 12/18, small metadata 11/16. Essential action/context text must not be 10px. Default controls 40px, compact row actions 36px, touch primary actions 44px. Gutters 24px desktop/16px narrow. Square tables/sheets, 4px control radius, 6px small panels. Use the design system's exact semantic colors and focus treatment.

## Preserve current progress

- Workload already retains its native five-column 850px table at 390px inside a labelled local scroll area. Do not reimplement or regress this.
- Accounts already has readable teacher labels, teacher/department search fields and retained hidden selections in the sampled editor. Keep these when replacing the editor shell.
- Account/reservation safe-draft code and typed schedule/assignment staging code exist in the dirty source. Read and verify them; do not blindly implement duplicates.
- Empty current-semester data does not mean all workload functionality is broken. A populated older-semester faculty scope exists. Odd fixture labels and duplicate abbreviations do not authorize deleting or rewriting records.
- Browser evidence is read-only. No end-to-end mutation, validation/conflict recovery, concurrency or drag-write acceptance has passed merely because screenshots exist.

## Execute in this order

### 1. Finish B00 and identify core blockers

Verify the source/assets actually served and API provenance without dumping environment or auth data. Reproduce the safe read states from BROWSER_AUDIT.md. On disposable data in the authorized local environment, execute: select faculty/semester/group -> create workload -> assign teacher and optional room -> place lesson -> edit the same lesson -> reload and authoritative read-back. Record exact results and retained identities. Never reset storage or clean fixtures wholesale.

If a reproduced defect prevents this journey, fix that narrow defect first. Read Rails route -> controller/contract/operation -> gateway -> API route/action/contract -> repository/authorization before changing a request. Do not start an infrastructure/search migration.

Reproduce V03 specifically: open `/admin/schedule?faculty_id=1&semester_id=9`, select `МК-3`, open its grid and compare browser address with frame src. The audited frame included `groups[]=789`, but the address did not; reopening the address lost the group. Fix committed context history through existing Turbo conventions. Then verify fresh URL, native reload, copied URL, Back/Forward and multi-group selection. Search typing must not silently change committed scope, and dirty drafts must be resolved before context changes.

### 2. Close B01 shared visual acceptance

Use Schedule and Workload to finish shared roles. The measured current headings are approximately 21.76px, resource controls often 28px, Calendar text 10px and Teachers heading 64px. Importing a new stylesheet is not the fix: inspect computed styles on the real controls, identify competing rules and migrate consumers to common roles.

After context is selected, replace the always-expanded choice wall with a compact readable summary and “Изменить контекст”. Use bounded searched faculty/group/department lists. Show selected/recent semesters with reachable history rather than 19 entries permanently consuming space. Disambiguate repeated abbreviations with full names/context; retain correct IDs. Apply the same ordering and label conventions across relevant pages.

Target the first work table/grid header within y=400 at 1280×900 after committed context and no expanded editor. Current evidence is around y=695/764. Meet this by reducing redundant chrome, not shrinking cells. Collapse optional planner panels if they leave too little center width; preserve all table geometry and keep selected load/destination readable. At 390px, collapse secondary monitoring links into an accessible disclosure; retain all destinations and keep active navigation visible. Target at most 160px navigation height with secondary panels closed, documenting a concrete content-driven exception if necessary.

Verify Schedule, populated Workload and an adjacent page after serving changed assets. Record computed heading/control/table sizes plus desktop/narrow comparisons before moving on.

### 3. Close B02 assignment, placement and editing

Finish a shared accessible Workload modal side sheet; the observed fixed 520px section was not a verified dialog. Place title, scrollable fields and reachable Save/Cancel in a stable structure. Implement initial focus, focus containment, inert background, Escape/dirty handling and focus return. Keep Schedule's inspector nonmodal. Do not simply add aria-modal to an aside.

Resolve the subject-change copy/affordance against the actual contract. A 25-entry activity catalog uses a bounded searchable selection list; do not assume every activity-type field deserves a tiny dropdown or an unbounded radio wall.

Click/tap/keyboard paths must execute the same operations as drag. Drag stages a labelled preview; Save persists. Never silently save on drop, optimistically remove the original record, reinterpret arbitrary text as an ID, or auto-retry an ambiguous timed-out mutation.

Assignment PUT replaces the whole teacher/room set. Add a dropped item by unioning retained IDs and send the complete set. Workload/event teachers use department membership IDs; account/reservation teachers use person IDs. Group-semester and base-group IDs are distinct. Read the actual search/write contract and preserve its identity type.

Editing keeps the event ID and load requirement. Recurring PATCH, dated transfer and atomic swap remain explicit separate operations. Never delete/create to simulate editing. On 422/409/network failure preserve the original and safe draft; on ambiguous save read back before retrying. Verify searches while dirty, cancel, invalid drop, reload and a concurrent edit scenario before claiming those guarantees.

### 4. Finish B03 Resources

Start from V05 Posts, then inspect Rooms and Subjects. Keep the existing full navigation but improve its hierarchy/wrapping. Show table/filter/create action first. Move create/edit forms into the shared sheet outside table row geometry. Make actual header/row tracks align, with all columns retained. Remove nested main landmarks. Use searched relationships with readable context and recoverable errors. Verify one complete create/edit/failure/read-back journey before expanding remaining resource variants.

### 5. Finish B04 Calendar

Treat periods, weekday transfers, holidays, semesters and reservations as distinct domain pages in the common shell. Preserve the four period meanings. Replace high-cardinality context selects with searched choices; keep the two-option semester-number enumeration native. Weekday transfers needs its five actual column tracks and a searched base-group field instead of numeric ID entry.

Reservations currently renders unavailable-data feedback with no table/editor. Trace which controller dependency fails and its real gateway/API contract before fixing it. Do not call an HTML 200 successful data loading, mask errors as empty arrays, fabricate IDs or claim existing picker acceptance while the page cannot render. Keep this diagnosis bounded to the reservation batch unless it blocks the core journey.

### 6. Finish B05 Accounts

Preserve all six columns and existing readable identity/search work. Replace the absolute action-cell editor with the common sheet; move permanent create fields behind one create action. Keep stable action cells and role-relevant fields/help. Retain existing scope selections when adding one result. Verify search failure, dirty query changes, validation/conflict, reload and role/scope authorization. Never reflect or draft passwords; preserve one-time credential no-store behavior without capturing secrets.

### 7. Complete B06 and report honestly

Verify Teachers and Notice with the shared scale while preserving their useful layouts. Check populated and empty states, keyboard/tap, long Cyrillic labels, errors, all preserved table columns, narrow active navigation, and required print/report behavior. Inspect 320px, 390px, 768px, 1280px and a wide desktop. Record any role you cannot exercise as pending, not passed.

Use relevant existing syntax/lint/loading checks and direct browser/runtime evidence; create no new tests. Capture before/after screenshots and inspect console/network for the changed journey. A green syntax check or host-source change does not prove rendered correctness.

After each batch update HANDOFF.md's current status table and append only concrete evidence: finding IDs, attributable files, runtime revision/fingerprints, tested route/context/viewport, checks, read-back result, screenshot paths, unresolved issues and exact next step. Preserve historical entries but do not leave the current table contradicting them. If browser access or deployment blocks a gate, keep that gate pending and report the precise new blocker; do not relabel source work as verified or jump batches to manufacture progress.

Final response: explain changed user behavior, completed batch gates, checks/evidence and remaining limitations. Do not claim the initiative complete until its actual interface and required flows are verified.
