# API Gateway — Frontend Integration Reference

**Base URL:** `http://<host>:2300`
**Date:** 2026-07-15
**Content type:** `admin` slice returns `application/vnd.api+json` (JSON:API); `authentication` and `portal` slices return `application/json`. Send request bodies as `application/json`.

This document describes what each endpoint returns so the client can be built against it. It reflects the **live** behavior of the running service, not just the source.

---

## 1. Conventions

### Envelope (JSON:API — `admin` slice)
List and single-resource responses use:
```jsonc
{
  "data": { "id": "1", "type": "faculty", "attributes": { … }, "relationships": { … }, "links": { … } },
  "included": [ … ],        // present when ?include or a serializer default pulls related resources
  "meta":  { "actions": [ { "name": "create", "href": "…", "method": "POST" }, … ] },
  "links": { "self": "…?page=1&per_page=20", "first": "…", "next": "…" }  // lists only
}
```
- `id` is always a **string**, even though it is a numeric PK server-side.
- `relationships.<name>.data` gives `{id,type}` linkage; the full related object appears in `included` when the endpoint eager-loads it.
- `links` inside a resource are HATEOAS navigation URLs — prefer following them over string-building.
- `meta.actions` advertises the write operations available on a collection.

### Pagination
- Query params: `?page=<int ≥1>&per_page=<int, bounded>`.
- Invalid values (`0`, negative, non-numeric, oversized) → **422**. Default `per_page` = 20.
- List `links` carry `self` / `first` / `next` (and `prev` where applicable).

### Errors
| Status | Body | When |
|--------|------|------|
| 400 | `{"error":"Malformed request body"}` | Broken / too-deeply-nested JSON |
| 401 | `{"error":"unauthorized"}` | Missing/invalid/expired token |
| 403 | `{"error":"forbidden"}` | Authenticated but not permitted |
| 403 | `{"error":"password_change_required", "message":"…"}` | `must_change_password` gate |
| 404 | `{"error":"<Resource> not found"}` | Unknown id on update/show |
| 422 | `{"errors":{"<field>":["message"]}}` or `{"errors":{"base":["request could not be processed"]}}` | Validation / coercion / DB constraint failure |
| 429 | `{"error":"too many requests"}` | Rate limit tripped |

Validation errors are keyed by field (nested under the resource key, e.g. `{"errors":{"faculty":{"name":["must be a string"]}}}`). Do not rely on 500-level bodies — the API converts DB errors to 422.

---

## 2. Authentication (`/auth`, `application/json`)

### `POST /auth/sessions` — login
Request: `{ "email": "...", "password": "..." }`
Response `201`:
```json
{ "access_token": "<JWT>", "refresh_token": "<opaque>", "token_type": "Bearer", "expires_in": 900 }
```
- Access token TTL = **900 s (15 min)**. Send as `Authorization: Bearer <access_token>`.
- Wrong credentials / unknown user → uniform `401 {"error":"invalid email or password"}`.
- Rate limits: 10/min per IP, 5/5min per email.

### `POST /auth/sessions/refresh` — rotate
Request: `{ "refresh_token": "..." }` → same envelope as login. The presented refresh token is **single-use** (rotated); store the new one. Old token becomes invalid.

### `DELETE /auth/sessions` — logout
Revokes the refresh token. Always permitted (even under the password-change gate).

### `POST /auth/password` — self-service password change
Request: `{ "current_password": "...", "new_password": "..." }` → returns a **fresh token pair**. Required (min 8 chars, policy-enforced). Clears `must_change_password` and revokes all other sessions.

### `POST /auth/accounts` — create account *(super_admin only)*
Request:
```jsonc
{ "account": {
    "email": "user@x",
    "role": "faculty_admin",              // super_admin|faculty_admin|kafedra_admin|scheduler|teacher|student
    "password": "optional — omit to auto-generate",
    "teacher_id": 12,                      // required for role=teacher
    "scopes": [ { "faculty_id": 2 } ]      // faculty_admin→faculty_id; kafedra_admin→kafedra_id; exactly one per row
} }
```
Response `201`: account resource; when the password was auto-generated it is returned **once** as `attributes.initial_password` and the account has `must_change_password: true`.

Scope rules (validated): global roles (super_admin/scheduler) and identity roles (teacher/student) must **not** carry scopes; scoped roles require ≥1 correctly-typed scope.

### `GET /auth/accounts` · `PATCH /auth/accounts/:id` · `DELETE /auth/accounts/:id`
`super_admin` only. Account shape:
```json
{ "data": { "id": "<uuid>", "type": "account",
  "attributes": { "email": "...", "role": "scheduler", "status": "active|disabled",
                  "teacher_id": null, "scopes": [ {"faculty_id":2,"kafedra_id":null} ] } } }
```

---

## 3. Authorization model (what the client can expect per role)

| Role | Reads | Writes |
|------|-------|--------|
| `super_admin` | everything | everything |
| `faculty_admin` | **all** resources (staff-wide read) | faculty/kafedra/schedule-tier resources **within its faculty scope** |
| `kafedra_admin` | all resources | in-kafedra edits within scope (cannot create/delete structure or load) |
| `scheduler` | all resources | all schedule-tier resources globally |
| `teacher` | only `/admin/me/*` | none |
| `student` | reserved | none |

Practical client implications:
- Any admin-tier account can render every read view; hide/disable **write** controls based on role + scope, and treat `403` as the authoritative boundary.
- A `teacher` account should be routed only to the `/admin/me/*` self-service views.

---

## 4. Admin resources (`/admin`, `application/vnd.api+json`, Bearer required)

### 4.1 Standard CRUD resources
All expose `GET (index)`, `POST (create)`, `PATCH /:id (update)`, `DELETE /:id (destroy)` unless noted, and follow the JSON:API envelope with `meta.actions`.

`faculties`, `kafedras`, `semesters`, `buildings`, `building_types`, `rooms`, `room_types`, `aviability_types`, `room_avialabilities`, `subjects`, `teachers`, `teacher_on_kafedras`, `teaching_assignments`, `time_slots`, `week_types`, `study_load_requirements`, `activity_types`, `posts`, `academic_titles`, `academic_degrees`, `curriculum_streams`, `specialities`, `assignment_room_references`, `student_groups` (also `GET /:id`).

**Create/update body shape** — wrap attributes under the singular resource key (Rails-style params, **not** JSON:API `data`):
```jsonc
POST /admin/faculties   { "faculty":       { "name": "...", "short_name": "..." } }
POST /admin/kafedras    { "kafedra":       { "name": "...", "short_name": "...", "faculty_id": 2 } }
POST /admin/teachers    { "teacher":       { "first_name":"...","last_name":"...","middle_name":"...",
                                             "post_id":1,"academic_degree_id":1,"academic_title_id":1 },
                          "kafedra_assignments": [ { "kafedra_id": 3, "pochasovoy": false } ] }  // optional
```
> ℹ️ **`id`, `created_at`, `updated_at` are server-owned and read-only.** If sent in a write body they are silently ignored (stripped by the repository layer) — the server assigns the id and timestamps. Never rely on setting them from the client.

**Sample — faculty (index item):**
```json
{ "id":"2","type":"faculty",
  "attributes":{"name":"Факультет управления процессами перевозок","short_name":"УПП"},
  "links":{"self":"…/admin/faculties/2","kafedras":"…/2/kafedras","specialities":"…/2/specialities",
           "teachers":"…/2/teachers","groups":"…/2/groups","schedule":"…/2/schedule","load":"…/2/load",
           "public_groups":"…/public/faculties/2/groups"} }
```

**Sample — teacher (index, with `included`):**
```json
{ "data":[ { "id":"1","type":"teacher",
    "attributes":{"first_name":"Ivan","middle_name":"Oleksandrovych","last_name":"Petrov","sovmestitel":null},
    "relationships":{"post":{"data":{"id":"1","type":"post"}},
                     "academic_degree":{"data":{"id":"1","type":"academic_degree"}},
                     "academic_title":{"data":{"id":"1","type":"academic_title"}},
                     "teacher_on_kafedras":{"data":[{"id":"1","type":"teacher_on_kafedra"}]}},
    "links":{"self":"…/teachers/1","load":"…/teachers/1/load","schedule":"…/teachers/1/schedule",
             "public_schedule":"…/public/teachers/1/schedule"} } ],
  "included":[ {"id":"1","type":"post","attributes":{"name":"Доцент","short_name":"доц."}}, … ] }
```

**Sample — time_slot:** `{"id":"1","type":"time_slot","attributes":{"call_number":1,"start_time":"… 08:30:00 +0000","end_time":"… 09:55:00 +0000"}}`
(Times serialize with a placeholder date — use the time-of-day component only.)

### 4.2 Nested / relational reads (GET)
```
/admin/faculties/:faculty_id/{kafedras,groups,schedule,teachers,load,specialities}
/admin/kafedras/:kafedra_id/{teachers,curriculum_streams,load,schedule}
/admin/specialities/:speciality_id/student_groups
/admin/teachers/:teacher_id/{load,schedule}
/admin/rooms/:room_id/schedule
```
Path ids are integer-validated → non-integer yields `422 {"errors":{"<id>":["must be an integer"]}}`.

> **`semester_id` is a required query param** on the schedule/load/curriculum_streams/groups projections
> (`faculties/:id/{groups,load,schedule}`, `kafedras/:id/{curriculum_streams,load,schedule}`,
> `teachers/:id/{load,schedule}`, `rooms/:id/schedule`). Omitting it → `422 {"errors":{"semester_id":["is missing"]}}`.
> These endpoints return an empty `{"data":[]}` when the (entity, semester) pair has no rows — not a 404.

### 4.3 Student-group semesters & load
```
GET/POST   /admin/student_groups/:id/semesters
POST       /admin/student_groups/:id/generate_semesters
PATCH/DEL  /admin/student_group_semesters/:id
POST/GET/DEL /admin/student_group_semesters/:id/load          // bulk load distribution
GET        /admin/student_group_semesters/:id/{curriculum_streams,schedule}
GET/POST/PATCH /admin/student_group_semesters/:id/periods     // academic calendar windows
```

### 4.4 Scheduling
```
GET/POST/PATCH/DELETE /admin/schedule_events
POST   /admin/schedule_events/from_recommendation
POST/GET /admin/schedule_events/:id/transfers      DELETE /admin/event_transfers/:id
POST   /admin/schedule_recommendations             // compute viable placements (rate-limited 30/min/IP)
POST/PATCH/DELETE /admin/event_teachers
GET    /admin/schedule/load_overview   GET /admin/schedule/load_stream
POST   /admin/teacher_time_constraints  DELETE /admin/teacher_time_constraints/:id
```
**Sample — schedule_event:**
```json
{ "id":"1","type":"schedule_event",
  "attributes":{"status":"modified","subgroup_number":0,"is_manual_change":false,
                "change_reason":"…","error":null,"sovmestitel":null,"day_of_week":5},
  "relationships":{"study_load_requirement":{"data":{"id":"1","type":"load_requirement"}},
                   "time_slot":{"data":{"id":"1","type":"time_slot"}},
                   "room":{"data":{"id":"1","type":"room"}},
                   "week_type":{"data":{"id":"2","type":"week_type"}},
                   "event_teachers":{"data":[{"id":"1","type":"event_teacher"}]}},
  "links":{"self":"…/schedule_events/1","load_requirement":"…/study_load_requirements/1","room":"…/rooms/1"} }
```
`day_of_week`: 1=Mon … 6=Sat. `week_type` codes: `even` (над чертой) / `odd` (под чертой) / `both` (общая).

### 4.5 Calendar, reservations, search, audit, views
```
GET/POST/DELETE  /admin/room_reservations   /admin/holidays   /admin/weekday_transfers
GET   /admin/search?q=…&types[]=disciplines&types[]=kafedras   // q required (422); types is an ARRAY param
GET   /admin/search/{disciplines,kafedras,teachers}?q=…        // per-entity trigram

GET   /admin/audit_logs  GET /admin/audit_logs/:id
GET   /admin/views/{teaching_load,teacher_constraints,room_reservations}   POST /admin/views/refresh
```

### 4.6 Uploads & media
```
POST /admin/uploads/excel   (super_admin) — curriculum/load ingestion
POST /admin/uploads/media   multipart field "file"; ≤ 50 MB
GET  /admin/files/:id/url   /admin/media_files/:id/url   // presigned URL for a clean file
GET  /admin/media_files (index) /:id (show)   GET /admin/files   DELETE /admin/files/:id
```
Media upload flow the client must handle:
- `POST /admin/uploads/media` returns **`201`** with `status:"quarantined"`, `servable:false`, `detected_mime`, `file_size`, `formatted_size`, `file_type`.
- The file is scanned/re-encoded **asynchronously**; poll `GET /admin/media_files/:id` for `status` transition to `clean` (then `servable:true`, request a URL) or `rejected` (with `rejection_reason`).
- Oversize → `422 {"errors":{"file":["file too large (… B) — max 52428800 B"]}}`; missing field → `422 must be a multipart file upload`.

Media resource shape:
```json
{ "id":"23","type":"media_file","attributes":{
  "title":"x.png","status":"quarantined","detected_mime":"image/png","original_mime":"image/png",
  "file_size":67,"rejection_reason":null,"servable":false,"formatted_size":"67 B","file_type":"image",
  "created_at":"…","updated_at":"…"} }
```

### 4.7 Real-time (SSE)
```
POST /admin/stream/ticket   → { ticket: "<single-use, 30s TTL>" }
GET  /admin/stream?ticket=<ticket>   → text/event-stream
```
Exchange the Bearer token for a one-time ticket, then open `EventSource(/admin/stream?ticket=…)`. Events are published on admin writes: `{ resource, action, id, account_id }`.

### 4.8 Self-service (`teacher` role)
```
GET /admin/me            /admin/me/kafedras   /admin/me/load   /admin/me/schedule
```
`teacher_id` is derived from the JWT — never sent as a param. Restricted to the caller's own data.

---

## 5. Public portal (`/public`, `application/json`, no auth)

Trimmed, read-only projections filtered to entities that actually appear in the schedule. Rate-limited (60/min/IP; search 20/min/IP).
```
GET /public/faculties        GET /public/semesters       GET /public/kafedras
GET /public/kafedras/:kafedra_id/{teachers,schedule}
GET /public/faculties/:faculty_id/groups
GET /public/teachers         GET /public/teachers/:teacher_id/schedule   GET /public/teachers?q=…
GET /public/rooms            GET /public/rooms/:room_id/{occupancy,schedule}
GET /public/groups/:group_semester_id/{now,schedule}
```
Public faculty shape is minimal (no HATEOAS links):
```json
{ "data":[ {"id":"1","type":"faculty","attributes":{"name":"…","short_name":"…"}} ] }
```

---

## 6. Map / navigation (`/map`, GeoJSON/PostGIS)
```
GET /map/buildings   /map/rooms   /map/floors   /map/vertices   /map/edges
GET /map/search/rooms   /map/vertical_transitions   /map/virtual_layers
GET /map/floor_edge_collections   /map/map_room_graph_links
GET /map/routes/room_to_room     // indoor routing between two rooms
```

---

## 7. Appendix A — Live response samples

Captured from the running service (2026-07-15). Bodies trimmed for length; shapes are exact.

**`GET /admin/faculties/:id/kafedras`** — kafedras scoped to a faculty:
```json
{ "data": [ { "id":"14","type":"kafedra",
  "attributes":{"name":"Информационные технологии","short_name":"ИТ"},
  "relationships":{"faculty":{"data":{"id":"2","type":"faculty"}}} } ] }
```

**`GET /admin/faculties/:id/teachers`** — note `sovmestitel` is `0|1`, and `teacher_on_kafedras` may be an empty object `{}` when none:
```json
{ "data": [ { "id":"2","type":"teacher",
  "attributes":{"first_name":"Иван","middle_name":"Степанович","last_name":"Акур","sovmestitel":0},
  "relationships":{"post":{"data":{"id":"0","type":"post"}},
                   "academic_degree":{"data":{"id":"13","type":"academic_degree"}},
                   "academic_title":{"data":{"id":"0","type":"academic_title"}},
                   "teacher_on_kafedras":{}} } ] }
```

**`GET /admin/faculties/:id/load?semester_id=1`** and **`GET /admin/schedule/load_overview?semester_id=1`** — the load-projection shape (plain JSON, **not** JSON:API; nested objects, numeric ids):
```json
{ "data": [ {
  "id":1783, "hours_per_event":2, "hours_assigned":1,
  "requires_consecutive_pairs":false, "split_strategy":"TOGETHER",
  "activity_type":{"id":1,"name":"лекция","short_name":"лек"},
  "stream":{ "id":1049, "course":2, "group_semester_id":435,
    "subject":{"id":1464,"name":"Математика","short_name":"Математика"},
    "kafedra":{"id":4,"name":"Высшая математика","short_name":"ВМ"},
    "student_group_semester":{"id":435,"semester_id":1,"students_now":1,"course_number":2} } } ] }
```
`load_overview` additionally carries `actual_hours` / control-sum enrichment per row; `split_strategy` ∈ `TOGETHER|SEPARATE`.

**`GET /admin/audit_logs`** — `after_data`/`before_data` are JSON **strings** (stringified snapshot), not nested objects:
```json
{ "data":[ { "id":"75","type":"audit_log","attributes":{
  "user_id":"<uuid>","action":"uploads_media.create","entity_type":"uploads_media",
  "entity_id":25,"before_data":null,"after_data":"{\"id\":25,\"title\":\"…\",…}",
  "ip_address":"…","created_at":"…"} } ] }
```

**`GET /admin/room_reservations`** / **`GET /admin/holidays`** — server-owned `created_by_id` + date columns (note `holiday_date`, `reserved_date`):
```json
{ "data":[ {"id":"1","type":"room_reservation","attributes":{
  "room_id":4,"teacher_id":1,"reserved_date":"2026-08-20","time_slot_id":3,
  "purpose":"Test","status":"active","created_by_id":"<uuid>","created_at":"…"},
  "links":{"self":"…/room_reservations/1","room":"…/rooms/4"}} ] }
{ "data":[ {"id":"2","type":"holiday","attributes":{
  "holiday_date":"2026-02-23","name":"…","created_by_id":"<uuid>","created_at":"…"}} ] }
```

**`GET /admin/media_files`** — once sanitized, `status:"clean"`, `servable:true`, and a `links.url` presigned link appears:
```json
{ "data":[ {"id":"25","type":"media_file","attributes":{
  "title":"x.png","status":"clean","detected_mime":"image/png","original_mime":"image/png",
  "file_size":67,"rejection_reason":null,"servable":true,"formatted_size":"67 B","file_type":"image",
  "created_at":"…","updated_at":"…"},"links":{"self":"…/media_files/25","url":"…"}} ] }
```
`status` lifecycle: `quarantined → scanning → clean | rejected`.

**`GET /admin/search?q=…&types[]=disciplines&types[]=kafedras`** — object keyed by requested type, each a JSON:API `data` array:
```json
{ "disciplines": [ … ], "kafedras": [ … ], "teachers": [ … ] }
```

### Portal (public) "current state" projections
**`GET /public/groups/:group_semester_id/now`** — what a group is doing right now:
```json
{ "data":{ "id":"373","type":"group_now","attributes":{
  "resolved_at":"2026-07-15T16:23:35+00:00","day_of_week":3,"week_type_name":"both",
  "time_range":"15:35 - 17:00","has_class":false,"current":[] } } }
```
**`GET /public/rooms/:room_id/occupancy`** — same shape with `type:"room_occupancy"`, `is_occupied`, `current:[]`. When a class is in progress, `current` holds the active event(s).

### Map (GeoJSON)
Map endpoints return a GeoJSON `FeatureCollection`, not the JSON:API envelope:
```json
{ "type":"FeatureCollection", "features":[ … ] }
```
`GET /map/routes/room_to_room` requires `from` and `to` query params (room ids) → `400 {"error":"bad parameters","messages":{"from":["is missing"],…}}` when absent.

## 8. Client checklist
- Store both tokens; refresh proactively before the 900 s access-token expiry; rotate the refresh token on every use.
- Handle the `must_change_password` `403` by routing to a forced password-change screen.
- Drive collection write-buttons from `meta.actions` + role/scope; treat `403` as the source of truth.
- Wrap attributes under the singular resource key on write; never send `id`/timestamps.
- For media, treat upload as async: create → poll status → request presigned URL only when `clean`.
- Follow `links` rather than concatenating URLs.
