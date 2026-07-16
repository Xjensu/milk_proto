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

## Local visual preview

In Vite development only, append `?admin-preview=1` to render the super-administrator shell without creating a session. Production builds ignore the query parameter. The preview exists for responsive visual verification and must not be used as an authentication path.
