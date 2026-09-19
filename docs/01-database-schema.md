# Database Schema

PostgreSQL 16. All tables use `bigserial` primary keys named `id` unless noted. All
`created_at`/`updated_at` are `timestamptz`. This document is the single source of truth
for column names — every endpoint spec in `03-slice-admin.md` / `04-slice-main.md`
references these exact names.

Required extensions (first migration): `CREATE EXTENSION IF NOT EXISTS citext;` (used by
`users.email`) and `CREATE EXTENSION IF NOT EXISTS pg_trgm;` (room text search,
`03-slice-admin.md §8`).

## 1. Baseline tables (from the existing API contract — unchanged)

These are reconstructed from `БД.txt` and MUST be implemented exactly as listed; every
other document assumes these names and types.

| Table | Columns | Notes |
|---|---|---|
| `activity_types` | `id`, `name`, `short_name` | Лекция, Практическое, Лабораторное, Практическое на КР, Экзамен, Зачёт, … (seed data — extended in §2.7) |
| `building_types` | `id`, `name`, `short_name` | |
| `week_types` | `id`, `name`, `code` | `code` **must** contain exactly the rows `both`, `even`, `odd` (unique, not null). The recommendation engine and conflict checks key off `code`, not `id`. |
| `time_slots` | `id`, `start_time` (`time`), `end_time` (`time`), `call_number` (int) | A "pair" (пара) |
| `posts` | `id`, `name`, `short_name` | Teacher post/position |
| `academic_degrees` | `id`, `name`, `short_name` | |
| `academic_titles` | `id`, `name`, `short_name` | |
| `subjects` | `id`, `name`, `short_name` | |
| `semesters` | `id`, `start_year` (int), `end_year` (int), `number` (smallint) | `UNIQUE(start_year, end_year, number)`. Display format defined in `00-overview.md §7`. |
| `room_types` | `id`, `name`, `activity_type_id` FK → `activity_types` | |
| `buildings` | `id`, `building_type_id` FK, `number` (text) | |
| `rooms` | `id`, `building_id` FK, `room_type_id` FK, `name`, `number` (text) | **No `capacity` column** — confirmed absent in baseline; do not add speculatively |
| `faculties` | `id`, `name`, `short_name` | |
| `kafedras` | `id`, `faculty_id` FK, `name`, `short_name` | |
| `specialities` | `id`, `faculty_id` FK, `name`, `duration_of_study` (smallint, years) | |
| `teachers` | `id`, `post_id` FK, `academic_degree_id` FK, `academic_title_id` FK, `first_name`, `middle_name`, `last_name` | All three FKs required |
| `teacher_on_kafedras` | `id`, `teacher_id` FK, `kafedra_id` FK, `pochasovoy` (bool, default `false`) | A teacher's affiliation to a kafedra; `pochasovoy` = hourly/adjunct. `UNIQUE(teacher_id, kafedra_id)`. Teaching assignments reference **this** row, not `teachers` directly, so the same person can be assigned separately per kafedra. Referenced memberships use `ON DELETE RESTRICT`. |
| `student_groups` | `id`, `speciality_id` FK, `year_of_admission` (date) | `students_now` is **not** stored here (see `student_group_semesters`) |
| `student_group_semesters` | `id`, `student_group_id` FK, `semester_id` FK, `students_now` (int), `duration` (int, weeks) | `UNIQUE(student_group_id, semester_id)` |
| `curriculum_streams` | `id`, `student_group_semester_id` FK (payload key `group_semester_id`), `subject_id` FK, `kafedra_id` FK, `course` (smallint) | One row = "this subject is taught to this group-semester by this kafedra" |
| `study_load_requirements` | `id`, `curriculum_stream_id` FK (payload key `stream_id`), `activity_type_id` FK, `hours_assigned` (int), `hours_per_event` (int) | The unit of "how many hours of lecture/lab/etc. this stream needs" |
| `teaching_assignments` | `id`, `study_load_requirement_id` FK (payload key `load_requirement_id`), `teacher_on_kafedra_id` FK | Assigns a teacher to a load-requirement line |
| `assignment_room_references` | `id`, `teaching_assignment_id` FK, `room_id` FK, `priority_level` (smallint) | `1` = engine's first choice, `2+` = fallback, in ascending priority order |
| `teacher_time_constraints` | `id`, `teacher_id` FK, `time_slot_id` FK, `week_type_id` FK, `day_of_week` (smallint 1–6), `semester_id` FK, `is_available` (bool) | Authoritative source for the administrator grid and recommendation engine; one row per teacher/slot/week/day/semester |
| `schedule_events` | see §2.7 (extended) | |

Materialized views (baseline, must exist, refreshed via `POST /admin/views/refresh`):
`mv_teacher_constraints`, `mv_room_occupancy_candidates` (or equivalent — the exact view
backing `schedule_recommendations` scoring; implement as needed by the recommendation
algorithm in `03-slice-admin.md §6`). Any new materialized view this spec adds is listed
in §5 and must be refreshed by the same endpoint.

## 2. New tables

### 2.1 Authentication & authorization

```sql
CREATE TABLE users (
  id                  bigserial PRIMARY KEY,
  email               citext UNIQUE NOT NULL,
  password_digest     text NOT NULL,             -- bcrypt/argon2, never plaintext
  full_name           text NOT NULL,
  teacher_id          bigint NULL REFERENCES teachers(id), -- optional link if this user IS a teacher
  status              text NOT NULL DEFAULT 'active' CHECK (status IN ('active','disabled')),
  must_change_password boolean NOT NULL DEFAULT true,
  created_by_id       bigint NULL REFERENCES users(id),    -- admin who provisioned this account; NULL only for the bootstrap superadmin
  last_login_at       timestamptz NULL,
  created_at          timestamptz NOT NULL DEFAULT now(),
  updated_at          timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE roles (
  id    bigserial PRIMARY KEY,
  code  text UNIQUE NOT NULL,   -- 'super_admin' | 'faculty_admin' | 'kafedra_admin' | 'dispatcher' | 'teacher'
  name  text NOT NULL
);
-- Seeded once via migration; see 02-slice-authentication.md §2 for the full capability matrix.

CREATE TABLE role_assignments (
  id           bigserial PRIMARY KEY,
  user_id      bigint NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  role_id      bigint NOT NULL REFERENCES roles(id),
  scope_type   text NOT NULL CHECK (scope_type IN ('global','faculty','kafedra')),
  faculty_id   bigint NULL REFERENCES faculties(id),
  kafedra_id   bigint NULL REFERENCES kafedras(id),
  created_at   timestamptz NOT NULL DEFAULT now(),
  CHECK (
    (scope_type = 'global'  AND faculty_id IS NULL AND kafedra_id IS NULL) OR
    (scope_type = 'faculty' AND faculty_id IS NOT NULL AND kafedra_id IS NULL) OR
    (scope_type = 'kafedra' AND kafedra_id IS NOT NULL)
  )
);
CREATE UNIQUE INDEX ux_role_assignments ON role_assignments
  (user_id, role_id, scope_type, COALESCE(faculty_id,0), COALESCE(kafedra_id,0));
```

A single user can hold multiple `role_assignments` — e.g. a `teacher` row (their own
schedule/self-service) plus a `kafedra_admin` row scoped to one `kafedra_id` (matches the
"teacher who also administers a department" requirement in `00-overview.md`).

```sql
CREATE TABLE refresh_tokens (
  id            bigserial PRIMARY KEY,
  user_id       bigint NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  token_digest  text NOT NULL,          -- SHA-256 of the raw token; raw token never persisted
  user_agent    text NULL,
  ip_address    inet NULL,
  issued_at     timestamptz NOT NULL DEFAULT now(),
  expires_at    timestamptz NOT NULL,
  revoked_at    timestamptz NULL
);
CREATE INDEX ix_refresh_tokens_user ON refresh_tokens(user_id) WHERE revoked_at IS NULL;
```

```sql
CREATE TABLE audit_logs (
  id           bigserial PRIMARY KEY,
  user_id      bigint NULL REFERENCES users(id),
  action       text NOT NULL,          -- e.g. 'schedule_event.create', 'user.disable'
  entity_type  text NOT NULL,
  entity_id    bigint NULL,
  before_data  jsonb NULL,
  after_data   jsonb NULL,
  ip_address   inet NULL,
  created_at   timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX ix_audit_logs_entity ON audit_logs(entity_type, entity_id);
```

> **Design note — no student accounts.** The baseline schema has no student entity, only
> a headcount (`students_now`). Schedule lookup by group/teacher/room (`04-slice-main.md`)
> is therefore **public, unauthenticated read access** — students don't log in, they pick
> their group from a dropdown. Only staff (teachers/admins) have `users` rows. This keeps
> the "no self-registration" rule meaningful (there's no student self-service surface to
> register into) and avoids inventing an unrequired subsystem.

### 2.2 Room reservations

A teacher reserving a room on a specific date/slot outside the regular curriculum
schedule (e.g. for a consultation, defense, or one-off event).

```sql
CREATE TABLE room_reservations (
  id            bigserial PRIMARY KEY,
  room_id       bigint NOT NULL REFERENCES rooms(id),
  teacher_id    bigint NOT NULL REFERENCES teachers(id),
  reserved_date date NOT NULL,
  time_slot_id  bigint NOT NULL REFERENCES time_slots(id),
  purpose       text NULL,
  status        text NOT NULL DEFAULT 'active' CHECK (status IN ('active','cancelled')),
  created_by_id bigint NOT NULL REFERENCES users(id),
  created_at    timestamptz NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX ux_room_reservations_slot
  ON room_reservations(room_id, reserved_date, time_slot_id)
  WHERE status = 'active';
```

The uniqueness index prevents two *reservations* from double-booking a room, but a
reservation must also be checked at write time against `schedule_events` occupying that
room on that date (recurring-pattern expansion — see §4). That check is application-level
(`03-slice-admin.md §8`), because it depends on expanding recurring events for the date in
question, which a plain SQL constraint cannot express.

### 2.3 Calendar: holidays & whole-day transfers

```sql
CREATE TABLE holidays (
  id            bigserial PRIMARY KEY,
  holiday_date  date NOT NULL,
  name          text NOT NULL,
  created_by_id bigint NOT NULL REFERENCES users(id),
  created_at    timestamptz NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX ux_holidays_date ON holidays(holiday_date);
```

A "red day" — no classes run on this date for anyone, regardless of the recurring
pattern.

```sql
CREATE TABLE weekday_transfers (
  id                bigserial PRIMARY KEY,
  transfer_date     date NOT NULL,            -- the actual calendar date affected
  follows_weekday   smallint NOT NULL CHECK (follows_weekday BETWEEN 1 AND 6),
  student_group_id  bigint NULL REFERENCES student_groups(id), -- NULL = applies university-wide
  reason            text NULL,
  created_by_id     bigint NOT NULL REFERENCES users(id),
  created_at        timestamptz NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX ux_weekday_transfers_global
  ON weekday_transfers(transfer_date) WHERE student_group_id IS NULL;
CREATE UNIQUE INDEX ux_weekday_transfers_group
  ON weekday_transfers(transfer_date, student_group_id) WHERE student_group_id IS NOT NULL;
```

Meaning: "on `transfer_date`, run the schedule that would normally run on
`follows_weekday`" (e.g. a Saturday follows the Monday pattern). Scoped globally by
default; a `student_group_id` row overrides the global one for that group only, which is
what the Calendar Management page's per-group override needs
(`07-frontend-pages.md §6`).

### 2.4 Calendar: individual event transfers

Moving or cancelling one occurrence of a recurring `schedule_events` row, without
affecting the recurring template itself.

```sql
CREATE TABLE event_transfers (
  id                      bigserial PRIMARY KEY,
  schedule_event_id       bigint NOT NULL REFERENCES schedule_events(id),
  original_date           date NOT NULL,        -- which calendar occurrence this overrides
  transfer_type           text NOT NULL CHECK (transfer_type IN ('moved','cancelled')),
  new_date                date NULL,
  new_time_slot_id        bigint NULL REFERENCES time_slots(id),
  new_room_id             bigint NULL REFERENCES rooms(id),
  reason                  text NULL,
  created_by_id           bigint NOT NULL REFERENCES users(id),
  created_at              timestamptz NOT NULL DEFAULT now(),
  CHECK (
    (transfer_type = 'cancelled' AND new_date IS NULL AND new_time_slot_id IS NULL) OR
    (transfer_type = 'moved' AND new_date IS NOT NULL)
  )
);
CREATE UNIQUE INDEX ux_event_transfers_occurrence
  ON event_transfers(schedule_event_id, original_date);
```

Read path: when rendering a group/teacher/room schedule for a concrete date, the API
first expands the recurring `schedule_events` template for that date (§4), then applies
any `event_transfers` row matching `(schedule_event_id, original_date)` — `cancelled`
removes it, `moved` relocates it (room/date/time only; the group/subject/teacher stay the
same, since a transfer doesn't change *what* is being taught).

### 2.5 Semester periods (lecture week / credit week / exam session)

```sql
CREATE TABLE student_group_semester_periods (
  id                          bigserial PRIMARY KEY,
  student_group_semester_id  bigint NOT NULL REFERENCES student_group_semesters(id) ON DELETE CASCADE,
  period_kind                 text NOT NULL CHECK (period_kind IN ('lecture_week','credit_week','exam_session')),
  start_date                  date NOT NULL,
  end_date                    date NOT NULL,
  created_at                  timestamptz NOT NULL DEFAULT now(),
  CHECK (end_date >= start_date)
);
CREATE UNIQUE INDEX ux_group_semester_periods ON student_group_semester_periods
  (student_group_semester_id, period_kind);
```

Despite the name, `lecture_week`'s date range is the **entire regular teaching period**
for that group-semester (e.g. Sept 1 – Dec 20) — the historical university term is kept
because it is what staff already call it on paper notices. `credit_week` (зачётная
неделя) and `exam_session` (экзаменационная сессия) are shorter trailing windows. All
three are set per `student_group_semester` individually (different groups can start/end
on different dates).

`schedule_events` with `event_date` inside a `lecture_week` window render as the regular
recurring weekly grid; events dated inside `credit_week`/`exam_session` windows are
one-off dated entries (§2.7) shown on the corresponding "Credit Week" / "Exam Session"
schedule view (`04-slice-main.md §4`, `07-frontend-pages.md §3`).

### 2.6 Course-number and display-format derivations (no new columns)

`course` for a `student_group` at a given `semester` is **always derived**, never stored
redundantly outside `curriculum_streams.course` (which is set once when the stream is
created — see §2.5 formula below for how the admin bulk-create endpoint computes it).

Formula, implemented as a pure function (Ruby: `Semesters::Ordinal`, see
`05-backend-patterns.md §7`):

```
ordinal(semester)      = (semester.start_year - EPOCH_YEAR) * 2 + semester.number
admission_semester(group) =
    the `semesters` row with number = 1 if month(year_of_admission) >= 9
                        else number = 2,
    start_year = year(year_of_admission) if number = 1
               else year(year_of_admission) - 1

course(group, semester) = floor((ordinal(semester) - ordinal(admission_semester(group))) / 2) + 1
```

Worked example (matches the brief): `year_of_admission = 2023-09-01` → admission academic
year `2023/2024`, admission semester = `(number: 1, start_year: 2023, end_year: 2024)`.
For target semester `(number: 1, start_year: 2023, end_year: 2024)`: `course = 1`. For
`(number: 2, start_year: 2024, end_year: 2025)` (i.e. the 4th enrolled semester): `course
= 2`.

`EPOCH_YEAR` is an arbitrary fixed constant (e.g. `2000`) purely to make `ordinal`
monotonic and comparable; it has no business meaning.

Group display string `"<group_short_name>-<course>"` and semester display string
`"<number> семестр <start_year>/<end_year>"` are Presenter concerns
(`05-backend-patterns.md §5`) computed at serialization time — never stored.

### 2.7 `schedule_events` (extended)

The baseline `schedule_events` table (payload shape confirmed by
`POST /admin/schedule_events/from_recommendation` in `БД.txt`) is extended with one
nullable column and two join tables to support dated (non-recurring) occurrences and to
make the teacher/load-requirement relationship explicit and multi-valued (the baseline
payload already sends `teacher_on_kafedra_ids` and `load_requirement_ids` as arrays).

```sql
CREATE TABLE schedule_events (
  id                bigserial PRIMARY KEY,
  room_id           bigint NULL REFERENCES rooms(id),        -- nullable: e.g. PE with no fixed room
  time_slot_id      bigint NOT NULL REFERENCES time_slots(id),
  call_number       smallint NOT NULL,                        -- denormalized from time_slot at write time
  day_of_week       smallint NULL CHECK (day_of_week BETWEEN 1 AND 6),
  week_type_id      bigint NULL REFERENCES week_types(id),
  event_date        date NULL,                                 -- set for one-off dated occurrences
  status            text NOT NULL DEFAULT 'active' CHECK (status IN ('active','cancelled')),
  created_by_id     bigint NOT NULL REFERENCES users(id),
  created_at        timestamptz NOT NULL DEFAULT now(),
  updated_at        timestamptz NOT NULL DEFAULT now(),
  CHECK (
    (event_date IS NULL AND day_of_week IS NOT NULL AND week_type_id IS NOT NULL) OR
    (event_date IS NOT NULL AND day_of_week IS NULL AND week_type_id IS NULL)
  )
);

CREATE TABLE schedule_event_load_requirements (
  schedule_event_id        bigint NOT NULL REFERENCES schedule_events(id) ON DELETE CASCADE,
  study_load_requirement_id bigint NOT NULL REFERENCES study_load_requirements(id),
  PRIMARY KEY (schedule_event_id, study_load_requirement_id)
);

CREATE TABLE schedule_event_teachers (
  schedule_event_id       bigint NOT NULL REFERENCES schedule_events(id) ON DELETE CASCADE,
  teacher_on_kafedra_id   bigint NOT NULL REFERENCES teacher_on_kafedras(id),
  PRIMARY KEY (schedule_event_id, teacher_on_kafedra_id)
);
```

Two mutually exclusive shapes, enforced by the `CHECK`:

- **Recurring template** (`event_date IS NULL`): the normal weekly lecture-period grid —
  "every `day_of_week`, on `week_type` weeks, this happens." Rendered for a concrete date
  by the expansion function in §4.
- **Dated instance** (`event_date IS NOT NULL`): a single occurrence — used for
  `credit_week`/`exam_session` entries (Экзамен/Зачёт activity types), and also created
  automatically as the "moved-to" side effect is **not** a new `schedule_events` row —
  moves are represented purely by `event_transfers` (§2.4) pointing at the recurring
  template, so the template stays the single source of truth. `schedule_events` rows with
  `event_date` set are only used for genuinely non-recurring activity types (exams,
  credits).

`schedule_event_teachers` may have **zero rows** (activity has no assigned teacher, e.g.
physical education) — this is intentional, not an error state. Zero `room_id` is likewise
valid.

The `activity_types` seed list is extended (data, not schema) to include, at minimum:
`Лекция`, `Практическое`, `Лабораторное`, `Практическое на КР`, `Экзамен`, `Зачёт` — all
flowing through the same `study_load_requirements` → `teaching_assignments` →
`schedule_events` pipeline the baseline already defines, per the requirement to avoid
parallel one-off tables for exams/credits.

### 2.8 Activity type → schedule-view rules (admin-configurable)

The brief requires that the admin can decide *which activity types are shown/accepted in
which schedule table* (lecture week / credit week / exam session) rather than having that
mapping hardcoded:

```sql
CREATE TABLE activity_type_period_rules (
  id                bigserial PRIMARY KEY,
  activity_type_id  bigint NOT NULL REFERENCES activity_types(id) ON DELETE CASCADE,
  period_kind       text NOT NULL CHECK (period_kind IN ('lecture_week','credit_week','exam_session')),
  created_at        timestamptz NOT NULL DEFAULT now(),
  UNIQUE (activity_type_id, period_kind)
);
```

An activity type may map to multiple period kinds (e.g. a consultation could appear in
both credit week and exam session). Seed defaults: Экзамен → `exam_session`; Зачёт →
`credit_week`; every other seeded type → `lecture_week`. The scoped schedule views
(`03-slice-admin.md §6`) filter by these rules **in addition to** the period date windows
(§2.5), and creation into a scoped view validates that the event's activity type is
allowed there (`422` otherwise). An activity type with no rule rows is not accepted into
any scoped view until the admin assigns it — surfaced as a warning in the admin UI
(`07-frontend-pages.md §10`).

## 3. Entity relationship summary (new tables only)

```
users ──< role_assignments >── roles
  │
  ├──< refresh_tokens
  ├──< audit_logs
  └── (optional) teacher_id → teachers

teachers ──< room_reservations
rooms    ──< room_reservations

student_groups ──< weekday_transfers (nullable scope)
student_group_semesters ──< student_group_semester_periods

schedule_events ──< event_transfers
schedule_events ──< schedule_event_load_requirements >── study_load_requirements
schedule_events ──< schedule_event_teachers >── teacher_on_kafedras
```

## 4. Recurring-event expansion (read-path algorithm)

Given a date `D` and a scope (group / teacher / room), rendering "what happens on `D`":

1. If `D` is in `holidays` → return empty (unless a `schedule_events` dated instance
   explicitly targets `D`, which does not happen for holidays by convention).
2. Determine the *effective weekday* for `D`: look up `weekday_transfers` for
   `(D, group_id)` first, then `(D, NULL)` (global); if found, `effective_weekday =
   follows_weekday`, else `effective_weekday = extract(isodow from D)`.
3. Determine the *effective week type parity* for `D` relative to the enclosing
   `student_group_semester_periods` (`lecture_week` row). Concretely:
   `week_index = floor((monday_of(D) - monday_of(start_date)) / 7)`; if `week_index` is
   even the parity code is `even` (числитель — the first teaching week is числитель, per
   the seed naming in the baseline where `Числитель` carries code `even`), otherwise
   `odd` (знаменатель). Implemented in one pure function (`Semesters::WeekParity`,
   colocated with `Semesters::Ordinal` in `05-backend-patterns.md §7`) used by both the
   expansion query and the recommendation engine, so the two can never disagree.
4. Select all `schedule_events` where `event_date IS NULL AND day_of_week =
   effective_weekday AND week_type.code IN ('both', <effective_parity>)`, joined through
   `schedule_event_load_requirements → study_load_requirements → curriculum_streams` to
   filter by the requested group/teacher/room, **plus** all dated `schedule_events` where
   `event_date = D` for that scope.
5. For each selected recurring event, check `event_transfers` for
   `(schedule_event_id, original_date = D)`: drop if `cancelled`, substitute
   room/date/time if `moved`.
6. Add any `event_transfers` rows whose `new_date = D` moved **from** another date, so
   they appear on `D`'s grid at their new slot.

This algorithm is implemented once as a Repository-level query object
(`Schedule::DayExpansion`, `05-backend-patterns.md §2.3`) and reused by every read
endpoint in `04-slice-main.md` and `03-slice-admin.md` — it must not be duplicated
per-endpoint.

## 5. Materialized views

In addition to the baseline recommendation-engine views, add:

```sql
CREATE MATERIALIZED VIEW mv_room_daily_occupancy AS
SELECT room_id, occupied_date, time_slot_id, source, source_id
FROM ( /* union of expanded schedule_events + active room_reservations,
          bounded to [today, today + 90 days] for a rolling window */ ) t;
```

Backs the Room Schedule / Reservations search pages (`07-frontend-pages.md §4`, `§5`),
refreshed by the same `POST /admin/views/refresh` endpoint (extend it to refresh this view
too — see `03-slice-admin.md §9`). A 90-day rolling window keeps refresh cost bounded;
reservations further out are still checked against the live tables at write time.

## 6. Indexing strategy

- Every FK column gets a plain btree index unless already covered by a unique index
  above.
- `schedule_events(day_of_week, week_type_id, time_slot_id)` — partial index `WHERE
  status = 'active' AND event_date IS NULL` — the hot path for the recurring grid.
- `schedule_events(event_date, time_slot_id)` — partial index `WHERE status = 'active' AND
  event_date IS NOT NULL` — the hot path for exam/credit views.
- `room_reservations(reserved_date)`, `holidays(holiday_date)`,
  `weekday_transfers(transfer_date)` — range-scan support for calendar rendering.
- `audit_logs(created_at)` — for retention/cleanup jobs and admin review filtering.
