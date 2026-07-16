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

## Group calendar

Calendar context follows faculty → semester → group. Lecture, credit, and exam periods are independent date ranges and map to `lecture_week`, `credit_week`, and `exam_session`. A scheduled occurrence may be moved without changing its recurring event template; the editor writes an event transfer from the side inspector.

## Local visual preview

In Vite development only, append `?admin-preview=1` to render the super-administrator shell without creating a session. Production builds ignore the query parameter. The preview exists for responsive visual verification and must not be used as an authentication path.
