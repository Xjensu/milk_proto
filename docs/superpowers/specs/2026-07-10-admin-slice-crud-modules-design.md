# Admin slice: audit_logs, room_reservations, holidays, weekday_transfers, event_transfers, student_group_semester_periods

Status: approved. Scope: apps/api_geteway, slice `:admin`.

## Decisions

- No table removal this pass (existing schema is already clean; removal deferred to a future pass with an explicit list).
- `created_by_id`/`user_id` FKs on new tables reference `accounts(id)` (`uuid`), not `users(id)` (`bigint`) — `users` doesn't exist in this codebase, `accounts` is the actor table.
- `audit_logs` is read-only from the API (index/show, `super_admin` only, filterable by `entity_type`/`entity_id`/`user_id`/date range). Writes happen only via `Admin::Events::AuditListener`, which now persists to the table instead of (in addition to) logging, dispatched through the existing `APIGeteway::Background::Dispatcher` (thread + `Async` fiber) so the write never blocks the request thread. Not Ractor: ROM/Sequel connections aren't Ractor-shareable in Ruby 3.4.
- `room_reservations.create` conflict-checks against active `schedule_events` on the same `room_id`/`time_slot_id`/weekday (derived from `reserved_date`). Scoped simplification: no week-type-parity exclusion yet (no date→parity pure function exists in the scheduling code today) — conservative, blocks on any matching active recurring event regardless of parity. `destroy` is a soft delete (`status: cancelled`).
- `holidays`, `weekday_transfers`: create/index/hard-destroy only, no update.
- `event_transfers`: nested under `schedule_events` (`POST/GET /admin/schedule_events/:id/transfers`), plus `DELETE /admin/event_transfers/:id` (hard delete = revert).
- `student_group_semester_periods`: nested under `student_group_semesters` (`GET/POST/PATCH .../periods`), repo upserts by `(student_group_semester_id, period_kind)`.
- Out of scope: `DayExpansion` read-path rebuild, week-parity pure function, recommendation-engine changes (separate follow-up, tracked as its own item — see `03-slice-admin.md §6` for the existing engine to be documented/disabled later).

## File layout (per module, following `teacher_time_constraints`/`room_avialabilities` conventions)

`config/db/migrate/<ts>_<table>.rb`, `slices/admin/relations/<table>.rb`, `slices/admin/repos/<name>_repo.rb` (`Admin::DB::Repo` + `commands`), `slices/admin/contracts/<name>/{create,update?,destroy,index}_contract.rb` (`Dry::Validation::Contract`), `slices/admin/actions/<name>/{index,create,update?,destroy}.rb` (`Admin::Action`), `slices/admin/presenters/<name>_presenter.rb`, `slices/admin/serializers/<name>_serializer.rb`, routes added to the `:admin` block in `config/routes.rb`.
