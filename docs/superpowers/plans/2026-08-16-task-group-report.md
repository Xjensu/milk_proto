# Task Report — `/group` (faculty → course → group → schedule-type picker)

Implements `docs/superpowers/plans/2026-08-16-task-group-brief.md` under the
ground rules of `docs/superpowers/plans/2026-08-16-remaining-public-pages-migration-prompt.md`.

All work is in `apps/schedule_web` (its own nested git repo). **Nothing was
staged or committed** — verified below.

---

## (a) Files created / modified, by layer

All paths relative to `apps/schedule_web/`.

### Entities
| File | Status |
|---|---|
| `app/lib/entities/faculty.rb` | **new** — `Dry::Struct` over the JSON:API faculty resource (`id`, `name`, `short_name`), plus `#monogram` / `#label`. |

### Gateways (the only code that reaches the network)
| File | Status |
|---|---|
| `app/lib/gateways/portal/faculties_gateway.rb` | **new** — `KEY = "portal.faculties.index"`, `#index` → `GET /public/faculties?page=1&per_page=100`, flattened via `Gateways::JsonApi.collection`, entity hashes out. |
| `app/lib/gateways/portal/groups_gateway.rb` | **new** — `KEY = "portal.groups.by_faculty"`, `#for_faculty(faculty_id:, date: nil)` → `GET /public/faculties/:id/groups`, `deep_symbolize_keys` passthrough (non-JSON:API shape, mirrors `TeacherScheduleGateway`). |
| `app/lib/gateways/portal/group_periods_gateway.rb` | **new** — `KEY = "portal.groups.periods"`, `#periods(group_semester_id:)` → `GET /public/groups/:id/periods`, same passthrough style. |

### Operations
| File | Status |
|---|---|
| `app/lib/operations/faculty_catalog_with_groups.rb` | **new** — fans the catalog read and the groups read out concurrently as two `Async::Task`s and returns the pair of Results **unmerged**, so a degraded groups list cannot drag the catalog into the same state. |

### Presenters
| File | Status |
|---|---|
| `app/lib/presenters/faculty_catalog.rb` | **new** — `< Presenters::Base`. `faculties`, `empty?`, `find`, `index_of`, `neighbour` (cyclic carousel). Accepts an already-fetched `result:` so the detail page does not re-fire the call. |
| `app/lib/presenters/faculty_detail.rb` | **new** — owns two independently-wrapped regions (`catalog`, `groups_region`); exposes `faculty`, `found?`, `previous_faculty`, `next_faculty`, `active_index`, `groups_by_course`, `courses`, `active_course`, `empty?`, `group_label`, `group_id`. |
| `app/lib/presenters/group_periods.rb` | **new** — `< Presenters::Base`. Fixed four-kind vocabulary, `available?` / `selected?` per type, `confirm_path` precomputed per type, `default_kind` (semester else first). |

### Controllers / routes
| File | Status |
|---|---|
| `app/controllers/groups_controller.rb` | **rewritten** — `index`, `faculty`, `periods`. Each frame action renders the frame partial for a `turbo_frame_request?` and the whole page otherwise. |
| `config/routes.rb` | **modified** — added `group_faculty` (`GET /group/:faculty_id`) and `group_periods` (`GET /group/:faculty_id/groups/:group_semester_id/periods`), both `\d+`-constrained. |

### Views
| File | Status |
|---|---|
| `app/views/groups/index.html.erb` | **rewritten** (was the "in development" stub). |
| `app/views/groups/faculty.html.erb` | **new** — full-page render with the panel already open (direct link / no-JS path). |
| `app/views/groups/_scene.html.erb` | **new** — grid + overlay shell, shared by both pages. |
| `app/views/groups/_detail.html.erb` | **new** — the `faculty_detail` Turbo Frame's whole content. |
| `app/views/groups/_schedules.html.erb` | **new** — the nested `faculty_schedules` Turbo Frame. |
| `app/views/shared/_region_error.html.erb` | **modified, additive** — optional `compact: true` local. |

### JavaScript
| File | Status |
|---|---|
| `app/javascript/controllers/faculty_controller.js` | **new** — open/close, Escape, course filtering, group + type selection, focus restoration across frame swaps, `pageshow`/snapshot reset. |

### Stylesheets
| File | Status |
|---|---|
| `app/assets/stylesheets/components/_faculty_picker.css` | **new** — port of `schedule_home/styles.css` 1205–1670. |
| `app/assets/stylesheets/application.css` | **modified** — one `@import url("components/_faculty_picker.css");` line (the `url(...)` form, per landmine #4). |
| `app/assets/stylesheets/components/_region_error.css` | **modified, additive** — `.region-error--compact`. |

### Wiring / infrastructure
| File | Status |
|---|---|
| `config/initializers/container.rb` | **modified** — three lazy registrations appended (`gateways.faculties`, `gateways.groups`, `gateways.group_periods`). |
| `app/lib/resilience/call.rb` | **modified** — three keys appended to `KNOWN_KEYS` so `/readyz` reports real breaker state from a cold process. |
| `config/locales/ru.yml` | **modified** — `group_search:` (2 dead keys) replaced by `group_picker:`; existing comment blocks untouched. |

---

## (b) Deviations from the brief / mission doc, with reasoning

### B1. The one flagged judgment call — Turbo Frames vs. full navigation

**Built: the brief's recommended nested Turbo Frames, with progressive
enhancement as the fallback rather than as a separate design.**

- `GET /group` renders the catalog eagerly plus an empty
  `<turbo-frame id="faculty_detail">`.
- `GET /group/:faculty_id` renders the whole panel. Faculty cards and carousel
  arrows are `<a … data-turbo-frame="faculty_detail">`.
- `GET /group/:faculty_id/groups/:group_semester_id/periods` renders the nested
  `<turbo-frame id="faculty_schedules">`. Group links target it.
- Course selection is pure client-side filtering (no request), exactly as the
  brief specified.

Why this rather than the two-full-navigation fallback: the detail panel is a
`position: fixed` overlay with a CSS open/close transition. A full navigation
per step would re-render the overlay from scratch on every course/group click,
which both loses the transition and re-fetches the whole catalog four times to
change one row.

Three refinements to the recommended shape, each because the code demanded it:

1. **The Stimulus controller lives outside both frames.** `.faculty-scope`
   wraps the grid *and* the overlay and carries `display: contents` (landmines
   #6 and #9). Every target it touches is inside a frame that gets replaced;
   the controller element itself never is, so no listener is ever lost to a
   frame swap. Verified by clicking through two frame swaps and re-testing
   every control afterwards (see (c) §4).
2. **Every control is a real link, and the frame endpoints render full pages
   when they are not frame requests.** So the whole picker works with
   JavaScript disabled — the course links carry `?course=N` and the server
   filters, the group links render the periods into a full page, and the
   schedule-type links go straight to their own `/schedule` URL. The Stimulus
   controller only intercepts. This is why the brief's "loading status" locale
   strings (`Загружаем факультеты…`, `Загружаем курсы…`) were **not** added:
   the data is server-rendered, so those states do not exist. Their React
   counterparts have no meaning here.
3. **No `data-turbo-action="advance"`.** The URL stays `/group` while the panel
   is open, matching the React component exactly (its `detailOpen` was
   component state and never touched the URL). Adding history entries would
   have meant reconciling Turbo snapshot restores with manually-toggled DOM
   classes for no behavioural gain.

### B2. The reference's scroll-revealed fixed footer is **not** ported

`schedule_home/styles.css` gives `.faculty-page .site-footer`
`position: fixed` + `left: 50%` + a `translate`, and pads every viewport with
84px of dead scroll range (`min-height: calc(100svh + 84px)` on both
`.faculty-page` and `.faculty-scene`, plus a `.faculty-detail::after` spacer)
purely to make that slide-in possible. That is verbatim the pattern
`components/_site_footer.css` exists to prevent (mission landmine #5). The
footer stays in flow; the three scroll-range hacks are dropped with it.
Verified: `_site_footer.css` still has no `position: fixed` and no `transform`
substring, and the footer sits at the viewport bottom on the short `/group`
page at both widths.

### B3. `animateProductToDetail` (the flyer) skipped — as the brief recommended

`.faculty-product-flyer` is not ported. The `.faculty-detail.is-open`
opacity/visibility transition carries the open, with the reference's asymmetric
`visibility 0s linear <dur>` timing kept intact (landmine #10).

### B4. Centring by auto margins instead of `translateX(-50%)`

`.faculty-detail-product` and `.faculty-detail-options` are centred with
`left: 0; right: 0; margin-inline: auto` rather than the reference's
`left: 50%; transform: translateX(-50%)`. Identical result; placement no longer
shares an element with a repositioning property, so no later rule can silently
discard half of it. (The reference itself already did this for
`.faculty-detail-options` in its "2026-08 exact centering" block.)

### B5. `grid-template-columns: repeat(auto-fit, …)` instead of `repeat(6, 1fr)`

The mockup hardcodes seven faculties and always fills its rows. The live
catalog has three, and a fixed six-column grid pinned them to the left half of
a 1280px screen with a void beside them (screenshotted before the change).
`repeat(auto-fit, minmax(0, calc(100% / 6)))` + `justify-content: center`
collapses the unused tracks and centres what remains; with six or more
faculties it is byte-for-byte the reference layout. Same substitution applied
to the 1180px (÷4) and 780/460px (÷2) breakpoints.

### B6. `.lookup-status` had to be defined, not reused

The brief says `.lookup-status` is "already used elsewhere in this codebase".
It is not — `grep` finds it only in `apps/frontend/src/styles.css` (the React
app). It is defined here in `_faculty_picker.css`, scoped under `.faculty-page`,
matching the React rule (muted caption, 12px).

### B7. `shared/_region_error` gained a `compact:` variant

The panel's course row cannot give up `.region-error`'s 200px `min-height`
inside a `position: fixed` options stack — it would overflow the viewport
bottom where nothing can scroll it into view. Rather than have the faculty
component restyle another component (mission §9), the shared partial and its
own stylesheet gained an opt-in `region-error--compact` modifier. Additive:
existing call sites are unchanged. This is how the brief's required "Повторить"
retry button is delivered — `_region_error` already renders exactly that
string, and its `data-turbo-frame="_self"` retry link re-navigates the
containing frame, i.e. the React `groupsRetryToken` behaviour without a page
reload.

### B8. Hardcoded colours replaced with tokens; one decoration dropped

`#fff` → `var(--paper)`, `#191817` → `var(--ink)`, `#d7d7d7` →
`var(--line-soft)`. The monogram's `text-shadow: 0 10px 24px rgba(21,19,17,.05)`
is dropped rather than introduce a shadow token for a value that is invisible
at 5% opacity (mission §6: no new hardcoded colours).

### B9. Dead `group_search:` locale block removed rather than left

Its only meaningful key was the "in development" notice written on the false
premise this task exists to correct. `grep` confirmed nothing referenced it.
The file's explanatory comment blocks are untouched, and the removal is
documented with a comment in place.

### B10. Faculty monogram has an initials fallback

`short_name` is nullable upstream; React renders an empty string, which would
be a blank, unidentifiable card. Falls back to the initials of `name`.

---

## Two real defects found and fixed during browser verification

Both were invisible to `curl` — the pages returned 200 with correct-looking
markup in each case.

**D1 — the panel's close button was completely dead.** `.site-header` is lifted
to `z-index: 260` (per the reference) so the profile control stays reachable
above the overlay, but this app's header is a full-width bar in normal flow,
so it also covered `.faculty-detail-close` in the same top-left corner and ate
the click. Playwright:

```
- <header class="site-header">…</header> intercepts pointer events
- retrying click action
locator.click: Timeout 30000ms exceeded.
```

Fixed the way the reference does it (`schedule_home/styles.css:43-51` gives
`.site-header` `pointer-events: none` with `pointer-events: auto` on its own
controls), scoped to `body.faculty-detail-open`. The page's back link is now
`visibility: hidden` while the panel is open rather than `opacity: 0` — at
opacity 0 it stayed in the tab order, so a keyboard visitor could tab to an
invisible control that navigates out of the panel.

**D2 — the confirm button navigated a frame instead of the page.** A bare `<a>`
inside a `<turbo-frame>` is captured by that frame. Clicking confirm loaded
`/schedule?…` into `faculty_schedules`, found no matching frame, and did
nothing; Playwright reported `confirm navigated to=http://localhost:3000/group`.
The same bug applied to the close link and (on the no-JS path) the
schedule-type links. All three now carry `data-turbo-frame="_top"`. Post-fix:
`confirm navigated to=http://localhost:3000/schedule?date=2026-07-29&group=513`.

---

## (c) Verification — real output

Environment: Docker stack up, `schedule_web` rebuilt with
`docker compose build schedule_web && docker compose up -d schedule_web` after
**every** CSS/JS/ERB change (four rebuild cycles). All `curl` used `--noproxy '*'`.

### 1. Container healthy, `/up` and `/readyz`

```
schedule_web Up 50 minutes (healthy)
  200  /up
  200  /readyz

{"status":"ready","redis":true,"breakers":{"portal.teachers.index":"closed","portal.rooms.index":"closed",
 "portal.faculties.index":"closed","portal.groups.by_faculty":"closed","portal.groups.periods":"closed"}}
```

The three new breaker keys are reported from a cold process (`KNOWN_KEYS`).

### 2. Every route, including the ones this task must not break

```
  200  /
  200  /teacher
  302  /room
  200  /group
  200  /teacher/1
  302  /room/1
  200  /group/4
  200  /group/4?course=3
  200  /group/4/groups/513/periods
  200  /group/1
  404  /group/999
  /room redirect target: http://localhost:3000/?auth=1
```

`/group/999` (faculty absent from the catalog) is a genuine 404; a *failed*
catalog is not — that answers 200 with `shared/_region_error` (see §7E).

### 3. Assets — every `/assets/*` URL on every new page

```
== /group            == /group/4         == /group/4/groups/513/periods
  200  /assets/application-bfcdf840.js
  200  /assets/application-caa4d8e9.css
  200  /assets/controllers/application-3affb389.js
  200  /assets/controllers/dialog_controller-d59505fc.js
  200  /assets/controllers/entity_search_controller-7fdf7984.js
  200  /assets/controllers/faculty_controller-82084f5b.js
  200  /assets/controllers/floor_controller-6e4c1be4.js
  200  /assets/controllers/hello_controller-708796bd.js
  200  /assets/controllers/index-ee64e1f1.js
  200  /assets/stimulus-loading-1fc53fe7.js
  200  /assets/stimulus.min-4b1e420e.js
  200  /assets/turbo.min-9fd88cd5.js
```
(identical list for all three pages)

And every `@import url(...)` target inside the digested `application.css`:

```
  200  /assets/components/_site_header-e8cdad71.css
  200  /assets/components/_site_footer-608dc5b1.css
  200  /assets/components/_stale_banner-48a08253.css
  200  /assets/components/_region_error-ed6b2ed1.css
  200  /assets/components/_dialog-ae7f92a3.css
  200  /assets/components/_scene-c3ac747d.css
  200  /assets/components/_transition_layer-05da6774.css
  200  /assets/components/_entity_search-71962bb6.css
  200  /assets/components/_teacher_record-3de1e085.css
  200  /assets/components/_faculty_picker-8f7ab6ee.css
```

### 4. Real browser pass — Playwright/Chromium at 1280×800 and 375×667

Full interaction walk-through, both viewports (identical results):

```
[1280] title="Группа"
[1280] cards=3
[1280] overflow={"scrollW":1280,"clientW":1280,"bodyOverflow":false}
[1280] footer={"top":728,"bottom":800,"viewport":800,"docHeight":800}
[1280] detail open class=faculty-detail is-open
[1280] body class=faculty-page faculty-detail-open
[1280] grid aria-hidden=true
[1280] focused after open=faculty-detail-close
[1280] monogram=С
[1280] courses=3
[1280] groups visible before course pick=0
[1280] url unchanged after course click=true (http://localhost:3000/group)
[1280] groups visible after course pick=СИ-3
[1280] types=ОБЫЧНОЕ,ЛЕКЦИОННАЯ НЕДЕЛЯ,ЗАЧЁТНАЯ НЕДЕЛЯ,ЭКЗАМЕНАЦИОННАЯ НЕДЕЛЯ
[1280] disabled types=ЗАЧЁТНАЯ НЕДЕЛЯ,ЭКЗАМЕНАЦИОННАЯ НЕДЕЛЯ
[1280] selected type=ОБЫЧНОЕ
[1280] confirm href (default)=/schedule?date=2026-07-29&group=513
[1280] after type click, active=ЛЕКЦИОННАЯ НЕДЕЛЯ
[1280] confirm href (lecture)=/schedule?date=2026-09-01&group=513
[1280] url unchanged after type click=true
[1280] confirm focusable=true
[1280] after next: monogram=ЭБТ
[1280] after next: focus=faculty-detail-nav faculty-detail-next
[1280] after next: empty msg=1 "Нет групп с расписанием на эту дату."
[1280] after next: dots active index=2
[1280] after Escape: detail class=faculty-detail
[1280] after Escape: body class=faculty-page
[1280] after Escape: grid clickable=true
[1280] after close button: detail class=faculty-detail
[1280] focus returned to card=4
[1280] re-opened faculty 4: courses=3 groups visible=0
[1280] confirm navigated to=http://localhost:3000/schedule?date=2026-07-29&group=513
[1280] after back: url=http://localhost:3000/group detail class=faculty-detail body=faculty-page
[1280] CSP violations=[]
[1280] failed requests=[]
[1280] final overflow={"scrollW":1280,"clientW":1280,"bodyOverflow":false}
```

The 375 run is line-for-line equivalent (`footer={"top":607,"bottom":667,
"viewport":667}`, `overflow bodyOverflow:false`, `CSP violations=[]`).

What this covers, item by item:
- **No horizontal overflow** at either width (`scrollWidth == clientWidth`).
- **Footer pinned** to the viewport bottom of the short `/group` page
  (`bottom == viewport`, `docHeight == viewport`).
- **Course click makes no request and does not navigate** (`url unchanged=true`)
  and reveals exactly that course's groups.
- **Type buttons**: two available, two rendered as non-interactive spans, the
  default is `semester`, and picking a different one rewrites the confirm href.
- **Focus**: lands on the close control when the panel opens, follows the
  carousel arrow across a frame swap, and returns to the originating card on
  close. Confirm is focusable.
- **Escape closes**, body class is cleared, and the grid underneath is
  clickable again (`elementFromPoint` hits the card — landmine #11).
- **Every control still works after two frame swaps** (the re-open →
  course → group → confirm sequence runs after the carousel and close cycles).

**Console/CSP isolation test** (`/group` on its own vs. controls):

```
== A. /group alone
   clean
== B. control: /teacher -> /teacher/1 (existing page, Turbo Drive visit)
   clean
== C. control: home -> /teacher via floor link
   clean
== D. /group -> confirm -> /schedule (404 target)
   HTTP 404 http://localhost:3000/schedule?date=2026-07-29&group=513
   console: Failed to load resource: the server responded with a status of 404 (Not Found)
   console: Applying inline style violates the following Content Security Policy directive 'style-src …
   console: Applying inline style violates the following Content Security Policy directive 'style-src …
```

`/group` and its whole interaction are **clean**: zero console errors, zero CSP
violations, zero failed requests. The only errors in the earlier combined run
come from following the confirm link to `/schedule`, **which is not routed in
this app yet** (it is the sibling task in this same migration). Rails serves
`public/404.html`, which contains an inline `<style>` block — that is what trips
`style-src`. It is not this page's markup and it disappears the moment
`/schedule` exists. Flagged in (d).

### 5. Degraded states — `docker compose stop api_geteway`, waited 70s (> `cache_fresh_ttl` 60s)

**A. previously visited `/group`** → stale banner, catalog still rendered, 200:
```
<div class="stale-banner" role="status"><span class="stale-banner__mark" …>⚠</span>
<a class="faculty-card" … data-faculty="1" href="/group/1"> … TEST FACUL UPDATED
<a class="faculty-card" … data-faculty="4" href="/group/4"> … СТРОИТЕЛЬНЫЙ ФАКУЛЬТЕТ
<a class="faculty-card" … data-faculty="6" href="/group/6"> … ФАКУЛЬТЕТ ЭКОНОМИКИ …
--- status: 200
```

**B. previously visited detail frame** → stale banner *and* the course list:
```
<div class="stale-banner" role="status"> … <span>Данные от 13:51, возможно устарели</span>
<div class="faculty-detail-courses" role="group" aria-label="Курс">
  <a aria-label="3 курс" … href="/group/4?course=3">3</a>
```

**C. previously visited periods frame** → stale banner *and* the type row:
```
<turbo-frame id="faculty_schedules">
  <div class="stale-banner" role="status"> … <span>Данные от 13:51, возможно устарели</span>
  <div class="faculty-detail-schedules" role="group" aria-label="Тип расписания">
    <a class="is-active" data-turbo-frame="_top" … href="/schedule?date=2026-07-29&group=513">ОБЫЧНОЕ</a>
```
(the two regions degrade independently — the catalog above was stale while the
periods below were separately stale, each with its own `cached_at`)

**D. never-visited region, cold cache** (`group_semester_id=777`) → compact
region error with a working retry, page still 200:
```
<turbo-frame id="faculty_schedules">
  <div class="region-error region-error--compact" role="alert">
    <p class="region-error__message">Не удалось загрузить данные</p>
    <a class="region-error__retry" href="/group/4/groups/777/periods" data-turbo-frame="_self">Повторить</a>
--- status: 200
```

**E. catalog cache flushed from Redis + upstream down** → region error with
header and footer intact, 200:
```
<header class="site-header">
  <div class="region-error" role="alert">
    <p class="region-error__message">Не удалось загрузить данные</p>
    <a class="region-error__retry" href="/group" data-turbo-frame="_self">Повторить</a>
  <div class="site-footer-bar">
--- status: 200
```

**F.** `/readyz` stayed honest throughout, then `docker compose start api_geteway`
and `/group` recovered to 200 with all three cards.

### 6. `translation missing` — every state, zero hits

```
  /group                          -> 0 missing
  /group/4                        -> 0 missing
  /group/4?course=3               -> 0 missing
  /group/4/groups/513/periods     -> 0 missing
  /group/1                        -> 0 missing   (faculty with no groups)
  /group/6                        -> 0 missing   (faculty with no groups)
  /group/4/groups/777/periods     -> 0 missing   (all types unavailable)
```

### 7. Existing behaviour intact (regression pass, real browser)

```
guide dialog open=true activeElement=BUTTON.dialog__close
after Escape body class=
floor wipe navigated to=http://localhost:3000/teacher
teacher cards=5
after search url=http://localhost:3000/teacher?query=%D0%B8%D0%B2 cards=0
guest /room -> http://localhost:3000/?auth=1
CSP=[]
console errors=[]
```
(the 0 cards for `ив` is correct — `GET /public/teachers?q=ив` returns
`{"data":[],"included":[]}` upstream)

### 8. Footer regression grep guard

```
$ grep -nE "position:\s*fixed|transform" app/assets/stylesheets/components/_site_footer.css
  clean: no 'position: fixed' and no 'transform' substring in _site_footer.css
```

### 9. Definition-of-done checklist from the brief

| # | Item | Result |
|---|---|---|
| 1 | `/group` 200, grid renders live data | ✅ 3 faculties from the live API |
| 2 | Faculty with data (id 4) → courses → groups → types per the state machine | ✅ §4 |
| 3 | Faculty with zero groups → correct empty state | ✅ id 1 and id 6: `Нет групп с расписанием на эту дату.` |
| 4 | Confirm navigates to `/schedule?group=513&date=…` | ✅ `…/schedule?date=2026-07-29&group=513` |
| 5 | All assets 200 | ✅ §3 |
| 6 | Zero console/CSP at 1280 and 375, no h-overflow, footer pinned | ✅ §4 (with the `/schedule` 404 caveat in (d)) |
| 7 | Stale banner on revisit / region error on cold cache, header+footer intact | ✅ §5 |
| 8 | No `translation missing` anywhere | ✅ §6 |

### 10. Nothing staged, nothing committed

```
$ git -C apps/schedule_web diff --cached --name-only | wc -l
0
$ git -C . diff --cached --name-only | wc -l
0
```
(`apps/schedule_web` is an initialised repo with no commits at all; every file
there is untracked working-tree state, exactly as found.)

---

## (d) Upstream limitations, open questions, things a human should decide

1. **`/schedule` is not routed yet, so the confirm target 404s.** This is the
   sibling task in this migration, not a defect here — but it is the source of
   the only console noise anywhere near this page (Rails' `public/404.html`
   ships an inline `<style>`, which trips `style-src`). `Presenters::GroupPeriods::SCHEDULE_PATH`
   is a documented literal `"/schedule"`; **whoever lands the `/schedule` route
   should replace it with `schedule_path(group:, date:)`.** A named-route helper
   could not be used now — it would raise at render time.
   *Not verified:* that the confirm link lands on a correct, rendered schedule.
   Only that it navigates to the right URL with the right query.

2. **`GET /public/faculties` is paginated with no totals.** The contract
   (`Portal::Contracts::Faculties::IndexContract`) allows `page`/`per_page`
   capped at 100 and the serializer emits no `meta`. The catalog is a
   single non-paginated grid by design, so the gateway asks for the upstream
   maximum in one request. **A university with more than 100 faculties would
   silently lose the tail.** The clean fix is a `meta.total` upstream (the same
   gap noted for `/public/teachers` and `/public/rooms`), not a client-side page
   loop that cannot tell "full page" from "last page". No api_geteway change was
   made.

3. **`GET /public/faculties/:id/groups` is *not* paginated** —
   `GroupsContract` accepts only `faculty_id` and an optional `date`, and the
   repo returns the whole faculty. Checked because the brief flagged
   landmine #16 as possibly applying; it does not. Client-side course filtering
   is therefore safe.

4. **`GET /public/groups/:id/periods` answers 200 with an empty `periods` array
   for a group id that does not exist** (verified: `group_semester_id=777`
   returns 200). The page degrades gracefully — all four types render disabled
   and confirm points at `/schedule?group=777` — but it cannot distinguish
   "unknown group" from "group with no configured periods", so it cannot 404.
   Only reachable by hand-editing a URL. A human may want the upstream to 404
   there; no change was made.

5. **Seed data is thin.** Only faculty 4 has any group today, and it has one
   group on one course. So the multi-course branch of `groups_by_course`, the
   wrap behaviour of a long group list, and a six-or-more-faculty grid row are
   **exercised only by construction, not against real data**. The grid change in
   B5 is specifically the one that behaves differently at scale — worth a second
   look once a fuller dataset exists.

6. **`Operations::FacultyCatalogWithGroups` fans out two `Async::Task`s.** It
   demonstrably works (every page renders), but I did not *measure* that the two
   upstream calls overlap — there is no timing instrumentation in this service
   and both calls are Redis-cache hits in normal operation, so the wall-clock
   difference is inside the noise.

7. **`.faculty-grid` uses `repeat(auto-fit, minmax(0, calc(100% / N)))`.**
   Verified in Chromium at 1280 and 375. Not verified in Firefox or Safari;
   `auto-fit` with a percentage track size is well-specified but this is the one
   rule in the file that is not a literal port of a shipped reference stylesheet.

---

# Fix round 1

Review verdict: spec compliance ✅, task quality "Needs changes" — three
Important findings plus three cheap Minor ones. All six addressed. Container
rebuilt twice this round; every claim below is pasted command output.

## I1 — cross-component restyling removed (mission §9)

`_faculty_picker.css` was reaching into `.site-header`, `.profile-button`,
`.back-button` and `.site-footer` by selector — inconsistent with the very
decision recorded in (b) B7, where the shared `_region_error` component was
extended with an opt-in modifier instead of being restyled from outside.

Replaced with a **generic, reusable page-overlay contract**: a page with a
full-viewport overlay adds `overlay-open` to `<body>` and nothing else. Each
shell component reacts in its own stylesheet.

| File | Now owns |
|---|---|
| `application.css` | `body.modal-open, body.overlay-open { overflow: hidden }` plus the documented distinction between a *dialog* (`modal-open`, transient, topmost) and a *page overlay* (`overlay-open`), and the layer table (180 overlay / 200 wipe / 260 header-while-overlay / 340 dialog-while-overlay) — recorded in one place because no single component can see all of it. |
| `_site_header.css` | `body.overlay-open .site-header { z-index: 260; pointer-events: none }`, `… .profile-button { pointer-events: auto }`, `… .back-button { visibility: hidden }`. |
| `_site_footer.css` | `body.overlay-open .site-footer { visibility: hidden }` (also the I3 fix). |
| `_dialog.css` | `body.overlay-open .dialog { z-index: 340 }`. |
| `_faculty_picker.css` | **no shell selectors at all** — replaced by a comment pointing at the contract. |

Two things the generalisation exposed and fixed:

- **The header z-index is now scoped to `body.overlay-open`, not to
  `.faculty-page`.** Making it unconditional would have lifted the header above
  the floor wipe (`.transition-layer`, z-index 200) on the home page, which is
  meant to cover it. Scoping to the state where an overlay actually exists is
  both more correct and reusable.
- **A latent stacking bug: the auth dialog rendered *underneath* the faculty
  panel.** `.dialog` is z-index 100, the panel 180 — and the D1 fix
  deliberately keeps `.profile-button` clickable above the panel, so the
  visitor could open a dialog they could not see. The reference works around
  this with a per-page `z-index: 320` on its backdrops; here it is
  `_dialog.css` reacting to `overlay-open`, so it holds for the next overlay
  page too. Screenshot check confirms the dialog now paints over the blurred
  panel.

The faculty-specific `body.faculty-detail-open` class is gone entirely
(`grep` confirms zero references); the controller and `faculty.html.erb` now
set `overlay-open`.

## I2 — Escape no longer tears down the panel under an open dialog

Confirmed reachable and confirmed broken. The first attempt — guarding on
`document.body.classList.contains("modal-open")` from the existing
`keydown@window` Stimulus action — **did not work**, and the browser said so:

```
I2 after Escape: panel still open = false (expect true)
I2 after Escape: group still picked =
```

Cause: `dialog_controller` listens on `document` in the **bubble** phase, which
runs before a `window` bubble listener. It had already cleared `modal-open` by
the time the guard read it. Fixed by registering the handler by hand in the
**capture** phase (`window.addEventListener("keydown", h, true)`), which sees
the state before anything has reacted to the keypress; the `data-action` was
removed from `_scene.html.erb` accordingly. Both the reason and the phase are
commented at the registration site.

## I3 — footer controls no longer tabbable behind the overlay

`opacity: 0` became `visibility: hidden`, in `_site_footer.css` (the owning
file), matching the fix already applied one rule earlier for `.back-button`.
Not transitioned, so landmine #10's discrete-property trap does not arise. The
footer-regression grep guard on that file still passes after the append.

## Minor findings bundled

- **M6** — `GroupsController#detail_locals` deleted. `#faculty` now passes
  `periods: nil, selected_group_id: nil` inline with a comment saying why (no
  group is picked at that step), rather than threading always-nil ivars
  through a shared helper.
- **M11** — `faculty#selectCourse` returns early when the clicked course is
  already `is-active`, so re-tapping the current course no longer discards the
  picked group and schedule type.
- **M8 / M9** — two deliberate departures from the brief's literal text that
  were correct but undocumented, recorded here:
  - **The groups error state keeps a retry control** even though the brief says
    "no retry button at this level — the endpoint is cheap and page reload is
    the recovery path". That reasoning applies to the *faculties* fetch on a
    full page load; the groups fetch lives inside a Turbo Frame where "reload
    the page" would throw away the visitor's whole panel state. `_region_error`
    already renders `Повторить` and its `data-turbo-frame="_self"` link
    re-navigates just that frame — the React `groupsRetryToken` behaviour with
    no page reload. The brief's own functional spec (step 2) asks for exactly
    this button, so its two statements disagree; the functional spec wins.
  - **`Entities::Faculty.from_json_api` reads `hash[:name]`, not the brief's
    `hash.dig(:attributes, :name)`.** `Gateways::JsonApi.collection` has
    already flattened `attributes` up to the top level by the time the entity
    sees the row (see `JsonApi.flatten`), so the brief's snippet would have
    produced a catalog of blank cards — the same class of bug as landmine #13.
    Verified against live data: the grid renders `TF2`, `С`, `ЭБТ` with their
    full names.

## Re-verification (real output)

Rebuilt with `docker compose build schedule_web && docker compose up -d schedule_web`
after each change. New digests prove the rebuild took: `_site_header-e3b6c7ac`,
`_site_footer-99625c5a`, `_dialog-3c395a66`, `_faculty_picker-bd16a534`,
`faculty_controller-16d54519`.

### Targeted pass — D1/D2 regressions plus I2, I3, M11

```
D1 body class = faculty-page overlay-open
D1 header z-index/pointer-events = 260 / none
D1 element on top of close control = the close control itself
D1 close still works -> detail class = faculty-detail
I3 footer visibility = hidden
I3 footer controls reported visible by Playwright = 0 (expect 0)
I3 back-button visibility = hidden
I3 profile-button clickable = true
I2 before: group=СИ-3 confirm=/schedule?date=2026-07-29&group=513
I2 dialog open = true
I2 dialog z-index = 340 (panel is 180, header 260)
I2 dialog actually on top = true
I2 body class with dialog open = faculty-page overlay-open modal-open
I2 after Escape: dialog open = false (expect false)
I2 after Escape: panel still open = true (expect true)
I2 after Escape: group still picked = СИ-3
I2 after Escape: confirm href = /schedule?date=2026-07-29&group=513
I2 second Escape closes panel = true body=faculty-page
M11 re-click active course: group still active = 1 confirm href = /schedule?date=2026-07-29&group=513 (was /schedule?date=2026-07-29&group=513)
D2 confirm navigated to = http://localhost:3000/schedule?date=2026-07-29&group=513
```

Every original finding holds and every new one is fixed: the close control is
the topmost element at its own coordinates (D1), confirm still escapes both
frames (D2), Escape dismisses only the dialog and the course/group/type
selection survives (I2), a second Escape then closes the panel, the footer is
`visibility: hidden` with zero tabbable controls while the profile button stays
clickable (I3), and re-clicking the active course preserves the selection (M11).

### Full original suite re-run — unchanged at 1280 and 375

```
[1280] body class=faculty-page overlay-open          <- was faculty-detail-open
[1280] focused after open=faculty-detail-close
[1280] url unchanged after course click=true
[1280] groups visible after course pick=СИ-3
[1280] disabled types=ЗАЧЁТНАЯ НЕДЕЛЯ,ЭКЗАМЕНАЦИОННАЯ НЕДЕЛЯ
[1280] confirm href (lecture)=/schedule?date=2026-09-01&group=513
[1280] after next: empty msg=1 "Нет групп с расписанием на эту дату."
[1280] after Escape: grid clickable=true
[1280] focus returned to card=4
[1280] confirm navigated to=http://localhost:3000/schedule?date=2026-07-29&group=513
[1280] CSP violations=[]   failed requests=[]
[1280] overflow={"scrollW":1280,"clientW":1280,"bodyOverflow":false}
[1280] footer={"top":728,"bottom":800,"viewport":800,"docHeight":800}
[375]  overflow={"scrollW":375,"clientW":375,"bodyOverflow":false}
[375]  footer={"top":607,"bottom":667,"viewport":667,"docHeight":667}
```
(the only line that changed from the pre-fix run is the body class)

### Isolation test — `/group` still clean, `/schedule` 404 still the only noise

```
== A. /group alone                                        clean
== B. control: /teacher -> /teacher/1 (Turbo Drive visit) clean
== C. control: home -> /teacher via floor link            clean
== D. /group -> confirm -> /schedule (404 target)
   HTTP 404 http://localhost:3000/schedule?date=2026-07-29&group=513
   console: Applying inline style violates … style-src …   (public/404.html's inline style block)
```

### Shared-component regression — three shell stylesheets were touched, so every page was re-checked

```
guide dialog open=true activeElement=BUTTON.dialog__close
after Escape body class=
floor wipe navigated to=http://localhost:3000/teacher
teacher cards=5
after search url=http://localhost:3000/teacher?query=%D0%B8%D0%B2 cards=0
guest /room -> http://localhost:3000/?auth=1
CSP=[]
console errors=[]
```
Plus a screenshot of `/` at 1280 confirming the home page renders identically
(header, floor list, footer), and one of the auth dialog now painting over the
blurred faculty panel.

### Routes, translations, assets, guards

```
  200 /   200 /teacher   302 /room   200 /group   200 /teacher/1   302 /room/1
  200 /group/4   200 /group/4?course=3   200 /group/4/groups/513/periods
  200 /group/1   404 /group/999

translation missing:  /group 0 · /group/4 0 · /group/4?course=3 0
                      /group/4/groups/513/periods 0 · /group/1 0 · /group/6 0

<body data-controller="dialog" class="faculty-page overlay-open">   (GET /group/4)
<body data-controller="dialog" class="faculty-page">                (GET /group)

all 12 /assets/* on /group -> 200
all 10 @import url(...) component stylesheets -> 200

grep -nE "position:\s*fixed|transform" components/_site_footer.css -> clean
grep -rn "faculty-detail-open" app/ config/            -> no references remain
```

### Hard rule

```
$ git -C apps/schedule_web diff --cached --name-only | wc -l
0
$ git -C . diff --cached --name-only | wc -l
0
```

## Still open after this round

Unchanged from the original report's (d): `/schedule` is not routed yet, so the
confirm link 404s and that page's `public/404.html` inline style block is the
only CSP violation anywhere near this work — `SCHEDULE_PATH` should become
`schedule_path(...)` when that route lands. The upstream faculties pagination
gap, the 200-for-unknown-group behaviour of the periods endpoint, and the thin
seed data all still stand as described there. Review findings M4, M5, M7 and M10
were deferred by the reviewer and are untouched.
