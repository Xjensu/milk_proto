# Public Period Exposure — Design

Date: 2026-08-08
Scope: `apps/api_geteway` (separate nested git repo)

## Goal

Expose a small, already-computed, public-safe slice of period data
(`period_kind`: `semester` / `lecture_week` / `credit_week` / `exam_session`)
through the public portal API, so the frontend redesign (see
`2026-08-08-schedule-home-redesign-design.md`) can build a real "which part of
the semester am I looking at" control instead of a client-side fake. This is a
prerequisite for that spec's group-schedule "schedule-type" step; it ships
first, on its own.

## Out of scope

- Any write path (`upsert`/admin period management) — untouched.
- Exposing anything beyond `period_kind` + the period's own `start_date`/
  `end_date` — no internal ids beyond the already-public `group_semester_id`,
  no teacher/room linkage (there isn't any on this table).
- Changing how `SemesterResolver` computes periods internally.

## Background

`student_group_semester_periods` (migration `20260710120006`, widened
`20260728120001`) stores one row per `(student_group_semester_id,
period_kind)`: `period_kind` (check-constrained to `semester` /
`lecture_week` / `credit_week` / `exam_session`), `start_date`, `end_date`.

`APIGeteway::Queries::SemesterResolver` already reads this table:
- `resolve_group_semester(group_semester_id:, date:)` reads the `semester`
  row to build the `SemesterContext` used in every public schedule response
  today (no `period_kind` on it — it's always the `'semester'` row).
- `special_periods_for(group_semester_id:, from:, to:)` returns `{Date =>
  period_kind}` for the three special kinds, and `schedule_occurrences.rb`
  already uses it internally to pick week-parity codes per date. The active
  period_kind for any date is therefore already known server-side — it's
  just never serialized out.
- `periods_for(group_semester_id:)` returns every configured period for a
  group-semester, already in the exact shape needed.

`Admin::Repos::StudentGroupSemesterPeriodRepo#for_group_semester` is the
existing admin-only read path with the same data; the read logic isn't
duplicated, it's reused.

## Changes

### 1. `period_kind` on the existing semester object

`Portal::Serializers::OccurrenceWindowSerializer#semester_hash` (used by
group/teacher/kafedra schedule responses' `meta.semester`) gains one field:

```ruby
def semester_hash(ctx)
  {
    group_semester_id: ctx.group_semester_id,
    semester_id: ctx.semester_id,
    number: ctx.semester_number,
    label: ctx.label,
    start_date: ctx.start_date.to_s,
    end_date: ctx.end_date.to_s,
    period_kind: ctx.period_kind   # new
  }
end
```

`SemesterContext` (the resolver's struct) gains a `period_kind` attribute,
populated by looking up `special_periods_for` at the window's anchor date and
falling back to `"semester"` when no special period is active that day.
Same treatment for the `semesters` array on the teacher endpoint (one entry
per group the teacher touches — each gets its own resolved `period_kind`).

Frontend `SemesterContext` type gains `period_kind: PeriodKind` to match.

### 2. New public periods endpoint

`GET /public/groups/:group_semester_id/periods` → new
`Portal::Actions::Groups::PeriodsIndex`, reusing
`SemesterResolver#periods_for` (or the admin repo's `for_group_semester` —
whichever is already reachable from the portal slice without a cross-slice
import; if neither is, add a thin `Portal::Repos::GroupPeriodRepo` that reads
the same relation read-only). Response:

```jsonc
{
  "group_semester_id": 513,
  "periods": [
    { "period_kind": "semester",      "start_date": "2026-09-01", "end_date": "2027-01-25" },
    { "period_kind": "lecture_week",  "start_date": "2026-09-01", "end_date": "2026-12-14" },
    { "period_kind": "credit_week",   "start_date": "2026-12-15", "end_date": "2026-12-21" },
    { "period_kind": "exam_session",  "start_date": "2026-12-22", "end_date": "2027-01-25" }
  ]
}
```

No auth required (same trust level as the rest of `/public`). Ordered by
`start_date`, matching `periods_for`'s existing order.

## Frontend consumption note (informative only — implemented in Spec B)

The group-schedule page will fetch this once a group is selected, render one
button per returned period (label via the existing `PERIOD_LABELS` map,
already in `apps/frontend/src/lib/api.ts`), and clicking one re-requests the
schedule with `date` set to that period's `start_date` — reusing the existing
`date` query param and window-resolution logic unchanged. No new schedule
endpoint needed.

## Testing

- Unit: `SemesterResolver` returns the correct `period_kind` for dates inside
  each special period and falls back to `"semester"` outside all of them.
- Request spec: `GET /public/groups/:id/periods` returns all configured
  periods for a group-semester with no auth header; 404 (or empty array —
  pick whichever matches existing portal conventions for an unknown id) for
  a nonexistent group-semester.
- Request spec: existing group/teacher schedule response specs updated to
  assert `period_kind` is present and correct for a date known to fall in a
  special period.
