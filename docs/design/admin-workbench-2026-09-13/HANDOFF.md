# Administrator design handoff

Updated: 2026-09-13 after authenticated Playwright MCP inspection and the focused UI pass. Existing UI implementation is partial and already present in the dirty worktrees. Read the current status below before the preserved historical entries.

## Current continuation — browser evidence takes precedence

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
