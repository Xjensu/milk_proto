# Administrator workbench implementation plan

Date: 2026-09-13. Execute only when the user requests implementation. The current analysis task creates this plan and its handoff, not the UI changes.

Read [DESIGN_SYSTEM.md](DESIGN_SYSTEM.md) as the fixed design contract and [AUDIT.md](AUDIT.md) for source findings. Work directly without agents. Do not add tests/test files. Keep the Russian interface and all table schemas.

## Current starting point after browser inspection

Read [BROWSER_AUDIT.md](BROWSER_AUDIT.md) before estimating or implementing these batches. UI changes already exist across B01–B05; their source-complete history does not pass the gates below. Do not recreate the now-loaded token/component/domain stylesheets. Do not reimplement the verified Workload narrow table fix or remove the new Accounts searches. This plan remains the batch order, with the following concrete unfinished deltas:

| Batch | Current evidence -> required delta |
| --- | --- |
| B00 | Authenticated read-only inspection and sampled Rails provenance obtained -> verify current full journey and API provenance; do not repeat login setup as the task |
| B01 | V01/V02/V08 -> reconcile competing heading/control rules; default 40px controls/14px text; compact committed context and narrow shell; actual computed-style acceptance |
| B02 | V03/V04 -> persist committed Schedule group context to browser history; real Workload sheet; verify existing assignment/drag/edit code and safe failure retention |
| B03 | V05 -> move resource creation/editing out of table tracks, align actual schemas, inspect Rooms/Subjects and verify one complete CRUD journey |
| B04 | V06 -> replace long context selects/raw group field, fix five-column transfer tracks, identify reservation failing read before accepting its editor, verify dated/period semantics |
| B05 | V07 -> move account editor out of overlapping action-cell popup; reuse person/scope search and draft code; verify role fields, failures and read-back |
| B06 | V08 -> normalize Teachers/Notice sizes, secondary monitoring disclosure, active narrow navigation, all required widths/roles/print |

For each delta: reproduce the named finding, trace the owning source, change the smallest relevant slice, serve the changed assets, verify the same state, then record resolved/pending evidence. Do not turn observed fixture peculiarities into a database-cleanup task. An empty current semester and unavailable reservations are different states.

## 1. Delivery strategy

Sequence: B00 baseline -> B01 shared system on accepted pages -> B02 complete workload/assignment/placement/edit journey -> B03 Resources -> B04 Calendar -> B05 Accounts -> B06 adjacent pages and final verification.

This order deliberately derives reusable controls from accepted Schedule/Workload rather than copying the anti-reference pages. B01 is not a license to refactor every stylesheet at once. B02 is split into sequential slices; finish each complete operation before adding another kind of drag.

If a required API capability is missing, implement the smallest contract-backed change for that slice. Do not start a generic API modernization. If a runtime defect blocks the active slice, fix it before visual expansion. If a different page has an unrelated defect, record it and continue the selected slice.

## 2. B00 — establish a truthful baseline

### Read and record

- Root and nested AGENTS.md; package HANDOFF.md; current routes and relevant templates.
- Independent Git status for root, Rails and API; current HEADs and dirty file names without dumping private file contents.
- Existing runtime instructions in `apps/schedule_web/docs/handoff/OPERATIONS.md` and `ASTRA_WORKING_SETUP.md`. These may describe an older runtime; verify before following a launch assumption.
- Source revision actually serving Rails/API, runtime mode (host/container), and current CSS/JS fingerprints. Compare a few nonsecret source hashes in the served runtime where no revision endpoint exists. Do not introduce a deployment subsystem just to capture this.
- Baseline table inventory: route, template, ordered headers, row grouping/merged cells, active variants/modes, responsive behavior, print semantics where applicable. Explicitly distinguish current source from externally supplied specification.

### Browser evidence

Capture Schedule, both Workload modes, Teachers, Notice, representative Resources (subjects/rooms), all five Calendar destinations, Accounts, Overview and Observability as read-only baseline where authorized. Record missing role access as “not accessible for this role,” not absent functionality. Use non-sensitive data; never capture generated passwords.

Start with 1280x900 and 390x844 screenshots, then add 768x1024, 1440x900 and a 320px check where layout needs them. Inspect computed heading/body/control fonts, failed assets and console errors. Do not recreate the reported unstyled appearance by deliberately disabling CSS and call it a baseline.

### Functional anchor

On disposable, clearly identified data in an authorized environment: choose faculty/semester/group -> create workload -> assign a teacher with optional room -> place lesson -> edit same lesson -> reload/read back. Record IDs privately as needed; document only safe evidence. Revert only records created for this verification if their dependencies make cleanup safe; never reset storage or bulk-delete fixtures.

If browser access is unavailable, record the exact failure and continue safe source preparation/documentation. Do not mark B00's runtime gate passed or silently substitute a liveness check for a journey. Do not bypass authentication or inspect saved browser sessions to manufacture access.

Exit: known working baseline or a concrete reproduced defect/blocker; current table inventory and source provenance; screenshots or explicit pending evidence items. No new product code required merely for this baseline.

## 3. B01 — shared primitives plus Schedule/Workload consistency

### Primary files

`apps/schedule_web/app/assets/stylesheets/application.css`

`app/assets/stylesheets/components/_admin_workspace.css`, `_admin_workload.css`

`app/views/layouts/admin.html.erb`, `app/views/admin/shared/`

`app/views/admin/schedules/show/show.html.erb`, `app/views/admin/workload/show/show.html.erb`

Paths following the first entry are relative to `apps/schedule_web`.

### Steps

1. Reconcile existing `_admin_tokens.css` with scoped type/spacing/control roles from DESIGN_SYSTEM.md. Retain its import before consuming admin CSS. Do not recolor root/public tokens or rename equivalent aliases without need.
2. Complete existing `_admin_components.css` for actual shared primitives. First consumers are Schedule and Workload: page header, button, labelled field and feedback. Use explicit small shared partials where markup is truly common.
3. Inspect remaining token references and computed control styles; scoped font inheritance already exists. Move rules only when their replacement is present. Avoid global search/replace of common variables used by public components.
4. Bring Schedule/Workload headings, filters, labels, buttons and notices to the shared roles. Preserve their composition and data columns. Do not remove required content to achieve equal heights.
5. Normalize selected/error/focus/pending states; place flash feedback inside the relevant frame for both full requests and Turbo responses without duplication.
6. Preserve Workload's already-corrected narrow scrolling table wrapper and all five columns. Verify other modes, timetable, required widths and 200% zoom; do not claim all responsive states passed from the one audited sample.
7. If extracting shell styles from `_admin_workspace.css`, move a coherent exact rule set and its media rules; do not leave a second competing definition below it.

Exit evidence: same computed page heading and control roles on Schedule and Workload; all columns retained; no public-home/sidebar regression; no missing asset imports; click create/edit still works and retains data. Compare both consuming pages plus one public page after any root reset/import change.

## 4. B02 — one complete workbench journey

### B02a: shared entity selection and assignment editor

Read `app/views/admin/workload/show/_create_record.html.erb`, `_edit_record.html.erb`, `_assignment.html.erb`; `app/lib/contracts/`; `Operations::Admin::SaveWorkloadAssignment`; `Gateways::Admin::WorkloadGateway`; `entity_search_controller.js`.

Read API `slices/admin/contracts/study_load_requirements/replace_assignments_contract.rb`, action `replace_assignments.rb`, `repos/teaching_assignment_repo.rb`, and applicable authorization rules.

Implement visible search/list/selected-summary primitive without changing request identity. Keep group-semester context, stream meaning, multi-teacher/multi-room selections and optional-room behavior. Search frame updates must not replace/reset the assignment form. Add typed drag for a membership/room only after click add/remove/save works. A drop adds to the draft; the final PUT sends retained selections too.

Migrate drawer styling/semantics into the common editor shell. Keep all safe draft input on validation/network/conflict. Preserve identity after editing an existing requirement; do not create a new requirement as an edit shortcut.

Acceptance: create workload; choose multiple groups where existing contract supports it; edit hours safely; choose two teacher memberships; add room; search again and retain choices; duplicate add is harmless; remove one item explicitly; save/read back all retained IDs. Verify invalid selection and failed save preserve the draft. Do not claim nonexistent optimistic-concurrency protection; inspect stale assignment behavior before promising it.

### B02b: workload-to-slot placement

Read `planner_drag_controller.js`, Schedule view/presenter, `Operations::Admin::PlaceRecommendation`, `CreateScheduleEvent`, their contracts and gateways. Read API create/from_recommendation actions and contracts.

Keep the current drag-to-open idea, replace generic text acceptance with a typed source, add clear source/destination states and cancellation. Click/tap selection reaches the same staged editor. Display required choices visibly; limited enum selects remain allowed by the design matrix. Validity comes from the server, not the drag CSS state.

Acceptance: select/drag the same requirement to the same cell and compare resulting form/operation. Save once, read back the event and remaining hours, reload. Try invalid/occupied destination, Escape, external text drop and context change while selected. No duplicate submission, no disappearance of source load, no stale destination label.

### B02c: existing lesson move/edit

Read `Operations::Admin::UpdateScheduleEvent`, `Contracts::ScheduleEventUpdate`, schedules update/transfers/swaps controllers and `ScheduleWorkspaceGateway`. Read API `schedule_events/update.rb`, `update_contract.rb`, transfers/swap actions, `schedule_event_repo.with_placement_lock` and notification helper.

Verify click edit before adding event drag. Add event-to-slot staging with unchanged event ID, same load requirement, retained teachers/rooms/subgroup/date/week semantics. A recurring move is labelled recurring. One-date transfer and swap are separate explicit commands and retain their dedicated contracts.

Acceptance: move a real disposable event; edit it; reload and compare ID/relationships; reject occupied target and retain original; cancel draft without changes; no-op save remains harmless. Check two-session conflict behavior and distinguish occupancy conflict prevention from stale-draft detection. Existing implementation may not support every optimistic concurrency guarantee: record/add a narrow precondition only if required, never invent a version field in the UI alone.

B02 complete only when the full journey from workload creation to saved/reloaded edit works by click/tap and the supported drag alternatives. Include one keyboard-only path. Do not proceed to broad Resources migration while the changed core journey regresses.

## 5. B03 — Resources without the form wall

### Primary files

`app/views/admin/resources/{posts,buildings,rooms,room_types}/index.html.erb`

`app/views/admin/student_groups/{index,semesters}.html.erb`

Corresponding `app/controllers/admin/resources/`, `student_groups_controller.rb`, `TeachersWorkspaceGateway`, API resource contracts/actions. Shared `posts` template covers posts, academic titles/degrees, subjects, activity types, building types, time slots and week types.

### Steps

1. Add one explicit shared resource navigation partial covering all existing destinations and correct active state. Do not leave each subpage with a different subset of links.
2. Use shared page header/gutter/table toolbar. Existing table is the focal point; Add opens an editor. Do not put a large empty create form permanently above every list.
3. Preserve and document each table's columns. Repair header/row alignment, native semantics and form placement. Time-slot editing has three fields but must retain the existing displayed column meaning; keep its editor outside the row grid.
4. Use shared editor for create/edit; label every field. Keep contracts' short-name/code requirements. Read-only values become default list state.
5. Replace relationship selects with readable search according to the control matrix. If existing API supports only pagination, preserve paging and add search only through a confirmed endpoint or narrow API addition. Do not invent `q` support.
6. Refactor touched redirect-on-failure paths to re-render safe values and translated errors. Show blocked deletion with dependency feedback; never optimistically remove before success.
7. Move resource-only styles into `_admin_resources.css`. Remove superseded definitions after route verification. Retain shared primitives in their shared file.

Acceptance per entity family: list -> filter/page -> create -> failed create -> successful create -> edit -> failed edit -> successful edit -> reload -> delete disposable record or verify expected dependency refusal. Do not add delete/update operations that routes lack. Verify long Cyrillic fields, empty catalogs, forbidden role and narrow table access. Check student group and group-semester identities remain distinct.

## 6. B04 — Calendar as a coherent scheduling tool

### Primary files

`app/views/admin/period_settings/show/show.html.erb`, `semesters/index.html.erb`, `room_reservations/index.html.erb`, `holidays/index.html.erb`, `weekday_transfers/index.html.erb`

Corresponding controllers; `ScheduleWorkspaceGateway`, `RoomScheduleGateway` and whichever existing gateway each controller actually calls. API period, reservation, holiday and weekday-transfer actions/contracts.

### Steps

1. Extract shared calendar navigation with existing five destinations and retained valid context.
2. Use the Schedule-style context primitive with correct group-semester identity for period configuration. Remove raw numeric fallback controls; show catalog failure and Retry.
3. Keep four period kinds as compact, clearly bounded inline edit units. Group delete with its own period, not as a separate grid sibling. Keep invalid submitted dates and visible field error after rejection.
4. Make semester and dated-rule list/forms consume shared primitives without changing table schemas.
5. Reservations: searchable room/person selection in both create and edit, labelled date/time. Preserve teacher person ID; do not send membership ID from the workload picker.
6. Weekday transfers: visible base-group search and explicit global scope; five aligned columns. Retain create/delete-only behavior unless the user separately requests update support. Holiday editing must not be faked by deletion/recreation.
7. Isolate calendar-only rules in `_admin_calendar.css` and verify navigation to Schedule with selected context.

Acceptance: configure each supported period through its existing upsert; invalid start/end retains dates; return to scoped schedule and read back period; create/edit reservation with searched teacher/room; conflict leaves original; create/delete disposable holiday and dated weekday rule; global vs group scope remains correct; all five pages share controls and keep their tables accessible.

Not in this batch: a new month calendar, drag period resizing, batch period updates or mass rescheduling. Those are new product operations, not necessary to fix the described inconsistency.

## 7. B05 — Accounts with deliberate scope changes

### Primary files

`app/views/admin/accounts/{index,credential}.html.erb`, `app/controllers/admin/accounts_controller.rb`

`app/javascript/controllers/account_{login,role,teacher,department}_controller.js`

`app/lib/gateways/admin/accounts_gateway.rb`, relevant search gateways; API `slices/authentication/actions/accounts/` and contracts/authorization reached by those actions.

### Steps

1. Make filters/table the default page; Add opens a common side sheet. Preserve six column meanings and paging/filter context.
2. Use the same sheet anatomy for edit. Replace numeric teacher/scope fields with searched labels and explicit selections. Preserve existing scopes even when they are not in current search results.
3. Role/status remain small enum controls. Role-specific sections explain requirements, disable irrelevant submission fields and do not silently erase existing data.
4. Generated login becomes a suggestion; manual edits are respected. Do not invent a persisted full-name account field if the current name input only generates login.
5. Keep safe submitted values and field/server feedback on failure. Do not echo passwords or leak backend error symbols. Distinguish forbidden/validation/network outcome.
6. Preserve one-time credential no-store response and deliberate generation operation. No screenshot/log/URL/browser draft storage of credentials.
7. Isolate account layout in `_admin_accounts.css`; remove absolute row editor styling after the replacement is verified.

Acceptance: create each role available to the current administrator; select required person/scope without IDs; edit login/status/scope; invalid input keeps safe fields; forbidden roles/scopes remain server-rejected; reset/generate credential deliberately without evidence leakage; table filters/page survive return; no per-row teacher lookup waterfall. Repeat with a scoped administrator when available and report role coverage accurately.

## 8. B06 — adjacent pages, responsive and final acceptance

Harmonize shared controls on Teachers, Notice, Overview and Observability without changing their task structure. Teachers retains availability grid and inheritance; no multi-cell toggle gesture. Notice keeps print schema and page-specific print rules. Observability receives compatibility styling only.

Finish the package verification matrix, rerun the full core journey after all shared CSS changes, and compare public home/teacher/group/room surfaces affected by shared imports. Verify keyboard, tap alternative, long labels, empty/error/loading/forbidden states, 200% zoom and reduced motion. For Notice verify print at narrow and desktop viewport, multipage rows, repeated headers, no clipped columns, no blank edge pages and no sidebar. Do not claim print acceptance from screen CSS.

Exit: every required batch accepted, or a precise partial status listing outstanding route/state/evidence. All changed UI must preserve table inventory. No new runtime dependencies, tests, raw ID controls in migrated normal journeys or unrelated infrastructure work.

## 9. Verification matrix and evidence format

| Dimension | Minimum proof |
| --- | --- |
| Styling | Screenshot before/after + computed font/control values on two shared consumers |
| Assets | Browser loaded current CSS imports/JS; no new 404/CSP/Turbo errors |
| Tables | Ordered headers/grouping compared with B00 at desktop/narrow; all cells reachable |
| Mutation | Actual request succeeds; authoritative read-back; reload retains result |
| Failure | Rejected/failed request retains original and safe draft; accurate Russian feedback |
| Drag | Same operation as click; cancel/invalid target/duplicate drop/context change checked |
| Keyboard/touch | End-to-end task without drag; visible focus; modal focus/return; tap targets |
| Scope | Authorized role succeeds; disallowed scope remains denied by API |
| Draft | Search preserves selections; close/context change handles dirty state deliberately |
| Concurrency | Document actual supported guarantee; demonstrate conflict behavior if affected |
| Regression | Neighbor sharing CSS plus full workload->schedule journey at final batch |

For each completed slice append to HANDOFF.md:

```text
Batch/slice:
Status: source-complete / runtime-verified / blocked
Files changed (attributable):
Source revision + dirty-scope identifier:
Runtime source/asset verification method:
Routes, role, safe selected context, viewport:
Checks and exact results:
Browser/mutation/read-back evidence paths:
Table inventory comparison:
Known remaining issue:
Next concrete action:
```

Use relevant installed commands, not a new test harness. Typical checks are `ruby -c` for changed Ruby, the repository's RuboCop invocation for touched Ruby, `bin/rails zeitwerk:check` after loading/namespace changes, and real route rendering for ERB. Use existing JS syntax tooling if available and inspect browser module errors. On Windows adapt invocation to the established runbook. Production restart does not compile assets; record the actual build/update path.

## 10. Changes explicitly out of scope

React retirement; Excel changes; generalized UI/component frameworks; new ORM/database ownership; Solr/search engine experiments; graph recommendations; infrastructure replatforming; blanket controller refactor; custom calendar engine; new account privilege model; arbitrary drag everywhere; table reorganization; new tests; agent review cycles; committing unrelated dirty changes.

The broader migration backlog remains in older handoffs. This initiative does not mark those tasks complete or remove them.
