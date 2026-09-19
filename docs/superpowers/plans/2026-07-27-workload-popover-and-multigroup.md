# Load List Popover Fix + Multi-Group Load Creation + Partial Assignment Save

> Completed work log (Phase C of the admin-panel epic).

**Goal:** Fix the "+" stream-disclosure layout-shift bug in the workload list, let an admin assign an arbitrary number of groups to one load definition in one form, and let kafedra-load teacher/room assignment save either field independently instead of requiring both together.

**Files changed:** `apps/frontend/src/features/admin/AdminWorkload.tsx`, `apps/frontend/src/styles.css`

## What shipped

1. **Stream popover** (`StreamIndicator`): replaced the inline `<tr className="stream-disclosure-row">` insertion (which pushed every row below it down on toggle, with no transition) with a `position: absolute` popover anchored to the "+" button inside its own cell. Closes on outside click or Escape (`pointerdown`/`keydown` listeners scoped to the popover's open state). Live-verified with before/after row `getBoundingClientRect().top` comparisons: 0px shift once Playwright's own scroll-into-view is controlled for (the first naive measurement showed a 460px "shift" that turned out to be Playwright auto-scrolling the far-down button into view, not a real layout bug — worth remembering next time a before/after position diff looks suspiciously large).
2. **Multi-group workload creation** (`WorkloadRecordInspector`): single `EntityPicker` group field replaced with the same chip multi-select pattern already used by the faculty-workload group filter (`.group-chip-list`) — reused, not reinvented. Submits one `createCurriculumStream` + `createStudyLoadRequirement` pair per selected group (the backend has no atomic multi-group primitive — confirmed during backend research, `curriculum_streams.create` is one `group_semester_id` per call). On partial failure, successfully-created groups are removed from the selection and the drawer stays open with an explanation, so resubmitting only retries the ones that failed (avoids duplicate creation on retry). Verified live: selecting 2 groups and submitting produced exactly `curriculum_streams -> 201, study_load_requirements -> 201` twice.
3. **Kafedra load assignment** (`AssignmentInspector`): teacher and room are independent backend resources (`teaching_assignments` vs `assignment_room_references`), so either can be saved alone - **except** a room reference always points at a `teaching_assignment_id`, and `contracts/teaching_assignments/create_contract.rb` requires `teacher_on_kafedra_id` (DB column is `null: false`, migration `20260309205634`). A brand-new row with only a room and no teacher, ever, has nothing to attach the room to. This is a real schema constraint, not a frontend gap - flagged rather than silently worked around with a migration, since making that column nullable has app-wide ripple effects (schedule-event creation pulls teacher info through the same chain) that deserve their own review.
   - What's fixed: the submit button is no longer permanently disabled unless a teacher is picked. Any single field (teacher OR room) now saves independently whenever there's something to attach it to.
   - What's now honest instead of silently blocked: submitting room-only on a row with no existing assignment shows *"Чтобы сохранить аудиторию для новой записи, сначала укажите преподавателя — аудитория привязывается к его назначению"* instead of leaving a disabled button with no explanation.
   - Verified live: room-only submit on a fresh row → 0 network calls, explanation shown. Teacher-only submit on a fresh row → `POST teaching_assignments -> 201`, no room call.

## Verification performed

- `npm run lint` (tsc -b) clean, `npm run test` (vitest): 23 files / 63 tests green.
- Live Playwright verification against the docker-compose stack for all three behaviors above, including a real end-to-end multi-group submission and a real teacher-only assignment save.
