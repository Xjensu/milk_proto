# Production Readiness Roadmap — `apps/api_geteway`

Date: 2026-07-12. Builds directly on `docs/security-review-2026-07-12.md` (referenced
below by section, e.g. "§3.1") and `docs/api-reference-2026-07-12.md`. Organized as
priority tiers, not a chronological plan — P0 items block any production traffic, P1
blocks real/multi-tenant load, P2 hardens before scale, P3 is infra/ops, P4 is cleanup
that doesn't affect safety but reduces future risk.

## P0 — Blockers (fix before any production traffic)

### 1. Fix cross-scope FK reassignment on `update` (security review §3.1)
Authorization on `update`/`destroy` currently resolves ownership from the row's
**pre-update** state only. Any reassignable FK in the request body (`kafedra_id`,
`faculty_id`, `stream_id`, `load_requirement_id`, `teacher_on_kafedra_id`, etc.) must
independently resolve its **target** owner and require the caller to hold both the
current and target scope. This is a live-confirmed privilege escalation, not a
theoretical one — do not ship multi-tenant scoped roles (`faculty_admin`/`kafedra_admin`)
into production until this lands.
- Fix in `slices/admin/authorization/authorizer.rb#resolve_owners`, mirroring the
  already-correct create-path pattern (`ScopeResolver#faculty_owner_for_speciality` and
  siblings).
- Add a regression test per affected resource: `teacher_on_kafedras`, `kafedras`,
  `curriculum_streams`, `study_load_requirements`, `teaching_assignments`.

### 2. Cap and throttle compute-heavy endpoints (security review §4.1)
`POST /admin/schedule_recommendations` costs 13ms→3.5s scaling with an array field that
isn't even in its schema, with no Rack::Attack rule and a 5-thread Puma pool. This is an
authenticated, non-privileged DoS lever today.
- Add strict array-length caps in every contract that accepts an array (reject at
  schema-validation entry, not after full coercion).
- Add Rack::Attack throttles for `schedule_recommendations`, `uploads/*`, and any other
  endpoint doing non-trivial DB/compute work per request — the current throttle list only
  covers login/refresh/password/public-search.
- Audit every other array-accepting contract (`student_group_semesters#load` accepts
  nested arrays of arrays — same risk shape) for the same unbounded-cost pattern.

### 3. Decide and enforce read-side scoping (security review §3.2)
Cross-kafedra **reads** on nested routes (`/admin/kafedras/:id/teachers` etc.) are not
scope-checked — confirmed live. Either this is intentional (staff-wide read visibility)
or it isn't. Make the call explicitly, document it, and if not intentional, apply the
same scope check reads currently skip. Shipping without an explicit decision here means
shipping an accidental information-disclosure surface.

## P1 — High priority (before real/multi-tenant load)

### 4. Stop leaking the SSE JWT into the reverse-proxy access log (§3.3)
`deploy/nginx/nginx.conf` has no `access_log off`/custom `log_format` for the `/admin/stream`
path, despite a code comment assuming that mitigation exists. Add a location block with
query-string scrubbing (or move to a short-lived signed ticket instead of the full JWT).

### 5. Fix the upload authorization inversion (§3.4)
`Uploads::Media` is misclassified as a `:read` action by the `Create/Update/Destroy`
suffix-matching classifier, so it skips both the write-policy tier and audit logging.
Today: any admin-tier role can upload media (unaudited), but only super_admin can upload
Excel. Decide the intended tier for each, then either rename the action class to match the
classifier convention or add an explicit `authorize_as` override — and make sure media
uploads land in `audit_logs` like every other write.

### 6. Log security events as first-class, not just generic access-log lines (§5)
Failed logins carry no attempted-identity anywhere in the logs (only "some IP got a 401");
403 permission-denials are explicitly excluded from `audit_logs` by the
`response.status >= 400` guard in `audit_write!`. Before production, add:
- A structured "auth_failure" log/event including attempted email (never password),
  IP, and request_id.
- A structured "permission_denied" event including account_id, role, resource, action,
  IP, and request_id (separate from — or alongside — the existing 400+ audit exclusion).
- Propagate `request_id` into the general application logger (`ErrorLogger`) so a 500 can
  be joined back to its access-log line without timestamp-guessing.

### 7. Size the DB and Redis connection pools intentionally (§4.2, §4.3)
No `max_connections` is configured for the Sequel/ROM pool anywhere (Sequel's small
default applies); Redis runs through a single unpooled client shared by every Puma
thread. Before increasing Puma thread/worker counts for real load: explicitly configure
the DB pool size to be ≥ Puma's thread count (per process, times worker count), and wrap
Redis in a connection pool (or confirm the redis-rb v5 client's own internal pooling
story) with an explicit timeout so one slow Redis call can't stall every thread.

## P2 — Medium (harden before scaling further)

### 8. Cap decompressed Excel size in `excel_processor` (§3.5)
`storage.py` reads the whole MinIO object into memory with no cap; pandas then parses the
full workbook. The only size gate today is on *compressed* upload size (20MB), which a
zip-bomb-shaped OOXML file trivially passes while expanding past the container's 256MB
memory limit. Add a decompressed-size/row-count ceiling before `pd.ExcelFile.parse`, and a
durable retry/dead-letter path so one bad file doesn't just drop silently when the worker
OOMs (current ingestion is fire-and-forget over Redis pub/sub with no queue).

### 9. Refresh ClamAV signatures on a schedule (§3.6)
No `freshclam` call exists anywhere — signatures are frozen at image build time. Add a
scheduled `freshclam` run (cron sidecar, or rebuild cadence) before relying on this as a
real malware control.

### 10. Strengthen password policy and close the SSE must-change-password gap (§3.7)
8-character minimum with no complexity check is thin for admin/scheduler accounts holding
real scope. Also, `Admin::Actions::Stream::Show` bypasses the `must_change_password` gate
that every other admin action enforces — decide whether that's acceptable (SSE is
read-only) or add a lighter-weight check that doesn't require an Authorization header.

### 11. Confirm structured (JSON) logging for the general application logger (§5c)
Only the dedicated `AccessLogger` sink is confirmed JSON. The general Hanami/dry-logger
instance (used for exceptions and non-audit failures) has no confirmed formatter override
— verify it's JSON in whatever environment production actually runs in, since plain-text
logs are much harder to alert on at scale.

## P3 — Infrastructure / operations (not code changes, but required before go-live)

- **Network topology**: `docker-compose.yml` publishes `api_geteway` on host port 2300
  directly. Confirm production topology puts nginx/BFF in front with no direct path to
  the app port, and that `client_max_body_size`/timeouts are set at that layer (currently
  absent from `deploy/nginx/nginx.conf`).
- **TLS termination**: confirm where TLS actually terminates and that no plaintext hop
  carries the JWT or session data across an untrusted network segment.
- **Puma worker/thread tuning**: move off defaults (`HANAMI_MAX_THREADS=5`, single
  process) deliberately, paired with item 7's connection-pool sizing — this is a joint
  decision, not two independent knobs.
- **Load testing**: run a realistic concurrency test against `schedule_recommendations`,
  `schedule_events` writes, and the SSE endpoints (`/admin/stream` has a documented
  50-connection cap — confirm that's compatible with expected concurrent admin sessions)
  before declaring the system load-tested.
- **Backup/DR**: Postgres backup/restore drill, MinIO bucket replication/versioning
  policy — not assessed in this review, needs its own pass.
- **Observability**: metrics (request latency/error rate per endpoint, DB pool
  utilization, Redis latency) and alerting on the new structured auth-failure/
  permission-denied events from item 6 — logs alone aren't an incident-response system
  without something watching them.
- **Secrets**: `.env.local` files hold real-looking dev secrets on disk (not committed,
  confirmed via `.gitignore` — not a current leak) — confirm production secrets come from
  a proper secrets manager, not an env-file pattern carried over from dev.

## P4 — Cleanup (no safety impact, reduces future risk / confusion)

- Fix the `authorize_as resource:, action: :write` bug — `:write` isn't a real
  `WRITE_MATRIX` key, so `student_groups#add_semester`, `#generate_semesters`,
  `student_group_semesters#load`/`#load_destroy`, `schedule_events#from_recommendation`,
  and `views#refresh` are all silently GLOBAL-only regardless of the scoped-access logic
  written for them. Either that's intended (then rewrite the intent as an explicit
  `require_role!` for clarity) or it's an unintentional lock-out of scoped admins who
  should have access — resolve the ambiguity either way.
- Unify the two disagreeing role-check lists for media file access
  (`media_files/show.rb`/`index.rb` check a nonexistent `"admin"` role string;
  `uploads/url.rb`/`media_files/url.rb` correctly use the real `ADMIN_ROLES` list) — fails
  closed today, but the inconsistency will confuse the next person who touches this code.
- Reconcile the design docs (`docs/03-slice-admin.md`, `docs/04-slice-main.md`) with the
  actual JSON:API response envelope — the docs describe a plain-nested shape that doesn't
  match reality; either update them or mark them superseded by
  `docs/api-reference-2026-07-12.md`.
- Fix the accepted-but-ignored `page`/`per_page` params on `GET /public/faculties`
  (silently unpaginated despite validating those params).
- Remove or wire up dead code flagged by the endpoint-catalog pass: unrouted
  `event_teachers` index/show contracts+serializers, unused
  `assignment_room_references` by_group/by_kafedra/with_teacher variants, the
  `stude_load_requirements` directory typo, `room_types`/`buildings` update actions
  skipping the 404-not-found branch every sibling resource has.

## Suggested sequencing

P0 items are independent of each other and can be parallelized across engineers; none of
them require a schema migration. P1 items 4–6 are also independent and low-risk to land
before P0 is fully done, if capacity allows — they don't touch the authorization core. P2
and P3 should follow, not precede, P0/P1: there's little value tuning thread pools or
scheduling ClamAV updates around an authorization bug that's still exploitable. P4 can be
picked up opportunistically whenever someone is already in the touched files.
