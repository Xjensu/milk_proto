# Administrator UX direction — 2026-09-23

This package records the design direction and the implemented two-date transfer slice. The generated examples are illustrative and do not define application data. The existing [workbench plan](../admin-workbench-2026-09-13/IMPLEMENTATION_PLAN.md), [design system](../admin-workbench-2026-09-13/DESIGN_SYSTEM.md), and [browser audit](../admin-workbench-2026-09-13/BROWSER_AUDIT.md) remain the acceptance contract.

## What works in Teachers

Teachers uses a visible search, readable result labels, explicit selection, and a separate creation drawer. These choices make a large catalog easier to scan than a long select. Preserve its search and constraints workflow. The older screenshot's 64px heading and oversized black search block are presentation exceptions, not dimensions to copy across the admin.

## Main UX problems

1. Schedule and Workload make operators re-scan large context selectors before reaching the table. Show the committed faculty, semester, and group or department in one compact summary. Put changing context in a bounded, searchable panel. The committed URL must survive reload and Back/Forward.
2. A schedule grid, workload source, and inspector compete for width. The timetable should get the available width; source and editor are separate, collapsible tasks. A lesson edit should open a labelled editor with a clear Save/Cancel boundary and retain the original record on failure.
3. Resources and Accounts mix list browsing with permanently visible creation or row-bound editing. Put one create action beside the list toolbar and use the same side sheet for create and edit. Keep table columns and order unchanged.
4. Calendar has distinct period, transfer, holiday, semester, and reservation processes. Give each its own focused screen composition inside the shared shell; keep the actual five-field transfer alignment and the existing period semantics.
5. Control sizes, heading hierarchy, and narrow navigation vary by page. Apply the shared 40px control and 14px text roles, a restrained 24px page heading, visible focus, and a compact mobile navigation disclosure. Preserve Manrope/Unbounded and the ink/velvet/almond palette.

## Generated visual specimens

- [Schedule with focused placement drawer](concepts/schedule-focus.png): useful for hierarchy, context density, table priority, and editor placement. Sample lessons, times, labels, and controls are illustrative. Implement against the real schedule table, routes, contracts, and selectors.
- [Resource list with separate creation drawer](concepts/resource-focus.png): useful for list-first composition and editor separation. Its room table columns and fields are illustrative and must **not** replace the current schema or existing Russian copy.
- [Two-date lesson transfer workspace](concepts/two-date-transfer.png): useful for the side-by-side date comparison, multi-selection toolbar, visible drop targets on either date, preview sheet, and positional week-parity key. Its sample data, columns, and conflict outcome are illustrative; preserve the real two-column date tables and operation semantics.

The images are not application assets. Do not embed them in the product. The current dark sidebar and real navigation destinations take precedence over details invented by image generation.

## Implementation order and gate

First authenticate the running app and verify the complete Schedule → Workload → teacher assignment → lesson placement → edit → reload/read-back journey, including the running source revision. Then complete the workbench plan's batches in order: shared primitives and Schedule/Workload, flow repair, Resources, Calendar, Accounts, and adjacent/responsive verification. Finish and verify each batch before the next. No new tests or test files.

The local administrator session was authenticated on 2026-09-23. The full Schedule → Workload → assignment → placement → edit → reload journey passed with disposable records, and both Rails and API runtime source was checked against the working tree. No credentials are stored here.

## Two-date transfer requirement — 2026-09-23

The existing `/admin/transfers` lesson mode already accepted two dates and rendered two tables, but its `group_grid` read showed recurring templates rather than authoritative dated overlays. The implementation now reads each date through the occurrence query, including moves and cancellations. Single drag/click and explicit swaps remain available.

The requested UX extends this foundation:

1. Show the selected group/semester and the two dates in a compact persistent summary. Keep a deliberate context editor, with both dates editable and reversible left/right roles. Label the tables by their actual dates; either table can be a source or destination, including a different slot on the same date.
2. Each occurrence is selectable by checkbox or keyboard, and remains draggable individually. A selection toolbar shows the count and destination action. For **multiple selected lessons**, the user chose this rule: move each selected lesson to the **other selected date at its existing time slot**. Preview the full mapping and conflicts before an explicit Save. A single drag to a particular cell may use that cell's time and either displayed date.
3. Keep the source record visible until the write succeeds; reload both authoritative dated reads after success. Show existing moved/cancelled state in text, not only color. A cancelled occurrence is not offered as a draggable active lesson.
4. Single occupied-cell drops remain an explicit swap. A batch target with an occupied slot must produce a conflict preview or a separately selected swap operation; it must not silently replace an occurrence.
5. The current API only writes one move or a two-occurrence swap atomically. Do not implement batch mode as sequential client POSTs: a later rejection would leave earlier moves saved. Add a typed batch contract and one authorized, locked API transaction that validates every affected occurrence and destination, detects conflicts among planned moves and existing reservations, writes all transfer/Outbox rows atomically, and returns a complete result. Rails should call that operation once. Resolve moved-in occurrences deliberately: the current repository only accepts an active `scheduled` source and the unique transfer key is `(schedule_event_id, original_date)`. The UI must not claim every displayed card can move until editing/replacing an existing overlay is supported safely.
6. In the recurring Schedule grid, a common-week lesson spans the full cell width; an even-week lesson occupies the **upper half** and an odd-week lesson the **lower half**. Use actual layout tracks and a short text label, so the distinction survives grayscale and color vision differences. Maintain the original time/day table axes and all events, including subgroup cases where more than one card shares a track. The dated transfer tables show actual occurrences for specific dates; parity there is secondary metadata, not a filter that hides a dated move.

## Live implementation evidence — 2026-09-23

- A compact committed context, two editable dates, a date-order switch, two dated tables, individual drag/click actions, checkbox selection, and a batch preview are running. Exchange selection lives inside the lesson editor instead of adding a third button to every table card. Each selected lesson maps to the other selected date at its existing slot. The API validates and writes the batch in one authorized transaction, including Outbox notifications.
- The dated read shows authoritative moved-in and cancelled occurrences. Moving an already moved lesson again and exchanging two moved-in lessons were saved and read back through the browser. A conflicting single move returned a Russian error, kept both original lessons in place, and reopened the editor with the date, slot, and reason retained. A visibly occupied batch destination disables Save in the preview. An intentionally submitted conflicting two-item batch was rejected atomically: both lessons stayed at their original displayed slots, both selections and the preview were restored, and Save was disabled again.
- Day-transfer and lesson-transfer tabs now navigate at page level, avoiding Turbo's `Content missing` response when leaving the framed day-transfer screen. The exact days → lessons → days path was verified in the browser. A moved-in lesson was also dragged to another slot on the same displayed date, saved/read back, then restored.
- The recurring Schedule grid now uses equal upper/lower tracks for even/odd weeks and full-cell cards for common weeks. Browser geometry confirmed equal tracks; the visible Russian labels also identify the week without color.
- Desktop and 390px captures: [transfer desktop](../../../.playwright-mcp/admin-transfers-final-desktop.png), [transfer mobile](../../../.playwright-mcp/admin-transfers-final-mobile-fixed.png), [Schedule parity](../../../.playwright-mcp/admin-schedule-parity-final.png). The screenshots are local evidence and may include fixture account labels; do not publish them externally without review.
- Runtime checks: Rails Zeitwerk and changed Ruby syntax passed; the transfer Stimulus module passed `node --check`; browser console errors were zero. The mobile page had no document-level horizontal overflow and retained both table columns in labelled scroll regions.

Remaining verification: teacher/room conflicts across groups, scoped-role authorization using a suitable fixture, and the broader Resources/Calendar/Accounts acceptance matrix from the workbench plan. The current UI changes do not claim those gates complete.
