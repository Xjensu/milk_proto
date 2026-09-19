# Administrator workbench audit

**Historical source baseline.** The findings below precede the implementation entries later appended to HANDOFF.md. Read [BROWSER_AUDIT.md](BROWSER_AUDIT.md) for the authenticated September 13 rendered evidence and explicit resolved findings. Browser access now works; new admin CSS exists and loads; the sampled Workload mobile-card conversion and raw account teacher display have been corrected. Do not use the old clean-worktree or browser-unavailable statements below as current status. Architectural/contract observations remain useful but must be checked against touched source.

Date: 2026-09-13. Scope: `apps/schedule_web` and the UI-facing contracts of `apps/api_geteway`. This is a source-grounded design and architecture audit, not a complete security audit or a claim of rendered visual verification.

## 1. Executive decision

Keep the existing scheduling workbench identity and task-specific tables. Consolidate the shared visual language, make entity selection consistent, and replace the form-heavy composition of Resources, Calendar and Accounts. The application does not need a new dashboard aesthetic or frontend framework.

The user reports working functionality. Treat that as the current product baseline, while verifying affected journeys during implementation. Source inspection finds failure-recovery weaknesses; these are scoped implementation concerns, not evidence that the entire application is nonfunctional.

The largest improvement is reducing context reconstruction: an administrator should see the selected faculty, semester, group/department and period while moving between workload, placement and editing. Drag should transfer a meaningful item to a visible destination. It must not make users guess whether they changed a recurring schedule, a single date or an assignment default.

## 2. Evidence and limits

| Evidence | Result | Interpretation |
| --- | --- | --- |
| Rails source HEAD | `cbafbb2a887dd989dbe5b5fc2441cd1efee26c43` | Rails worktree was clean before documentation edits |
| API source HEAD | `0065358a53af0d80917fb9ea400727297b83c181` | API had substantial pre-existing modified/untracked files; HEAD alone does not identify its full working source |
| Workspace root | Many pre-existing changes/untracked artifacts and deletions | Preserve them; do not stage root wholesale |
| `curl.exe --noproxy '*' ... http://localhost:3000/up` | HTTP 200 | A Rails liveness endpoint responds; source provenance and admin behavior are not established by this |
| Browser runtime | Selection returned “No browser is available”; discovery returned `[]` | No current authenticated screenshots, computed-style comparisons or interaction verification |
| Local technical specification | `docs/ТЕХЗАДАНИЕ (2) (1).docx`, text extracted from OOXML | Describes layered architecture, schedule entry/editing, administrator ergonomics and responsiveness; contains zero native OOXML tables and no embedded media |
| Existing handoffs | Read selected working setup, visual contract and README | Historical pointers; not current runtime acceptance |

The local specification does not establish explicit table column layouts. Do not claim the current five-column workload list has been certified against the user's full external specification. Freeze current columns/grouping and record the missing external schema as a limit. This does not block styling the existing structures.

No application code, runtime data, storage, dependencies or deployments were changed by this planning task. No mutations or new tests were executed. Browser evidence must be collected by the implementer before a visual batch can be accepted.

## 3. Architecture map: assets worth preserving and weaknesses

| Element | Strength | Weakness / consequence | Recommendation |
| --- | --- | --- | --- |
| Rails 8 / ERB / Turbo / Stimulus / importmap / Propshaft | Existing server-rendered stack supports small incremental changes and ordinary forms | Page markup and CSS conventions are uneven; shared patterns are copied under page-specific names | Consolidate a small set of ERB/CSS primitives; retain the stack |
| `app/lib/contracts`, `operations`, `gateways`, `presenters`, `policies` | Typed operations and pure formatting separate concerns on newer workspaces | Older resource/account/calendar controllers call gateways and build options directly | Migrate only touched interaction logic into explicit operations/presenters; avoid a wholesale rewrite |
| `Gateways::Admin::ScheduleWorkspaceGateway` | Distinct create/update/transfer/swap/period operations; mutations use one attempt | Several errors collapse to broad symbols; some 404 cases become upstream-unavailable | Preserve endpoint shapes; improve safe error mapping when migrating editors |
| `Gateways::Admin::WorkloadGateway` | Atomic replacement endpoint already exists for teacher/room defaults | Name `teacher_ids` inside replacement can be confused with person IDs; replacement is not append | Explicitly document membership identity; stage complete retained sets |
| Hanami admin slice | Contracts, authorization, placement lock, repositories and Outbox architecture support safe UI operations | UI designers can easily overpromise capabilities from endpoint names; heterogeneous response shapes and older contracts need per-flow reading | Keep business rules here; map every new interaction to real contracts |
| Authentication slice | Accounts have their own `/auth/accounts` API and one-time credential handling | UI uses `/admin/accounts`, so guessed API paths would be wrong | Preserve gateway separation |
| Shared stylesheet entry point | Existing color/font identity; fingerprint-compatible `@import url(...)` convention | `_admin_workspace.css` mixes shell, planner, resources, calendar, accounts and monitoring | Extract page domains in controlled batches, preserving import order |
| Search controller | GET fallback, debounce, IME guard and disconnect cleanup | Shared global focus record and optional form ID need care with multiple searches; not a complete ARIA combobox | Reuse visible search lists; give forms unique IDs and bounded frame ownership |
| Dialog controller | Escape, focus-return and bfcache handling exist | Does not establish a complete modal focus trap/inert-background contract; admin layout does not automatically install it | Reuse suitable behavior only after tracing mounts; complete modal semantics locally |
| Excel processor / React application | Existing independent capabilities and migration references | Easy source of unrelated scope expansion | Leave both intact |

## 4. Confirmed cross-page source findings

All Rails paths below are relative to `apps/schedule_web`.

### A01 — undefined tokens and competing typography

`app/assets/stylesheets/components/_admin_workspace.css` uses `--admin-display` in `.admin-page__header h1`, but the defined font tokens are `--font-display` and `--font-ui`. The same stylesheet references undefined `--muted`, `--paper-muted` and `--accent`. These declarations fall back through CSS inheritance/initial behavior instead of the intended design values.

The generic resource/calendar/account heading uses `clamp(28px, 4vw, 48px)`. Schedule/Workload use `.schedule-topbar h1`, `clamp(18px, 1.7vw, 25px)` and the valid display font. The overview header has another 21–30px scale. This is a concrete source of hierarchy drift; actual computed font loading still needs browser verification.

`application.css` inherits fonts for button/a/input but omits select/textarea. Some page selectors compensate; others do not. Fix inside admin scope to avoid public side effects.

### A02 — partial styling, not literally absent CSS

Resources, Calendar and Accounts have rules around `.resource-*`, `.operations-*` and `.accounts-*` in `_admin_workspace.css`. The user's “unstyled” appearance can reflect incomplete control rules, inconsistent layouts, undefined variables or stale/missing loaded assets. Do not report missing stylesheets as a proven runtime cause.

`.operations-page` has padding; `.resources-page` defines a grid/gap without a common page gutter. `.accounts-page` mainly specializes table columns. The generic form rule controls grid layout but does not provide a complete consistent input/button/label/error system. These are stronger starting diagnoses than adding another catch-all override.

### A03 — generic resource grids do not represent every record shape

`.resource-table__row` defaults to three columns. Weekday transfers render five headings/values without a corresponding five-column variant. Accounts adds a six-column override, illustrating the inconsistency. Resource forms nested as row children contain several fields, while header cells are siblings in a different grid. Matching text labels does not make those tracks line up.

Use native tables with valid cells/forms where practical; otherwise define explicit shared track geometry per existing schema. Preserve logical columns. Do not solve misalignment by removing columns or hiding action data.

### A04 — mobile changes violate the new preservation constraint

`_admin_workload.css` at 680px makes table/tbody block elements, hides the header and changes rows/cells into labelled cards. `_admin_workspace.css` at 760px collapses resource/account rows into one column; its overview rules hide the second audit column at 680px. Review Notice's responsive rules too when harmonizing tables.

Under this initiative, preserve all columns and access through local scrolling. Required tables must not become a different data structure just because the viewport narrows.

### A05 — failure recovery is not consistent

`app/controllers/admin/accounts_controller.rb#create/update` redirects to a fresh index on failure and interpolates `result.failure` into Russian copy. `period_settings/periods_controller.rb#create` redirects on invalid dates/API failure, preserving context but not submitted dates. Resource CRUD also uses redirects. These source paths can discard safe input and close the user's task.

During each affected page migration, render the same editor with safe draft values, distinguish validation/conflict/network errors, and keep selection context. Never redisplay a submitted password.

### A06 — ID entry leaks the data model into common tasks

Account editing requests teacher and scope IDs; weekday transfers accept a base group ID; reservation editing accepts a teacher ID; calendar context falls back to faculty/semester ID fields when options are unavailable. Replace with readable selection and explicit unavailable states. A failed catalog is not a reason to make an administrator discover database keys.

### A07 — entity search is inconsistent and incomplete as a selection experience

Workload has subject/room search and visible teacher/room choices; Teachers and Notice use readable search results. Account creation has teacher/department searches, but editing falls back to numeric IDs. Room reservation creation searches teachers and then places results in a select; editing differs again. Faculty/semester lists commonly fetch only the first 100 items. Resource catalog browsing does not automatically mean full-catalog search exists.

Do not filter only the loaded first page and label the result global search. Reuse real endpoints; mark genuinely missing search capabilities as a small API follow-up.

### A08 — drag is a good starting pattern, with limited current scope

`planner_drag_controller.js` supports load requirement drag, writes an ID to DataTransfer, and on drop changes a `.planner-add` URL with `selected_load` before clicking it. It opens the placement path; it does not persist directly. Existing event links are not draggable moves. The controller also accepts generic `text/plain`, so numeric external text can enter the path.

Build on its operation staging, but use typed payloads and complete cancellation/cleanup. Do not claim the existing highlight guarantees a valid slot. Validity remains a server decision.

### A09 — landmarks, labels and overlay behavior need repair alongside layout

The admin layout owns `<main class="admin-main">`; several resource/account/calendar templates put another `<main>` inside it. Replace inner main with an appropriately labelled section/div. Resource div tables have row roles with ordinary spans/forms and incomplete cell semantics. Several create fields rely on placeholders. Account editor popovers are absolutely positioned within rows, which creates clipping/collision risks. Workload uses a fixed side sheet whose modality must be verified.

These are source findings and rendering risks, not claims that every keyboard interaction was tested and failed.

## 5. Page-by-page direction

| Surface / route family | Keep | Improve | Drag policy |
| --- | --- | --- | --- |
| Schedule `/admin/schedule`, `/admin` | Source load rail, remaining hours, group/period context, timetable, inspector, server recommendations | Compact shared context; stronger selected/destination states; readable text; stable editor draft; explicit recurring/date distinction | Workload to slot, then existing event to slot after PATCH flow is verified; stage/save |
| Workload `/admin/workload` | Faculty/department modes; existing five-column table; subject/group/hour meaning; separate record and assignment editing | Shared header/context; less oversized mode decoration; search near selections; retained selected chips; common side sheet | Teachers and rooms to existing assignment cells/editor; do not reorder rows |
| Teachers `/admin/teachers` | Search, profile/membership separation, week/semester choice, availability matrix, schedule preview | Shared controls/type scale; reduce numbered-section clutter where it adds no task meaning; label inheritance/read-only states consistently | Keep click availability toggles. Multi-cell painting is deferred; current endpoint toggles cells individually |
| Notice `/admin/notice` | Teacher/semester/period sequence; five-column report; print separation | Shared selection/control styling and empty/error language; retain print-specific typography | None; this is a report |
| Resources `/admin/posts`, titles/degrees/subjects/activity/week/time/building types, buildings/rooms/room types/student groups | Existing entity fields, server validation, pagination and dependencies | Stable category navigation, current table, one explicit create/edit task; labelled fields; searchable relationships; no inline form wall | No decorative reorder. Entity selections may be reused in workbench assignment flows |
| Group semesters `/admin/student_groups/:id/semesters` | Group identity and existing semester fields | Readable breadcrumb, coherent table/editor, link back to scoped schedule/calendar | None until a real server operation requires it |
| Calendar `/admin/operations` | Four period kinds, dates, group context, schedule return link | Same context picker as Schedule; compact period rows with dates and status; deletion belongs with its period; preserve rejected dates | Period timeline/resizing deferred; do not replace date fields with a speculative calendar engine |
| Semesters `/admin/operations/semesters` | Existing fields and create operation | Shared form and table styling; readable academic-year choices | None |
| Reservations `/admin/room_reservations` | Existing reservation list/columns and CRUD | Search rooms/teachers consistently in create and edit; separate browse/filter from record editor | Optional future room/time placement only after a real supported grid exists; not required now |
| Holidays `/admin/holidays` | Date/name/action columns, create/delete contract | Table-first page, labelled compact editor, clear scope | None; no update endpoint is exposed by Rails |
| Weekday transfers `/admin/weekday_transfers` | Date/follows weekday/group/reason/actions semantics | Five aligned columns; base-group search; explicit “Все группы”; clear dated-rule copy | None; do not fake edit with delete/create |
| Accounts `/admin/accounts` | Role/status filtering, six logical columns, scoped management, one-time credentials | Table-first browse; one create/edit sheet; role-specific sections; searched person/scope selection; translated errors | Do not drag roles/scopes or accounts; permission changes need deliberate explicit forms |
| Overview `/admin/overview` | Journal and detail inspector | Shared typography/pagination; keep all columns at narrow widths | None |
| Observability `/admin/observability` | Operational visibility | Only shared style compatibility; no new monitoring project | None |
| Sidebar | Dark identity, role-aware availability, current navigation structure | Consistent selected/focus states; accessible narrow labels; resource/calendar active matching across subroutes | No navigation drag/reorder |

## 6. API constraints that determine the UX

| Interaction | Rails / API contract | Consequence |
| --- | --- | --- |
| Create workload | `WorkloadGateway#create_group_load`; POST `/admin/student_group_semesters/:id/load`, `{streams: [stream]}` | Read load contract for the stream/group mapping; do not substitute base group IDs |
| Assign teachers/rooms | Rails POST `/admin/workload/assignments`; API PUT `/admin/study_load_requirements/:id/assignments`, `{assignment: {teacher_on_kafedra_ids: [...], room_ids: [...]}}` | Full replacement, not append. Keep retained selections. Membership IDs, not person IDs. Inspected contract caps each distinct list at 20; rooms need a teacher |
| Place recommendation | Rails POST `/admin/schedule/placements`; API POST `/admin/schedule_events/from_recommendation`, `{recommendation: ...}` | Recommendation staging remains server-checked |
| Manual placement | Rails POST `/admin/schedule/events`; API POST `/admin/schedule_events`, `{schedule_event: ...}` | Preview must supply required existing form contract, not a guessed subset |
| Edit recurring/existing event | Rails PATCH `/admin/schedule/events/:event_id`; API PATCH `/admin/schedule_events/:id` | Existing action rejects changing load_requirement_id; keep event ID |
| Dated transfer | API POST `/admin/schedule_events/:id/transfers`, `{event_transfer: ...}` | Do not conflate with recurring move; read transfer action for exact date fields |
| Swap | API POST `/admin/schedule_events/:id/swap`, `{swap: ...}` | Use atomic endpoint, not two sequential PATCH calls |
| Group period | API POST `/admin/student_group_semesters/:id/periods`, `{period: {period_kind, start_date, end_date}}` | Existing upsert can power explicit date edit; no need for an invented PATCH route |
| Accounts | API `/auth/accounts` | Rails route names are not API route names; retain no-store credential rendering |
| Search | `/admin/search/disciplines`, `/teachers`, `/rooms`, `/kafedras` as exposed by actual routes/gateways | Shared QueryContract is trimmed q 3–120; inspect each action's shape and scope |

`schedule_event_repo.with_placement_lock` opens an outer transaction and acquires a PostgreSQL advisory transaction lock. The event update reads the row for update, checks conflicts, changes relations and enqueues notification within that outer block. The inner `repo.transaction` in the action does not mean notification is outside all transactions. Preserve this structure.

`teaching_assignment_repo.replace_for_requirement` locks the requirement and retains existing assignment IDs where possible. This is valuable for adding drag assignment without destructive identity churn. Neither a row lock nor an advisory lock automatically provides stale-browser-draft detection; do not promise optimistic concurrency without a precondition/read-back design and runtime evidence.

## 7. Priority and success measures

First unify primitives on Schedule/Workload and verify the complete workload-to-lesson journey. Then migrate Resources, Calendar and Accounts one at a time. This extracts the system from accepted pages before applying it to anti-references.

Acceptance measures are practical: zero raw ID inputs in migrated normal flows; zero hidden/removed table columns; same computed font/control styles for shared roles; all drag actions achievable by click/tap and keyboard; safe drafts preserved on validation/recoverable failure; persisted changes survive reload; no failed assets or new console errors. Count operation submissions to ensure one Save does not produce duplicate mutations.

Do not use arbitrary visual scores, animation count or component count as success metrics. The operator's task and preserved institutional table semantics are the measure.

## 8. Related documents

- [Design system and interaction contracts](DESIGN_SYSTEM.md)
- [Implementation batches and acceptance](IMPLEMENTATION_PLAN.md)
- [Continuation state](HANDOFF.md)
- [Copy-ready implementation prompt](IMPLEMENTATION_PROMPT.md)
