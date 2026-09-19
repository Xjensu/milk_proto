# One-Off Schedule Events (`specific_date`) + Atomic Occurrence Swap

> Completed work log (Phase F, part 2, of the admin-panel epic).

**Goal:** Give `schedule_events` a way to represent a class that happens on one calendar date (exam sessions) rather than every week, teach the occupancy checks about it, and make swapping two occurrences a single server-side transaction instead of two client calls.

**Files changed:**
- Migrations: `config/db/migrate/20260728120002_add_specific_date_to_schedule_events.rb` (new)
- New: `lib/api_geteway/queries/occurrence_filter.rb`, `slices/admin/actions/schedule_events/swap.rb`, `slices/admin/contracts/event_transfers/swap_contract.rb`
- Changed: `lib/api_geteway/queries/schedule_grid.rb`, `slices/admin/relations/{rooms,student_groups,teachers}.rb`, `slices/admin/repos/{schedule_event,room_reservation,event_transfer}_repo.rb`, `slices/admin/scheduling/create_schedule_event_for_stream.rb`, `slices/admin/scheduling/values/recommendation.rb`, `slices/admin/actions/schedule_events/{create,from_recommendation}.rb`, `slices/admin/actions/{rooms,teachers,student_group_semesters}/schedule.rb`, the schedule contracts, both `schedule_grid_row_serializer.rb`, `schedule_event_serializer.rb`, `config/routes.rb`

## `specific_date`

`NULL` keeps the old meaning (a recurring weekly template keyed on `day_of_week` + `week_type_id`). A set value pins the row to one calendar date. `day_of_week` stays `NOT NULL` and is **re-derived server-side** from `specific_date` on write, in both `Create` and `FromRecommendation` — so every existing query that groups or filters by weekday keeps working untouched, and a client that sends a mismatched `day_of_week` cannot land the class in the wrong grid column.

## Occupancy was the real work

Matching on `day_of_week` alone would make a single exam occupy *every* occurrence of that weekday. `APIGeteway::Queries::OccurrenceFilter.condition` builds the comparison:

- recurring vs recurring — same weekday and same `week_type_id` (unchanged behaviour)
- one-off vs one-off — same `specific_date`
- one-off vs recurring — the recurring row's weekday matches the date's, and its week-type **code** is in `WeekParity.codes_for(date)`
- recurring vs one-off — conservative: any one-off already on that weekday blocks, because a weekly template has no single date to compare against

Applied by `Rooms`/`StudentGroups`/`Teachers#has_occupied?` (all gained an optional `specific_date:`), by `ScheduleEventRepo#any_active_for_room_day_slot?` (gained `date:`), and therefore by `RoomReservationRepo#conflicts?`, which now passes the reservation's own date instead of just its weekday. `ScheduleGrid` gained the equivalent rule for the date-scoped views plus a new `for_group_semester_on_date`.

`Recommendation` (a `Data.define`) gained `specific_date` with a defaulted `initialize`, so the scheduling engine — which only ever builds recurring placements — constructs it unchanged.

### Live evidence the check is genuinely date-aware, not weekday-wide

With a one-off in room 1 / slot 4 on Mon 2026-10-05:

| action | result |
|---|---|
| reserve room 1 slot 4 on **2026-10-05** | `409 room already occupied` |
| reserve room 1 slot 4 on **2026-10-12** (also a Monday) | `201` |
| `GET .../schedule?date=2026-10-05` | the one-off |
| `GET .../schedule?date=2026-10-12` or `2026-10-06` | empty |

And through `from_recommendation` with `specific_date: "2027-01-11"` (a Monday) while sending `day_of_week: 3`: stored `day_of_week` came back as **1**; a repeat POST for the same room/slot/date returned `409 slot_taken`; the same room/slot on `2027-01-18` returned `201`.

## `POST /admin/schedule_events/:id/swap`

Two client-side transfer calls have a real TOCTOU window — `idx_schedule_events_occupancy` is **not unique**, so nothing below the application layer catches a half-applied swap. The endpoint takes `{ swap: { source_date, target_event_id, target_date, reason? } }` and writes both `event_transfers` rows inside one `EventTransferRepo#swap_occurrences` transaction: each event takes over the other's date, time slot and room.

Live-verified atomicity: a swap whose **second** leg duplicates an existing `(schedule_event_id, original_date)` returns 422 and leaves **zero** transfers on the first leg's event — confirmed by reading `GET /schedule_events/:id/transfers` afterwards. A clean swap returns 201 with both rows and the expected mirrored slot/room values.

## Discovered during verification

**The ROM relation schema is inferred at boot, so a migration alone is not enough.** After `docker-compose up -d --build api_geteway` followed by `hanami db migrate`, the column existed in Postgres but the running container had already inferred its schema without it: `specific_date` was silently dropped from writes and the serializer raised `ROM::Struct::MissingAttribute`, surfacing as a 500. A `docker restart api_geteway` after the migration fixed it. Rebuild → migrate → **restart** is the correct order for any migration in this project.

`config/db/structure.sql` is *not* regenerated by `hanami db migrate` in the container despite the "structure dumped" log line — the file's mtime never changes. It is already stale in the repo relative to the July migrations, so it was left alone rather than hand-edited.
