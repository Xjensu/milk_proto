# Schedule-filling implementation handoff

This document is a ready-to-use brief for the agent that will implement and execute the administrator workflow for the Construction Faculty timetable.

## Objective

Populate the first semester of the 2026/2027 academic year for the Construction Faculty through the administrator UI. The source timetable is:

<https://www.bsut.by/images/MainMenuFiles/Obrazovanie/Studentam/dlya_studentov_dnevnoi_fo/semestr/str.pdf>

The product UI is Russian. Implementation notes and this handoff are English.

## Source timetable

The PDF is a one-page Microsoft Excel export titled `РАСПИСАНИЕ НА 1-Й СЕМЕСТР 2026/2027`. It contains a weekly timetable with six day columns (Monday–Saturday), five lesson slots, lesson type, subject, teacher and room.

Visible Construction Faculty groups:

- `СИ-11`
- `СП-11`
- `САм-11`
- `СВм-11`
- `СИ-21`
- `СВм-21`
- `СИ-31`
- `СВм-31`
- `СИ-41`
- `САм-41`
- `СВм-41`
- `СА-51`

The source includes teacher names and academic titles for many cells. When a teacher cannot be identified confidently, use the default teacher representation `ст.преп.` and leave other optional teacher fields at their defaults. Do not invent a person identity or expose a raw database ID to the administrator.

## Required reference data

Before timetable entry, verify that the API and UI contain:

1. Semester `number = 1`, `start_year = 2026`, `end_year = 2027`.
2. Faculty `Строительный факультет`.
3. The twelve groups listed above, attached to the selected semester as group-semester records.
4. Subjects from the PDF.
5. Activity types used by the PDF (`лек`, `ПЗ`/`пр`, `лаб`, and any other actual source value).
6. Time slots matching the source: `08:30–09:55`, `10:10–11:35`, `11:45–13:10`, `14:00–15:25`, `15:35–17:10` (confirm the final end time against the configured catalog).
7. Teachers and teacher-on-department memberships where known.
8. Rooms and buildings where known.

If `/admin/semesters`, `/admin/faculties`, or the faculty group endpoint returns an empty `data` collection, stop the schedule workflow and report the missing seed/backup. Do not create fake faculties, groups or semesters as a workaround.

## Domain identity rules

- Scheduling and workload placement use `student_group_semester_id`.
- Dated weekday-transfer rules use the base `student_group_id`.
- Workload assignment uses `teacher_on_kafedra_id`, not the base teacher person ID.
- Account and reservation people use the base teacher person ID.
- Normalize identifiers once and preserve the same identity through authorization and persistence.

## Required administrator workflow

### 1. Establish context

Open `Расписание` or `Нагрузка`, select the first semester 2026/2027, select `Строительный факультет`, select one or more group-semester records, and commit the context with `Открыть расписание` or the equivalent action.

The committed faculty, semester, selected groups, focus group and period must be represented in the browser URL. Reloading or opening the displayed URL must restore the same context. Search typing must not commit a new context.

### 2. Create or verify workload

For every PDF subject/activity/group combination that needs scheduling:

- select the subject through a searchable, readable selector;
- select the actual activity type;
- select the group-semester;
- enter total hours and hours per event according to the source workload;
- preserve any existing workload records instead of duplicating them;
- save and verify the authoritative workload row after reload.

The workload table must retain its current five columns and meanings: `Кафедра`, `Дисциплина и группа`, `Вид занятия`, `Часы`, `Назначение`.

### 3. Assign teachers and rooms

Open `Назначить` for an incomplete workload row. Search and select the teacher membership and room using visible bounded result lists. Preserve all already selected teachers and rooms when adding another one; the assignment PUT replaces the full collections.

The write contract is:

`PUT /admin/study_load_requirements/:id/assignments`

with:

```json
{
  "assignment": {
    "teacher_on_kafedra_ids": ["..."],
    "room_ids": ["..."]
  }
}
```

At least one teacher is required when rooms are selected. Save only after reviewing the complete retained selection. Verify the Russian success feedback and reload the row.

### 4. Place lessons

In `Расписание`, open the relevant group grid and period. Click an empty cell (or use drag and drop as an enhancement) to open the cell inspector. The click/tap/keyboard path must remain fully capable.

The inspector must show group, period, weekday, date if applicable, time slot, selected workload, subgroup, week type, status, assigned teachers and rooms. Dragging must stage a labelled operation; it must not silently save.

Create the lesson with the existing schedule-event contract. Do not delete and recreate an event to simulate placement or editing. After saving, reload the grid and confirm the lesson in the authoritative response.

### 5. Edit and verify

Open an existing lesson and edit the same record through PATCH. Preserve its event ID and workload relationship. Keep recurring edits, one-date transfers and atomic swaps as distinct operations.

For every edit:

1. capture the original visible lesson;
2. change one meaningful field (day, slot, week type, status, teacher or room subset);
3. save;
4. verify success or a translated validation/conflict error;
5. reload and confirm the authoritative final cell.

On 422, 409, forbidden or recoverable network errors, retain the original record and safe form draft. Never automatically retry an unknown mutation outcome.

## UX requirements for the implementation

- Use searchable visible lists for groups, subjects, teachers, rooms and departments; do not use long raw-ID selects.
- Keep the dark sidebar, Manrope/Unbounded typography, ink/velvet/almond visual identity and existing table schemas.
- Keep all table columns and order; use labelled horizontal scrolling on narrow screens.
- Use one consistent header, field, button, feedback and side-sheet pattern across Schedule and Workload.
- Keep the editor dirty state through search and validation failures.
- Do not store passwords, tokens or generated credentials in drafts, URLs, screenshots or logs.
- Provide equally capable click/tap and keyboard alternatives for drag-and-drop actions.
- Resolve dirty drafts before changing semester, faculty or group context.

## Acceptance checklist

The implementation is complete only when all of the following are evidenced in the running application:

- reference catalogs are populated and selectable;
- all twelve source groups can be opened in the selected semester;
- at least one workload is created or matched without duplication;
- a teacher and room assignment is saved while retaining prior selections;
- at least one PDF lesson is created in the correct group/day/slot;
- the lesson is edited in place and remains after reload;
- a conflict or validation failure keeps the original lesson and draft values;
- copied URL, native reload, Back/Forward and multi-group context preserve scope;
- desktop and 390px viewport checks show no page overflow and preserve table columns;
- browser console has no relevant application errors;
- final evidence includes the source PDF, context, workload, assignment, placement, edit and reload states.

## Current environment blocker

During the last test run, the authenticated panel loaded but the live API returned empty collections for semesters, faculties and Construction Faculty groups. The database counts for `semesters`, `faculties`, `student_groups` and `teachers` were all zero. Restore the reference-data seed or backup before attempting the acceptance workflow.
