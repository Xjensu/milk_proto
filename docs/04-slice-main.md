# Slice: `main`

Two audiences, two mount paths:

- `/public/*` — **unauthenticated** schedule lookups for students/teachers/anyone (no
  `Authenticate` step at all — see the design note in `01-database-schema.md §2.1`).
- `/me/*` — authenticated self-service for logged-in staff (teachers, admins), requires
  `Authenticate` but no role-specific `Authorize` beyond "is a valid user" (each action is
  implicitly scoped to `current_user`).

All read endpoints here are backed by the shared `Schedule::DayExpansion` /
range-expansion query objects defined in `01-database-schema.md §4` — the same code the
`admin` slice uses for its schedule views. Never reimplement the expansion logic
per-endpoint; wrap it and shape the output for the audience.

## 1. Reference/selector endpoints (public)

Support the cascading-selector UX pattern used on every schedule page
(`07-frontend-pages.md`):

| Method | Path | Purpose |
|---|---|---|
| `GET` | `/public/faculties` | faculty picker |
| `GET` | `/public/faculties/:id/semesters` | semesters that currently have at least one group with schedule data — feeds the semester picker after faculty is chosen |
| `GET` | `/public/faculties/:id/student_groups?semester_id=` | group picker, `<name>-<course>` display strings, same shape as `03-slice-admin.md §4`'s equivalent but without auth |
| `GET` | `/public/teachers?q=` | typeahead search for the Teacher Schedule page |
| `GET` | `/public/rooms` | room list (optionally `?building_id=`, `?q=` text search over name/number — same semantics as `03-slice-admin.md §8`'s search; teachers use this for their self-service reservation flow since they cannot reach the admin-slice search) |
| `GET` | `/public/semesters/current` | server's notion of "current" semester (by today's date falling inside a `lecture_week`/`credit_week`/`exam_session` window for the majority of groups, or nearest upcoming) — used to default the date/semester pickers so users aren't forced to pick every time |

## 2. Student group schedule

### `GET /public/schedule/student_group/:student_group_semester_id`

Query: `date=YYYY-MM-DD` (single day) **or** `week_start=YYYY-MM-DD` (full week grid,
default if neither given: current week). Returns the expanded grid for that
group-semester:

```json
{
  "data": {
    "period": "lecture_week",
    "days": [
      {
        "date": "2025-03-10",
        "day_of_week": 1,
        "events": [
          {
            "time_slot": { "id": 2, "call_number": 1, "start_time": "08:00", "end_time": "09:35" },
            "subject": { "id": 5, "name": "Базы данных" },
            "activity_type": { "id": 1, "short_name": "Лек" },
            "teachers": [ { "id": 14, "full_name": "Петров И.И." } ],
            "room": { "id": 10, "name": "Ауд. 101" } ,
            "is_transferred": false
          }
        ]
      }
    ]
  }
}
```

`is_transferred: true` (plus `transfer_reason`) flags occurrences affected by an
`event_transfers` row, so the frontend can highlight them in a distinct color — directly
implements the ТЗ requirement "visual highlighting of changes in a color different from
the base one." Cancelled occurrences are simply omitted with a separate
`cancelled_events` array noting what was cancelled, so the frontend can still show a
struck-through entry rather than a silent gap.

`period` reflects which window `date`/`week_start` falls into
(`lecture_week`/`credit_week`/`exam_session`), driving which page chrome the frontend
shows (`07-frontend-pages.md §3`).

## 3. Teacher schedule

### `GET /public/schedule/teacher/:teacher_id`

Query: **either** `semester_id=` (returns the full recurring weekly grid for that
semester, all periods) **or** `date=YYYY-MM-DD` (single day, any period) — matches the
two lookup modes described for the Teacher Schedule page. Response shape mirrors §2 but
each event additionally carries the `student_group` display string (`"ПИ-23-1"`), since a
teacher's grid spans multiple groups:

```json
{ "data": { "days": [ { "date": "...", "events": [
  { "time_slot": {...}, "subject": {...}, "activity_type": {...},
    "student_group": { "id": 12, "display_name": "ПИ-23-1" },
    "room": {...} | null, "is_transferred": false }
] } ] } }
```

`room: null` is valid and expected (e.g. physical education, per the ТЗ's explicit "room
may be absent" note).

## 4. Room schedule / occupancy

### `GET /public/schedule/rooms?room_ids[]=&date=`

Occupancy for a **list** of rooms on one day (defaults to today if `date` omitted) —
matches "occupancy of a specific list of rooms for today or another day":

```json
{ "data": [
  { "room": { "id": 10, "name": "Ауд. 101" },
    "events": [ { "time_slot": {...}, "subject": {...}, "student_group": {...}, "teachers": [...] } ],
    "reservations": [ { "time_slot": {...}, "teacher": {...}, "purpose": "..." } ]
  }
] }
```

Includes the group in each slot per the requirement, and merges `room_reservations` into
the same per-room timeline as `schedule_events` so free/busy is unambiguous at a glance.
Backed by `mv_room_daily_occupancy` (`01-database-schema.md §5`) for the 90-day rolling
window; falls back to a live query outside that window.

## 5. Self-service (`/me/*`, authenticated)

| Method | Path | Notes |
|---|---|---|
| `GET` | `/me/schedule` | teacher's own schedule; identical shape to §3, `teacher_id` implied from `current_user.teacher_id`. `404` if the current user has no linked `teacher_id`. |
| `GET` | `/me/time_constraints` | own `teacher_time_constraints`, read-only (created by admins/dispatchers, not self-service, so no `POST` here) |
| `GET` | `/me/room_reservations` | own reservations |
| `POST` | `/me/room_reservations` | same validation as `03-slice-admin.md §8`, `teacher_id` forced to `current_user.teacher_id` regardless of body content — implements "teacher reserves a room" as genuine self-service, not just a dispatcher action |
| `DELETE` | `/me/room_reservations/:id` | `404` (not `403`) if the reservation belongs to someone else — avoid confirming existence of other teachers' reservations by id |
