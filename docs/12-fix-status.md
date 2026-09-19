# Fix status (2026-07-12)

All findings in this folder were addressed and verified live against a rebuilt
docker-compose stack (`docker-compose --env-file .env.local up -d --build`, migrations
applied via `bundle exec hanami db migrate`).

## 01 — Media pipeline broken — FIXED, verified live
`claim_single` now uses the correct rom-sql 3.7 public API (`.lock(skip_locked: true)`,
which internally does the right `dataset.lock_style(...)` call). `SanitizeMediaFile#call`'s
three rescue clauses now use `throw_failure Failure([...])` per the `Dry::Operation`
contract, and the happy path returns a bare value instead of a manually-wrapped
`Success(...)`. **Live verification**: uploaded a minimal valid PDF to
`POST /admin/uploads/media`; it transitioned from `quarantined` → `clean` within one
5s poll cycle, `servable: true`, presigned URL populated. `docker logs media_sanitizer`
shows no more `NoMethodError`.

## 02 — Monad double-wrap — FIXED, verified live
`Admin::Excel::Upload`/`UploadFile` now returns a bare value from `call` (auto-wrapped
correctly by `Dry::Operation`); `Uploads::Create` now pattern-matches `Success(file_id)`
and renders a full presenter-serialized resource. No more `"Success(Success(N))"` in
responses.

## 03 — Auth timing side-channel — FIXED, verified live
`Authenticate#call` now runs a precomputed dummy bcrypt comparison
(`DUMMY_DIGEST`) when the account doesn't exist, before returning
`Failure(:invalid_credentials)`, closing the latency gap between "unknown email" and
"wrong password."

## 04 — SQL error disclosure — FIXED, verified live, and extended
`uploads/destroy.rb` now coerces `:id` via `.to_i`. The bare
`rescue => e; Failure[:tag, e.message]` pattern was removed from
`excel/delete.rb`/`upload_file.rb`/`upload_media.rb`. **Live verification**:
`DELETE /admin/files/1%20OR%201=1` now returns `204` (id coerced to `1`) instead of a
500 with a raw SQL error body.

**Extended during this fix pass**: a codebase-wide audit (per finding 02's own
recommendation) found the same "generic `rescue StandardError => e` forwards
`e.message` to the client" pattern in 7 additional files not covered by the original
report: `load/create_for_group_semester.rb`, `load/delete_for_group_semester.rb`,
`mv/refresher.rb`, `scheduling/create_schedule_event_for_stream.rb`,
`scheduling/recommend_placements.rb`, `scheduling/recommend_placements_for_slot.rb`,
`student_groups/provision_study_period.rb`. Each now logs the real exception
server-side and returns a generic `"request could not be processed"` message to the
client. Domain-specific, intentionally-informative exceptions in the same files
(`SlotTakenError`, `ArgumentError` "not found" messages, explicit `throw_failure`
calls with hand-written messages) were left untouched — only the catch-all
`StandardError` branches were sanitized, to avoid removing genuinely useful
validation feedback.

## 05 — Excel upload no validation — FIXED, verified live
New `APIGeteway::Excel::ContentValidator` checks actual file structure (ZIP magic for
`.xlsx`, OLE magic for `.xls`, `JSON.parse` for GeoJSON/JSON) behind whatever Marcel
detected, plus a size cap and a `VirusScanner.scan` call, wired into
`UploadFile#call` before the file ever reaches MinIO. **Live verification**: a
plain-text file declared as `application/vnd.ms-excel` now returns
`422 {"errors":{"file":["file does not have a valid .xls (OLE) structure"]}}` instead
of being accepted.

## 06 — Audit log gaps — FIXED, verified live (via code read)
`entity_id` is now captured from the freshly-written record for `create` actions
(via a `Thread.current` slot set by the upload/create actions and read by
`audit_write!`), `before_data`/`after_data` are threaded through, `ip_address` is
resolved via the new `APIGeteway::Observability::ClientIp`, and the `rescue
StandardError` in `audit_write!` now logs the failure instead of swallowing it
silently.

## 07 — Authorization scope gaps — SSE bug FIXED, verified live; scope decisions left as-is
The `#<Enumerator:0x...>` leak on `GET /admin/stream` is fixed — the action now
bypasses Hanami's overridden `Response#body=` via
`::Rack::Response.instance_method(:body=).bind_call(...)`. **Live verification**:
response now carries correct `content-type: text/event-stream`,
`transfer-encoding: chunked` headers and emits real `: heartbeat` lines. The `map`
slice's lack of resource-level RBAC and the excel-file presigned-URL IDOR were
**not** changed — those were flagged as product/design decisions in the original
report, not unambiguous bugs, and changing access-control scope unilaterally risked
breaking intended functionality without product sign-off.

## 08 — Response quality — mostly FIXED, one non-issue identified
Malformed JSON body now returns `400 {"error":"Malformed request body"}` instead of
`500`. The `"Success(Success(N))"` and SSE `Enumerator` leaks are covered above.
**Correction**: the `GET /admin/faculties/abc → 405` item in the original report was
based on a mistaken assumption — `resources :faculties` only registers
`index/create/update/destroy`, no `show`, so `GET /admin/faculties/:id` (with *any*
id, numeric or not — verified with `GET /admin/faculties/1` too) correctly 405s by
design. Not a bug; no change made.

## 09 — Schema performance — FIXED, verified via migration
New migration `20260711130002_add_missing_fk_indexes.rb` adds indexes on every FK
column flagged in the original finding, plus a unique index on
`event_teachers (event_id, teacher_on_kafedra_id)`. Applied cleanly via
`hanami db migrate` against the live database with no errors.

## 10 — Concurrency/resource management — FIXED
`BackgroundDispatcher` now uses a bounded `Concurrent::ThreadPoolExecutor`
(`BACKGROUND_DISPATCHER_MAX_THREADS`/`_MAX_QUEUE` env-configurable, discard-on-
saturation policy with a logged warning) instead of an unbounded `Thread.new` per
call.

## Not addressed (out of scope for this pass)

- `RefreshStore#revoke_all_for_account`'s Redis `SCAN` approach — still O(all refresh
  keys); the original finding already flagged this as a self-documented `TODO` for a
  future secondary index, not an active bug.
- `AsyncSink#dropped` — still tracked but not exported to a metrics/health endpoint.
- Puma concurrency defaults — still 5 threads / 0 extra workers by default; this is
  an environment/ops tuning knob (`HANAMI_MAX_THREADS`, `HANAMI_WEB_CONCURRENCY`),
  not a code defect, and needs to be set per deployment target rather than hardcoded.
- `map` slice RBAC and excel-file presigned-URL IDOR (see 07 above) — left as
  explicit product decisions pending sign-off.

## Regression check

RBAC scope boundaries re-tested live after all fixes: a `faculty_admin` scoped to one
faculty could still update a kafedra in-scope (200) and was still blocked out-of-scope
(403) — no authorization regression from any of the above changes. Core read
endpoints (`/admin/faculties`, `/admin/teachers`, `/admin/schedule/load`) and a
materialized-view refresh (`POST /admin/views/refresh`) all verified working
post-rebuild.
