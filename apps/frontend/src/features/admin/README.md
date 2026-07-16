# Administration workspace

The administration UI is task-oriented. It must never ask an operator to type or interpret database identifiers.

## Shared context

`workspace/WorkspaceContext.tsx` stores the current faculty, semester, group, teacher, room, and date. Context is mirrored to the URL and session storage. Changing a parent selection clears dependent values: faculty clears the group, while semester clears the group and schedule-specific selections.

Semesters use the format `3 semester 2026/2027`. Student groups use `<name>-<course>`, where the course is derived from the selected semester and admission year. Nullable teacher and room assignments are displayed as `Not assigned`.

## Overview

Overview intentionally contains only service status, active users, and the event journal. Each projection is requested independently. If the gateway does not expose a projection, the page shows a capability-unavailable state and never substitutes fabricated metrics.

## Schedule

Group schedules follow faculty → semester → student group. Teacher and room schedules support either semester context or an exact date. Occupancy accepts a date and multiple rooms. All large collections use searchable entity pickers; semesters use a horizontal card rail.

Schedule events may omit a teacher or room. Selecting an event opens the shared side inspector. The current gateway does not expose exact-date teacher/room queries, room occupancy projection, dashboard health, or active-user presence; their UI states remain explicit until those routes are available.

## Workload

Faculty workload is loaded through the scoped faculty entrypoint after faculty and semester selection. Operators may keep all groups or narrow the rendered projection to selected groups. Activity types become dynamic hour columns.

Department workload uses faculty → semester → department context. Rows are grouped by discipline and expose activity cells containing planned hours, room, and teacher; missing assignments remain explicit. The gateway load serializer currently omits the student-group code, so the UI falls back to the speciality name and course when no group name is present.

Selecting an activity cell opens an assignment inspector that creates or updates the existing teaching-assignment and room-reference join resources. A separate inspector creates a curriculum stream and its workload requirement together using searchable discipline and group selection.

## Group calendar

Calendar context follows faculty → semester → group. Lecture, credit, and exam periods are independent date ranges and map to `lecture_week`, `credit_week`, and `exam_session`. A scheduled occurrence may be moved without changing its recurring event template; the editor writes an event transfer from the side inspector.

Schedule event details can update day, time slot, room, state, and administrative reason or delete the recurring event. The current gateway create contract still requires a room, even though schedule reads correctly support missing rooms; creating roomless events remains a documented gateway contract gap.

## Catalog

Specialities are always loaded after faculty selection. Student groups require faculty and semester context and are displayed as `<speciality>-<course>` because the current schema has no separate group code. Creating a group selects a faculty speciality and admission year, then uses the existing automatic provisioning action to generate its semester records.

Reservations start with room text search and a date. Creation uses a side inspector with searchable teacher selection and readable time slots. Reference collections remain available as compact cards and never expose database identifiers.

## Accounts

Account creation uses a side inspector. Faculty and department administrators receive searchable access-area selectors; the operator never enters scope identifiers. Existing scopes resolve to readable names when the associated catalog data is available.

## Local visual preview

In Vite development only, append `?admin-preview=1` to render the super-administrator shell without creating a session. Production builds ignore the query parameter. The preview exists for responsive visual verification and must not be used as an authentication path.
