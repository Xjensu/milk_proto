# API Reference — milk_schedule (`apps/api_geteway`)

Generated 2026-07-12 from the actual routes/contracts/presenters (not from the older
aspirational spec docs) and cross-checked against live requests against the running dev
stack. **If this disagrees with `docs/03-slice-admin.md` / `docs/04-slice-main.md`, trust
this file** — those docs describe a plain-nested-JSON response shape; the real API
returns a JSON:API-style envelope almost everywhere (see Conventions below). Confirmed
live, e.g. `GET /admin/teachers`:
```json
{"data":[{"id":"1","type":"teacher","attributes":{"first_name":"Ivan","last_name":"Petrov","sovmestitel":null},
"relationships":{"post":{"data":{"id":"1","type":"post"}}},"links":{"self":"...","load":"...","schedule":"..."}}],
"included":[...],"meta":{"actions":[{"name":"create","href":"...","method":"POST"}]},
"links":{"self":"...","first":"...","next":"..."}}
```
and `POST /auth/sessions` (flat, not JSON:API):
```json
{"access_token":"...","refresh_token":"...","token_type":"Bearer","expires_in":900}
```

Ruby/Hanami 2.3 API. 4 slices mounted per `config/routes.rb`: `authentication` (`/auth`),
`admin` (`/admin`), `portal` (`/public`), `map` (`/map`).

## Conventions

**Envelope.** Most admin/portal endpoints return a JSON:API-ish shape built by a `Presenter`+`Serializer` pair:
```json
{"data": {"id","type","attributes":{...},"relationships":{...}},
 "included": [...],
 "meta": {"actions": [{"name","href","method"}, ...]},
 "links": {"self", ...}}
```
Collections wrap `data` in an array and add top-level pagination `links` (`self/first/prev/next`), built in `slices/admin/actions/concerns/response_helpers.rb#present`. Two other renderers exist and matter for the "is this really JSON:API" question:
- `render_serialized` → bare `jsonapi-serializer` hash (`{data, included}`, **no** pagination `links`, no `meta.actions`).
- `render_json` → fully custom flat/plain hash (used by sessions, uploads/url, media/url, search.unified, me.show, generate_semesters, SSE, map slice, etc).

**Pagination.** Where supported: `?page=` (int, `>0`, default 1), `?per_page=` (int, `>0`, `≤100` default cap, default value 20 — a few endpoints default to 50 instead, noted inline). Endpoints without an `index_contract` page/per_page rule are unpaginated (return everything).

**Errors.** `422` → `{"errors": {"field": ["msg"]}}` or nested `{"errors": {"resource": {"field": ["msg"]}}}`; some ops-style failures → `{"errors": {"code": "message"}}` or `{"error": "message"}`. `401` → `{"error":"unauthorized"}`. `403` → `{"error":"forbidden"}`. `404` → `{"error":"<x> not found"}`. `409` (conflict, e.g. double-booking / concurrent accept) → `{"error": "..."}`.

**Admin auth tiers.** All `/admin/*` routes require `Authorization: Bearer <JWT>` (except `GET /admin/stream`, see below). Reads (index/show) pass for any authenticated **non-self-service** admin account; self-service (`teacher`/`student`) accounts are blocked except `/admin/me/*`. Writes are tiered via `Admin::Authorization::Policy::WRITE_MATRIX` (`slices/admin/authorization/policy.rb`) — resource+action not listed in the matrix **defaults to `GLOBAL`** (secure default-deny):
- **GLOBAL** — `super_admin` only.
- **FACULTY** — + `faculty_admin` if the target row is within their faculty scope.
- **KAFEDRA** — + `kafedra_admin` if within their kafedra scope.
- **SCHEDULE** — + `scheduler` globally (unrestricted) + scoped admins in-scope.

**Fixed 2026-07-13** (was previously a known gap, security review §3.1): `update` now also
resolves the owner implied by any reassignable FK present in the request body (`kafedras.faculty_id`,
`teacher_on_kafedras.kafedra_id`, `curriculum_streams.kafedra_id`, `study_load_requirements.stream_id`,
`teaching_assignments.{load_requirement_id,teacher_on_kafedra_id}`) and requires the caller to hold
**both** the current and the target scope — a scoped admin can no longer pull another kafedra's
resources into their own scope, or exile their own rows into a kafedra nobody manages, via PATCH.

**Fixed 2026-07-13**: the `authorize_as resource:, action: :write` bug (`:write` isn't a real
`WRITE_MATRIX` key, so these silently fell back to `GLOBAL`/super_admin-only regardless of the
tier written for them) is resolved. Current effective tiers: `student_groups#add_semester`/`#generate_semesters`
→ **FACULTY** (resource `student_group_semesters`), `student_group_semesters#load`(POST)/`#load_destroy`(DELETE)
→ **KAFEDRA**, ownerless — reachable by faculty_admin/kafedra_admin/scheduler/super_admin, with the
existing in-handler kafedra scope check still enforced, `schedule_events#from_recommendation` →
**SCHEDULE**. `views#refresh` stays **GLOBAL** deliberately (global infra operation).
`schedule_recommendations#create` was already unaffected (its SCHEDULE tier was always reachable as
designed) — the stray `:write` override there has been removed.

**Read-side scoping (§3.2)**: confirmed as an intentional design decision, not a bug — reads are a
staff-wide visibility panel by design (see `Admin::Authorization::Policy`'s header comment). Only
writes are scope-restricted.

**Fixed 2026-07-15**: `schedule_events#from_recommendation`'s multi-owner gap is closed.
`load_requirement_ids[]` can span several kafedras' load, so `ScopeResolver#multi_owner_groups`
resolves one owner list per id and `Authorizer#authorize!` requires the caller to pass the tier
check against **every** group, not just one — a scoped `kafedra_admin`/`faculty_admin` may now
accept a recommendation only when every referenced load requirement falls within their own scope;
`scheduler` keeps unconditional access. Live-verified: a `kafedra_admin` scoped to kafedra 1
accepting a recommendation entirely within kafedra 1 → passes authorization (reaches the real
placement-conflict check, e.g. `409 slot_taken`, instead of `403`); the same account referencing a
load requirement from a different kafedra → `403`.

---

## 1. Authentication slice (`/auth`)

No RBAC tiers here — self-contained. Responses are **flat, not JSON:API**.

### POST /auth/sessions — login
No auth. Body: `{ email: required string, password: required string }`.
Success `201`:
```json
{"access_token":"...", "refresh_token":"...", "token_type":"Bearer", "expires_in": 900}
```
Failure: 401 `{"error":"unauthorized"}` or 422 validation errors. **Rate-limited**: 10/min per IP, 5/5min per email (confirmed live — repeated test logins tripped this).

### POST /auth/sessions/refresh — rotate access token
No auth header; body `{ refresh_token: string }`. Rotates (single-use) the refresh token via Redis store, reloads the account (rejects if disabled), returns the same flat token-pair shape as login.

### DELETE /auth/sessions — logout
Body `{ refresh_token: string }`. Revokes it (idempotent, server-side). Always `204`, no body.

### POST /auth/accounts — super_admin provisions an account
Auth: bearer JWT + `require_role!("super_admin")` (enforced in code, confirmed live: any other role gets consistent 403/401 behavior on this route).
Body, nested under `account`:
```
email: required, filled string, format /@/
password: optional filled string, min_size 8   (omit → server generates one)
role: required, one of [super_admin, faculty_admin, kafedra_admin, scheduler, teacher, student]
teacher_id: optional integer >0, nullable       (required for role=teacher)
scopes: optional array of { scope_type, faculty_id: int?, kafedra_id: int? }
```
Rules: identity roles (`teacher`/`student`) must not carry `scopes`; `teacher` requires `teacher_id`; global roles (`super_admin`/`scheduler`) must not carry `scopes`; scoped roles (`faculty_admin`/`kafedra_admin`) require ≥1 scope, each scope must set exactly one of `faculty_id`/`kafedra_id` matching the role.
Response (type `account`), `201`: attributes `email, role, status, teacher_id, scopes` (array of `{faculty_id, kafedra_id}`), plus **one-time** `initial_password` attribute present only immediately after a password-less create.

### GET /auth/accounts — list accounts
Auth: super_admin only. `?page=`/`?per_page=` (int>0, ≤100; default 1/20).

### PATCH /auth/accounts/:id — update account
Auth: super_admin only. Body nested under `account`, all optional: `status` (`active`/`disabled`), `password` (min 8), `role`, `teacher_id`, `scopes`. Must supply ≥1 field.

### DELETE /auth/accounts/:id
Auth: super_admin only; `422` `{"error":"cannot delete your own account"}` if targeting self. `204` on success.

### POST /auth/password — self-service password change
Auth: bearer JWT (reachable even when `must_change_password: true`). Body: `{ current_password, new_password (min 8, must differ) }`. Success `200`: fresh flat token pair.

---

## 2. Admin slice (`/admin`)

### 2.1 Reference / lookup CRUD (all GLOBAL-tier writes — super_admin only)

Uniform pattern: `resources :X, only: [:index,:create,:update,:destroy]`, paginated index (page/per_page, per_page≤100).

| Resource | Create body (nested under singular key) | Response attributes | Relationships |
|---|---|---|---|
| `academic_degrees` | `name` (req), `short_name` (req) | `name`, `short_name` | — |
| `academic_titles` | `name` (req), `short_name` (req) | `name`, `short_name` | — |
| `activity_types` | `name` (req), `short_name` (opt, nullable) | `name`, `short_name` | — |
| `aviability_types` | `name` (req) | `name` | — |
| `building_types` | `name` (req), `short_name` (req) | `name`, `short_name` | — |
| `room_types` | `activity_type_id` (req int), `name` (req) | `name` | `belongs_to activity_type` |
| `posts` | `name` (req), `short_name` (req) | `name`, `short_name` | — |
| `semesters` | `start_year` (req int), `end_year` (req int), `number` (req int, 1 or 2) | `number`, `start_year`, `end_year` | — |
| `week_types` | `name` (req), `code` (req) | `name`, `code` | — |
| `time_slots` | `call_number` (opt int, nullable), `start_time` (req), `end_time` (req) | `call_number`, `start_time`, `end_time` | — |
| `buildings` | `building_type_id` (req int), `number` (opt int, nullable) | `number` | `belongs_to building_type` |
| `subjects` | `name` (req), `short_name` (req) | `name`, `short_name` | — |

All updates: same fields, all optional.

### 2.2 Org structure

#### faculties
`GET /admin/faculties` (any admin) · `POST`/`PATCH`/`DELETE` (GLOBAL). Create: `faculty: {name, short_name}` (both req). Response: attrs `name, short_name`; links `self, kafedras, specialities, teachers, groups, schedule, load, public_groups`.
- `GET .../:faculty_id/kafedras` → `kafedra` list.
- `GET .../:faculty_id/groups?semester_id=` (req) → `studying_group` rows: `year_of_admission, speciality_id, speciality_name, duration_of_study, students_now, group_semester_id, course, is_studying`.
- `GET .../:faculty_id/schedule?semester_id=` (req) → `schedule_grid_row[]` (shape below).
- `GET .../:faculty_id/teachers` → `teacher` list.
- `GET .../:faculty_id/load?semester_id=` (req) → `Load::Entry` shape (below).
- `GET .../:faculty_id/specialities` → `speciality` list.

#### kafedras
`GET` any admin · `POST`/`PATCH`/`DELETE` **FACULTY**-tier. Create: `kafedra: {faculty_id (req), name (req), short_name (req)}`. Response: attrs `name, short_name`; `belongs_to faculty`; links `self, teachers, curriculum_streams, schedule, load, public_teachers, public_schedule`.
- `.../:kafedra_id/teachers`, `.../curriculum_streams?semester_id=(req)&course=(opt)`, `.../load?semester_id=(req)`, `.../schedule?semester_id=(req)`.
> **Fixed 2026-07-15**: `.../curriculum_streams` (and the same-serializer siblings
> `student_group_semesters#curriculum_streams`/`#load`(GET)/`#load_destroy`(DELETE)) used to
> `500` (`ROM::Struct::MissingAttribute`) whenever a stream had `study_load_requirements` —
> the repo's DB combine didn't fetch the `teaching_assignments` chain the serializer requests.
> Now returns the full graph (`teaching_assignments.teacher_on_kafedra`,
> `.assignment_room_references.room`) as `included` resources.

#### specialities
`GET` any admin · `POST`/`PATCH`/`DELETE` **FACULTY**. Create: `speciality: {faculty_id (req), name (req), duration_of_study (req int)}`. Response: attrs `name, duration_of_study`; `belongs_to faculty`; `.../student_groups` sub-route.

#### student_groups
`GET`/`GET :id` any admin · `POST`/`PATCH`/`DELETE` **FACULTY**.
Create: `student_group: {speciality_id (req), year_of_admission (req date)}` + optional `auto_provision` (bool, default true — also creates the first semester row), `students_now` (default 1), `semester_duration` (default 18). Update: `student_group: {name?, students_now?, year_of_admission?, faculty_id?}` — **note**: `name`/`faculty_id` are accepted but not present on the response (write-only/legacy fields, don't rely on them round-tripping). Response: attr `year_of_admission` only; `belongs_to speciality`; links `self, semesters`.
- `GET .../:id/semesters` → list.
- `POST .../:id/semesters` (GLOBAL-effective): `student_group_semester: {semester_id (req), students_now?, duration?}` → `201`, bare serialized `student_group_semester`.
- `POST .../:id/generate_semesters` (GLOBAL-effective): no body → `{"data":{"provisioned": <count>}}` (not JSON:API).

#### student_group_semesters
`PATCH`/`DELETE` **FACULTY**. Update: `{students_now?, duration?}`. Response: attrs `students_now, duration, course_number`; `belongs_to semester/student_group`; links `self, curriculum_streams, schedule, load, public_now, public_schedule`.

- **POST `.../:id/load`** (KAFEDRA-tier, ownerless — `student_group_semester_load` in
  `Policy::WRITE_MATRIX`; reachable by faculty_admin/kafedra_admin/scheduler/super_admin, with
  an in-handler all-or-nothing kafedra-scope check) — bulk curriculum ingestion, the main
  load-authoring endpoint:
```
streams: required array (non-empty), each:
  subject_id, kafedra_id, course: required int
  requirements: optional array, each:
    activity_type_id: required int
    hours_per_event, hours_assigned: optional int
    requires_consecutive_pairs: optional bool
    split_strategy: optional string
    teaching_assignments: optional array, each:
      teacher_on_kafedra_id: required int
      room_references: optional array, each: { room_id: required int, priority_level: optional int }
```
  `201` returns the new curriculum_streams (§2.4 shape).
- **GET `.../:id/load`** (any admin) → `Load::Entry` shape.
- **DELETE `.../:id/load`** (KAFEDRA-tier, ownerless — same gate as the POST above) → `200` with
  pre-delete snapshot (curriculum_streams §2.4 shape), or `404` if no distribution exists.
- **GET/POST/PATCH `.../:id/periods`** (GET any admin; POST/PATCH FACULTY): `period: {period_kind (req, one of lecture_week/credit_week/exam_session), start_date (req), end_date (req, ≥start)}` — upserts by `(student_group_semester_id, period_kind)`, same handler for POST/PATCH. No delete route.

**Shared `Load::Entry` shape** (non-JSON:API, `{"data":[...]}`) — backs faculties/load, kafedras/load, teachers/load, me/load, student_group_semesters load(GET), schedule/load_overview:
```
id, hours_per_event, hours_assigned, requires_consecutive_pairs, split_strategy,
activity_type: {id,name,short_name},
stream: {id, course, group_semester_id, subject:{id,name,short_name}, kafedra:{id,name,short_name},
         student_group_semester:{id,semester_id,students_now,course_number,
           semester:{id,number,start_year,end_year}|null,
           student_group:{id,year_of_admission,speciality:{id,name}}|null}},
teachers: [{teaching_assignment_id, teacher_on_kafedra_id, teacher:{id,last_name,first_name,middle_name}|null, kafedra:{id,name}|null}],
stream_siblings: [group_semester_ids...],
reservations: [{id, teaching_assignment_id, priority_level, room:{id,name}|null}],
actual_hours, control_sum,
week_breakdown: {both, even, odd}
```

**Shared `schedule_grid_row` shape** (JSON:API, type `schedule_grid_row`) — backs every admin `.../schedule` read:
attrs `day_of_week, subgroup_number, status, is_manual_change, time_slot_id, time_slot_start_time, time_slot_end_time, week_type_id, week_type_name, room_id, room_name, room_number, subject_id, subject_name, subject_short_name, group_id, group_year_of_admission, group_semester_id, course, kafedra_id, kafedra_name, activity_type_id, activity_type_name, teachers`.

### 2.3 Teachers & self-service

#### teachers
`GET` any admin · `POST` **FACULTY** · `PATCH` **KAFEDRA** (field-restricted) · `DELETE` **FACULTY**.
Create, nested `teacher`: `post_id, academic_degree_id, academic_title_id, first_name, middle_name, last_name` (all req), `sovmestitel` (opt int, nullable); top-level optional `kafedra_assignments: [{kafedra_id (req), pochasovoy (opt)}]`.
**Field policy on update**: a `kafedra_admin` may only PATCH `post_id, academic_degree_id, academic_title_id, sovmestitel` — any other field → `403 {"errors":[{"detail":"Forbidden fields for your role: ..."}]}`.
Response: attrs `first_name, middle_name, last_name, sovmestitel`; `belongs_to post/academic_degree/academic_title`; `has_many teacher_on_kafedras` (lazy); links `self, load, schedule, public_schedule`.
- `.../:teacher_id/load?semester_id=` → `Load::Entry`. `.../:teacher_id/schedule?semester_id=` → `schedule_grid_row[]`.
- `GET .../:teacher_id/kafedras` returns the teacher's current `teacher_on_kafedra` membership rows directly, including the related teacher and kafedra. This bounded endpoint replaces client-side filtering of the global junction list.

#### teacher_on_kafedras (junction)
`GET` any admin · `POST` **FACULTY** · `PATCH` **KAFEDRA** · `DELETE` **FACULTY**.
Create: `{teacher_id (req), kafedra_id (req), pochasovoy (opt, nullable)}`. Response: attr `pochasovoy`; `belongs_to teacher, kafedra`; links `self, teacher, kafedra, public_teacher_schedule`.
`GET ?kafedra_id=` returns at most 100 memberships for that exact department; without
the filter, the standard `page`/`per_page` listing remains unchanged. The workload editor
uses the exact filter so a teacher from another department cannot be selected from a
truncated global page.
The pair `(teacher_id, kafedra_id)` is unique. Duplicate create or update returns `409`. Deleting a membership used by a teaching assignment or schedule event returns `409`; the database foreign keys use `ON DELETE RESTRICT` as the concurrency-safe backstop.
> **Fixed 2026-07-13**: PATCHing `kafedra_id` to a kafedra outside the caller's scope now
> correctly 403s (was the confirmed live exploit in security review §3.1) — the caller must
> hold both the row's current kafedra scope and the target kafedra scope.

#### teaching_assignments
`GET`/`GET :id` any admin · `POST` **FACULTY** · `PATCH` **KAFEDRA** · `DELETE` **FACULTY**.
Create: `{load_requirement_id (req), teacher_on_kafedra_id (req)}`. Response: **no scalar attributes** — only `belongs_to teacher_on_kafedra` (nested teacher+kafedra), `belongs_to load_requirement` (type `load_requirement`: `hours_per_event, hours_assigned, split_strategy, requires_consecutive_pairs` + its own `belongs_to activity_type/curriculum_stream`), `has_many assignment_room_references` (lazy).

#### teacher_time_constraints — direct index/create/destroy
`GET` any staff account; `POST`/`DELETE` **KAFEDRA**. `GET` requires positive
`teacher_id` and `semester_id`. Create:
`{teacher_id, time_slot_id, week_type_id, day_of_week, semester_id (all req positive int), is_available (opt bool)}`.
Response (flat, no relationships): `teacher_id, time_slot_id, week_type_id, day_of_week, is_available, semester_id`.
The direct index and recommendation engine read the authoritative table; the legacy
`GET /admin/views/teacher_constraints` reporting endpoint remains materialized.

#### Self-service `/admin/me/*`
Teacher-linked accounts only; `teacher_id` always derived from JWT.

| Method | Path | Response |
|---|---|---|
| GET | `/admin/me` | flat: `{"data":{"type":"me","id":<account_id>,"attributes":{role, must_change_password, teacher_id, first_name, middle_name, last_name, sovmestitel}}}`; 404 if no linked teacher |
| GET | `/admin/me/semesters` | `semester[]` (`number, start_year, end_year`), limited to semesters reached through the caller's own teaching assignments |
| GET | `/admin/me/kafedras` | type `me_kafedra`, attr `pochasovoy`, `belongs_to kafedra` |
| GET | `/admin/me/load?semester_id=` (opt) | `Load::Entry`, scoped to own teacher_id |
| GET | `/admin/me/schedule?semester_id=` (**required**) | `schedule_grid_row[]`, scoped to own teacher_id |

### 2.4 Curriculum & schedule domain

#### curriculum_streams
`GET` any admin · `POST` **FACULTY** · `PATCH` **KAFEDRA** · `DELETE` **FACULTY**.
Create: `{group_semester_id, subject_id, kafedra_id, course (all req int)}`. Response: attrs `course, group_semester_id`; `belongs_to student_group_semester/kafedra/subject`; `has_many study_load_requirements`; links `self, group_semester, kafedra`. Destroy → `204`.

#### study_load_requirements
`GET` any admin · `POST` **FACULTY** · `PATCH` **KAFEDRA** · `DELETE` **FACULTY**.
Create: `{stream_id (req), activity_type_id (req), hours_assigned (req), hours_per_event (opt), requires_consecutive_pairs (opt bool), split_strategy (opt)}`. Response (type `load_requirement`): attrs `hours_per_event, hours_assigned, split_strategy, requires_consecutive_pairs`; `belongs_to activity_type/curriculum_stream`; `has_many teaching_assignments` (lazy).
**Destroy returns 200 (not 204)** with a cascade snapshot:
```json
{"data":{"id":"..","requirement":{...,"activity_type":{...},"stream":{"id","subject":{...},"kafedra":{...},"group_semester_id"}},
 "cascaded":{"teaching_assignments":[ids],"assignment_room_references":[ids],"schedule_events":[ids],"event_teachers":[ids]}}}
```

#### schedule_events (+ transfers)
`GET` any admin · `POST`/`PATCH`/`DELETE` **SCHEDULE**.
Create, nested `schedule_event`: `load_requirement_id, time_slot_id, room_id, week_type_id` (req int), `status` (req: `scheduled|cancelled|modified`), `day_of_week` (req int — **no 1–6 range check here**, unlike schedule_recommendations), optional `subgroup_number, is_manual_change, change_reason, error, sovmestitel, teacher_on_kafedra_ids[]`. Response: attrs `status, subgroup_number, is_manual_change, change_reason, error, sovmestitel, day_of_week`; `belongs_to study_load_requirement/time_slot/room/week_type`; `has_many event_teachers`. Destroy → `204`.

- **POST `.../from_recommendation`** (SCHEDULE — see multi-owner note above: every `load_requirement_ids[]` entry must be in the caller's scope, not just one): nested `recommendation`: `day_of_week, time_slot_id, call_number, week_type_id` (req int), `week_type_code` (req string), `room_id` (req int), `teacher_on_kafedra_ids[]` (req), `load_requirement_ids[]` (req); optional `score, group_semester_ids[], pair_event_id, is_day_edge, week_type_recommendation, subgroup_number`. `201` on success; `409` on `:slot_taken` race, else `422`.
> **Fixed 2026-07-15**: `from_recommendation` used to `500` (`ROM::Struct::MissingAttribute:
> undefined method 'room'`) on a *successful* create — the event was actually written, but the
> response serialization crashed because the operation returned the bare just-inserted struct
> instead of re-fetching with the presenter's full combine graph (the same re-fetch the sibling
> `schedule_events#create` action already did). Now returns a proper `201` with the full graph.
- **POST `.../:id/transfers`** (SCHEDULE): nested `event_transfer`: `original_date` (req), `transfer_type` (req: `moved`|`cancelled`); optional `new_date, new_time_slot_id, new_room_id, reason` (`new_date` required when `moved`). `201`, or `422` if a transfer already exists for that occurrence.
- **DELETE `/admin/event_transfers/:id`** → `204`.

#### event_teachers — create/update/destroy only
`POST`/`PATCH`/`DELETE` **SCHEDULE**. Create: `{event_id (req), teacher_on_kafedra_id (req)}`. Response: no scalar attrs, only `belongs_to teacher_on_kafedra`.

#### schedule_recommendations — create only
`POST /admin/schedule_recommendations`, **SCHEDULE**. Rate-limited: 30/min per IP. Body nested `schedule_recommendation`: `student_group_semester_id, day_of_week, time_slot_id` (req int, **`day_of_week` must be 1–6**); optional `split_threshold` (int).
> **Fixed 2026-07-13** (security review §4.1): any array anywhere in the request body longer
> than 500 elements — including undeclared fields the contract doesn't even define — is now
> rejected with `422` before dry-validation's schema coercion ever walks it, closing the
> unauthenticated-cost DoS lever this endpoint used to have (was 13ms→3.5s scaling with an
> undeclared array's length). This guard applies to every admin/auth/portal/map POST/PATCH
> body, not just this endpoint.

Response (non-JSON:API, `{"data":[...groups]}`), each group:
```
load_requirement_id, hours_per_event, hours_assigned, requires_consecutive_pairs, split_strategy,
subject:{id,name,short_name}, activity_type:{id,name,short_name},
stream:{id,course,group_semester_id,subject,kafedra,student_group_semester:{...}},
actual_hours, control_sum, requires_split, subgroup_number,
teachers:[{teacher_on_kafedra_id,id,last_name,first_name,middle_name}],
placements:[{score,day_of_week,time_slot_id,call_number,week_type_id,week_type_code,room_id,room_name,room_number,
             teacher_on_kafedra_ids,load_requirement_ids,group_semester_ids,pair_event_id,is_day_edge,
             week_type_recommendation,subgroup_number}]
```
Each `placements[]` entry is exactly the `recommendation` body accepted by `POST /admin/schedule_events/from_recommendation` — pass one straight through to accept it. Failures: `404` (`:not_found`), else `422` (`:no_remaining_hours`, `:slot_unavailable`, `:recommendation_failed`).

#### Schedule load overview + SSE

| Method | Path | Alias | Auth |
|---|---|---|---|
| GET | `/admin/schedule/load` | `/admin/schedule/load_overview` | any admin |
| GET | `/admin/schedule/load/stream` | `/admin/schedule/load_stream` | any admin — standard Bearer header only, **no** `?access_token=` fallback (unlike `/admin/stream`), so a native browser `EventSource` can't authenticate directly against this one |

`load`: `semester_id` (req), `faculty_id` (opt). `load/stream`: **both** `semester_id` and `faculty_id` required. Overview → `Load::Entry` shape.

**SSE stream**: hijacks the socket; `501` if unsupported; `503` (`Retry-After: 30`) past 50 concurrent connections. Subscribes to Redis `schedule.load.faculty.#{faculty_id}`. Sends `event: load.snapshot` (full array) then `event: load.updated` (single object) per recompute; `: keepalive` every 15s. Entry shape (leaner than the REST overview):
```
id, hours_assigned, hours_per_event, requires_consecutive_pairs, split_strategy,
activity_type:{id,name,short_name}, stream:{id, subject:{id,name,short_name}},
actual_hours, control_sum, week_breakdown:{both,even,odd}
```

### 2.5 Rooms, availability, reservations, calendar

#### rooms
`GET` any admin · `POST`/`PATCH`/`DELETE` GLOBAL. Create: `{building_id (req), room_type_id (req), name (opt), number (opt)}`. Response: attrs `name, number`; `belongs_to building/room_type`; links `self, schedule, public_schedule, public_occupancy`. `.../:room_id/schedule?semester_id=` (req) → `schedule_grid_row[]`, unpaginated.

#### room_avialabilities
`GET`/`POST`/`PATCH`/`DELETE` GLOBAL. Create: `{room_id (req), aviability_type_id (req), name (req), number (opt, nullable)}`. Response: attrs `name, number`; `belongs_to room, aviability_type`.

#### assignment_room_references
`GET` any admin (index has **no filter params**, just pagination) · `POST`/`PATCH`/`DELETE` **KAFEDRA**. Create: `{teaching_assignment_id (req), room_id (req), priority_level (opt int)}`. Response: attr `priority_level`; `belongs_to teaching_assignment, room`.

#### room_reservations — no update route
`GET ?room_id=&date_from=&date_to=&page=&per_page=` (all optional, default per_page **50**), any admin · `POST` **SCHEDULE** · `DELETE` **SCHEDULE** (soft-delete: sets `status=cancelled`, doesn't remove the row).
Create: `{room_id, teacher_id, reserved_date, time_slot_id (all req), purpose (opt)}`. Conflict → `409 {"error":"room already occupied for that date/slot"}`. Response (flat): `room_id, teacher_id, reserved_date, time_slot_id, purpose, status, created_by_id, created_at`.

#### holidays — no update route, GLOBAL only
`GET`/`POST` (default per_page 50), `DELETE`. Create: `{holiday_date (req), name (req)}`. Duplicate date → `422`.

#### weekday_transfers — no update route
`GET`/`POST` (default per_page 50), `DELETE`, **SCHEDULE**. Create: `{transfer_date (req), follows_weekday (req int 1-6), student_group_id (opt), reason (opt)}`. Duplicate → `422`.

#### aviability_types
`GET`/`POST`/`PATCH`/`DELETE`, GLOBAL. Create: `{name (req)}`.

### 2.6 Uploads, media, audit, search, views, stream

| Method | Path | Auth |
|---|---|---|
| POST | `/admin/uploads/excel` | GLOBAL (super_admin only), resource `uploads_excel` |
| POST | `/admin/uploads/media` | KAFEDRA-ownerless (any admin-tier role), resource `uploads_media` |
| GET | `/admin/files/:id/url` | any admin + ownership check |
| GET | `/admin/files` | any admin |
| DELETE | `/admin/files/:id` | GLOBAL |

Rate-limited: 20/min per IP on both upload endpoints (security review §4.1).

- **POST /admin/uploads/excel**: multipart `file` restricted to xlsx/xls/geo+json/json (checked by real content sniffing, not the declared Content-Type — confirmed live); optional `semester_id`. `201` → type `excel_file`, attrs `title, status, semester_id, created_at, updated_at`; links `self, url`.
- **POST /admin/uploads/media**: multipart `file`, magic-byte MIME sniff (images/pdf/docx/xlsx), max 50MB — confirmed live that disguised executables/scripts are rejected regardless of claimed Content-Type. Uploads to quarantine, async-sanitized; poll `GET /admin/media_files/:id` or listen on `GET /admin/stream`.
  > **Fixed 2026-07-13** (security review §3.4): this was previously misclassified as a
  > `:read` action by the class-name-suffix inference (its class is `Media`, not
  > `Create`/`Update`/`Destroy`), so it skipped both the write-policy tier and audit
  > logging — any admin-tier role could upload media, unaudited. It's now an explicit
  > `authorize_as resource: "uploads_media", action: :create` and correctly lands in
  > `audit_logs` like every other write. The effective access level (any admin-tier role)
  > is unchanged — only the classification and audit trail were fixed.
- **GET /admin/files/:id/url** / **GET /admin/media_files/:id/url**: flat `{"url":"...", "expires_in":900, "filename":"..."}` — 15-min presigned MinIO URL, always forces download (`Content-Disposition: attachment`), never renders inline.
- **GET /admin/media_files/:id/url** additionally returns `422` if the file isn't `clean` yet (still being sanitized).
- **DELETE /admin/files/:id**: `204`.

#### media_files
`GET` (index/show) any admin — admin-tier roles (`super_admin`/`faculty_admin`/`kafedra_admin`/`scheduler`)
see all, others only their own uploads. (**Fixed 2026-07-13**: `index`/`show` used to check for a
role literally named `"admin"`, which doesn't exist, so only `super_admin` ever saw the full list —
now unified with the same `ADMIN_ROLES` list `uploads/url`/`media_files/url` already used correctly.)
Response (type `media_file`): attrs `title, status, detected_mime, original_mime, file_size, rejection_reason, created_at, updated_at, servable (bool), formatted_size, file_type`; links `self, url (null unless clean), events`.

#### audit_logs — super_admin only
`GET ?entity_type=&entity_id=&user_id=&date_from=&date_to=&page=&per_page=` (default 50), `GET /:id`. Response: attrs `user_id, action, entity_type, entity_id, before_data, after_data, ip_address, created_at`.

#### search (pg_trgm, `?q=`, min 3 chars) — any admin
- `GET /admin/search?q=&types[]=` (subset of `disciplines/kafedras/teachers`) → flat `{"disciplines":[...],"kafedras":[...],"teachers":[...]}` (bare arrays, not full envelope).
- `GET /admin/search/disciplines?q=` / `/kafedras?q=` / `/teachers?q=` → bare `{"data":[...], "included":[...]}`.
- `GET /admin/search/rooms?q=` searches room name/number and building number, escaping
  wildcard characters and returning at most 50 fully serialized room rows.

#### views — materialized-view reads + refresh
`GET /admin/views/teaching_load?faculty_id=&semester_id=`, `.../teacher_constraints?teacher_id=&semester_id=`, `.../room_reservations?faculty_id=&semester_id=` (all params optional, any admin); `POST /admin/views/refresh` (GLOBAL). No pagination. Response type/attrs are wide flat rows — see full field lists in the source catalog if you need every column; `refresh` → `{"data":{"id":"current","type":"view_refresh","attributes":{"status":"..."}}}`.

#### POST /admin/stream/ticket + GET /admin/stream — real-time SSE
**Changed 2026-07-13** (security review §3.3): the long-lived access JWT no longer travels
in the SSE query string. Instead:
1. `POST /admin/stream/ticket` — normal Bearer-header auth, any admin-tier role. `201`:
   `{"data":{"type":"stream_ticket","attributes":{"ticket":"...","expires_in":30}}}`.
   Going through the full auth pipeline to mint this also closes the previous
   `must_change_password` bypass on this route (security review §3.7).
2. `GET /admin/stream?ticket=<ticket>` — the ticket from step 1, single-use (redeemed/deleted
   on first read) and expires in 30s even if unused, so a log line that captures it is
   worthless. `401` `{"error":"Missing ticket"}` / `{"error":"Invalid or expired ticket"}`;
   `403` if the ticket's bound role isn't admin-tier. `text/event-stream`, no `event:` name
   set (client sees default `message`), heartbeat comment every 25s. Payload after every
   successful admin write (and media sanitization):
```json
{"resource":"teachers","action":"update","id":"42","account_id":"7","at":"2026-07-12T10:00:00Z"}
```
(`status`/`reason` keys added only for media-sanitization pushes.)

---

## 3. Portal slice (`/public`) — fully public, read-only

Auth optional: missing/invalid Bearer never rejects, just runs anonymously; a valid **teacher** token adds an `is_me` flag on `rooms.occupancy`/`groups.now` results. All routes `GET`, real JSON:API envelope throughout.

Faculties/semesters/kafedras/teachers/rooms index and faculties/groups are filtered to only entities that actually appear in `schedule_events` (no dead pickers with zero schedule data).

| Method/Path | Query params | Response | Pagination |
|---|---|---|---|
| GET /public/faculties | `page`/`per_page` | `name, short_name` | yes (**fixed 2026-07-13** — was validated but ignored, always returned everything) |
| GET /public/semesters | `page`/`per_page` | `number, start_year, end_year` | yes |
| GET /public/kafedras | `page`/`per_page` | `name, short_name, faculty_id` | yes |
| GET /public/kafedras/:kafedra_id/teachers | — | `teacher` + post/degree/title | none |
| GET /public/kafedras/:kafedra_id/schedule?semester_id= (req) | | `schedule_grid_row[]` | none |
| GET /public/faculties/:faculty_id/groups?semester_id= (req) | | `speciality_name, course, students_now, group_semester_id, year_of_admission, duration_of_study` | none |
| GET /public/teachers | `q` (min 2 chars, free-text FIO — bypasses pagination) else `page`/`per_page` | `teacher` list | yes (only without `q`) |
| GET /public/teachers/:teacher_id/schedule?semester_id= (req) | | `schedule_grid_row[]` | none |
| GET /public/rooms | `page`/`per_page` | `room` + building/room_type | yes |
| GET /public/rooms/:room_id/occupancy?datetime= (opt) | | `room_occupancy` (below) | n/a |
| GET /public/rooms/:room_id/schedule?semester_id= (opt, blank→`[]`) | | `schedule_grid_row[]` | none |
| GET /public/groups/:group_semester_id/now?datetime= (opt) | | `group_now` (below) | n/a |
| GET /public/groups/:group_semester_id/schedule | | `schedule_grid_row[]` | none |

Portal `schedule_grid_row` (leaner than admin's — no internal ids): `day_of_week, subgroup_number, status, time_slot_start_time, time_slot_end_time, week_type_name, room_name, room_number, subject_name, subject_short_name, group_semester_id, course, kafedra_name, activity_type_name, teachers[]({id,last_name,first_name,middle_name,teacher_on_kafedra_id}), speciality_name, group_display_name`.

**`rooms.occupancy`** — free/occupied right now (or `?datetime=`): `resolved_at, day_of_week, week_type_name, time_range, is_occupied (bool), current[]` (each: `subject_name, subject_short_name, course, group_semester_id, group_display_name, kafedra_name, activity_type_name, subgroup_number, teachers[]`). Teacher token adds `is_me: bool` per teacher entry.

**`groups.now`** — same idea keyed by group: `has_class (bool), current[]` (`room_name, room_number, subject_name, subject_short_name, course, kafedra_name, activity_type_name, subgroup_number, teachers[]`).

---

## 4. Map slice (`/map`)

**Correction to a common assumption**: this slice **requires** a valid Bearer JWT (any of
the 6 roles passes, no scope check) — confirmed live (401 without a token). It is not
public despite living outside `/admin`. `Content-Type` is always
`application/vnd.api+json` even though every body is plain GeoJSON, not JSON:API —
a header/body mismatch to be aware of client-side (don't assume JSON:API parsing here).
No pagination anywhere; raw PostGIS SQL, no presenter classes.

| Method/Path | Params | Response |
|---|---|---|
| GET /map/buildings | — | `FeatureCollection`; feature: `id, geometry, properties:{name, number, code}` |
| GET /map/rooms | — | feature: `id, geometry, center` (point), `properties` (raw passthrough) |
| GET /map/floors | — | feature: `id, level` — **no geometry key** |
| GET /map/search/rooms | `query` (ILIKE, omitted → all) | same as `/map/rooms`, **hard-capped LIMIT 10** |
| GET /map/vertical_transitions | — | feature: `id, geometry, transition_type, properties` |
| GET /map/virtual_layers | — | feature: `id, geometry, layer_name, properties` |
| GET /map/floor_edge_collections | — | feature: `id, geometry, properties` |
| GET /map/vertices | — | feature: `id, floor` (int level), `geometry, coord_hash` |
| GET /map/edges | — | feature: `id, geom` (key is `geom`, not `geometry`), `source, target, weight, reverse_weight, properties` |
| GET /map/routes/room_to_room | `from`, `to` (req int room ids) | see below |
| GET /map/map_room_graph_links | — | feature (**no top-level id**): `geometry, properties:{floor, room}` |

**`routes/room_to_room`** — each Feature in the returned `FeatureCollection` is **one
ordered path segment** (`properties: {step, cost, floor_from, floor_to}`), not one combined
line — concatenate by `step` order and watch `floor_from`/`floor_to` for level changes.
`404 {"error":"One or both rooms are not connected to the routing graph"}` if disconnected;
empty `FeatureCollection` (200) if no path — frontend should treat both as "no route."

---

## Notes for frontend integration

1. **Always check `meta.actions`/`links` on JSON:API responses instead of hardcoding which
   verbs are allowed** — the presence of a `create`/`update`/`destroy` affordance reflects
   the caller's actual permissions for that resource.
2. **Cross-tenant scope enforcement on reassignable FKs is now server-enforced** (`kafedras.faculty_id`,
   `teacher_on_kafedras.kafedra_id`, `curriculum_streams.kafedra_id`, `study_load_requirements.stream_id`,
   `teaching_assignments.{load_requirement_id,teacher_on_kafedra_id}`) — a scoped admin attempting to
   PATCH one of these to an out-of-scope target now gets `403`. Client-side guards against this are
   no longer strictly necessary, though still good UX (fail fast before the round-trip).
3. Several resources return **200 with a payload on delete** instead of `204`
   (`study_load_requirements` destroy returns a cascade snapshot) — don't assume every
   DELETE is bodyless.
4. Two SSE endpoints exist with different auth mechanisms: `/admin/stream` (ticket-based —
   `POST /admin/stream/ticket` then `GET /admin/stream?ticket=`, works with native
   `EventSource`) vs. `/admin/schedule/load/stream` (header-only, needs a fetch-based SSE
   client, not a browser `EventSource`).
5. **Any array anywhere in a POST/PATCH body longer than 500 elements is rejected with
   `422`**, across every slice (admin/auth/portal/map) — including under keys a contract
   doesn't even declare. Don't send paginated bulk operations as one giant array; page them
   client-side into batches ≤500.
6. `POST /admin/schedule_recommendations` (30/min per IP) and `POST /admin/uploads/*`
   (20/min per IP) are now individually rate-limited, on top of the existing login/refresh/
   password/public-search throttles — a `429 {"error":"too many requests"}` from these
   isn't necessarily abuse, just budget for it in bulk-import UIs.
