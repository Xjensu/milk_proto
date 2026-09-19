# Task Brief — `/group` page (faculty → course → group picker)

> Self-contained. You have no other context beyond this file, the mission
> doc it was derived from (`docs/superpowers/plans/2026-08-16-remaining-public-pages-migration-prompt.md`
> — read it first for the ground rules, architecture, landmines and
> verification protocol; this brief only fills in the specifics for this one
> page), and the repository itself.

## What this page does

Replaces the stub at `app/views/groups/index.html.erb` (currently renders an
"in development" message under the false premise that `/public/groups` — a
collection endpoint that never existed — was needed). It isn't. The real flow
is faculty → course → group, confirmed live:

- `GET /public/faculties` → JSON:API collection.
- `GET /public/faculties/:faculty_id/groups[?date=]` → plain (non-JSON:API)
  hash, groups whose own semester window contains `date` (default: today).
  **An empty `groups` array is normal**, not an error (confirmed live: faculty
  id 1 and id 6 both returned `"groups":[]` on 2026-08-16; only faculty id 4
  had data).
- `GET /public/groups/:group_semester_id/periods` → plain hash, the group's
  configured schedule-type windows (semester / lecture_week / credit_week /
  exam_session).

Verified live payloads (2026-08-16, via api_geteway on :2300):

```
GET /public/faculties
{"data":[
  {"id":"1","type":"faculty","attributes":{"name":"TEST Facul Updated","short_name":"TF2"}},
  {"id":"4","type":"faculty","attributes":{"name":"Строительный факультет","short_name":"С"}},
  {"id":"6","type":"faculty","attributes":{"name":"Факультет экономики и бизнес-технологий","short_name":"ЭБТ"}}
]}

GET /public/faculties/4/groups
{"meta":{"resolved_at":"...","anchor_date":"2026-08-16","timezone":"Europe/Moscow"},
 "groups":[{"group_semester_id":513,"group_id":82,"course":3,"speciality_name":"СИ",
   "students_now":1,"year_of_admission":"2023-09-01","duration_of_study":4,
   "semester":{"semester_id":1,"number":1,"label":"1 семестр 2025/2026",
     "start_date":"2026-07-29","end_date":"2026-11-11","period_kind":"semester"}}]}

GET /public/groups/513/periods
{"group_semester_id":513,"periods":[
  {"period_kind":"semester","start_date":"2026-07-29","end_date":"2026-11-11"},
  {"period_kind":"lecture_week","start_date":"2026-09-01","end_date":"2026-09-07"}]}
```

Use faculty id `4` / group_semester_id `513` for your own manual testing —
it's the only faculty in seed data with a group scheduled today.

## Functional spec (from `apps/frontend/src/features/public/GroupPicker.tsx`, read in full — 229 lines)

State machine:
1. Faculty grid loads on mount (`GET /public/faculties`). Show a loading
   status, then the grid. A fetch failure shows an inline error (no retry
   button at this level — the endpoint is cheap and page reload is the
   recovery path).
2. Clicking a faculty card opens a **detail panel** (course → group →
   schedule-type → confirm), fetching `GET .../groups` for just that faculty,
   lazily, on open — not prefetched for all faculties. Loading/error/empty
   states each render inline; the error state has a **"Повторить" (retry)
   button** that re-fires the same fetch (`groupsRetryToken` counter pattern
   in the React source — re-request without a page reload).
3. Groups are grouped by `course` (ascending). Clicking a course number shows
   that course's groups as buttons labeled `"{speciality_name}-{course}"`
   (e.g. "СИ-3"). This is pure client-side filtering of the one fetched
   groups list — no new request per course click.
4. Clicking a group fetches `GET /public/groups/:id/periods` and populates
   four schedule-type buttons (ОБЫЧНОЕ / ЛЕКЦИОННАЯ НЕДЕЛЯ / ЗАЧЁТНАЯ НЕДЕЛЯ /
   ЭКЗАМЕНАЦИОННАЯ НЕДЕЛЯ = semester / lecture_week / credit_week /
   exam_session). A type button is `disabled` unless that `period_kind` is
   present in the response. Default selection: `semester` if present, else
   the first period returned.
5. The confirm button (only enabled once a group is picked) navigates to
   `/schedule?group=<group_semester_id>&date=<selected_period.start_date>`
   (date omitted if no matching period, which shouldn't happen in practice).
6. Faculties are also carousel-navigable inside the open detail panel (‹ / ›
   arrows cycle `activeIndex` through all faculties, re-triggering step 2 for
   the newly active one) and closeable back to the grid.

Edge cases to replicate: zero faculties (rare, but don't crash — grid is just
empty), zero groups for a faculty on the current date ("Нет групп с
расписанием на эту дату" — `groups.length === 0` after a successful, non-error
fetch), a groups-fetch failure with working retry.

## Visual spec

Reference file: `schedule_home/group.html` (structure) — this is a *static
mockup* with all faculties hardcoded and the detail panel markup present but
empty (`aria-hidden="true"`, populated by `group.js` at runtime in the
reference). Port the **structure and class names**, not the hardcoded
faculty list (that comes from the live API).

CSS: `schedule_home/styles.css` lines **1205–1670** (`/* Faculty selection —
catalog / detail */` through the block ending before `/* Schedule page */` at
1671), including the responsive sub-blocks inside that range (`@media
max-width: 1180px` at 1479, `780px` at 1486, `460px` at 1499, plus the
carousel-centering fixes documented at 1509–1670). Port verbatim; do not
rename `.faculty-*` classes (flat vocabulary, not BEM — see mission §6).

Key class names from the reference (both `group.html` and `GroupPicker.tsx`
already agree on these, so this is low-risk): `.faculty-page` (body class),
`.faculty-scene`, `.faculty-grid`, `.faculty-card`, `.faculty-product`,
`.faculty-monogram`, `.faculty-card-label`, `.faculty-detail` /
`.faculty-detail.is-open`, `.faculty-detail-close`, `.faculty-detail-nav` /
`.faculty-detail-prev` / `.faculty-detail-next`, `.faculty-detail-product`,
`.faculty-detail-options`, `.faculty-detail-dots`, `.faculty-detail-courses`,
`.faculty-detail-groups`, `.faculty-detail-schedules`,
`.faculty-detail-confirm`, `.lookup-status` (shared loading/error/empty
status class, already used elsewhere in this codebase — reuse it, don't
invent a new one).

The React source's `animateProductToDetail` (a `cloneNode` + Web Animations
API flyer that flies the clicked card's monogram to the detail panel) is a
decorative flourish, not a functional requirement. **Recommendation:
skip it** and rely on the `.faculty-detail.is-open` opacity/visibility
transition already defined in the reference CSS (asymmetric
`visibility 0s linear <dur>` per landmine #10 below) — it's already correct,
self-contained, and doesn't need a Stimulus controller doing
`getBoundingClientRect()` gymnastics or off-element node cloning (which would
also fight landmine #9 below, since the clone lives outside the controller's
own subtree). If you *do* port the flyer animation, justify it in your report
against this recommendation; either way the confirm flow must work without it
(don't gate functionality behind the animation completing).

## Architecture — the recipe, filled in

**1. Entity** — `app/lib/entities/faculty.rb`:
```ruby
module Entities
  class Faculty < Dry::Struct
    transform_keys(&:to_sym)
    attribute :id, Types::String
    attribute :name, Types::String
    attribute :short_name, Types::String.optional.default(nil)

    def self.from_json_api(hash)
      new(id: hash[:id], name: hash.dig(:attributes, :name).to_s,
          short_name: hash.dig(:attributes, :short_name))
    end
  end
end
```
No entity needed for group rows or periods — both are plain (non-JSON:API)
hashes, and this codebase's convention for that shape (see
`TeacherScheduleGateway`) is to pass the deep-symbolized hash straight
through and let the presenter read it directly, not wrap it in a
`Dry::Struct`. Follow that precedent here.

**2. Gateways** — three, one per upstream shape (mirrors `TeachersGateway` vs
`TeacherScheduleGateway` already being separate classes despite both being
"teacher" resources):

- `app/lib/gateways/portal/faculties_gateway.rb < BaseGateway`
  KEY `"portal.faculties.index"`. `#index` → `GET /public/faculties` →
  `JsonApi.collection(document).map { |row| Entities::Faculty.from_json_api(row).to_h }`
  (mirror `TeachersGateway#index` exactly, including the `JsonApi.collection`
  helper — read `app/lib/gateways/json_api.rb` for its contract before using
  it).
- `app/lib/gateways/portal/groups_gateway.rb < BaseGateway`
  KEY `"portal.groups.by_faculty"`. `#for_faculty(faculty_id:, date: nil)` →
  `GET /public/faculties/#{faculty_id}/groups#{query_string(date: date)}` →
  `document.deep_symbolize_keys` (mirror `TeacherScheduleGateway#schedule`'s
  raw-passthrough style — this is a non-JSON:API shape).
- `app/lib/gateways/portal/group_periods_gateway.rb < BaseGateway`
  KEY `"portal.groups.periods"`. `#periods(group_semester_id:)` →
  `GET /public/groups/#{group_semester_id}/periods` →
  `document.deep_symbolize_keys`.

**3. Register** in `config/initializers/container.rb` (append, don't
reorder the existing lines):
```ruby
ScheduleWeb::Container.register("gateways.faculties")      { Gateways::Portal::FacultiesGateway.new }
ScheduleWeb::Container.register("gateways.groups")         { Gateways::Portal::GroupsGateway.new }
ScheduleWeb::Container.register("gateways.group_periods")  { Gateways::Portal::GroupPeriodsGateway.new }
```
Also add all three KEY strings to `Resilience::Call::KNOWN_KEYS` in
`app/lib/resilience/call.rb` (append to the existing `%w[...]` array) so
`/readyz` reports real breaker state from boot, per mission §7.

**4/5/6. Presenters, controllers, routes, views** — the interaction has two
genuinely separate network moments (open a faculty → fetch its groups;
pick a group → fetch its periods), which in a server-rendered Hotwire app
means two lazy-loaded Turbo Frames rather than one big eager page load.
Recommended design (deviate and justify in your report if you find a cleaner
Rails-idiomatic shape once you're in the code):

- `GET /group` (existing route, `groups#index`) —
  `Presenters::GroupPicker.new` wraps only the faculties list (one gateway
  call). View renders the `.faculty-grid` eagerly plus an empty
  `.faculty-detail` shell containing a `<turbo-frame id="faculty_detail">`
  with no `src` (populated on demand).
- `GET /group/:faculty_id` (new route, e.g. `groups#faculty`, as:
  `:group_faculty`) — `Presenters::FacultyDetail.new(faculty_id:, date:)`
  wraps the `#for_faculty` call, exposes `groups_by_course` (mirror
  `groupByCourse` from the React source: group by `course`, sorted
  ascending). Rendered **as a Turbo Frame response** matching
  `faculty_detail`'s id, so clicking a faculty card
  (`data-turbo-frame="faculty_detail"`) swaps only that panel. Course and
  group selection inside the loaded groups list is pure client-side
  filtering (a small Stimulus controller toggling visibility by
  `data-course`/`data-group` attributes) — the full groups-for-faculty
  response already has everything needed, no further request until a group
  is actually picked.
- `GET /group/:faculty_id/groups/:group_semester_id/periods` (new route,
  e.g. `groups#periods`, as: `:group_periods`) — thin controller action
  wrapping `GroupPeriodsGateway#periods`, rendered into a nested
  `<turbo-frame id="faculty_schedules">` inside the detail panel, populating
  the four schedule-type buttons with the disabled/enabled + default-selection
  logic from step 4 above.
- The confirm button is a plain `<%= link_to %>` to `schedule_path(group:
  group_semester_id, date: selected_period_start_date)` — no JS required for
  the navigation itself, only for tracking which period button is "selected"
  client-side before confirming (Stimulus, scoped to the schedules turbo-frame).

If nested Turbo Frames prove awkward in practice (e.g. the carousel's
prev/next needs the whole detail panel to re-navigate anyway), a simpler
fallback is acceptable: make `GET /group/:faculty_id` do the full page
(non-frame) response including groups, and make the schedule-type step a
second full navigation to `GET /group/:faculty_id/groups/:group_semester_id`
that renders periods + confirm button, giving up the "no full navigation"
SPA feel for the detail panel but keeping the same URLs and gateway/presenter
layering. State which you built and why in your report — this is the one
place in this task where the "right" Rails answer isn't obvious from existing
precedent (no other page in this app has a two-level lazy drill-down).

Presenters must expose `stale?`/`failed?`/`cached_at` per `Presenters::Base`
(wrap each gateway result independently — a stale groups list must not hide a
fresh periods list and vice versa, same principle as `TeacherRecord` keeping
its two calls independent).

**7. Stylesheet** — `app/assets/stylesheets/components/_faculty_picker.css`
(new file, root class `.faculty-page`/`.faculty-scene` per the reference),
imported via `@import url("components/_faculty_picker.css");` in
`application.css` (bare `@import "…"` 404s under Propshaft — landmine #4).

**8. Locale keys** — add a new top-level section to `config/locales/ru.yml`
(additive; don't touch the existing `group_search:` block, which becomes
dead but the ground rules only require additions, not cleanup — leave it or
remove it if genuinely unused, your call, just don't break the file's
existing comment blocks). Strings needed: loading status ("Загружаем
факультеты…"), faculties-fetch error, "Загружаем курсы…", groups error +
retry label, "Нет групп с расписанием на эту дату", the four schedule-type
labels, course/group/schedule-type aria-labels, confirm aria-label. Use exact
Russian copy from `GroupPicker.tsx` (quoted above) — it's already the
correct, reviewed copy.

## Landmines that apply here (see mission §5 for full text)

- **#3 asset precompile** — you will touch CSS/JS repeatedly while getting
  the carousel/detail-panel interaction right; rebuild the container
  (`docker compose build schedule_web`) before trusting any visual check,
  not just restart.
- **#4 `@import url(...)`** for the new stylesheet.
- **#9 Stimulus target scope** — the carousel prev/next, course buttons,
  group buttons, and schedule-type buttons are all inside one detail panel;
  if you split them across nested Turbo Frames, remember Stimulus controller
  scope is the DOM subtree at the time an action fires — a controller whose
  element gets replaced by a frame swap loses its listeners; re-verify
  clicks still work after every frame navigation, not just on first load.
- **#10 visibility transition** — `.faculty-detail`'s open/close already
  uses the correct asymmetric-timing pattern in the reference CSS; don't
  "simplify" it to a single `transition: all` or you'll reintroduce the
  focus-into-hidden-element bug this pattern exists to avoid.
- **#16-adjacent** — `GET /public/faculties/:id/groups` has no documented
  pagination in what you'll see from the mission doc's endpoint list; verify
  whether a large faculty could paginate before assuming the full list always
  arrives in one response (check `apps/api_geteway/slices/portal/contracts/`
  read-only if uncertain).

## Definition of done (subset of mission §11)

1. `GET /group` returns 200, faculty grid renders with live data.
2. Opening a faculty with data (id 4 in seed) shows courses → groups →
   schedule-type buttons behaving per the state machine above.
3. Opening a faculty with zero groups on the current date shows the correct
   empty state, not a blank panel or a false error.
4. Confirm navigates to `/schedule?group=513&date=...` for a real pick.
5. Assets all 200 (extract `/assets/*` from the rendered page).
6. Zero console errors/CSP violations at 1280 and 375px; no horizontal
   overflow; footer pinned.
7. `docker compose stop api_geteway`, wait past `cache_fresh_ttl` (60s):
   a previously-opened faculty detail shows the stale banner on revisit; a
   never-visited one (or the faculties list itself on a cold cache) shows
   `shared/_region_error` with header/footer intact. Restart api_geteway
   afterwards.
8. No `translation missing` anywhere on the page in any state.
