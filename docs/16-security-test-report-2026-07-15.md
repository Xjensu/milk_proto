# API Gateway — Security & Code-Review Report

**Target:** `apps/api_geteway` (Hanami 2.3 scheduling API)
**Date:** 2026-07-15
**Method:** Static code review of the auth / RBAC / persistence / upload layers + live black-box and grey-box testing against the running stack (`http://localhost:2300`, Docker: api_geteway, postgres, redis, minio, media_sanitizer).
**Auth used:** seeded `super_admin` (`admin@milk-schedule.local`) plus a purpose-created scoped `faculty_admin` for RBAC boundary tests. All test data was cleaned up afterward.

---

## 1. System overview

Automated timetable-management API for a university (faculties → kafedras → teachers, curriculum streams → study load → teaching assignments → schedule events, rooms/buildings with a PostGIS routing layer).

| Layer | Technology |
|-------|-----------|
| Framework | Hanami 2.3 (slices: `authentication`, `admin`, `portal`, `map`) |
| Persistence | rom-sql 3.7 + Sequel 5 on PostgreSQL + PostGIS |
| Cache / tokens / rate-limit / SSE | Redis 5 |
| Object storage | MinIO (S3) via Shrine |
| AuthN | JWT (HS256) access tokens + opaque Redis-backed refresh tokens |
| AuthZ | Capability matrix + scope resolver (faculty/kafedra ownership chains) |
| Async | `async` fibers + `concurrent-ruby` thread pool (BackgroundDispatcher) |
| Media | marcel (magic-byte MIME) + ruby-vips (re-encode) + mini_exiftool + clamby (ClamAV) |
| Middleware | rack-cors, rack-attack, JSON body parser, structured access/error logging |

### Entry points & data-flow map

```
Client ─► Rack::Cors ─► ErrorLogger ─► Rack::Attack (Redis counters) ─► AccessLogger ─► body_parser(JSON)
       ─► Hanami router ─► Slice::Action
             │  before: authenticate! (JWT → CurrentAccount, thread-local)
             │  before: password-change gate (JWT claim)
             │  before: authorize! (capability matrix + ScopeResolver → DB owner lookup)
             │  handle: contract.call(params) ─► repo (rom-sql/Sequel, parameterized) ─► Postgres
             │  after:  audit_write! (audit_logs + Redis SSE publish)
             └► serializer (jsonapi-serializer) ─► JSON response

Sinks reached by user input:  Postgres (Sequel, parameterized) · MinIO (Shrine) · Redis · SSE stream · structured logs
No shell exec, no template/HTML render, no outbound HTTP on the request path, no deserialization of user objects.
```

Because there is no SQL string interpolation, no shell, and no HTML rendering on the request path, the classic injection/XSS/RCE surface is small. The meaningful risk concentrates in **mass assignment at the persistence boundary**, **upload handling**, and **operational logging/observability**.

---

## 2. Findings

Severity is relative to a production high-load deployment.

### Resolution status (updated 2026-07-15, verified live after rebuild)

| # | Finding | Status |
|---|---------|--------|
| F-1 | Mass assignment / client-controlled PKs | ✅ **Fixed** — verified |
| F-2 | Virus scan disabled & fails open | ✅ **Code fixed** (fail-closed by default in production) · ⚠️ deployment must provide a working ClamAV |
| F-3 | Client IP unattributable behind proxy | ⚠️ **Open — deployment config** (`TRUSTED_PROXIES`) |
| F-4 | Background job failures bypass structured logging | ✅ **Fixed** |
| F-5 | Fiber reactor overhead with no benefit | ◻️ **Deferred by design** (Phase-5 async driver swap) |
| F-6 | Refresh mass-revocation (SCAN) + rate-limit counter race | ✅ **Fixed** (both) |
| F-7 | List includes load full reference tables | ◻️ **Info — no change** |
| F-8 | `PATCH /admin/academic_titles/:id` always 500 | ✅ **Fixed** — verified (found during the CRUD refactor) |

### F-1 · HIGH — Mass assignment: client controls primary keys and timestamps

**Where:** every create/update action following the pattern `repo.create(request.params[:resource])` — confirmed in ≥13 create actions (`faculties`, `kafedras`→via same base, `subjects`, `semesters`, `posts`, `student_groups`, `teachers`, `week_types`, `activity_types`, `building_types`, `academic_titles`, `academic_degrees`, `aviability_types`, `event_teachers`) and the update actions (e.g. `faculties/update.rb` → `request.params[:faculty]`).

**Root cause:** the action validates the contract against `request.params.to_h`, but then persists the **raw** `request.params[:resource]` — not the contract's sanitized output. The contract *checks* the input; it never *filters* it. rom-sql inserts every key that maps to a real column.

**Evidence (live):**
```
POST /admin/faculties  {"faculty":{"name":"MassTest","short_name":"MASS","id":999999,"created_at":"1970-01-01"}}
→ 201; response id = "999999"
DB log: INSERT INTO "faculties" ("name","short_name","id") VALUES ('MassTest','MASS',999999)
Postgres: row exists at id=999999 while faculties_id_seq.last_value = 95   ← sequence desynced
```
`id` is `integer` (int4). A client can set it to any value.

**Impact:**
- **Sequence-collision DoS:** set `id` to the sequence's next value (e.g. 96); the next server-generated insert collides on the PK → 500/`23505`. Repeatable → write-path denial of service on any core table.
- **Audit forgery:** `created_at` / `updated_at` are client-settable, corrupting the timeline the audit trail depends on.
- **Integrity corruption:** arbitrary PKs break assumptions elsewhere (FK targeting, pagination, ID-range logic).

**Fix applied (✅ verified live):** centralized a `writable` scrubber in `slices/admin/db/repo.rb` that strips `PROTECTED_WRITE_ATTRS = %i[id created_at updated_at]` from every payload, applied inside `create_with_associations` / `update_with_associations`. All bare-`create` call sites were routed through the scrub (converted to `create_with_associations`, or given an inline `repo.writable(...)` where the action needs the lightweight struct — `student_groups`, `teachers`, `schedule_events`, `holidays`, `room_reservations`, `weekday_transfers`, `time_slots`). Post-rebuild test: `POST /admin/faculties {…,"id":777777}` now returns the next sequence id (777777 ignored) across every write path; contracts still whitelist the domain fields, the repo guards the server-owned ones.

---

### F-2 · MEDIUM — Virus scanning is silently disabled and fails open in the running stack

**Where:** `lib/api_geteway/media/sanitizers/virus_scanner.rb`; deployment config.

**Observed (live upload of a text file renamed `.png`):**
```
[SanitizeMediaFile] starting id=24 mime=image/png attempt=1
VirusScanner: ClamAV not found — skipping scan for media_sanitize_...png   ← AV skipped
... later rejected by vips re-encode ("not a known file format")
```
`clamscan` **is** present in the container (`/usr/bin/clamscan`), but `Clamby.clamscan?` returns false (missing virus DB / clamd expectation), and `MEDIA_REQUIRE_VIRUS_SCAN` is unset — so the scanner returns `{ clean: true, reason: "scan skipped" }`. The code has a correct fail-closed mode, but it is **off**.

**Impact:** the vips re-encode still blocks non-image content, but a *valid* image / PDF / DOCX carrying malware would be accepted, since AV never runs. In production this is the difference between "quarantine caught it" and "we served malware from `media-clean`".

**Fix applied (✅ code, ⚠️ deployment):** `VirusScanner::REQUIRE_SCAN` now defaults to **fail-closed in production** — required when `MEDIA_REQUIRE_VIRUS_SCAN=true`, explicitly overridable with `=false`, and when unset it is required if `HANAMI_ENV=production` (dev/test stay fail-open for local convenience). **Deployment still owes** a working ClamAV (clamd socket + `freshclam` DB, or a working `clamscan` + definitions) so `Clamby.clamscan?` returns true; otherwise uploads in production will correctly but universally fail as retryable. Recommend a boot health check that surfaces AV availability.

---

### F-3 · MEDIUM (operational) — Client IP is unattributable behind a proxy as currently configured

**Where:** `lib/api_geteway/observability/client_ip.rb`; `TRUSTED_PROXIES` env.

Every audit and access-log entry recorded during testing shows `172.18.0.1` (the Docker bridge). The resolver code is correct — it walks `X-Forwarded-For` skipping trusted hops — but `TRUSTED_PROXIES` is empty, so it falls back to `REMOTE_ADDR`. In production behind a load balancer this means **every logged action, including `permission_denied` security events, is attributed to the LB's IP**, defeating the stated goal of tracing an intruder.

**Fix:** set `TRUSTED_PROXIES` to the proxy/LB CIDR(s) and ensure the proxy sets `X-Forwarded-For`. Add a boot-time warning when the app is in production with an empty `TRUSTED_PROXIES`.

---

### F-4 · LOW — Background job failures bypass structured logging

**Where:** `lib/api_geteway/background/dispatcher.rb:51`.

```ruby
rescue StandardError => e
  warn "[BackgroundDispatcher] job failed — ..."   # STDERR, not the JSON logger
```
Sanitize/outbox job exceptions are written to STDERR via `warn`, not the injected structured logger. In a JSON-log/alerting pipeline these failures are invisible. The durable relay is the recovery backstop, but you lose the *signal* that jobs are failing.

**Fix applied (✅):** the rescue now routes through a `_log_job_failure` helper that uses the injected `@logger` when present (structured), falling back to `warn` only when no logger is wired — mirroring the existing `_log_saturation` fallback. `dispatch` return semantics unchanged.

---

### F-5 · LOW — Fiber reactor currently adds overhead with no benefit

**Where:** `dispatcher.rb` — `@pool.post { Async { job.call } }`.

Every dispatched job spins up an `Async` reactor inside a worker thread, but the I/O it performs (Sequel/pg, redis-rb 5.x) is **synchronous/blocking and not fiber-aware yet**. So the reactor yields nothing today — it is pure per-job setup cost. The code comments correctly mark async-pg/async-redis as "Phase 5". Not a bug, but the "fiber-aware" design is aspirational until the blocking drivers are swapped; today it is a bounded thread pool with extra allocation.

**Fix / note:** either defer the `Async{}` wrapper until an async-aware driver is actually in use, or proceed with the Phase-5 driver swap so the reactor earns its cost.

---

### F-6 · LOW — Refresh-token mass revocation and rate-limit counter edge cases

- `RefreshStore#revoke_all_for_account` did `SCAN` + `GET`-per-key over all refresh tokens (O(N) across the whole keyspace) on every password change. **Fixed (✅):** a secondary index set `refresh_set:<account_id>` now tracks each account's active token keys; `issue`/`revoke`/`rotate` maintain it (SADD/SREM + self-cleaning EXPIRE) and `revoke_all_for_account` reads it via `SMEMBERS` and deletes in one pipelined round-trip — no SCAN. Verified live (post-password-change, prior refresh token → 401).
- `RackAttackStore#increment` set `EXPIRE` only on the first increment; a crash between `INCRBY` and `EXPIRE` could leave a non-expiring counter that permanently throttles an IP/email. **Fixed (✅):** increment + conditional expire now run atomically in a single Lua `EVAL`. Verified live (throttle still trips 429).

---

### F-7 · INFO — List includes load full reference tables

The `teachers` index eager-loads `posts`, `academic_titles`, `academic_degrees`, `faculties`, `kafedras` with **no `WHERE … IN (page ids)`** filter — the entire lookup table per association. Negligible while those tables are small; revisit if any reference table grows large. (No N+1: rom-sql issues one batched query per association regardless of page size — verified.)

---

### F-8 · MEDIUM — `PATCH /admin/academic_titles/:id` returned 500 unconditionally

**Where:** `slices/admin/contracts/academic_titles/update_contract.rb`.

Found while round-trip-testing every CRUD resource after the action refactor (the original audit never exercised this specific update). The file `update_contract.rb` declared a class named **`DestroyContract`** — a copy-paste error. Zeitwerk resolves `contracts.academic_titles.update_contract` by loading that file, which re-opened the already-defined `DestroyContract` and called `params` a second time:

```
Dry::Validation::DuplicateSchemaError: Schema has already been defined
→ PATCH /admin/academic_titles/:id  ⇒  500 (every call, since inception)
```

**Fix applied (✅ verified):** renamed the class to `UpdateContract` to match its filename. `PATCH` now returns `200` on success and `404` for a missing row.

**Note (not a bug):** a similar filename/class mismatch exists at `contracts/student_groups/student_groups_contract.rb` (declares `ByFacultyContract`). It is unreferenced dead code and does **not** break production boot — verified by running a full `HANAMI_ENV=production` eager load, which completes cleanly. Left untouched.

---

## 3. What was tested and held up (verified strengths)

These were probed and behaved correctly — worth recording so the review is balanced.

| Area | Test | Result |
|------|------|--------|
| JWT | `alg:none` forged token | Rejected `400` (explicit algorithm on verify) |
| JWT | garbage / missing Bearer | `401` |
| Login | wrong password vs unknown user | Identical `{"error":"invalid email or password"}` — no user enumeration |
| Rate limiting | login/refresh/password/search/uploads throttles | Present, **Redis-backed & pooled** (shared across workers) |
| Security headers | login response | CSP (`default-src 'none'`…), `X-Frame-Options: DENY`, `X-Content-Type-Options: nosniff`, `X-XSS-Protection` |
| CORS | request from `evil.com` origin | No `Access-Control-Allow-Origin` echoed; credentials disabled |
| RBAC read | faculty_admin reading other faculties | `200` (deliberate staff-wide read visibility) |
| RBAC write | faculty_admin creating kafedra **in** scope | `201` |
| RBAC write | faculty_admin creating kafedra **out** of scope | `403` |
| RBAC write | faculty_admin editing GLOBAL-tier (`faculties`, `holidays`, accounts) | `403` |
| RBAC reassign | moving a kafedra into another faculty | `403` (reassignment guard) |
| Password gate | any endpoint while `must_change_password` | `403` until password changed; fresh token pair on change |
| SQL injection | `q=' OR 1=1--` in trigram search | Empty result, parameterized (`Sequel.lit("… ?", q)`) |
| Pagination abuse | `per_page=1000000` / `-1` / `abc`, `page=-5` | `422` (bounded & type-checked) |
| Type confusion | `name` as object/array | `422 must be a string` |
| Malformed body | broken JSON / 200-level nested JSON | `400 Malformed request body` (depth-limited) |
| Bad id | huge/overflow/string ids | `422` generic message, **no stack trace leak** |
| Upload | oversize (60 MB > 50 MB) | `422` before scan |
| Upload | text disguised as PNG | Quarantined → async vips re-encode → **`rejected`**, never servable |
| Upload | valid PNG | `201 status:quarantined servable:false` → sanitized asynchronously |
| Audit | every write | Row in `audit_logs` with actor, resource, id, IP, before/after JSON |

---

## 4. Test artifacts

- Injected/created rows (faculties 94/95/999999, kafedra 107, `fadmin_test@x.local` account) were deleted via the API after testing (`204`). Media rows 23/24 remain as `rejected/quarantined` (harmless, not servable).
- No destructive action was taken against pre-existing data.

---

*See `17-api-frontend-reference-2026-07-15.md` for the endpoint/response contract intended for frontend consumption.*
