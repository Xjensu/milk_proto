# Task Report — `/schedule` page (group timetable)

Implements `docs/superpowers/plans/2026-08-16-task-schedule-brief.md` under the
ground rules of `docs/superpowers/plans/2026-08-16-remaining-public-pages-migration-prompt.md`.

All paths are relative to `apps/schedule_web/` unless stated otherwise.
Nothing was committed or staged — every change below is uncommitted working-tree
state.

---

## (a) Files created / modified, by layer

### Entity

| File | Change |
|---|---|
| `app/lib/entities/schedule_event.rb` | **modified, additive.** Four new attributes exactly as the brief specifies: `subgroup_number` (`Types::Integer.default(0)`), `state`, `original_date`, `transfer_reason` (all `.optional.default(nil)`), populated in `.from_row` from the matching payload fields. Added `.visible_for_subgroup?(number, filter)` (class + instance form), `added?`, `removed?`, `state_class`. No renames, no removals — the teacher/room record pages read none of the new fields. |

### Gateways

| File | Change |
|---|---|
| `app/lib/gateways/portal/group_schedule_gateway.rb` | **new.** `KEY = "portal.groups.schedule"`. Mirrors `TeacherScheduleGateway#schedule` verbatim (raw pass-through, `deep_symbolize_keys`) against `/public/groups/:id/schedule?date=`. |
| `app/lib/gateways/portal/group_now_gateway.rb` | **new.** `KEY = "portal.groups.now"`. JSON:API-shaped, so it returns `document.dig("data", "attributes")&.deep_symbolize_keys`, not the whole document. |

### Operation

| File | Change |
|---|---|
| `app/lib/operations/group_schedule_window.rb` | **new.** Concurrent fan-out of the two calls via `Async::Task.current.async { … }.map(&:wait)`, returning the pair unmerged — same shape and reasoning as `Operations::FacultyCatalogWithGroups`. |

### Presenter

| File | Change |
|---|---|
| `app/lib/presenters/group_schedule.rb` | **new.** Modeled on `Presenters::TeacherRecord` (`parse_date`, `weeks_from`, `occurrences_result`, `payload` carried over near-verbatim). Exposes `schedule` (a `WeekSchedule`), `now` (its own `Presenters::Base`), `found?`, `identity`, `semester_label`, `transfers`, `today`, `subgroup`, `visibility_for`, `cell_empty_for`, `mobile_empty_for`, `glances`, `viewing_today?`, `key_for`. |
| `app/lib/presenters/group_periods.rb` | **modified.** Removed the `SCHEDULE_PATH = "/schedule"` literal and its comment; `#confirm_path` now calls `Rails.application.routes.url_helpers.schedule_path(**query)`. Looked up per call rather than by `include`-ing `url_helpers` at class-definition time, because `config/routes.rb` loads *after* eager loading in production. |
| `app/lib/presenters/week_schedule.rb` | **not touched** (brief ruling honoured). |
| `app/lib/policies/week_window.rb` | **not touched** (brief ruling honoured). |

### Resilience / container

| File | Change |
|---|---|
| `app/lib/resilience/call.rb` | **modified.** Added `portal.groups.schedule` and `portal.groups.now` to `KNOWN_KEYS` so `/readyz` reports them from a cold process. |
| `config/initializers/container.rb` | **modified.** Registered `gateways.group_schedule` and `gateways.group_now` with the existing lazy-block pattern. |

### Controller / routes

| File | Change |
|---|---|
| `app/controllers/schedules_controller.rb` | **new.** Thin: params → presenter → `render status: (@page.found? ? :ok : :not_found)`, following `TeachersController#show`. |
| `config/routes.rb` | **modified.** `get "schedule" => "schedules#show", as: :schedule`. |

### Helpers

| File | Change |
|---|---|
| `app/helpers/application_helper.rb` | **modified** (was empty). `russian_plural_key`, `short_date`, `schedule_event_note`. |

### Views

| File | Change |
|---|---|
| `app/views/schedules/show.html.erb` | **new.** Page shell: identity strip, controls, degraded states, mobile + desktop bodies, legend, dialogs via `content_for :dialogs`. |
| `app/views/schedules/_subgroup_selector.html.erb` | **new.** |
| `app/views/schedules/_desktop_table.html.erb` | **new.** |
| `app/views/schedules/_event_button.html.erb` | **new.** |
| `app/views/schedules/_mobile_schedule.html.erb` | **new.** |
| `app/views/schedules/_mobile_glance.html.erb` | **new.** |
| `app/views/schedules/_agenda_event.html.erb` | **new.** |
| `app/views/schedules/_event_dialog.html.erb` | **new.** |
| `app/views/schedules/_transfers_dialog.html.erb` | **new.** |
| `app/views/shared/_week_selector.html.erb` | **modified — bug fix, see (b)1.** |
| `app/views/shared/_schedule_table.html.erb`, `_schedule_mobile.html.erb`, `_schedule_event.html.erb` | **not touched** (brief ruling honoured). |

### JavaScript

| File | Change |
|---|---|
| `app/javascript/controllers/schedule_controller.js` | **new.** Subgroup filter. |
| `app/javascript/controllers/dialog_controller.js` | **modified.** Added a `pageshow` reset (landmine #11) + its `disconnect` teardown. |

### Stylesheets

| File | Change |
|---|---|
| `app/assets/stylesheets/components/_schedule.css` | **new.** ~830 lines ported from `schedule_home/styles.css` 1671–2157 and 2159–2551. |
| `app/assets/stylesheets/components/_schedule_selector.css` | **new.** `.schedule-selector` extracted verbatim out of `_teacher_record.css` — see (b)2. |
| `app/assets/stylesheets/components/_teacher_record.css` | **modified.** The `.schedule-selector*` block removed (replaced by a pointer comment); `.teacher-week-selector` placement modifier stays. |
| `app/assets/stylesheets/application.css` | **modified.** Two `@import url(...)` lines added in the required order. |

### Locales

| File | Change |
|---|---|
| `config/locales/ru.yml` | **modified, additive.** New `date.formats.day_month`, and the `/schedule` keys appended inside the existing `schedule:` block (`weekdays_full`, `page_aria`, `controls_aria`, `table_aria`, `mobile_aria`, `day_selector_aria`, `subgroup_selector_aria`, `subgroup.*`, `transfers.*`, `legend.*`, `event.*`, `glance.*`, `empty`, `day_empty`, `not_found`). The existing generic keys (`dash`, `week.*`, `weekdays_short.*`, `week_selector_aria`) are reused, not duplicated. All explanatory comment blocks preserved. |

### Not modified

`apps/api_geteway` was **read only** — no upstream change was needed.

---

## (b) Deviations from the brief / mission doc, with reasoning

### 1. Fixed a pre-existing bug in `shared/_week_selector.html.erb` (not in scope, but blocking)

The brief told me to reuse the existing week selector if it is generic. It is
generic — but it was **silently broken**, on the already-migrated teacher and
room record pages as well as on mine.

It built each button's `<form url:>` as `url_for(query.merge("week" => parity))`,
i.e. `action="/teacher/1?week=odd"` with no form fields. Submitting a **GET**
form replaces the action URL's query string with the serialized form data, so
the navigation went to `/teacher/1` and the week never changed. Turbo takes the
same route (it rebuilds `url.search` from the form entries), so both the JS and
no-JS paths were affected.

Measured before the fix, in real Chromium:

```
form actions: ["/teacher/1?week=even","/teacher/1?week=odd"]
buttons: ["Чётная|is-active is-current","Нечётная|"]
url after click: http://localhost:3000/teacher/1        <-- week param lost
```

Fix: keep `action` at `request.path` and carry the target parameters as
`hidden_field_tag … id: nil`. After the fix (same test, both pages):

```
before: http://localhost:3000/schedule?group=513&date=2026-08-07
after : http://localhost:3000/schedule?group=513&date=2026-08-07&week=even
active: ["Нечётная","Чётная[active]"]  errors: []
---
before: http://localhost:3000/teacher/1
after : http://localhost:3000/teacher/1?week=even
active: ["Нечётная","Чётная[active]"]  errors: []
```

I judged forking a working-looking-but-broken partial worse than repairing it.
**This is a behaviour change on two existing pages** (their week selector now
actually works) and is the item most worth a human's attention in this report.

### 2. Extracted `.schedule-selector` into its own component stylesheet

`.schedule-selector` was defined in `components/_teacher_record.css`, but this
page needs it for two controls (week + subgroup). Depending on a stylesheet
named after another page contradicts mission §9 ("one component, one
stylesheet, one root class… no component restyles another"). Rules were moved
**verbatim** into `components/_schedule_selector.css`, imported *before*
`_teacher_record.css` and `_schedule.css` so the equal-specificity placement
modifiers (`.teacher-week-selector button`, the 820px overrides) still win.
Verified byte-for-byte equivalent computed styles on both record pages — see
(c)9.

### 3. Three root classes in `_schedule.css`, not one

`.schedule-main`, `.schedule-event-backdrop`, `.transfers-backdrop`. The layout
renders page dialogs through its own `yield :dialogs` slot, i.e. outside
`<main>`, so they cannot be nested under the page root. I chose to use that
established extension point rather than move the dialogs inside `<main>` to win
a single root class. All three belong to this page and this file restyles no
other component.

### 4. Reference CSS deliberately **not** ported

* `.schedule-page .site-footer { position: fixed; … transform: translate(…) }`
  and the `min-height: calc(100svh + 92px)` padding that only exists to give
  that fixed footer something to reveal itself over. This app pins its footer
  with `main { flex: 1 0 auto }` + `.site-footer { margin-top: auto }`
  (landmine #5); `_site_footer.css` is grep-guarded and stays clean.
* `.schedule-back` — the shared header already renders `.back-button` from
  `content_for :back_path`.
* The `footer-visible` scroll-gated class and its scroll listener.

### 5. Subgroup filter: pre-rendered variants rather than client-side re-derivation

The brief requires the filter to change visible events **with no network
request**, and mission §9 requires the rule to live in exactly one place.
Rendering three server-side variants and having JS pick one satisfies both:
every conditional element carries `data-subgroup-visible="<space separated
filters>"` computed by the presenter (including the desktop cells' empty dots,
the mobile empty state and all three glance cards), and
`schedule_controller.js` does nothing but set-membership. The subgroup rule
(`Entities::ScheduleEvent.visible_for_subgroup?`) is **never restated in
JavaScript**. `Presenters::WeekSchedule` therefore needed no `subgroup:`
parameter and was left untouched.

The buttons remain real GET form submits, so the filter also works with
JavaScript off (server-side filtering by `?subgroup=`), and `preventDefault`
only fires after the required targets are confirmed present (landmine #9).

### 6. `/now` used as the authority for "a class is running", not as a clock replacement

Per the brief's recommendation I call `/public/groups/:id/now` rather than
re-deriving minute arithmetic. Two refinements:

* **Only a *fresh* outcome is trusted for the `:now` state.** `/now` answers a
  question about *this minute*; a stale cached copy would happily claim a
  lesson is in progress hours after it ended. On stale/failed, the glance falls
  through to the "next class"/"nothing left" branches instead of lying.
* `/now` cannot answer "what is next", so that branch is computed from the
  already-fetched occurrences against `/now`'s own `resolved_at` (Europe/Moscow,
  resolved upstream), falling back to `Time.current` — `config.time_zone` is
  Europe/Moscow too, so the two agree.

A failed `/now` hides nothing but the `is-now` treatment; it never blocks the
page (verified live, (c)7).

### 7. Mobile day tabs are links, not `role="tab"` buttons

The reference uses `role="tablist"`/`role="tab"` on client-side buttons. Here
each day is a real `?day=mon..sat` link (matching the existing
`shared/_schedule_mobile` precedent and `Policies::WeekWindow.mobile_day_for`),
so `role="tab"` would be a lie to assistive tech. `aria-current="page"` marks
the active day instead. Visual result is identical.

### 8. `/schedule` with a missing/unknown group returns 404, not a redirect

Following the `TeachersController#show` precedent the brief pointed at. The
signal is `meta.semester == null` — upstream answers 200 with an empty window
for an unknown `group_semester_id` (verified live with id 999999), it does not
404. A *failed* call is explicitly **not** a 404: that renders the page with
`shared/_region_error` at 200, the same rule `GroupsController#status_for` uses.

### 9. Russian plurals are selected in a helper, not by I18n

`rails-i18n` is not a dependency, so I18n only knows the English one/other
rule and `t(..., count: 2)` would have shipped "2 занятий".
`ApplicationHelper#russian_plural_key` picks one/few/many, matching the React
page's own `plural()`.

### 10. Added a `pageshow` reset to the shared `dialog_controller`

Landmine #11 requires overlay reset on bfcache restore; the shared controller
had none, so a dialog left open before a back-navigation would return open,
swallowing every click and holding the scroll lock. Three lines, benefits every
page. Not a page edit, so mission §9 is intact.

---

## (c) Verification — real output

Environment note: **Docker Desktop was down when I resumed** (the daemon had
died because drive C: had 0.2 GB free). I cleared the regenerable npm cache
(→ 6.57 GB free), restarted Docker Desktop, and ran `docker builder prune -af`
(reclaimed 7.481 GB) as the mission doc §10 directs. Full stack brought back up
and healthy before every measurement below. `docker compose build schedule_web
&& docker compose up -d schedule_web` was run after the last CSS/JS/ERB change
and before every visual check.

### 1. Container healthy, `/up` and `/readyz` 200

```
schedule_web  Up 9 seconds (healthy)
up:200
readyz:200
schedule:200
```

`/readyz` also proves the new breaker keys are declared (KNOWN_KEYS), measured
here while `api_geteway` was **stopped**:

```json
{"status":"ready","redis":true,"breakers":{"portal.teachers.index":"closed",
"portal.rooms.index":"closed","portal.faculties.index":"closed",
"portal.groups.by_faculty":"closed","portal.groups.periods":"closed",
"portal.groups.schedule":"closed","portal.groups.now":"closed",
"portal.teachers.schedule":"closed"}}
http:200
```

### 2. Route statuses

```
/                                          200
/up                                        200
/readyz                                    200
/teacher                                   200
/room                                      302
/group                                     200
/teacher/1                                 200
/room/1                                    302
/schedule?group=513                        200
/schedule?group=513&date=2026-08-07        200
/schedule                                  404
/schedule?group=999999                     404
/schedule?group=abc                        404

=== guest redirect target for /room ===
http://localhost:3000/?auth=1
http://localhost:3000/?auth=1
```

### 3. Assets actually load

Every `/assets/*` URL on `/`, `/teacher`, `/room`, `/group`, `/teacher/1`,
`/schedule?group=513` and `/schedule?group=513&date=2026-08-07` returned 200
(the check printed only failures; none printed). Every `@import url(...)` inside
the digested `application.css`:

```
css: /assets/application-251d3009.css
  IMPORT /assets/components/_site_header-e3b6c7ac.css -> 200
  IMPORT /assets/components/_site_footer-99625c5a.css -> 200
  IMPORT /assets/components/_stale_banner-48a08253.css -> 200
  IMPORT /assets/components/_region_error-ed6b2ed1.css -> 200
  IMPORT /assets/components/_dialog-3c395a66.css -> 200
  IMPORT /assets/components/_scene-c3ac747d.css -> 200
  IMPORT /assets/components/_transition_layer-05da6774.css -> 200
  IMPORT /assets/components/_entity_search-71962bb6.css -> 200
  IMPORT /assets/components/_schedule_selector-a9c328d2.css -> 200
  IMPORT /assets/components/_teacher_record-261a226e.css -> 200
  IMPORT /assets/components/_faculty_picker-bd16a534.css -> 200
  IMPORT /assets/components/_schedule-6fc8c2fc.css -> 200
```

(Two `IMPORT ... -> 000` lines in the raw output are my grep matching the
literal `url(...)` text inside `application.css`'s own explanatory comment, not
real requests.)

### 4. Real browser pass at 1280 and 375

Zero console errors, zero CSP violations (`securitypolicyviolation` listener),
zero failed requests, `hOverflow: 0` everywhere, footer pinned everywhere.

```
### schedule @1280  .../schedule?group=513&date=2026-08-07
  errors: [] csp: [] failedReq: []
  metrics: {"hOverflow":0,"docH":900,"viewH":900,"footerBottom":900,"footerPinned":true,"desktopTable":1,"mobileAgenda":0,...,"selectorButtons":5}
### teacher1 @1280   errors: [] csp: [] failedReq: []  {"hOverflow":0,"footerPinned":true,...}
### home @1280       errors: [] csp: [] failedReq: []  {"hOverflow":0,"footerPinned":true,...}
### teachersearch @1280  errors: [] csp: [] failedReq: []  {"hOverflow":0,"footerPinned":true,...}
### group @1280      errors: [] csp: [] failedReq: []  {"hOverflow":0,"footerPinned":true,...}
### notfound @1280   csp: []  (the only "error" is the intended 404 status)
### schedule @375
  errors: [] csp: [] failedReq: []
  metrics: {"hOverflow":0,"docH":800,"viewH":800,"footerBottom":800,"footerPinned":true,"desktopTable":0,"mobileAgenda":1,...}
### teacher1 @375, home @375, teachersearch @375, group @375, notfound @375
  all: errors: [] csp: [] failedReq: []  hOverflow: 0  footerPinned: true
```

**Definition-of-done #4 (both directions):** at 1280 `desktopTable:1,
mobileAgenda:0`; at 375 `desktopTable:0, mobileAgenda:1`.

Screenshots taken at both widths, plus the two dialogs, and inspected visually:
identity strip "СИ 3 курс" + "1 СЕМЕСТР 2025/2026", ВСЕ/1/2 + НЕЧЁТНАЯ/ЧЁТНАЯ
selectors (current-week dot present), ПЕРЕНОСЫ with count 1, week-number cell
"31", ПН–СБ heads, the `moved_in` occurrence rendered in almond with
"ДОБАВЛЕНО", legend, pinned footer. At 375: sticky day tabs (ПН 03 … СБ 08),
glance card "03.08 / 1 занятие / ПЕРВОЕ · … / 10:10", timeline agenda, no
desktop table.

### 5. Definition-of-done #1 — real data on the page

```
identity: СИ 3 курс
kind    : 1 семестр 2025/2026
/schedule while signed in: 200 {"status":3,"identity":"СИ 3 курс","kind":"1 семестр 2025/2026","transfers":"1","transfersDisabled":false}
```

### 6. Definition-of-done #2 — subgroup filter changes visible events, no request

Seed data has **no** subgroup-specific occurrence (all 40 `schedule_events`
rows are `subgroup_number = 0`, confirmed by query), so the server cannot
produce a count change. I therefore verified the mechanism in two independent
ways.

**(i) The wiring, live, on real data** — button state, URL and network:

```
initial requests: ["GET http://localhost:3000/schedule?group=513&date=2026-08-07"]
before filter: {"events":3,"dots":9,"active":["Все","Нечётная"],"url":"?group=513&date=2026-08-07"}
after subgroup=1 : {"active":["1","Нечётная"],"url":"?group=513&date=2026-08-07&subgroup=1"}
  network during filter click: []
after subgroup=2 : {"active":["2","Нечётная"],"url":"?group=513&date=2026-08-07&subgroup=2"}
  network during filter click: []
after subgroup=all: {"active":["Все","Нечётная"],"url":"?group=513&date=2026-08-07"}
  network during filter click: []
```

**(ii) The filtering itself, in the browser**, driving the shipped controller
against the shipped DOM with only the server-supplied visibility lists
rewritten in place (to the exact lists the presenter was independently proven to
emit for subgroup 1/2 events — see (c)8):

```
relabelled: {"total":3,"labels":["all 1","all 2","all 1 2"]}
filter=all: {"visibleEvents":3,"visibleDots":9,"active":["Все","Нечётная"],"search":"?group=513&date=2026-08-07"}
filter=1  : {"visibleEvents":2,"visibleDots":9,"active":["1","Нечётная"],"search":"?group=513&date=2026-08-07&subgroup=1"}
filter=2  : {"visibleEvents":2,"visibleDots":10,"active":["2","Нечётная"],"search":"?group=513&date=2026-08-07&subgroup=2"}
filter=all: {"visibleEvents":3,"visibleDots":9,"active":["Все","Нечётная"],"search":"?group=513&date=2026-08-07"}
network requests during all four clicks: []
errors: []
```

Visible events drop 3 → 2 under each subgroup filter, and the empty dot appears
(9 → 10) in the cell that empties under filter 2. **Zero requests.**

### 7. Definition-of-done #7 — degraded states, live

`api_geteway` stopped, waited past `cache_fresh_ttl` (60 s):

```
=== previously-visited page (expect stale banner, data intact) ===
http:200
stale-banner   : 2
region-error   : 0
site-header    : 1
site-footer    : 2
schedule-event : 24
schedule-table : 2
banner text    : Данные от 09:28, возможно устарели
translation missing: 0

=== never-visited page (expect region error, header+footer intact) ===
http:200
region-error   : 3
stale-banner   : 0
site-header    : 1
site-footer    : 2
retry link     : /schedule?group=513&amp;date=2026-09-14
schedule-table : 0
schedule-legend: 1
translation missing: 0
```

**`/now` failing on its own** — isolated by dropping only the
`cache:portal.groups.now*` Redis entries while the schedule entry stayed warm,
so the schedule call served stale and `/now` failed hard with no cache at all:

```
http:200
http page rendered chars: 24798
schedule-event buttons  : 24
desktop table           : 1
mobile agenda events    : 1
glance cards present    : 3
region-error            : 0
stale-banner            : 2
transfers dialog        : 1
translation missing     : 0
  glance variant: ''  attrs: aria-live="polite" data-subgroup-visible="all"
  glance variant: ''  attrs: aria-live="polite" data-subgroup-visible="1" hidden="hidden"
  glance variant: ''  attrs: aria-live="polite" data-subgroup-visible="2" hidden="hidden"
```

The whole timetable still renders; all three glance cards are present and
**none** carries `is-now` — it degraded rather than claiming a class was
running. `api_geteway` was restarted afterwards and confirmed back at 200.

### 8. Definition-of-done #3 and #5 — states seed data cannot reach

`moved_in` **is** reachable from seed data and is verified live: with
`?date=2026-08-07`, occurrence `2026-08-08` comes back `state: "moved_in",
original_date: "2026-08-07"` and renders `class="schedule-event is-added"` with
"ДОБАВЛЕНО" (visible in the 1280 screenshot), and `transfers[]` has one entry so
the ПЕРЕНОСЫ button is enabled with count 1 (transfers dialog screenshot:
`+ Философия / Перенесено / 07.08 → 08.08 / 08:30`).

`cancelled`, subgroup 1/2, and a live `has_class: true` are **not** in seed data
at the current wall-clock time. I attempted to create the data (`UPDATE
schedule_events … subgroup_number`, `INSERT INTO event_transfers … 'cancelled'`)
and **the write was refused by the permission system**. I did not work around
it. Instead I exercised the same code paths non-destructively: a `rails runner`
script inside the container drives the **real** `Presenters::GroupSchedule` and
renders the **real** view partials against constructed payloads containing
subgroup-1, subgroup-2, `moved_in` and `cancelled` occurrences plus a
`has_class: true` `/now`. **60 of 60 checks passed**, no database or upstream
touched:

```
--- presenter: identity / semester / found? ---
PASS  found? / identity / semester_label / transfers count
PASS  transfer 1 added? / transfer 2 added? / transfer 2 reason
--- presenter: subgroup visibility lists ---
PASS  shared event visible for              (all 1 2)
PASS  subgroup-1 event visible for          (all 1)
PASS  subgroup-2 event visible for          (all 2)
PASS  cell 10:10 empty for (none: both subgroups own one)
PASS  cell 08:30 empty for (shared event, never empty)
PASS  cell on a day with no events is empty for all filters
--- presenter: entity state mapping ---
PASS  moved_in -> added? / state_class (is-added) / original_date
PASS  cancelled -> removed? / state_class (is-removed) / transfer_reason
--- presenter: glance states ---
PASS  viewing_today?
PASS  glance[all].state (fresh /now has_class:true)     -> :now
PASS  glance[all].subject / glance[all].time (11:35, end of time_range)
PASS  glance[1].state -> :now
PASS  glance[2].state (nothing running for this subgroup) -> :next
PASS  glance[2].subject / glance[2].time
--- presenter: glance with /now failed (must degrade, not lie) ---
PASS  glance state with /now failed is never :now
        (state was :day, subject "Общий предмет")
--- presenter: 'nothing left today' + 'other day' glances ---
PASS  glance[all].state after last class -> :done
PASS  glance[all].state on another day   -> :empty_day
--- presenter: not found (semester null) ---
PASS  found? with semester null / found? with non-numeric id
--- presenter: cache-hit shape (string keys, per landmine #14) ---
PASS  identity / glance / transfers survive a JSON round trip
--- views: real partials rendered against these events ---
PASS  desktop moved_in button carries is-added
PASS  desktop cancelled button carries is-removed
PASS  desktop moved_in note copy (Добавлено) / cancelled note copy (Убрано)
PASS  subgroup-1 button note copy (1 ПГ) / visibility list (all 1)
PASS  agenda moved_in carries is-added / agenda cancelled carries is-removed
PASS  dialog moved_in status band (event-detail-status is-added)
PASS  dialog moved_in 'Перенос' label / 'с 07.08' value
PASS  dialog cancelled status band (event-detail-status is-removed)
PASS  dialog cancelled 'Причина' label / reason text
PASS  dialog subgroup value for sub1
PASS  glance :now uses is-now + Сейчас + end time 11:35
PASS  glance :done copy (На сегодня всё) + is-finished
PASS  glance other-day empty copy (Нет занятий)
PASS  transfers dialog moved row / cancelled row / date arrow
--- views: no missing translations anywhere in the rendered fragments ---
PASS  fragments with 'translation missing'  ([])
==========================================
60 passed, 0 failed, 60 total
==========================================
```

Separately, `has_class: true` **is** real in this dataset — the upstream
endpoint takes a `datetime` override, so the data source the glance depends on
is confirmed, it just cannot be hit at the actual current time (today is a
Sunday):

```
=== /now with a datetime during a real class (Mon 2026-08-10 10:30 MSK) ===
"time_range": "10:10 - 11:35",  "has_class": true,
"current": [{ "room_number": "1207", "subject_name": "Скриптовые языки программирования",
              "activity_type_name": "лекция", "subgroup_number": 0, "teachers": [] }]
=== /now at a time with no class (Mon 2026-08-10 06:00 MSK) ===
has_class= False time_range= None
```

### 9. Interaction, and existing behaviour intact

Event detail dialog (clicked in a real browser):

```
event dialog: { "open": true, "ariaHidden": "false",
 "focused": "schedule-panel-close", "bodyModal": true,
 "status": "лекция · 1315", "name": "Скриптовые языки программирования",
 "items": ["Группа=СИ 3 курс","Преподаватель=Щербаков С. В.","Аудитория=1315",
           "Время=Вторник · 08:30–09:55","Подгруппа=Все"],
 "pointerEventsOfClosed": "none" }
after Escape, open dialogs: 0  modal-open: false
transfers dialog: { "open": true,
 "rows": ["transfer-row is-added | + Философия Перенесено 07.08 → 08.08 08:30"] }
after backdrop click, open: 0
keyboard/filter: {"tag":"BUTTON","focused":true}
```

`pointerEventsOfClosed: "none"` confirms landmine #11's requirement on the
closed overlays.

Shared dialogs, home wipe, search — all still working from/on these pages:

```
auth dialog opened from /schedule: {"name":"auth","ariaHidden":"false","focus":"BUTTON.dialog__close","modal":true}
after Escape, open dialogs: 0 | focus returned to: profile-button
contacts dialog from footer: contacts
home floors: ["3 F Группа ↗","2 F Преподаватель ↗"]
after floor click url: http://localhost:3000/group | errors: []
teacher search url: http://localhost:3000/teacher?query=%D0%B0 | entity cards: 5 | errors: []
```

**Record-page regression check** (the two things this task changed underneath
them — the extended entity and the extracted selector CSS). `/teacher/395` is
used because teacher 1 has zero occurrences in this window (its correct empty
state), while 395 has real ones:

```
/teacher/395 @1280: { "scheduleSection":1, "weekButtons":2,
 "buttonStyle":{"textTransform":"uppercase","fontSize":"10px","fontWeight":"800","minHeight":"31px","cursor":"pointer","position":"relative"},
 "selectorBox":{"display":"flex","borderTopWidth":"1px","padding":"4px","gap":"5px","background":"rgb(255, 255, 255)"},
 "formDisplay":"contents", "table":1, "mobile":0, "events":1,
 "emptyState":0, "regionError":0, "hOverflow":0 }
/teacher/395 @375 : { ..., "buttonStyle":{...,"fontSize":"8.5px","minHeight":"33px"},
 "selectorBox":{...,"gap":"4px"}, "table":0, "mobile":1, "events":1, "hOverflow":0 }
/room/1 status: 200
/room/1 @1280: { "scheduleSection":1, "weekButtons":2,
 "buttonStyle":{"textTransform":"uppercase","fontSize":"10px","fontWeight":"800","minHeight":"31px",...},
 "selectorBox":{"display":"flex","borderTopWidth":"1px","padding":"4px","gap":"5px",...},
 "formDisplay":"contents", "table":0, "emptyState":1, "regionError":0, "hOverflow":0 }
csp: [] | errors: [] | failed: []
```

The selector's computed styles (uppercase / 10px / 800 / 31px min-height,
flex + 1px border + 4px padding + 5px gap, `display: contents` on the form, and
the 820px overrides at 8.5px / 33px / 4px gap) are unchanged by the extraction,
on both record pages and at both widths. `/room/1` returns 200 with its schedule
section and its correct empty state (no region error → the admin call
succeeded), so the extended `Entities::ScheduleEvent` still deserializes the
admin room payload.

### 10. No `translation missing`

```
/                                          0
/teacher                                   0
/group                                     0
/teacher/1                                 0
/schedule?group=513                        0
/schedule?group=513&date=2026-08-07        0
/schedule?group=999999                     0
```

Plus the stale page, the region-error page, the `/now`-failed page (0 each,
shown in §7) and all rendered dialog/glance fragments (§8).

### 11. Footer grep guard

```
=== _site_footer.css grep guard (must print nothing) ===
  (clean)
```

### 12. Database left untouched

The blocked write never happened; re-checked after all testing:

```
 id  | subgroup_number       id | schedule_event_id | original_date | transfer_type
-----+-----------------     ----+-------------------+---------------+---------------
  40 |               0       32 |                40 | 2026-08-07    | moved
  58 |               0
 103 |               0
```

Identical to the snapshot taken before testing. The only mutation anywhere was
deleting two regenerable Redis **cache** entries, which repopulated on the next
request.

---

## (d) Needs a human decision / could not verify

1. **`shared/_week_selector.html.erb` behaviour change on two existing pages.**
   The week selector on `/teacher/:id` and `/room/:id` was a no-op and now
   works. Intended, verified, but it is a change to already-reviewed pages —
   flagging it explicitly. (b)1 has the before/after browser output.

2. **`state: "cancelled"` and `subgroup_number` 1/2 are not verified
   end-to-end.** No such row exists in seed data, and my attempt to create one
   was refused by the permission system. Both are verified at the
   presenter + real-view-partial level (60/60 checks, (c)8) and the browser-side
   filter mechanism is verified against real DOM ((c)6ii), but the full
   upstream → gateway → entity path for those two specific field values has not
   been exercised against a live payload. Note the risk is low: `moved_in` *was*
   exercised live through that whole path, and it reads `state`,
   `original_date` and `transfer_reason` from the same `.from_row` mapping, with
   `subgroup_number` present and correct (`0`) on every live occurrence.
   **If you want end-to-end proof, the two statements are:**
   ```sql
   UPDATE schedule_events SET subgroup_number = 1 WHERE id = 58;
   UPDATE schedule_events SET subgroup_number = 2 WHERE id = 103;
   INSERT INTO event_transfers (schedule_event_id, created_by_id, original_date, transfer_type, reason)
   VALUES (58, '483b7037-9308-4098-be00-69484bb69a0b', '2026-08-10', 'cancelled', 'Проверка отмены');
   ```
   then `GET /schedule?group=513&date=2026-08-07` (flush
   `cache:portal.groups.schedule*` or wait 60 s first), and revert with
   `subgroup_number = 0` on both rows plus deleting the inserted transfer.

3. **A live "class in progress" glance could not be produced**, because the
   real current time is a Sunday with no classes. The upstream `has_class: true`
   response is confirmed to exist via its `datetime` override ((c)8), and the
   presenter's `:now` branch and its rendered markup are verified against it,
   but no browser screenshot shows the almond `is-now` card driven by real
   wall-clock time.

4. **Weak focus indicator on `.schedule-selector` buttons** (pre-existing,
   inherited from the reference): focus-visible sets `outline: none` and relies
   on a 4 %-alpha background tint. I did not change it, because it is now a
   shared component used by already-reviewed pages and altering it would change
   their appearance. My own new controls (`.schedule-event`,
   `.mobile-agenda-event`, `.mobile-day-tab`) do have real 2 px focus outlines.
   Worth a deliberate decision.

5. **`Resilience::Call::KNOWN_KEYS` is still missing two pre-existing keys** —
   `portal.teachers.schedule` and the admin room schedule key. I added only my
   own two. `portal.teachers.schedule` happens to appear in `/readyz` output via
   the discovered-key merge, but only after something exercises it.

6. **Drive C: is very tight.** It was at 0.2 GB free and had killed the Docker
   daemon; it is at ~6.6 GB after I cleared the npm cache and the Docker build
   cache. Nothing in the repo caused it, but the next build will eat into that
   again. The WSL VHDX does not shrink on its own, so a
   `wsl --manage docker-desktop-data --set-sparse true` / compaction pass is
   probably the real fix — that is a host-level decision, so I did not do it.

7. **No automated test suite was added.** `apps/schedule_web` has no `test/` or
   `spec/` directory and no test gems in the Gemfile, so there was no existing
   harness to extend. The 60-check probe described in (c)8 lives in my scratch
   directory, not in the repo. If you want it kept, say so and I will land it as
   a proper test file (which would mean introducing a test framework to this
   service).

---

# Fix round 1

Addresses review findings **I-1**, **I-2** and **M-1**. The
`shared/_week_selector.html.erb` fix was re-verified by the reviewer as correct
and correctly scoped, and is unchanged. Findings M-2 … M-9 were deferred by the
coordinator and are untouched.

Container rebuilt (`docker compose build schedule_web && docker compose up -d
schedule_web`) after every CSS/ERB change and before each measurement below.
Nothing committed or staged.

## Files changed in this round

| File | Change |
|---|---|
| `app/assets/stylesheets/components/_schedule_selector.css` | **I-1.** Gained its own `@media (max-width: 820px)` block holding the two mobile rules moved out of `_schedule.css`. |
| `app/assets/stylesheets/components/_schedule.css` | **I-1** the two `.schedule-selector` rules removed (pointer comment left in their place). **M-1** `.schedule-panel-close` hover and focus-visible split, focus-visible given a real 2px outline. **M-1 bundle** `.event-detail-item .event-detail-link:focus-visible` likewise. **I-2** new `.schedule-out-of-period*` block. |
| `app/lib/presenters/group_schedule.rb` | **I-2.** `found?` rewritten; new `out_of_period?`, `available_periods`, `Period` struct, `anchor_date` reader; private `periods_result` / `configured_periods` / `group_known?`; constructor takes `periods_gateway:` (defaults to the already-registered `gateways.group_periods`). |
| `app/views/schedules/show.html.erb` | **I-2.** Three-state branch, out-of-period checked first. |
| `app/views/schedules/_out_of_period.html.erb` | **I-2, new.** |
| `config/locales/ru.yml` | **I-2.** Two small targeted additions: `date.formats.day_month_year`, and the `schedule.out_of_period.*` block. |

`config/routes.rb` and `application.css` were **not** touched, so there was no
overlap with the concurrent `/guide` task. `ru.yml` was the one shared file I
needed; I re-read it immediately before editing and used two targeted edits. The
sibling task subsequently rewrote that file, so I re-verified afterwards that
both tasks' keys coexist (see §Locale coexistence below).

---

## I-1 — the shared selector's mobile rules now live with the control

**Audit first.** `_schedule.css` contained exactly the two rules the reviewer
found, and no other foreign selectors:

```
=== all .schedule-selector occurrences in _schedule.css ===
24: *   * The reference's `.schedule-selector` rules, which now live in
531:  .schedule-selector { width: 100%; min-height: 43px; padding: 4px; gap: 4px; }
532:  .schedule-selector button { flex: 1; min-height: 33px; padding: 0 6px; font-size: 8.5px; }

=== any other cross-component selectors in _schedule.css ===
  (none)
```

Both were moved into a new `@media (max-width: 820px)` block in
`_schedule_selector.css`. 820px is the breakpoint at which *both* callers swap
their desktop table for a mobile layout — confirmed, not assumed:

```
=== media breakpoints in _teacher_record.css ===
370:@media (max-width: 820px) {
481:@media (max-width: 430px) {
```

After the move, `_schedule.css` names `.schedule-selector` only in comments, and
the rules exist in exactly one file:

```
=== confirm the moved rules live only in _schedule_selector.css ===
_schedule_selector.css:77:  .schedule-selector { width: 100%; min-height: 43px; padding: 4px; gap: 4px; }
_schedule_selector.css:78:  .schedule-selector button { flex: 1; min-height: 33px; padding: 0 6px; font-size: 8.5px; }
```

Braces balanced in all three files (comments stripped before counting):

```
_schedule.css                braces 157/157 OK  media-blocks=3
_schedule_selector.css       braces 11/11 OK  media-blocks=1
_teacher_record.css          braces 88/88 OK  media-blocks=2
```

### Ownership proven in the browser, not just by grep

Enumerating `document.styleSheets` (descending through `@import` and `@media`)
for every rule matching `.schedule-selector`, on both callers at 375px — all
nine rules, **including both media-query rules**, come from
`_schedule_selector-*.css`; none from `_schedule-*.css`:

```
/teacher/395 @375 — .schedule-selector rules, by stylesheet:
    _schedule_selector-6e221b4b.css  [all]                 .schedule-selector
    _schedule_selector-6e221b4b.css  [all]                 .schedule-selector button
    _schedule_selector-6e221b4b.css  [all]                 .schedule-selector button:hover, .schedule-selector button:focus-visible
    _schedule_selector-6e221b4b.css  [all]                 .schedule-selector button.is-active
    _schedule_selector-6e221b4b.css  [all]                 .schedule-selector button.is-active:hover, .schedule-selector button.is-active:focus-visible
    _schedule_selector-6e221b4b.css  [all]                 .schedule-selector button.is-current::after
    _schedule_selector-6e221b4b.css  [all]                 .schedule-selector button.is-active.is-current::after
    _schedule_selector-6e221b4b.css  [(max-width: 820px)]  .schedule-selector
    _schedule_selector-6e221b4b.css  [(max-width: 820px)]  .schedule-selector button

/schedule @375 — identical list, same stylesheet.
```

### Computed sizing unchanged on all three callers

Values at 375px are identical to the pre-fix measurements recorded in (c)9:

| Page | width | box min-h | box gap | box width | btn min-h | btn font | btn pad-l | btn flex-grow |
|---|---|---|---|---|---|---|---|---|
| `/teacher/395` | 1280 | auto | 5px | 159px | 31px | 10px | 12px | 0 |
| `/teacher/395` | 375 | **43px** | **4px** | **357px** (= parent) | **33px** | **8.5px** | **6px** | **1** |
| `/schedule` | 1280 | auto | 5px | 124px | 31px | 10px | 12px | 0 |
| `/schedule` | 375 | **43px** | **4px** | **357px** | **33px** | **8.5px** | **6px** | **1** |
| `/room/1` (signed in) | 1280 | auto | 5px | 159px | 31px | 10px | 12px | 0 |
| `/room/1` (signed in) | 375 | **43px** | **4px** | **357px** | **33px** | **8.5px** | **6px** | **1** |

Raw output for the room record page, with the sign-in and status asserted this
time (the first attempt reported `null` because the sign-in step had silently
not happened):

```
sign-in landed on: http://localhost:3000/ | any dialog message: "admin@milk-schedule.local"
/room/1 @375 -> status 200 url http://localhost:3000/room/1
  selector styles: {"boxWidthPx":357,"parentWidthPx":357,"boxMinHeight":"43px","boxPadding":"4px","boxGap":"4px","btnMinHeight":"33px","btnFontSize":"8.5px","btnPaddingLeft":"6px","btnFlexGrow":"1"}
/room/1 @1280 selector styles: {"boxWidthPx":159,"parentWidthPx":1180,"boxMinHeight":"auto","boxPadding":"4px","boxGap":"5px","btnMinHeight":"31px","btnFontSize":"10px","btnPaddingLeft":"12px","btnFlexGrow":"0"}
  errors: [] failed: []
```

Cascade note: the moved rules now load *before* `_teacher_record.css` rather
than after. Checked for collisions — that file's own 820px block sets
`.teacher-week-selector { width }`, `.teacher-week-selector .schedule-selector-form
{ flex }` and `.teacher-week-selector button { width }`; the only property in
common is `width: 100%` with an identical value. The measurements above confirm
no behavioural change.

---

## I-2 — out-of-period is now 200 with working navigation, not a 404

**Reproduced the reviewer's finding first.** `meta.semester: null` really does
cover two different situations, and `/periods` really does separate them:

```
=== schedule: semester null for a VALID group with an out-of-period date? ===
  date=2026-08-07   semester= semester | occurrences= 6
  date=2026-12-15   semester= null     | occurrences= 0
  date=2025-01-05   semester= null     | occurrences= 0

=== periods: the date-independent oracle ===
  group=513      periods= 2 ['semester', 'lecture_week'] http 200
  group=1        periods= 1 ['exam_session']             http 200
  group=999999   periods= 0 []                           http 200

=== current Rails behaviour (the bug) ===
  /schedule?group=513                    -> 200
  /schedule?group=513&date=2026-12-15    -> 404      <-- wrong
  /schedule?group=999999                 -> 404
```

### Design

`found?` returns true for a group `/periods` knows; `out_of_period?` is the
subset of that where the window came back without a semester. Three deliberate
choices:

* **The periods call is lazy, not fanned out.** It is only needed when
  `meta.semester` is nil — the rare path. Fetching it concurrently would add a
  third upstream call to *every* ordinary page view to answer a question that
  view never asks. Mission §7's "fan out rather than serialize" is about calls
  you know you need, so the happy path still makes exactly the two concurrent
  calls it did before, and the rare path makes one extra.
* **A failed periods call resolves to "assume the group is real."** Showing a
  live group an out-of-period page with working navigation is a much smaller
  error than hiding it behind a 404; a bogus id lands on the same page rather
  than a wrong one.
* **The state is a junction, not a dead end.** Every period the group *does*
  have is a link to its own start date, plus a link back to the picker. Period
  names reuse the existing `group_picker.schedule_types.*` keys (with
  `default: kind`, so an unexpected `period_kind` degrades to its raw name
  instead of "translation missing").

### Statuses after the fix

```
  /schedule?group=513                      -> 200
  /schedule?group=513&date=2026-12-15      -> 200   <-- was 404
  /schedule?group=513&date=2025-01-05      -> 200   <-- was 404
  /schedule?group=513&date=2026-08-07      -> 200
  /schedule?group=999999                   -> 404   <-- still 404
  /schedule?group=999999&date=2026-12-15   -> 404   <-- still 404
  /schedule?group=abc                      -> 404
  /schedule?                               -> 404
```

### The page itself

```
=== out-of-period page contents (group 513, date 2026-12-15) ===
  http:200
  site-header                        1
  site-footer                        2
  schedule-out-of-period block       1
  schedule-table (must be 0)         0
  generic empty-day (must be 0)      0
  not_found copy (must be 0)         0
  translation missing                0
  title      : На эту дату расписание не составлено
  requested  : Запрошенная дата: 15.12.2026
  period links: [('/schedule?date=2026-07-29&group=513', 'ОБЫЧНОЕ', 'с 29.07'),
                 ('/schedule?date=2026-09-01&group=513', 'ЛЕКЦИОННАЯ НЕДЕЛЯ', 'с 01.09')]
  picker link: ('/group', 'Выбрать другую группу')

=== 404 page still intact for a genuinely unknown group ===
  schedule-not-found: 1 | out-of-period: 0 | header: 1 | footer: 2
```

Real browser, both widths — shell intact, no overflow, footer pinned, zero
console errors and zero CSP violations:

```
@1280: { "hOverflow": 0, "header": true, "backButton": true, "profileButton": true,
         "footerPinned": true,
         "title": "На эту дату расписание не составлено",
         "requested": "Запрошенная дата: 15.12.2026",
         "links": ["/schedule?date=2026-07-29&group=513 | ОБЫЧНОЕ с 29.07",
                   "/schedule?date=2026-09-01&group=513 | ЛЕКЦИОННАЯ НЕДЕЛЯ с 01.09",
                   "/group | Выбрать другую группу"],
         "noTable": true }
  errors: [] csp: [] failed: []
@375:  same, hOverflow 0, footerPinned true, errors: [] csp: [] failed: []
```

**The offered navigation actually works** (waited on the real URL change rather
than on network-idle, which is what made the first attempt look like a no-op):

```
first offered period -> {"url":"http://localhost:3000/schedule?date=2026-07-29&group=513",
                         "identity":"СИ 3 курс","kind":"1 семестр 2025/2026",
                         "events":1,"table":1,"outOfPeriod":0}
picker link       -> http://localhost:3000/group
header back button -> http://localhost:3000/group
```

Screenshot inspected at 1280: display-font heading, printed-label captions,
hairline period rows, bordered picker button, header back arrow and profile
control present, footer pinned — the page reads as part of this design, not as
an error screen.

---

## M-1 — dialog close button (and, bundled, the link beside it)

`.schedule-panel-close`'s hover and focus states were one rule with
`outline: none`. Split: `:hover` keeps the dim, `:focus-visible` gets
`opacity: 1` plus `outline: 2px solid var(--ink); outline-offset: -2px` — the
same treatment the page's other new controls already had.

My first measurement showed `matchesFocusVisible: false` — because I opened the
dialog with a **mouse** click, and Chromium does not apply `:focus-visible` to a
programmatic `.focus()` following a pointer interaction. Re-measured on the real
keyboard path (focus an event button, press Enter):

```
event button focused by keyboard: {"cls":"schedule-event","matchesFocusVisible":true,
  "outlineStyle":"solid","outlineWidth":"2px","outlineColor":"rgb(21, 19, 17)","outlineOffset":"-2px"}
close button after Enter-opened dialog: {"cls":"schedule-panel-close","matchesFocusVisible":true,
  "outlineStyle":"solid","outlineWidth":"2px","outlineColor":"rgb(21, 19, 17)","outlineOffset":"-2px","opacity":"1"}
after Shift+Tab back to close  : {"cls":"schedule-panel-close","matchesFocusVisible":true,
  "outlineStyle":"solid","outlineWidth":"2px","outlineOffset":"-2px","opacity":"1"}
```

**Bundled addition (not a review finding).** Tabbing on from the close button
revealed the same defect one stop later, inside the same new dialog: the teacher
link had `outlineStyle: "none"` and was distinguished only by a colour shift on
text already at 0.52 opacity. Fixed, scoped to `.event-detail-item` so the
record pages' own use of `.event-detail-link` is untouched:

```
before: {"cls":"event-detail-link","matchesFocusVisible":true,"outlineStyle":"none","outlineWidth":"3px","opacity":"0.52"}
after : {"cls":"event-detail-link","matchesFocusVisible":true,"outlineStyle":"solid","outlineWidth":"2px","outlineColor":"rgb(21, 19, 17)","outlineOffset":"2px"}
```

One measurement to read precisely: the hover probe reported
`{"opacity":"1","outlineStyle":"solid"}`, because the button was still
keyboard-focused while hovered — `:focus-visible` is declared after `:hover` at
equal specificity, so the ring correctly survives a hover instead of being
dimmed away. The plain `:hover { opacity: .45 }` path (hover without focus) is
still in the stylesheet; I did not measure it in isolation.

---

## Locale coexistence with the concurrent `/guide` task

That task rewrote `ru.yml` after my edits. Verified in-container that both
tasks' keys resolve and the file still parses:

```
  schedule.out_of_period.title                 На эту дату расписание не составлено
  schedule.out_of_period.requested             Запрошенная дата: %{date}
  schedule.out_of_period.periods_intro         Расписание составлено на другие периоды:
  schedule.out_of_period.from                  с %{date}
  schedule.out_of_period.pick_another          Выбрать другую группу
  schedule.empty                               На выбранной неделе занятий нет
  schedule.day_empty                           Нет занятий
  schedule.not_found                           Расписание не найдено
  group_picker.schedule_types.semester         ОБЫЧНОЕ
  group_picker.schedule_types.lecture_week     ЛЕКЦИОННАЯ НЕДЕЛЯ
  date.formats.day_month_year                  %d.%m.%Y
  (sibling) guide_page.title                   Гайд
  total top-level keys: 19
```

---

## Re-verification: presenter/view probe, now 86 checks

The probe from round 0 was extended for the new contract (a fake periods
gateway) and the new states. Two round-0 assertions were *replaced* because they
encoded the bug being fixed (`found?` with `semester: null` expected `false`).

```
--- presenter: found? / out_of_period? / 404 (fix round 1, I-2) ---
PASS  out-of-period: found? (must be true -> 200)
PASS  out-of-period: out_of_period?
PASS  out-of-period: available period kinds            (["semester","lecture_week"])
PASS  out-of-period: first period path                 (/schedule?date=2026-07-29&group=513)
PASS  out-of-period: anchor_date echoed for the copy
PASS  unknown group: found? (must stay false -> 404)
PASS  unknown group: out_of_period?
PASS  periods call failed: found? (must be true -> 200)
PASS  periods call failed: out_of_period?
PASS  periods call failed: no period links to offer
PASS  normal window: out_of_period?
PASS  normal window: found?
PASS  failed schedule: found? (region_error at 200)
PASS  failed schedule: out_of_period?
PASS  found? with non-numeric id
PASS  out_of_period? with non-numeric id

--- views: out-of-period state (fix round 1, I-2) ---
PASS  out-of-period title copy
PASS  out-of-period echoes the requested date
PASS  out-of-period lists the semester period
PASS  out-of-period lists the lecture week
PASS  out-of-period links to the period start
PASS  out-of-period shows the period start date
PASS  out-of-period offers the picker
PASS  out-of-period keeps its own root class
PASS  out-of-period renders no schedule table
PASS  out-of-period renders no generic empty-day copy
PASS  no-links variant still offers the picker
PASS  no-links variant renders no empty <ul>

--- views: no missing translations anywhere in the rendered fragments ---
PASS  fragments with 'translation missing'

==========================================
86 passed, 0 failed, 86 total
==========================================
```

## Re-verification: full regression sweep

```
=== routes (incl. the sibling task's /guide) ===
  /                200      /group                                200
  /up              200      /guide                                200
  /readyz          200      /teacher/1                            200
  /teacher         200      /teacher/395                          200
  /room            302      /room/1                               302
  /schedule?group=513                        200
  /schedule?group=513&date=2026-08-07        200
  /schedule?group=513&date=2026-12-15        200
  /schedule?group=999999                     404
  /schedule?group=abc                        404
  /schedule                                  404

=== translation missing (all 0) ===
  / 0 | /teacher 0 | /group 0 | /guide 0 | /teacher/1 0 | /teacher/395 0
  | /schedule?group=513 0 | ...&date=2026-08-07 0 | ...&date=2026-12-15 0
  | /schedule?group=999999 0

=== footer grep guard ===
  (clean)

=== every asset 200 on the changed pages, plus every component @import ===
  (no FAIL lines = all 200)
```

Zero console errors and zero CSP violations on every page measured in this round.

---

## Still open after this round

Unchanged from section (d): the `_week_selector` behaviour change on two
existing pages (reviewed and approved), `state: "cancelled"` / subgroup 1-2 not
verified end-to-end because the DB write was refused by the permission system,
no live "class in progress" glance (today is a Sunday), the inherited weak focus
tint on `.schedule-selector` buttons themselves, the two pre-existing
`KNOWN_KEYS` omissions, host disk pressure on C:, and no test file landed (the
probe is now 86 checks and still lives in the scratch directory).

One item added by this round:

* **The out-of-period page cannot show the group's own name.** Neither
  `/periods` nor a semester-less `/schedule` window carries
  `group_display_name`, so the state leads with its message rather than with
  "СИ 3 курс". Everything else (header, back link, period links, picker) is
  present. Showing the code would need either a `group_display_name` on the
  periods payload upstream or a separate lookup; I judged neither worth it for
  this state, but it is a visible difference from the normal page's identity
  strip.
