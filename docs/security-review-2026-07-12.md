# Security & Code Quality Review — `apps/api_geteway`

Date: 2026-07-12. Scope: `apps/api_geteway` (Hanami 2.3 API), plus `apps/excel_processor`
(Python ingestion worker) and `media_sanitizer` where the data flow from `api_geteway`
leads there. Method: static code review + live manual testing against the running dev
stack (`docker-compose`, containers already up) using the provided super_admin account
(`admin@milk-schedule.local`) and accounts created during testing. All findings below are
either a direct code citation or a reproduced live request/response; nothing here is
speculative.

## 1. Stack summary

- **Runtime**: Ruby / Hanami 2.3, Puma (single process, 5 min/max threads by default),
  Sequel/ROM for Postgres 16 (PostGIS + pg_trgm + citext), Redis (refresh tokens,
  Rack::Attack counters, SSE pub/sub), MinIO (S3-compatible object storage via Shrine),
  ClamAV (via `clamby`) for upload scanning.
- **Companion services**: `excel_processor` (Python/pandas, consumes uploaded workbooks
  via Redis pub/sub), `media_sanitizer` (same Ruby codebase, DB-polling relay, not an HTTP
  microservice), nginx (`deploy/nginx/nginx.conf`, proxies `/api/` to a `bff` service).
- **AuthN**: JWT (HS256) access tokens (15 min TTL) + opaque Redis-backed refresh tokens
  (14-day TTL, single-use rotation). **AuthZ**: role + scope (`global`/`faculty`/`kafedra`)
  claims in the JWT, checked by a policy/`ScopeResolver` layer in the `admin` slice.

## 2. Entry points and data flow

| Entry point | Slice/mount | Auth | Sink |
|---|---|---|---|
| Login/refresh/logout/password, account CRUD | `authentication` (`/auth`) | none → JWT issued | Postgres `users`/`role_assignments`(-equivalent), Redis (refresh store) |
| ~80 CRUD + nested routes (teachers, kafedras, schedule_events, rooms, load, recommendations, uploads, search, audit_logs, …) | `admin` (`/admin`) | Bearer JWT + role/scope policy | Postgres (via ROM/Sequel, parameterized), Redis, MinIO |
| SSE push | `admin` (`/admin/stream`) | JWT via **query string** | Redis pub/sub → HTTP chunked stream |
| Read-only schedule/reference lookups | `portal` (`/public`) | none | Postgres (read-only queries) |
| Building/room/floor graph data | `map` (`/map`) | **JWT required in practice** (see §5.6) | Postgres/PostGIS |
| Excel upload → ingestion | `POST /admin/uploads/excel` | JWT, currently super_admin-only (see §3.4) | MinIO → Redis pub/sub → `excel_processor` (pandas) → Postgres |
| Media upload → sanitize → serve | `POST /admin/uploads/media`, `GET .../url` | JWT, currently any admin-tier role (see §3.4) | MinIO (quarantine) → `media_sanitizer` (Ractor entropy scan, ClamAV, image/OLE sanitization) → MinIO (clean) → presigned URL |

Everything written from a request eventually lands in Postgres through the ROM
repository layer (parameterized — no raw SQL interpolation found), or in MinIO through
Shrine, or in Redis. No shell-exec, no eval, no XML/YAML unsafe-deserialization path was
found anywhere in the traced flows.

## 3. Confirmed vulnerabilities

### 3.1 CRITICAL — Cross-scope privilege escalation via FK reassignment on `update` (live-reproduced)

A `kafedra_admin` scoped to kafedra 1 can move a resource they legitimately own into a
kafedra they do **not** own, because authorization on `update` resolves ownership from the
**pre-update** DB row only — the new FK value in the request body is never checked.

**Code**: `slices/admin/authorization/authorizer.rb:109-115` (`resolve_owners`) loads the
current row and computes ownership from it; the body's `kafedra_id`/`faculty_id`/etc. is
never re-validated against the caller's scope. Affected update contracts accept
reassignable FKs: `kafedras.faculty_id`, `teacher_on_kafedras.kafedra_id`,
`curriculum_streams.kafedra_id`, `study_load_requirements.stream_id`,
`teaching_assignments.{load_requirement_id,teacher_on_kafedra_id}`.

**Live reproduction**:
```
# Created a kafedra_admin scoped to kafedra_ids:[1] via POST /auth/accounts
POST /auth/sessions {"email":"kafadmin_v3@test.local","password":"Probe12345!"} → 201, JWT with scopes.kafedra_ids=[1]

# teacher_on_kafedras/1 originally belongs to kafedra 1 (owned)
PATCH /admin/teacher_on_kafedras/1  Authorization: Bearer <kafedra-1-scoped token>
Body: {"teacher_on_kafedra":{"kafedra_id":25}}
→ HTTP 200 — row reassigned to kafedra 25, which this account does NOT own.
```
Reverted immediately after confirmation (`PATCH .../1 {"kafedra_id":1}` → 200) and the two
test accounts (`probe1@test.local`, `kafadmin_v3@test.local`) were deleted. No lasting data
change remains.

**Impact**: any scoped admin can pull another kafedra's teachers/streams/assignments into
their own scope, or exile their own inconvenient rows into a kafedra nobody manages,
without any global-admin approval. This is the same bug class as the previously-fixed
"nested-parent create/id conflation" (see project history), reopened on the update path.

**Fix direction**: `resolve_owners` (or wherever `update`/`destroy` grants authorization)
must also resolve the owner implied by every reassignable FK present in the request body,
and require the caller to own **both** the current and the target owner.

### 3.2 HIGH — Cross-scope **read** is not enforced on nested resources (live-reproduced)

The same kafedra-1-scoped account could read kafedra 25's teacher list:
```
GET /admin/kafedras/25/teachers   Authorization: Bearer <kafedra-1-scoped token>
→ HTTP 200 (expected 403)
```
Combined with §3.1, and consistent with the code-level finding that `Policy`'s
default-deny (`GLOBAL` tier) only governs actions listed in `WRITE_MATRIX` — reads under
nested routes don't appear to run through the same scope check as writes. This needs a
decision: if cross-kafedra read is intentional (staff-wide visibility), it should be
documented and the write-side gap (§3.1) becomes the only fix needed; if not, nested
`show`/`index` actions need the same scope check as writes.

### 3.3 HIGH — `/admin/stream` JWT-in-query-string is not actually excluded from access logs

`slices/admin/actions/stream/show.rb` authenticates via `?access_token=<jwt>` (SSE can't
set an Authorization header) with a code comment claiming the token is "excluded from
access logs (nginx config)". Checked `deploy/nginx/nginx.conf` in full: there is no
`access_log off` and no custom `log_format` scrubbing the query string anywhere — default
nginx access logging is active and will persist the full URL, including the live JWT, to
`/var/log/nginx/access.log`. The app's own `AccessLogger` does strip
`access_token`/`refresh_token`/etc. from its JSON log (`SENSITIVE_PARAMS`,
`access_logger.rb:23`), but that's a different log than the one the comment is actually
relying on. A 15-minute-lived token is a bounded exposure window, but log files routinely
outlive that by months.

### 3.4 MEDIUM — Upload authorization is inverted from what the naming suggests

`Admin::Action`'s auto-classifier (`action.rb:19-23`) maps action class name suffixes
`Create/Update/Destroy` to write actions; anything else defaults to `:read`.
`Uploads::Media` doesn't match that suffix, so it's classified `:read` → gated only by
"is any admin-tier role" (faculty_admin/kafedra_admin/scheduler/super_admin) and — because
`WRITE_ACTIONS` for audit logging is `%i[create update delete]` — **media uploads are
never written to `audit_logs`**. Meanwhile `Uploads::Create` (Excel) is correctly
classified `:create`, falls through to the `GLOBAL` (super_admin-only) default tier since
`"uploads"` isn't in `WRITE_MATRIX`, so today only super_admin can upload Excel files but
any faculty/kafedra admin or scheduler can upload media, unaudited — very likely backwards
from intent, and the missing audit trail on media uploads is a real gap on its own
regardless of who's allowed to do it.

### 3.5 MEDIUM — Excel ingestion has no decompressed-size ceiling (OOM path in `excel_processor`)

`apps/excel_processor/src/infrastructure/storage.py:10-16` reads the whole object into
memory (`response.read()`, no cap); `data_sync_service.py:398` parses the full workbook
with pandas. The only size gate is `EXCEL_MAX_FILE_SIZE` (20 MB, Ruby-side,
`content_validator.rb:17`) — bounding **compressed** upload size only. A small,
highly-compressible OOXML file (repetitive shared-strings XML — a standard zip-bomb
shape) can pass the 20 MB compressed check and expand well past the container's
`memory: 256M` cap (`docker-compose.yml:74-78`), OOM-killing the ingestion worker.
Ingestion is driven by Redis pub/sub with no durable retry queue, so each such file
reliably takes the worker down once processed.

### 3.6 LOW — ClamAV signatures are frozen at image-build time

No `freshclam` call exists anywhere in the Dockerfile, compose file, or entrypoints — the
scanner runs forever against the signature set baked in when the image was built.

### 3.7 LOW — Weak password policy; SSE bypasses the must-change-password gate

`passwords/update_contract.rb:11` — 8-character minimum, no complexity rule beyond
"differs from current password." Separately, `Admin::Actions::Stream::Show` (by necessity,
since `EventSource` can't set a header) only checks role, not the
`must_change_password` gate every other admin action enforces — an account still on its
initial/forced password can open the live event stream.

## 4. Performance / resource-exhaustion findings

### 4.1 `POST /admin/schedule_recommendations` — unbounded-array cost paid before validation (live-reproduced)

The contract (`schedule_recommendations/create_contract.rb`) doesn't even define a
`stream_ids` key, yet sending one costs real server time proportional to (worse than
linear in) its length, before the request fails for unrelated missing required fields:

| Array length | Wall time | HTTP |
|---|---|---|
| 100 | 0.014 s | 422 |
| 5,000 | 0.16 s | 422 |
| 20,000 | 0.86 s | 422 |
| 50,000 | 3.49–5.72 s | 422 |

This endpoint is reachable by the `SCHEDULE` policy tier — global scheduler **and** any
scoped faculty/kafedra admin, not just super_admin (`policy.rb:64`). It has **no
Rack::Attack throttle** (confirmed — `config/app.rb` only throttles
login/refresh/password/public-search) and **no request body size cap** at the Puma or
nginx layer. With Puma's default 5-thread pool, roughly 5 concurrent requests like the
50k-element one above are enough to saturate every request-handling thread in the process
for several seconds each, denying service to every other user — from a single
authenticated, non-privileged account. Needs an array-length cap in the contract (reject
early, not after full parse/coerce) and a throttle on this and similarly expensive
compute endpoints.

### 4.2 Puma thread pool vs. Postgres connection pool — unmatched, unconfigured on both sides

`config/puma.rb` defaults to 5 min/max threads (`HANAMI_MAX_THREADS`/`HANAMI_MIN_THREADS`).
No `max_connections`/pool size is configured anywhere for the Sequel/ROM connection
(`config/database.yaml` only sets the URI) — Sequel's own small default applies. Already
flagged internally as an ops tuning gap (`docs/12-fix-status.md`) for the Puma side only;
the DB-pool side isn't configured at all, which is the half of this equation actually
likely to produce `Sequel::PoolTimeout` under concurrent load before thread starvation
becomes visible.

### 4.3 Single unpooled Redis connection shared across all request threads

`config/providers/redis.rb` registers one `Redis.new(...)` instance for the whole process
(no `ConnectionPool` wrapper found anywhere), used synchronously in the request path for
refresh-token lookups, Rack::Attack counters, and SSE publish. A slow or hung Redis
connection serializes/stalls every Puma thread waiting on that one socket, rather than
just exhausting a bounded pool while other threads proceed.

### 4.4 Ractor/Fiber usage — narrow and correctly scoped, not a request-path concern

`Ractor.new` is used only for one-shot Shannon-entropy scans in the media-sanitization
pipeline (`media/sanitizers/entropy_scanner.rb`), each wrapped in `rescue Ractor::Error`.
The `async` gem is used only inside a bounded background-job dispatcher
(`background/dispatcher.rb`, `Concurrent::ThreadPoolExecutor`, max 10 threads) — not wired
into Puma's own request threads. No Ractor/Fiber misconfiguration was found in the
request-serving path; the earlier premise that this needed dedicated scrutiny didn't pan
out — it's a bounded, non-request-path use, and evaluated as such.

## 5. Logging / auditability gaps

The successful-write audit trail is genuinely solid: `audit_logs` captures
`account_id`/`resource`/`action`/`id`/`ip_address`/before-after snapshots for every create/
update/delete (except media uploads — §3.4), and there's a proper per-request
`x-request-id` propagated into the JSON access log. What's missing, for an incident
responder reconstructing an attack after the fact:

- **Failed logins carry no attempted-identity.** `authenticate.rb` returns
  `Failure(:invalid_credentials)` with no logger call; the only trace is a generic access-log
  line (status 401, IP, path, request_id) — the targeted email is in the POST body, which
  the access logger never inspects. A credential-stuffing burst is visible as "many 401s
  from IP X," not "these N accounts were targeted."
- **403 permission-denials are explicitly excluded from the audit trail**
  (`concerns/authorization.rb` — `audit_write!` returns early when
  `response.status >= 400`). They do appear in the generic access log with
  account_id/role/ip, but not as a first-class security event, so there's no easy query
  for "who has been probing for access they don't have."
- **The general application logger has no confirmed structured (JSON) format** — no
  `config/environments/` override was found; only the dedicated `AccessLogger` sink is
  confirmed JSON. Exception logs (`ErrorLogger`) don't carry the request_id, so a 500 can't
  be joined back to its access-log line by anything but timestamp/path proximity.
- No sensitive data (passwords, Authorization headers, raw bodies) was found leaking into
  any log sink — this part is clean.

## 6. Verified safe (evidence-backed, do not re-flag)

- JWT: HS256, secret from required settings (no hardcoded secret in app code), `alg:none`
  and garbage tokens both correctly rejected live (401).
- Refresh tokens: opaque, Redis-TTL-backed, single-use rotation, revoked server-side on
  logout and on password change (closes stale-token replay).
- `must_change_password` enforced as a global before-hook on every admin/authenticated
  action except password-update/logout (and the one SSE exception, §3.7).
- Timing-safe login (dummy bcrypt comparison on unknown email).
- Account provisioning is genuinely `require_role!("super_admin")`-gated in code, not just
  documented; ownership resolution on **create** correctly re-derives the parent's real
  owner from the DB rather than trusting a client-asserted id (this is the create-side fix
  for the earlier "nested-parent conflation" bug class — §3.1 is the update-side gap that
  reopens it).
- File upload content-type checks are real magic-byte checks, not just declared
  Content-Type: a disguised Windows executable renamed `.xlsx` was rejected as
  `application/x-msdownload`, and a PHP payload renamed `.jpg` was rejected as
  `text/x-php`, both live-tested.
- Path traversal on storage keys is closed (`File.basename` + charset allowlist before
  building the MinIO key); presigned URLs force `attachment` disposition (no
  render-in-browser XSS vector) with 15-minute expiry; IDOR checks exist on file/media URL
  minting (except the dead-role inconsistency in §"minor" below).
- MinIO buckets have anonymous access explicitly disabled at provisioning.
- SQL injection attempts (query params, path segments) produced no errors and no behavior
  change — confirms parameterized queries throughout the tested paths; oversized string
  inputs and negative/absurd pagination params are rejected by contract validation in
  single-digit milliseconds, not processed.
- Rack::Attack is real and reasonably tuned: login throttled both per-IP (10/60s) and
  per-email (5/300s) — confirmed live (this review's own repeated logins tripped the
  per-email throttle mid-session); refresh, password-change, and public-search endpoints
  are each throttled too.
- `/map/*` requires a valid bearer token in practice (401 without one) — more restrictive
  than the design docs implied, not a leak.

## 7. Could not verify — would need more to say anything concrete

- **Production topology**: whether the host port mapping seen in dev compose
  (`api_geteway` on 2300) is also open directly in production, bypassing nginx/BFF entirely
  — needs the production infra/deploy config, not present in this repo.
- **Live schema drift from migrations**: relied on migration files, did not diff against
  `\d` on the running Postgres container.
- **TLS termination and any CDN/WAF-level logging** in front of nginx — out of repo scope.

## 8. Bottom line

The RBAC/authz design is fundamentally sound and mostly self-consistent (default-deny,
real ownership resolution on create, real server-side revocation) — which makes the
update-path gap in §3.1 more notable, not less: it's a narrow, specific regression in an
otherwise-careful system, not evidence of an unprincipled one. The two highest-priority
fixes before this goes anywhere near production traffic are §3.1 (re-check target
ownership on FK-reassigning updates) and §4.1 (cap array-shaped params before validation
and throttle compute-heavy endpoints) — the second one is a single unauthenticated-cost
DoS lever available to any logged-in account today. Logging is good for after successful
writes and bad for the two things an incident responder actually starts with: who got
locked out trying to log in, and who got denied access to something they tried to reach.
