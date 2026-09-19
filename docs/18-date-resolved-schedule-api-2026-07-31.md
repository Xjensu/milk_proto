# Date-Resolved Schedule API — Frontend Reference

**Date:** 2026-07-31
**Base URL (dev):** `http://localhost:2300`
**Design doc:** `docs/superpowers/specs/2026-07-31-date-resolved-schedule-design.md`

## What changed, in one paragraph

The user no longer picks a semester. The server holds Moscow date/time, matches it against
**each group's own configured semester window**, and returns a real two-week calendar —
current week + next week — with class transfers already applied to the occurrences *and*
listed separately for a "changes" panel. `semester_id` is gone from every portal schedule
endpoint. Room occupancy moved out of the public portal into `/admin`.

## Breaking changes

| What | Action required |
|---|---|
| `semester_id` removed from `/public/faculties/:id/groups`, `/public/teachers/:id/schedule`, `/public/kafedras/:id/schedule` | Stop sending it. Sending it anyway is harmless — it is ignored, not rejected. |
| Portal schedule responses are no longer JSON:API | Read `.occurrences` / `.transfers` / `.meta` instead of `.data[].attributes`. |
| `GET /public/rooms/:room_id/occupancy` **deleted** | Use `GET /admin/rooms/:room_id/occupancy` (requires auth). |
| `GET /public/rooms/:room_id/schedule` **deleted** | Use `GET /admin/rooms/:room_id/occupancy` or the existing `GET /admin/rooms/:room_id/schedule`. |
| `GET /public/semesters` | Still works, now **deprecated**. Nothing should need it. |
| Per-event `GET /schedule_events/:id/transfers` fan-out | No longer needed on portal pages — transfers arrive composed. The admin day-grid may keep using it. |

## Core concept: the occurrence window

Every schedule endpoint returns the same envelope.

```jsonc
{
  "meta": {
    "resolved_at": "2026-07-31T17:48:09+03:00",  // server time, Moscow
    "anchor_date": "2026-07-31",                 // the date the window was built around
    "timezone": "Europe/Moscow",
    "window":  { "start_date": "2026-07-27", "end_date": "2026-08-09" },
    "weeks": [
      { "start_date": "2026-07-27", "end_date": "2026-08-02",
        "parity": "even", "iso_week": 30, "is_current": true  },
      { "start_date": "2026-08-03", "end_date": "2026-08-09",
        "parity": "odd",  "iso_week": 31, "is_current": false }
    ],
    "semester": { /* see below — may be null */ }
  },
  "occurrences": [ /* real classes on real dates */ ],
  "transfers":   [ /* the change list */ ],
  "holidays":    [ { "date": "2026-09-02", "name": "…" } ]
}
```

The window is always **Monday→Sunday of the anchor's week, plus the following week** (14 days).
`weeks[].is_current` marks which one contains the anchor — use it to pick the default tab.

**`meta.window` is the calendar range; `occurrences` may cover less of it.** Occurrences are
clipped to each group's own semester period, so a semester starting or ending mid-window yields
a partial fortnight. Example: group 513's semester runs 29.07→11.11; anchoring at 09.11 returns
a `window` of 09.11→22.11 but occurrences only on 09.11 and 10.11. Render empty days as empty —
do not infer "no data" from a day having no entries. `transfers[]` is clipped the same way: a
transfer survives when either `original_date` or `new_date` falls inside an active semester.

### `occurrences[]`

One entry per class **on a real calendar date**. Recurrence, week parity, one-off events,
holidays, weekday transfers and class transfers are already resolved server-side.

```jsonc
{
  "date": "2026-07-27",              // the date it actually happens
  "state": "scheduled",              // "scheduled" | "moved_in" | "cancelled" | "reserved"
  "day_of_week": 1,                  // 1=Mon … 7=Sun
  "specific_date": null,             // non-null = a one-off event, not a weekly recurrence
  "subgroup_number": 0,              // 0 = whole group; 1/2 = lab sub-group
  "status": "scheduled",
  "time_slot_start_time": "08:30:00",
  "time_slot_end_time":   "09:55:00",
  "week_type_name": "even",
  "room_name": "Лаборатория теплотехники №156",
  "room_number": "156",
  "subject_name": "Философия",
  "subject_short_name": "Философия",
  "activity_type_name": "лекция",
  "group_semester_id": 513,
  "group_display_name": "СИ 3 курс",
  "course": 3,
  "speciality_name": "СИ",
  "kafedra_name": "Философия, история и политология",
  "teachers": [
    { "id": 198, "last_name": "Рябцева", "first_name": "Наталья",
      "middle_name": "Адамовна", "event_teacher_id": 20, "teacher_on_kafedra_id": 8 }
  ],
  "original_date": null,             // set when state = "moved_in"
  "transfer_id": null,               // set when state != "scheduled"
  "transfer_reason": null
}
```

**How to render each `state`:**

| `state` | Meaning | Suggested rendering |
|---|---|---|
| `scheduled` | Normal class on its usual date | Normal card |
| `moved_in` | Moved **to** this date from `original_date` | Badge "перенесено с {original_date}"; room/time already reflect the new placement |
| `cancelled` | Cancelled on this date | Struck through / greyed — deliberately still returned so the class does not silently vanish |
| `reserved` | Room booking, not a class (admin room endpoint only) | Distinct style; `activity_type_name` is `"Бронирование"`, group fields are `null` |

A class moved *away* from a date simply does not appear on that date — no placeholder.

### `transfers[]`

The standalone change list, for a "переносы" panel. Contains **every** transfer touching the
window, including ones whose resulting occurrence landed outside it.

```jsonc
{
  "id": 26,
  "transfer_type": "moved",          // "moved" | "cancelled"
  "original_date": "2026-07-27",
  "new_date": "2026-07-28",          // null when cancelled
  "reason": null,
  "subject_name": "Веб-технологии",
  "activity_type_name": "лекция",
  "group_display_name": "СИ 3 курс",
  "room_name": "Радиационная безопасность",   // the ORIGINAL room, not the new one
  "room_number": "155",
  "time_slot_start_time": "08:30:00",
  "time_slot_end_time": "09:55:00",
  "teachers": [ /* … */ ]
}
```

Note the enrichment fields describe the class **as it was originally scheduled**. The new
placement is on the corresponding `moved_in` occurrence.

### `meta.semester`

```jsonc
{ "group_semester_id": 513, "semester_id": 1, "number": 1,
  "label": "1 семестр 2025/2026",
  "start_date": "2026-07-29", "end_date": "2026-11-11" }
```

- **Group endpoint** → a single `semester` object, or `null`.
- **Teacher endpoint** → `meta.semesters` (**array**, plural) — a teacher spans groups whose
  semester windows differ.
- **Kafedra and admin room endpoints** → `semester` is always `null` (many groups, no single window).

**`semester: null` with empty `occurrences` is a valid 200, not an error.** It means the group
exists but has no semester period configured covering that date. Show "расписание на эту дату
не настроено", not an error state.

---

## Endpoints

### `GET /public/faculties/:faculty_id/groups`

The entry point. Given a faculty, returns the groups whose own semester window contains the
date **and** that actually have a schedule.

| Param | Required | Notes |
|---|---|---|
| `date` | no | `YYYY-MM-DD`. Defaults to Moscow today. |

```jsonc
{
  "meta": { "resolved_at": "…", "anchor_date": "2026-07-31", "timezone": "Europe/Moscow" },
  "groups": [
    { "group_semester_id": 513, "group_id": 82, "course": 3,
      "speciality_name": "СИ", "students_now": 1,
      "year_of_admission": "2023-09-01", "duration_of_study": 4,
      "semester": { "semester_id": 1, "number": 1, "label": "1 семестр 2025/2026",
                    "start_date": "2026-07-29", "end_date": "2026-11-11" } }
  ]
}
```

Use `group_semester_id` for every subsequent schedule call. Group by `course` for the UI.

**A group is omitted when any of these hold** — all three are intentional:
1. it has no `semester` period covering the date;
2. it has **no schedule events at all**;
3. its computed course exceeds the speciality's `duration_of_study`.

So an empty `groups: []` is normal and must render as "нет групп с расписанием на эту дату".

### `GET /public/groups/:group_semester_id/schedule`

Two-week occurrence window for one group. Param: optional `date`.
Returns the full envelope with `meta.semester` as a single object.

### `GET /public/teachers/:teacher_id/schedule`

Same envelope. `meta.semesters` is an **array**. Occurrences span all the teacher's groups.
Pair with `GET /public/teachers?search=` to find the teacher first.

### `GET /public/kafedras/:kafedra_id/schedule`

Same envelope, union of all the kafedra's schedule-bearing group-semesters active on the
anchor date. `meta.semester` is `null`.

### `GET /public/groups/:group_semester_id/now`

Unchanged in shape. "Now" now means **Moscow now** when `datetime` is omitted.
Optional `datetime` (ISO 8601) overrides it.

### `GET /admin/rooms/:room_id/occupancy` 🔒

Requires `Authorization: Bearer <token>`.

| Param | Behaviour |
|---|---|
| *(none)* | Current + next week |
| `date=YYYY-MM-DD` | That single day — `window.start_date == window.end_date`, `weeks` holds one descriptor |

Same envelope, `meta.semester: null`. Occurrences include **internal ids**
(`room_id`, `subject_id`, `kafedra_id`, `activity_type_id`, `group_id`, `time_slot_id`,
`week_type_id`, event `id`) — the public portal deliberately omits these; admin needs them for
links and drag targets. Active `room_reservations` are merged in as `state: "reserved"`.

### Multiple teachers and multiple rooms per class 🔒

**Teachers — already supported, nothing new.** `event_teachers` is a join table; several rows
per event means several teachers. A unique index on `(event_id, teacher_on_kafedra_id)` blocks
the same teacher twice.

| | |
|---|---|
| `POST /admin/event_teachers` | body `{"event_teacher":{"event_id":1,"teacher_on_kafedra_id":2}}` |
| `DELETE /admin/event_teachers/:id` | `:id` is the `event_teacher_id` carried in each `teachers[]` entry |

Every occurrence's `teachers[]` already returns **all** of them — render the whole array, never
`teachers[0]`.

**Rooms — new.** `schedule_events.room_id` stays the **primary** room (NOT NULL, unchanged, still
what conflict detection and `event_transfers.new_room_id` operate on). A new `event_rooms` table
holds only the **additional** rooms.

| | |
|---|---|
| `GET /admin/event_rooms?event_id=38` | `{"data":[{"id":1,"event_id":38,"room_id":1,"room_name":"…","room_number":"101"}]}` |
| `POST /admin/event_rooms` | body `{"event_room":{"event_id":38,"room_id":1}}` → `201` |
| `DELETE /admin/event_rooms/:id` | `204` |

Guards (all `422`): attaching the event's own primary room → `"is already the primary room of
this event"`; attaching a duplicate → `"is already attached to this event"`; exceeding the limit
→ `"event already has the maximum of 2 rooms"`.

**A class may carry at most 2 teachers and at most 2 rooms**, counting the primary of each.
Neither join table has a cardinality constraint, so this is a product rule enforced in the
create actions (`APIGeteway::EventLimits`) as well as in the UI — a UI-only guard would be
advisory, since these endpoints are a public API surface. A third teacher returns
`422 {"errors":{"base":["event already has the maximum of 2 teachers"]}}`.

Occurrences now carry **`additional_rooms: [{id, name, number}]`** next to
`room_name`/`room_number` — usually `[]`. Render primary + extras together.

Two consequences worth knowing:
- **A room reads as occupied when it is a secondary room too.** `/admin/rooms/:id/occupancy`
  matches events where the room is either primary or in `event_rooms`. Verified: room 1 shows
  event 38 (whose primary room is 5) on the dates it runs.
- **Additional rooms are series-level.** `event_rooms` has no per-date variant, so — like
  `event_teachers` — attaching a room applies to every occurrence. A single-date room change is
  still an `event_transfer` with `new_room_id`.

### Calendar periods and their own timetables 🔒

*Added 2026-08-01.*

A group-semester's periods are dated rows in `student_group_semester_periods`:
`period_kind ∈ semester | lecture_week | credit_week | exam_session`, each with
`start_date`/`end_date`.

**Each period has its own timetable.** `week_types` now carries a code per period
alongside the parity codes:

| week_types.code | meaning |
|---|---|
| `even` / `odd` / `both` | the ordinary semester timetable |
| `lecture_week` / `credit_week` / `exam_session` | that period's own timetable |

Occurrence resolution swaps the active code set when a date falls inside a special
period, so **the special schedule replaces the ordinary one** for those dates — a
lecture week does not also run the normal parity timetable. Duration is the
period's own `start_date`/`end_date`; there is no separate duration setting.

Resolution is per group-semester, so one group can be in a session while another
is in ordinary weeks, which matters on the teacher and kafedra views.

| | |
|---|---|
| `GET /admin/student_group_semesters/:id/periods` | configured periods |
| `POST` / `PATCH` `…/periods` | upsert one period (one row per kind) |
| `DELETE /admin/student_group_semester_periods/:id` | remove a period |
| `GET /admin/student_group_semesters/:id/schedule?period=<kind>` | that period's grid; omit (or `semester`) for the ordinary one; a bogus value → `422` |

To create a class into a period, set its `week_type_id` to the week type whose
`code` equals the period kind. **Match on `code`, never on a hardcoded id** — ids
differ per environment.

`DELETE …/student_group_semester_periods/:id` matters more than it looks: a stray
dated period silently blanks part of a group's timetable for the dates it covers,
and until it existed a period added by mistake was permanent.

### `GET /admin/teachers/:teacher_id/notice` 🔒

The "Извещение" report. Params: `semester_id` (required), `period` (required, one
of the four kinds; anything else → `422`).

```jsonc
{
  "meta": {
    "teacher": { "id": 395, "last_name": "…", "first_name": "…", "middle_name": "…" },
    "period_kind": "semester", "period_label": "Учебный семестр",
    "start_date": "2026-07-29", "end_date": "2026-11-11",
    "semester": { … }
  },
  "rows": [
    { "date": "2026-08-04", "weekday": 2,
      "time_slot_start_time": "08:30:00", "time_slot_end_time": "09:55:00",
      "call_number": 1,
      "groups": ["СИ 3 курс"],                    // "Поток, группа"
      "activity_type_name": "лекция", "subject_name": "Веб-технологии",
      "rooms": ["Ауд. 1315", "Ауд. 101"] }        // primary first, then extras
  ]
}
```

Occurrences of the **same class taught to several groups collapse into one row** —
that is why the column is "Поток, группа". Cancelled occurrences are dropped: a
notice lists what must actually be taught.

A period with no dates configured for this teacher's groups returns `200` with
`start_date`/`end_date` `null` and `rows: []`. That is a configuration gap, not an
error — say so in the UI rather than showing an empty table.

### `GET /admin/calendar/week_parity` 🔒

Ask the server what parity a date has, instead of reimplementing the rule client-side.

```jsonc
{ "date": "2026-09-01", "parity": "odd", "iso_week": 35, "day_of_week": 2,
  "week_start": "2026-08-31", "week_end": "2026-09-06",
  "active_week_type_codes": ["odd", "both"] }
```

The rule lives in exactly one place server-side (`APIGeteway::WeekParity#code_for`). If it ever
changes — a different epoch, a university-specific offset — this endpoint changes with it and
no frontend edit is needed. **Do not compute parity in the client.**

---

## Errors

| Case | Status | Body |
|---|---|---|
| Malformed `date` (wrong shape) | 422 | `{"errors":{"date":["is in invalid format"]}}` |
| Well-shaped but impossible date (`2026-13-45`) | 422 | `{"errors":{"date":["is not a valid date"]}}` |
| Unknown id | 404 | `{"error":"not found"}` |
| Missing/invalid token on `/admin/*` | 401 | — |
| No semester configured for the date | **200** | `meta.semester: null`, empty arrays |

Treat both 422 messages the same way; the distinction is format-check vs calendar-check.

---

## Suggested page flows

**Public schedule page**
1. `GET /public/faculties` → faculty picker.
2. `GET /public/faculties/:id/groups` → group picker, grouped by `course`. No semester picker.
3. `GET /public/groups/:gsid/schedule` → render `meta.weeks` as two tabs, bucket `occurrences`
   by `date`, and show `transfers` in a side panel.
4. Week paging: re-request with `?date=` set to a date in the target week.

**Teacher schedule page**
1. `GET /public/teachers?search=…` → pick teacher.
2. `GET /public/teachers/:id/schedule` → identical rendering; `meta.semesters` is an array.

**Admin room occupancy**
1. `GET /admin/rooms` → room picker.
2. `GET /admin/rooms/:id/occupancy` for the fortnight view, or `?date=` for one day.
3. Style `state: "reserved"` differently from classes.

---

## Verified against live data (2026-07-31)

| Check | Result |
|---|---|
| Faculty 1 today (semester starts 01.09) | `groups: []` — correct |
| Faculty 4 today (semester 29.07–11.11) | returns gs 513 with course + label |
| Faculty 1 on `?date=2026-09-01` | `[]` — gs 8 fails the course rule, gs 788 has 0 events; both exclusions correct |
| Group 513 window | Mon 27.07 → Sun 09.08, parity even/odd, `is_current` set |
| Transfer composition | occurrences carry `moved_in` + `original_date` + `transfer_id`; a two-way swap (ids 26/28) renders on both dates |
| `transfers[]` | returned as a separate enriched list |
| Times | `"08:30:00"` (timestamp date component stripped) |
| Teacher / kafedra / now | 200 |
| `/public/rooms/:id/occupancy` | 404 — removed as designed |
| `/admin/calendar/week_parity` | matches the design doc's worked example exactly |
| `/admin/rooms/5/occupancy` and `?date=` | both modes correct |
| Room reservation overlay | renders as `state:"reserved"`, `"Бронирование"` |
| **Period replaces the ordinary schedule** | inside a dated lecture week only its own classes resolve; outside it nothing from that period leaks in |
| **`?period=` grid** | 4 rows for `semester`, 0 for an unconfigured period, `422` for a bogus value |
| **Notice report** | 14 rows for teacher 395 / semester 1, multi-room rendered as `Ауд. 1315, Ауд. 101` |
| **Period DELETE** | `204`, then `404` on repeat |
| **2-teacher / 2-room limit** | 2nd teacher `201`, 3rd `422`; 3rd room `422` |
| **group name in admin grid** | teacher and room views both return `СИ 3 курс` |
| Unauthenticated `/admin/*` | 401 |
| **`weekday_transfers`** | 09.11 (Mon, follows Tue) returns `day_of_week: 2`; control Mon 16.11 returns `day_of_week: 1` |
| **Holiday skip** | class present at baseline, gone once a holiday was inserted on that date, listed in `holidays[]` |
| **`cancelled` transfer** | occurrence kept with `state:"cancelled"` + `transfer_id` + reason, and listed in `transfers[]` with `new_date: null` |
| **One-off `specific_date`** | appears only on its pinned date with `specific_date` populated |
| **Semester clipping** | anchoring at 09.11 (semester ends 11.11) returns 0 occurrences past the end date; in-semester transfers survive |
| Legacy `?semester_id=` | 200 — ignored, not rejected |
| `/public/rooms/:id/schedule`, `/occupancy` | 404 |
| `/public/semesters`, `/public/rooms` | 200 (deprecated / retained) |
| `meta` shape | group → `semester` object; teacher → `semesters` array; kafedra + admin room → `semester: null` |

The holiday, cancelled-transfer and one-off checks required inserting temporary rows (no such
data existed); each was deleted afterwards and the table counts confirmed back at their
original values.

## Known limitations

- **Kafedra and multi-group windows issue one grid query per group-semester.** Fine at current
  scale; the obvious thing to batch if a kafedra ever spans hundreds of group-semesters.
- **Kafedra scoping is applied after expansion, not in SQL.** A group-semester can carry
  `curriculum_streams` from several kafedras (group 513 is taught by both kafedra 1 and 55), so
  the kafedra window expands the whole group and then filters on the internal `kafedra_id`.
  A transfer for a template that produced no occurrence anywhere in the window cannot be
  attributed to a kafedra and is dropped rather than risk a cross-kafedra leak.
- **A per-occurrence teacher substitution is not representable.** `event_teachers` has no
  per-date variant, so a teacher change applies to the whole series. Unchanged by this work.
- **The frontend is served from a built image, not a dev server.** `docker compose up -d` does
  NOT pick up code changes for either `frontend` or `api_geteway` — neither has a bind mount.
  Use `docker compose build <svc> && docker compose up -d --force-recreate <svc>`, or you will
  be looking at a stale bundle while the source says otherwise.
- **Two `week_types` rows (`test`, `test2026`) still carry 5 schedule events.** Those codes match
  no active code set, so those events never resolve into any schedule view. Pre-existing; worth
  cleaning up.
- **`week_type_name` reflects the DB value** (currently the literal `"even"`/`"odd"`), which is
  not a display string. Render from `meta.weeks[].parity` if you want localised labels.
