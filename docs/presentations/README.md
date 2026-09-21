# Administrator schedule creation workflow

`administrator_schedule_creation_workflow.pptx` is a rebuilt, ten-slide, 16:9 PowerPoint deck for a narrated administrator walkthrough. Every slide is a single full-bleed screenshot from the application; there are no added titles, callouts, or instructional overlays.

The slide order follows the operational path:

1. Select semester, faculty, and group context.
2. Open the teacher workspace and choose the teacher context.
3. Create a workload record manually.
4. Edit the workload record and assign its teacher/resource data.
5. Place and edit the lesson in the schedule grid; activity type is visible in the lesson cards.
6. Open the weekday-transfer workflow.
7. Review room-reservation context before placing a reservation.
8. Open the authenticated `Импорт Excel` entry point for batch data import.
9. Review the administrator notification workspace.
10. Show the resulting group schedule to a student/user.

Screenshot sources are the browser evidence under `docs/design/admin-workbench-2026-09-13/evidence/`, the authenticated Excel-entry capture under `.playwright-mcp/`, and the public group schedule capture under `apps/frontend/qa-schedule-group.png`. The deck contains no credentials, cookies, tokens, or browser session data.

The deck can be regenerated with:

```sh
python3 docs/presentations/build_schedule_presentation.py
```

Validation performed after generation: `unzip -t` completed without errors; all XML and relationship parts parsed successfully; the package contains ten slides, and every slide image occupies the full 16:9 slide extent (`0,0` to `12192000 × 6858000` EMU).
