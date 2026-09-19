# Date-Resolved Portal Schedule — Design

**Date:** 2026-07-31
**Scope:** `apps/api_geteway` — `portal` slice, `admin` slice, shared `lib/api_geteway/`

## Problem

Every portal schedule endpoint requires the client to pass `semester_id`. The user has to
pick a semester before they can see a timetable, and the API has no notion of "now".

Meanwhile the schedule returned is a pure recurrence template: `schedule_events` rows carry
`day_of_week` + `week_type_id` (+ an optional `specific_date` for one-offs). Per-occurrence
moves and cancellations live in `event_transfers` and are **not composed by the backend at
all** — the frontend resolves them client-side with one `GET /schedule_events/:id/transfers`
per event.

## Goal

The server holds Moscow date/time, matches it against each group's own configured semester
window, and returns a real two-week calendar with transfers applied and listed.

## Established facts (verified against the codebase)

- Per-group semester dates live in `student_group_semester_periods`, `period_kind = 'semester'`
  (migration `20260728120001_widen_group_period_kinds`). Columns: `student_group_semester_id`,
  `start_date`, `end_date`. Unique on `(student_group_semester_id, period_kind)`.
- `semesters` carries **no dates** — only `number`, `start_year`, `end_year`. Date resolution
  therefore *must* go through `student_group_semester_periods`.
- There is **no `teacher_events` table**. Teacher linkage is
  `event_teachers` → `teacher_on_kafedras` → `teachers`.
- `event_transfers`: `schedule_event_id`, `original_date`, `transfer_type ∈ {moved, cancelled}`,
  `new_date`, `new_time_slot_id`, `new_room_id`, `reason`. Unique on
  `(schedule_event_id, original_date)`.
- `holidays`: `holiday_date` (unique), `name`.
- `weekday_transfers`: `transfer_date` (unique globally when `student_group_id IS NULL`),
  `follows_weekday` (1..6), optional `student_group_id`.
- `schedule_events.specific_date` — `NULL` means recurring template, non-`NULL` pins the row
  to one calendar date.
- The parity rule (`ISO week even → "even", odd → "odd"`) is currently written out in **three**
  places: `APIGeteway::WeekParity`, `Portal::Relations::WeekTypes#id_by_time`,
  `Admin::Relations::WeekTypes#id_by_time`.

## Decisions

| # | Decision | Rationale |
|---|---|---|
| D1 | Moscow time = **fixed `+03:00` offset** | MSK has had no DST since 2011. A fixed offset avoids adding a tzdata gem to the Gemfile. |
| D2 | Group with **no `period_kind='semester'` row is excluded** | Strict. No guessed dates. Missing configuration is visible, not silently wrong. |
| D3 | `semester_id` **removed** from portal schedule endpoints; optional `?date=YYYY-MM-DD` added | One resolution path, not two. `date` enables week paging and deterministic verification without mocking the clock. |
| D4 | Window is **fixed at current + next week** (Mon–Sun ISO weeks, 14 days) | Matches the requirement exactly; predictable payload size. |
| D5 | Room occupancy + room schedule **move to `admin`**; `GET /public/rooms` index stays | Occupancy is an operations concern. The index still backs public pickers and map/search. |
| D6 | New composite endpoints return **plain JSON**, not JSON:API | The response carries `meta` plus three parallel collections. JSON:API would force `transfers`/`holidays` into `meta`. Existing single-collection endpoints keep JSON:API unchanged. |
| D7 | Kafedra schedule = **union** of that kafedra's schedule-present group-semesters active on the anchor date | A kafedra spans groups with different semester windows; there is no single kafedra-level semester. |
| D8 | Parity rule consolidated into `APIGeteway::WeekParity`; both `WeekTypes#id_by_time` delegate | The stated requirement is that this rule may change. It must exist in exactly one place. |
| D9 | **Occurrences are clipped to each group's own semester period** | Added 2026-07-31 after audit. The original spec specified only "current + next week" and produced classes *after* the semester had ended (group 513: 3 occurrences past its 11.11 end date). The requirement was "расписание **на этот семестр**". Clipping is per-occurrence against its own `group_semester_id`'s period — a single global min/max would leak the widest group's dates onto the narrowest. Occurrences whose group-semester has no resolved context are kept, so the D2 misconfiguration stays visible rather than being silently hidden. Transfers are clipped by date, since `TransferNotice` carries no `group_semester_id`. |

## Architecture

Three layers, all slice-agnostic code in `lib/api_geteway/` so `portal` and `admin` share it —
following the existing `ScheduleGrid` / `SchedulePresence` precedent (POROs receiving a ROM
relation or raw Sequel DB at construction, because ROM relations are scoped per slice and
cannot be resolved globally).

### Layer 1 — Calendar primitives

**`APIGeteway::Calendar::Clock`** (new, `lib/api_geteway/calendar/clock.rb`)

```ruby
MOSCOW_OFFSET = "+03:00"
now          -> Time    # Time.now.getlocal(MOSCOW_OFFSET)
today        -> Date
anchor(param) -> Date   # nil/blank → today; else Date.parse (ArgumentError propagates → 422)
```

**`APIGeteway::WeekParity`** (extended in place, `lib/api_geteway/week_parity.rb`)

```ruby
code_for(date)       -> "even" | "odd"        # THE RULE — the single line that changes
codes_for(date)      -> [code_for(date), "both"]
day_of_week(date)    -> 1..7                   # ISO, Mon=1
week_start(date)     -> Date                   # Monday of date's week
week_end(date)       -> Date                   # Sunday of date's week
week_descriptor(date, current_week_start:) -> { start_date:, end_date:, parity:, iso_week:, is_current: }
window(anchor, weeks: 2) -> { start_date:, end_date:, weeks: [descriptor, ...] }
```

`Portal::Relations::WeekTypes#id_by_time` and `Admin::Relations::WeekTypes#id_by_time` are
rewritten to call `APIGeteway::WeekParity.code_for`. Their public signatures and return values
do not change, so `RoomOccupancyNow` and every existing caller keep working.

### Layer 2 — Semester resolution

**`APIGeteway::Queries::SemesterResolver`** (new, `lib/api_geteway/queries/semester_resolver.rb`)

Constructed with a ROM relation (`.dataset.naked.db`), matching `SchedulePresence`.

```ruby
SemesterContext = Struct.new(
  :group_semester_id, :semester_id, :semester_number, :start_year, :end_year,
  :start_date, :end_date, :label, keyword_init: true
)

active_group_semester_ids(date)                       -> Sequel dataset (subquery-usable)
resolve_group_semester(group_semester_id:, date:)     -> SemesterContext | nil
active_for_faculty(faculty_id:, date:)                -> [StudyingGroupResult + semester ctx]
active_for_teacher(teacher_id:, date:)                -> [group_semester_id]
active_for_kafedra(kafedra_id:, date:)                -> [group_semester_id]
```

`label` is built as `"#{number} семестр #{start_year}/#{end_year}"`.

All listing methods intersect with `SchedulePresence#group_semester_ids` so a group is only
returned when it actually has schedule events (D2 + the existing portal-wide rule).

### Layer 3 — Occurrence expansion

**`APIGeteway::Queries::ScheduleOccurrences`** (new, `lib/api_geteway/queries/schedule_occurrences.rb`)

Constructed with the slice's `schedule_events` relation; internally builds its own
`ScheduleGrid` and reads `event_transfers`, `holidays`, `weekday_transfers` off the raw DB.

```ruby
Occurrence = Struct.new(
  # every public ScheduleGridRow field, plus:
  :date,                 # Date this occurrence actually happens on
  :state,                # "scheduled" | "moved_in" | "cancelled"
  :original_date,        # set when state == "moved_in"
  :transfer_id,          # set when state != "scheduled"
  :transfer_reason,
  keyword_init: true
)

TransferNotice = Struct.new(
  :id, :schedule_event_id, :transfer_type, :original_date, :new_date,
  :new_time_slot_id, :new_room_id, :reason,
  :subject_name, :subject_short_name, :activity_type_name,
  :group_display_name, :room_name, :room_number,
  :time_slot_start_time, :time_slot_end_time, :teachers,
  keyword_init: true
)

OccurrenceWindow = Struct.new(
  :anchor_date, :start_date, :end_date, :weeks,
  :occurrences, :transfers, :holidays, keyword_init: true
)

for_group_semester(group_semester_id:, anchor:)   -> OccurrenceWindow
for_teacher(teacher_id:, group_semester_ids:, anchor:) -> OccurrenceWindow
for_kafedra(kafedra_id:, group_semester_ids:, anchor:) -> OccurrenceWindow
for_room(room_id:, anchor:, date: nil)            -> OccurrenceWindow
```

#### Expansion algorithm

Given a set of `ScheduleGridRow` templates and a date range `[from, to]`:

1. **Load overlays once** for the whole range (never per-date):
   - `holidays WHERE holiday_date BETWEEN from AND to`
   - `weekday_transfers WHERE transfer_date BETWEEN from AND to`
   - `event_transfers WHERE schedule_event_id IN (template ids) AND (original_date BETWEEN from AND to OR new_date BETWEEN from AND to)`

   The `OR new_date BETWEEN` half is **load-bearing**: an occurrence moved *into* the window
   from a date outside it would otherwise never be fetched and would silently disappear.

2. **For each date `D` in the range:**
   - If `D` is a holiday **and** no `weekday_transfers` row targets `D`, skip `D` entirely.
   - Effective weekday: a `weekday_transfers` row for `D` supplies `follows_weekday`;
     a group-scoped row (`student_group_id` matching) wins over a global row
     (`student_group_id IS NULL`). Otherwise `WeekParity.day_of_week(D)`.
   - Emit a candidate for each template where either:
     - `specific_date == D`, or
     - `specific_date.nil?` **and** `day_of_week == effective_weekday` **and**
       `week_type_code ∈ WeekParity.codes_for(D)`

3. **Apply transfers**, keyed on `(schedule_event_id, D)`:
   - `cancelled` → keep the occurrence at `D`, set `state: "cancelled"`, attach
     `transfer_id` / `transfer_reason`. It stays visible so the UI can strike it through.
   - `moved` → drop the candidate at `D`; emit a new occurrence at `new_date` with
     `state: "moved_in"`, `original_date: D`, and `time_slot_*` / `room_*` overridden from
     `new_time_slot_id` / `new_room_id` when those are non-`NULL`. Emit it only if `new_date`
     falls inside `[from, to]`.

4. **Sort** occurrences by `(date, time_slot_start_time, subgroup_number)`.

5. **Build `transfers`** from every `event_transfers` row loaded in step 1, enriched with the
   subject/group/room/teacher fields of its parent event, regardless of whether its resulting
   occurrence landed inside the window. This is the standalone list the frontend renders as a
   "changes" panel.

Note that step 3 cannot be folded into the SQL of step 2: a `moved` transfer relocates an
occurrence onto a date the template's `WHERE` clause never matched, so no single predicate
produces the correct set.

**`APIGeteway::Queries::RoomOccupancyWindow`** (new, `lib/api_geteway/queries/room_occupancy_window.rb`)

Wraps `ScheduleOccurrences#for_room` and overlays `room_reservations`
(`WHERE room_id = ? AND status = 'active' AND reserved_date BETWEEN from AND to`), emitting them
as occurrences with `state: "reserved"` so a room's day view shows classes and reservations together.

## Endpoints

### Portal (`/public`)

| Method | Path | Params | Returns |
|---|---|---|---|
| GET | `/public/faculties/:faculty_id/groups` | `date?` | groups + course + resolved semester |
| GET | `/public/groups/:group_semester_id/schedule` | `date?` | occurrence window |
| GET | `/public/groups/:group_semester_id/now` | `datetime?` | current class (Moscow clock, transfer-aware) |
| GET | `/public/teachers/:teacher_id/schedule` | `date?` | occurrence window |
| GET | `/public/kafedras/:kafedra_id/schedule` | `date?` | occurrence window (D7 union) |
| GET | `/public/rooms` | `page?`, `per_page?` | unchanged |
| GET | `/public/semesters` | — | unchanged, **deprecated** |
| ~~GET~~ | ~~`/public/rooms/:room_id/occupancy`~~ | | **removed** → admin |
| ~~GET~~ | ~~`/public/rooms/:room_id/schedule`~~ | | **removed** → admin |

`semester_id` is removed from `faculties/groups`, `teachers/schedule` and `kafedras/schedule`
contracts. Passing it is ignored, not rejected — an unknown key must not 422 an otherwise
valid request.

### Admin (`/admin`)

| Method | Path | Params | Returns |
|---|---|---|---|
| GET | `/admin/rooms/:room_id/occupancy` | `date?` | with `date`: that single day. Without: current + next week. |
| GET | `/admin/rooms/:room_id/schedule` | `semester_id?`, `date?` | unchanged (existing endpoint) |
| GET | `/admin/calendar/week_parity` | `date?` | the parity descriptor for the admin UI |

`/admin/rooms/:room_id/occupancy` is authorized `resource: "rooms", action: :read`, matching
the sibling `rooms.schedule` action.

## Response shapes

### Occurrence window (portal group/teacher/kafedra schedule, admin room occupancy)

```json
{
  "meta": {
    "resolved_at": "2026-09-01T10:23:00+03:00",
    "anchor_date": "2026-09-01",
    "timezone": "Europe/Moscow",
    "window": { "start_date": "2026-08-31", "end_date": "2026-09-13" },
    "weeks": [
      { "start_date": "2026-08-31", "end_date": "2026-09-06", "parity": "odd",  "iso_week": 35, "is_current": true  },
      { "start_date": "2026-09-07", "end_date": "2026-09-13", "parity": "even", "iso_week": 36, "is_current": false }
    ],
    "semester": {
      "group_semester_id": 8, "semester_id": 3, "number": 1,
      "label": "1 семестр 2026/2027",
      "start_date": "2026-09-01", "end_date": "2027-01-31"
    }
  },
  "occurrences": [
    {
      "date": "2026-09-01", "state": "scheduled", "day_of_week": 2,
      "time_slot_start_time": "08:00:00", "time_slot_end_time": "09:30:00",
      "week_type_name": "Нечётная", "room_name": "...", "room_number": "301",
      "subject_name": "...", "subject_short_name": "...",
      "activity_type_name": "Лекция", "subgroup_number": 0,
      "group_semester_id": 8, "group_display_name": "...", "course": 2,
      "speciality_name": "...", "kafedra_name": "...",
      "teachers": [{ "id": 4, "last_name": "...", "first_name": "...", "middle_name": "..." }],
      "original_date": null, "transfer_id": null, "transfer_reason": null
    }
  ],
  "transfers": [
    {
      "id": 12, "transfer_type": "moved",
      "original_date": "2026-09-03", "new_date": "2026-09-05",
      "reason": "...", "subject_name": "...", "activity_type_name": "Лекция",
      "group_display_name": "...", "room_name": "...", "room_number": "301",
      "time_slot_start_time": "08:00:00", "time_slot_end_time": "09:30:00",
      "teachers": [ ... ]
    }
  ],
  "holidays": [ { "date": "2026-09-02", "name": "..." } ]
}
```

`meta.semester` is `null` for the kafedra endpoint (D7 — no single semester) and for the admin
room endpoint. For the teacher endpoint it is an array under `meta.semesters` instead, since a
teacher may span groups in different semester windows.

### Faculty groups (date-resolved)

```json
{
  "meta": {
    "resolved_at": "2026-09-01T10:23:00+03:00",
    "anchor_date": "2026-09-01",
    "timezone": "Europe/Moscow"
  },
  "groups": [
    {
      "group_semester_id": 8, "group_id": 3, "course": 2,
      "speciality_name": "...", "students_now": 24,
      "year_of_admission": "2025-09-01", "duration_of_study": 4,
      "semester": {
        "semester_id": 3, "number": 1, "label": "1 семестр 2026/2027",
        "start_date": "2026-09-01", "end_date": "2027-01-31"
      }
    }
  ]
}
```

### Week parity (admin)

```json
{
  "date": "2026-09-01", "parity": "odd", "iso_week": 35, "day_of_week": 2,
  "week_start": "2026-08-31", "week_end": "2026-09-06",
  "active_week_type_codes": ["odd", "both"]
}
```

## Error handling

- Malformed `date` / `datetime` → `422 { "errors": { "date": ["is not a valid date"] } }`,
  matching the existing `Groups::Now#parse_datetime` shape.
- `group_semester_id` with no `period_kind='semester'` row covering the anchor date →
  `200` with `meta.semester: null` and empty `occurrences`. Not a 404: the group exists,
  it simply has no semester configured for that date, and the frontend should say so.
- Unknown `group_semester_id` / `teacher_id` / `room_id` → `404`.

## Out of scope

- Tests. Verification is live HTTP probing against the running stack.
- Frontend changes (`apps/frontend` is a separate nested git repo).
- Any change to `event_transfers` write endpoints — this work is read-side only.
- Migrations. Every table this design needs already exists.
