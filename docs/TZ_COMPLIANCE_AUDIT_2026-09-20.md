# Technical-specification compliance audit

**Date:** 2026-09-20

**Scope:** requirements in `docs/ТЕХЗАДАНИЕ (2) (1).docx`; source and current implementation evidence only.
**Source revisions inspected:** root `d299adc`, Rails `db3d316`, API `212c763`.

## Method and evidence boundary

The original specification is short and sets product outcomes rather than detailed
screen/API contracts. This audit maps every mandatory outcome to the delivered Rails
and Hanami paths, then separates three states:

- **Delivered:** an end-to-end UI/API path exists in the inspected source and is also
  reported as loaded by `design/admin-workbench-2026-09-13/IMPLEMENTATION_STATUS_2026-09-19.md`.
- **Missing:** the required user journey has no complete UI/API path.
- **Unverified:** source may exist, but no current evidence proves the requirement's
  measurable result in a representative environment.

Existing untracked root files (`.agents/`, `.playwright-mcp/`, `package-lock.json`,
`storage/redis_data/`) were not changed or attributed to this audit.

## Requirements traceability

| Technical-specification requirement | Evidence | Assessment |
| --- | --- | --- |
| Schedule authoring, correction, temporary lesson moves/replacements, and visually distinct changes | Rails Schedule workspace; `POST /admin/schedule/events/:id/transfers`; transfer/swap endpoints; `admin/lesson_transfers`; date-resolved occurrence serializers expose transfer state. | **Delivered in source.** Do not reopen this as a missing feature. Its acceptance still needs disposable-data browser read-back. |
| Teaching-load/schedule work by faculty, department, student group, and individual teacher | Group view: `/schedule?group=…`; teacher view: `/teacher/:id`; API-only aggregate reads: `GET /admin/faculties/:faculty_id/schedule?semester_id=` and `GET /admin/kafedras/:kafedra_id/schedule?semester_id=`. | **Missing end-to-end for faculty and department.** Rails has no route, gateway, presenter, or view consuming either aggregate API endpoint. The workload page is not a timetable and does not substitute for it. |
| Date-specific schedule view and printing | Group and teacher pages accept `date`, render a day mode, and have print controls. | **Delivered for group and teacher. Missing for the required faculty/department views** because those views do not exist. |
| Dated changes included in a date-specific view | Portal group/teacher actions call the occurrence-window expansion and expose transfers; the current implementation status records the selected-date view and dated transfer editor. | **Delivered in source.** Verify with a real moved and cancelled occurrence before release. |
| Find free rooms and temporarily reserve one for a teacher | Room occupancy resolves events, transfers and reservations; teacher workspace has create/cancel reservation; admin reservation management uses the same placement lock. | **Delivered in source.** Conflict, edit and reload journeys remain evidence work with disposable records. |
| Individual notices for lecture week/exam session, including department batch print | Notice workspace, single PDF job, and department batch job restrict export to `lecture_week` and `exam_session`. | **Delivered in source.** The worker/output journey still needs an end-to-end run with disposable data. |
| Separate administrator and ordinary-user interfaces with access protection | Rails admin base controller requires a staff account; teacher workspace is separate; Hanami account roles and scoped authorization exist. | **Delivered structurally; authorization coverage is unverified.** The specification does not define a role matrix, so a role-by-role acceptance matrix must be agreed before calling it complete. |
| Layered microservice structure: database, API gateway, frontend | Independent Rails and Hanami repositories, typed gateways/operations/presenters, and API repositories/actions are present. | **Delivered structurally.** |
| Response within two seconds on a stable connection | Rails exposes request-duration metrics and Prometheus configuration exists. No target percentile, production-like dataset, or measured journey report was found. | **Unverified.** Metrics are observability, not proof of the two-second target. |
| 99% availability and reliability | Health/metrics and one availability alert exist. No availability SLO, alert policy tied to 99%, redundancy/restore evidence, or reporting window was found. | **Unverified.** |
| Growth without material performance loss | No capacity model or realistic load-test evidence was found. | **Unverified.** |
| Usable, ergonomic UI/UX and operation across devices/browsers | The administrator workbench has a current design system, browser audit, implementation plan, shared sheets/controls, local table scrolling and responsive rules. | **Partially delivered; acceptance remains open.** The running evidence has no complete current matrix across 320/390/768/1280/1440, 200% zoom, keyboard paths, long Cyrillic data and browser families for the core business journey. |

## Confirmed functional gap

### F01 — Faculty and department schedules are not a user-facing feature

This is the only clear mandatory functional omission discovered from the original
technical specification.

The API contains two existing aggregate reads:

- `Admin::Actions::Faculties::Schedule` calls
  `ScheduleEventRepo#for_faculty_in_semester`.
- `Admin::Actions::Kafedras::Schedule` calls
  `ScheduleEventRepo#for_kafedra_in_semester`.

Both accept only `semester_id` and serialize a regular schedule grid. They are not
date-resolved occurrence views, so they do not automatically apply dated transfers,
cancellations, holidays, or the exact-day printing behavior that the group and teacher
surfaces have. More importantly, no Rails code calls either route. A repository search
found no Rails gateway, presenter, controller route, or ERB surface for an aggregate
faculty/department timetable.

The correct completion criterion is therefore not merely wiring the two existing
endpoints into a page. Faculty and department schedules must use the same date-resolved
expansion semantics as group and teacher schedules, clearly distinguish moved/cancelled
occurrences, and print the selected date.

## Implementation plan for F01

The product decisions below are resolved. The sequence preserves the existing Rails
ERB/Hotwire/importmap and Hanami boundaries. Performance and availability work is deferred
and does not block this functional plan.

1. **Capture a baseline.** Record scoped Git status and serving revisions. With disposable
   data, create one ordinary occurrence, one moved occurrence, and one cancelled occurrence
   under a faculty and department. Record their authoritative API read-back and the current
   group/teacher date views. No database reset.
2. **Implement staff-only access.** Faculty and department aggregate schedules and their
   print views require authenticated staff access. Enforce access in Hanami as well as
   Rails; hiding navigation alone is insufficient. Trace the exact route through
   Rails controller/operation/gateway to the Hanami action, contract, scope resolver and
   repository before changing a request. Preserve faculty and department scope checks.
3. **Add one date-resolved aggregate read model in Hanami.** Extend the existing occurrence
   query/repository for faculty and department unions, filtering each occurrence by its
   actual faculty/department ownership. Apply the same semester-window clipping, transfers,
   cancellations, holidays, room context and `Europe/Moscow` date handling used by the
   portal group and teacher windows. Do not reuse the old `for_*_in_semester` grid as if it
   represented a dated result.
4. **Expose small explicit API contracts.** Each read accepts a validated aggregate ID and
   an optional ISO date; it returns the existing occurrence-window shape. Keep IDs internal
   to the selected scope and return safe 403/404/422 distinctions. Do not add a generic
   schedule endpoint.
5. **Build Rails adapters and presentation.** Add dedicated gateways, presenters and routes.
   Reuse the existing schedule table/day-mode/print partials only where its group/teacher
   assumptions are not leaked. The context selector must show the chosen faculty or
   department, selected date, and a bounded searchable catalogue where necessary.
6. **Make print match the visible date.** The print action must retain selected scope, date,
   transfer highlighting and cancellation treatment. It must not print a different weekly
   default than the visible day.
7. **Verify the complete journey.** Test group, teacher, faculty and department separately:
   ordinary occurrence, moved occurrence, cancelled occurrence, no-period state, forbidden
   scope, invalid date, print, reload/back navigation and a long Cyrillic label. Confirm the
   selected dated view matches the API response after each mutation.

## Administrator UX continuation

The September workbench package is the authoritative UX plan:
`design/admin-workbench-2026-09-13/IMPLEMENTATION_PLAN.md`,
`DESIGN_SYSTEM.md`, and the later `IMPLEMENTATION_STATUS_2026-09-19.md`.
It already addresses the reported floating controls, oversized schedule editor,
inconsistent spacing, searchable high-cardinality choices, hover/focus states, and
table responsiveness. It must be continued rather than replaced.

The UI review used the `ui-ux-pro-max` guidance for labelled forms, explicit submit
feedback, visible keyboard focus, a click/tap/keyboard alternative to drag operations,
and a bounded z-index hierarchy. The existing plan already adopts those decisions.
For the aggregate schedule feature, apply the same rules:

- Keep one compact context strip above the timetable; put the complete faculty/department
  search in a sheet or dedicated region rather than leaving a tall selection wall open.
- Keep only one primary action on the screen. Use hover/focus for supplemental metadata,
  never for information required to operate the schedule.
- Reuse the dark sidebar, Manrope/Unbounded, ink/velvet/almond tokens, 40px controls,
  36px table actions and 44px primary touch actions. Do not create a second visual system.
- Preserve timetable columns and axes at narrow widths in a labelled, keyboard-reachable
  scrolling region; do not convert timetable cells into cards or hide columns.
- Ensure every drag operation has the same explicit click/tap and keyboard staging path.

## Release evidence plan for the remaining nonfunctional requirements

The user deferred performance, load-profile and availability work on 2026-09-20.
Items 1–3 below are retained as a future backlog, not acceptance gates for the current
functional and UX work. Browser usability and authorization verification remain in scope.

1. Agree the load profile: concurrent public readers, concurrent administrators, dataset
   size, and the percentile interpretation of “under two seconds.”
2. Measure the representative group, teacher, room-availability, reservation, workload,
   aggregate-schedule and notice journeys at that profile. Record p50/p95/p99, error rate,
   database/Redis pool behavior and browser timings.
3. Define the 99% reporting period, monitored endpoints, alert owner, backup/restore
   objective and recovery evidence. Connect the existing metrics/alerts to that decision.
4. Run the agreed browser/device matrix on the real core journey, including keyboard,
   touch alternative, 200% zoom, slow/failure responses and long Russian labels.
5. Run an authorization matrix for super administrator, faculty administrator, department
   administrator, scheduler and teacher, including denied cross-scope reads/writes. Record
   the intended rights for the educational-methodological office explicitly.

## Confirmed product decisions — 2026-09-20

1. Faculty and department aggregate schedules are for authenticated staff only.
2. Both aggregate views must support viewing and printing the selected date, including
   dated transfers and cancellations.
3. Traffic profiles, optimization and the two-second/99% measurement work are deferred
   to a later phase. These specification requirements remain unverified, not waived.
