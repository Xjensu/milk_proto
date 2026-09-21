# Administrator design handoff

Updated: 2026-09-13 after authenticated Playwright MCP inspection and the focused UI pass. Existing UI implementation is partial and already present in the dirty worktrees. Read the current status below before the preserved historical entries.

## 2026-09-21 — Retry live browser acceptance

- Status: public schedule read-only evidence refreshed; staff administrator mutation/read-back remains pending.
- Browser evidence: Playwright opened `/schedule?group=1` at a 390×844 viewport. The rendered page exposed the date field and “По дате” action, weekday links, subgroup and week selectors, the event card, print action, and the disabled “Переносы 0” state. The page returned no console errors.
- Administrator access: navigating to `/admin/workload` redirected to `/?auth=1` and showed the normal login dialog. No credentials were entered and no business mutation was sent.
- Source/runtime context: Rails and Hanami containers remain healthy; the public route exercised the current running Rails revision. This evidence does not certify the staff-only B00/B02 journey, resource CRUD, calendar mutations or account read-back.
- API read-back: the running Hanami service returned HTTP 200 for `/public/groups/1/schedule?date=2026-09-20` with `meta.anchor_date = 2026-09-20`, one semester descriptor, seven occurrences, and explicit `transfers` and `holidays` arrays.
- Next action: obtain an already authenticated staff browser session, then execute the documented workload → assignment → placement → edit → reload journey before claiming final acceptance.

## 2026-09-21 — Reconcile F01 source status

- The older `docs/TZ_COMPLIANCE_AUDIT_2026-09-20.md` was written before the aggregate schedule slice and still describes faculty/department schedules as missing. That file is preserved as historical audit evidence.
- Current source status: Hanami exposes staff-protected date-resolved reads at `GET /admin/faculties/:faculty_id/schedule/occurrences` and `GET /admin/kafedras/:kafedra_id/schedule/occurrences`; both use the shared occurrence query with semester clipping, transfers, cancellations and holidays. Rails routes these through `AggregateScheduleGateway`, `AggregateSchedule` operation/presenter and `/admin/aggregate_schedule`, with a selected-date print path.
- Assessment: F01 is source-complete and runtime-liveness verified, while authenticated scope, dated transfer/cancellation read-back and print evidence remain pending. The old audit must not be used as proof that the feature is still absent.

## 2026-09-21 — Remove aggregate print shell gap

- Finding: aggregate print CSS hid the sidebar and context panels but left the two-column admin shell active, which could reserve an empty sidebar track in print output.
- Behavior: the aggregate print rule now switches `.admin-shell` and `.admin-main` to a block print layout with no viewport minimum height. The selected-date report table, columns and transfer/cancellation styling remain unchanged.
- Rails revision: `e8f9f65`.
- Verification: Docker image rebuild, forced `schedule_web` recreation, Rails Zeitwerk check and internal `/up` HTTP 200 passed. Authenticated print preview remains pending.

## 2026-09-21 — Verify public selected-date print state

- Playwright emulated print media on `/schedule?group=1&date=2026-09-15`.
- The schedule table remained displayed, the mobile agenda was hidden, the date input/navigation controls were suppressed by print CSS, and the rendered date field still held `2026-09-15` before print. No console errors were reported.
- This verifies the public group print contract only; authenticated aggregate print preview remains pending.

## 2026-09-21 — Collapse aggregate context picker

- Finding: the aggregate faculty/department catalog was permanently expanded above the report, creating an unnecessary tall context wall after a scope had already been selected.
- Behavior: the catalog is now inside an accessible native `<details>` disclosure. The summary keeps the selected Russian scope label visible; opening it exposes the existing scope tabs, search field and bounded option list. Report columns, URL parameters and API identity remain unchanged.
- Rails revision: `ce75c66`.
- Verification: Docker build, forced `schedule_web` recreation, Rails Zeitwerk check, internal `/up` HTTP 200 and `git diff --check` passed. Authenticated computed-height and keyboard evidence remain pending.

## 2026-09-21 — Bound Schedule inspector assignment pickers

- Finding: the Schedule inspector’s teacher and room checkbox catalogs were unbounded inside the sticky editor, so a large result set could push the confirmation action below a long scroll.
- Behavior: `.planner-assignment-choices` now has a labelled, keyboard-reachable 248px scrolling region with readable 40px options and hover/focus states. The search fields, checked values, form names and server operations are unchanged; the existing sticky submit remains available.
- Rails revision: `200d149`.
- Verification: Docker build, forced `schedule_web` recreation, Rails Zeitwerk check, internal `/up` HTTP 200 and `git diff --check` passed. Authenticated computed-height and save/read-back evidence remain pending.

## 2026-09-21 — Add aggregate disclosure focus state

- The aggregate context disclosure summary now has an explicit visible keyboard focus ring and hover state using the shared velvet focus treatment.
- Rails revision: `b29b4fa`.
- Verification: `git diff --check` passed; Docker image rebuilt and `schedule_web` recreated; Zeitwerk and `/up` passed; the served fingerprinted workspace CSS contains the focus rule. The rule is scoped to the aggregate administrator control and does not affect public UI.

## Current continuation — browser evidence takes precedence

### 2026-09-20 — Preserve batch Notice workload context

- Status: source-complete; authenticated batch PDF queue/read-back evidence remains pending.
- Finding: the department batch Notice export returned to a reduced workload URL containing only department and semester, dropping faculty, selected groups and the workload query.
- Behavior: batch export forms and status/return links now preserve faculty, selected group IDs and the bounded workload query while keeping the existing department/semester/period contract for the API job.
- Files changed: `apps/schedule_web/app/controllers/admin/notice/exports_controller.rb`, `apps/schedule_web/app/views/admin/workload/show/show.html.erb`.
- Source revision: Rails `b81b8bf`.
- Runtime/source verification: container Ruby syntax, Workload ERB compilation, Rails Zeitwerk and `git diff --check` passed.
- Routes, role, context, viewport: Workload department mode and `/admin/notice/export_batch`, staff administrator, selected faculty/groups/query/semester; authenticated batch export status remains pending.
- Next action: rebuild `schedule_web`, then verify queue failure and completion return to the same department workload context.

### 2026-09-20 — Preserve Notice export search context

- Status: source-complete; authenticated PDF queue/read-back evidence remains pending.
- Finding: the Notice PDF action submitted teacher and semester IDs but dropped the active teacher search query. A queue failure or export status return could therefore lose the operator’s current context.
- Behavior: individual Notice export requests and status/return links now carry the bounded Russian teacher query alongside teacher, semester and period parameters. Report rows, period restrictions and batch export behavior remain unchanged.
- Files changed: `apps/schedule_web/app/controllers/admin/notice/exports_controller.rb`, `apps/schedule_web/app/views/admin/notice/show/show.html.erb`.
- Source revision: Rails `6d28d2d`.
- Runtime/source verification: container Ruby syntax, Notice ERB compilation, Rails Zeitwerk and `git diff --check` passed.
- Routes, role, context, viewport: `/admin/notice`, staff administrator, selected teacher/search/semester/period; authenticated export and status read-back remain pending.
- Next action: rebuild `schedule_web`, then verify PDF queue failure/success returns to the same Notice context.

### 2026-09-20 — Align Schedule group search control

- Status: source-complete; authenticated computed-size evidence remains pending.
- Finding: the Schedule group selector search field retained a 36px legacy height while the shared administrator control contract requires 40px ordinary controls.
- Behavior: the group search field now uses the same 40px height as the surrounding context controls. Group filtering, checkbox selection and URL parameters are unchanged.
- Files changed: `apps/schedule_web/app/assets/stylesheets/components/_admin_workspace.css`.
- Source revision: Rails `15681ee`.
- Runtime/source verification: Rails Zeitwerk, `git diff --check`, Docker build, Propshaft precompile, `schedule_web` recreation and `/up` health response passed.
- Routes, role, context, viewport: `/admin/schedule`, staff administrator, selected faculty/group context; authenticated desktop and narrow measurements remain pending.
- Next action: measure the Schedule context controls and complete the core authenticated read-back journey.

### 2026-09-20 — Align searchable administrator controls

- Status: source-complete; computed-size browser evidence remains pending.
- Finding: several searchable catalog fields and Calendar reservation row actions still used 36px controls, below the shared 40px default target.
- Behavior: Accounts, Calendar, Resources and Workload search controls now use 40px fields; reservation row actions also use 40px. Table row density, schemas and primary touch actions remain unchanged.
- Files changed: `apps/schedule_web/app/assets/stylesheets/components/_admin_accounts.css`, `app/assets/stylesheets/components/_admin_calendar.css`, `app/assets/stylesheets/components/_admin_resources.css`, `app/assets/stylesheets/components/_admin_workload.css`.
- Source revision: Rails `f4d4402`.
- Runtime/source verification: Rails Zeitwerk and `git diff --check` passed. Docker/Propshaft deployment and authenticated computed-size verification remain pending.
- Next action: rebuild `schedule_web`, then measure the focused search and reservation controls in the authenticated browser.

### 2026-09-20 — Remove legacy account disclosure styles

- Status: source-complete; authenticated modal geometry evidence remains pending.
- Finding: obsolete `<details>` account-editor rules remained in shared and Accounts CSS after editors moved to native dialogs. Those rules duplicated positioning, padding and open-state layout and could override the common sheet contract.
- Behavior: the former disclosure rules are removed. Native account dialogs now use the shared sheet styles plus the scoped fixed-position guard; account table columns, actions and form contracts are unchanged.
- Files changed: `apps/schedule_web/app/assets/stylesheets/components/_admin_workspace.css`, `apps/schedule_web/app/assets/stylesheets/components/_admin_accounts.css`.
- Source revision: Rails `22380ad`.
- Runtime/source verification: Rails Zeitwerk and `git diff --check` passed; a fresh Docker/Propshaft build remains the deployment check for this CSS-only slice.
- Routes, role, context, viewport: `/admin/accounts`, staff account manager, existing account row; desktop and narrow sheet geometry remain pending without authenticated browser access.
- Next action: rebuild `schedule_web`, then verify the editor’s computed modal geometry and internal scroll.

### 2026-09-20 — Preserve reservation filters across pagination

- Status: source-complete; authenticated reservation pagination evidence remains pending.
- Finding: Room Reservations pagination links retained only room/date filters. Moving between pages silently dropped the active room search, teacher search and Calendar context.
- Behavior: previous/next reservation links now carry the existing room, teacher, date and Calendar context parameters while changing only the page number. Reservation columns and API contracts are unchanged.
- Files changed: `apps/schedule_web/app/views/admin/room_reservations/index.html.erb`.
- Source revision: Rails `7157d7d`.
- Runtime/source verification: Reservation ERB compilation, Rails Zeitwerk and `git diff --check` passed.
- Routes, role, context, viewport: `/admin/room_reservations`, staff schedule writer, active search/date filters; browser pagination read-back remains pending.
- Next action: verify that a filtered page transition preserves the visible query and selected context in the authenticated browser.

### 2026-09-20 — Keep account editors in native modal sheets

- Status: source-complete; authenticated overlap/read-back evidence remains pending.
- Finding: the legacy `.account-editor { position: relative; }` rule still matched the new native account `<dialog>` editors. That allowed the editor to inherit table-layout positioning and reproduced the Accounts overlap risk from V07.
- Behavior: account editors now explicitly use fixed positioning inside the shared modal-sheet contract. The six-column account table, row actions, role scopes and password handling are unchanged.
- Files changed: `apps/schedule_web/app/assets/stylesheets/components/_admin_accounts.css`.
- Source revision: Rails `3c81c35`.
- Runtime/source verification: Rails Zeitwerk, Accounts ERB compilation, `git diff --check`, Docker image build, Propshaft precompile and `schedule_web` recreation passed.
- Routes, role, context, viewport: `/admin/accounts`, staff account manager, existing account row; desktop and narrow modal geometry remain pending without an authenticated browser session.
- Next action: verify the account editor's computed position, internal scroll and Escape/focus return in the authenticated browser, then continue the remaining reservation dependency read-back.

### 2026-09-18 — academic-structure catalogs added to the Rails checkout (runtime verification pending)

The current schedule-filling handoff identified the missing administrative path for the reference data that precedes group creation and workload allocation. Source tracing confirmed that Hanami already exposes authorized CRUD actions and contracts for faculties, departments (`kafedras`) and specialities, while Rails had no routes or resource tabs for them. Existing subject, activity-type, time-slot, week-type and student-group catalog tabs already cover the other required reference data. Curriculum streams were deliberately not surfaced as a new catalog: the current workload workflow creates and manages schedule load requirements directly, and the schedule-filling handoff does not require manual stream creation.

The Rails checkout now contains `Справочники → Факультеты`, `Кафедры` and `Специальности`. The three pages share one scoped controller and ERB form partial, use the existing `TeachersWorkspaceGateway` with the actual `/admin/faculties`, `/admin/kafedras` and `/admin/specialities` envelopes, retain safe drafts after recoverable failures, and select related faculties by name and short name rather than raw ID input. The resource switcher and page CSS preserve the existing square table/sheet system and local table scrolling.

Validation: Ruby syntax passed for the controller, gateway and routes; `bundle exec rails routes -g 'faculties|kafedras|specialities'` listed all twelve routes; `bundle exec rails zeitwerk:check` passed; ERB compilation source generated; scoped whitespace checks passed. On 2026-09-18, `schedule_web` was rebuilt and recreated through the normal Compose path. The running container's controller, view and CSS SHA-256 values match the checkout; `/up` and the fingerprinted application/resource CSS both return HTTP 200; unauthenticated requests to all three routes correctly redirect to sign-in; its logs contain no startup or request error after recreation.

Authenticated browser, mutation, and read-back evidence remains pending. Playwright MCP cannot start because the local Chrome binary is absent; its documented installation command failed with a registry proxy-authentication error. Do not claim this slice is visually or operationally accepted until an authenticated browser session is available. The next action is to restore the browser dependency/proxy path, then verify each page with a super-admin session, including create, validation retention, reload and relationship read-back.

Read [BROWSER_AUDIT.md](BROWSER_AUDIT.md) for measurements, exact reproduction, 17 screenshots, partial runtime provenance and untested behavior. The copy-ready next execution task is [BROWSER_BASED_CONTINUATION_PROMPT.md](BROWSER_BASED_CONTINUATION_PROMPT.md); the complete initiative prompt remains [IMPLEMENTATION_PROMPT.md](IMPLEMENTATION_PROMPT.md).

Playwright MCP successfully opened the local application and normal administrator sign-in succeeded. No account insertion or business mutation was performed. Do not store the supplied credentials or rerun the account-creation snippet. The old browser-unavailable reports below are historical.

Preserve existing implementation: token/component/domain CSS loads; Workload keeps its five columns in a labelled narrow scroll region; account person labels and searched edit fields render. No batch is fully accepted from these observations. Existing account failure-retention and schedule drag code must be inspected and exercised, not blindly rewritten or accepted.

Focused continuation: the user narrowed implementation to Resources, Calendar and Accounts and asked to restore continuation-only Schedule/Workload changes. Those Schedule/Workload changes were reverted; their historical browser measurements remain evidence only. The current pass is UI/UX focused: table-first create disclosures, coherent calendar/resource navigation, scoped square surfaces and preserved table schemas. Functional mutation journeys remain unverified because the user reports existing functionality is working and this pass did not perform business writes.

The historical sections 3–4 below describe the initial source audit. Later appended entries record intermediate source work and sometimes contradict one another about runtime deployment/search support. Use this current section and the browser audit for observed state; inspect touched source for anything not runtime-tested. Never interpret the last appended source claim as a completed acceptance gate.

## 1. Read this first

The user asks for a consistent administrator design system and much better UX while preserving the accepted Schedule/Workload character and all required table structures. They now say functionality works. This supersedes older notes telling agents to assume workload/schedule creation is broken. Future implementation still verifies those flows and fixes any reproduced regression first.

Work directly. No sub-agents or review ceremonies. No new tests/test files. English communication/docs, Russian UI. Preserve all dirty work in root and nested repositories. Do not change runtime data or deploy for a documentation-only request.

## 2. Package and authority

| File | Purpose |
| --- | --- |
| [AUDIT.md](AUDIT.md) | Historical source findings, architecture and contract evidence |
| [BROWSER_AUDIT.md](BROWSER_AUDIT.md) | Current rendered findings V01–V08, screenshots and evidence limits |
| [DESIGN_SYSTEM.md](DESIGN_SYSTEM.md) | Fixed token/type/control/layout/drag/editor decisions |
| [IMPLEMENTATION_PLAN.md](IMPLEMENTATION_PLAN.md) | Ordered batches, target files and acceptance gates |
| [IMPLEMENTATION_PROMPT.md](IMPLEMENTATION_PROMPT.md) | Copy the entire file to start implementation |
| [BROWSER_BASED_CONTINUATION_PROMPT.md](BROWSER_BASED_CONTINUATION_PROMPT.md) | Copy the entire file to resume the existing partial implementation |
| Root `AGENTS.md` | Workspace constraints and latest priority |
| `apps/schedule_web/AGENTS.md` | Detailed Rails implementation rules |
| `apps/api_geteway/AGENTS.md` | API contract/identity/integrity guardrails |

This package overrides conflicting older design plans, mandatory reviewer-agent instructions and old broken-flow assumptions for this initiative. Preserve other architecture/security/data rules. Older Rails `docs/handoff/OPERATIONS.md`, `ASTRA_WORKING_SETUP.md`, `MEMORY_INDEX.md` and `VISUAL_CONTRACT.md` are useful references but must not reopen unrelated platform tasks. The package is intentionally model-independent; a less capable implementation model should execute the specified decisions rather than invent its own aesthetic.

## 3. Initial source audit history (superseded where noted above)

- Inspected Rails routes, admin layout/sidebar, shared tokens/reset/imports, admin/workload CSS, Schedule/Workload/Teachers/Notice/resource/calendar/account views, selected controllers, operations, gateways and Stimulus controllers.
- Inspected API routes, search query contract, schedule update action/contract/lock, assignment replacement contract/action/repository and notification transaction context.
- Read selected prior working/visual handoffs and local specification text. The DOCX has zero native table elements and no embedded media; no explicit table-layout appendix was found. Do not certify schema parity from it.
- Rails source at audit: `cbafbb2a887dd989dbe5b5fc2441cd1efee26c43`; clean before docs edits.
- API source at audit: `0065358a53af0d80917fb9ea400727297b83c181`; substantial pre-existing dirty changes. Preserve all of them, including existing untracked specs; do not add more tests.
- Root has many unrelated dirty/untracked/deleted files. No global add/reset/cleanup.
- Local Rails `/up` responded HTTP 200 with explicit localhost proxy bypass.
- Browser setup returned no available browser; browser list was empty. No authenticated screenshots, computed-style results, live admin mutations or running-revision proof were obtained in this audit.
- W3C dragging and modal-dialog references were consulted for interaction requirements; links are in DESIGN_SYSTEM.md. Modern web guidance was queried; no browser-support claim or speculative containment optimization was adopted from it.
- Final documentation checks passed: all local Markdown links resolve, code fences are balanced, no Unicode replacement characters were found, and scoped Git whitespace checks reported no errors. Rechecked that all four reported undefined CSS tokens have no definitions in the inspected stylesheets. Rails status after writing contains only its AGENTS.md modification; UI source remains unchanged.

The source audit confirms partial CSS and undefined tokens, not literal absence of all styling. If continuing, avoid repeating the earlier preliminary missing-styles interpretation. Determine whether the reported runtime appearance also involves stale or failed assets.

## 4. Initial source findings (read current V01–V08 before acting)

1. `_admin_workspace.css` uses undefined `--admin-display`, `--muted`, `--paper-muted`, `--accent`; generic heading scale differs sharply from Schedule/Workload.
2. Partial resource/account/calendar styles live inside the shared workspace file, with inconsistent gutters, control coverage and row grids.
3. Five-field weekday-transfer rows use a generic three-column grid; several resource header and nested form tracks differ. Preserve schemas while fixing geometry.
4. Workload narrow CSS turns tables into cards; resource/account narrow CSS collapses row tracks; audit narrow CSS hides a column. These conflict with the user's current table-preservation rule.
5. Account edit/calendar fallbacks/weekday transfers/reservation edit expose raw IDs. Replace with readable searches using the correct semantic identity.
6. Accounts and period mutations redirect on failure, losing safe draft fields; several errors are generic/backend symbols. Repair within the relevant page batch.
7. Existing planner drag selects a load and opens a destination link. It does not move events or save directly. Extend staged operation semantics, not optimistic destructive DOM moves.
8. Assignment PUT replaces complete membership/room sets. A dropped item must be unioned with retained selected values, not sent alone.
9. Schedule update outer placement-lock transaction includes notification. Do not “fix” atomicity by misunderstanding its inner transaction.
10. Scope/identity and concurrency guarantees require real contract reading. A lock is not proof of stale-browser-draft rejection.

## 5. Identifier cheat sheet

| UI concept | Identity to retain | Frequent mistake |
| --- | --- | --- |
| Schedule/workload selected group | Student group semester | Sending a base student group ID because the local parameter is named group_id |
| Weekday transfer optional group | Base student group | Reusing planner group-semester ID |
| Teacher account / reservation teacher | Person/teacher ID | Sending department membership |
| Workload or event teacher | teacher_on_kafedra membership ID | Sending a person ID from generic teacher search |
| Existing event | Event ID, with same load requirement | Delete/create or changing requirement to simulate move |
| Room | Room ID, with building/name as display context | Trusting displayed room number as globally unique ID |

Read back omitted/empty/null semantics when editing. Do not clear unloaded relationships. API account paths live under `/auth/accounts`. API period write is POST upsert. Rails route names differ.

## 6. Implementation state

| Batch | Status | Next work |
| --- | --- | --- |
| Planning/docs | Updated with authenticated browser evidence | Use the continuation prompt |
| B00 baseline | Partial: authenticated read-only browser baseline restored; core write/read-back journey pending | Run the safe workload → assignment → placement → edit → reload journey on disposable local data |
| B01 shared system | Focused continuation-only Schedule/Workload changes reverted per user direction; prior runtime evidence retained as historical | Leave Schedule/Workload behavior unchanged; only reopen on a reproduced regression |
| B02 workbench journey | Staging code exists; writes/failures unverified | V03/V04: committed URL, real editor behavior, assignment/placement/edit read-back |
| B03 Resources | Focused UI pass: table-first create disclosure and shared navigation styling served; CRUD read-back pending | Complete editor sheet/relationship UX and verify representative create/edit/failure states |
| B04 Calendar | Focused UI pass: table-first create disclosure, calendar navigation styling and five-column transfer geometry served; reservation dependency still pending | Finish reservation diagnosis and verify calendar page states at narrow widths |
| B05 Accounts | Focused UI pass: create workflow collapsed behind an accessible disclosure; six-column report preserved; row editor contract unchanged | Replace overlapping row editor with the shared real sheet and verify role/search/failure states |
| B06 final QA | Initial Teachers/Notice and two narrow pages inspected only | Normalize adjacent pages; finish widths, keyboard, roles, print and final journey |

## 7. Exact next session opening

1. Read root AGENTS.md and nested Rails AGENTS.md; read BROWSER_AUDIT.md, DESIGN_SYSTEM.md and the B00/B01 plan sections.
2. Capture independent Git status; do not assume the audit HEADs still apply.
3. Read current runtime runbook and verify localhost source/asset provenance. Do not dump env/config/session files or process command lines.
4. Use Playwright MCP and the authorized session if it remains valid; otherwise normal sign-in. The browser worked in this audit. Record only a newly reproduced access/tooling blocker. Do not bypass sign-in by extracting tokens from old artifacts.
5. Complete B00 evidence and core-flow baseline on safe data. If a regression is reproduced, fix only that blocker before styling.
6. Complete B01 on Schedule/Workload using existing primitives and the browser-derived decisions, preserving tables and prior changes. Fix a reproduced core blocker before visual work if B00 exposes one. Capture comparison evidence, update this current state table, then continue B02. Do not jump to a later source-only batch to avoid unfinished acceptance.

If the browser remains unavailable, say exactly which acceptance items are pending. Safe source work can continue but must remain source-complete/unverified. Do not mark B00 or any UI batch runtime-verified from `/up` or Ruby syntax.

## 8. How to keep this handoff useful

Append concise evidence entries using IMPLEMENTATION_PLAN.md's format and update batch status. Record new decisions only if an actual constraint invalidates a prescribed decision; explain the source and narrow deviation. Do not reproduce the whole transcript, add generic reviews or announce the same blocker as progress.

Do not overwrite this evidence history or mark skipped work complete. Final implementation delivery must report changed behavior, checks, remaining visual/runtime limitations and exact documentation links. A later model should be able to identify the next unfinished slice without reopening the full repository history.

## 9. Implementation evidence — 2026-09-13

### Focused UI pass — Resources, Calendar and Accounts

- Scope: implementation was narrowed by the user to the three administrator tabs above; Schedule and Workload continuation-only changes were reverted before this pass.
- Attributable Rails files: `app/views/admin/accounts/index.html.erb`, the four resource index views under `app/views/admin/resources/`, `app/views/admin/semesters/index.html.erb`, `app/views/admin/holidays/index.html.erb`, `app/views/admin/weekday_transfers/index.html.erb`, `app/views/admin/room_reservations/index.html.erb`, and the scoped component stylesheets `_admin_components.css`, `_admin_accounts.css`, `_admin_resources.css`, `_admin_calendar.css`.
- UI behavior: create forms are keyboard-accessible native disclosure sections, so table-first browsing is the default while existing form fields, Turbo frames and endpoints remain in place. Account report columns remain six columns. Calendar weekday transfers retain five columns. Resource inline form fields now participate in their report row tracks; Save/Delete remain in the existing action column.
- Control correction: following the user's explicit direction, visible native dropdowns were removed from the focused Accounts, Calendar and Resources views. Accounts now uses visible radio choice groups for role, status and faculty scope. Calendar uses visible choice groups for faculty, semester, group, weekday, room, teacher and time slot. Resource relationship fields use visible choice groups. Long choice groups scroll inside their own bounded region; values and existing hidden picker targets keep the current request identity.
- Runtime provenance: changed Rails source was copied into the running `schedule_web` container, `bundle exec rails assets:precompile` completed, and the service was restarted. Served fingerprints included `application-d611f14b.css` and `components/_admin_resources-56fd3a50.css`.
- Browser checks: authenticated super-admin session; `/admin/accounts`, `/admin/posts`, `/admin/operations?faculty_id=1&semester_id=8`, `/admin/weekday_transfers?faculty_id=1&semester_id=8`, and `/admin/schedule?faculty_id=1&semester_id=9`; 1280×900 and 390×844. Accounts create disclosure was opened by keyboard-capable native summary interaction; account editor opened as a bounded right sheet with no document overflow. At 390px, Accounts had document width 375px and table scroll width 940px; weekday transfers had document width 375px and table scroll width 860px; both had no page overflow. Posts computed row fields aligned at x=269 and x=695, with actions in the final column. No browser console errors were returned, and the changed CSS assets returned HTTP 200.
- No-dropdown check: the rendered Accounts, Posts and Calendar period routes reported zero `<select>` elements and no document overflow at the tested desktop viewport. Calendar period choice groups were bounded to 176px and the route rendered without console errors.
- Evidence: authenticated browser snapshots are in `.playwright-mcp/page-2026-09-13T13-44-36-900Z.yml`, `.playwright-mcp/page-2026-09-13T13-44-37-809Z.yml`, `.playwright-mcp/page-2026-09-13T13-44-05-292Z.yml`, and `.playwright-mcp/page-2026-09-13T13-42-29-925Z.yml`; the non-sensitive narrow Accounts viewport capture is `admin-accounts-table-first-390.png`.
- Verification limits: no business mutation was performed in this UI pass. The shared real modal sheet, searched Calendar group picker, reservation data dependency, and full create/edit/failure read-back journeys remain pending. `erb` standalone syntax piping is unsuitable for these Rails templates because existing inline ERB control expressions are not valid as raw Ruby; actual Rails rendering of all sampled routes succeeded.
- Next action: replace the Accounts row `details` editor with the existing real modal/dialog shell or a shared admin sheet controller while preserving its search and draft contract; then finish the Calendar reservation diagnosis and representative resource/account failure-state checks.

Batch/slice: B00 baseline / B01 shared system foundation
Status: B00 blocked on supported browser connection; B01 source-complete
Files changed (attributable): `apps/schedule_web/app/assets/stylesheets/application.css`, `app/assets/stylesheets/components/_admin_tokens.css`, `app/assets/stylesheets/components/_admin_components.css`, `app/assets/stylesheets/components/_admin_workspace.css`, `app/assets/stylesheets/components/_admin_workload.css`, `app/views/admin/schedules/show/show.html.erb`, `app/views/admin/workload/show/show.html.erb`
Source revision + dirty-scope identifier: Rails `cbafbb2a887dd989dbe5b5fc2441cd1efee26c43`; root `8b5b7ae143b23cce251a969708b6bd2ef256443c`; API `0065358a53af0d80917fb9ea400727297b83c181`. Rails already had the user-owned `AGENTS.md` modification; only the listed Rails UI files were touched for this slice. Root/API changes predate this slice and were preserved.
Runtime source/asset verification method: Source imports use the existing Propshaft-compatible `@import url(...)` convention. `bin/rails zeitwerk:check` and a Rails runner view-path load check passed. The supported browser client could not be connected because its installed `browser-client.mjs` module was missing; no screenshot, computed-style, asset-network, or authenticated mutation evidence was captured.
Routes, role, safe selected context, viewport: Not runtime-verified. Source targets are admin Schedule and Workload at the existing routes; no role, selected data, or viewport claim is made.
Checks and exact results: `bin/rails zeitwerk:check` passed; Rails runner view-path check exited 0; `git diff --check` reported no whitespace errors; Impeccable detector returned `[]`; no unresolved uses of the previously undefined `--muted`, `--paper-muted`, or `--accent` aliases remain. Schedule table source remains one time column plus the existing weekday columns. Workload remains five columns in the order `Кафедра`, `Дисциплина и группа`, `Вид занятия`, `Часы`, `Назначение`.
Browser/mutation/read-back evidence paths: None available in this session. `/up` liveness is not used as proof of the core journey.
Table inventory comparison: No columns, order, grouping, row meaning, axes, totals, or report semantics changed. Workload narrow presentation now retains the semantic table and exposes a labelled horizontal scrolling region; Schedule builder scrolling is explicitly labelled and keyboard focusable.
Known remaining issue: B00 functional anchor and all visual/runtime evidence remain pending. B02 assignment selection, typed drag staging, placement, and existing event move/edit verification has not started.
Next concrete action: establish a supported browser connection or record the tooling blocker again, then inspect the current B02 contracts and complete the workload assignment slice before broadening to Resources.

Batch/slice: B02a shared entity selection and assignment editor
Status: source-complete; runtime and mutation verification pending
Files changed (attributable): `apps/schedule_web/app/javascript/controllers/assignment_picker_controller.js`, `app/views/admin/workload/show/_assignment.html.erb`, `app/assets/stylesheets/components/_admin_workload.css`
Source revision + dirty-scope identifier: Rails `cbafbb2a887dd989dbe5b5fc2441cd1efee26c43` plus the B01 UI files listed above; API contract/action/repository inspected at `0065358a53af0d80917fb9ea400727297b83c181` with pre-existing dirty changes preserved.
Runtime source/asset verification method: `node --check` passed for the new Stimulus controller; `bin/rails zeitwerk:check` passed; the Impeccable detector returned `[]`. No supported browser client was available for route rendering or interaction evidence.
Routes, role, safe selected context, viewport: Source targets the existing workload assignment route and preserves `teacher_on_kafedra_ids[]` and `room_ids[]`; role/context/viewport remain unverified.
Checks and exact results: Teacher and room options retain their existing checkbox names and selected state. Search filters the complete server-provided option list within the open editor. Typed drag payloads are accepted only for the matching picker kind and only check an existing option. Duplicate drops are harmless. The final form submits all checked teachers and rooms together, matching the API replace-all contract and its up-to-20 validation.
Browser/mutation/read-back evidence paths: None available; no assignment PUT was sent.
Table inventory comparison: No table markup or report semantics changed.
Known remaining issue: This slice has not been proven against a live authenticated assignment save, failure retention, or reload/read-back. B02b placement and B02c existing lesson movement remain source-unimplemented.
Next concrete action: reconnect a supported browser and verify assignment save/failure/search retention, then implement staged workload-to-slot placement through the existing create operation.

Batch/slice: B02b typed workload-to-slot drag guard
Status: source-complete; runtime verification pending
Files changed (attributable): `apps/schedule_web/app/javascript/controllers/planner_drag_controller.js`
Source revision + dirty-scope identifier: Rails `cbafbb2a887dd989dbe5b5fc2441cd1efee26c43` plus the earlier attributable UI files; existing Rails dirty state was preserved.
Runtime source/asset verification method: `node --check app/javascript/controllers/planner_drag_controller.js` passed. No browser connection was available.
Routes, role, safe selected context, viewport: Existing Schedule planner route and authorized administrator context only; runtime details unverified.
Checks and exact results: The controller now writes and accepts only `application/x-load-requirement`; arbitrary external `text/plain` drops are rejected. Click/tap remains available through the existing cell link and the drop still stages `selected_load` through the existing destination flow. No direct save or optimistic DOM move was introduced.
Browser/mutation/read-back evidence paths: None available; no placement mutation was sent.
Table inventory comparison: Schedule time plus weekday columns remain unchanged.
Known remaining issue: Full placement preview/cancel/invalid-target behavior and authoritative read-back remain unverified; existing event-to-cell move semantics are still pending B02c.
Next concrete action: Verify the typed drag and click paths in an authenticated browser, then inspect and implement the existing event move/edit operation without changing event identity.

Batch/slice: B02b/B02c staged schedule drag and existing-event editor
Status: source-complete for typed staging; runtime and failure-retention verification pending
Files changed (attributable): `apps/schedule_web/app/javascript/controllers/planner_drag_controller.js`, `app/views/admin/schedules/show/show.html.erb`, `app/assets/stylesheets/components/_admin_workspace.css`
Source revision + dirty-scope identifier: Rails `cbafbb2a887dd989dbe5b5fc2441cd1efee26c43` plus the earlier attributable UI files; API update action/contract/repository inspected at `0065358a53af0d80917fb9ea400727297b83c181` with existing dirty state preserved.
Runtime source/asset verification method: Both changed JavaScript controllers pass `node --check`; `bin/rails zeitwerk:check` passes; the detector returns `[]`. The browser client remains unavailable, so no authenticated route or mutation was exercised.
Routes, role, safe selected context, viewport: Existing admin Schedule route, authorized administrator context, and existing event PATCH route; runtime role/context/viewport unverified.
Checks and exact results: Event drag now uses `application/x-schedule-event`, opens the existing inspector with target day/time staged from the destination cell, and does not submit on drop. The PATCH form keeps the same event ID and does not expose a load requirement change. Recurring edit remains separate from the dated transfer and atomic swap forms. External text drops remain rejected.
Browser/mutation/read-back evidence paths: None available; no event PATCH or transfer/swap mutation was sent.
Table inventory comparison: Schedule retains the existing time column and weekday columns. No table markup or event relationships were changed.
Known remaining issue: The existing update controller redirects after failure, so safe submitted teacher/room/date values and staged destinations have not yet been proven to survive 422/409/network failure. Two-session stale-draft behavior remains unverified.
Next concrete action: Repair the Schedule failure path to retain safe draft input in the inspector, then move to the shared Resources batch after live browser access is restored or the runtime limitation is recorded again.

Batch/slice: B03 resource navigation and table geometry foundation
Status: source-complete for shared navigation and narrow table geometry; CRUD/editor runtime verification pending
Files changed (attributable): `apps/schedule_web/app/views/admin/resources/_navigation.html.erb`, resource index views, `app/views/admin/student_groups/index.html.erb`, `app/views/admin/student_groups/semesters.html.erb`, `app/assets/stylesheets/components/_admin_resources.css`, `app/assets/stylesheets/components/_admin_workspace.css`, `app/assets/stylesheets/application.css`
Source revision + dirty-scope identifier: Rails `cbafbb2a887dd989dbe5b5fc2441cd1efee26c43`; API `0065358a53af0d80917fb9ea400727297b83c181` before the attributable reservation/account changes below. Existing nested dirty files were preserved.
Runtime source/asset verification method: Rails Zeitwerk check and Ruby syntax checks pass; UI detector returned `[]`. No supported browser is available (`agent.browsers.list()` returned `[]`), so route rendering, loaded asset fingerprints, and CRUD read-back remain unverified.
Routes, role, safe selected context, viewport: Existing super-admin resource routes and student-group routes; role, context, and viewport unverified.
Checks and exact results: Resource and student-group pages now use the same complete navigation partial with active state. Resource cards retain table-first composition. The narrow CSS no longer collapses resource/account rows to a one-column layout; dense content remains horizontally accessible inside the resource table card. No resource table column schemas were changed.
Browser/mutation/read-back evidence paths: None available; no resource mutation was sent.
Table inventory comparison: Existing resource and group-semester table columns remain in place. The group-semester identifier display remains a documented identity issue pending readable catalog projections.
Known remaining issue: Resource create/edit forms are still inline and redirect-on-failure; relationship selectors and group-semester raw identifiers remain to be migrated under the full B03 acceptance gate.
Next concrete action: Repair resource failure re-render and searchable relationship selectors, then verify one complete resource CRUD flow with authoritative reload/read-back.

Batch/slice: B04 reservation identity projection / B05 account identity labels
Status: source-complete for removing visible raw teacher IDs in these editors; searchable edit scopes and runtime verification pending
Files changed (attributable): `apps/api_geteway/slices/admin/relations/room_reservations.rb`, `slices/admin/repos/room_reservation_repo.rb`, `slices/admin/serializers/room_reservation_serializer.rb`, `slices/authentication/serializers/account_serializer.rb`, `slices/authentication/actions/accounts/index.rb`, `apps/schedule_web/app/lib/gateways/admin/accounts_gateway.rb`, `app/views/admin/accounts/index.html.erb`, `app/views/admin/room_reservations/index.html.erb`
Source revision + dirty-scope identifier: API `0065358a53af0d80917fb9ea400727297b83c181` plus existing dirty state; Rails `cbafbb2a887dd989dbe5b5fc2441cd1efee26c43` plus the attributable UI changes. No tests were added.
Runtime source/asset verification method: API and Rails Ruby syntax checks pass; Rails Zeitwerk check passes; UI detector returned `[]`. Browser availability remains empty.
Routes, role, safe selected context, viewport: Existing account and room-reservation admin routes; role/context/viewport unverified.
Checks and exact results: Account list serialization now carries a safe teacher name projection and the edit view uses it with a hidden teacher identity. Existing scope identities are retained in hidden fields while labels are shown. Reservation edit no longer presents a numeric teacher input; it uses the searched labelled options and retains the current person selection. Teacher account/reservation identity remains person ID; workload/event identity remains teacher-on-kafedra membership ID.
Browser/mutation/read-back evidence paths: None available; no account or reservation mutation was sent.
Table inventory comparison: Account six-column row meaning and reservation columns remain unchanged.
Known remaining issue: Account edit scope additions still lack a searched selector, reservation edit still needs a live searched replacement for changing teacher, and redirect-on-failure draft retention is not fixed. API projection behavior is syntax-checked but not runtime-read back.
Next concrete action: Implement searched edit selectors with retained current values, then verify account and reservation failure/conflict/reload behavior in an authenticated browser.

Batch/slice: Compose runtime provenance and compatibility correction
Status: affected services rebuilt and healthy; authenticated browser verification remains blocked
Files changed (attributable): `apps/schedule_web/app/views/admin/accounts/index.html.erb`, `apps/api_geteway/slices/authentication/actions/accounts/index.rb`
Runtime source/asset verification method: The project runs from the root `docker-compose.yml`. `schedule_web` and `api_geteway` have no application bind mounts, so both images were rebuilt and recreated. Propshaft precompiled the new admin CSS and Stimulus bundles. Host and container SHA-256 values matched for the inspected Rails and API files. `docker compose ps` reported both services healthy; `GET http://localhost:3000/up` returned 200.
Checks and exact results: The first rebuilt run exposed `Array#compact_blank` incompatibility in the Hanami account projection and the account teacher search template. Both uses were replaced with Ruby-compatible `compact.join(...).strip` handling; API `ruby -c` passed, the API image was rebuilt, and the service was recreated. Subsequent unauthenticated requests to the account and reservation routes reached Rails and returned the expected authentication redirect; no new API 500 appeared in the post-fix log window.
Known remaining issue: The supported in-app browser is unavailable (`agent.browsers.list()` returned `[]`), and direct HTTP checks lack the authorized administrator session. No authenticated visual, mutation, or read-back acceptance evidence is claimed. The stack contains pre-existing observability orphan warnings and remains running.
Next concrete action: Restore an authorized browser session, verify the full core journey and failure retention, then complete searched edit scopes and remaining B03–B06 source slices.

Follow-up build note: After correcting the Rails template's second `compact_blank` use, `docker compose build schedule_web` and a direct `docker build --pull=false` retry both stopped at Docker Hub `ruby:3.4.2` metadata resolution timeout before executing build steps. The source fix is present in the Rails worktree but is not yet in the running image; retry the image build when registry access is available, then recreate `schedule_web`.

Resolution: Registry-free rebuild from the existing `milk_schedule-schedule_web:latest` image completed successfully. The temporary Dockerfile was removed, `schedule_web` was recreated, and host/container hashes matched for the changed account and reservation controllers/templates. `GET http://localhost:3000/up` returned 200 and Compose reports `schedule_web` healthy.

Batch/slice: B05 account edit search and retained scope selection
Status: source-complete for edit search staging; runtime interaction and mutation verification pending
Files changed (attributable): `apps/schedule_web/app/controllers/admin/accounts_controller.rb`, `app/views/admin/accounts/index.html.erb`, `app/javascript/controllers/account_scope_picker_controller.js`, `app/assets/stylesheets/components/_admin_accounts.css`
Source revision + dirty-scope identifier: Rails `cbafbb2a887dd989dbe5b5fc2441cd1efee26c43` plus the attributable changes above; pre-existing nested dirty files preserved.
Runtime source/asset verification method: The current Rails source was copied into a registry-free image derived from the existing `milk_schedule-schedule_web:latest`; Propshaft precompile completed, the service was recreated, host/container hashes matched for the changed files, and Compose reports `schedule_web` healthy.
Routes, role, safe selected context, viewport: Existing `/admin/accounts` route; role, selected account and viewport remain unverified without an authorized browser session.
Checks and exact results: Account edit now has isolated teacher and department search frames that preserve list filters. Teacher selection updates the existing hidden person identity. Scope selection appends a deduplicated `faculty_id` or `kafedra_id` hidden input to the same PATCH collection, preserving existing scopes. No nested search form remains inside the PATCH form. `node --check`, Ruby syntax, Rails Zeitwerk and `git diff --check` passed.
Browser/mutation/read-back evidence paths: None; direct unauthenticated HTTP only proves the liveness endpoint and cannot exercise the protected editor.
Table inventory comparison: The six account columns and their order remain unchanged.
Known remaining issue: Search result click, scope deduplication, failed PATCH re-render and authoritative account read-back need authenticated browser verification. Faculty scope additions still use the same picker infrastructure but currently require the existing short faculty control where available.
Next concrete action: Restore supported browser access and verify the account search/edit flow, then continue B04 calendar searchable reservation editing and B06 final consistency checks.

Batch/slice: B04 reservation room search
Status: source-complete for create-side room search; edit-side search and runtime interaction verification pending
Files changed (attributable): `apps/schedule_web/app/lib/gateways/admin/teachers_workspace_gateway.rb`, `app/controllers/admin/room_reservations_controller.rb`, `app/views/admin/room_reservations/index.html.erb`, `app/javascript/controllers/reservation_room_picker_controller.js`
Source revision + dirty-scope identifier: Rails `cbafbb2a887dd989dbe5b5fc2441cd1efee26c43`; API room search action/route/repository already existed in the preserved API worktree. No unrelated files were staged or reset.
Runtime source/asset verification method: Rails was rebuilt from the current local application image, Propshaft precompile completed, and `schedule_web` was recreated through Compose. Ruby/JS syntax and Rails Zeitwerk passed; the service reports healthy.
Routes, role, safe selected context, viewport: Existing `/admin/room_reservations` route, super-admin-only behavior and current calendar context; role and viewport remain unverified in the unavailable browser.
Checks and exact results: The gateway calls the existing authorized `GET /admin/search/rooms?q=...` contract. Search results are shown in a labelled frame and selecting one stages the existing room select; no write is sent. The native room list remains available. Table columns remain unchanged.
Browser/mutation/read-back evidence paths: None; direct requests lack an authorized session and no reservation mutation was sent.
Table inventory comparison: Reservation table retains its existing five columns and order.
Known remaining issue: Reservation edit rows still need the same searched room/person replacement flow; authenticated conflict/read-back and narrow layout verification remain pending.
Next concrete action: Add isolated edit-row room/person search while preserving the original row, then verify the complete Calendar acceptance matrix in an authorized browser.

Batch/slice: B04 reservation edit search
Status: source-complete; runtime endpoint/provenance verified; authenticated interaction and mutation verification pending
Files changed (attributable): `apps/schedule_web/app/controllers/admin/room_reservations_controller.rb`, `app/views/admin/room_reservations/index.html.erb`, `app/javascript/controllers/reservation_teacher_picker_controller.js`, `app/javascript/controllers/reservation_room_picker_controller.js`, `app/assets/stylesheets/components/_admin_calendar.css`
Source revision + dirty-scope identifier: Rails `cbafbb2a887dd989dbe5b5fc2441cd1efee26c43` plus the attributable files above; API route/action/repository were pre-existing preserved worktree changes.
Runtime source/asset verification method: The current Rails image was rebuilt from the local running image base with Propshaft precompile, the temporary build file was removed, and `schedule_web` was recreated via Compose. Container hashes matched the current Rails source. The existing API `GET /admin/search/rooms?q=abc` route was reached inside `api_geteway` and returned 401 without an authorization token, confirming route reachability without exposing credentials.
Routes, role, safe selected context, viewport: Existing `/admin/room_reservations`; edit-row search retains room/date filters and uses the existing super-admin route guard. Role and viewport remain unverified in the unavailable browser.
Checks and exact results: Each reservation row now groups room/person search inside a disclosure, keeping the existing table row geometry and five-column schema. Results are isolated by reservation ID; choosing a room or person updates only that row's existing PATCH select and does not submit. Ruby/JS syntax, Rails Zeitwerk, undefined-admin-token scan and `git diff --check` passed. Compose reports `schedule_web` and `api_geteway` healthy; `/up` returned 200.
Browser/mutation/read-back evidence paths: None; no credentialed mutation was sent and no authenticated browser session is available.
Table inventory comparison: Reservation table remains five columns in the existing order; search tools are contained in the existing action area disclosure.
Known remaining issue: Full Calendar acceptance still needs authenticated search selection, conflict retention, successful update, authoritative reload/read-back, and narrow/responsive inspection. B03 resource CRUD and B06 final verification remain unfinished.
Next concrete action: Reconnect a supported authorized browser, run the core workload-to-schedule journey and Calendar mutation matrix, then close the remaining B03/B06 gates.

Batch/slice: B04/B05 safe failure drafts
Status: source-complete for account and reservation create/update re-render paths; runtime verification pending
Files changed (attributable): `apps/schedule_web/app/controllers/admin/accounts_controller.rb`, `app/controllers/admin/room_reservations_controller.rb`, `app/views/admin/accounts/index.html.erb`, `app/views/admin/room_reservations/index.html.erb`
Source revision + dirty-scope identifier: Rails `cbafbb2a887dd989dbe5b5fc2441cd1efee26c43` plus the attributable UI/API changes already listed; unrelated nested dirty files preserved.
Runtime source/asset verification method: Source-only after the Compose image rebuild was blocked by registry metadata timeout. No mutation was sent. Password and generated-password fields are excluded from account drafts.
Routes, role, safe selected context, viewport: Existing `/admin/accounts` and `/admin/room_reservations` routes; administrator role, context and viewport remain unverified without the supported browser session.
Checks and exact results: Account create/update failures now re-render the existing list with safe email, role, status, teacher identity and scope values retained; reservation create/update failures re-render the same page with submitted room, teacher, date, time slot and purpose values, while the original row remains authoritative for an update failure. Russian failure messages distinguish conflict, forbidden and validation outcomes. Rails controller syntax passes and no `compact_blank` use remains in the touched account path.
Browser/mutation/read-back evidence paths: None; the new Rails image could not be rebuilt after the final template change, and browser discovery remains empty.
Table inventory comparison: Account six columns and reservation five columns remain unchanged.
Known remaining issue: Re-rendered drafts need authenticated 422/409/network verification. Account edit scope additions and reservation teacher replacement are still not fully searchable in the edit forms.
Next concrete action: Rebuild `schedule_web` when Docker registry access recovers, then verify these failure states and implement searched edit selectors with preserved existing values.

Batch/slice: B00 baseline / B01 continuation fixes
Status: source-complete; authenticated runtime and mutation verification pending
Files changed (attributable): `apps/schedule_web/app/views/admin/schedules/show/show.html.erb`, `app/views/admin/weekday_transfers/index.html.erb`, `app/assets/stylesheets/components/_admin_components.css`, `app/assets/stylesheets/components/_admin_workspace.css`, `app/assets/stylesheets/components/_admin_calendar.css`, `app/assets/stylesheets/components/_admin_accounts.css`
Source revision + dirty-scope identifier: Rails current dirty worktree on branch `main`; all listed files already contained pre-existing user changes, and this continuation preserved those changes while adding only the scoped history, control, heading, transfer-grid and account-sheet adjustments.
Runtime source/asset verification method: Current host source was inspected; the supported Playwright browser reached `http://localhost:3000` and `/up` returned 200. The protected admin route redirected to the public sign-in dialog. No credentials, cookies or old auth artifacts were inspected.
Routes, role, safe selected context, viewport: Public `/up` and protected `/admin/schedule?faculty_id=1&semester_id=9`; authenticated role, selected context and target viewports remain pending because the current browser session is unauthenticated.
Checks and exact results: Ruby syntax passed for touched controllers; `node --check` passed for the touched Stimulus controllers; Rails loading command exited 0; `git diff --check` found no whitespace errors; Impeccable detector returned `[]`. Schedule now uses Turbo history advancement on its shell frame, the heading is 24/32 desktop and 20/28 narrow, admin form controls inherit the 40px role, weekday transfer rows declare five tracks, and the Accounts editor is bounded as a full-height sheet. No table columns or order were changed.
Browser/mutation/read-back evidence paths: Browser snapshot is in the session output at `.playwright-mcp/page-2026-09-13T12-38-39-214Z.yml`; no authenticated screenshot or mutation/read-back evidence exists for this continuation.
Table inventory comparison: Schedule time plus weekday columns, Workload five columns and Accounts six columns remain unchanged; weekday transfer content now has five layout tracks including actions.
Known remaining issue: B00 core workload → assignment → placement → edit → reload journey, V03 URL/reload proof, computed-size acceptance, real modal semantics, resource table-form alignment, reservation availability, and full B03–B06 browser gates remain unfinished.
Next concrete action: Resume with an authorized authenticated browser session, capture B00 baseline and run the safe core journey before accepting B01 or starting further UI batches.

Batch/slice: B00/B01 authenticated runtime continuation
Status: runtime-verified for Schedule context history and compact shared roles; core mutation and populated narrow-table gates pending
Files changed (attributable): `apps/schedule_web/app/views/admin/schedules/show/show.html.erb`, `app/views/admin/workload/show/show.html.erb`, `app/assets/stylesheets/components/_admin_workspace.css`, `app/assets/stylesheets/components/_admin_workload.css`, plus the scoped B01 files listed in the preceding entry.
Source revision + dirty-scope identifier: Rails current dirty worktree on branch `main`; runtime was updated by documented registry-free file copy and asset precompile because `docker compose build schedule_web` could not resolve `ruby:3.4.2` metadata from Docker Hub.
Runtime source/asset verification method: `schedule_web` was restarted after precompilation. Served assets returned 200 with current fingerprints `application-b5c9d21a.css`, `_admin_workspace-854a1010.css` and `_admin_workload-818ab648.css`; the container reported healthy.
Routes, role, safe selected context, viewport: Authenticated super-admin, `/admin/schedule?semester_id=9&faculty_id=1&groups[]=789` and `/admin/workload?faculty_id=1&semester_id=9`; Schedule desktop browser viewport was 914×976 after login and Workload was also checked at 390px wide.
Checks and exact results: Selecting `МК-3` changed the browser URL to include `groups[]=789`; a fresh navigation to that URL restored the checked group and grid. Computed Schedule heading was 24px/32px, committed context height 65px and first grid top 362px. Workload heading was 24px/32px, committed context height 138px and results top 376px. Schedule context inputs/buttons measured 40px. No console errors were reported. Current semester 9 has no workload rows, so populated table geometry was not claimed.
Browser/mutation/read-back evidence paths: `.playwright-mcp/page-2026-09-13T13-23-14-294Z.yml`, `.playwright-mcp/page-2026-09-13T13-25-39-090Z.yml`; no mutation or authoritative write read-back evidence yet.
Table inventory comparison: Schedule retains one time column plus the existing weekday columns. Workload table schema was untouched; the five-column narrow table remains pending populated-data verification.
Known remaining issue: B00 mutation anchor is still pending; B02 assignment/placement/edit operations and B03–B06 acceptance remain unfinished. The host curl probe returned 502 while the container healthcheck reported healthy, so liveness is based on browser 200 responses and container health rather than that probe.
Next concrete action: Use an older populated semester/faculty scope to verify Workload’s five-column scroll region, then complete the safe write/read-back journey before proceeding to B02.

Batch/slice: Focused UI/UX pass - Accounts, Calendar and Resources
Status: source and authenticated visual runtime checks completed for the focused layout pass; mutation and real dialog semantics remain pending
Files changed (attributable): focused admin tokens/components/resources/calendar/accounts styles, focused administrator ERB views, and the focused Stimulus picker/controller files.
Runtime source/asset verification method: Current Rails source was copied into the running schedule_web container, Propshaft assets were precompiled, and the service was restarted through Docker Compose. Served CSS fingerprint: application-5766d961.css; the container remained healthy.
Routes, role, context, viewport: Authenticated administrator session; /admin/accounts, /admin/posts, /admin/operations, and /admin/student_groups; desktop 1440x900 and narrow 390x844.
Checks and exact results: Focused pages use visible radio/list choices and no rendered select elements on the checked routes. At desktop, document width matched the viewport (1425/1425 with the browser content area) and the main workbench/header expanded across the available canvas; Accounts header width was 1153px. At 390px, all checked pages had document width equal to content width (375/375) with no page-level overflow, closed create/edit disclosures, and zero fresh console errors. Account edit role selection changed the retained hidden role from scheduler to super_admin with no controller errors. node --check, Rails loading, git diff --check, and the Impeccable detector passed. No mutation was sent.
Browser/mutation/read-back evidence paths: Browser session snapshots are in the Playwright session output for the authenticated checks; screenshot capture was attempted but did not complete, so no screenshot is claimed as evidence. No write/read-back evidence exists for this slice.
Table inventory comparison: Existing account, reservation, resource and group-semester table structures were preserved; no columns, order, headers or row meaning were changed. Dense content remains locally scrollable where the existing table requires it.
Known remaining issue: The account/resource editors use the existing disclosure-based Rails forms and still need conversion to a true accessible dialog/sheet with focus containment, inert background, Escape and dirty-close handling. Room reservations still renders the unavailable-data message in the current runtime, so reservation picker behavior is not verified. Raw numeric fallback fields in calendar settings/weekday transfers and full searchable catalog behavior remain pending. No CRUD mutation or authoritative reload/read-back has been claimed.
Next concrete action: Implement the shared real sheet semantics for Accounts/Resources, then trace and repair the reservation data dependency before performing authenticated failure, mutation and read-back checks.


Batch/slice: Buildings and Room Types content recovery plus Calendar scale
Status: source-complete; runtime redeployment and authenticated visual verification pending because Docker Desktop Linux engine is currently unavailable
Files changed (attributable): apps/schedule_web/app/views/admin/resources/buildings/index.html.erb, app/views/admin/resources/room_types/index.html.erb, app/assets/stylesheets/components/_admin_calendar.css, and the calendar page view class markers.
Source/runtime finding: Buildings and Room Types previously suppressed their entire table when the companion building-type or activity-type catalog failed. The page now keeps the primary rows visible, shows stored relationship names, and withholds only catalog-dependent create/edit controls until the companion list is available. Calendar controls now use larger 44px choices, 14px labels, a wider two-column context layout, and four period cards across desktop.
Checks: JavaScript syntax and git diff --check pass. Host HTTP and browser runtime could not be rechecked after the local Docker Linux engine stopped; no runtime success is claimed for this slice.
Remaining issue: Docker Desktop service cannot currently expose the dockerDesktopLinuxEngine named pipe, so the modified views/styles are not yet redeployed after the latest source edits. Restart the local engine, copy the focused files, precompile assets, and verify /admin/buildings, /admin/room_types and /admin/operations at desktop and narrow widths.


Runtime verification update: API gateway was found stopped and restarted through Docker Compose. After fixing the Hash-versus-object access in the live templates (building/activity catalog entries now use [:id] and [:name]) and restarting schedule_web, authenticated browser checks succeeded. /admin/buildings rendered 19 resource rows and /admin/room_types rendered 17 rows at desktop and narrow widths. Both routes reported zero fresh console errors and no document overflow. Calendar /admin/operations reported 44px navigation at 1440px and 390px with no overflow. Resources navigation now also measures 44px with 14px text. The pages expose one closed native dialog for create actions. No mutation was sent.


Calendar sheet runtime update: Migrated create actions for Semesters, Holidays, Weekday Transfers and Room Reservations to the shared native dialog sheet controller, preserving existing form fields and endpoints. Authenticated browser verification at 390px confirmed Semesters and Weekday Transfers render one closed dialog, zero visible select controls, 44px calendar navigation, no document overflow and zero fresh console errors. Opening the Weekday Transfer sheet produced a native dialog with a backdrop and initial focus on the close control. Docker Compose reports api_geteway and schedule_web healthy. No mutation was sent.


Resource editor geometry update: Converted the remaining resource row editors for posts, buildings, room types and rooms into shared native dialog sheets. Visible table values remain in their original columns; edit and delete actions now share one compact action cell, so row height is 61px in the checked narrow and desktop renders instead of expanding around inline forms. The buildings and room types tables continue to render stored relation labels even when companion catalogs are unavailable.

Files changed (attributable): `apps/schedule_web/app/views/admin/resources/posts/index.html.erb`, `buildings/index.html.erb`, `room_types/index.html.erb`, `rooms/index.html.erb`, and `app/assets/stylesheets/components/_admin_resources.css`.
Runtime source/asset verification: Files were copied into `schedule_web`, Propshaft was precompiled, and the service was restarted with Docker Compose. Current service state is healthy; current application asset fingerprint is `application-3da01bda.css`.
Routes, role, viewports: Authenticated administrator browser session; `/admin/buildings`, `/admin/room_types`, `/admin/rooms`, `/admin/posts` at 390px and `/admin/rooms` at 1440px. No mutation was sent.
Checks and exact results: Buildings rows measured 61px at 390px, 375px document width matched the client width, and the first edit sheet opened as a native dialog with `rgba(21, 19, 17, .34)` backdrop and focus on `������� ��������`. Room Types rendered 16 rows, zero visible selects, no overflow and zero fresh console errors. Posts rendered 24 row edit dialogs, retained name/short-name values and had zero visible selects, no overflow and zero fresh console errors. Rooms retained all four columns; first rows measured 61px at 1440px with document/client width 1425px and zero fresh console errors.
Table comparison: Resource table columns, order, headers and row meaning remain unchanged. Only the action cell composition changed from inline form fields to a shared editor sheet.
Remaining issue: CRUD success, validation/conflict retention, searchable relationship selection and authenticated read-back are still pending. Calendar at 390px currently uses 44px navigation controls and wraps to 148px of content-driven navigation height; this is documented as a narrow navigation exception caused by keeping all destinations visible.
Next concrete action: Continue Calendar and Accounts runtime acceptance, then run the required failure/read-back and responsive checks without altering Schedule or Workload.

Calendar control scale correction: The period context submit control was still receiving a legacy 36px/10px rule despite the page-level scale. Added a scoped override so the primary `�������� �������` action is 44px with 14px text, matching the Calendar touch target and the other focused controls.

Runtime verification: After copying `_admin_calendar.css`, precompiling Propshaft and restarting `schedule_web`, authenticated `/admin/operations` at 390px measured the context submit at 44px with 14px text. Document and client widths both measured 375px; no page overflow and zero fresh console errors were observed. The five Calendar navigation destinations remain visible and wrap to a 148px content-driven block at this width.

Remaining issue: Calendar and Accounts mutation/read-back and full accessibility/failure-state verification are still pending. The current pass has not changed Schedule or Workload.

Accounts runtime spot check: At authenticated 390px, `/admin/accounts` rendered without page overflow and without visible select controls. Opening `������� ������������` produced a native modal with the shared backdrop and initial focus on `������� ��������`; Escape closed it and returned focus to the create trigger. No fresh console errors were reported. This verifies sheet mechanics and responsive geometry only; account CRUD, validation and authorization remain pending.

Calendar entity fallback correction: Weekday Transfers now loads the existing student group catalog through the Rails gateway and presents visible group radio choices with `��� ������`; when the catalog is unavailable it explains that the transfer will apply to all groups instead of exposing a raw numeric ID field. Period Settings similarly presents an unavailable-catalog message instead of raw faculty or semester ID inputs.

Runtime verification: Ruby controller and ERB syntax passed. After copying the controller/views and restarting `schedule_web`, authenticated `/admin/weekday_transfers` at 390px rendered the group choices as radios, zero selects, one closed native create dialog, document/client width 375px and no overflow. Zero fresh console errors were observed.

Remaining issue: The visible group catalog is bounded inside the editor but still needs search for very large catalogs; CRUD read-back, invalid-date retention and full Calendar role/keyboard coverage remain pending.

Weekday Transfer catalog usability update: Added `choice_filter_controller.js` and a 40px local search field to the visible bounded group-choice list. The list remains radio-based, retains `��� ������`, and reports an explicit empty result state without introducing a dropdown or raw ID field.

Runtime verification: After correct Docker Compose deployment, authenticated `/admin/weekday_transfers` at 390px opened the native create sheet. The group search measured 40px; 101 visible options were filtered to 0 for a no-match query and the empty-state message became visible. The sheet retained zero selects, no document overflow and zero fresh console errors. Rails/ERB/Node syntax checks passed. Served assets included `controllers/choice_filter_controller-44c8b707.js` and `application-73804223.css`.

Remaining issue: Search currently filters the bounded catalog already loaded by the existing gateway; a server-backed full-catalog search is still outside the current API contract. CRUD/read-back and validation/conflict coverage remain pending.

Accounts sheet structure update: Wrapped create and edit content in the shared `admin-sheet__body` layout and applied a consistent 16px internal rhythm with 44px submit controls. Long account forms now scroll inside the sheet instead of bypassing the common editor geometry; table and account fields remain unchanged.

Runtime verification: After deployment, authenticated `/admin/accounts` at 390px opened the create sheet with a 664px scroll viewport and 949px content height, confirming internal scrolling. Initial focus remained on `������� ��������`; the page stayed at 375px document/client width with no overflow and zero fresh console errors. ERB syntax passed.

Remaining issue: Account mutation, validation/conflict retention, authorization and full search/read-back verification remain pending.

Desktop Calendar context composition: Committed period context now collapses into a readable summary showing faculty, semester and group, with an explicit Change context disclosure for the full radio selection form. This removes the always-expanded context wall while preserving all existing choices and URL parameters.

Runtime verification: After deployment, authenticated /admin/operations?faculty_id=1&semester_id=9&group_id=789 at 1440px showed the context summary at 56px and the four period cards beginning at y=352px. Opening the disclosure restored the full 535px selection form, with no document overflow and zero fresh console errors. The compact summary text rendered the selected faculty, semester and group correctly.

Remaining issue: Calendar CRUD success/read-back and conflict handling remain pending. Narrow behavior is deprioritized per the latest user direction; Schedule and Workload were not changed.

Desktop Accounts filter composition: On the PC layout, role and status choices now remain visible as horizontally scrollable choice strips rather than wrapping into a tall block. No option is hidden; the local strip preserves readable labels and keyboard reachability.

Runtime verification: At authenticated 1440px, the Accounts filter reduced from 281px to 114px and the six-column table moved from y=540px to y=374px. Document/client width remained 1425px with no overflow and zero fresh console errors. No dropdowns or table changes were introduced.

Desktop primary action scale: Standardized the focused Accounts, Resources and Calendar create actions to the 44px primary control role with 14px text. Existing table actions and schemas are unchanged.

Runtime verification: Authenticated 1440px checks measured ������� ������������, �������� ��������� and �������� ��������� ���� at 44px with 14px text. Document/client widths remained equal on Accounts, Posts and Holidays. The reservation route currently has no create action in its rendered unavailable-data state, so that action remains pending until the reservation data dependency is resolved.

Reservation runtime and desktop editor update: The previously unavailable reservation page was traced to `slices/admin/serializers/room_reservation_serializer.rb`, where `compact_blank` raised on the API runtime. The serializer now builds the teacher label with supported Ruby operations. The API syntax check passed, the API file was copied into the Compose container, and `api_geteway` was restarted. The authenticated reservation page now renders five rows and its existing five-column table.

Files changed (attributable): `apps/api_geteway/slices/admin/serializers/room_reservation_serializer.rb`, `apps/schedule_web/app/views/admin/room_reservations/index.html.erb`, and `apps/schedule_web/app/assets/stylesheets/components/_admin_calendar.css`.

Runtime verification: After copying the Rails template/style into `schedule_web`, precompiling Propshaft and restarting through Docker Compose, authenticated `/admin/room_reservations` at 1440x900 rendered the table with five rows, no page overflow (1425/1425 content/client width) and zero fresh console errors. Each row measured 64px. The edit action opened a real native dialog sheet at x=841, width=560px, with the PATCH form inside the sheet, one-column field flow and a 44px Save control. The viewport screenshot is `admin-reservations-desktop-20260913.png` in the browser evidence output; the page snapshot is `.playwright-mcp/page-2026-09-13T18-37-20-853Z.yml`.

Table comparison: The five reservation columns and their order remain `Аудитория`, `Дата / слот`, `Преподаватель`, `Назначение`, `Действия`. Visible row values are separate from the action cell; edit/search/delete controls no longer expand the table row with the full form.

Focused desktop verification: At authenticated 1440px, `/admin/buildings` rendered 18 rows, `/admin/room_types` 16 rows, `/admin/accounts` 9 rows, and `/admin/operations` rendered the Calendar heading. Each checked page reported document width equal to client width and zero visible select elements. No fresh console errors were observed.

Remaining issue: Reservation CRUD success, validation/conflict retention and authoritative write read-back remain pending. Accounts and resource CRUD/read-back, and the broader Calendar mutation gates, also remain pending. The latest user direction makes desktop the implementation priority; Schedule and Workload were not changed in this slice.

Next concrete action: Continue the desktop UI/UX pass on the three requested workspaces, then perform the permitted existing mutation and read-back checks before marking any batch complete.
Shared dirty-editor behavior update: Replaced the browser two-choice confirmation in `admin_sheet_controller.js` with an in-sheet accessible alert dialog offering `Сохранить`, `Отменить изменения`, and `Продолжить редактирование`. Save submits the existing non-GET form, discard closes the sheet and clears the draft marker, and continue returns focus to the editor. Added scoped visual treatment in `_admin_components.css`.

Runtime verification: After copying the controller and stylesheet into `schedule_web`, precompiling Propshaft and restarting via Docker Compose, authenticated `/admin/accounts` at 1440px opened the create sheet, accepted a disposable draft email, and Escape displayed all three dirty actions. The prompt occupied the sheet without a nested native dialog. Discard closed the sheet, removed the prompt, and returned focus to `Создать пользователя`. The password field reported `autocomplete="new-password"` and no value. Zero fresh console errors were reported. Served assets included `controllers/admin_sheet_controller-9201fe44.js` and `application-7ac85af3.css`.

Remaining issue: Full mutation success, validation/conflict retention, search persistence while dirty, and authoritative read-back remain pending for the focused workspaces. This behavior was verified on Accounts; Calendar and Resources share the controller but still need equivalent dirty-form runtime checks.
Dirty-sheet cross-page verification: On authenticated `/admin/buildings` at 1440px, the first resource edit sheet accepted a disposable unsaved number change, Escape opened the shared three-action prompt, and `Отменить изменения` closed the sheet, returned focus to the row `Изменить` action, and left the authoritative row value unchanged. This confirms the shared dirty-close behavior also works for Resources. No business mutation was sent.
Calendar choice-label update: Weekday Transfer group choices were technically visible but all rendered as the same generic label. The view now uses the available speciality/group/year/course context to produce readable labels such as `МТ · 2022`, while retaining the radio identity and local search. Transfer table rows resolve selected group IDs through the same visible label map and show a neutral fallback instead of exposing a numeric ID.

Runtime verification: After copying the view into `schedule_web` and restarting Docker Compose, authenticated `/admin/weekday_transfers` at 1440px opened the create sheet with 101 group choices; the first choices were distinct (`Все группы`, `МТ · 2022`, `МТ · 2021`, `МВ · 2021`, `МО · 2021`). Zero generic `Учебная группа` labels remained, document/client width was 1440/1440, and no fresh console errors were reported. No business mutation was sent.

Remaining issue: The catalog still uses the existing bounded 100-item gateway result; server-backed pagination/search is outside the current contract and remains pending. Calendar CRUD read-back and failure/conflict verification remain pending.
Student Groups resource-shell update: Moved the permanent create form behind the shared `admin-sheet` action and moved each inline speciality/year editor into a native sheet. The three-column table remains table-first with visible speciality/year values and a compact action cell containing the semester link, edit action and delete action. Existing create/update/delete/semester routes are unchanged.

Runtime verification: After copying the view and `_admin_resources.css` into `schedule_web`, precompiling assets and restarting Docker Compose, authenticated `/admin/student_groups` at 1440px rendered 50 rows and 51 closed dialogs, with no visible select controls and document/client width 1425/1425. The first edit sheet measured 560px wide, used one form column, focused `Закрыть редактор`, and Escape returned focus to the row edit trigger. No fresh console errors were reported and no business mutation was sent.

Remaining issue: Student group CRUD success/read-back and companion group-semester editor verification remain pending. The desktop layout is the active priority per the latest user direction.
Group-semester editor update: The linked group-semester page now uses the shared sheet for parameter edits. Its five-column table keeps the identifier, semester, student count, parameters and actions visible, while the existing PATCH endpoint and Calendar link remain intact. Product labels were translated to Russian where the old `group-semester` wording was exposed.

Runtime verification: Authenticated `/admin/student_groups/1/semesters` at 1440px rendered eight rows and eight closed editor dialogs with zero visible select controls and document/client width 1425/1425. The first editor opened as a 560px native sheet with a one-column form, initial focus on `Закрыть редактор`, and zero fresh console errors. No business mutation was sent.

Remaining issue: Group-semester PATCH success/read-back and error retention remain pending.
Desktop route sweep: Authenticated Playwright navigation checked all 12 Resources routes (`posts`, `subjects`, `academic_titles`, `academic_degrees`, `activity_types`, `time_slots`, `week_types`, `building_types`, `buildings`, `rooms`, `room_types`, `student_groups`) plus Calendar `semesters`, `holidays`, `weekday_transfers`, and `room_reservations` at 1440px. Every route rendered a Russian heading and current content, had zero visible select elements, zero page-level horizontal overflow, and zero fresh console errors. Rendered row counts were 23, 50, 8, 39, 26, 7, 8, 10, 18, 37, 16, 50, 19, 2, 1, and 5 respectively. No business mutations were sent.

Remaining issue: These route checks establish desktop rendering and composition only. CRUD success, validation/conflict recovery, authorization boundaries, and authoritative read-back are still pending.
Shared sheet body normalization: Converted remaining Calendar and Resource create dialogs from the legacy disclosure body wrapper to `admin-sheet__body`, so create and edit sheets now share the same padding, internal scroll and form rhythm. This affected Buildings, Room Types, Rooms, Posts, Holidays, Semesters, Weekday Transfers and Room Reservations; endpoints and fields were unchanged.

Runtime verification: After explicit Docker Compose copies (the first PowerShell copy loop used incorrect root-relative paths and was corrected) and a schedule_web restart, authenticated desktop checks for Buildings, Holidays, Weekday Transfers and Room Reservations found zero legacy dialog body wrappers, shared sheet bodies on every dialog, no document overflow, and no fresh console errors. The `/admin/semesters` path was not a valid rendered target in this check; the established `/admin/operations/semesters` route remains the valid Calendar route.

Remaining issue: Full mutation and read-back gates remain pending. No responsive implementation work was added; desktop remains the active priority.
Corrected Calendar route verification: The valid semester route `/admin/operations/semesters` was rechecked after deployment. At 1440px it rendered `Учебные семестры`, 19 table rows, one shared sheet body, zero legacy dialog body wrappers, document/client width 1425/1425, and no fresh console errors. The earlier `/admin/semesters` probe was only a wrong route probe and is not treated as a runtime defect.

Current implementation status: Desktop visual composition and shared sheet mechanics are verified across the requested Accounts, Calendar and Resources workspaces. Business mutation/read-back, validation/conflict recovery and authorization role coverage remain open acceptance gates.
Final desktop visual evidence capture: Saved viewport screenshots for `/admin/accounts`, `/admin/operations/semesters`, and `/admin/buildings` as `admin-accounts-desktop-final-20260913.png`, `admin-calendar-semesters-desktop-final-20260913.png`, and `admin-resources-buildings-desktop-final-20260913.png` in the browser evidence output. These captures contain no passwords or one-time credentials.

Verification checks: `node --check app/javascript/controllers/admin_sheet_controller.js`, ERB compilation for Student Groups and group-semester templates, and scoped `git diff --check` passed. Compose reports `schedule_web` and `api_geteway` healthy.
Controlled resource mutation/read-back: Authenticated desktop `/admin/buildings` opened the first edit sheet, submitted its existing values unchanged through the real PATCH form, received `Здание обновлено.`, and closed the sheet. A fresh navigation to the same route retained the first row value and all 18 rows. The page still had zero visible selects and no fresh console errors. This was an idempotent write with no net data change.

Acceptance impact: The shared sheet now has one authoritative success/read-back verification for Resources. Other resource CRUD paths, validation/conflict retention and Calendar/Accounts mutations remain unverified.
## Focused continuation — reservation mutation read-back fix (2026-09-13)

- **Scope:** Calendar reservation editor; desktop administrator surface only. No Schedule/Workload source was changed for this slice.
- **Reproduced defect:** An unchanged edit of an existing reservation submitted valid fields, but the API returned 500 and Rails rendered a generic 422. API logs identified RoomReservationSerializer calling reservation.teacher on the bare record returned by RoomReservationRepo#update_with_associations.
- **Source change:** apps/api_geteway/slices/admin/repos/room_reservation_repo.rb now overrides the aggregate read relation with root.combine(:teacher). Mutation read-backs therefore hydrate the same teacher association already used by the filtered index relation.
- **Deployment/provenance:** Copied the attributable API repo file into the running api_geteway container and restarted that service with Docker Compose. No database reset or fixture cleanup. docker compose exec -T api_geteway ruby -c /app/slices/admin/repos/room_reservation_repo.rb returned Syntax OK.
- **Runtime route/role:** Authenticated super administrator, http://localhost:3000/admin/room_reservations, 1440px desktop browser.
- **Controlled write/read-back:** Opened the first visible reservation editor and submitted its retained values without changing them. Rails request POST /admin/room_reservations/5 with method override PATCH returned 303; API PATCH /admin/room_reservations/5 returned 200 and audit logging completed. Rails displayed Бронирование обновлено.; the refreshed list rendered normally.
- **Browser checks:** 0 console errors after the corrected mutation; reservation table remained visible with five rows. The earlier failing reservation 4 was preserved and no fallback or optimistic DOM success was introduced.
- **Remaining:** A full reservation create, conflict, invalid-date and forbidden-role journey remains pending. The focused desktop visual pass for Accounts, Calendar and Resources is complete at the inspected routes; broader role/concurrency coverage remains pending.
- **Next action:** Continue final evidence/status reconciliation for the three focused administrator sections without changing accepted Schedule/Workload composition.

## Focused continuation — desktop primary action refinement (2026-09-13)

- **Scope:** Accounts, Calendar and Resources desktop composition. Schedule and Workload were not changed.
- **Source change:** Scoped the three focused primary create actions to content width with 20px horizontal padding and preserved the 44px / 14px control role. This removes the full-canvas banner effect while retaining a clear single primary action.
- **Deployment/provenance:** Copied `_admin_accounts.css`, `_admin_calendar.css` and `_admin_resources.css` into the running `schedule_web` container, ran `rails assets:precompile`, and restarted `schedule_web` through Docker Compose. Served application CSS fingerprint: `application-e709f81f.css`.
- **Runtime routes/role/viewport:** Authenticated super administrator; `/admin/accounts`, `/admin/operations/semesters`, `/admin/buildings`; 1440px desktop.
- **Checks and exact results:** Accounts create action measured 203x44px at 14px; Calendar semester create measured 164x44px at 14px; Buildings create measured 164x44px at 14px. Accounts and Calendar headings remained 24px/32px. Buildings rendered one table with 18 data rows. All checked pages had document/client width 1425/1425 and zero fresh console errors.
- **Table comparison:** No table columns, order, headers, grouping or row meaning changed.
- **Remaining:** Full negative/conflict and role authorization journeys remain pending; this pass intentionally deprioritizes responsive work per the latest user direction.
- **Next action:** Continue focused desktop interaction verification for searchable selection, dirty drafts and safe mutation read-back across Calendar and Accounts.

## Focused continuation — searchable dirty draft retention (2026-09-13)

- **Scope:** Account create sheet interaction on desktop.
- **Runtime route/role/viewport:** Authenticated super administrator; `/admin/accounts`; 1440px.
- **Interaction check:** Opened `Создать пользователя`, entered a disposable email draft, searched teachers with `Pet`, selected the first result, then searched departments with `ПСи` while the sheet remained open. The result frames updated in place.
- **Exact result:** The native sheet stayed open; teacher hidden identity remained `1`; selected label became `Выбран преподаватель: Petrov Ivan Oleksandrovych`; the email draft remained `ui-search-draft@example.test`; the department result frame returned one result. No account submission was made.
- **Browser checks:** No fresh console errors. The sheet was closed without a business mutation.
- **Remaining:** Full account and Calendar invalid/conflict/role mutation coverage remains pending.

## Focused continuation — refined desktop evidence capture (2026-09-13)

- **Evidence:** Fresh full-page screenshots were captured after the scoped primary-action refinement: `admin-accounts-desktop-action-refined-20260913.png`, `admin-calendar-desktop-action-refined-20260913.png`, and `admin-resources-buildings-desktop-action-refined-20260913.png` in the Playwright evidence output.
- **Runtime check:** The final Resources check rendered the Russian `Здания` heading, an Add action at 164x44px, no page overflow (1440/1440), and zero console errors. Compose services remained healthy.
- **Table comparison:** Resource table inventory remains unchanged.

## Focused continuation — explicit search labels (2026-09-13)

- **Scope:** Accounts edit sheets and Calendar reservation search disclosures.
- **Source change:** Wrapped teacher and department edit search fields in explicit Russian labels and replaced placeholder-only wording with useful search guidance. Applied the same label treatment to reservation room and teacher search fields. GET parameters, Turbo frame targets and selection identity are unchanged.
- **Deployment/provenance:** Copied the two changed ERB templates into the running `schedule_web` container and restarted it through Docker Compose. Rails controller syntax check returned `Syntax OK`.
- **Runtime routes/role/viewport:** Authenticated super administrator; `/admin/accounts` and `/admin/room_reservations`; 1440px desktop.
- **Checks and exact results:** Account edit search inputs reported wrapped labels for teacher and department fields. Reservation search inputs reported wrapped labels `Поиск аудитории` and `Поиск преподавателя`; the reservation table rendered five rows. Both routes had no page overflow and zero fresh console errors.
- **Table comparison:** No table columns, order or row structure changed.
- **Remaining:** Full negative/conflict and role authorization journeys remain pending.

## Focused continuation — remove beige canvas (2026-09-13)

- **Scope:** Accounts, Calendar and Resources desktop workspaces only.
- **Source change:** Scoped the workspace and document canvas backgrounds to `var(--admin-paper)` in `_admin_resources.css` and `_admin_calendar.css`. The existing `--admin-panel` token and its accent/state uses remain intact; no public or accepted Schedule/Workload styling was changed.
- **Deployment/provenance:** Copied the changed styles into `schedule_web`, ran Propshaft asset precompilation, and restarted through Docker Compose. Served CSS fingerprint: `application-e9ea6b6a.css`.
- **Runtime routes/role/viewport:** Authenticated super administrator; `/admin/accounts`, `/admin/operations/semesters`, `/admin/buildings` and a Schedule regression check; desktop browser.
- **Checks and exact results:** Accounts, Calendar and Buildings reported page and body backgrounds `rgb(255, 255, 255)`, with no page overflow. Buildings rendered the Russian `Здания` heading and 18 rows with zero console errors. Schedule retained the original `rgb(247, 246, 243)` body canvas.
- **Evidence:** Fresh browser snapshots are in the Playwright evidence output for the three focused routes.
- **Remaining:** Full negative/conflict and role authorization journeys remain pending.

## Focused continuation — remove beige tone from focused surfaces (2026-09-13)

- **Scope:** Accounts, Calendar and Resources desktop workspaces only.
- **Source change:** On pages containing `.resources-page` or `.operations-page`, scoped `--admin-panel` to the cool neutral `#f3f5f6` and kept the workspace/document canvas on `var(--admin-paper)`. The global admin token remains unchanged.
- **Deployment/provenance:** Copied `_admin_resources.css` into `schedule_web`, precompiled Propshaft assets and restarted through Docker Compose. Served CSS fingerprint: `application-1665a1fe.css`.
- **Runtime checks:** Authenticated Accounts reported body/page backgrounds `rgb(255, 255, 255)` and focused panel token `#f3f5f6`; no console errors. Schedule regression check retained body background `rgb(247, 246, 243)` and panel token `#f7f6f3`.
- **Invariant:** No tables, routes, selectors, operations or public pages were changed.
- **Remaining:** Full negative/conflict and role authorization journeys remain pending.

## Focused continuation — sticky sheet actions (2026-09-13)

- **Scope:** Shared editor behavior for focused Accounts, Calendar and Resources sheets.
- **Source change:** Added a shared rule in `_admin_components.css` that keeps the existing non-GET form submit action sticky at the bottom of the scrolling sheet body. Account credential-generation actions are covered separately; search forms remain ordinary in-flow controls.
- **Deployment/provenance:** Copied the stylesheet into `schedule_web`, precompiled Propshaft assets and restarted via Docker Compose. Served CSS fingerprint: `application-6edb29ad.css`.
- **Runtime routes/role/viewport:** Authenticated super administrator; `/admin/accounts` and `/admin/room_types`; 1440px desktop.
- **Checks and exact results:** Room Type edit opened as a native dialog with a 44px sticky Save control and focus on the close button. Account create opened with a 720px scroll viewport and 803px content height; its 44px Save control computed as `position: sticky`. No fresh console errors.
- **Invariant:** Form endpoints, fields, tables and Schedule/Workload styling remain unchanged.
- **Remaining:** Full negative/conflict and role authorization journeys remain pending.

## Focused continuation — focused typography cleanup (2026-09-13)

- **Scope:** Accounts, Calendar and Resources desktop surfaces.
- **Source change:** Scoped legacy admin label rules so focused kickers, monitoring metadata, signed-in role metadata and the Calendar schedule link meet the prescribed minimum readable sizes. Calendar schedule action is now a 40px / 14px secondary action.
- **Deployment/provenance:** Copied `_admin_resources.css`, precompiled Propshaft and restarted `schedule_web` through Docker Compose. Served CSS fingerprint: `application-6f2cae43.css`.
- **Runtime routes/role/viewport:** Authenticated super administrator; `/admin/operations?faculty_id=1&semester_id=9&group_id=789` and `/admin/accounts`; desktop.
- **Checks and exact results:** Rendered leaf text below 11px count was zero on both routes. Calendar schedule action measured 235x40px at 14px. Calendar body remained white with 1440/1440 document/client width and zero fresh console errors; Accounts retained 1425/1425 width and zero fresh console errors.
- **Invariant:** Schedule was not changed; focused tables and field contracts are unchanged.
- **Remaining:** Full negative/conflict and role authorization journeys remain pending.

## 2026-09-13 — Focused desktop workspaces fill the available screen

- Status: completed for the focused desktop layout slice; no Schedule/Workload source changes.
- Finding addressed: Accounts, Calendar and Resources now explicitly stretch through the full Rails admin content column beside the fixed 224px sidebar. The page surfaces have no max-width or shrink-to-content behavior; tables retain their local scrolling behavior and the 24px internal gutter.
- Attributable source: `apps/schedule_web/app/assets/stylesheets/components/_admin_resources.css` (scoped desktop full-workspace rule for Accounts, Calendar and Resources).
- Runtime provenance: deployed to the running `schedule_web` container with Docker Compose, assets precompiled as `application-6d14e6ec.css`, and the container restarted.
- Browser checks: authenticated Playwright session at 1440x900 visited `/admin/accounts`, `/admin/operations/semesters`, and `/admin/buildings`. Each focused page measured x=224, width=1201, right=1425, matching `.admin-main`; document scrollWidth/clientWidth both measured 1425. No visible `<select>` controls were present on these routes. Browser console was clear after the fresh checks.
- Visual evidence: `docs/design/admin-workbench-2026-09-13/evidence/accounts-fullscreen-1440.png`.
- Regression check: `/admin/schedule?faculty_id=1&semester_id=9` remained on the existing shell/content column; no Schedule source was edited.
- Remaining issue: the browser viewport still reserves the native vertical scrollbar width when the page is taller than the viewport; this is browser chrome, not a page width cap. The next focused UI pass should continue with visual polish inside this full-width canvas.
## 2026-09-13 — Accounts filter uses the full desktop canvas

- Status: completed for the Accounts desktop filter refinement.
- Finding addressed: the role filter no longer collapses into a narrow internal horizontal scroller. Role choices are now presented as one visible row across the full Accounts workspace; status choices and the search action occupy a clear second row.
- Attributable source: `apps/schedule_web/app/assets/stylesheets/components/_admin_accounts.css`.
- Runtime provenance: copied the stylesheet into the running `schedule_web` container, precompiled Propshaft assets as `application-22515117.css`, and restarted through Docker Compose.
- Browser checks: authenticated Playwright session at 1440x900. `/admin/accounts` measured the filter at 1153px wide with role options 1119px wide, 40px high, `flex-wrap: nowrap`, and `overflow: visible`; document scrollWidth/clientWidth both measured 1425. The focused route had zero visible native selects and no console errors.
- Regression smoke check: `/admin/operations/semesters` and `/admin/buildings` still measured x=224, width=1201, right=1425, with no page overflow, no visible native selects and zero console errors.
- Visual evidence: `docs/design/admin-workbench-2026-09-13/evidence/accounts-fullscreen-current-1440.png`.
- Invariant: all six account table columns and their order remain unchanged; no Schedule/Workload source was edited.
- Remaining: complete negative/conflict and role authorization journeys remain pending.
## 2026-09-13 — Resources navigation aligned for the full desktop canvas

- Status: completed for the Resources navigation refinement.
- Finding addressed: the twelve reference destinations now use a deliberate six-column, two-row desktop grid. Labels and active state are aligned across the full workspace instead of forming an uneven trailing row.
- Attributable source: `apps/schedule_web/app/assets/stylesheets/components/_admin_resources.css`.
- Runtime provenance: copied the stylesheet into the running `schedule_web` container, precompiled Propshaft assets as `application-d368bcd4.css`, and restarted through Docker Compose.
- Browser checks: authenticated Playwright session at 1440x900 on `/admin/buildings`. Each navigation item measured 178x44px in six equal 177.5px tracks; all twelve labels remained visible, document scrollWidth/clientWidth both measured 1425, and the browser console had zero errors.
- Visual evidence: `docs/design/admin-workbench-2026-09-13/evidence/resources-fullscreen-current-1440.png`.
- Invariant: all existing navigation destinations and routes remain present; Buildings and Room Types table content and schemas are unchanged.
- Remaining: complete negative/conflict and role authorization journeys remain pending.
## 2026-09-13 — Calendar navigation aligned for the full desktop canvas

- Status: completed for the Calendar navigation refinement.
- Finding addressed: the five Calendar sections now use equal-width desktop tracks, making the navigation use the available workspace instead of clustering at the left.
- Attributable source: `apps/schedule_web/app/assets/stylesheets/components/_admin_calendar.css`.
- Runtime provenance: copied the stylesheet into the running `schedule_web` container, precompiled Propshaft assets as `application-338b2aaf.css`, and restarted through Docker Compose.
- Browser checks: authenticated Playwright session at 1440x900 on `/admin/operations/semesters`. Each Calendar destination measured 215x44px across five equal tracks; page measured x=224, width=1201, right=1425; document scrollWidth/clientWidth both measured 1425; console had zero errors.
- Visual evidence: `docs/design/admin-workbench-2026-09-13/evidence/calendar-fullscreen-current-1440.png`.
- Invariant: all five Calendar destinations, routes, table columns and calendar semantics remain unchanged.
- Remaining: complete negative/conflict and role authorization journeys remain pending.
## 2026-09-13 — Focused route sweep after desktop navigation refinements

- Status: completed for the current desktop visual sweep; deeper mutation and authorization gates remain pending.
- Runtime routes checked in the authenticated Playwright session at 1440x900: Resources `posts`, `subjects`, `academic_titles`, `academic_degrees`, `activity_types`, `time_slots`, `week_types`, `building_types`, `buildings`, `rooms`, `room_types`, `student_groups`; Calendar `operations/semesters`, `holidays`, `weekday_transfers`, and `room_reservations`.
- Exact results: every route rendered its Russian page heading; focused pages measured the full available desktop column (x=224, width=1201 where the vertical scrollbar was present, or width=1216 when absent); document width matched client width on each sample; no visible native `<select>` control was found; fresh browser console checks returned zero errors.
- Source check: focused templates contain no `<select>`, `select_tag`, `collection_select` or `options_for_select` usage in the audited route families.
- Content checks: Buildings and Room Types rendered their tables and edit dialogs; Reservations rendered its table and six editor dialogs; all existing navigation destinations remained reachable.
- Evidence: refreshed desktop captures are in `docs/design/admin-workbench-2026-09-13/evidence/` (`accounts-fullscreen-current-1440.png`, `calendar-fullscreen-current-1440.png`, `resources-fullscreen-current-1440.png`).
- Invariant: table schemas, route contracts and existing Schedule/Workload source remain unchanged.
- Remaining: focused write journeys beyond the previously verified reservation read-back, plus negative/conflict and role authorization coverage, remain pending.
## 2026-09-13 — Focused editor IDs and modal semantics cleaned up

- Status: completed for the focused editor accessibility refinement.
- Finding addressed: repeated hidden editor forms no longer reuse HTML IDs. Accounts, Reservations and Buildings now generate unique IDs for repeated search fields, hidden identity fields, scope fields, dates, time slots, purpose fields, status controls and building type/number fields.
- Attributable source: `apps/schedule_web/app/views/admin/accounts/index.html.erb`, `apps/schedule_web/app/views/admin/room_reservations/index.html.erb`, and `apps/schedule_web/app/views/admin/resources/buildings/index.html.erb`.
- Runtime provenance: copied the three templates into the running `schedule_web` container and restarted through Docker Compose.
- Browser checks: authenticated Playwright session at 1440x900. Accounts, Reservations and Buildings reported zero duplicate IDs; Reservations rendered five rows and Buildings rendered 18 rows with 19 dialogs. Each route had no horizontal overflow and zero console errors.
- Modal check: the create editor on each route opened as a native modal (`dialog.open=true`, `:modal=true`) with its `aria-labelledby` heading and a focused editor control.
- Invariant: request names, route contracts, table columns and record identity values were preserved; no Schedule/Workload source was edited.
- Remaining: full negative/conflict and role authorization journeys remain pending.
## 2026-09-13 — Visible field labels completed in Calendar and Buildings sheets

- Status: completed for the focused form semantics refinement.
- Finding addressed: new and edit Reservation sheets now expose visible Russian labels for room search, teacher search, reservation date and purpose. Building create/edit sheets now label the building number field explicitly. Placeholder text remains supportive guidance only.
- Attributable source: `apps/schedule_web/app/views/admin/room_reservations/index.html.erb` and `apps/schedule_web/app/views/admin/resources/buildings/index.html.erb`.
- Runtime provenance: copied both templates into the running `schedule_web` container and restarted through Docker Compose.
- Browser checks: authenticated Playwright session at 1440x900 opened both create sheets. Reservations reported visible labels `Поиск аудитории`, `Поиск преподавателя`, `Дата бронирования` and `Назначение`; Buildings reported `Номер здания`. Both opened as native modal dialogs, had zero duplicate IDs, no horizontal overflow and zero console errors.
- Invariant: request names, table schemas, routes and selection identities remain unchanged.
- Remaining: full negative/conflict and role authorization journeys remain pending.
## 2026-09-13 — Room and Room Type editors made record-safe

- Status: completed for the explicit Resources room editor refinement.
- Finding addressed: repeated room and room-type sheets no longer reuse generated radio/input IDs. Create and edit forms now use unique IDs per record and catalog choice, and text fields have visible Russian labels for room number/name and room-type name.
- Attributable source: `apps/schedule_web/app/views/admin/resources/rooms/index.html.erb` and `apps/schedule_web/app/views/admin/resources/room_types/index.html.erb`.
- Runtime provenance: copied both templates into the running `schedule_web` container and restarted through Docker Compose.
- Browser checks: authenticated Playwright session at 1440x900. Rooms rendered 37 rows and Room Types rendered 16 rows; both create sheets opened as native modal dialogs, reported zero duplicate IDs, no horizontal overflow and zero console errors.
- Invariant: table columns, routes, request parameter names and catalog identities remain unchanged.
- Remaining: full negative/conflict and role authorization journeys remain pending.
## 2026-09-13 — Time Slot editor values normalized for native controls

- Status: completed for the generic Resources editor runtime cleanup.
- Finding addressed: Time Slot edit inputs previously received values such as `2026-`, which are invalid for native `input[type=time]` controls. The generic template now extracts the `HH:mm` portion for both the table display and edit fields.
- Attributable source: `apps/schedule_web/app/views/admin/resources/posts/index.html.erb`.
- Runtime provenance: copied the template into the running `schedule_web` container and restarted through Docker Compose.
- Browser checks: authenticated Playwright session at 1440x900 on `/admin/time_slots`. Existing edit values rendered as valid `08:30`, `10:00`, `10:10`, `11:35` examples; invalid time values count was zero, duplicate ID count was zero, page width matched client width at 1425, and the browser returned zero warnings and zero errors.
- Invariant: time-slot table meaning, stored parameter names, routes and row count (7) remain unchanged.
- Remaining: full negative/conflict and role authorization journeys remain pending.
## 2026-09-13 — Student Group editor made record-safe

- Status: completed for the remaining high-volume Resources editor.
- Finding addressed: Student Group specialty radios and year fields now have unique record-scoped IDs. Create fields for admission year, student count and semester duration have explicit Russian labels and clearer guidance.
- Attributable source: `apps/schedule_web/app/views/admin/student_groups/index.html.erb`.
- Runtime provenance: copied the template into the running `schedule_web` container and restarted through Docker Compose.
- Browser checks: authenticated Playwright session at 1440x900 on `/admin/student_groups`. Fifty rows rendered; duplicate ID count was zero; the create sheet opened as a native modal; visible labels included `Год набора`, `Количество студентов` and `Длительность семестра`; document width matched client width at 1425 and the console had zero errors.
- Invariant: group table columns, semester links, routes, request names and specialty identities remain unchanged.
- Remaining: full negative/conflict and role authorization journeys remain pending.
## 2026-09-13 — Group Semester editor labels and IDs completed

- Status: completed for the remaining group-semester editor surface.
- Finding addressed: repeated group-semester sheets no longer reuse hidden group, student-count or duration field IDs. The numeric fields now use visible Russian labels and record-scoped IDs.
- Attributable source: `apps/schedule_web/app/views/admin/student_groups/semesters.html.erb`.
- Runtime provenance: copied the template into the running `schedule_web` container and restarted through Docker Compose.
- Browser checks: authenticated Playwright session at 1440x900 on `/admin/student_groups/1/semesters`. Eight rows rendered; duplicate ID count was zero; all sixteen editor fields had explicit labels (`Количество студентов`, `Длительность семестра`); page width matched client width at 1425 and the console had zero errors.
- Invariant: the five-column group-semester table, group identity, semester links, routes and request names remain unchanged.
- Remaining: full negative/conflict and role authorization journeys remain pending.
## 2026-09-13 — Week Type editor no longer leaves an orphan field label

- Status: completed for the generic Resources conditional-form cleanup.
- Finding addressed: Week Type sheets previously rendered a `Короткое название` label without an input because the conditional was applied only to the field helper. The generic template now conditionally renders the entire label/field pair.
- Attributable source: `apps/schedule_web/app/views/admin/resources/posts/index.html.erb`.
- Runtime provenance: copied the template into the running `schedule_web` container and restarted through Docker Compose.
- Browser checks: authenticated Playwright session at 1440x900 on Week Types, Posts and Time Slots. Week Types reported zero orphan `Короткое название` labels, zero duplicate IDs and no overflow; all three routes had zero console errors.
- Invariant: Week Type name/code fields, generic catalog schemas, routes and request names remain unchanged.
- Remaining: full negative/conflict and role authorization journeys remain pending.
## 2026-09-13 — Focused table actions use the compact control role

- Status: completed for the focused desktop action-density refinement.
- Finding addressed: row-level Edit/Delete and reservation/account row actions now use the prescribed compact 36px control role, while primary actions and sheet Save controls retain their larger 40–44px sizing.
- Attributable source: `apps/schedule_web/app/assets/stylesheets/components/_admin_resources.css`.
- Runtime provenance: copied the stylesheet into the running `schedule_web` container, precompiled Propshaft assets as `application-a6a7e7f3.css`, and restarted through Docker Compose.
- Browser checks: authenticated Playwright session at 1440x900 on Buildings, Accounts and Reservations. First row actions measured 36px high at 13px/18px; each route remained width-safe at 1425/1425 and fresh console checks returned zero errors.
- Invariant: row action meaning, table columns, editor triggers and destructive-action separation remain unchanged.
- Remaining: full negative/conflict and role authorization journeys remain pending.
## 2026-09-13 — Main Calendar workspace navigation expanded

- Status: completed for the main Calendar operations navigation slice.
- Finding addressed: `Периоды расписания` used the older left-clustered navigation while Calendar catalog pages used the full-width treatment. The operations route now presents all five Calendar destinations in equal desktop tracks.
- Attributable source: `apps/schedule_web/app/assets/stylesheets/components/_admin_calendar.css`.
- Runtime provenance: copied the stylesheet into the running `schedule_web` container, precompiled Propshaft assets as `application-9f8dd603.css`, and restarted through Docker Compose.
- Browser checks: authenticated Playwright session at 1440x900 on `/admin/operations?faculty_id=1&semester_id=9&group_id=789`. Five tabs measured 218x44px, spanning x=272 through x=1392 inside the 1216px workspace; page width matched the viewport and console errors were zero.
- Visual evidence: `docs/design/admin-workbench-2026-09-13/evidence/calendar-operations-current-1440.png`.
- Invariant: Calendar routes, period cards, context identity and table semantics remain unchanged.
- Remaining: full negative/conflict and role authorization journeys remain pending.
## 2026-09-13 — Focused table regions are named and keyboard-reachable

- Status: completed for the focused table navigation semantics.
- Finding addressed: dense table containers now expose a named `region` landmark and `tabindex="0"`, allowing keyboard users to reach the local scrolling surface without changing table columns or converting rows to cards.
- Attributable source: focused table templates under `apps/schedule_web/app/views/admin/accounts`, `app/views/admin/resources`, `app/views/admin/room_reservations`, `app/views/admin/holidays`, `app/views/admin/semesters`, and `app/views/admin/student_groups`.
- Runtime provenance: copied the changed templates into the running `schedule_web` container and restarted through Docker Compose.
- Browser checks: authenticated Playwright session at 1440x900 on Accounts, Reservations, Buildings and Student Groups. Each table card reported `role=region`, a heading reference through `aria-labelledby`, and `tabindex=0`; programmatic focus landed on the region; document width remained equal to client width and fresh console checks returned zero errors.
- Invariant: table columns, grouping, row meaning and local horizontal scrolling behavior remain unchanged.
- Remaining: full negative/conflict and role authorization journeys remain pending.## 2026-09-13 — Full-width focused workspaces and served table focus treatment

- Status: completed for the requested desktop composition refinement on Accounts, Calendar and Resources.
- Finding addressed: the focused administrator pages now use the available desktop canvas with a white main surface, equal-width Calendar/Resources navigation tracks, and keyboard-reachable table regions. The new scoped table-region focus rule is scoped to the focused admin workspaces.
- Attributable source: `apps/schedule_web/app/assets/stylesheets/components/_admin_resources.css`, `app/assets/stylesheets/components/_admin_calendar.css`, `app/assets/stylesheets/components/_admin_accounts.css`, and the focused table templates listed in the preceding handoff entry.
- Runtime provenance: the latest Rails assets were precompiled in the running `schedule_web` container as `application-4013baf1.css`, then the service was restarted through Docker Compose.
- Browser checks: authenticated Playwright session at 1440x900 on `/admin/accounts`, `/admin/room_reservations`, and `/admin/buildings`. Headings rendered in Russian; each route reported a 1201px main canvas within a 1440px viewport, white body background, no horizontal page overflow, and a focusable named table region; fresh console errors were zero. Programmatic focus reached the regions, while visual `:focus-visible` activation remains pending a real Tab sequence. Ruby controller syntax checks returned `Syntax OK` for Accounts and Room Reservations.
- Visual evidence: `docs/design/admin-workbench-2026-09-13/evidence/accounts-fullscreen-current-1440.png`, `calendar-fullscreen-current-1440.png`, `resources-fullscreen-current-1440.png`, and `calendar-operations-current-1440.png`.
- Invariant: Schedule source and composition were not changed by this slice; table schemas, columns, ordering, row meanings, routes and operation contracts remain unchanged.
- Remaining: mutation read-back, invalid/conflict handling, role coverage, and full browser acceptance across all prescribed viewports remain pending. Next action is a focused interaction pass for the three requested workspaces.## 2026-09-13 — Table-region focus ring uses an existing semantic token

- Status: completed for the focused table keyboard indication.
- Finding addressed: the first scoped focus rule referenced an undefined token and therefore computed to no visible outline. It now uses the existing `--velvet` semantic color with a 2px outline and 3px offset.
- Attributable source: `apps/schedule_web/app/assets/stylesheets/components/_admin_resources.css`.
- Runtime provenance: copied the corrected stylesheet into `schedule_web`, precompiled Propshaft assets as `application-80cd91bd.css`, and restarted through Docker Compose.
- Browser checks: authenticated Playwright session at `/admin/accounts` on 1440x900 used `focus({ focusVisible: true })`; the named table region remained focused and computed `rgb(75, 38, 47) solid 2px` with a 3px offset. Fresh console errors were zero.
- Invariant: no table structure, columns, row meaning, navigation or Schedule styling was changed.
- Remaining: full mutation read-back, invalid/conflict handling and role coverage remain pending.## 2026-09-13 — Temporary account-password generation no longer returns 500

- Status: completed for the Accounts credential-generation regression.
- Finding addressed: UUID account records were incorrectly coerced with `Integer(id)` in the Rails Accounts gateway, and the controller used an unavailable `ActiveModel::Type::Boolean` constant while parsing the submitted generation flag. Together these caused the Generate temporary password action to fail before the API update operation.
- Attributable source: `apps/schedule_web/app/lib/gateways/admin/accounts_gateway.rb` and `app/controllers/admin/accounts_controller.rb`.
- Runtime provenance: copied the corrected Rails source into the running `schedule_web` container and restarted through Docker Compose. No API contract or database schema was changed.
- Browser checks: authenticated Playwright opened Accounts, opened an existing account editor, and submitted the real Russian `Сгенерировать временный пароль` action. The result page title was `Временный пароль`; an authenticated same-origin form submission returned HTTP 201 with `Cache-Control: no-store, max-age=0` and `Pragma: no-cache`. The response body and generated credential were not inspected or captured. Fresh browser errors were zero on the successful request path.
- Invariant: account UUID identity, PATCH `/auth/accounts/:id`, one-time credential behavior, password omission from drafts and API serializer, and role/scope authorization remain unchanged.
- Remaining: invalid/conflict and forbidden-role coverage for Accounts remains pending.## 2026-09-13 — Accounts validation failures reopen the same editor

- Status: completed for the Accounts safe-draft interaction slice.
- Finding addressed: a failed account create/update response preserved submitted values in the HTML but returned the workspace with every editor closed. The shared sheet controller now accepts an optional failed-editor ID and reopens that native dialog after the response is rendered.
- Attributable source: `apps/schedule_web/app/javascript/controllers/admin_sheet_controller.js` and `app/views/admin/accounts/index.html.erb`.
- Runtime provenance: copied the controller and template into `schedule_web` and restarted through Docker Compose; the previous UUID gateway and boolean parsing fixes remain deployed.
- Browser checks: authenticated Playwright opened the first account editor, submitted a deliberately invalid email through the real Save path, and received HTTP 422. The response rendered `data-admin-sheet-open-id-value` without escaped quotes; the same dialog reported `open=true` and `:modal=true`, retained `invalid-email`, and retained zero password values. The 422 network entry was the expected validation response; no 500 occurred. Ruby and JavaScript syntax checks passed.
- Invariant: passwords remain excluded from drafts, generation remains one-time/no-store, UUID identity and API contracts are unchanged, and table structure is untouched.
- Remaining: Accounts forbidden-role and conflict coverage, plus representative Resources/Calendar failure-retention journeys, remain pending.## 2026-09-13 — Reservation validation reopens the Calendar sheet

- Status: completed for the Calendar reservation safe-draft interaction slice.
- Finding addressed: reservation create/update already retained a safe draft on recoverable failure, but the native editor was closed after the 422 response. The view now passes the failed create/update editor ID to the shared sheet controller.
- Attributable source: `apps/schedule_web/app/views/admin/room_reservations/index.html.erb`.
- Runtime provenance: copied the template into `schedule_web` and restarted through Docker Compose.
- Browser checks: authenticated Playwright opened `Бронирования аудиторий`, submitted an intentionally incomplete reservation through the real `Забронировать` action, and received HTTP 422. The create sheet reopened with `open=true` and `:modal=true`; date and purpose drafts remained present as empty values, and no reservation was created. Fresh browser errors were zero for this path; Ruby syntax checks passed.
- Invariant: room/teacher person identity, existing reservation fields, table columns and reservation API contracts remain unchanged.
- Remaining: conflict/forbidden reservation coverage and representative Resources failure-retention remain pending.## 2026-09-13 — Generic Resources failures retain the active sheet draft

- Status: completed for the generic Resources create/edit failure-retention slice.
- Finding addressed: Posts, Subjects, Activity Types, Building Types, Time Slots and Week Types redirected after failed create/update requests, discarding the active sheet context. The controller now keeps submitted safe attributes, reloads the current catalog, returns HTTP 422, and passes the failed editor ID to the shared native sheet controller. The generic template uses the retained values for both create and edit fields.
- Attributable source: `apps/schedule_web/app/controllers/admin/resources/posts_controller.rb` and `app/views/admin/resources/posts/index.html.erb`.
- Runtime provenance: copied the controller and template into `schedule_web` and restarted through Docker Compose.
- Browser checks: authenticated Playwright rendered Posts at 1440x900 with 24 editors, a 1201px main canvas and no initial console errors. An intentionally empty create submission returned 422 and reopened `new-resource-editor` as a modal with the empty safe draft and Russian feedback. An intentionally empty edit submission returned 422 and reopened `edit-resource-posts-0` as a modal with the empty retained name and feedback. The two 422 network entries were expected validation responses; no record was created or changed. Ruby syntax passed.
- Invariant: resource schemas, routes, API operations, table columns and existing Russian labels remain unchanged.
- Remaining: dedicated Buildings/Rooms/Room Types failure retention and conflict/forbidden coverage remain pending.## 2026-09-13 — Dedicated Resources editors retain relationship drafts

- Status: completed for Buildings, Rooms and Room Types failure retention.
- Finding addressed: dedicated resource controllers redirected after failed create/update operations. They now retain submitted attributes, reload the catalog context, return HTTP 422 and reopen the corresponding native sheet. Radio relationships for building type, room building/type and room-type activity are restored from the safe draft.
- Attributable source: `apps/schedule_web/app/controllers/admin/resources/buildings_controller.rb`, `rooms_controller.rb`, `room_types_controller.rb`, and their three `index.html.erb` views.
- Runtime provenance: copied all six changed files into `schedule_web` and restarted through Docker Compose.
- Browser checks: authenticated Playwright rendered Buildings, Rooms and Room Types at 1440x900. All reported a 1201px main canvas, no horizontal overflow, zero visible native selects, and zero initial console errors. An intentionally incomplete Room Type create submission returned 422, reopened the native `new-resource-editor` with `open=true` and `:modal=true`, retained the empty name and no activity selection, and displayed Russian failure feedback. Ruby syntax checks passed for all four Resources controllers.
- Invariant: resource identities, relationship IDs, routes, table schemas and existing catalog operations remain unchanged; no record was created by the negative-path check.
- Remaining: dedicated Buildings/Rooms negative-path browser checks, conflict/forbidden coverage and successful CRUD read-back remain pending.

## 2026-09-13 — Calendar period validation retains the affected card draft

- Status: completed for the Calendar period validation slice; the broader Calendar batch remains in progress.
- Source: `apps/schedule_web/app/controllers/admin/period_settings/periods_controller.rb` now renders the current Calendar context on recoverable period failures instead of redirecting; `app/views/admin/period_settings/show/show.html.erb` uses the submitted dates for the affected card while preserving the persisted delete action.
- Runtime provenance: deployed to the `schedule_web` container with Docker Compose and restarted; the served page was `http://localhost:3000/admin/operations?faculty_id=1&semester_id=9&group_id=789`, title `Календарь периодов`, Russian document locale.
- Browser evidence: at the authenticated administrator route, submitted the first period card with `2026-12-31` as start and `2026-01-01` as end. The response was HTTP 422, retained both values, kept all four period cards rendered, kept the same URL/context, and displayed `Даты периода некорректны.`.
- Layout check: the served page measured 1216px for the main workspace at the desktop browser viewport and had no document-level horizontal overflow. Screenshot: `docs/design/admin-workbench-2026-09-13/evidence/calendar-periods-current.png`.
- Verification: Ruby syntax checks passed for both period controllers. Browser console contained only the expected failed-request entry for the deliberate 422 validation check; no 500 occurred after the renderer fix.
- Remaining issue: the full Calendar mutation/read-back matrix, including valid period persistence, reservations and weekday-transfer operations, remains to be completed and verified.
- Next action: continue with the remaining focused Calendar/Resources/Accounts visual and runtime gates, preserving the accepted Schedule composition.


## 2026-09-13 — Calendar period field labels brought to the readable admin scale

- Status: runtime-verified visual slice.
- Source: `apps/schedule_web/app/assets/stylesheets/components/_admin_calendar.css`; added a Calendar-scoped rule for period card labels at 12px/18px, leaving the accepted Schedule and public styles untouched.
- Runtime provenance: copied the stylesheet into `schedule_web`, ran Rails asset precompile, and restarted the container. Served application fingerprint: `application-15c29552.css`.
- Browser checks: authenticated `/admin/operations?faculty_id=1&semester_id=9&group_id=789` at 1440x900; computed period labels were 12px/18px, date inputs 14px and 44px high, Save buttons 44px high, and console errors were zero.
- Evidence: `docs/design/admin-workbench-2026-09-13/evidence/calendar-periods-readable-labels.png`.
- Table/layout invariant: no table structure changed; the four period editor units remain separate and the desktop document has no horizontal overflow.
- Remaining issue: the complete mutation/read-back and cross-page acceptance matrix for Calendar, Resources and Accounts is still pending.
- Next action: continue the focused visual/runtime audit on Resources and Accounts, then verify the remaining Calendar routes and editor states.

## 2026-09-19 — Excel import workspace

- Status: source-complete; runtime verification blocked by the pre-existing stopped API/PostgreSQL stack.
- Attributable source: `apps/schedule_web/app/controllers/admin/excel_imports_controller.rb`, `app/lib/gateways/admin/excel_imports_gateway.rb`, `app/javascript/controllers/excel_import_controller.js`, `app/views/admin/excel_imports/`, `app/assets/stylesheets/components/_admin_excel_imports.css`, scoped navigation/routes, and the API Excel-file serializer/upload service plus migration `20260919000000_add_original_filename_to_excel_files.rb`.
- Behaviour: a super administrator can open `Импорт Excel`, drag one `.xls` or `.xlsx` file into the visible workspace, retain the current semester context when present, see a paginated status history, refresh manually, receive five-second polling while uploads are pending, and delete a file after confirmation. Rails keeps the API token server-side; no file picker, dropdown, WebSocket, worker rewrite, or public-UI styling was introduced.
- API contract: Rails uses `GET /admin/files`, multipart `POST /admin/uploads/excel`, and `DELETE /admin/files/:id`. The new immutable `original_filename` column/projection keeps the source file name visible after the worker changes `title` to its detected structure name.
- Verification: Ruby syntax checks passed for the changed Rails controller/gateways/HTTP client and API service/serializer/migration; JavaScript syntax passed for the Stimulus controller. Runtime route, authenticated browser, upload/read-back, polling, deletion, asset, and migration checks remain unverified because `api_geteway` and `postgres` were already stopped and their existing logs show an incomplete database migration state. No storage reset or unrelated migration was attempted.
- Next action: restore the existing API/PostgreSQL environment, apply only the intended migration after confirming the outstanding migration set, rebuild the API and Rails assets, then perform the authenticated drag-upload → status polling → authoritative read-back → delete journey.

## 2026-09-19 — Excel import runtime recovery

- Status: service recovery completed; authenticated workflow verification remains pending.
- Runtime: rebuilt and restarted `api_geteway` and `schedule_web` with Docker Compose. Both containers report healthy, Rails `/up` returns successfully from inside the application container, and `GET /admin/files` now returns its expected unauthenticated `401` response rather than a server error.
- Database: applied only `20260919000000_add_original_filename_to_excel_files.rb` to the running PostgreSQL database. The `excel_files.original_filename` column is present and the migration is recorded, resolving the serializer/database schema mismatch that caused the 500 response.
- Remaining: verify the complete authenticated administrator journey: open the workspace, drag-upload an Excel file, observe status polling and authoritative read-back, then delete it.

## 2026-09-19 — Excel import 500 repair

- Status: fixed and deployed.
- Root cause: authenticated `GET /admin/excel_imports` raised `Dry::Container::KeyError` for `gateways.admin.excel_imports` before it could call the API. The gateway class existed, but its dependency-container registration was omitted.
- Fix: registered `Gateways::Admin::ExcelImportsGateway` in `apps/schedule_web/config/initializers/container.rb`, rebuilt the Rails image, and recreated `schedule_web`.
- Verification: Rails health check succeeds; the running container resolves and instantiates `ScheduleWeb::Container["gateways.admin.excel_imports"]` as `Gateways::Admin::ExcelImportsGateway`; no new Excel-import `KeyError` or 500 entries appear after the rebuild.

## 2026-09-19 — Excel import template route repair

- Status: fixed and deployed.
- Root cause: after the container registration was corrected, `app/views/admin/excel_imports/index.html.erb` still called `refresh_admin_excel_imports_path`. The named route generated by Rails is `admin_refresh_excel_imports_path`, so the signed-in page raised `NoMethodError` during rendering and returned 500.
- Fix: replaced the invalid helper, rebuilt the Rails image, and recreated `schedule_web`.
- Verification: Rails health check succeeds. The actual Excel-import template now renders inside the rebuilt Rails container with a successful file-list outcome and produces the expected `/admin/excel_imports/refresh` URL.

## 2026-09-19 — Excel import authenticated controller repair

- Status: fixed and deployed.
- Root cause: the authenticated `GET /admin/excel_imports` path received `Success<Array>` from `ExcelImportsGateway#semesters`, while `selected_semester_label` treated the value as a `Resilience::Outcome` and called `.data`. Live Rails logs captured `NoMethodError: undefined method 'data' for an instance of Array` at `excel_imports_controller.rb:63`.
- Fix: the controller now consumes the gateway's actual array contract with `Array(@semesters_result.value!)` before selecting the current semester.
- Verification: rebuilt and recreated `schedule_web`; Rails and API containers are healthy. Runtime checks inside the deployed Rails image cover the successful semester label, successful empty file list, upstream-failure state, template refresh URL and generated routes. The exact unauthenticated localhost route returns the expected login redirect, and post-rebuild logs contain no new Excel-import exception or 500.


## 2026-09-13 — Focused Resources and Accounts desktop pass rechecked

- Status: runtime-verified visual audit.
- Routes: `/admin/buildings`, `/admin/room_types`, `/admin/rooms`, `/admin/posts`, and `/admin/accounts`, authenticated as the existing administrator at 1440x900.
- Results: Buildings and Room Types render their content lists and shared navigation; each route uses the full 1201px main workspace beside the retained dark sidebar. Accounts renders the six-column table-first layout with the create action separated from row geometry.
- Interaction checks: zero visible native selects on all sampled focused routes; no initial alert state; no document overflow beyond the desktop shell. Accounts has no open row editor on initial load, and each edit action is represented as a separate dialog.
- Evidence: `docs/design/admin-workbench-2026-09-13/evidence/resources-buildings-current.png`, `docs/design/admin-workbench-2026-09-13/evidence/accounts-current.png`.
- Table invariant: resource and account table column meanings remain visible; no cards or hidden columns were introduced.
- Remaining issue: representative successful Resource/Calendar CRUD read-back and complete role/scope coverage remain pending.
- Next action: continue mutation-safe editor verification and address any concrete rendered defect found in the focused routes.
## 2026-09-20 — Staff date-resolved aggregate schedules and Schedule context

- Status: source-complete and runtime-verified for the faculty aggregate read and Rails rendering; department scope and full mutation/browser matrix remain pending.
- Functional slice: added staff-authorized date-resolved faculty and department occurrence endpoints in Hanami, reusing the occurrence-window transfer/cancellation semantics. Added the Rails gateway, operation, presenter, route, navigation item and Russian aggregate table with selected-date printing.
- UX slice: made the Schedule frame commit its selected context to browser history and constrained the planner inspector with its own viewport and sticky primary action so long editors remain operable.
- Runtime provenance: rebuilt `api_geteway` and `schedule_web` through Docker Compose; Rails `/up` returned 200, the aggregate route returned 200 for an authenticated faculty request and rendered two date-resolved occurrences, and unauthenticated access returned 302. Hanami routes, Rails routes and changed Ruby/JavaScript syntax checks passed.
- Attributable revisions: Rails `e3e3408`, `c915abc`; API `abb2dcd`, `27c31f9`.
- Runtime follow-up: direct authenticated API checks returned 200 for both faculty and department occurrence windows, 422 for an invalid ISO date, and 401 without a token. Rails rendered the department page with the selected department label and two lessons for the selected date.
- Remaining: verify department authorization/printing with a scoped account, moved/cancelled fixtures, copied URL Back/Forward behavior in a live browser, and continue B01/B04–B06 visual/runtime slices. Performance and availability measurements remain deferred by product decision.

## 2026-09-20 — Narrow-shell disclosure and heading consistency

- Status: source-complete and deployed.
- Finding addressed: secondary Grafana/Prometheus/Loki links consumed the narrow administrator shell, and Teachers/Schedule headings used larger page-specific scales than the shared workbench.
- Attributable source: `apps/schedule_web/app/views/admin/shared/_sidebar.html.erb`, `app/javascript/controllers/monitoring_disclosure_controller.js`, and `app/assets/stylesheets/components/_admin_workspace.css`.
- Behavior: monitoring links remain open on desktop and become an accessible collapsed disclosure at widths up to 760px; Schedule, aggregate Schedule and Teachers headings use the shared 24px/32px desktop role and 20px/28px narrow role.
- Runtime provenance: rebuilt and recreated `schedule_web`; `/up` returned 200, the served page contains the new Stimulus controller fingerprints, and the aggregate route still renders its date table.
- Remaining: browser-computed narrow measurements and the complete B01/B04–B06 acceptance matrix still require a functioning authenticated Playwright session.

## 2026-09-20 — Workload editor primary action reachability

- Status: source-complete and deployed.
- Finding addressed: long workload create/edit/assignment sheets required scrolling to reach their primary save action.
- Attributable source: `apps/schedule_web/app/assets/stylesheets/components/_admin_workload.css`.
- Behavior: the existing server-backed primary action remains inside the editor form and now stays available at the bottom of the sheet while its body scrolls; no form fields, contracts or table columns changed.
- Runtime provenance: Rails image rebuilt and recreated; `/up` returned 200 and `rails zeitwerk:check` passed. Rails revision: `32e6a1a`.
- Remaining: browser bounding-box confirmation and complete workload failure/read-back coverage remain pending.

## 2026-09-20 — Lesson-transfer calendar rendering repair

- Status: fixed and runtime-verified.
- Finding: the dated lesson-transfer workspace called the missing `ru.date.formats.long` translation and returned HTTP 500 whenever two dates and a group were selected.
- Fix: replaced the unavailable localized formatter with an explicit Russian numeric date presentation while preserving the two timetable schemas, transfer controls and ISO values submitted to the API.
- Verification: authenticated `/admin/transfers?semester_id=1&faculty_id=1&group_semester_id=1&left_date=2026-09-21&right_date=2026-09-22` now returns HTTP 200 and renders both dated grids, lessons and transfer actions. Rails revision: `b968316`.
- Remaining: mutation/read-back, conflict handling and browser keyboard/drag acceptance remain pending.

The Russian locale now also defines the shared `date.formats.long` (`ДД.ММ.ГГГГ`) used by remaining date presenters, preventing the same missing-translation failure in future calendar/public render paths. Rails revision: `9287586`.

## 2026-09-20 — Weekday-transfer validation draft retention

- Status: source-complete and deployed; authenticated negative-path browser confirmation remains pending.
- Behavior: recoverable weekday-transfer failures now carry the safe submitted date, weekday, optional group and reason back through the redirect, reopen the transfer sheet, and restore the selected controls. No credentials or unrelated data are stored.
- Attributable source: `apps/schedule_web/app/controllers/admin/weekday_transfers_controller.rb` and `app/views/admin/weekday_transfers/index.html.erb`; Rails revision `2a78492`.
- Verification: ERB generation, diff checks and Docker rebuild passed; the Hanami contract independently returns `422` for an invalid date/weekday payload and creates no record.
- Remaining: authenticated Rails 422 flow, successful create/delete read-back and conflict coverage remain pending.

## 2026-09-20 — Aggregate schedule context filtering repair

- Status: source-complete and deployed; authenticated browser interaction remains pending with the unavailable Playwright transport.
- Finding: faculty search and its option list were mounted as separate `choice-filter` controller instances, so typing could not filter the list and could raise a missing-target error.
- Behavior: faculty and department aggregate-schedule catalogs now share one controller instance per search region, expose labelled Russian search fields, hide non-matching options, and show an empty-result message without changing the selected scope or table schema.
- Attributable source: `apps/schedule_web/app/views/admin/aggregate_schedules/show.html.erb`; Rails revision `072ad46`.
- Verification: ERB generation, `git diff --check`, Docker rebuild, Rails `zeitwerk:check`, and controller syntax check passed.
- Remaining: live keyboard filtering, selected-scope navigation and full aggregate mutation/read-back evidence remain pending.

## 2026-09-20 — Notice narrow-table semantics repair

- Status: source-complete and deployed; visual narrow/print browser comparison remains pending.
- Finding: the Notice report changed from a table to block/card-like rows below 680px, hiding the table header and violating the required column-preservation rule.
- Behavior: narrow Notice reports now remain real tables inside an explicitly horizontally scrollable region; all columns, order, headers and print rules remain unchanged.
- Attributable source: `apps/schedule_web/app/assets/stylesheets/components/_admin_notice.css`; Rails revision `11130f7`.
- Verification: stylesheet diff check, Docker asset rebuild and Rails `zeitwerk:check` passed.
- Remaining: verify the narrow scroll affordance, keyboard reachability and desktop/narrow print output in a live browser.

## 2026-09-20 — Holiday editor draft retention and labels

- Status: source-complete and deployed; authenticated negative-path browser confirmation remains pending.
- Finding: a failed holiday create redirected away from the editor and discarded the submitted date/name; both controls also lacked visible labels.
- Behavior: recoverable holiday failures now re-render the Calendar page with the submitted date and name, reopen the editor, and show Russian field labels. Successful creation and delete routes remain unchanged.
- Attributable source: `apps/schedule_web/app/controllers/admin/holidays_controller.rb` and `app/views/admin/holidays/index.html.erb`; Rails revision `a1c634f`.
- Verification: Rails Ruby/ERB syntax checks, diff check, Docker rebuild and `rails zeitwerk:check` passed.
- Remaining: authenticated 422 retention, successful holiday create/delete read-back and conflict evidence remain pending.

## 2026-09-20 — Semester editor draft retention

- Status: source-complete and deployed; authenticated negative-path browser confirmation remains pending.
- Finding: invalid or conflicting semester creation redirected away and discarded the submitted academic years and semester number.
- Behavior: failed semester creation now re-renders the Calendar page with safe submitted values, reopens the editor, and maps conflict/permission failures to Russian feedback. Successful creation still selects the new semester.
- Attributable source: `apps/schedule_web/app/controllers/admin/semesters_controller.rb` and `app/views/admin/semesters/index.html.erb`; Rails revision `f1be325`.
- Verification: Ruby/ERB syntax checks, diff check, Docker rebuild and `rails zeitwerk:check` passed.
- Remaining: authenticated 422 retention and successful semester creation/read-back evidence remain pending.

## 2026-09-20 — Schedule edit draft retention

- Status: source-complete and deployed; authenticated conflict/validation browser confirmation remains pending.
- Finding: Schedule event update failures redirected with only a message, losing the submitted day, time, week, subgroup, status, room and assignment selections.
- Behavior: failed updates now retain a safe non-secret draft in the redirect flash and rehydrate the selected event editor, including checked teacher/room assignments and fallback room identity. The existing event ID and update operation remain unchanged.
- Attributable source: `apps/schedule_web/app/controllers/admin/schedules/update_controller.rb` and `app/views/admin/schedules/show/show.html.erb`; Rails revision `0195ff0`.
- Verification: Ruby/ERB syntax checks, diff check, Docker rebuild and `rails zeitwerk:check` passed.
- Remaining: authenticated invalid/conflict response, original-record preservation and authoritative reload/read-back evidence remain pending.

## 2026-09-20 — Dated transfer draft retention

- Status: source-complete and deployed; authenticated conflict/validation browser confirmation remains pending.
- Finding: failed recurring-to-dated transfer requests redirected with only an alert and discarded the submitted dates, action, target slot, room and reason.
- Behavior: transfer failures now preserve a separate safe draft and reopen the dated-transfer disclosure, while keeping recurring edits and dated transfers on their existing distinct operations.
- Attributable source: `apps/schedule_web/app/controllers/admin/schedules/transfers_controller.rb` and `app/views/admin/schedules/show/show.html.erb`; Rails revision `05daa46`.
- Verification: Ruby/ERB syntax checks, diff check, Docker rebuild and `rails zeitwerk:check` passed.
- Remaining: authenticated invalid/conflict response, cancellation/move read-back and original-record preservation evidence remain pending.

The same dated-transfer editor now offers readable room choices from the selected workload (with a current-room fallback) instead of exposing a raw room-ID input. Rails revision: `e1adf79`.

## 2026-09-20 — Schedule create draft retention

- Status: source-complete and deployed; authenticated occupied-slot/validation browser confirmation remains pending.
- Finding: failed Schedule event creation redirected with an alert and discarded the staged assignment, room, slot and period inputs.
- Behavior: failed creates now preserve a safe draft through the redirect and rehydrate the same selected-cell editor, including multi-teacher/multi-room selections and period-specific date values. The create operation and load identity remain unchanged.
- Attributable source: `apps/schedule_web/app/controllers/admin/schedules/events_controller.rb` and `app/views/admin/schedules/show/show.html.erb`; Rails revision `aedae5b`.
- Verification: Ruby/ERB syntax checks, diff check, Docker rebuild and `rails zeitwerk:check` passed.
- Remaining: authenticated occupied-slot rejection, successful create/read-back and remaining-hours verification remain pending.

## 2026-09-20 — Schedule swap draft retention

- Status: source-complete and deployed; authenticated conflict/read-back browser confirmation remains pending.
- Finding: rejected dated swaps discarded the source date, target lesson, target date and reason.
- Behavior: failed swaps now preserve a separate draft and reopen the swap disclosure without merging it with recurring edits or dated transfers.
- Attributable source: `apps/schedule_web/app/controllers/admin/schedules/swaps_controller.rb` and `app/views/admin/schedules/show/show.html.erb`; Rails revision `6e81cff`.
- Verification: Ruby/ERB syntax checks, diff check, Docker rebuild and `rails zeitwerk:check` passed.
- Remaining: authenticated occupied-target rejection, successful swap/read-back and original-event preservation evidence remain pending.

The Schedule and dated lesson-transfer selectors now use readable teacher, room and lesson labels without exposing raw database IDs in normal controls. Request values remain unchanged. Rails revision: `69069bb`.

## 2026-09-20 — Calendar dated-transfer draft recovery

- Status: source-complete and deployed; authenticated conflict/read-back browser confirmation remains pending.
- Finding: the two-date Calendar move/swap dialog was client-staged, so a rejected API mutation returned to the page without the submitted operation values.
- Behavior: move and swap failures now carry a separate safe draft through the redirect; the Stimulus dialog restores the correct operation, event identity, dates, target slot/lesson, room, and reason before reopening. Move and swap remain distinct server operations.
- Attributable source: `apps/schedule_web/app/controllers/admin/lesson_transfers_controller.rb`, `app/views/admin/lesson_transfers/show.html.erb`, and `app/javascript/controllers/lesson_transfer_drag_controller.js`; Rails revision `29ef4e4`.
- Verification: Ruby/ERB/JavaScript syntax checks, diff check, Docker rebuild and `rails zeitwerk:check` passed.
- Remaining: authenticated rejected move/swap confirmation, successful read-back and original-event preservation evidence remain pending.

## 2026-09-20 — Student-group create draft retention

- Status: source-complete and deployed; authenticated validation/conflict browser confirmation remains pending.
- Finding: failed student-group creation discarded the selected speciality, admission year, student count, semester duration and automatic-provisioning choice.
- Behavior: failed creates now re-render the group catalog with safe submitted values and reopen the create sheet. Successful creation and provisioning contracts remain unchanged.
- Attributable source: `apps/schedule_web/app/controllers/admin/student_groups_controller.rb` and `app/views/admin/student_groups/index.html.erb`; Rails revision `c9d6ab0`.
- Verification: Ruby/ERB syntax checks, diff check, Docker rebuild and `rails zeitwerk:check` passed.
- Remaining: authenticated rejected-create confirmation and successful group/read-back evidence remain pending.

## 2026-09-20 — Group-semester edit draft retention

- Status: source-complete and deployed; authenticated validation/conflict browser confirmation remains pending.
- Finding: failed updates to a group-semester's student count or duration redirected away and discarded the edited row values.
- Behavior: failed updates now re-render the selected group's semester list, preserve the submitted values for the affected row, and reopen that row editor. Successful updates retain their existing redirect and identity.
- Attributable source: `apps/schedule_web/app/controllers/admin/student_groups_controller.rb` and `app/views/admin/student_groups/semesters.html.erb`; Rails revision `5908d7e`.
- Verification: Ruby/ERB syntax checks, diff check, Docker rebuild and `rails zeitwerk:check` passed.
- Remaining: authenticated rejected-update confirmation and successful group-semester read-back evidence remain pending.

## 2026-09-20 — Student-group edit draft retention

- Status: source-complete and deployed; authenticated validation/conflict browser confirmation remains pending.
- Finding: failed group edits discarded the selected speciality and admission year and returned to the unedited list.
- Behavior: failed edits now re-render the catalog with the affected row's safe draft values and reopen that row editor. Successful updates retain the existing operation and identity.
- Attributable source: `apps/schedule_web/app/controllers/admin/student_groups_controller.rb` and `app/views/admin/student_groups/index.html.erb`; Rails revision `40a05f8`.
- Verification: Ruby/ERB syntax checks, diff check, Docker rebuild and `rails zeitwerk:check` passed.
- Remaining: authenticated rejected-edit confirmation and successful group update/read-back evidence remain pending.

## 2026-09-20 — Teacher search label cleanup

- Status: source-complete and deployed; authenticated browser visual confirmation remains pending.
- Finding: Teachers search results displayed a raw database ID beside every readable teacher label.
- Behavior: result labels now show the teacher name and role only; the existing link still carries the internal identifier server-side.
- Attributable source: `apps/schedule_web/app/views/admin/teachers/show/show.html.erb`; Rails revision `80e88d9`.
- Verification: ERB syntax check, diff check, Docker rebuild and `rails zeitwerk:check` passed.
- Remaining: complete Teachers mutation/read-back and responsive browser acceptance remain pending.

## 2026-09-20 — Teacher create draft retention

- Status: source-complete and deployed; authenticated validation/conflict browser confirmation remains pending.
- Finding: failed teacher creation redirected away and discarded names, post, academic degree/title, part-time status and department selections.
- Behavior: failed creates now retain a safe draft, reopen the teacher drawer, and restore all submitted controls. Passwords are not part of this operation or draft.
- Attributable source: `apps/schedule_web/app/controllers/admin/teachers/create_controller.rb`, `app/controllers/admin/teachers/mutation_controller.rb`, and `app/views/admin/teachers/show/_create.html.erb`; Rails revision `ba375f0`.
- Verification: Ruby/ERB syntax checks, diff check, Docker rebuild and `rails zeitwerk:check` passed.
- Remaining: authenticated rejected-create confirmation and successful teacher/read-back evidence remain pending.

## 2026-09-20 — Teacher profile draft retention

- Status: source-complete and deployed; authenticated validation/conflict browser confirmation remains pending.
- Finding: failed teacher profile updates discarded submitted names, academic selections and part-time status, reverting the visible form to the persisted record.
- Behavior: failed updates now preserve a separate safe profile draft and rehydrate the selected teacher editor, including an intentionally blank middle name.
- Attributable source: `apps/schedule_web/app/controllers/admin/teachers/update_controller.rb`, `app/controllers/admin/teachers/mutation_controller.rb`, and `app/views/admin/teachers/show/_profile.html.erb`; Rails revision `139a5a3`.
- Verification: Ruby/ERB syntax checks, diff check, Docker rebuild and `rails zeitwerk:check` passed.
- Remaining: authenticated rejected-update confirmation and successful teacher profile/read-back evidence remain pending.

## 2026-09-20 — Preserve intentionally blank teacher profile drafts

- Status: source-complete and deployed; authenticated negative-path browser confirmation remains pending.
- Finding: the profile draft path claimed to preserve a cleared middle name, but the view used presence fallback and restored the persisted name instead.
- Behavior: submitted profile values now take precedence whenever the draft contains the field, including an intentionally blank middle name and empty optional selections.
- Attributable source: `apps/schedule_web/app/views/admin/teachers/show/_profile.html.erb`; Rails revision `a8566d6`.
- Verification: ERB generation and Ruby syntax checks passed.
- Remaining: authenticated rejected-update confirmation and successful teacher profile/read-back evidence remain pending.

## 2026-09-20 — Preserve blank Calendar and group drafts

- Status: source-complete and deployed; authenticated negative-path browser confirmation remains pending.
- Finding: invalid blank numeric/date submissions in semester and student-group editors could be replaced by fresh defaults or persisted row values on re-render.
- Behavior: when a draft contains a field, its submitted value now remains visible, including blank values; fresh editors still receive the existing defaults.
- Attributable source: semester and student-group ERB editors; Rails revision `320b089`.
- Verification: ERB generation, Ruby syntax and diff checks passed.
- Remaining: authenticated rejected-create/update confirmation and successful read-back evidence remain pending.

## 2026-09-20 — Remove reservation fallback identifiers

- Status: source-complete and deployed; authenticated reservation read/write confirmation remains pending.
- Finding: reservation room and teacher option fallbacks exposed raw database IDs when catalog labels were incomplete.
- Behavior: fallback labels now communicate catalog context without showing identifiers; internal room and teacher values remain unchanged for the API contracts.
- Attributable source: `apps/schedule_web/app/controllers/admin/room_reservations_controller.rb`; Rails revision `41f4a1c`.
- Verification: diff check and container Ruby syntax check passed.
- Remaining: authenticated reservation list/create/edit conflict and read-back evidence remain pending.

## 2026-09-20 — Remove room catalog fallback identifiers

- Status: source-complete and deployed; authenticated resource visual confirmation remains pending.
- Finding: room create/edit building choices exposed a raw building ID when the catalog had no display number.
- Behavior: incomplete building labels now use a catalog-context message while preserving the internal relationship value.
- Attributable source: `apps/schedule_web/app/views/admin/resources/rooms/index.html.erb`; Rails revision `ed012b0`.
- Verification: diff check and container ERB compilation passed.
- Remaining: authenticated resource CRUD/read-back and narrow browser evidence remain pending.

## 2026-09-20 — Remove period context fallback identifiers

- Status: source-complete and deployed; authenticated period configuration confirmation remains pending.
- Finding: period context labels exposed a raw group-semester ID when the catalog lacked a display name.
- Behavior: context and group selectors now use a catalog-context label while preserving the group-semester identifier internally.
- Attributable source: `apps/schedule_web/app/views/admin/period_settings/show/show.html.erb`; Rails revision `c4a84fa`.
- Verification: diff check and container ERB compilation passed.
- Remaining: authenticated period save/rejection/read-back and browser evidence remain pending.

## 2026-09-20 — Keep period actions inside their editor units

- Status: source-complete and deployed; authenticated period interaction remains pending.
- Finding: the four period save forms and their delete forms were sibling grid items, allowing a delete action to appear under a different period column.
- Behavior: each period now owns a compact unit containing its save editor and optional delete action; all period kinds, routes and payloads remain unchanged.
- Attributable source: period settings view and Calendar stylesheet; Rails revision `8f2a777`.
- Verification: diff check and container ERB compilation passed.
- Remaining: authenticated period save/delete/read-back and responsive browser evidence remain pending.

## 2026-09-20 — Expose reservation pagination

- Status: source-complete and deployed; authenticated reservation navigation remains pending.
- Finding: the reservation API request was paginated at 50 rows, but the Calendar view had no controls to reach later pages.
- Behavior: accessible previous/next links now preserve room and date filters while changing only the reservation page.
- Attributable source: `apps/schedule_web/app/views/admin/room_reservations/index.html.erb`; Rails revision `172d8b4`.
- Verification: diff check and container ERB compilation passed.
- Remaining: authenticated reservation list/create/edit conflict and read-back evidence remain pending.

## 2026-09-20 — Expose weekday-transfer pagination

- Status: source-complete and deployed; authenticated transfer navigation remains pending.
- Finding: weekday transfers were fetched in 50-row pages, but the five-column table had no controls for subsequent pages.
- Behavior: previous/next navigation now preserves the day-transfer mode while changing only the page.
- Attributable source: `apps/schedule_web/app/views/admin/weekday_transfers/index.html.erb`; Rails revision `bdd0249`.
- Verification: diff check and container ERB compilation passed.
- Remaining: authenticated transfer create/delete/read-back and conflict evidence remain pending.

## 2026-09-20 — Expose holiday pagination

- Status: source-complete and deployed; authenticated holiday navigation remains pending.
- Finding: the holiday endpoint was paginated at 50 rows, but the Calendar table exposed only the current page.
- Behavior: previous/next controls now preserve Calendar context and change only the holiday page.
- Attributable source: `apps/schedule_web/app/views/admin/holidays/index.html.erb`; Rails revision `be55a39`.
- Verification: diff check and container ERB compilation passed.
- Remaining: authenticated holiday create/delete/read-back and conflict evidence remain pending.

## 2026-09-20 — Preserve blank resource relationships

- Status: source-complete and deployed; authenticated resource validation confirmation remains pending.
- Finding: shared academic catalogs and dedicated room/building/type editors used truthy relationship fallbacks, so a failed blank selection could be replaced by the persisted relationship.
- Behavior: submitted relationship keys now take precedence, including blank values; persisted nested labels remain the fallback for normal read-only rows.
- Attributable source: shared academic catalog form and room/building/room-type editors; Rails revision `b605552`.
- Verification: diff check and container ERB compilation passed.
- Remaining: authenticated resource 422/conflict and successful CRUD read-back evidence remain pending.

## 2026-09-20 — Normalize shared context typography

- Status: source-complete and deployed; browser computed-style confirmation remains pending.
- Finding: shared Schedule/Calendar context labels and period controls still used 10px text, below the design-system minimum for context and primary controls.
- Behavior: those shared admin roles now use 12px text while preserving existing spacing, control heights, table schemas and page-specific layouts.
- Attributable source: `apps/schedule_web/app/assets/stylesheets/components/_admin_workspace.css`; Rails revision `60b3626`.
- Verification: diff check passed.
- Remaining: computed-style checks at required viewports and complete browser acceptance remain pending.

## 2026-09-20 — Keep Accounts create action reachable

- Status: source-complete and deployed; browser bounding-box confirmation remains pending.
- Finding: the Accounts create form is nested inside role-selection controller markup, so the shared sheet-body sticky-action rule did not keep its primary action visible during long-form scrolling.
- Behavior: create and edit account form submissions now use the same reachable sticky action treatment within the Accounts sheet; fields and account contracts remain unchanged.
- Attributable source: `apps/schedule_web/app/assets/stylesheets/components/_admin_accounts.css`; Rails revision `328c506`.
- Verification: diff check passed.
- Remaining: authenticated Accounts mutation/read-back and computed viewport evidence remain pending.

## 2026-09-20 — Keep Teachers create action reachable

- Status: source-complete and deployed; browser bounding-box confirmation remains pending.
- Finding: the Teachers create drawer placed its primary submit at the end of a long scrolling form without a persistent action treatment.
- Behavior: the teacher creation action now remains reachable while the drawer body scrolls; the existing fields, draft retention and create contract remain unchanged.
- Attributable source: `apps/schedule_web/app/assets/stylesheets/components/_admin_teachers.css`; Rails revision `b80d722`.
- Verification: diff check passed.
- Remaining: authenticated teacher mutation/read-back and computed viewport evidence remain pending.

## 2026-09-20 — Remove workload assignment identifier fallbacks

- Status: source-complete and deployed; authenticated workload search/read-back remains pending.
- Finding: workload assignment options exposed a raw teacher ID when a teacher label was unavailable, and an empty room label could be rendered for incomplete search data.
- Behavior: assignment choices now use readable catalog-context fallbacks while retaining membership and room identifiers in the submitted replacement set.
- Attributable source: workload presenter and assignment partial; Rails revision `966269c`.
- Verification: diff check, Ruby syntax and container ERB compilation passed.
- Remaining: authenticated multi-selection, failed-save retention and authoritative workload read-back remain pending.

## 2026-09-20 — Remove remaining administrator identifier labels

- Status: source-complete and deployed; authenticated visual confirmation remains pending.
- Finding: four presenter fallbacks still exposed raw teacher or department IDs when catalog labels were missing.
- Behavior: workload, Notice and Teachers presenters now use readable catalog-context labels for incomplete data; internal IDs remain available to operations and links.
- Attributable source: workload, Notice and Teachers presenters; Rails revision `bfb1cb4`.
- Verification: raw-ID fallback scan is clean; diff check and container Ruby syntax checks passed.
- Remaining: authenticated visual/read-back evidence remains pending.

## 2026-09-20 — Remove catalog identifier display fallbacks

- Status: source-complete and deployed; authenticated visual confirmation remains pending.
- Finding: aggregate catalogs, duplicate time-slot labels and Teachers reference options still displayed raw IDs in incomplete or duplicate-label cases.
- Behavior: these labels now use catalog-context or ordinal wording; internal IDs and ordering remain unchanged.
- Attributable source: aggregate and Teachers presenters; Rails revision `4d29b1e`.
- Verification: identifier-pattern scan is clean for visible fallback labels; diff check and container Ruby syntax checks passed.
- Remaining: authenticated visual/read-back evidence remains pending.

## 2026-09-20 — Remove raw identifiers from audit views

- Status: source-complete and deployed; authenticated audit visual confirmation remains pending.
- Finding: the audit list and inspector printed raw entity and actor IDs as the primary object and user labels.
- Behavior: audit views now show the localized entity type and a readable staff context label; the existing detail action and sanitized payload still provide the audit record.
- Attributable source: Overview presenter and audit/observability views; Rails revision `a1b1f53`.
- Verification: diff check and container Ruby syntax check passed.
- Remaining: authenticated audit navigation and payload read-back evidence remain pending.

## 2026-09-20 — Compact committed Schedule context

- Status: source-complete; authenticated computed-height and interaction evidence remains pending.
- Finding: after a valid semester/faculty/group selection, Schedule kept all three catalog selectors expanded as a tall context wall, pushing the timetable below the working-space budget.
- Behavior: the committed state now presents one readable semester/faculty/group summary and an explicit `Изменить контекст` disclosure. Existing selectors, search behavior, identifiers and URL parameters remain unchanged inside the disclosure.
- Files changed: `apps/schedule_web/app/lib/presenters/admin/schedule_workspace.rb`, `apps/schedule_web/app/views/admin/schedules/show/show.html.erb`, `apps/schedule_web/app/assets/stylesheets/components/_admin_workspace.css`.
- Source revision: Rails `9672a27`; root worktree source remains limited to the handoff update.
- Runtime/source verification: template compiled through Rails' Erubi handler; presenter syntax and diff checks passed; rebuilt asset verification remains required after the next container build.
- Routes, role, context, viewport: `/admin/schedule`, staff administrator, selected semester/faculty/group; browser measurements at 1280×900 and 390×844 remain pending.
- Table inventory: unchanged; timetable headers, axes, cells and event semantics were not modified.
- Known remaining issue: browser connector is unavailable, so open/close, focus return and actual height reduction cannot yet be confirmed.
- Next action: rebuild `schedule_web`, verify asset loading and run the next B01 shared-control slice against Workload.

## 2026-09-20 — Normalize Schedule and Workload controls

- Status: source-complete; computed-style and keyboard/touch evidence remains pending.
- Finding: Schedule and Workload used competing 44–48px rules for ordinary context links and search controls, while the design contract calls for 40px default controls and 14px form text.
- Behavior: ordinary Schedule/Workload selectors and search controls now share a 40px/14px role; page-specific primary actions retain their larger touch target treatment.
- Files changed: `apps/schedule_web/app/assets/stylesheets/components/_admin_workspace.css`.
- Source revision: Rails `66eb35e`.
- Runtime/source verification: diff check passed; asset rebuild and Rails loading verification are pending in the current container cycle.
- Routes, role, context, viewport: `/admin/schedule` and `/admin/workload`, staff administrator, selected context; browser measurements at 1280×900, 768×1024 and 390×844 remain pending.
- Table inventory: unchanged; Schedule timetable and Workload five-column table retain their existing headers and row semantics.
- Known remaining issue: browser connector is unavailable, so actual computed sizes, focus treatment and narrow layout cannot yet be confirmed.
- Next action: rebuild the web container, run Rails/API checks, then continue with B02 workload assignment and placement verification.

## 2026-09-20 — Announce staged Schedule drag actions

- Status: source-complete; authenticated drag, keyboard and read-back evidence remains pending.
- Finding: typed Schedule drag sources selected a destination through the existing URL flow, but provided no live feedback about the selected load/event or staged destination.
- Behavior: the existing typed load/event drag path now announces source and destination state in a shared `aria-live` status region; click/tap links, destination URLs and save operations remain unchanged.
- Files changed: `apps/schedule_web/app/javascript/controllers/planner_drag_controller.js`, `apps/schedule_web/app/views/admin/schedules/show/show.html.erb`, `apps/schedule_web/app/assets/stylesheets/components/_admin_workspace.css`.
- Source revision: Rails `9b2b846`.
- Runtime/source verification: Node syntax check, Rails Erubi template compilation and diff check passed; rebuilt asset verification remains pending.
- Routes, role, context, viewport: `/admin/schedule`, staff administrator, selected group; browser drag/keyboard checks at 1280×900 and 390×844 remain pending.
- Table inventory: unchanged; timetable axes, headers, cells and event links remain intact.
- Known remaining issue: browser connector is unavailable, so live announcements and destination validation cannot yet be observed.
- Next action: rebuild `schedule_web` with the staged-drag assets, then continue the B02 placement/edit source audit.

## 2026-09-20 — Add retry state for reservation reads

- Status: source-complete; authenticated dependency failure and read-back evidence remains pending.
- Finding: when reservations, rooms or time slots failed to load, Calendar showed only a static unavailable sentence and offered no recovery action.
- Behavior: the reservation workspace now distinguishes the unavailable state, preserves the current URL filters and exposes a Russian `Повторить` action inside the Calendar frame. Create/edit controls remain withheld until required read dependencies succeed.
- Files changed: `apps/schedule_web/app/views/admin/room_reservations/index.html.erb`.
- Source revision: Rails `a52d490`.
- Runtime/source verification: Rails Erubi template compilation and diff check passed.
- Routes, role, context, viewport: `/admin/room_reservations`, staff schedule writer, current room/date filters; browser retry and responsive evidence remain pending.
- Table inventory: unchanged; reservation columns, order and row semantics remain intact.
- Known remaining issue: the browser connector is unavailable, and the underlying authenticated reservation failure has not yet been reproduced live.
- Next action: rebuild `schedule_web`, then continue B04 context and dated-rule verification.

## 2026-09-20 — Fix period context field semantics

- Status: source-complete; authenticated keyboard and context-switch evidence remains pending.
- Finding: Periods wrapped fieldsets containing radio labels inside outer `<label>` elements, creating invalid nested label semantics and unreliable accessible grouping.
- Behavior: faculty and semester context controls now use explicit labelled field groups; radio inputs, context parameters and automatic submit behavior remain unchanged.
- Files changed: `apps/schedule_web/app/views/admin/period_settings/show/show.html.erb`, `apps/schedule_web/app/assets/stylesheets/components/_admin_calendar.css`.
- Source revision: Rails `0a3ff81`.
- Runtime/source verification: Rails Erubi template compilation and diff check passed.
- Routes, role, context, viewport: `/admin/operations`, staff administrator, faculty/semester/group context; keyboard and responsive evidence remain pending.
- Table inventory: unchanged; period editor units and their four period kinds remain intact.
- Known remaining issue: browser connector is unavailable, so focus order and context disclosure behavior cannot yet be observed.
- Next action: rebuild `schedule_web`, verify Calendar assets, then continue B04 dated-rule state handling.

## 2026-09-20 — Add retry states for dated Calendar reads

- Status: source-complete; authenticated failure and read-back evidence remains pending.
- Finding: Holidays and weekday transfers rendered a static unavailable message without a recovery action.
- Behavior: both pages now expose a Russian `Повторить` action that preserves the current URL context, mode and page while keeping create/delete controls hidden until the read succeeds.
- Files changed: `apps/schedule_web/app/views/admin/holidays/index.html.erb`, `apps/schedule_web/app/views/admin/weekday_transfers/index.html.erb`.
- Source revision: Rails `5e742d0`.
- Runtime/source verification: Rails Erubi compilation for both templates and diff check passed.
- Routes, role, context, viewport: `/admin/holidays` and `/admin/transfers?mode=days`, staff administrator; browser retry and responsive evidence remain pending.
- Table inventory: unchanged; holiday three-column and weekday-transfer five-column structures remain intact.
- Known remaining issue: authenticated read failures and retry behavior cannot be observed until the browser connector is available.
- Next action: rebuild `schedule_web`, then move to B05 Accounts editor overlap and scope selection.

## 2026-09-20 — Add removable account scope chips

- Status: source-complete; authenticated account mutation/read-back and editor-overlap evidence remains pending.
- Finding: Accounts scope search could append a faculty or department scope, but neither existing nor newly added scopes had an explicit remove action.
- Behavior: every visible scope is now a removable labelled chip. Removal deletes only the matching hidden scope input; untouched scopes remain in the complete replacement collection sent by the existing account contract.
- Files changed: `apps/schedule_web/app/javascript/controllers/account_scope_picker_controller.js`, `apps/schedule_web/app/views/admin/accounts/index.html.erb`, `apps/schedule_web/app/assets/stylesheets/components/_admin_accounts.css`.
- Source revision: Rails `d12db99`.
- Runtime/source verification: Node syntax check, Rails Erubi template compilation and diff check passed.
- Routes, role, context, viewport: `/admin/accounts`, staff account manager, existing account row and preserved filter context; browser chip, failure and read-back evidence remain pending.
- Table inventory: unchanged; all six account columns remain intact.
- Known remaining issue: browser connector is unavailable, so scope removal and editor positioning cannot yet be observed.
- Next action: rebuild `schedule_web`, then finish the B05 editor state review.

## 2026-09-20 — Align account editor sheet geometry

- Status: source-complete; authenticated overlap and focus-return evidence remains pending.
- Finding: the Accounts fixed dialog inherited the native `<dialog>` margin and content-box sizing, allowing the declared 520px sheet to render wider than its available track.
- Behavior: the edit sheet now uses border-box viewport sizing, zero native margin, no duplicate border and a bounded viewport height while retaining the shared modal controller.
- Files changed: `apps/schedule_web/app/assets/stylesheets/components/_admin_accounts.css`.
- Source revision: Rails `b535bf3`.
- Runtime/source verification: diff check passed; asset rebuild and computed geometry verification remain pending.
- Routes, role, context, viewport: `/admin/accounts`, staff account manager, existing account row; browser overlap checks at 1280×900 and 390×844 remain pending.
- Table inventory: unchanged; six account columns and action semantics remain intact.
- Known remaining issue: browser connector is unavailable, so actual sheet bounds and modal focus behavior cannot yet be observed.
- Next action: rebuild `schedule_web`, verify Accounts assets, then continue B05 failure/read-back review.

## 2026-09-20 — Preserve manual account logins

- Status: source-complete; authenticated create/edit failure and read-back evidence remains pending.
- Finding: typing a name regenerated and overwrote an existing or manually edited account email on every change.
- Behavior: login generation now acts as a suggestion. It updates an empty or previously generated value, while an existing/manual value is preserved until the administrator changes it deliberately.
- Files changed: `apps/schedule_web/app/javascript/controllers/account_login_controller.js`, `apps/schedule_web/app/views/admin/accounts/index.html.erb`.
- Source revision: Rails `7b79a8c`.
- Runtime/source verification: Node syntax check, Rails Erubi template compilation and diff check passed.
- Routes, role, context, viewport: `/admin/accounts`, staff account manager, create and edit sheets; browser manual-login and mutation evidence remain pending.
- Table inventory: unchanged; account columns and filters remain intact.
- Known remaining issue: browser connector is unavailable, so manual override behavior cannot yet be observed interactively.
- Next action: rebuild `schedule_web`, then continue B05 failure-state and one-time credential review.

## 2026-09-20 — Add Notice search retry action

- Status: source-complete; authenticated Notice failure and print evidence remains pending.
- Finding: a failed Notice teacher search showed only `Повторите запрос позже` and had no retry action.
- Behavior: the existing query and semester context now remain available through a Russian `Повторить` action inside the Notice frame; report columns and print controls are unchanged.
- Files changed: `apps/schedule_web/app/views/admin/notice/show/show.html.erb`.
- Source revision: Rails `24dd440`.
- Runtime/source verification: Rails Erubi template compilation and diff check passed.
- Routes, role, context, viewport: `/admin/notice`, staff administrator, teacher query and semester context; browser retry and print evidence remain pending.
- Table inventory: unchanged; Notice retains its five report columns and print-only schema.
- Known remaining issue: browser connector is unavailable, so retry interaction and print layout cannot yet be observed.
- Next action: rebuild `schedule_web`, then complete B06 shared-state and responsive source review.

## 2026-09-20 — Collapse secondary monitoring navigation

- Status: source-complete; authenticated responsive navigation evidence remains pending.
- Finding: the shared sidebar rendered monitoring links expanded on every admin page, consuming vertical space despite being secondary navigation.
- Behavior: monitoring services now render as a collapsed native disclosure and remain available from the same sidebar location when opened.
- Files changed: `apps/schedule_web/app/views/admin/shared/_sidebar.html.erb`.
- Source revision: Rails `549954c`.
- Runtime/source verification: in-container Rails ERB compilation and diff check passed.
- Routes, role, context, viewport: all staff admin routes; sidebar at desktop and narrow widths; browser evidence remains pending.
- Table inventory: unchanged.
- Known remaining issue: browser connector is unavailable, so interactive disclosure and narrow-navigation behavior cannot yet be observed.
- Next action: continue B06 source review and perform browser acceptance when the connector is available.

## 2026-09-20 — Normalize disclosure focus states

- Status: source-complete; keyboard and responsive browser evidence remains pending.
- Finding: navigation group and monitoring disclosure summaries had no shared visible focus treatment.
- Behavior: keyboard focus now uses the admin almond outline with a consistent offset on both disclosure types.
- Files changed: `apps/schedule_web/app/assets/stylesheets/components/_admin_workspace.css`.
- Source revision: Rails `bf9f68b`.
- Runtime/source verification: in-container Rails Zeitwerk check passed; CSS diff is scoped to shared admin disclosure selectors.
- Routes, role, context, viewport: all staff admin routes; desktop and narrow sidebar navigation; browser evidence remains pending.
- Table inventory: unchanged.
- Known remaining issue: browser connector is unavailable, so keyboard focus visibility cannot yet be observed interactively.
- Next action: complete B06 source review and run the planned browser acceptance pass when the connector is available.

## 2026-09-20 — Add searchable Resource relationship selectors

- Status: source-complete; authenticated Resource CRUD and responsive evidence remains pending.
- Finding: room and building editors rendered relationship catalogs as unfiltered radio-card walls, and room rows could expose raw relationship IDs when nested labels were unavailable.
- Behavior: building types, buildings, room types and activity types now use bounded searchable choice lists with readable Russian labels and an explicit empty result state. Room table fallbacks use translated context text instead of database IDs; submitted relationship identifiers and existing contracts are unchanged.
- Files changed: `apps/schedule_web/app/views/admin/resources/buildings/index.html.erb`, `apps/schedule_web/app/views/admin/resources/rooms/index.html.erb`, `apps/schedule_web/app/views/admin/resources/room_types/index.html.erb`, `apps/schedule_web/app/assets/stylesheets/components/_admin_resources.css`.
- Source revision: Rails `e2606fa`.
- Runtime/source verification: ERB compilation for all three templates, Rails Zeitwerk check and `git diff --check` passed.
- Routes, role, context, viewport: `/admin/buildings`, `/admin/rooms`, `/admin/room_types`, super administrator; create/edit relationship selection and narrow sheets require browser verification.
- Table inventory: unchanged; building, room and room-type columns remain in their existing order and row meaning.
- Known remaining issue: browser connector is unavailable, so search filtering, submit/read-back and CRUD failure retention cannot yet be observed interactively.
- Next action: continue B03 resource form and dependency-state review, then perform the complete CRUD acceptance path when browser access is available.

## 2026-09-20 — Add searchable student-group specialization selectors

- Status: source-complete; authenticated group CRUD and responsive evidence remains pending.
- Finding: student-group create/edit used an unfiltered specialization card wall even though the catalog can grow beyond a small stable enumeration.
- Behavior: both forms now provide a bounded searchable specialization list with readable labels and an explicit no-match state; the existing `speciality_id` request field and group table remain unchanged.
- Files changed: `apps/schedule_web/app/views/admin/student_groups/index.html.erb`.
- Source revision: Rails `727916b`.
- Runtime/source verification: Rails ERB compilation and `git diff --check` passed.
- Routes, role, context, viewport: `/admin/student_groups`, super administrator, create/edit group sheets; browser selection, failure retention and read-back remain pending.
- Table inventory: unchanged; group specialization, admission year and action columns remain intact.
- Known remaining issue: browser connector is unavailable, so search and CRUD behavior cannot yet be observed interactively.
- Next action: continue B03 dependency and form-state review, then verify the full Resource CRUD journeys.

## 2026-09-20 — Add searchable Calendar context selectors

- Status: source-complete; authenticated period configuration and responsive evidence remains pending.
- Finding: Periods context rendered faculty and group-semester catalogs as unfiltered choice walls, making larger catalogs difficult to scan and inconsistent with the fixed selector rules.
- Behavior: faculty and group-semester context now expose bounded searchable lists with readable labels and explicit no-match states. Existing context parameters, automatic submit behavior and period semantics remain unchanged.
- Files changed: `apps/schedule_web/app/views/admin/period_settings/show/show.html.erb`, `apps/schedule_web/app/assets/stylesheets/components/_admin_calendar.css`.
- Source revision: Rails `469688f`.
- Runtime/source verification: Rails ERB compilation and `git diff --check` passed.
- Routes, role, context, viewport: `/admin/operations`, staff administrator, faculty/semester/group context; browser search, context retention and period read-back remain pending.
- Table inventory: unchanged; four period editor units and their field meanings remain intact.
- Known remaining issue: browser connector is unavailable, so selector behavior and context history cannot yet be observed interactively.
- Next action: continue B04 reservation dependency diagnosis and Calendar failure-state review.

## 2026-09-20 — Add searchable account faculty scope selector

- Status: source-complete; authenticated role/scope mutation and responsive evidence remains pending.
- Finding: super-admin account creation exposed faculty scope choices as an unfiltered radio-card wall, unlike the existing searched teacher and department selectors.
- Behavior: the faculty scope now has a bounded searchable list and explicit no-match state; hidden scope identity and role-driven submission behavior are unchanged.
- Files changed: `apps/schedule_web/app/views/admin/accounts/index.html.erb`, `apps/schedule_web/app/assets/stylesheets/components/_admin_accounts.css`.
- Source revision: Rails `1c45190`.
- Runtime/source verification: Rails ERB compilation and `git diff --check` passed.
- Routes, role, context, viewport: `/admin/accounts`, super administrator, create sheet and faculty-admin role; browser selection, failure and read-back evidence remain pending.
- Table inventory: unchanged; all six account columns remain intact.
- Known remaining issue: browser connector is unavailable, so scope filtering and role-specific form behavior cannot yet be observed interactively.
- Next action: continue final B05 account state review and then complete B06 acceptance source checks.

## 2026-09-20 — Normalize Teachers header and search controls

- Status: source-complete; browser computed-style and responsive evidence remains pending.
- Finding: Teachers retained a 34–74px display heading and oversized search controls, unlike the shared admin header and Notice control roles.
- Behavior: the Teachers heading now uses a bounded 24–42px scale, and the search input/button use the shared 40px control height while preserving the dark search panel and existing workspace composition.
- Files changed: `apps/schedule_web/app/assets/stylesheets/components/_admin_teachers.css`.
- Source revision: Rails `28a5a52`.
- Runtime/source verification: Rails Zeitwerk check and `git diff --check` passed.
- Routes, role, context, viewport: `/admin/teachers`, staff administrator, teacher search and selected workspace; computed-style and narrow/desktop browser evidence remain pending.
- Table inventory: unchanged; teacher availability grid and schedule preview semantics remain intact.
- Known remaining issue: browser connector is unavailable, so actual computed dimensions and responsive wrapping cannot yet be observed.
- Next action: complete B06 Notice/Observability source review and run browser acceptance when available.

## 2026-09-20 — Label Observability table scroll regions

- Status: source-complete; responsive and keyboard browser evidence remains pending.
- Finding: three Observability metric tables relied on card-level overflow without a labelled, keyboard-reachable scrolling region; only the audit table had a dedicated wrapper.
- Behavior: all four monitoring tables now use explicit labelled scroll regions, retain their existing columns, and expose scoped table headers. Card overflow no longer absorbs the scroll semantics.
- Files changed: `apps/schedule_web/app/views/admin/observability/show.html.erb`, `apps/schedule_web/app/assets/stylesheets/components/_admin_workspace.css`.
- Source revision: Rails `435d550`.
- Runtime/source verification: Rails ERB compilation, Zeitwerk check and `git diff --check` passed.
- Routes, role, context, viewport: `/admin/observability`, staff administrator, desktop and narrow widths; browser scrolling and focus evidence remain pending.
- Table inventory: unchanged; breaker, request, duration and audit schemas remain intact.
- Known remaining issue: browser connector is unavailable, so narrow overflow and keyboard reachability cannot yet be observed interactively.
- Next action: complete remaining B06 source checks and rerun the final source verification matrix.

## 2026-09-20 — Preserve initial department aggregate context

- Status: source-complete; authenticated aggregate navigation and read-back evidence remains pending.
- Finding: switching the staff aggregate workspace from Faculty to Department carried `scope_id=0`, so the first department-mode request returned a 404 even when the selected faculty had departments.
- Behavior: when department mode has no explicit department yet, the operation selects the first available department from the already loaded faculty catalog. Explicit department selections remain unchanged; API authorization still evaluates the selected department.
- Files changed: `apps/schedule_web/app/lib/operations/admin/load_aggregate_schedule.rb`.
- Source revision: Rails `7c8c77c`.
- Runtime/source verification: Ruby syntax, Rails Zeitwerk and `git diff --check` passed.
- Routes, role, context, viewport: `/admin/aggregate_schedule`, staff administrator, faculty-to-department mode switch; browser URL/history and date-resolved read-back remain pending.
- Table inventory: unchanged; seven aggregate schedule columns remain intact.
- Known remaining issue: browser connector is unavailable, so the mode switch and selected-date transfer/cancellation rendering cannot yet be observed interactively.
- Next action: continue F01 source review for date/print semantics and authorization edge cases.

## 2026-09-20 — Remove raw group-semester identifiers from the table

- Status: source-complete; authenticated group-semester read-back remains pending.
- Finding: the group-semester table visibly rendered the database row ID and fell back to a raw `semester_id` when catalog context was incomplete.
- Behavior: the five-column table now shows a readable `Группа-семестр` label and semantic semester fallback text. Internal row IDs remain only in form/action targets and request values.
- Files changed: `apps/schedule_web/app/views/admin/student_groups/semesters.html.erb`.
- Source revision: Rails `45b36df`.
- Runtime/source verification: Rails ERB compilation and `git diff --check` passed.
- Routes, role, context, viewport: `/admin/student_groups/:id/semesters`, super administrator, group-semester list and editor; browser read-back remains pending.
- Table inventory: unchanged at five columns; only the first column's visible wording was made semantic.
- Known remaining issue: browser connector is unavailable, so the readable labels and Calendar link context cannot yet be observed interactively.
- Next action: continue the final raw-identifier audit without changing table schemas.

## 2026-09-20 — Add Observability audit retry action

- Status: source-complete; authenticated failure and responsive evidence remains pending.
- Finding: an unavailable audit read showed a passive message without a recovery action.
- Behavior: the Observability audit section now exposes a Russian `Повторить` link using the current request URL, preserving route state and surrounding monitoring context.
- Files changed: `apps/schedule_web/app/views/admin/observability/show.html.erb`.
- Source revision: Rails `feb954a`.
- Runtime/source verification: Rails ERB compilation and `git diff --check` passed.
- Routes, role, context, viewport: `/admin/observability`, super administrator, audit failure state; browser retry evidence remains pending.
- Table inventory: unchanged; audit columns and scroll region remain intact.
- Known remaining issue: browser connector is unavailable, so retry behavior cannot yet be observed interactively.
- Next action: continue final B06 failure-state and print source review.

## 2026-09-20 — Add searchable Workload group selectors

- Status: source-complete; authenticated multi-group creation/filter/read-back evidence remains pending.
- Finding: Workload faculty filtering and new-record creation rendered groups in bounded scroll walls without search, despite groups being a high-cardinality entity.
- Behavior: both group surfaces now provide a labelled searchable filter with an explicit no-match state. Existing multi-select checkboxes, retained selections, group IDs and create contract are unchanged.
- Files changed: `apps/schedule_web/app/views/admin/workload/show/show.html.erb`, `apps/schedule_web/app/views/admin/workload/show/_create_record.html.erb`, `apps/schedule_web/app/assets/stylesheets/components/_admin_workload.css`.
- Source revision: Rails `cb7c049`.
- Runtime/source verification: ERB compilation for both templates, Rails Zeitwerk check and `git diff --check` passed.
- Routes, role, context, viewport: `/admin/workload`, staff administrator, faculty filter and create sheet; browser search, draft retention and authoritative read-back remain pending.
- Table inventory: unchanged; Workload retains its existing five columns and dense table behavior.
- Known remaining issue: browser connector is unavailable, so multi-group filtering and create/read-back cannot yet be observed interactively.
- Next action: continue B02 assignment draft and placement source review.

## 2026-09-20 — Add searchable Schedule group and assignment selectors

- Status: source-complete; authenticated schedule group and inline assignment interaction evidence remains pending.
- Finding: the Schedule workspace rendered its group selector and inline lesson teacher/room collections without the searchable controls already used by Workload, despite these being high-cardinality entities.
- Behavior: Schedule now provides a labelled local group filter plus searchable teacher and room assignment lists in both create and edit inspectors. Existing checkbox names, IDs, retained selections, and event mutation routes are unchanged.
- Files changed: `apps/schedule_web/app/views/admin/schedules/show/show.html.erb`, `apps/schedule_web/app/assets/stylesheets/components/_admin_workspace.css`.
- Source revision: Rails `fb059a2`.
- Runtime/source verification: Schedule ERB compilation, Rails Zeitwerk check and `git diff --check` passed in the running `schedule_web` container.
- Routes, role, context, viewport: `/admin/schedule`, staff administrator, selected faculty/group grid and create/edit inspectors; browser search and authoritative read-back remain pending.
- Table inventory: unchanged; the seven-day Schedule grid and its columns remain intact.
- Known remaining issue: browser connector is unavailable, so searchable Schedule interaction and the complete create/edit/read-back journey cannot yet be observed interactively.
- Next action: continue B02 placement and draft-retention source review.

## 2026-09-20 — Clarify Calendar reservation dependency failures

- Status: source-complete; authenticated reservation reads and conflict/read-back evidence remain pending.
- Finding: the Calendar reservation page collapsed failures from the reservation list, room catalog and time-slot catalog into one generic message, making the known unavailable-data state difficult to diagnose.
- Behavior: the page now identifies the failed required dependency or dependencies in Russian and keeps all reservation editors withheld until those reads succeed. Existing filters, retry URL and API contracts are unchanged.
- Files changed: `apps/schedule_web/app/controllers/admin/room_reservations_controller.rb`, `apps/schedule_web/app/views/admin/room_reservations/index.html.erb`.
- Source revision: Rails `5104f44`.
- Runtime/source verification: controller Ruby syntax, reservation ERB compilation, Rails Zeitwerk check and `git diff --check` passed in the running `schedule_web` container.
- Routes, role, context, viewport: `/admin/room_reservations`, staff schedule writer, current room/date filters; browser dependency-failure and retry evidence remain pending.
- Table inventory: unchanged; reservation columns, ordering and row semantics remain intact.
- Known remaining issue: browser connector is unavailable, so the failing dependency cannot yet be confirmed against an authenticated live response.
- Next action: continue B04 period and dated-rule source verification, then return to the disposable core journey when browser access is restored.

## 2026-09-20 — Add searchable Calendar reservation room lists

- Status: source-complete; authenticated reservation search and read-back evidence remain pending.
- Finding: reservation filtering and create/edit forms rendered the loaded room catalog as long radio-card walls, despite rooms being high-cardinality and server search already being available.
- Behavior: filter, create and edit room lists now expose labelled local search with explicit no-match states. Selected room values, hidden identity fields, picker controllers and reservation request contracts are unchanged.
- Files changed: `apps/schedule_web/app/views/admin/room_reservations/index.html.erb`.
- Source revision: Rails `fc30fdf`.
- Runtime/source verification: reservation ERB compilation, controller syntax, Rails Zeitwerk check and `git diff --check` passed in the running `schedule_web` container.
- Routes, role, context, viewport: `/admin/room_reservations`, staff schedule writer, create/edit/filter room selectors; browser search and mutation/read-back remain pending.
- Table inventory: unchanged; reservation columns and row semantics remain intact.
- Known remaining issue: browser connector is still unavailable, so room filtering and the underlying reservation read failure cannot yet be observed interactively.
- Next action: continue B04 period and dated-rule source verification.

## 2026-09-20 — Expose weekday-transfer group catalog failures

- Status: source-complete; authenticated dated-rule creation and read-back evidence remain pending.
- Finding: the day-transfer controller converted a failed student-group catalog read into an empty list, making a dependency failure indistinguishable from a legitimate global-scope transfer.
- Behavior: session expiry now covers both transfer and group reads. The empty-state copy distinguishes an unavailable group catalog from a genuinely empty catalog and explicitly states the resulting global scope.
- Files changed: `apps/schedule_web/app/controllers/admin/lesson_transfers_controller.rb`, `apps/schedule_web/app/views/admin/weekday_transfers/index.html.erb`.
- Source revision: Rails `86f697d`.
- Runtime/source verification: controller syntax, weekday-transfer ERB compilation, Rails Zeitwerk check and `git diff --check` passed in the running `schedule_web` container.
- Routes, role, context, viewport: `/admin/transfers?mode=days`, super administrator, global or group-scoped dated transfer; browser failure-state and read-back evidence remain pending.
- Table inventory: unchanged; the five transfer columns and row semantics remain intact.
- Known remaining issue: browser connector is unavailable, so group-catalog failure and explicit global-scope behavior cannot yet be observed interactively.
- Next action: continue B04 period-editor source verification.

## 2026-09-20 — Show period catalog read failures

- Status: source-complete; authenticated period persistence and read-back evidence remain pending.
- Finding: a failed period read was converted into an empty period collection, leaving blank date editors and obscuring the unavailable state.
- Behavior: the period page now distinguishes a valid group with no configured periods from a failed period read and offers a retry action while preserving the selected context. Failed period form drafts continue to use the existing re-render path.
- Files changed: `apps/schedule_web/app/controllers/admin/period_settings/show_controller.rb`, `apps/schedule_web/app/controllers/admin/period_settings/periods_controller.rb`, `apps/schedule_web/app/views/admin/period_settings/show/show.html.erb`.
- Source revision: Rails `00fecb5`.
- Runtime/source verification: both controller syntax checks, period ERB compilation, Rails Zeitwerk check and `git diff --check` passed in the running `schedule_web` container.
- Routes, role, context, viewport: `/admin/operations`, staff administrator, selected faculty/semester/group; browser retry and period persistence evidence remain pending.
- Table inventory: unchanged; period editors are form units rather than tables.
- Known remaining issue: browser connector is unavailable, so the failure distinction and retry action cannot yet be observed interactively.
- Next action: continue B04 holiday and calendar-context source verification.

## 2026-09-20 — Add Notice context retry actions

- Status: source-complete; authenticated Notice failure and report read-back evidence remain pending.
- Finding: Notice displayed retryable search/report failures, but failed teacher and semester context reads had no recovery action.
- Behavior: failed teacher and semester context states now expose the same labelled retry action while preserving the current query and selected context. Report schema and print/export controls are unchanged.
- Files changed: `apps/schedule_web/app/views/admin/notice/show/show.html.erb`.
- Source revision: Rails `6df5f20`.
- Runtime/source verification: Notice ERB compilation, Rails Zeitwerk check and `git diff --check` passed in the running `schedule_web` container.
- Routes, role, context, viewport: `/admin/notice`, staff administrator, teacher search and semester context; browser retry and report evidence remain pending.
- Table inventory: unchanged; Notice retains its five report columns and print semantics.
- Known remaining issue: browser connector is unavailable, so context retry behavior cannot yet be observed interactively.
- Next action: continue Overview and Observability source verification.

## 2026-09-20 — Add Teachers department-search retry

- Status: source-complete; authenticated department search and teacher read-back evidence remain pending.
- Finding: a failed department search in the Teachers workspace showed an error without a recovery action, while the surrounding catalog states already offered retry.
- Behavior: the department-search failure now includes a labelled retry link preserving the current teacher and search context. Department membership forms and identifiers remain unchanged.
- Files changed: `apps/schedule_web/app/views/admin/teachers/show/_departments.html.erb`.
- Source revision: Rails `6e4e325`.
- Runtime/source verification: department partial ERB compilation, Rails Zeitwerk check and `git diff --check` passed in the running `schedule_web` container.
- Routes, role, context, viewport: `/admin/teachers`, staff administrator, selected teacher and department query; browser retry and membership read-back remain pending.
- Table inventory: unchanged; Teachers availability and schedule structures remain intact.
- Known remaining issue: browser connector is unavailable, so department-search retry behavior cannot yet be observed interactively.
- Next action: finish Overview/Observability state review and run final source checks.

## 2026-09-20 — Clarify empty Observability metrics

- Status: source-complete; authenticated responsive and monitoring-state evidence remains pending.
- Finding: empty breaker, request and duration collections rendered blank table bodies, and the audit failure message was not exposed as a status region.
- Behavior: each metric table now has an explicit Russian empty row with the correct unchanged column span, and audit unavailability is announced through `role="status"`. Metric schemas, headings and scroll regions remain unchanged.
- Files changed: `apps/schedule_web/app/views/admin/observability/show.html.erb`.
- Source revision: Rails `64eda37`.
- Runtime/source verification: Observability ERB compilation, Rails Zeitwerk check and `git diff --check` passed in the running `schedule_web` container.
- Routes, role, context, viewport: `/admin/observability`, super administrator, empty and audit-failure states; browser accessibility and responsive evidence remain pending.
- Table inventory: unchanged; all three metric tables and the audit table retain their original columns and ordering.
- Known remaining issue: browser connector is unavailable, so empty-state announcement and narrow table behavior cannot yet be observed interactively.
- Next action: run final source-level checks across Rails and API, then retry the browser acceptance matrix.

## 2026-09-20 — Final source-level verification pass

- Status: source verification complete for the implemented slices; runtime mutation/read-back and responsive browser evidence remain pending.
- Checks: all 35 Rails administrator ERB templates compiled in the running `schedule_web` container; Rails `zeitwerk:check` passed; Hanami routes loaded successfully; aggregate faculty/department occurrence routes and administrator reservation routes are present; the aggregate occurrence repository passed Ruby syntax validation.
- Scope: no source changes in this verification pass; existing Rails, API and root untracked files were preserved.
- Known remaining issue: the authenticated Playwright transport still closes before a browser session can be used, so the B00 core journey, responsive matrix, authorization matrix and print/read-back acceptance cannot be claimed complete.
- Next action: rerun the same acceptance matrix when the browser transport is available; performance and availability measurement remain deferred by product decision.

## 2026-09-20 — Announce empty Overview audit state

- Status: source-complete; authenticated accessibility and responsive evidence remains pending.
- Finding: the empty audit journal state lacked a status role, unlike the unavailable state and adjacent monitoring surfaces.
- Behavior: the empty audit message is now announced with `role="status"`; audit filtering, pagination, table columns and inspector behavior are unchanged.
- Files changed: `apps/schedule_web/app/views/admin/overview/show.html.erb`.
- Source revision: Rails `01270c8`.
- Runtime/source verification: Overview ERB compilation, Rails Zeitwerk check and `git diff --check` passed in the running `schedule_web` container.
- Routes, role, context, viewport: `/admin/overview`, staff administrator, empty audit journal; browser accessibility evidence remains pending.
- Table inventory: unchanged; the four-column audit table remains intact.
- Known remaining issue: browser transport is still closed, so live announcement behavior cannot yet be observed.
- Next action: retry browser acceptance when the connector recovers.

## 2026-09-20 — Verify serving container liveness

- Status: runtime liveness verified; authenticated browser acceptance remains pending.
- Evidence: from inside the running `schedule_web` container, `/` returned HTTP 200 and protected administrator routes `/admin/overview`, `/admin/aggregate_schedule`, and `/admin/room_reservations` returned HTTP 302 to authentication as expected.
- Scope: read-only requests only; no credentials, records or sessions were changed.
- Known remaining issue: this confirms route protection and serving health, not authenticated rendering, responsive behavior, mutation success or read-back.
- Next action: restore browser transport/authentication and execute the documented acceptance matrix.

## 2026-09-20 — Add Workload subject-search retry

- Status: source-complete; authenticated Workload creation and draft/read-back evidence remain pending.
- Finding: a failed subject search in the new workload editor displayed passive text without the recovery action used by other high-cardinality searches.
- Behavior: the subject-search failure now exposes a labelled retry link using the current request URL. Existing selected groups, search context and creation contract remain unchanged.
- Files changed: `apps/schedule_web/app/views/admin/workload/show/_create_record.html.erb`.
- Source revision: Rails `9216c11`.
- Runtime/source verification: workload creation ERB compilation, Rails Zeitwerk check and `git diff --check` passed in the running `schedule_web` container.
- Routes, role, context, viewport: `/admin/workload`, staff administrator, new workload editor and subject query; browser retry and creation/read-back evidence remain pending.
- Table inventory: unchanged; Workload retains its existing table columns and semantics.
- Known remaining issue: browser transport is unavailable, so the retry interaction cannot yet be observed.
- Next action: retry the browser acceptance matrix when the connector recovers.

## 2026-09-20 — Add Period Settings context retry links

- Status: source-complete; authenticated period-context and persistence evidence remain pending.
- Finding: failed faculty and semester catalog reads in Period Settings showed passive hints without recovery actions.
- Behavior: both context failures now expose labelled retry links that preserve the current URL and Turbo frame. Period selection, group identity and date editors are unchanged.
- Files changed: `apps/schedule_web/app/views/admin/period_settings/show/show.html.erb`.
- Source revision: Rails `e12e565`.
- Runtime/source verification: Period Settings ERB compilation, Rails Zeitwerk check and `git diff --check` passed in the running `schedule_web` container.
- Routes, role, context, viewport: `/admin/operations`, staff administrator, faculty/semester/group context; browser retry and persistence evidence remain pending.
- Table inventory: unchanged; period editors retain their existing form structure.
- Known remaining issue: browser transport is unavailable, so context retry behavior cannot yet be observed.
- Next action: rerun browser acceptance when the connector recovers.

## 2026-09-20 — Announce Workload unavailable state

- Status: source-complete; authenticated Workload failure and read-back evidence remain pending.
- Finding: the Workload unavailable-load state had a retry action but lacked a status role for assistive technology.
- Behavior: the state now uses `role="status"`; retry URL, context retention and workload table semantics are unchanged.
- Files changed: `apps/schedule_web/app/views/admin/workload/show/show.html.erb`.
- Source revision: Rails `2747dea`.
- Runtime/source verification: Workload ERB compilation, Rails Zeitwerk check and `git diff --check` passed in the running `schedule_web` container.
- Routes, role, context, viewport: `/admin/workload`, staff administrator, unavailable load state; browser announcement and retry evidence remain pending.
- Table inventory: unchanged; Workload retains its existing five-column table.
- Known remaining issue: browser transport is unavailable, so live status announcement cannot yet be observed.
- Next action: rerun browser acceptance when the connector recovers.

## 2026-09-20 — Add remaining administrator retry actions

- Status: source-complete; authenticated Accounts, academic-catalog and Workload assignment evidence remain pending.
- Finding: Accounts, Academic Catalogs and Workload room-search failures still showed passive unavailable text without recovery actions.
- Behavior: each state now exposes a labelled retry link using the current request URL. Account drafts, catalog editors, assignment selections and API identities remain unchanged.
- Files changed: `apps/schedule_web/app/views/admin/accounts/index.html.erb`, `apps/schedule_web/app/views/admin/resources/academic_catalogs/index.html.erb`, `apps/schedule_web/app/views/admin/workload/show/_assignment.html.erb`.
- Source revision: Rails `5a0b227`.
- Runtime/source verification: all three changed ERB templates compiled, Rails Zeitwerk check and `git diff --check` passed in the running `schedule_web` container.
- Routes, role, context, viewport: Accounts, Academic Catalogs and Workload assignment routes, staff administrator; browser retry and mutation/read-back evidence remain pending.
- Table inventory: unchanged; account, catalog and Workload structures retain their existing schemas.
- Known remaining issue: browser transport is unavailable, so retry interactions cannot yet be observed.
- Next action: rerun browser acceptance when the connector recovers.

## 2026-09-20 — Add resource failure retry actions

- Status: source-complete; authenticated resource and Excel failure/read-back evidence remain pending.
- Finding: Buildings, Rooms, Room Types, Posts and Excel file-list failures displayed passive unavailable text without recovery actions.
- Behavior: each unavailable state now exposes a labelled retry link using the current request URL. Resource tables, editor contracts, upload state and file data remain unchanged.
- Files changed: `apps/schedule_web/app/views/admin/resources/buildings/index.html.erb`, `rooms/index.html.erb`, `room_types/index.html.erb`, `posts/index.html.erb`, `apps/schedule_web/app/views/admin/excel_imports/_files.html.erb`.
- Source revision: Rails `f777d0a`.
- Runtime/source verification: all five changed ERB templates compiled, Rails Zeitwerk check and `git diff --check` passed in the running `schedule_web` container.
- Routes, role, context, viewport: resource catalog routes and `/admin/excel_imports`, staff administrator; browser retry and CRUD/upload read-back evidence remain pending.
- Table inventory: unchanged; all resource and import list structures remain intact.
- Known remaining issue: browser transport is unavailable, so retry behavior cannot yet be observed interactively.
- Next action: rerun browser acceptance when the connector recovers.

## 2026-09-20 — Add semester catalog retry action

- Status: source-complete; authenticated semester catalog and creation/read-back evidence remain pending.
- Finding: the Calendar semester page showed a passive unavailable message without recovery, unlike the other dated-rule pages.
- Behavior: the unavailable semester list now exposes a labelled retry action that preserves the current URL and Turbo frame. Semester columns and creation behavior are unchanged.
- Files changed: `apps/schedule_web/app/views/admin/semesters/index.html.erb`.
- Source revision: Rails `2474174`.
- Runtime/source verification: semester ERB compilation, Rails Zeitwerk check and `git diff --check` passed in the running `schedule_web` container.
- Routes, role, context, viewport: `/admin/semesters`, staff administrator, calendar context; browser retry and catalog read-back remain pending.
- Table inventory: unchanged; semester table headers, order and row meaning remain intact.
- Known remaining issue: browser connector is unavailable, so retry behavior cannot yet be observed interactively.
- Next action: continue B04 holiday and calendar-context source verification.

Batch/slice: B01 shared control contract and B02 failed placement draft retention
Status: source-complete; runtime/browser verification pending
Files changed (attributable): `apps/schedule_web/app/assets/stylesheets/components/_admin_tokens.css`, `_admin_components.css`, `_admin_workspace.css`, `_admin_workload.css`, `app/javascript/controllers/admin_context_history_controller.js`, `app/controllers/admin/schedules/events_controller.rb`
Source revision + dirty-scope identifier: Rails `e6560d0`; root pre-existing untracked files were preserved and not staged.
Runtime source/asset verification method: Rails controller syntax, ERB compilation, Zeitwerk loading, JavaScript syntax and `git diff --check` passed. The running browser connector remains unavailable, so computed-style and authenticated mutation evidence are pending.
Routes, role, safe selected context, viewport: Existing staff-only Schedule and Workload routes; runtime role, selected data and viewport remain unverified.
Checks and exact results: Admin ordinary controls now use the prescribed 40px baseline, 14px body text, compact 36px row controls and 44px primary actions; Schedule headings use 24/32 desktop and 20/28 narrow roles; Workload context/editor spacing is reduced after the legacy page rules while its five-column table remains unchanged. Schedule frame history stores the requested same-origin URL before frame navigation and uses the response URL on completion. Failed event creation now redirects with `selected_load` retained so the safe draft remains visible in the same placement inspector.
Browser/mutation/read-back evidence paths: None available; no write was sent.
Table inventory comparison: Schedule time plus weekday columns and Workload `Кафедра`, `Дисциплина и группа`, `Вид занятия`, `Часы`, `Назначение` remain unchanged.
Known remaining issue: The browser connector is still unavailable; full B00/B02 journey, computed control sizes, asset fingerprints and failure read-back remain unverified.
Next concrete action: Continue source inspection and implementation of the next plan delta, then retry authenticated runtime verification when the browser session is available.

Runtime follow-up: Rails image `milk_schedule-schedule_web:latest` was rebuilt successfully and `schedule_web` recreated. Host/container SHA-256 for `_admin_workload.css` matched (`6af84ebf1346bfe41d4d3665b3283a4e13c84730f59111e3a09feab925f1c4f7`). Propshaft served the new `components/_admin_tokens-a2f58e62.css` (`--admin-control: 40px`) and `components/_admin_workload-9c2c278d.css` (B01 authority block). `/up` returned the healthy response; unauthenticated `/admin/schedule`, `/admin/workload`, and `/admin/aggregate_schedule` each returned the expected 302 to `/?auth=1`. Authenticated browser and mutation evidence remain pending.

Runtime follow-up: Playwright MCP recovered for read-only checks. The current browser session had no staff authentication and `/admin/aggregate_schedule` redirected to `/?auth=1`; no credentials were entered. Public routes `/group` and `/group/1` rendered with zero console errors. Inside the Compose network, unauthenticated requests to both aggregate Hanami routes returned HTTP 401 (`{"error":"unauthorized"}`), confirming API-side staff protection. Hanami route loading and syntax checks passed for both occurrence actions and the aggregate repository; Rails Zeitwerk remained green.

Batch/slice: Aggregate schedule shared admin primitive alignment
Status: source-complete; unauthenticated runtime verified, authenticated read-back pending
Files changed (attributable): `apps/schedule_web/app/views/admin/aggregate_schedules/show.html.erb`, `app/assets/stylesheets/components/_admin_workspace.css`
Source revision + dirty-scope identifier: Rails `7ad2b51`; root pre-existing untracked files preserved.
Runtime source/asset verification method: Aggregate ERB compilation and Rails Zeitwerk passed. The Rails image rebuilt successfully, Propshaft served `_admin_workspace-333e6d1b.css`, `/up` returned 200, and unauthenticated `/admin/aggregate_schedule` returned the expected 302 to `/?auth=1`.
Routes, role, safe selected context, viewport: Existing staff-only aggregate route; selected date/faculty/department and viewport remain unauthenticated/unverified.
Checks and exact results: The aggregate page now uses the shared `admin-page__header`, `admin-primary`, and `admin-secondary` primitives. Date/search inputs use the 40px default control height; the seven-column dated report and print behavior remain unchanged.
Browser/mutation/read-back evidence paths: Current Playwright session has no staff authentication; no mutation was sent. Public browser checks had zero console errors.
Table inventory comparison: Aggregate dated report remains seven columns in the same order: time, group, subject, activity, teacher, room, change.
Known remaining issue: Authenticated aggregate selection, transfer/cancellation read-back and print evidence remain pending.
Next concrete action: Continue the plan's remaining source/runtime acceptance work with an authenticated disposable-data session.

Browser follow-up after rebuild: Playwright loaded the public home at `http://localhost:3000/` with HTTP 200, zero console errors/warnings, and all 38 observed document/static requests returned 200, including the current application bundles and `_admin_workspace-333e6d1b.css`. The tab has no authenticated staff session, so protected administrator rendering and mutations remain unclaimed.

Batch/slice: B06 Teachers workspace shared shell alignment
Status: source-complete; runtime asset/liveness verified, authenticated visual evidence pending
Files changed (attributable): `apps/schedule_web/app/assets/stylesheets/components/_admin_teachers.css`
Source revision + dirty-scope identifier: Rails `560316d`; root pre-existing untracked files preserved.
Runtime source/asset verification method: Rails Zeitwerk, Teachers ERB compilation, JavaScript syntax and `git diff --check` passed. The Rails image rebuilt and `schedule_web` was recreated; `/up` returned 200 and Propshaft served `_admin_teachers-121ac18f.css`.
Routes, role, safe selected context, viewport: Existing `/admin/teachers` staff workspace; authenticated role, teacher selection and viewport remain pending.
Checks and exact results: The custom Teachers header now uses the shared 82px desktop shell, 24/32 heading (20/28 narrow), 14px action text, 12px labels and 40px ordinary controls. Constraint and six-day schedule tables were not altered.
Browser/mutation/read-back evidence paths: No authenticated session was available; no mutation was sent.
Table inventory comparison: Teacher availability grid and day schedule preview retain their existing axes, rows and columns.
Known remaining issue: Authenticated computed-style, keyboard and narrow viewport evidence remains pending.
Next concrete action: Continue B06 Notice/adjacent-page consistency and retry the authenticated browser journey.

Batch/slice: B06 Notice workspace shared shell alignment
Status: source-complete; runtime asset/liveness verified, authenticated visual and print evidence pending
Files changed (attributable): `apps/schedule_web/app/assets/stylesheets/components/_admin_notice.css`
Source revision + dirty-scope identifier: Rails `af6aacf`; root pre-existing untracked files preserved.
Runtime source/asset verification method: Notice ERB compilation, Rails Zeitwerk and `git diff --check` passed. The Rails image rebuilt and `schedule_web` was recreated; `/up` returned 200 and Propshaft served `_admin_notice-403fe669.css`.
Routes, role, safe selected context, viewport: Existing `/admin/notice` staff workspace; authenticated teacher/semester context, desktop/narrow rendering and print remain pending.
Checks and exact results: Notice context/report headers, labels, links and actions now use the shared 40px/14px control rhythm while the five-column notice report and print-specific table rules remain unchanged.
Browser/mutation/read-back evidence paths: No authenticated session was available; no export mutation was sent.
Table inventory comparison: Notice retains its existing `Число`, `Часы`, `Поток, группа`, `Вид занятий`, `Аудитория` columns and print semantics.
Known remaining issue: Authenticated responsive and print evidence remains pending.
Next concrete action: Continue final adjacent-page consistency and retry the authenticated browser journey.

Batch/slice: B06 adjacent-page control targets
Status: source-complete; runtime asset/liveness verified, authenticated visual evidence pending
Files changed (attributable): `apps/schedule_web/app/assets/stylesheets/components/_admin_workspace.css`
Source revision + dirty-scope identifier: Rails `3938377`; root pre-existing untracked files preserved.
Runtime source/asset verification method: Rails Zeitwerk, Accounts and period-settings ERB compilation and `git diff --check` passed. The Rails image rebuilt and `schedule_web` was recreated; `/up` returned 200 and Propshaft served `_admin_workspace-1f9d99b9.css`.
Routes, role, safe selected context, viewport: Existing Accounts, Calendar period and Observability admin routes; authenticated context and viewport remain pending.
Checks and exact results: Scoped final rules raise Accounts filters, Calendar period controls, account search controls and monitoring service links to the shared 40px target; labels use 12px/16px. Existing table structures and monitoring data semantics were unchanged.
Browser/mutation/read-back evidence paths: No authenticated session was available; no mutation was sent.
Table inventory comparison: No table columns, order, grouping or report semantics changed.
Known remaining issue: Authenticated responsive and focus evidence remains pending.
Next concrete action: Retry staff browser access and complete the remaining B00/B02 mutation/read-back gate.

Batch/slice: B01/V02 Workload committed-context disclosure
Status: source-complete; runtime asset/liveness verified, authenticated interaction evidence pending
Files changed (attributable): `apps/schedule_web/app/views/admin/workload/show/show.html.erb`, `app/assets/stylesheets/components/_admin_workload.css`
Source revision + dirty-scope identifier: Rails `e648166`; root pre-existing untracked files preserved.
Runtime source/asset verification method: Workload ERB compilation, Rails Zeitwerk and `git diff --check` passed. The Rails image rebuilt and `schedule_web` was recreated; `/up` returned 200 and Propshaft served `_admin_workload-f540cb0d.css`.
Routes, role, safe selected context, viewport: Existing `/admin/workload` staff workspace; authenticated context and viewport remain pending.
Checks and exact results: Once semester/faculty/department or teacher context is valid, the page presents one readable Russian summary band and an explicit `Изменить контекст` disclosure. Existing searchable semester, faculty, department, teacher and group controls remain inside the disclosure; workload table columns and request parameters are unchanged.
Browser/mutation/read-back evidence paths: No authenticated session was available; no mutation was sent.
Table inventory comparison: Workload keeps `Кафедра`, `Дисциплина и группа`, `Вид занятия`, `Часы`, `Назначение` in the same order.
Known remaining issue: Browser computed-height, disclosure keyboard behavior and full workload-to-schedule read-back remain pending.
Next concrete action: Retry staff browser access and verify context disclosure plus the complete B02 journey.

## 2026-09-20 — Unify Calendar workspace navigation

- Status: source-complete; authenticated Calendar navigation and CRUD read-back evidence remain pending.
- Finding: the five Calendar workspaces duplicated their destination navigation, allowing active-state and transfer-route behavior to drift between pages.
- Behavior: a scoped `admin/calendar_navigation` partial now owns the five Calendar destinations, shared calendar context links, Turbo frame targeting and active-page semantics. Periods, semesters, reservations, holidays and weekday transfers pass their active key; the weekday-transfer destination consistently opens day-transfer mode.
- Files changed: `apps/schedule_web/app/views/admin/_calendar_navigation.html.erb`, `holidays/index.html.erb`, `period_settings/show/show.html.erb`, `room_reservations/index.html.erb`, `semesters/index.html.erb`, `weekday_transfers/index.html.erb`.
- Source revision: Rails `8a374ef`.
- Runtime/source verification: Rails image rebuilt, Propshaft precompiled, schedule_web recreated, Rails Zeitwerk check passed, and all six changed ERB templates compiled in the rebuilt container. The container reported healthy status; host `/up` was temporarily answered by the local proxy with HTTP 502 despite the healthy container.
- Routes, role, safe selected context, viewport: existing staff-only Calendar routes and session calendar context; authenticated navigation and responsive evidence remain pending.
- Table inventory: unchanged; period, semester, reservation, holiday and transfer table structures remain intact.
- Known remaining issue: current Playwright session has no staff authentication, and the local host proxy did not return `/up` during this rebuild check.
- Next concrete action: retry authenticated Calendar navigation and the B04 create/delete/read-back journey when the staff browser session is available.

## 2026-09-20 — Preserve selected semester in period editors

- Status: source-complete; authenticated invalid-period and read-back evidence remain pending.
- Finding: the period page and failed-save renderer always resolved the remembered/default semester, even when a valid `semester_id` was present in the URL or submitted form. A failed editor could therefore reload groups from a different semester while retaining the submitted dates.
- Behavior: both controllers now prefer the submitted semester when it exists in the loaded catalog, then fall back to the existing remembered-semester resolver only when the request is absent or invalid. Group-semester identity and safe period drafts remain tied to the selected context.
- Files changed: `apps/schedule_web/app/controllers/admin/period_settings/show_controller.rb`, `periods_controller.rb`.
- Source revision: Rails `d5eff57`.
- Runtime/source verification: Rails image rebuilt, both changed controllers returned `Syntax OK`, Rails Zeitwerk check passed, and the recreated `schedule_web` container became healthy.
- Routes, role, safe selected context, viewport: existing staff-only period routes; authenticated invalid-range preservation and responsive evidence remain pending.
- Table inventory: unchanged; period editor units and calendar tables retain their existing structure.
- Known remaining issue: current browser session has no staff authentication, so invalid submission and authoritative API read-back cannot yet be observed interactively.
- Next concrete action: continue B04 reservation and dated-transfer contract inspection, then retry authenticated period acceptance.

## 2026-09-20 — Preserve reservation drafts during search

- Status: source-complete; authenticated reservation search, conflict and read-back evidence remain pending.
- Finding: the create reservation sheet used separate GET forms for room and teacher searches. Those requests carried the search text but omitted the current room, teacher, date, time-slot and purpose fields, so a Turbo response could silently reset selections already made in the editor.
- Behavior: a scoped `reservation-draft` Stimulus controller captures the editor values into the search request. The controller restores a permitted draft on the server before rendering the reservation frame, while the existing room/teacher IDs and API contract remain unchanged.
- Files changed: `apps/schedule_web/app/javascript/controllers/reservation_draft_controller.js`, `app/controllers/admin/room_reservations_controller.rb`, `app/views/admin/room_reservations/index.html.erb`.
- Source revision: Rails `ecb1a20`.
- Runtime/source verification: JavaScript syntax, controller syntax, reservation ERB compilation, Rails Zeitwerk check and `git diff --check` passed. Rails image rebuilt and the recreated `schedule_web` container became healthy.
- Routes, role, safe selected context, viewport: existing staff-only reservation create/edit routes; authenticated search interaction and responsive evidence remain pending.
- Table inventory: unchanged; reservation columns, row meaning and pagination remain intact.
- Known remaining issue: current browser session has no staff authentication, so the search-preservation behavior and conflict read-back cannot yet be observed interactively.
- Next concrete action: inspect weekday-transfer creation and deletion context, then retry the authenticated reservation journey.

## 2026-09-20 — Normalize global weekday transfer scope

- Status: source-complete; authenticated global/group transfer create-delete and occurrence read-back evidence remain pending.
- Finding: the explicit `Все группы` option submitted an empty `student_group_id`, and the Rails controller passed that blank value to the API integer contract. Global transfers could therefore be rejected as invalid instead of using the supported nullable scope.
- Behavior: weekday-transfer attributes now compact blank values before the gateway call. The global option reaches the API as an omitted/nil group scope, while selected groups retain their integer identity.
- Files changed: `apps/schedule_web/app/controllers/admin/weekday_transfers_controller.rb`.
- Source revision: Rails `ab1e5ea`.
- Runtime/source verification: Rails image rebuilt, controller syntax returned `Syntax OK`, Rails Zeitwerk check passed, and the recreated `schedule_web` container became healthy.
- Routes, role, safe selected context, viewport: existing staff-only weekday-transfer create/delete route; authenticated scope and occurrence evidence remain pending.
- Table inventory: unchanged; transfer columns and pagination remain intact.
- Known remaining issue: current browser session has no staff authentication, so API validation and schedule read-back cannot yet be observed interactively.
- Next concrete action: continue the B04 occurrence/read-back contract audit, then retry authenticated Calendar acceptance.

## 2026-09-20 — Respect group-scoped transfers on holidays

- Status: source-complete; authenticated occurrence read-back remains pending.
- Finding: occurrence expansion skipped a holiday only when there were no transfers at all. A group-scoped transfer on a holiday therefore allowed unaffected groups to match their ordinary weekday schedule.
- Behavior: holiday filtering now runs per template. A global weekday transfer permits all groups; a group-scoped transfer permits only its matching group; unaffected groups remain suppressed for the holiday. Existing weekday precedence and dated event-transfer handling are unchanged.
- Files changed: `apps/api_geteway/lib/api_geteway/queries/schedule_occurrences.rb`.
- API source revision: `0d91ab3`.
- Runtime/source verification: API syntax returned `Syntax OK`, the Hanami image rebuilt, the API container was recreated and became healthy, and `git diff --check` passed.
- Routes, role, safe selected context, viewport: existing staff/public occurrence consumers; authenticated or seeded occurrence comparisons remain pending.
- Table inventory: unchanged; no UI table or report structure changed.
- Known remaining issue: the current browser session has no staff authentication and no disposable seeded holiday/group-transfer read-back has been executed.
- Next concrete action: continue API occurrence contract inspection for cancellation/move precedence, then run authenticated schedule acceptance when the session is available.

## 2026-09-20 — Add Student Groups catalog retry

- Status: source-complete; authenticated catalog failure and CRUD read-back evidence remain pending.
- Finding: Student Groups was the remaining high-cardinality resource page whose dependency failure state only displayed passive text and offered no recovery action.
- Behavior: the failure state now exposes a labelled retry link targeting the existing `resources-workspace` Turbo frame and preserving the current URL. Group selectors, pagination and CRUD contracts remain unchanged.
- Files changed: `apps/schedule_web/app/views/admin/student_groups/index.html.erb`.
- Source revision: Rails `6e52ff8`.
- Runtime/source verification: Rails image rebuilt, the changed ERB compiled in the container, Rails Zeitwerk check passed, and the recreated `schedule_web` container became healthy.
- Routes, role, safe selected context, viewport: existing staff-only Student Groups catalog; authenticated retry and read-back evidence remain pending.
- Table inventory: unchanged; group columns, row meaning and pagination remain intact.
- Known remaining issue: current browser session has no staff authentication, so the retry state cannot yet be observed interactively.
- Next concrete action: continue the resource/catalog failure-state sweep, then retry authenticated B03/B05 acceptance.

## 2026-09-20 — Add group-semester catalog retry

- Status: source-complete; authenticated group-semester failure and edit/read-back evidence remain pending.
- Finding: the Student Group Semesters detail page still rendered a passive dependency failure without a recovery action.
- Behavior: the failure state now exposes a labelled retry link targeting the existing `resources-workspace` Turbo frame and preserving the current group URL.
- Files changed: `apps/schedule_web/app/views/admin/student_groups/semesters.html.erb`.
- Source revision: Rails `3e686e0`.
- Runtime/source verification: Rails image rebuilt, the changed ERB compiled in the container, Rails Zeitwerk check passed, and the recreated `schedule_web` container became healthy.
- Routes, role, safe selected context, viewport: existing staff-only group-semester route; authenticated retry and edit/read-back evidence remain pending.
- Table inventory: unchanged; group-semester columns, row meaning and editor behavior remain intact.
- Known remaining issue: current browser session has no staff authentication, so the retry state cannot yet be observed interactively.
- Next concrete action: continue the remaining adjacent-page failure-state sweep, then retry authenticated B03/B05 acceptance.

## 2026-09-20 — Collapse Schedule inspector at intermediate widths

- Status: source-complete; authenticated computed-layout and placement journey evidence remain pending.
- Finding: the Schedule planner kept its optional 300px inspector in a three-column layout through 1280px viewports. With the dark admin sidebar present, this left the timetable with an unnecessarily narrow working region even though the legacy collapse breakpoint was 1180px.
- Behavior: at 1181–1360px the planner keeps the source-load rail and timetable columns, then moves the optional inspector below the grid. The existing two-column and mobile tiers remain unchanged; the inspector is still scrollable/sticky at wider desktop widths.
- Files changed: `apps/schedule_web/app/assets/stylesheets/components/_admin_workspace.css`.
- Source revision: Rails `83c88f9`.
- Runtime/source verification: Rails image rebuilt with Propshaft precompile, the served asset was fingerprinted as `_admin_workspace-fafa21d8.css`, Rails Zeitwerk check passed, and the recreated `schedule_web` container became healthy.
- Routes, role, safe selected context, viewport: existing staff-only Schedule route; authenticated 1280px/1360px computed-layout and mutation evidence remain pending.
- Table inventory: unchanged; Schedule time and weekday columns remain in the same table and retain local horizontal scrolling.
- Known remaining issue: current browser session has no staff authentication, so the intermediate-width layout cannot yet be inspected interactively.
- Next concrete action: continue the responsive B01/B02 source pass and retry the authenticated Schedule placement journey.

## 2026-09-20 — Retain staged Workload rooms during search

- Status: source-complete; authenticated assignment search, save and read-back evidence remain pending.
- Finding: the Workload assignment sheet replaced its room fieldset through a Turbo frame, but the room search request omitted rooms already checked in the draft. A search could therefore silently remove a staged room before the collection-replacing assignment PUT.
- Behavior: a scoped `assignment-search-context` controller carries checked `room_ids[]` into `selected_room_ids[]`. The Workload presenter restores those IDs as retained room choices after the search, and the final form submits the complete retained collection. Existing teacher selections, room authorization and API contracts remain unchanged.
- Files changed: `apps/schedule_web/app/javascript/controllers/assignment_search_context_controller.js`, `app/controllers/admin/workload/show_controller.rb`, `app/lib/presenters/admin/workload_workspace.rb`, `app/views/admin/workload/show/_assignment.html.erb`.
- Source revision: Rails `8f014e8`.
- Runtime/source verification: JavaScript syntax, controller syntax, presenter syntax, assignment ERB compilation and Rails Zeitwerk check passed. The Rails image rebuilt with Propshaft precompile and the recreated `schedule_web` container became healthy.
- Routes, role, safe selected context, viewport: existing staff-only Workload assignment route; authenticated search/save/read-back and responsive evidence remain pending.
- Table inventory: unchanged; Workload retains its five-column report table and assignment semantics.
- Known remaining issue: current browser session has no staff authentication, so staged-room preservation cannot yet be observed interactively.
- Next concrete action: continue the B02 assignment draft audit for teacher/room collection retention, then retry authenticated workload-to-schedule acceptance.

Follow-up: Rails `81f703c` removes previously injected `selected_room_ids[]` fields before each repeated room search, preventing duplicate query values while preserving the same retained collection behavior. JavaScript syntax and `git diff --check` passed.

## 2026-09-20 — Preserve Workload editor drafts during subject search

- Status: source-complete; authenticated editor search, validation and read-back evidence remain pending.
- Finding: the Workload edit sheet’s subject search navigated the whole workspace without carrying unsaved activity type, split strategy or hour values. Searching could therefore discard a dirty editor before the user submitted the PATCH.
- Behavior: a scoped `workload-editor-draft` controller serializes the current subject, activity, split and hour fields into the search request. The controller and presenter restore those values after the result reload, while the update operation and subject identity contract remain unchanged.
- Files changed: `apps/schedule_web/app/javascript/controllers/workload_editor_draft_controller.js`, `app/controllers/admin/workload/show_controller.rb`, `app/lib/presenters/admin/workload_workspace.rb`, `app/views/admin/workload/show/_edit_record.html.erb`.
- Source revision: Rails `8712498`.
- Runtime/source verification: JavaScript syntax, controller syntax, presenter syntax, edit-template ERB compilation and Rails Zeitwerk check passed. The Rails image rebuilt with Propshaft precompile and the recreated `schedule_web` container became healthy.
- Routes, role, safe selected context, viewport: existing staff-only Workload edit route; authenticated draft/search/PATCH/read-back and responsive evidence remain pending.
- Table inventory: unchanged; Workload retains its five-column report table and row semantics.
- Known remaining issue: current browser session has no staff authentication, so dirty-editor preservation cannot yet be observed interactively.
- Next concrete action: inspect Workload creation search for the same draft-loss pattern, then retry the authenticated B02 journey.

## 2026-09-20 — Preserve Workload creation drafts during search

- Status: source-complete; authenticated creation search, POST validation and read-back evidence remain pending.
- Finding: the Workload creation sheet’s subject search navigated the full workspace while selected groups, activity type, split mode and hour values remained only in the form DOM. Searching could discard a dirty creation draft.
- Behavior: the existing `workload-editor-draft` bridge now carries creation fields and checked groups through `editor_draft[]` parameters. The presenter restores those values, including an explicit empty-group selection, before the create form is rendered again. The create operation and group identity contract remain unchanged.
- Files changed: `apps/schedule_web/app/javascript/controllers/workload_editor_draft_controller.js`, `app/controllers/admin/workload/show_controller.rb`, `app/lib/presenters/admin/workload_workspace.rb`, `app/views/admin/workload/show/_create_record.html.erb`.
- Source revision: Rails `ae9cc47`.
- Runtime/source verification: JavaScript syntax, controller syntax, presenter syntax, create/edit ERB compilation and Rails Zeitwerk check passed. The Rails image rebuilt with Propshaft precompile and the recreated `schedule_web` container became healthy.
- Routes, role, safe selected context, viewport: existing staff-only Workload creation route; authenticated draft/search/POST/read-back and responsive evidence remain pending.
- Table inventory: unchanged; Workload retains its five-column report table and row semantics.
- Known remaining issue: current browser session has no staff authentication, so creation draft preservation cannot yet be observed interactively.
- Next concrete action: retry authenticated B02 creation and edit journeys when a staff browser session is available.

## 2026-09-20 — Align Rooms resource table tracks

- Status: source-complete; authenticated Rooms rendering and CRUD evidence remain pending.
- Finding: Rooms renders four report columns, but inherited the generic three-track resource grid. The final action cell therefore created an implicit track and could misalign headers, values and actions.
- Behavior: the Rooms resource table now declares four explicit tracks with a bounded local minimum width: building, room type, room identity and actions. Existing column order, row meaning and horizontal access remain unchanged.
- Files changed: `apps/schedule_web/app/assets/stylesheets/components/_admin_resources.css`.
- Source revision: Rails `4fd7465`.
- Runtime/source verification: Rails image rebuilt with Propshaft precompile, Rails Zeitwerk check passed, and the recreated `schedule_web` container was healthy after startup.
- Routes, role, safe selected context, viewport: existing staff-only Rooms catalog; authenticated computed-track and CRUD evidence remain pending.
- Table inventory: unchanged; Rooms retains its four columns in the existing order.
- Known remaining issue: current browser session has no staff authentication, so rendered track alignment cannot yet be observed interactively.
- Next concrete action: continue the remaining Resources track audit, then retry authenticated B03 acceptance.

## 2026-09-20 — Keep Account search actions in normal flow

- Status: source-complete; authenticated Account sheet geometry and update evidence remain pending.
- Finding: the Account editor sticky-submit selector matched every nested form, including GET teacher and department search forms. Search buttons could therefore become competing sticky actions inside the modal body.
- Behavior: only non-GET mutation forms retain the sticky primary action. Search controls remain in normal document flow while the save and password-generation actions stay reachable in long editors.
- Files changed: `apps/schedule_web/app/assets/stylesheets/components/_admin_accounts.css`.
- Source revision: Rails `f7ac612`.
- Runtime/source verification: Rails image rebuilt with Propshaft precompile, Rails Zeitwerk check passed, and the recreated `schedule_web` container became healthy.
- Routes, role, safe selected context, viewport: existing staff-only Accounts routes; authenticated modal geometry, keyboard and update evidence remain pending.
- Table inventory: unchanged; Accounts retains its six-column report.
- Known remaining issue: current browser session has no staff authentication, so computed modal behavior cannot yet be observed interactively.
- Next concrete action: continue B05 keyboard/focus source verification, then retry authenticated Account acceptance.

## 2026-09-21 — Authenticated core journey and schedule inspector fixes

- Status: authenticated browser acceptance resumed with the supplied staff account; the core read/edit path now renders successfully.
- Evidence: `/admin/workload` loaded semester 1 and the Construction faculty with three workload rows. `/admin/schedule` opened group СИ-1 with existing lessons and the load rail; a direct event URL rendered the inspector with teacher, room, edit, transfer and swap controls. Browser console errors were zero after the fix. `/admin/aggregate_schedule?faculty_id=1&semester_id=1` resolved the faculty context and displayed two dated occurrences for 21.09.2026.
- Fixes: moved `event_entry` initialization outside the nested form block and corrected the swap target `select_tag` call, removing two 500 paths when opening an event inspector. Aggregate schedule now uses the requested `faculty_id` when the displayed URL omits `scope_id`, preventing an UNKNOWN/ID-0 context on fresh navigation.
- Files changed: `apps/schedule_web/app/views/admin/schedules/show/show.html.erb`, `apps/schedule_web/app/lib/operations/admin/load_aggregate_schedule.rb`.
- Source revisions: Rails `bb15593`, `3b3809b`, `3c68eae`.
- Runtime verification: Rails image rebuilt with Propshaft precompile, `schedule_web` recreated healthy, API and PostgreSQL healthy, and authenticated Playwright read-back completed on 127.0.0.1. No credentials were recorded in evidence or documentation.
- Remaining acceptance: no destructive mutation was performed against fixture data. A disposable workload create/assignment and lesson transfer/edit should be exercised next, followed by responsive computed-layout checks for Schedule, Workload, Resources and Accounts.

## 2026-09-21 — Disposable workload-to-schedule journey and responsive shell pass

- Status: B00/B02 functional anchor completed against disposable records and cleaned up after read-back.
- Journey evidence: created temporary workload requirement `5` for group СИ-1, assigned Кожедуб Сергей Сергеевич, selected it in Schedule, placed event `9` on Monday slot 4, edited the same event to Tuesday slot 5 with status “Изменено”, reloaded the direct event URL, and confirmed the same event ID, teacher, day, time and status. The remaining load changed from 2/2 to 0, then returned after the event and requirement were deleted. The created requirement and event are absent after cleanup.
- Responsive evidence: at 390×844 Schedule and Workload keep document width at 390px; Workload retains its 850px five-column table inside the labelled local scroller. Accounts keeps 390px document width, 20px heading, 44px primary controls and no page overflow. At 1280×900 Accounts keeps the role options in a local horizontal rail and document width at 1280px instead of expanding to 1419px.
- Fixes: the final shared admin skin now applies the 20px narrow heading role after domain styles, and the desktop Accounts role/status option rail is bounded to its own scroller.
- Files changed: `apps/schedule_web/app/assets/stylesheets/components/_admin_studio.css`, `apps/schedule_web/app/assets/stylesheets/components/_admin_accounts.css`.
- Source revisions: Rails `924e6a2`, `68a4176`.
- Runtime verification: Rails image rebuilt with Propshaft precompile, `schedule_web` recreated healthy, Rails Zeitwerk check passed, and authenticated Playwright console errors remained at zero on the verified pages. No fixture records remain from the journey.
- Remaining acceptance: complete the calendar/resource/account failure-state and print matrix, then rerun the core journey after those batches. Performance and availability optimization remain deferred by instruction.

## 2026-09-21 — Calendar and resource browser acceptance

- Status: B03/B04 read and disposable transfer acceptance advanced.
- Evidence: authenticated `/admin/rooms` and `/admin/subjects` render their list-first resource workspaces with explicit navigation and action controls. `/admin/operations`, `/admin/operations/semesters`, `/admin/holidays`, `/admin/room_reservations` and `/admin/transfers?mode=days` all load through the shared Calendar shell. Reservations now expose searchable room/teacher controls and an empty result state instead of the historical unavailable-data page.
- Transfer journey: created a temporary global weekday transfer for 2026-10-01, verified the five-column row (`Дата`, `Следует дню`, `Группа`, `Причина`, `Действия`), then deleted it through the explicit confirmation path. The list returned to “Переносы не добавлены.” and no browser errors occurred.
- Remaining acceptance: resource CRUD failure/read-back, period validation persistence, reservation mutation/conflict handling, account failure states and Notice/print verification remain pending. Performance and availability optimization remain deferred by instruction.

## 2026-09-21 — Subject resource create/delete read-back

- Status: authenticated subject resource mutation path verified with a disposable record.
- Evidence: `/admin/subjects` opened the Russian editor with full and short-name fields. A temporary `Codex Acceptance Subject 20260921` / `CAS-0921` subject was created and the success state reported “Дисциплина создано.”; it was then removed through the authenticated DELETE route and confirmed absent from PostgreSQL. Browser console errors remained at zero.
- Scope: this validates the existing list-first editor and mutation contract without changing fixture subjects. No new test files were added.
- Remaining acceptance: resource update and failure-state read-back, period validation persistence, reservation mutation/conflict handling, account failure states and Notice/print verification remain pending. Performance and availability optimization remain deferred by instruction.

## 2026-09-21 — Reservation read and mutation hardening

- Status: B04 reservation list/create/edit/delete path runtime-verified after targeted fixes.
- Findings fixed: the new reservation teacher picker lacked its live status target and raised a Stimulus error; failed reservation submissions targeted an inner Turbo frame and produced a missing-frame error instead of preserving the editor; fresh reservation navigation reopened the editor because an empty draft hash was treated as a failed draft; Hanami serialization used Rails-only `compact_blank`; and cancelled reservations remained in the active admin list.
- Behavior: teacher selection now announces the chosen person without console errors, mutation forms render full-page failure feedback with preserved draft data, clean navigation leaves the sheet closed, reservation reads serialize correctly, and soft-cancelled rows disappear from the active list while remaining stored as cancelled history.
- Files changed: `apps/schedule_web/app/views/admin/room_reservations/index.html.erb`, `app/controllers/admin/room_reservations_controller.rb`, `app/javascript/controllers/admin_sheet_controller.js`, `apps/api_geteway/slices/admin/serializers/room_reservation_serializer.rb`, `apps/api_geteway/slices/admin/repos/room_reservation_repo.rb`.
- Source revisions: Rails `7f05b07`, `6cfb405`; API `005c2b2`.
- Runtime evidence: created reservation `2026-10-15` in room 173 for Кожедуб Сергей Сергеевич, edited its purpose, deleted it, and confirmed the active list returned to the empty state. Browser console errors were zero after the fixes; the API and Rails containers rebuilt healthy.
- Conflict evidence: a disposable reservation on 2026-10-16/slot 1 was created, a duplicate attempt returned the Russian conflict message while preserving the editor draft and leaving one active row, and the anchor was then soft-cancelled. The API create response now hydrates associations before serialization, so successful writes no longer appear as generic failures after persistence.
- Additional source revision: API `a9bc2a0`.
- Period evidence: `/admin/operations?faculty_id=1&semester_id=1&group_id=1` rejected an inverted disposable lecture-week range while preserving both submitted dates, accepted and read back a temporary 2028-01-01…2028-01-07 period, then deleted it through the explicit confirmation path. The existing semester period remained unchanged; console errors were zero.

## 2026-09-21 — Account edit contract alignment

- Status: authenticated account edit read-back verified.
- Finding: the Accounts editor submitted empty optional `password` and `teacher_id` values. Hanami’s update contract correctly rejects blank optional values, so saving an unchanged global administrator returned 422 even though the UI supplied no changes.
- Behavior: Rails now removes blank optional fields at the request boundary and only renders the existing teacher identity field when a teacher is actually linked. The member PATCH route remains unchanged and role/scope authorization stays server-side.
- Source revision: Rails `249e07f`.
- Runtime evidence: saved the existing super-administrator record without changing its identity or credentials; API returned 200, the editor closed, the account remained readable, and browser console errors were zero.
- Remaining acceptance: invalid account draft feedback, generated one-time credential flow and scoped-role coverage remain pending. Password values were not captured.

- Follow-up: the Accounts index now renders shared success/error feedback. Repeating the unchanged edit after this fix displayed “Учётная запись обновлена.”, closed the editor, and produced zero browser errors. Source revision: Rails `1bd0c38`.
- Remaining acceptance: explicit conflict rejection and failed reservation draft read-back, period validation persistence, account failure states and Notice/print verification remain pending. Performance and availability optimization remain deferred by instruction.
