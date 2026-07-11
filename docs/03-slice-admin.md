# Slice: `admin`

Mount path: `/admin/*`. Every action requires `Authenticate` + `Authorize` (see
`02-slice-authentication.md §5`). All list endpoints support `page`/`per_page` query
params (default `per_page=50`, max `200`) and return the pagination envelope defined in
`05-backend-patterns.md §6`.

## 1. User & role management (`super_admin` only)

| Method | Path | Notes |
|---|---|---|
| `GET` | `/admin/users` | filter by `status`, `role_code`, `q` (name/email search) |
| `POST` | `/admin/users` | `{ email, full_name, temporary_password, teacher_id? }` → creates user with `must_change_password: true`. `422` on duplicate email. |
| `PATCH` | `/admin/users/:id` | update `full_name`, `status` (enable/disable) |
| `POST` | `/admin/users/:id/role_assignments` | `{ role_id, scope_type, faculty_id?, kafedra_id? }` — add a role grant |
| `DELETE` | `/admin/role_assignments/:id` | revoke a grant |
| `POST` | `/admin/users/:id/reset_password` | admin-forced reset: generates a new temporary password, sets `must_change_password: true`, revokes all refresh tokens for that user |

Disabling a user (`status: 'disabled'`) immediately invalidates their session within ≤60s
via the Redis-cached status check (`02-slice-authentication.md §5`) and revokes all
refresh tokens synchronously. Adding or revoking a `role_assignment` increments the
user's `roles_version:{user_id}` Redis counter in the same transaction hook, invalidating
outstanding access tokens' stale role snapshots within seconds
(`02-slice-authentication.md §5`).

## 2. Static reference data — standard CRUD resources

These follow one uniform pattern: `GET` (list, paginated, optionally
`?q=` name search), `GET /:id`, `POST`, `PATCH /:id`, `DELETE /:id` (hard delete blocked
with `409` if referenced by dependent rows — see `05-backend-patterns.md §3` for the FK
guard pattern). Payload key is the singular resource name wrapping its attributes, per
the confirmed `БД.txt` contract (e.g. `{ "activity_type": { "name": "...", ... } }`).

| Resource | Path | Key fields | Required role |
|---|---|---|---|
| Activity types | `/admin/activity_types` | `name`, `short_name` | `super_admin` |
| Building types | `/admin/building_types` | `name`, `short_name` | `super_admin` |
| Week types | `/admin/week_types` | `name`, `code` | `super_admin` (seed-only in practice; `code` immutable after creation — `PATCH` rejects `code` changes with `422`) |
| Time slots | `/admin/time_slots` | `start_time`, `end_time`, `call_number` | `super_admin` |
| Posts | `/admin/posts` | `name`, `short_name` | `super_admin` |
| Academic degrees | `/admin/academic_degrees` | `name`, `short_name` | `super_admin` |
| Academic titles | `/admin/academic_titles` | `name`, `short_name` | `super_admin` |
| Subjects | `/admin/subjects` | `name`, `short_name` | `super_admin`, `faculty_admin`, `kafedra_admin` |
| Semesters | `/admin/semesters` | `start_year`, `end_year`, `number` | `super_admin` (also auto-created by the student-group bulk entrypoint, §4) |
| Room types | `/admin/room_types` | `name`, `activity_type_id` (sent as **string**, per confirmed contract) | `super_admin`, `dispatcher` |
| Buildings | `/admin/buildings` | `building_type_id`, `number` (string) | `super_admin`, `dispatcher` |
| Rooms | `/admin/rooms` | `building_id`, `room_type_id`, `name`, `number` — **no `capacity`** | `super_admin`, `dispatcher` |

`GET /admin/rooms` returns each room with nested `building` and `room_type` (confirmed
contract). All negative-test contracts from `БД.txt` (`422` on missing required FK,
`422` on wrong type) apply verbatim — implement via `dry-validation` contracts per
`05-backend-patterns.md §4`.

## 3. Faculty structure

| Method | Path | Notes |
|---|---|---|
| `GET`/`POST`/`PATCH`/`DELETE` | `/admin/faculties` | `{ name, short_name }` |
| `GET`/`POST`/`PATCH`/`DELETE` | `/admin/kafedras` | `{ faculty_id, name, short_name }` |
| `GET /admin/faculties/:id/kafedras` | | list kafedras for one faculty — used by every faculty→kafedra cascading selector |
| `GET`/`POST`/`PATCH`/`DELETE` | `/admin/specialities` | `{ faculty_id, name, duration_of_study }` |
| `GET /admin/faculties/:id/specialities` | | **Specialities page**: select faculty first, then list — this is the endpoint it calls |
| `GET`/`POST`/`PATCH`/`DELETE` | `/admin/teachers` | `{ post_id, academic_degree_id, academic_title_id, first_name, middle_name, last_name }` — all three FKs required |
| `GET /admin/teachers?q=` | | typeahead search by full name, for the Teacher Schedule page's teacher picker |
| `GET`/`POST`/`DELETE` | `/admin/teacher_on_kafedras` | `{ teacher_id, kafedra_id, pochasovoy? }` |
| `GET /admin/kafedras/:id/teachers` | | confirmed contract — all teachers on one kafedra |

Scope enforcement: `kafedra_admin` may only write kafedra-scoped resources
(`teacher_on_kafedras`, subjects taught by their kafedra) for their own `kafedra_id`;
`faculty_admin` may write anything under their `faculty_id` including creating new
kafedras/specialities within it.

## 4. Student groups — smart bulk entrypoint

This is the entrypoint that removes the need to manually create a `student_group` and
then N `student_group_semester` rows by hand.

### `GET /admin/faculties/:id/student_groups?semester_id=`

**Student Groups page** data source: faculty selected first, then semester, returns
groups that have a `student_group_semesters` row for that `semester_id`, each rendered
with the derived display string:

```json
{ "data": [
  { "id": 12, "display_name": "ПИ-23-1", "students_now": 25,
    "speciality": { "id": 3, "name": "Программная инженерия" },
    "student_group_semester_id": 88 }
] }
```

### `POST /admin/student_groups`

```json
{ "student_group": { "speciality_id": 3, "year_of_admission": "2023-09-01" },
  "students_now": 25 }
```

Transaction (`Groups::CreateWithSemesters` Operation, `05-backend-patterns.md §7`):

1. Insert `student_groups` row.
2. Compute `admission_semester` from `year_of_admission` (`01-database-schema.md §2.6`
   formula).
3. Load the group's `speciality.duration_of_study` (years) → `N = duration_of_study * 2`
   semesters to generate.
4. For `i` in `0..N-1`: compute the target `(start_year, end_year, number)` by ordinal
   offset; `find_or_create` the `semesters` row (this is the one place `semesters` rows
   are auto-created — avoids requiring the admin to pre-populate years manually); create a
   `student_group_semesters` row `{ semester_id, students_now, duration: null }`.
5. Return the created group plus the full list of generated `student_group_semesters`.

`201` with:
```json
{ "data": { "student_group": {...}, "generated_semesters": [ { "id":.., "semester": {...display}, "course": 1 }, ... ] } }
```

`422` if `speciality_id` invalid or `students_now < 0`.

### `POST /admin/student_groups/:id/semesters`

Confirmed baseline contract, kept for manual edge cases (repeated year, academic leave):
`{ "student_group_semester": { "semester_id", "students_now", "duration" } }` → `201`.

## 5. Curriculum & workload

### 5.0 Baseline granular endpoints (kept as-is)

The confirmed baseline contracts remain available unchanged, alongside the bulk
entrypoint below — external scripts and the E2E suite in `БД.txt` depend on them:

| Method | Path | Payload key | Notes |
|---|---|---|---|
| `POST` | `/admin/curriculum_streams` | `curriculum_stream` | `{ group_semester_id, subject_id, kafedra_id, course }` — `course` accepted per baseline; if omitted, derived server-side (`01-database-schema.md §2.6`); if provided and inconsistent with the derivation, `422` |
| `POST` | `/admin/study_load_requirements` | `study_load_requirement` | `{ stream_id, activity_type_id, hours_assigned, hours_per_event }` — FK is `stream_id`, **not** `group_semester_id` |
| `POST` | `/admin/teaching_assignments` | `teaching_assignment` | `{ load_requirement_id, teacher_on_kafedra_id }` |
| `POST` | `/admin/assignment_room_references` | `assignment_room_reference` | `{ teaching_assignment_id, room_id, priority_level }` |

Each also gets the standard `GET`(list)/`GET /:id`/`PATCH`/`DELETE` per the §2 CRUD
pattern. The bulk entrypoint (§5.1) is a transaction that composes these same writes — it
introduces no semantics the granular endpoints can't express.

### 5.1 Curriculum bulk entrypoint

### `POST /admin/curriculum/bulk`

The core "don't make the admin touch four tables by hand" endpoint. Creates a
`curriculum_streams` row and one or more `study_load_requirements` rows (and optionally
their `teaching_assignments`/`assignment_room_references`) atomically.

```json
{
  "curriculum_stream": {
    "student_group_semester_id": 88,
    "subject_id": 5,
    "kafedra_id": 2
  },
  "load_requirements": [
    {
      "activity_type_id": 1,
      "hours_assigned": 36,
      "hours_per_event": 2,
      "teaching_assignment": {
        "teacher_on_kafedra_id": 14,
        "room_ids": [ { "room_id": 10, "priority_level": 1 }, { "room_id": 11, "priority_level": 2 } ]
      }
    },
    { "activity_type_id": 3, "hours_assigned": 18, "hours_per_event": 2 }
  ]
}
```

`course` on `curriculum_streams` is derived server-side from `student_group_semester_id`
via the formula in `01-database-schema.md §2.6` — never accepted from the client.
`teaching_assignment` is optional per load requirement (workload can be entered before a
teacher is assigned — see §5.2). Runs inside one DB transaction; any validation failure
rolls back the whole request and returns `422` with per-line error details keyed by array
index. `201` returns the fully nested created structure. If a `curriculum_streams` row
already exists for `(student_group_semester_id, subject_id, kafedra_id)`, the endpoint
appends the new `load_requirements` to it instead of erroring (idempotent-ish
upsert-by-natural-key), since re-running this entrypoint to add another activity type to
an existing subject/group is a normal admin workflow.

### 5.2 Workload views

Two read models back the **Workload** page's two modes. Both are served by dedicated
Repository query objects (`05-backend-patterns.md §2.3`), not the raw CRUD resources
above — they pivot data the client should never have to reassemble from N REST calls.

### `GET /admin/workload/faculty`

Query: `faculty_id` (required), `semester_ids[]` (required, 1+),
`student_group_semester_ids[]` (optional — omitted means "all groups in scope for those
semesters"). The filter is expressed as `student_group_semester_ids` — not plain group
ids — because the brief requires selecting groups **per semester individually**: with
multiple semesters selected, a plain group id is ambiguous about which semester's rows to
include.

```json
{
  "data": {
    "activity_types": [ { "id": 1, "short_name": "Лек" }, { "id": 2, "short_name": "Прак" } ],
    "rows": [
      {
        "kafedra": { "id": 2, "short_name": "ПО" },
        "subject": { "id": 5, "name": "Базы данных" },
        "group": { "id": 12, "display_name": "ПИ-23-1" },
        "hours_by_activity_type": { "1": 36, "3": 18 }
      }
    ]
  }
}
```

Matches the described faculty-workload table: kafedra code(id), subject name, group
`<name>-<course>`, then one dynamic column per activity type showing hours. Cells with no
data are simply absent from `hours_by_activity_type` (frontend renders blank).

### `GET /admin/workload/kafedra`

Query: `kafedra_id` (required), `semester_ids[]` (required),
`student_group_semester_ids[]` (optional filter, same semantics as the faculty view).

```json
{
  "data": {
    "activity_types": [
      { "id": 1, "code": "lecture", "short_name": "Лек" },
      { "id": 2, "code": "practical", "short_name": "Прак" },
      { "id": 4, "code": "practical_kr", "short_name": "Прак.КР" },
      { "id": 3, "code": "lab", "short_name": "Лаб" },
      { "id": 5, "code": "exam", "short_name": "Экз" },
      { "id": 6, "code": "credit", "short_name": "Зач" }
    ],
    "subjects": [
      {
        "subject": { "id": 5, "name": "Базы данных" },
        "groups": [
          {
            "group": { "id": 12, "display_name": "ПИ-23-1" },
            "students_now": 25,
            "cells": {
              "1": {
                "load_requirement_id": 101,
                "hours_assigned": 36,
                "hours_per_event": 2,
                "teaching_assignment_id": 55,
                "teacher": { "id": 14, "full_name": "Петров И.И." },
                "room": { "id": 10, "name": "Ауд. 101" }
              },
              "3": null
            }
          }
        ]
      }
    ]
  }
}
```

Matches the described kafedra-workload table: subject name is the grouping key
(shown once per subject block), rows are groups (`<name>-<course>`, headcount), columns
are the fixed activity-type set with `{hours, room, teacher}` per cell. `cells["3"]: null`
means "Лабораторное" has no `study_load_requirement` yet for that subject/group — the
frontend renders it as an empty, clickable cell (see `07-frontend-pages.md §2`).

### `PATCH /admin/workload/cells/:load_requirement_id`

Assign/change teacher and/or room for an existing cell:

```json
{ "teaching_assignment": { "teacher_on_kafedra_id": 14 },
  "room_references": [ { "room_id": 10, "priority_level": 1 } ] }
```

Upserts the `teaching_assignments` row (creates if none exists for this
`load_requirement_id` yet) and replaces its `assignment_room_references`. `403` if the
caller's `kafedra_admin` scope doesn't match the cell's kafedra. `200` with the updated
cell shape (same as one entry in `cells` above).

### `POST /admin/workload/cells`

Creates a brand-new cell — i.e. a new `study_load_requirements` row for a *subject that
already has a `curriculum_streams` row* (adding e.g. "Зачёт" to a subject that so far only
had lectures). Body identical to one entry of `load_requirements` in §5.1 plus
`curriculum_stream_id`. If no `curriculum_streams` row exists yet for that subject/group,
the client should call `POST /admin/curriculum/bulk` instead (§5.1) — the "create new
record for group" button on the Workload page (`07-frontend-pages.md §2`) opens a form
that ultimately calls whichever of the two the situation needs, decided client-side by
whether the subject row already exists in the currently loaded table.

## 6. Schedule recommendations & events

### `POST /admin/schedule_recommendations`

Confirmed baseline contract:
```json
{ "schedule_recommendation": { "student_group_semester_id": 1, "day_of_week": 1, "time_slot_id": 1 } }
```
`200` with `data: []` of scored candidate placements (`day_of_week`, `time_slot_id`,
`call_number`, `week_type_id`, `week_type_code`, `room_id`, `teacher_on_kafedra_ids[]`,
`load_requirement_ids[]`, `score`), sorted `score DESC`. Scoring/candidate-generation
algorithm reads from the materialized views (`01-database-schema.md §5`) and is otherwise
unchanged baseline behavior — this document does not redefine it, only extends its
inputs (constraints, transfers, holidays must all be excluded from candidate generation,
which is a change to the view definitions, not the endpoint contract). `422` if
`day_of_week` outside `1..6`.

### `POST /admin/schedule_events/from_recommendation`

Confirmed baseline contract — body is the `placement` object verbatim under
`recommendation`. `201` on success. `409` if the slot was taken between recommendation
and commit. `422` on validation failure.

**TOCTOU guard**: `SELECT ... FOR UPDATE` is *not* sufficient here — when the slot is
still free there are no rows to lock, so two concurrent commits into the same free slot
would both pass the conflict check and both insert (the classic phantom-row problem).
Instead, the commit operation takes **transaction-scoped advisory locks** before running
the conflict check, inside the same transaction (`05-backend-patterns.md §4` shows the
implementation):

```
pg_advisory_xact_lock(hashtext('slot:' || room_id || ':' || day_key || ':' || time_slot_id))
pg_advisory_xact_lock(hashtext('teacher:' || teacher_on_kafedra_id || ':' || day_key || ':' || time_slot_id))  -- one per teacher
pg_advisory_xact_lock(hashtext('group:' || student_group_semester_id || ':' || day_key || ':' || time_slot_id))
```

where `day_key` is `day_of_week:week_type_code` for recurring events or the ISO date for
dated ones. Locks are acquired in a **sorted, deduplicated order** to prevent deadlocks
between concurrent commits touching overlapping resources. The conflict check itself must
treat `week_type` `both` as overlapping with both `even` and `odd`, and must consider all
three axes — room, each assigned teacher, and the student group — not just the room.

### `POST /admin/schedule_events`

Manual creation, bypassing the recommendation engine — needed for exam/credit dated
entries and any ad-hoc placement an admin makes directly:
```json
{
  "schedule_event": {
    "room_id": 10,
    "time_slot_id": 2,
    "event_date": "2025-01-14",
    "load_requirement_ids": [101],
    "teacher_on_kafedra_ids": [14]
  }
}
```
(`event_date` XOR `day_of_week`+`week_type_id` — same shape as recommendation commit but
without a `call_number`, derived from `time_slot`). Same conflict-check/transaction
guarantees as `from_recommendation`. `201`/`409`/`422`.

### `PATCH /admin/schedule_events/:id` · `DELETE /admin/schedule_events/:id`

Edit or remove a **recurring template**. `DELETE` sets `status: 'cancelled'` (soft
delete — history is kept for audit and so past-dated views remain accurate) rather than
hard-deleting. `PATCH` allows changing `room_id`/`time_slot_id`/teacher/room refs; changing
`day_of_week`/`week_type_id` re-runs the same conflict check as creation.

### Exam / credit-week / lecture-week scoped views

### `GET /admin/schedule/lecture_week` · `GET /admin/schedule/credit_week` · `GET /admin/schedule/exam_session`

Query: `student_group_semester_id` (required). Returns the events whose `event_date`
(dated instances) or expanded recurring occurrences fall inside the matching
`student_group_semester_periods` window (`01-database-schema.md §2.5`) **and** whose
activity type is mapped to that `period_kind` in `activity_type_period_rules`
(`01-database-schema.md §2.8`). Same CRUD affordances as `schedule_events` above, scoped
to that window — this is what lets the admin "separately view the schedule for each of
these events and add/change/delete data" per the requirement, without inventing three
parallel tables. Creating an event through a scoped view whose activity type is not
mapped to that view's `period_kind` returns `422` / `activity_type_not_allowed_in_period`.

### `GET /admin/activity_type_period_rules` · `PUT /admin/activity_type_period_rules`

The admin-configurable mapping behind the scoped views. `GET` returns all rules grouped
by activity type. `PUT` replaces the full rule set atomically:

```json
{ "rules": [
  { "activity_type_id": 5, "period_kinds": ["exam_session"] },
  { "activity_type_id": 6, "period_kinds": ["credit_week"] },
  { "activity_type_id": 1, "period_kinds": ["lecture_week"] }
] }
```

`super_admin` only. Full replacement (not per-row PATCH) keeps the invariant simple: what
you see in the settings screen is exactly what is stored.

## 7. Teacher time constraints

### `POST /admin/teacher_time_constraints` · `GET /admin/teacher_time_constraints`

Confirmed baseline contract:
```json
{ "teacher_time_constraint": { "teacher_id": 1, "time_slot_id": 1, "week_type_id": 1, "day_of_week": 1, "semester_id": 1, "is_available": false } }
```
`201`. `DELETE /admin/teacher_time_constraints/:id` removes one.

### `POST /admin/views/refresh`

Confirmed baseline contract — no body, `200 "refreshed"`. Extended
(`01-database-schema.md §5`) to also refresh `mv_room_daily_occupancy`.

**Server-side auto-refresh**: the API itself triggers a debounced refresh
(`REFRESH MATERIALIZED VIEW CONCURRENTLY`, coalesced via a Redis flag so a burst of
writes causes one refresh, not N) after any successful write to
`teacher_time_constraints`, `holidays`, `weekday_transfers`, or `room_reservations` —
**including** teacher self-service reservations via `POST /me/room_reservations`
(`04-slice-main.md §5`), whose callers cannot reach this admin-only endpoint. Correct MV
freshness must never depend on a client remembering to call refresh; the explicit
endpoint remains as the baseline-compatible manual escape hatch (and for the E2E suite in
`БД.txt`, which calls it directly).

### `GET /admin/views/teacher_constraints?teacher_id=&semester_id=`

Confirmed baseline contract — reads from the materialized view.

## 8. Room reservations

### `GET /admin/rooms/search?q=`

Full-text-ish search over `rooms.name`/`rooms.number` (and building number) for the
**Reservations page**'s search box. `ILIKE '%q%'` is sufficient at this scale; add a
trigram index (`pg_trgm`) if search volume grows.

### `POST /admin/room_reservations`

```json
{ "room_reservation": { "room_id": 10, "teacher_id": 3, "reserved_date": "2025-02-03", "time_slot_id": 2, "purpose": "Консультация" } }
```
Validates against both `room_reservations` (unique index,
`01-database-schema.md §2.2`) and the expanded `schedule_events` for that room/date/slot
(via `Schedule::DayExpansion`, `01-database-schema.md §4`). `409` if occupied either way.
`201` on success.

### `GET /admin/room_reservations?room_id=&date_from=&date_to=`

### `DELETE /admin/room_reservations/:id`

Sets `status: 'cancelled'`.

## 9. Calendar management

### `POST`/`GET`/`DELETE` `/admin/holidays`

`{ "holiday": { "holiday_date": "2025-01-01", "name": "Новый год" } }`

### `POST`/`GET`/`DELETE` `/admin/weekday_transfers`

`{ "weekday_transfer": { "transfer_date": "2025-11-02", "follows_weekday": 1, "student_group_id": null } }`
— `student_group_id: null` = university-wide; set it for a per-group override (the
Calendar Management page's "configure calendar for an individual group" mode,
`07-frontend-pages.md §6`).

### `POST /admin/schedule_events/:id/transfers`

Moves or cancels one occurrence of a recurring event:
```json
{ "event_transfer": { "original_date": "2025-03-10", "transfer_type": "moved",
  "new_date": "2025-03-14", "new_time_slot_id": 3, "new_room_id": 12, "reason": "..." } }
```
`201`. `GET /admin/schedule_events/:id/transfers` lists them. `DELETE
/admin/event_transfers/:id` reverts to the original occurrence.

### `GET`/`POST`/`PATCH` `/admin/student_group_semesters/:id/periods`

`{ "period": { "period_kind": "exam_session", "start_date": "2025-01-10", "end_date": "2025-01-24" } }`
— one row per `period_kind` (`lecture_week`, `credit_week`, `exam_session`), upserted by
kind.

## 10. Printable notices (извещения)

Implements the ТЗ requirement to auto-fill and batch-print per-teacher schedule notices
for the lecture-week and exam-session periods.

### `GET /admin/teachers/:id/notices/print?period_kind=lecture_week|exam_session&semester_id=`

Generates a single-teacher PDF (that teacher's expanded schedule for the given period,
rendered via a server-side HTML→PDF template) and returns a `200` with a JSON body
containing a **pre-signed MinIO URL** (`08-security.md §8`), not the binary directly:
```json
{ "data": { "download_url": "https://.../milk-schedule-notices/....pdf?X-Amz-...", "expires_at": "..." } }
```

### `POST /admin/kafedras/:id/notices/print_batch`

```json
{ "notices_batch": { "period_kind": "exam_session", "semester_id": 1 } }
```
Generates one PDF per teacher on that kafedra (`teacher_on_kafedras` for `kafedra_id`)
and returns either a single merged PDF or a zip archive (implementer's choice — a merged
PDF is simpler and matches "batch output to print"), same pre-signed-URL response shape
as the single-teacher endpoint. `202` if generation is queued asynchronously for large
kafedras (recommended once a kafedra has more than ~20 teachers, to stay within the
non-functional 2-second response budget — poll `GET
/admin/notices/print_batch/:job_id` for status, `200` with the download URl once
`status: "done"`).

Both endpoints render from the same underlying schedule data (`Schedule::DayExpansion`,
`01-database-schema.md §4`) already used everywhere else — no separate notices data
model.

## 11. File uploads, media pipeline, live events & audit trail

Endpoints added after the sections above were written; documented here from the current
implementation (verified against the running stack 2026-07-11, following the fixes in
`docs/security-review-2026-07-11/`).

### `POST /admin/uploads/excel`

Multipart upload of an Excel workbook or GeoJSON file, consumed downstream by
`excel_processor`. Field: `file` (multipart), optional `semester_id`.

Validation, in order: size cap (`EXCEL_MAX_FILE_SIZE`, default 20 MB) → magic-byte MIME
detection (`marcel`) → container-structure check (xlsx/xls must actually be a valid
zip/OLE container, not just named like one; geojson/json must parse) → ClamAV scan. A
failure at any step returns `422` before the file ever reaches storage:

```json
{ "errors": { "file": ["file does not have a valid .xlsx (OOXML/zip) structure"] } }
```

Success — `201`:
```json
{ "status": "ok", "file_id": 27 }
```
`file_id` is a plain integer (previously leaked as the string `"Success(Success(25))"` —
see `02-monad-double-wrap-defect.md`, fixed). The upload is persisted with
`uploaded_by_account_id` set to the caller, used by `GET /admin/files/:id/url` below.

### `GET /admin/files/:id/url`

Presigned MinIO URL for an uploaded excel/GeoJSON file. Restricted to the uploader or an
admin-tier role (`super_admin`, `faculty_admin`, `kafedra_admin`, `scheduler`) — matches
the equivalent media-file endpoint below; previously unrestricted to any authenticated
admin (`07-authorization-scope-gaps.md`, fixed).

`403 { "error": "forbidden" }` if neither owner nor admin-tier. `404` if not found. `200`:
```json
{ "url": "https://minio.../uploads/excel/...", "expires_in": 900, "filename": "edges.geojson" }
```

### `POST /admin/uploads/media`

Multipart upload of an image/PDF/document, quarantined then asynchronously sanitized
(magic-byte MIME check, ClamAV, entropy scan, format-specific re-encode/macro-strip).
Field: `file` (multipart). `201`:
```json
{ "status": "quarantined", "media_file_id": 16,
  "links": { "self": "/admin/media_files/16", "events": "/admin/stream" } }
```
`422 { "errors": { "file": [...] } }` on validation failure (unsupported type / too large).

### `GET /admin/media_files/:id`

Poll for sanitization status. `status` transitions `quarantined` → `scanning` →
`clean` | `rejected`. Only `clean` is `servable: true` and carries a `url` link:
```json
{ "data": { "id": "19", "type": "media_file", "attributes": {
  "title": "photo.png", "status": "clean", "detected_mime": "image/png",
  "rejection_reason": null, "servable": true, "formatted_size": "68 B" },
  "links": { "self": "...", "url": "http://.../admin/media_files/19/url", "events": "..." } } }
```

### `GET /admin/media_files/:id/url`

Same ownership rule as the excel equivalent above (uploader or admin-tier role). `422` if
the file hasn't reached `clean` yet.

### `GET /admin/stream`

SSE event stream (`text/event-stream`, chunked, no `Content-Length`) for real-time admin
notifications — resource writes and media-file status changes. Auth via
`?access_token=<jwt>` query param (EventSource cannot set headers); admin-tier roles only.
Emits `data: <json>\n\n` per event and a `: heartbeat\n\n` comment approximately every 25s
of silence. (Previously the response body was the literal string
`"#<Enumerator:0x...>"` instead of an actual stream —
`07-authorization-scope-gaps.md`, fixed.)

### `GET /admin/audit_logs` · `GET /admin/audit_logs/:id`

```json
{ "data": { "id": "39", "type": "audit_log", "attributes": {
  "user_id": "483b7037-...", "action": "faculties.delete", "entity_type": "faculties",
  "entity_id": 93,
  "before_data": "{\"id\":93,\"name\":\"Test Faculty\",\"short_name\":\"TF\"}",
  "after_data": null, "ip_address": "172.18.0.1", "created_at": "..." } } }
```
`entity_id` is now populated on `create` actions (previously always `null` — the newly
created row's id wasn't threaded through). `before_data`/`after_data` hold a JSON
snapshot of the record as it existed immediately before/after the write (creates populate
`after_data`; deletes populate `before_data`; updates populate `after_data` only — a full
pre-image on update would require an extra fetch in every write action and was judged out
of scope, see `06-audit-log-gaps.md`). `ip_address` is the resolved client IP, walking
`X-Forwarded-For` from trusted proxies. Filter query params: `entity_type`, `entity_id`,
`user_id`, `date_from`, `date_to`.
