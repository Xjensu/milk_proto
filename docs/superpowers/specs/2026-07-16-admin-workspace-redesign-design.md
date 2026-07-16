# Administrator Workspace Redesign

## Purpose

Redesign the university scheduling administration experience around human-readable workflows instead of raw database records. The frontend must guide administrators through faculty, semester, department, group, teacher, room, and date context while the API gateway exposes task-oriented projections over the existing normalized schema.

The implementation must preserve the current relational model wherever possible. IDs remain stable transport identifiers but never appear as primary interface copy.

## Scope and delivery sequence

The complete program is split into independently testable releases:

1. Shared workspace context, navigation, human-readable formatting, Overview, and schedule browsing.
2. Faculty and department workload views, assignment inspectors, and aggregate workload mutations.
3. Group calendar periods, occurrence transfers, reservations, specialities, and student-group provisioning.
4. Configurable schedule views, drag-and-drop enhancements, performance hardening, and removal of remaining raw CRUD exposure.

The first release establishes the context and UI primitives used by every later release. No page should build its own semester, faculty, or group selection logic.

## Architectural direction

Use a context-driven administration workspace with thin, domain-oriented gateway entrypoints.

- PostgreSQL remains the authoritative normalized write model.
- Existing CRUD routes remain available for exceptional maintenance and compatibility.
- Read entrypoints return complete, readable projections for one administrator task.
- Aggregate mutation entrypoints validate and commit one user intention transactionally.
- Redis holds short-lived presence heartbeats and safe, scoped lookup caches.
- The existing outbox remains the schedule-change notification mechanism.
- Materialized views are introduced or extended only for measured high-cost reads.

This avoids frontend joins across raw tables, reduces request waterfalls, and prevents presentation code from depending on database column layout.

## Shared workspace context

Create a typed `WorkspaceContext` containing:

- selected semester;
- selected faculty;
- selected department;
- selected student-group semester;
- selected teacher;
- selected room or room set;
- selected working date;
- active schedule mode and period view.

Selections are dependent. Changing a faculty clears incompatible departments and groups; changing a semester clears an incompatible group-semester selection. Valid context is reflected in URL search parameters for refresh, sharing, and browser navigation. A versioned session-storage record restores convenience context only when it is still valid.

Semester labels always use `<number> semester <start year>/<end year>`.

Student groups are always selected after faculty and semester. Labels use `<group code>-<course>`. Course is derived from the selected semester and the group's admission academic year. The API should return the derived course in projections; the frontend formatter remains a defensive fallback and is covered by unit tests.

## Interaction and visual system

All visible product text and implementation documentation are written in English.

The workspace uses a restrained desktop-tool layout:

- persistent primary navigation;
- a compact context rail for active semester and organizational scope;
- one dominant working surface;
- a right-side inspector for details, editing, creation, conflicts, and drag sources;
- a full-height sheet replacing the inspector on narrow screens.

Use quiet neutral surfaces, subtle translucent layering, one semantic accent, compact controls, and strong typographic hierarchy. Apply material principles without imitating macOS-specific APIs or adding decorative blur.

Large collections use accessible searchable comboboxes with bounded results and keyboard navigation. Semester selection uses a horizontal, scrollable card rail rather than a long select. Tables are retained only when row-and-column comparison is the fastest comprehension model.

GSAP is limited to purposeful inspector transitions, selection continuity, rejected-drop feedback, and drag previews. UI motion stays below 300 ms, animates transforms and opacity, cleans up on unmount, and respects `prefers-reduced-motion`. High-frequency administrative actions receive little or no motion.

Drag-and-drop is progressive enhancement. Every assignment or transfer also has a complete click and keyboard workflow.

## Overview

Remove faculty counts, workload counts, file counts, and other generic metrics. Keep only:

- a status bar for API, database, Redis/streaming, and background processing;
- an active-user list with role, current area, and last activity;
- a readable event journal.

Audit entries use resolved descriptions such as “Anna Petrova moved Mathematics for CS-21 to 14:30.” Selecting a user or journal entry opens an inspector containing the full context and technical identifiers only in a secondary diagnostic section.

Dashboard failures are isolated. One failed health source must not blank the other service states, active users, or journal.

## Schedule workspace

The page has Group, Teacher, Room, and Occupancy modes.

### Group schedule

Selection order is Faculty, Semester, Student group. The group result is a `student_group_semester`, not a raw `student_group`.

### Teacher schedule

The administrator chooses either:

- a semester and a teacher, producing the schedule intersecting the current date range; or
- an exact date and a teacher, producing that day's schedule.

Teachers are found through debounced full-text search or a bounded list. Search results show full name, department affiliations, and post when available.

### Room schedule

Room selection follows the same semester-or-date model. Every event displays its student group. Missing teacher and room assignments are valid and display `Not assigned`.

### Occupancy

The administrator selects today or another date and a list of rooms. The result is a time-based occupancy grid with sticky room labels, reservation and teaching-event distinction, and readable group information.

Selecting any schedule event opens an inspector for editing, occurrence transfer, cancellation, permissions, and conflicts.

## Workload workspace

The page begins with two modes: Faculty workload and Department workload.

### Faculty workload

Selection order is Faculty, Semester, then one, several, or all groups in that semester.

Rows contain:

- department code;
- subject name;
- group label;
- one dynamic hours column for every included activity type.

The matrix groups related rows by subject while preserving accessible table semantics. Sticky subject and group columns remain visible during horizontal scrolling.

### Department workload

Selection order is Department, Semester, and optional group scope. Subjects are expandable sections. Each group row contains the group label and student count, followed by configured activity columns.

The initial activity set includes lectures, practical classes, course-project practice, laboratory classes, examinations, and credits. Each activity cell can contain hours, room, and teacher. Missing values remain actionable rather than appearing as errors.

Selecting a cell opens an inspector for assigning or changing hours, teacher, and preferred room, subject to permissions. Creating a record uses a staged inspector with subject search, group selection, activity requirements, teacher assignments, and rooms. Teachers and rooms may be assigned by search, click, keyboard, or drag-and-drop.

## Calendar and schedule periods

Each student-group semester independently defines:

- lecture period;
- credit period;
- examination session.

These names do not imply seven-day durations. Each period has arbitrary inclusive start and end dates.

The administrator selects Faculty, Semester, and Group before editing periods. A day view supports moving or cancelling an individual occurrence without modifying its recurring template. Existing `event_transfers` represent occurrence-level changes.

Activity types remain ordinary scheduling data. Configurable schedule views determine which activity types appear in lecture, credit, examination, or future administrator-defined views.

## Reservations and catalog pages

Reservations provide full-text room search, date range, availability timeline, and a creation inspector.

Specialities require faculty selection before displaying or creating speciality records.

Student Groups require Faculty and Semester selection before listing group-semester records. Creation requires:

- group code;
- speciality belonging to the selected faculty;
- admission year;
- current student count;
- study duration derived from the speciality unless explicitly permitted otherwise.

Before submission, the inspector previews all semester memberships that will be generated from the admission semester through the configured duration. Provisioning is one transactional command and returns the created group plus all generated group-semester projections.

Raw CRUD resources move to a secondary `System data` area. Relationship fields use readable pickers, and raw IDs are confined to optional diagnostics.

## Minimal schema changes

Preserve existing tables and relationships with these corrections:

1. Add `student_groups.code`, with a uniqueness constraint scoped to prevent ambiguous group labels.
2. Make `schedule_events.room_id` nullable.
3. Add timestamps or optimistic-lock versions to concurrently editable schedule and workload records.
4. Add `schedule_view_definitions` with stable key, English name, optional period kind, and ordering.
5. Add `schedule_view_activity_types` to map activity types into one or more schedule views.

Teachers remain optional through the existing `event_teachers` relationship. Group period dates remain in `student_group_semester_periods`.

Database migrations must be backward compatible, provide safe defaults where required, and add supporting indexes for new uniqueness and lookup paths.

## Gateway interfaces

Existing relevant routes are retained, including faculty group lookup, enriched workload reads, schedule grids, occurrence transfers, group periods, teacher search, reservations, and group provisioning.

Add or refine task-oriented interfaces:

- `GET /admin/workspace/options`
- `GET /admin/dashboard/health`
- `GET /admin/dashboard/active_users`
- `GET /admin/schedules/groups/:group_semester_id`
- `GET /admin/schedules/teachers/:teacher_id?semester_id=…|date=…`
- `GET /admin/schedules/rooms/:room_id?semester_id=…|date=…`
- `POST /admin/schedules/rooms/occupancy-query`
- `GET /admin/workloads/faculty/:faculty_id?semester_id=…&group_semester_ids[]=…`
- `GET /admin/workloads/kafedra/:kafedra_id?semester_id=…&group_semester_ids[]=…`
- `POST /admin/workloads/group-semesters/:id/entries`
- `PATCH /admin/workloads/entries/:id`
- `POST /admin/student-groups/provision`
- `GET /admin/schedule-view-definitions`
- `PATCH /admin/schedule-view-definitions/:id`

Large filter sets use JSON request bodies. Search endpoints return bounded results. Collections use cursor pagination when stable ordering is available.

Every projection includes both stable identifiers for commands and readable nested data: semester label, group code and course, subject, faculty, department, teacher name, room/building, activity type, hours, dates, permissions, and conflict state.

Aggregate mutations validate scope and conflicts inside one transaction, return the updated projection, and enqueue an outbox event for cache invalidation and live updates.

## Presence and service health

Active-user presence is ephemeral and does not require a permanent relational table. Authenticated frontend activity refreshes a Redis heartbeat with account ID, role, current area, and expiry. The dashboard projection resolves account details and returns active sessions ordered by recent activity.

Health checks expose only safe operational state and latency. They must not reveal credentials, hostnames, internal exceptions, or unrestricted infrastructure details.

## Frontend module boundaries

Use focused modules rather than a monolithic admin page:

- workspace context state, URL synchronization, and option loaders;
- domain formatters for semester, group, teacher, room, and audit descriptions;
- accessible semester rail and searchable entity picker;
- inspector shell and staged form primitives;
- Overview feature;
- Schedule feature split by query mode and common schedule projection;
- Workload feature split by faculty matrix, department matrix, and assignment inspector;
- Calendar periods and transfer inspector;
- Reservations, Specialities, and Student Groups features;
- typed API clients and runtime normalization at the network boundary.

Avoid shared barrel files for large feature modules. Fetch independent resources in parallel and avoid effects that derive state that can be computed during render.

## Permissions and errors

The gateway returns explicit per-record or per-action capabilities where scope is not obvious from the account role.

- `403`: keep the control visible but disabled and explain the required permission.
- `409`: preserve local edits and compare the current server version with the proposed change.
- `422`: attach validation messages to readable fields.
- network failure: keep existing data visible when safe and offer a scoped retry.
- rejected drag: restore the source position and explain the scheduling conflict.

Null teacher or room values display `Not assigned` and remain editable when permissions allow.

## Performance

- Parallelize independent initial requests.
- Load dependent collections only after parent context is selected.
- Debounce remote search and cancel superseded requests.
- Build maps for repeated relationship resolution.
- Apply rendering containment to self-contained matrix regions with graceful fallback.
- Add windowing only after measured row-count thresholds justify its complexity.
- Keep GSAP transforms compositor-friendly and clean up all animations.
- Version and minimize session-storage context.

## Accessibility and responsive behavior

All controls use native semantics or established accessible primitives. Inspectors manage focus, support Escape, return focus to their trigger, and prevent interaction with obscured content on narrow screens.

Desktop displays navigation, workspace, and inspector. Tablet collapses primary navigation while preserving the inspector. Mobile uses sequential context screens and a full-height inspector sheet. Workload matrices retain horizontal scrolling and sticky headers without converting comparative data into unreadable cards.

Reduced-motion users keep state changes without spatial animation. Drag-and-drop has keyboard and button alternatives.

## Verification

Unit tests cover formatting, course derivation, relationship normalization, context invalidation, API request construction, and projection mapping.

Component tests cover dependent selection, semester rails, searchable pickers, inspector focus, permissions, empty states, validation, and keyboard assignment.

Gateway request and contract tests cover every added query mode and mutation payload, including nullable teacher/room behavior and authorization scope.

Browser tests cover:

- Overview degradation and active users;
- group, teacher, room, and occupancy schedule workflows;
- faculty and department workload workflows;
- workload creation and assignment;
- group period editing and occurrence transfer;
- reservation search and creation;
- speciality filtering;
- student-group provisioning.

Visual verification runs at desktop, tablet, and mobile widths. Performance checks use large workload and occupancy fixtures. Accessibility checks cover keyboard-only operation, focus restoration, reduced motion, and readable errors.

## Documentation and commits

Each changed page or API method receives concise English documentation near its feature or in the API reference. Implementation commits remain small, coherent, and frontend-focused whenever possible. AI-related files and unrelated user changes are never included in commits.
