# Administrator workbench implementation status

Date: 2026-09-19

## Delivered behaviour

- Public group and teacher schedules support a selected date, apply dated transfers, and expose a print action for that dated view.
- Teachers can find a currently available room, see slot occupancy, create a temporary reservation, and cancel their own reservation. The API returns readable room and slot context for those reservations.
- Administrator room reservations and schedule-event placement use the same transaction-scoped placement lock. Conflicting concurrent writes return a conflict instead of silently overlapping an occupied room or slot.
- Individual and department notice PDF export is queued as a durable job. The Rails status page reads the same job instead of resubmitting it; the worker renews its lease, saves individual files, and produces one merged batch file.
- The Calendar transfer editor uses an explicit searchable group choice, preserves the global scope option, keeps the five-column transfer table, and now labels every input in Russian.
- Schedule shows five nearby semester choices in its main context rail; the full history opens in the existing searchable sheet and keeps the same Turbo URL context.
- Workload creation and editing use a searchable visible activity-type catalog while retaining the radio-button request identity.
- Calendar now separates day-wide transfers from dated lesson transfers. The latter shows two date grids, stages drag-and-drop moves or atomic swaps for confirmation, and provides a dated time editor when a lesson card is selected.
- The admin workspaces retain the existing dark sidebar and Schedule/Workload composition while sharing labelled choice lists, sheet editors, focused feedback, and table scrolling regions. The Schedule placement workspace deliberately retains its original compact native controls and dense planner geometry.
- The administrator shell provides a keyboard skip link that moves focus past the persistent sidebar to the current workspace and respects reduced-motion preferences.
- Narrow administrator navigation, monitoring, return, and sign-out actions retain at least 44px interaction height.

## Current runtime evidence

- Root runtime revision: `bca6311`.
- Rails runtime revision: `abac8e8` (`Increase narrow admin navigation targets`).
- API runtime revision: `212c763` (`Serialize reservation edits with timetable placement`).
- The rebuilt `schedule_web` container is healthy and `GET /up` returns HTTP 200.
- Rails assets precompiled successfully during the rebuild.
- No new test files were added.

## Evidence boundary

The supplied development database was not modified for verification. At this snapshot it has zero `study_load_requirements`, zero `schedule_events`, and zero `room_reservations`. Therefore the following flows are implemented and source/runtime-loaded, but still require browser verification with disposable scheduling data:

1. Create a workload, retain multiple teacher/room assignments, place a lesson, edit the same lesson, reload, and read it back.
2. Create and edit an administrator reservation, then verify the conflict response retains the editor input.
3. Submit a dated transfer and verify its result in the selected-date public schedule.
4. Queue an individual and a batch notice export, wait for completion, and open the generated private download links.

Firefox automation was not used for this snapshot after the user chose to perform browser testing directly. The remaining work is acceptance evidence on real disposable data, not fixture cleanup or a database reset.
