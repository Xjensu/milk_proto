# Frontend Pages — UX Specification

Every page below states: entry selection flow, data source (endpoint from
`03-slice-admin.md`/`04-slice-main.md`), layout, and interactions. Built from the
reusable components in `06-frontend-architecture.md §7`.

## 1. App shell

Two shells sharing the same auth session:

- **Public shell** (`/schedule/*`): top nav with three links — Student Group Schedule,
  Teacher Schedule, Room Schedule — plus a "Staff login" link. No sidebar.
- **Admin shell** (`/admin/*`, `/me/*`): left sidebar grouped as: *Static Data* (activity
  types, buildings, rooms, posts, degrees, titles, subjects, time slots, week types),
  *Structure* (faculties, kafedras, specialities, teachers), *Student Groups*,
  *Curriculum & Workload*, *Schedule* (group/teacher/room views + recommendation flow),
  *Calendar*, *Reservations*, *Users* (super_admin only). Sidebar items are filtered by
  `RoleGuard` — a `kafedra_admin` never sees *Users* or faculties-outside-their-scope
  actions.

## 2. Curriculum & Workload page

The most interactive admin screen. Top-level toggle: **Faculty workload** /
**Kafedra workload**.

### 2.1 Faculty workload mode

1. Select **Faculty** (`GET /public/faculties` or admin equivalent).
2. Select **Semester(s)** — multi-select, from `GET /admin/faculties/:id/semesters`-style
   listing (semesters with data for that faculty).
3. Optional: select specific **Student groups** for those semesters, or leave as "all
   groups" (checkbox "All groups in scope" defaults checked).
4. `PivotTable` renders `GET /admin/workload/faculty` result: columns = **Kafedra
   (id)**, **Subject**, **Group** (`<name>-<course>`), then one column per activity type
   showing hours. Read-focused view — no cell editing here (editing happens in kafedra
   mode, where the acting `kafedra_admin`'s scope actually matches). `faculty_admin`/
   `super_admin` may still click through a row to jump into that kafedra's workload view
   pre-filtered to that subject/group.

### 2.2 Kafedra workload mode

1. Select **Faculty**, then **Kafedra** (cascading), then **Semester(s)**, then optional
   **Student groups** (same "all groups" default as §2.1).
2. `PivotTable` renders `GET /admin/workload/kafedra`: grouped by **Subject** (subject
   name shown once per block), rows = **Group** (`<name>-<course>`, student count),
   columns = the activity-type set returned in the response's `activity_types` array
   (typically Лекции, Практические, Практическое на КР, Лабораторные, Экзамен, Зачёт —
   but **driven by the data, not hardcoded**, so admin-added activity types appear as
   columns automatically), each cell showing **hours / room / teacher ФИО** stacked in
   three lines.
3. **Empty cell** (no `study_load_requirement` yet for that subject/group/activity-type):
   rendered as a dashed placeholder. Clicking it (if the user's role/scope permits
   writing) opens the **Cell Editor** dialog:
   - If the subject/group has no `curriculum_streams` row at all yet, the dialog is the
     **full bulk-create wizard**: pick activity type(s) to add in one go, hours
     assigned/per event for each, optionally teacher + room(s) with priority — submits to
     `POST /admin/curriculum/bulk`.
   - If the subject/group already exists (other cells are filled) but this one
     activity-type is missing, the dialog is a single-line form — submits to
     `POST /admin/workload/cells`.
4. **Filled cell**: clicking opens the same Cell Editor pre-populated, submitting to
   `PATCH /admin/workload/cells/:load_requirement_id`. Read-only users (e.g. a
   `faculty_admin` viewing a kafedra outside anything — not applicable since faculty
   admins can view/edit all kafedras in their faculty; a `teacher` role simply never sees
   this page) see the cell without the click affordance.
5. **"Create new record for group"** button above the table: opens the same bulk-create
   wizard as the empty-cell case, but starting from a blank slate — pick **Subject**
   first (async combobox), then **Group** (from the groups already in scope for the
   selected semester(s)), then proceed as above. This is the explicit "admin shouldn't
   have to depend on DB structure" entrypoint — one form, one submit, four tables written
   atomically server-side.

## 3. Schedule — Student Group page

1. Select **Faculty** → **Semester** → **Student group** (cascading, per
   `03-slice-admin.md §4` / `04-slice-main.md §1`).
2. A `DatePeriodBadge` + tab control lets the admin switch between **Lecture period**
   (the regular recurring weekly grid), **Credit week**, and **Exam session** — each tab
   hits the matching `GET /admin/schedule/{lecture_week|credit_week|exam_session}`
   endpoint (`03-slice-admin.md §6`) scoped to the selected `student_group_semester_id`.
3. `WeekGrid` renders the period. Admin (with write scope) can:
   - Click an empty slot → runs `POST /admin/schedule_recommendations` for that
     `(day_of_week, time_slot_id)`, shows ranked candidates in a side panel (score,
     teacher, room), admin picks one → `POST /admin/schedule_events/from_recommendation`.
   - Click an existing event → edit (`PATCH /admin/schedule_events/:id`), delete
     (`DELETE`), or **transfer** this one occurrence
     (`POST /admin/schedule_events/:id/transfers`) without touching the recurring
     template — opens a small form: new date, new time slot, new room, reason.
   - Transferred/cancelled occurrences render highlighted (distinct background) per
     `is_transferred` in the API response.
4. A **"Print notice"** button (visible in Teacher Schedule mode, §4, and as **"Print all
   notices for kafedra"** on the Kafedra Workload page, §2.2) calls
   `GET /admin/teachers/:id/notices/print` or
   `POST /admin/kafedras/:id/notices/print_batch` (`03-slice-admin.md §10`) and opens the
   returned pre-signed URL in a new tab — implements the ТЗ's lecture-week/exam-session
   notice printing requirement without a dedicated page.

Public visitors see the same `WeekGrid` read-only at `/schedule/group`, sourced from
`GET /public/schedule/student_group/:id`.

## 4. Schedule — Teacher page

1. Two modes, switched by a toggle: **By semester** (select **Semester**, then search/pick
   **Teacher** via `AsyncSelect` against `GET /public/teachers?q=`) or **By date** (pick a
   single **Date**, then the same teacher search).
2. `WeekGrid` (semester mode) or single-day list (date mode) from
   `GET /public/schedule/teacher/:teacher_id`. Each entry shows the **student group**
   alongside subject/room, since one teacher's day spans multiple groups. Entries with no
   room render "Room: —" rather than an empty cell (explicit per the ТЗ note that room may
   be absent).
3. Admin variant (`/admin/schedule/teacher`) adds the same edit/transfer affordances as
   §3, scoped to events involving that teacher.

## 5. Schedule — Room page (occupancy)

1. Select one or more **Rooms** (multi-select, `AsyncSelect` against
   `GET /public/rooms`, optionally pre-filtered by **Building**) and a **Date** (defaults
   to today).
2. Renders one `WeekGrid`-style single-day column per selected room from
   `GET /public/schedule/rooms?room_ids[]=...&date=...`, each slot showing **student
   group**, subject, and teacher if occupied, or "Free" if not — merges
   `schedule_events` and `room_reservations` into one timeline per room, matching the
   requirement to see occupancy across a room list for a chosen day.
3. Admin variant lets an admin/dispatcher click a free slot to jump straight into the
   Reservations flow (§7) pre-filled with that room/date/slot.

## 6. Calendar Management page

1. Toggle: **University-wide calendar** / **Per-group calendar**. Per-group mode adds a
   Faculty→Semester→Group cascading selector.
2. Three tabs:
   - **Holidays** (`/admin/holidays`) — simple date+name list, add/remove.
   - **Weekday transfers** (`/admin/weekday_transfers`) — "on `[date]`, follow the
     `[weekday]` schedule" rows; when in per-group mode, `student_group_id` is set on
     create, otherwise left `null` (global).
   - **Individual event transfers** — a searchable list of a group's/teacher's upcoming
     recurring events (reuses the `WeekGrid`) with a "Move" action per event, opening the
     same transfer form as §3.

   Room reservations also surface on this page's date views (read-only badges on the
   affected dates, sourced from `GET /admin/room_reservations?date_from=&date_to=`) —
   the brief explicitly asks for teacher room reservations to be visible in the
   calendar, not only on the Room Schedule page.
3. A **Semester periods** panel (visible when a specific group-semester is in scope) lets
   the admin set/edit the three date ranges (`lecture_week`, `credit_week`,
   `exam_session`) via `GET`/`POST`/`PATCH /admin/student_group_semesters/:id/periods` —
   three simple start/end date inputs.

## 7. Reservations page

1. A prominent **text search box** searching rooms by name/number — the explicit "text
   search for a room" requirement — debounced, results shown as a list with
   building/type. Admin/dispatcher roles call `GET /admin/rooms/search?q=`; the `teacher`
   role calls `GET /public/rooms?q=` (`04-slice-main.md §1`), since teachers do not have
   admin-slice access.
2. Selecting a room opens its **day/week occupancy** (reusing the Room Schedule grid,
   §5) so the dispatcher/teacher can see free slots at a glance.
3. Clicking a free slot opens the reservation form: date, time slot (pre-filled from the
   clicked slot), purpose, and — for `teacher` role — `teacher_id` is implicit (self); for
   `dispatcher`/admin roles, a teacher picker is shown. Submits to
   `POST /admin/room_reservations` (staff-on-behalf) or `POST /me/room_reservations`
   (teacher self-service), per `04-slice-main.md §5`.
4. A "My reservations" list (teacher role) or "All reservations" list (dispatcher/admin)
   below, with cancel action.

## 8. Specialities page

1. Select **Faculty** first (`GET /public/faculties`).
2. List of **Specialities** for that faculty (`GET /admin/faculties/:id/specialities`),
   each showing `name` and `duration_of_study` (years).
3. "Add speciality" form scoped to the selected faculty: `name`, `duration_of_study`.

## 9. Student Groups page

1. Select **Faculty**, then **Semester** (`GET /admin/faculties/:id/student_groups
   ?semester_id=`).
2. Table of groups active in that semester, displayed as `<group>-<course>`, with student
   count.
3. **"Create group"** form: pick **Speciality** (scoped to the already-selected faculty),
   **Year of admission** (date/year picker). On submit
   (`POST /admin/student_groups`), the response's `generated_semesters` list is shown as
   a confirmation summary ("Created ПИ-25 with 8 semesters: 1 семестр 2025/2026 … 2
   семестр 2028/2029") before returning to the table — makes the bulk-generation behavior
   visible rather than a silent side effect.

## 10. Reference data, faculty structure, and user management screens

Standard admin CRUD table + modal-form pattern (list with search/pagination, "Add" button
opening a form dialog, row actions Edit/Delete with a confirmation for Delete, inline
`409` "cannot delete — in use" toast if the FK guard rejects it):

- Activity types, building types, week types, time slots, posts, academic degrees,
  academic titles, subjects, buildings, room types, rooms (`03-slice-admin.md §2`).
- Faculties, kafedras (nested under a faculty in the nav, but independently listable),
  teachers (with post/degree/title async selects), teacher-on-kafedra assignments
  (`03-slice-admin.md §3`).
- Users & roles (`03-slice-admin.md §1`, `super_admin` only): user list with status
  toggle, "Create user" (email, full name, generates a temporary password shown once),
  role-assignment editor (role + scope picker: global/faculty/kafedra).
- **Schedule-view rules** (`super_admin` only, under *Static Data*): a matrix screen —
  rows = activity types, columns = the three period kinds (lecture week / credit week /
  exam session), checkboxes per cell — saved via
  `PUT /admin/activity_type_period_rules` (`03-slice-admin.md §6`). Activity types with
  no checked cell show an inline warning ("not accepted in any schedule view"), per
  `01-database-schema.md §2.8`.

These screens are intentionally the least custom-built part of the frontend — a single
generic `AdminCrudPage<T>` component (config: columns, form schema, endpoints) covers all
of them, so implementation effort concentrates on the bespoke pages (§2–§7).
