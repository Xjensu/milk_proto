# Copy-ready implementation prompt

You are the fullstack engineer implementing the Milk Schedule administrator design system in this workspace. Implement the prescribed design and interaction plan; this is an execution request, not another request to produce a plan. Work directly without sub-agents. Continue through the ordered batches while preserving working behavior and recording honest verification status.

## Product brief

The administrator functionality is reported to work. The problem is inconsistent visual styling and UX across pages. Schedule and Workload are largely acceptable references; Teachers and Notice also contain useful established patterns. Resources (`Справочники`), Calendar (`Календарь`) and Accounts (`Учётные записи`) are anti-references for current design/composition. Bring them into one coherent scheduling workbench without replacing the accepted identity.

The operator spends long sessions allocating workload, assigning teachers/rooms, placing lessons and correcting schedules. Make these actions efficient and understandable. Use frequent drag and drop for meaningful workload/assignment/schedule operations, with equally capable click/tap and keyboard alternatives. Most entity selection must use visible searchable lists, not dropdown menus. Small stable enumerations may use native selects.

All product interface text must be Russian. Chat, documentation, handoffs and implementation explanations must be English.

## Read in this order

1. Workspace `AGENTS.md`.
2. `apps/schedule_web/AGENTS.md`; also `apps/api_geteway/AGENTS.md` before touching API behavior.
3. `docs/design/admin-workbench-2026-09-13/HANDOFF.md` for current batch/evidence state.
   Read its current continuation section and `BROWSER_AUDIT.md` before the historical source-complete entries. The authenticated browser audit supersedes obsolete browser-unavailable and missing-CSS claims.
4. `docs/design/admin-workbench-2026-09-13/DESIGN_SYSTEM.md` for fixed visual/interaction decisions.
5. `docs/design/admin-workbench-2026-09-13/IMPLEMENTATION_PLAN.md` for ordered execution and gates.
6. Relevant sections of `docs/design/admin-workbench-2026-09-13/AUDIT.md` for source evidence, not every historical handoff.

Read the actual touched source and contracts after the package. The package guides implementation; it does not replace current source inspection. Do not assume file line numbers, HEADs, loaded assets or runtime behavior remain unchanged.

This package supersedes older conflicting reviewer-agent ceremonies, instructions to assume schedule editing is broken, and visual recipes for this initiative. Preserve the broader migration backlog without working on it now. Select only useful skills; user/project constraints override new-test/delegation/reconfirmation workflows in skills.

## Hard constraints

- Do not change table structure: columns/order, headers, groupings, merged cells, row meaning, axes, totals or report semantics. Do not hide columns or turn tables into cards on mobile. Repair geometry and semantics while retaining content. Use local scrolling for dense tables.
- The local technical-specification DOCX did not contain explicit table layouts. Preserve the existing schemas; do not claim specification parity or invent missing schemas. If a later external specification conflicts, record the precise mismatch before changing structure.
- Preserve dark admin sidebar, Manrope body/controls, Unbounded headings and ink/velvet/almond identity. Do not introduce a blue dashboard kit, decorative metric grid, rounded-card redesign, new font or arbitrary per-page styling.
- Use Rails ERB/Hotwire/Stimulus/importmap/Propshaft. No React islands, Tailwind/shadcn, component framework, bundler migration or generic CRUD renderer.
- Rails owns UI/contracts/operations/presenters/gateways; Hanami owns authorization, transactions/repositories and database rules. No business database in Rails.
- Preserve Excel and React repositories. Do not reset storage, clean fixtures wholesale, refactor infrastructure/search engines or change deployment architecture.
- No new tests or test files. Use appropriate existing syntax/lint/loading checks and controlled runtime/browser verification. Do not remove pre-existing tests to comply with this instruction.
- Root, Rails and API have independent Git state. Preserve all pre-existing changes. Never `git add .`, reset, stash away unrelated work or commit someone else's dirty files. Do not commit unless the user requests it.
- Keep credentials, cookies, tokens, generated passwords, proxy auth and session state out of output, screenshots and documentation. Do not inspect old browser auth artifacts to gain access.

## Fixed design implementation

Use the exact roles/values in DESIGN_SYSTEM.md. Summary: page header 82px minimum, heading Unbounded 600 24px/32px desktop and 20px/28px narrow; body/form text Manrope 14px/20px; table content 13px/18px; supporting text 12px/18px; no essential text below 11px. Controls 40px, compact table actions 36px, touch primary actions 44px. Main gutter 24px desktop and 16px narrow. Preserve existing sidebar 224px/184px and 980px/680px thresholds. Tables/sheets stay square; controls 4px radius; small panels 6px.

Continue the existing admin-scoped tokens and shared ERB/CSS primitives consumed by Schedule and Workload. `_admin_tokens.css`, `_admin_components.css`, `_admin_resources.css`, `_admin_calendar.css` and `_admin_accounts.css` already exist and load in the audited runtime. Reuse them, inspect their cascade and finish coverage. Import via `@import url(...)`. Do not recreate these files, append competing patch sections to the monolithic stylesheet or globally recolor public pages.

The original undefined-token report predates the current loaded token file. Check remaining references individually and reuse equivalent existing aliases. Scoped control font inheritance exists, but rendered controls still have inconsistent sizes and coverage. Check actual computed values, loaded fingerprints and failed assets rather than treating the file's presence as completion.

## Browser-confirmed starting state and priority

Playwright MCP worked with normal administrator sign-in. Do not execute an account insertion snippet or store supplied credentials. Recheck the current tool/session; only report a browser blocker after a fresh failure. Existing UI/API changes are dirty work to preserve, not changes you authored.

Read V01–V08 in BROWSER_AUDIT.md and use its screenshots as the before-state. The shared styles and fonts load. Workload's five-column narrow table is already preserved in a labelled scroll region. Account teacher labels and searched edit fields render. Preserve those improvements.

Concrete unfinished work: Schedule selection changes only the frame URL and loses the group on reopening the displayed address; accepted-page headings/controls are too small while Teachers has a 64px heading; context delays Schedule/Workload tables to around 695/764px; Posts row fields and five-column weekday transfers misalign; Accounts has an overlapping table-cell editor and oversized permanent create block; Reservations displays unavailable data. Follow the plan's batch order and trace the reservation failure before attributing it to API or inventing fallbacks.

Apply DESIGN_SYSTEM.md's browser-derived decisions: compact committed context, visible unambiguous searched choices, first working table within the proposed 400px desktop budget, narrow monitoring disclosure, explicit shared field coverage, real accessible sheet, preserved table schemas. These are targets to verify, not claims of existing compliance. Existing safe-draft and typed drag code must be inspected and runtime-checked before replacement or acceptance. No full write/failure/concurrency journey was executed during the documentation audit.

Shared primitives: page header, context selection, button variants, labelled field, feedback, entity search/list/selected summary, editor shell, pagination. Keep domain tables and operations explicit. One primary action per local task; destructive actions separate; no hover-only essential actions.

## Entity selection and context

Teachers, subjects, groups, rooms, departments and specialties use visible searchable results with readable secondary context and retained selected values. Do not place them in long selects even if fixture data is small. Faculty can use a short visible list when complete and small. Semester shows current/recent options with reachable history. Period/week/mode switches use visible choices. Account role/status and six weekdays may use native selects. Follow the full control matrix.

Existing shared admin search contract uses q length 3–120; existing Stimulus debounce is 500ms. Preserve ordinary GET submit/Enter, IME handling and disconnect cleanup. Do not invent server search parameters. Do not search only the first loaded page while claiming full-catalog search. Give repeated search forms unique IDs and isolate results so updating them does not replace the draft form.

Keep selection context in the URL where appropriate and preserve Back/reload. Clear incompatible child choices when parent changes. Dirty drafts require deliberate resolution before changing context; search results must preserve draft values. Never expose raw ID entry as a normal fallback for catalog/network failure.

Identity is critical: planner/workload group selection normally uses student_group_semester_id; dated weekday-rule group uses base student_group_id. Account/reservation teachers use teacher person ID; workload/event assignment uses teacher_on_kafedra membership ID. Labels do not identify records. Normalize once and preserve the same identity through authorization and write.

## Drag, save and failure rules

Implement click/tap/keyboard selection and Save first, then connect drag to the same staged operation. Existing planner_drag only selects a load and opens a destination; it is not complete event movement. Use typed payloads, reject arbitrary text drops, keep the original visible and clean all transient state on cancel/drop/disconnect/Turbo navigation.

Workload -> cell opens a placement preview with group, period, weekday/date, pair and requirement visible. Membership/room -> assignment adds to the retained draft selection. Event -> cell stages PATCH of the same event. Never silently persist on drop. Duplicate drops must not duplicate relationships or submissions. Hover indicates a candidate, not guaranteed server availability.

The assignment endpoint is PUT `/admin/study_load_requirements/:id/assignments` with `{assignment: {teacher_on_kafedra_ids: [...], room_ids: [...]}}`: it replaces complete collections. Send retained selections as well as the dropped item. The inspected contract permits up to 20 distinct items per list and requires a teacher when rooms are selected. Do not use several requests per chip.

Existing event editing uses PATCH and cannot change load_requirement_id. Preserve event ID and relationships. Recurring edit, one-date transfer and atomic swap are different operations. Do not use delete/create or two PATCH calls to simulate them. Do not silently apply a single event move to stream siblings.

Keep the original and safe draft values on 422, 409, forbidden or recoverable network failure. Distinguish an unknown write outcome from a definite rejection. Disable duplicate local submission but do not automatically retry mutations after timeout; read back before retrying. Do not claim success from an optimistic DOM move.

Preserve the API outer placement transaction/lock and atomic notification/related writes. Row locks do not prove stale-browser-draft detection. Before promising concurrency protection beyond occupancy checks, inspect the actual precondition behavior and verify with two sessions.

## Editors

Schedule uses a nonmodal inspector so destination selection remains possible. Workload/Resources/Accounts use a consistent multi-field side sheet; Calendar period forms can remain inline. One editor per workspace. Heading names record/action, body has labelled fields and inline errors, footer has Save/Cancel. Search does not nest another modal or destroy draft state.

Modal semantics must be real: labelled dialog, focus containment, inert background, Escape, focus return, one scroll lock. Do not just add aria-modal to a fixed aside. Dirty close/context switch offers Save, Discard and Continue editing; routine unchanged navigation needs no confirmation. Do not store sensitive drafts in browser storage.

Accounts/Resources/Calendar currently include redirect-on-failure paths. Repair touched forms to retain safe submitted input, translate safe errors and keep their editor open. Passwords are never reflected back. Account one-time credentials preserve no-store and are excluded from evidence captures.

## Execution sequence

Start immediately with B00: inspect independent Git status, verify running Rails/API source and asset provenance, connect the supported browser, capture the existing table inventory and visual baseline, and verify a safe workload->teacher assignment->lesson placement->edit->reload journey. `/up` 200 is liveness only. If a regression is reproduced, fix the active journey before broad styling.

Then B01: complete existing shared tokens/primitives on Schedule and Workload, equalize visual roles, preserve the corrected mobile table, and verify both pages plus a neighboring/public page touched by shared CSS.

Then B02 sequentially: shared entity selection and safe assignment editor; teacher/room drag into retained assignment selection; workload-to-slot placement; existing lesson move/edit with distinct dated transfer/swap. Finish a full verified operation before the next.

Then B03 Resources: stable grouped navigation, table-first browsing, single Add/Edit sheet, labels and searchable relationships, valid table geometry, retained failure drafts. Cover all resource families using the existing shared posts template and dedicated building/room/group pages.

Then B04 Calendar: shared five-section navigation/context, four clear period edit units, searchable reservation room/person, aligned holiday/weekday-rule tables, no raw ID fallback, retained invalid dates. Do not build a new calendar engine or unsupported edit endpoints.

Then B05 Accounts: search/filter/table-first page; common create/edit sheet; searched person/scopes; small role/status controls; retained scopes/drafts; respectful login suggestion; deliberate credential generation.

Then B06: shared consistency on Teachers/Notice/Overview/Observability, responsive/accessibility/print verification and final complete core journey. Keep all table columns and existing availability/report semantics. No unrelated platform work.

Use IMPLEMENTATION_PLAN.md for exact files, substeps and acceptance. Do not stop after only tokens or the first restyled page and call the full initiative complete. If a real blocker prevents continuation, preserve partial work and report the exact blocked gate plus next action.

## Verification and evidence

Use relevant installed Ruby syntax/lint and Rails loading checks; render ERB routes in the actual app; inspect browser console/network and current asset fingerprints. Do not create tests. Verify each changed mutation by authoritative read-back and reload. Verify invalid input, conflict, cancel, search with retained selection, long Cyrillic values, empty/error states, forbidden scope, Back/reload and keyboard/tap alternatives.

Check 1280x900, 1440x900, 768x1024, 390x844 and essential 320px behavior, plus 200% zoom. Compare table headers/grouping to B00. Tables scroll locally; page does not overflow. Focus is visible, modal focus returns, no duplicated handlers after Turbo navigation, no failed assets. Notice additionally needs print verification with complete columns and multipage behavior.

The latest browser audit successfully used authenticated Playwright MCP. Try the current tool/session; do not reuse the initial audit's unavailable-browser limitation. If access now fails, record the precise new failure and mark affected visual/runtime items unverified. Continue safe source work without claiming completion. Do not bypass authentication or manufacture screenshot evidence.

For each completed slice update HANDOFF.md with status, attributable files, source/runtime provenance, routes/role/viewport, exact checks/results, safe evidence locations, table comparison, remaining issue and next concrete action. Existing historical handoffs are not runtime proof.

## Communication and finish

Give concise English progress updates about concrete findings and completed slices. Make routine implementation decisions within this prescribed design without asking for permission to proceed. Ask only when necessary missing information blocks a specific operation; explain the actual constraint. Do not delegate or propose another design exploration.

Final delivery should summarize actual changed behavior, list completed batches, link the updated handoff and evidence, name verification performed, and clearly state any remaining runtime/visual/role coverage gaps. Do not claim a complete or production-ready interface from syntax checks, screenshots alone, or incomplete mutation read-back. Begin with B00 now.
