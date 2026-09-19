# Two-Day Transfer Grids, Occurrence Editor, Cross-Period Planning

> Completed work log (Phase F, part 3, of the admin-panel epic).

**Goal:** Replace the flat "move a scheduled class" list with two side-by-side day grids that support drag-to-transfer, drag-onto-occupied-to-swap, and click-to-edit one occurrence — and let the schedule builder create classes into any calendar period, not just weekly recurrences.

**Files changed:**
- New: `src/features/admin/schedule/DayScheduleGrid.tsx`, `OccurrenceEditor.tsx`, `teacherOptions.ts`, `dayGrid.test.ts`, `slotEditor.test.ts`
- Changed: `src/features/admin/AdminOperations.tsx`, `AdminOperations.test.tsx`, `src/features/admin/schedule/SlotEditor.tsx`, `src/lib/api.ts`, `src/styles.css`
- Backend: `lib/api_geteway/queries/schedule_grid.rb` (adds `event_teacher_id` to the teacher JSON)

## Day grids

`DayScheduleGrid` is a new component, not a variant of `BuilderGrid`/`WeekGrid`: those are weekday × slot for a whole recurring week, this is one column pinned to a real date. Rows are time slots; each cell is a drop target. Drag-and-drop is native HTML5 with a custom `application/x-day-event` MIME payload, following `SlotEditor`'s `.load-drop-zone` pattern — no DnD library.

Drop handling in `AdminOperations`:
- **free cell** → `POST /schedule_events/:id/transfers` (`moved`, carrying the source room)
- **occupied cell** → `POST /schedule_events/:id/swap`, the atomic endpoint, rather than overwriting

## The part that is easy to get wrong: transfers are not in the grid response

`GET /student_group_semesters/:id/schedule?date=` resolves *recurrence* — weekday plus week-type parity plus one-offs — but knows nothing about `event_transfers`. Rendering it directly would mean a drag produced a `201` and then **no visible change**, because the moved occurrence still comes back on its original date.

So `resolveDay` composes the day actually shown: drop occurrences moved away from this date, mark cancelled ones as cancelled rather than hiding them, and add occurrences moved *into* this date at their new slot and room. It is a pure function with unit tests (`dayGrid.test.ts`). The transfer list is fetched per event of the group (one `GET /schedule_events/:id/transfers` each) — fine at the tens-of-events scale this page works at, but the obvious thing to replace if a group ever carries hundreds.

**Already-moved occurrences are deliberately not draggable.** Moving one again would need its existing transfer *replaced*, and the API offers no atomic way to do that (`event_transfers` has DELETE but no PATCH, and the unique index on `(schedule_event_id, original_date)` rejects a second row). Delete-then-create would leave the occurrence in limbo if the create failed. They are undone from the editor instead ("Отменить перенос"), then re-dragged.

## Occurrence editor

Clicking a card opens an `Inspector` drawer with three sections:

- **Преподаватель** — replace or remove via `event_teachers`. This is honestly labelled as applying to *all* occurrences: `event_teachers` has no per-date variant, so a per-occurrence teacher swap is not representable in the current schema.
- **Аудитория** — replace, scoped either to this date (an `event_transfer` keeping the same date and slot but a new room) or to the whole series (`PATCH /schedule_events/:id`).
- **Отмена** — cancel this date (`transfer_type: "cancelled"`), and undo a transfer when the card is a moved-in one.

**"Remove the room" is not offered**, and the drawer says why: `schedule_events.room_id` is `NOT NULL`, so an occurrence cannot exist without a room — cancelling it is the honest equivalent.

The editor needs the id of the `event_teachers` row to PATCH/DELETE it, and there is no index endpoint for that resource, so the schedule grid's teacher JSON now carries `event_teacher_id` alongside `teacher_on_kafedra_id`.

## Cross-period planning in the builder

`SlotEditor` now loads the group's stored calendar periods and asks which one the class belongs to. `semester`, `lecture_week` and `credit_week` are ordinary teaching weeks and stay weekly recurrences; `exam_session` is the only date-bound one and sends `specific_date` through `from_recommendation`.

The date is chosen from a list rather than a free `<input type="date">`: only dates inside the exam period **that fall on the grid cell's weekday**. The server re-derives `day_of_week` from `specific_date`, so any other date would silently move the class to a different column of the grid the operator just clicked. Periods with no stored dates are labelled "(даты не заданы)" so the link back to the calendar page is obvious.

## Bug found and fixed during verification

`kafedraTeacherOptions` (extracted from `SlotEditor`, where it had lived since an earlier phase) requested `per_page=500`. The admin index contracts cap `per_page` at 100 and answer **422** above it, so the request always threw and the `catch` returned an empty list — meaning **the slot editor's fallback teacher dropdown has been silently empty this whole time**, not just the new editor. Replaced with paging (100/page, capped at 12 pages). Live: 15 options for kafedra 1 where there were previously 0.

## Verification performed

- `npm run lint` (tsc -b) clean; `npm run test` 25 files / 73 tests green (11 new assertions across `dayGrid.test.ts` and `slotEditor.test.ts`).
- Live Playwright run against the docker-compose stack, driving the real UI and reading the network log. Native DnD had to be dispatched manually with a shared `DataTransfer` — Playwright's `dragTo` drives mouse events only and never populates it, so the component's `getData` would come back empty.

| step | UI result | network |
|---|---|---|
| period cards | `["Учебный семестр","Lecture period","Credit period","Exam session"]` | — |
| save exam period | "Calendar period saved." | `POST 200 .../periods` |
| drag onto free cell | left empties, right gains "перенесено с 2026-11-02" | `POST 201 .../22/transfers` |
| undo from editor | left card returns, right back to one card | `DELETE 204 /event_transfers/22` |
| drag onto occupied cell | both cards swap sides | `POST 201 .../22/swap` |
| transfers written | `20: 11-02 → 11-03 slot 2 room 2`, `21: 11-03 → 11-02 slot 1 room 1` | — |
| occurrence editor | sections `["Преподаватель","Аудитория","Отмена"]`, 15 teacher options | — |
| builder exam session | date options `["2027-01-04","2027-01-11","2027-01-18"]` (Mondays inside the period) | — |

All seeded events and transfers were deleted afterwards; `schedule_events` and `event_transfers` were confirmed back to their pre-run state.

## Environment gotcha worth recording

`docker-compose up -d --build api_geteway` also restarts sibling containers, including `milk_schedule_frontend`, which re-binds port 4173 **alongside** the vite dev server. Both listen, connections go to either, and the browser silently gets the old production bundle — which looked exactly like "my changes aren't taking effect". Stop the frontend container again after every api rebuild.
