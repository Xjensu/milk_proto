# schedule_home Design Adoption — Design

Date: 2026-08-08
Scope: `apps/frontend` (separate nested git repo)
Depends on: `2026-08-08-public-period-exposure-design.md` (backend, for the
group-schedule period-jump control only — everything else in this spec is
independent of it and can ship first if convenient)

## Goal

Replace the current public (unauthenticated) surface of `apps/frontend` —
home, group/teacher/room lookup, schedule display — with the exact visual
design, copy, layout, fonts and interaction feel of the `schedule_home/`
mockup at the repo root, driven by the app's real API instead of the
mockup's hardcoded mock data. This supersedes the visual direction from
`2026-07-30-public-home-schedule-redesign-design.md` for the public surface.

Everything the mockup does that has no real backing data or conflicts with a
deliberate backend security decision is adapted or dropped, per the scoping
conversation — see "Deltas from the mockup" below. Nothing here changes
business logic beyond what's already exposed by the public API (plus the one
new periods endpoint from Spec A).

## Out of scope (unchanged)

- Admin panel (`features/admin/**`)
- Login as a *page* is removed for the public flow (replaced by a modal, see
  below) but `Login`'s request logic, `PasswordChange`, `TeacherSelfService`
  and the `admin`/`self` branches of `App.tsx` are otherwise untouched —
  same components, same API calls, reached the same way once authenticated.
- `WeekGrid.tsx` (the day-of-week grid used by `AdminSchedule` /
  `TeacherSelfService`) — not touched; `OccurrenceWeekGrid` in the same file
  is the one being redesigned, and only its public-facing usage.

## Navigation architecture

Add `react-router-dom`, mounted only for the public subtree. `App.tsx`'s
existing `view` state machine keeps driving `login` / `password` / `admin` /
`self` exactly as today (unauthenticated users land in the router; a
successful staff login exits the router into the existing state machine, and
logout re-enters the router at `/`).

Routes (mirroring schedule_home's pages 1:1):

| Route | Mockup page | Component |
|---|---|---|
| `/` | `index.html` | `PublicHome` |
| `/group` | `group.html` | `GroupPicker` |
| `/teacher` | `teacher.html` | `EntitySearch mode="teacher"` |
| `/room` | `room.html` | `EntitySearch mode="room"` (gated, see below) |
| `/schedule` | `schedule.html` | `SchedulePage` (reads `?group=`/`?teacher=`/`?room=` + id) |
| `/teacher/:id` | `teacher-detail.html` | `TeacherDetail` |
| `/room/:id` | `room-detail.html` | `RoomDetail` (gated) |
| `/guide` | `guide.html` | `Guide` |

A shared `PublicLayout` route wrapper renders the header, footer, and the
four shared dialogs (Auth, Contacts, Cookies, Guide-picker — built once as
real components, not copy-pasted per page like the mockup; visually
identical, DRY implementation).

## Visual/asset porting

- `schedule_home/styles.css` and `guide.css` are copied into
  `apps/frontend/src/features/public/` (new directory) as their own
  stylesheets, imported only by the public route tree, so they cannot
  collide with the existing admin `styles.css` (class names in the mockup
  are not namespaced, so isolation is by import scope, not renaming).
- Fonts: same Google Fonts `<link>`s (Manrope, Unbounded) added to
  `index.html`.
- Each mockup page becomes one React component reproducing its exact DOM
  structure/classes so the existing CSS applies unmodified. No new visual
  elements, no rearrangement.
- Decorative pixel-art (`teacher-pixel-head` / room floor-plan) ported
  as static, non-data-driven decoration exactly as in the mockup — same
  hardcoded pixel patterns, per your instruction to keep them regardless of
  the real-data trim elsewhere on those pages.
- The 3 Contacts-dialog icons currently pulled from the `unpkg.com/reicon`
  CDN are inlined as local SVGs instead (matching how every other icon in
  the mockup is already handled) rather than adding an external script tag
  to a production app.
- The orphaned `assets/user-circle-outline.svg` is not wired up; the profile
  icon keeps using the mockup's actual pattern (inline SVG in the header).

## Deltas from the mockup (functional)

| Mockup behavior | What ships instead | Why |
|---|---|---|
| Room search/schedule fully public | Stays gated behind login (`RoomLoginGate`, restyled to match the new visual language) | Backend deliberately moved room occupancy to admin-only; not reopening that |
| `±2 weeks` pager (doesn't exist in mockup, exists in current app) | Removed | Explicit instruction; no design equivalent |
| Binary "Нечётная/Чётная" week toggle computed from `new Date()`, fake content | Same two-button visual, bound to the real `weeks[]` from `OccurrenceWindow` (real dates, real `parity`, real lessons); `is_current` picks the default tab | Real data exists and fits the same UI slot |
| `group.html`'s 4th picker step (Regular/Lecture/Credit/Exam), fake generated group codes | Faculty → course → group wired to `listFaculties`/`listPublicGroups` (real). Schedule-type becomes a period-jump control *on the schedule page* (button per period from the new `/public/groups/:id/periods`, click sets `date` to that period's `start_date`) rather than a pre-selection step, since periods are per-group and the group isn't known until after this step | Real period data is per-group-semester (Spec A); moving it to the schedule page is the only place it can be wired to something real |
| `teacher-detail.html`/`room-detail.html` dossier: subjects, capacity, equipment, "12 hardcoded profiles" | Real fields only (teacher `post`/`academic_degree`/`academic_title` from the search response; room `number`/`building.number`/`room_type.name`) + their real schedule | No public data backs the dropped fields; inventing data isn't acceptable |
| Auth: full-page `Login` view | Login modal, reachable from every public page, visually identical to the mockup's `#authDialog` — but the submit handler calls the real `api.login` (reusing today's `Login` logic/error handling), not the mockup's `event.preventDefault()`-only stub | Confirmed with you — visual only, not the mockup's inert form |
| Cookie consent / Contacts dialogs (don't exist in current app) | Included, wired for real (`localStorage` + `document.cookie` persistence; contacts shows real placeholder links, swappable later) | Confirmed with you — part of the shipped design, not new business functionality |
| Guide book (`guide.html`) | Ported as-is, static content, including the custom page-flip engine | No data dependency, purely explanatory |

## Data wiring summary

- `PublicHome`: no data. Three floor buttons route to `/group`, `/teacher`,
  `/room` (room floor still visible/clickable — the login gate lives on the
  `/room` route itself, matching how `HomePage.tsx` handles it today via
  `onRequireLogin`, not by hiding the option).
- `GroupPicker`: `api.listFaculties()` for the faculty cards,
  `api.listPublicGroups(facultyId, date)` for the course/group step,
  restyled into the mockup's carousel/detail-overlay chrome.
- `EntitySearch`: `api.listPublicTeachers(query)` / `api.listPublicRooms(query)`,
  client-side pagination over the real result set (page size matches the
  mockup: 8 for teachers, 9 for rooms).
- `SchedulePage`: `api.groupSchedule` / `api.teacherSchedule` /
  `api.roomOccupancyWindow`, rendered through a redesigned
  `OccurrenceWeekGrid` (same component, restyled to the mockup's table/agenda
  markup) — real `transfers[]` feed the "Переносы" panel, real occurrence
  `state` feeds the modal/legend (scheduled/moved_in/cancelled/reserved).
  Mobile day-tabs + swipe gesture + "Сейчас/Далее" glance card are ported
  from the mockup as pure presentation on top of real occurrence data for
  today's date.
- `TeacherDetail` / `RoomDetail`: entity fields come from the same
  `listPublicTeachers`/`listPublicRooms` search call (fetched by id/filtered
  client-side, since there's no single-entity endpoint — see Spec A's report,
  no such endpoint exists and adding one is out of scope here since the
  search response already has everything needed), schedule from
  `teacherSchedule`/`roomOccupancyWindow`.
- `Guide`: static, no API calls.

## Testing

- Existing tests for `PublicSchedulePage.test.tsx`, `WeekGrid.test.tsx`,
  `normalize.test.ts` get updated/split to match the new component
  boundaries (`SchedulePage`, `OccurrenceWeekGrid` restyle) — same
  normalization logic, new render shape.
- New tests: `GroupPicker` faculty→course→group flow against mocked
  `api.listFaculties`/`listPublicGroups`; `EntitySearch` search+pagination
  against mocked results; route-gating test confirming `/room` and
  `/room/:id` show `RoomLoginGate` when unauthenticated and the real content
  when a token is present.
- Manual verification in-browser (dev server via the already-open Docker
  container) of: full home→group→schedule flow, full home→teacher→detail
  flow, room gate behavior logged-out vs logged-in, cookie consent
  persistence across reload, guide book page-turning, mobile viewport
  (day-tabs/swipe/glance card).
