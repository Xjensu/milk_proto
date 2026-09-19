# Task Brief — `/schedule` page (group timetable)

> Self-contained. You have no other context beyond this file, the mission
> doc it was derived from (`docs/superpowers/plans/2026-08-16-remaining-public-pages-migration-prompt.md`
> — read it first for ground rules, architecture, landmines and the
> verification protocol; this brief only fills in specifics for this page),
> and the repository itself.

## Scope ruling — group mode only

The React `SchedulePage.tsx` is a three-mode page (`?group=`, `?teacher=`,
`?room=`). **Do not port teacher/room mode.** Both already have dedicated,
already-migrated Rails pages (`/teacher/:id`, `/room/:id`) with their own
schedule section built in. This Rails `/schedule` route serves **group mode
only**: `GET /schedule?group=<group_semester_id>&date=<YYYY-MM-DD>`. `date`
is optional (defaults upstream to today). There is no `?teacher=`/`?room=`
handling to build.

## Live-verified data

```
GET /public/groups/513/schedule
{"meta":{"resolved_at":"...","anchor_date":"2026-08-16","timezone":"Europe/Moscow",
  "window":{"start_date":"2026-08-10","end_date":"2026-08-23"},
  "weeks":[{"start_date":"2026-08-10","end_date":"2026-08-16","parity":"even","iso_week":32,"is_current":true},
           {"start_date":"2026-08-17","end_date":"2026-08-23","parity":"odd","iso_week":33,"is_current":false}],
  "semester":{"group_semester_id":513,"semester_id":1,"number":1,"label":"1 семестр 2025/2026",
    "start_date":"2026-07-29","end_date":"2026-11-11","period_kind":"semester"}},
 "occurrences":[
   {"date":"2026-08-10","state":"scheduled","day_of_week":1,"specific_date":null,
    "subgroup_number":0,"status":"scheduled",
    "time_slot_start_time":"10:10:00","time_slot_end_time":"11:35:00",
    "week_type_name":"Каждую неделю","room_name":"Уч.ауд.каф.","room_number":"1207",
    "additional_rooms":[],"subject_name":"Скриптовые языки программирования",
    "subject_short_name":"Скриптовые языки.програм.","activity_type_name":"лекция",
    "group_semester_id":513,"group_display_name":"СИ 3 курс","course":3,
    "speciality_name":"СИ","kafedra_name":"Информационно-управляющие системы и технологии",
    "teachers":[],"original_date":null,"transfer_id":null,"transfer_reason":null},
   {"...":"a later row has teachers:[{\"id\":395,\"last_name\":\"Щербаков\",\"first_name\":\"Сергей\",\"middle_name\":\"Васильевич\",\"event_teacher_id\":46,\"teacher_on_kafedra_id\":263}]"}
 ]}
 (top-level also carries "transfers" — confirmed present on the teacher
 endpoint's identical shape; truncated out of this curl by output length,
 verify it yourself with `curl ... | wc -c` / a full dump before assuming
 its exact shape, though `Entities::ScheduleEvent`/gateway don't need to
 touch it — see below)

GET /public/groups/513/periods
{"group_semester_id":513,"periods":[
  {"period_kind":"semester","start_date":"2026-07-29","end_date":"2026-11-11"},
  {"period_kind":"lecture_week","start_date":"2026-09-01","end_date":"2026-09-07"}]}
 -- NOT needed by this page. GroupPicker (/group task) already consumes this
 endpoint to populate the schedule-type selector before navigating here.
 SchedulePage.tsx never calls it, and schedule_home/schedule.html's controls
 are subgroup + week only (no schedule-type selector). Confirmed by reading
 both. Do not build a periods gateway for this task — the /group task owns it.

GET /public/groups/513/now
{"data":{"id":"513","type":"group_now","attributes":{
  "resolved_at":"2026-08-16T16:07:01+03:00","day_of_week":7,
  "week_type_name":"Чётная","time_range":"15:35 - 17:00",
  "has_class":false,"current":[]}}}
```

`/now` **is** needed, but not for anything `SchedulePage.tsx` calls — the
React/reference implementation computes the mobile "Сейчас/Сегодня" glance
banner **client-side** from `new Date()` against the already-fetched
occurrences (see `schedule_home/schedule.js` lines 229-270, e.g.
`mobileGlance.classList.add('is-now')` when a `currentItem` is found by
scanning `actualItems` against `now.getHours()*60+now.getMinutes()`). Since
Rails renders server-side, you have two options for that same "what's
happening right now" glance banner:

1. Reimplement the client JS's minute-comparison logic in Ruby against the
   already-fetched occurrences (no extra request).
2. Call `GET /public/groups/:id/now`, which already resolves exactly this
   server-side (`has_class`, `current[]`, `time_range`) — one more request,
   zero reimplemented date-arithmetic.

**Recommendation: use `/now` (option 2).** It's purpose-built for exactly
this, matches mission §7's "prefer cacheable GETs" default, and avoids
re-deriving logic that's easy to get subtly wrong (timezone, "how do ties at
minute-end break" etc. — the upstream author already solved it). Build
`Gateways::Portal::GroupNowGateway` for it. Fan the two calls (`schedule`,
`now`) out concurrently via `Async::Task.current.async { ... }.wait` pairs in
the presenter/controller (same spirit as `Operations::RoomWeekSchedule`, just
two calls instead of six) rather than serial `await`s — not because serial
would block the reactor (`Resilience::Call` already yields), but because it's
free latency to give up otherwise.

## Ruling on Presenters::WeekSchedule / Policies::WeekWindow reuse

Read both files in full before starting
(`app/lib/presenters/week_schedule.rb`, `app/lib/policies/week_window.rb`).

**`Policies::WeekWindow` — reuse as-is, no changes.** It's pure date math
(week boundaries, parity, weekday indexing, mobile-day resolution), already
proven against this exact upstream on the teacher page, and this page's data
shape is identical (`meta.weeks[]` on both endpoints).

**`Presenters::WeekSchedule` — reuse as-is.** Its public surface
(`events`/`days`/`time_slots`/`cell`/`mobile_events`/`empty?`) is exactly
what a day×time-slot grid needs, and nothing about it is teacher-specific —
it operates purely on `Entities::ScheduleEvent` rows plus a `week`/`weeks`/
`mobile_day`, all of which this page has too. **Do not fork it.**

**`Entities::ScheduleEvent` — extend, additively.** The live payload already
matches every attribute this struct currently declares (confirmed field-for-
field above — no rename needed, unlike the historical teacher-name bug). But
this page needs three things the teacher/room record pages never used, so add
these attributes (all `.optional.default(nil)` except `subgroup_number`,
which should default to `0` — the "shared, all-subgroups" sentinel):

```ruby
attribute :subgroup_number, Types::Integer.default(0)
attribute :state,           Types::String.optional.default(nil)
attribute :original_date,   Types::String.optional.default(nil)
attribute :transfer_reason, Types::String.optional.default(nil)
```

populate from `row[:subgroup_number]`, `row[:state]`, `row[:original_date]`,
`row[:transfer_reason]` in `.from_row`. Confirmed enum for `state` (from
`apps/frontend/src/features/schedule/WeekGrid.tsx` line 60-65 and
`types.ts`'s `OccurrenceState`): `scheduled | moved_in | cancelled |
reserved`. Confirmed subgroup filter semantics (`WeekGrid.tsx` line 94):
`subgroup_number == 0` events are shared across all subgroups and always
visible; `1`/`2` are subgroup-specific and visible only when the filter is
`"all"` or matches exactly. Implement that same predicate in
`WeekSchedule#cell`/`#mobile_events` (add a `subgroup:` filter param) or in
the view layer — your call, but it must live in exactly one place per
mission §9.

## Ruling on view-layer reuse: do NOT reuse `shared/_schedule_table` etc.

`shared/_schedule_table.html.erb`, `shared/_schedule_mobile.html.erb`,
`shared/_schedule_event.html.erb` (read all three) back the **record page's**
embedded schedule section (`/teacher/:id`, `/room/:id`). Confirmed by
reading them: they hardcode `.teacher-schedule-*`/`.teacher-mobile-*` class
names (styled in `_teacher_record.css`), have no subgroup filtering, no
transfer added/removed styling, no event-detail dialog, and their mobile
layout is a simple day-switcher + flat list — not the reference's
glance-first agenda this page requires (see below). Reusing them for
`/schedule` would mean bolting four new features onto a partial whose name
and stylesheet both say "teacher record", which mission §9 explicitly rules
out ("one component, one stylesheet, one root class... no component restyles
another").

**Write new, page-specific partials** under `app/views/schedule/` (or
similarly scoped) using the reference's own class vocabulary — `.schedule-
table`, `.schedule-event`, `.mobile-schedule`, `.mobile-day-tabs`,
`.mobile-glance`, `.mobile-agenda`, `.mobile-agenda-event`,
`.schedule-event-backdrop` (event detail dialog), `.transfers-backdrop`
(transfers list dialog), `.schedule-legend`. They may still call
`Policies::WeekWindow.weekday_index` etc. directly, same as the existing
partials do — only the ERB/CSS output differs, not the data layer.

## Functional spec (from `SchedulePage.tsx`, full file, and
`apps/frontend/src/features/schedule/WeekGrid.tsx`, read both in full)

- **Identity strip**: group's own display code (read from
  `occurrences[0].group_display_name` — `meta` carries no group-identity
  field, only `semester`, confirmed in both the live payload and a comment
  in `SchedulePage.tsx`'s `resolveGroupIdentity`; falls back to
  `meta.semester.label` when the window has zero occurrences) + the
  semester label.
- **Subgroup selector**: three buttons, Все / 1 / 2. Filters visible events
  per the semantics above. Client-side only (no request) — same window data,
  different filter.
- **Week selector**: reuse the same pattern as the teacher page's
  `shared/_week_selector` partial if it fits (check
  `app/views/shared/_week_selector.html.erb` — not read for this brief;
  verify it's generic enough, i.e. takes a `schedule:` presenter and doesn't
  assume teacher context. If it's already generic, reuse it — don't fork a
  week selector that already exists and works).
- **Transfers button**: shows a count (`transfers.length`), disabled at 0,
  opens the transfers dialog on click.
- **Desktop table**: day × time-slot grid exactly like the teacher page's,
  but each event button gets `is-added`/`is-removed` classes per `state`
  (`moved_in`→added, `cancelled`→removed, else neither), and clicking an
  event opens the **event detail dialog** (not a `teacher_detail_path` link
  like the record page's cell — this page's cells open a modal with
  activity type, rooms, subject, group/subgroup, teachers, and — when
  `state` warrants it — either "Перенос: с {original_date}" (moved_in) or
  "Причина: {transfer_reason}" (cancelled)).
- **Mobile**: **not a compressed table** — see visual spec below, this is a
  structurally different information design (day tabs + a "what's happening
  right now/next" glance card + a scrollable agenda list), ported from
  `schedule_home/`'s `.mobile-schedule`/`.mobile-glance`/`.mobile-agenda`,
  not from the teacher page's `.teacher-mobile-*` day-switcher.
- **Legend**: static "Добавлено"/"Убрано" key at the page bottom
  (`.schedule-legend`), always visible, not just when transfers exist —
  matches both the reference and `SchedulePage.tsx`.
- **Loading/error/empty**: a fetch failure renders `region_error`-equivalent
  text in place of the grid, not the generic "nothing selected" empty state
  (`ScheduleBody` in the React source separates these on purpose — replicate
  that separation, don't let a real failure render as "no schedule").

## Visual spec

`schedule_home/schedule.html` for structure (note: the reference's own
`<body class="schedule-page">`, `.schedule-main`, `.schedule-controls`,
`.schedule-identity`, `.schedule-selector` ×2, `.transfers-button`,
`.schedule-table-wrap` / `#scheduleTable`, `.schedule-legend`,
`.schedule-event-backdrop`/`#eventDialog`, `.transfers-backdrop`/
`#transfersDialog` — port all of these class names verbatim).

CSS, three blocks in `schedule_home/styles.css`:
- **Lines 1671–2137** (`/* Schedule page */`) — desktop table, controls,
  identity strip, legend, dialogs. Includes responsive tweaks at `@media
  max-width: 980px` (2138) and `640px` (2144), plus a `min-width: 981px`
  rule at 2155 (almost certainly "hide mobile agenda on desktop" — verify
  and mirror the inverse for "hide desktop table on mobile").
- **Lines 2159–2540** (`/* Mobile schedule: glance-first agenda instead of a
  compressed desktop table */`) — this is the important one; it's long
  (~380 lines) because it's a genuinely different layout, not a media-query
  shrink of the desktop table. Read it in full before implementing the
  mobile view; don't guess its structure from the class names alone.
  Includes a `@media max-width: 390px` sub-block at 2541 for the smallest
  breakpoint.

`schedule_home/schedule.js` lines 229-270 is the reference implementation of
the glance card's "Сейчас: {event}" / "Сегодня: На сегодня всё" /
next-lesson logic — read it even though you're replacing its data source
with `/now` server-side, because the **display logic** (what text appears in
which state) still needs porting into ERB.

## Architecture — the recipe, filled in

**1. Entity** — extend `Entities::ScheduleEvent` as specified above (do not
create a new entity class).

**2. Gateways** — two new:
- `app/lib/gateways/portal/group_schedule_gateway.rb < BaseGateway`, KEY
  `"portal.groups.schedule"`. Mirror `TeacherScheduleGateway#schedule`
  exactly (same raw-passthrough style, same `deep_symbolize_keys`), hitting
  `/public/groups/#{group_semester_id}/schedule#{query_string(date: date)}`.
- `app/lib/gateways/portal/group_now_gateway.rb < BaseGateway`, KEY
  `"portal.groups.now"`. `#now(group_semester_id:)` → `GET
  /public/groups/#{group_semester_id}/now` → this one **is** JSON:API-shaped
  (`{"data":{"id":...,"type":"group_now","attributes":{...}}}` — confirmed
  live above), so return `document.dig("data", "attributes")&.deep_symbolize_keys`
  rather than the whole document.

**3. Register** in `container.rb`:
```ruby
ScheduleWeb::Container.register("gateways.group_schedule") { Gateways::Portal::GroupScheduleGateway.new }
ScheduleWeb::Container.register("gateways.group_now")      { Gateways::Portal::GroupNowGateway.new }
```
Add `portal.groups.schedule` and `portal.groups.now` to
`Resilience::Call::KNOWN_KEYS`.

**4. Presenter** — `app/lib/presenters/group_schedule.rb`, modeled directly
on `Presenters::TeacherRecord` (read it in full — you're mirroring its
`initialize`, `weeks_from`, `occurrences_result`, `payload` private methods
almost verbatim, swapping the teacher-lookup half out for nothing — there's
no separate "record" call here, just schedule + now). Exposes `schedule`
(a `WeekSchedule`), plus whatever the identity strip and glance card need
(`group_display_name`, `semester_label`, `now` outcome wrapped in its own
`Presenters::Base`-style wrapper so a failed `/now` call degrades
independently rather than taking down the whole page — same independence
principle as `TeacherRecord`'s two calls).

**5. Controller** — `app/controllers/schedules_controller.rb` (new;
`GroupsController` already exists for `/group` and is a different resource).
`def show; @page = Presenters::GroupSchedule.new(group_semester_id:
params[:group], date: params[:date], subgroup_param: params[:subgroup]); end`
— thin, per mission §4. Route: `get "schedule" => "schedules#show", as:
:schedule`. Redirect or 404 if `params[:group]` is blank (this route has no
teacher/room fallback per the scope ruling above — decide 400 vs redirect-
to-`/group` and note your choice; either is defensible, just be consistent
with how `TeachersController#show` handles a missing/invalid id — read it,
it returns 404 via `render status: ... unless found?`, follow that
precedent: not-found rather than a redirect).

**6. Views** — new partials under `app/views/schedule/` per the "do NOT
reuse" ruling above, plus `app/views/schedules/show.html.erb` as the page
shell. Use `shared/_stale_banner`/`shared/_region_error` for the schedule
call's degraded states (the `/now` call degrading should probably just hide
the glance card rather than showing a banner on top of a working schedule —
your call, but a `/now` failure must never block the rest of the page from
rendering, per mission §7's "degrade, never fail blank").

**7. Stylesheet** — `app/assets/stylesheets/components/_schedule.css` (new),
ported from the two CSS blocks above, imported via `@import
url("components/_schedule.css");` in `application.css`.

**8. Locale keys** — extend the existing `schedule:` block in `ru.yml`
(it already has `dash`, `time_header`, `week_selector_aria`, `week.*`,
`weekdays_short.*` — reuse these, they're generic) with page-specific
additions: subgroup selector labels/aria, transfers button label, legend
labels ("Добавлено"/"Убрано" — check if these already exist somewhere else
in `ru.yml` before adding duplicates), glance card copy ("Сейчас" / "Сегодня"
/ "На сегодня всё"), event-detail dialog field labels ("Перенос", "Причина"),
group-not-found or schedule-not-found empty/error copy.

## Landmines that apply here (mission §5)

- **#13/#14** — you're extending an entity and adding a JSON:API-shaped
  gateway (`/now`) alongside plain-hash ones (`/schedule`) in the same task;
  don't let the two response shapes bleed into each other, and remember a
  cache-hit round-trips through JSON (symbolize on read).
- **#3/#4** — CSS/JS changes need a full rebuild to verify, and the new
  stylesheet's `@import` must use the `url(...)` form.
- **#12** — the mobile glance card almost certainly has an icon or fill
  element; check it isn't `display: inline` before relying on `width/height:
  100%` inside it.
- **#9** — the event-detail dialog and transfers dialog both need Stimulus
  wiring (open/close, backdrop click); keep their targets inside one
  controller's element, and don't let a `preventDefault()` fire before
  confirming the target exists (a subgroup filter button or event card that
  silently does nothing is worse than an unstyled one).
- **#11** — both dialogs are full-viewport overlays; `pointer-events: none`
  when closed, and reset on `pageshow` for bfcache.

## Definition of done (subset of mission §11)

1. `GET /schedule?group=513` returns 200 with real data; identity strip
   shows "SI 3 курс"-equivalent + "1 семестр 2025/2026".
2. Subgroup filter changes visible events without a network request (check
   Network tab / server logs — no second request on click).
3. An event with `state: "moved_in"` or `"cancelled"` (you may need to
   create test data via the admin side, or find one in seed data — check
   before assuming none exist) renders with the correct `is-added`/
   `is-removed` styling and dialog copy.
4. Desktop table hidden on mobile viewport, mobile agenda hidden on desktop
   (verify both directions, not just one).
5. `/now`-sourced glance card shows correct text for "class in progress" vs
   "next class today" vs "nothing left today" states (seed data's `has_class:
   false` case is easy to hit — find or construct a "true" case too if
   possible, or explicitly note in your report that this state is unverified).
6. Assets 200, zero console/CSP errors at 1280/375, no horizontal overflow,
   footer pinned.
7. Degraded-state check: stop `api_geteway`, confirm schedule shows stale
   banner (revisit) or region error (cold), and confirm a `/now` failure
   alone (if you can isolate it) doesn't take down the rest of the page.
   Restart `api_geteway` afterwards.
8. No `translation missing` anywhere on the page in any state.
