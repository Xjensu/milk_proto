# Agent Prompt — Migrate the Remaining Public Pages from React to Rails

> Hand this file to the next agent verbatim. It is written as an instruction set,
> not as documentation: everything in it is either a requirement, a verified
> fact, or a landmine that has already exploded once in this codebase.

---

## 1. Mission

`apps/schedule_web` (Rails 8 + Hotwire + Falcon, port 3000) is replacing the
public half of the React SPA in `apps/frontend` (port 4173, which keeps the admin
panel). Six of the React public routes are already migrated. **Your job is to
migrate the three that remain**, at the same level of quality, then retire the
corresponding React routes only if explicitly asked.

You must preserve four things simultaneously. None of them is optional, and they
are listed in priority order when they conflict:

1. **Correctness and security** — no regressions, no data leaks between accounts.
2. **The established architecture** — the layer contracts in §4. New pages must
   look like they were written by the same hand as the existing ones.
3. **Availability and throughput** — the fiber reactor must never block (§7).
4. **Visual and functional fidelity** to the design reference (§6).

---

## 2. Scope — exactly three pages

The React public router (`apps/frontend/src/features/public/PublicApp.tsx`)
declares eight routes. Status:

| Route | React component | Status |
|---|---|---|
| `/` | `PublicHome` | ✅ migrated |
| `/teacher` | `EntitySearch mode=teacher` | ✅ migrated |
| `/room` | `EntitySearch mode=room` | ✅ migrated |
| `/teacher/:id` | `TeacherDetail` | ✅ migrated |
| `/room/:id` | `RoomDetail` | ✅ migrated |
| **`/group`** | **`GroupPicker`** (229 lines) | ⚠️ **stub — replace it** |
| **`/schedule`** | **`SchedulePage`** (159 lines) | ❌ **not built** |
| **`/guide`** | **`Guide`** + `guidePages.tsx` (181 lines) | ❌ **not built** |

Read the React components as the **functional** specification (what states exist,
what the user can do) and `schedule_home/` as the **visual** specification. Where
they disagree, `schedule_home/` wins on appearance and the React component wins
on behaviour — and say so in your report.

### 2.1 `/group` — replace the stub

`app/views/groups/index.html.erb` currently renders an "in development" message.
That was based on a **wrong conclusion that you must not inherit**: someone
observed `GET /public/groups` returning 404 and concluded group data was
unavailable. There is no *collection* endpoint, but the picker never needed one.
The real flow is faculty → group, and both endpoints exist and are routed:

- `GET /public/faculties`
- `GET /public/faculties/:faculty_id/groups`

Read `GroupPicker.tsx` for the interaction (it is the largest remaining
component — carousel/selection behaviour, course grouping) and
`schedule_home/group.html` plus the `/* Faculty selection — catalog / detail */`
block in `schedule_home/styles.css` (~line 1205) for the visuals.

Selecting a group leads to `/schedule` for that group-semester.

### 2.2 `/schedule` — the group timetable

Data (all routed under the public portal slice):

- `GET /public/groups/:group_semester_id/schedule?date=YYYY-MM-DD`
- `GET /public/groups/:group_semester_id/periods`
- `GET /public/groups/:group_semester_id/now`

Visuals: `schedule_home/schedule.html` + the `/* Schedule page */` block
(~line 1671) and, critically, `/* Mobile schedule: glance-first agenda instead of
a compressed desktop table */` (~line 2159). The mobile layout is a **different
information design**, not a squeezed table — implement it as such.

Note `Presenters::WeekSchedule` and `Policies::WeekWindow` already exist and were
built for the teacher/room record pages. Reuse them if the shape fits; extend
rather than fork if it nearly fits; explain in your report if you had to diverge.

### 2.3 `/guide` — the guide page

A full page, distinct from the existing guide **dialog** (`shared/_guide_dialog`,
which takes a `floors:` local and must keep working). Content structure lives in
`guidePages.tsx`; visuals in `schedule_home/guide.html` and `guide.css`.

Decide deliberately whether the page and the dialog share partials, and justify
it. Duplicated content that can drift is a defect.

---

## 3. Ground rules (read before touching anything)

- **Do NOT `git commit` or `git add`.** The user stages and commits manually.
  This is a hard rule and has been violated before.
- **Never claim a check you did not run.** Paste real command output. An honest
  "I could not verify X" is worth far more than a fabricated pass. Every serious
  bug found in this project so far surfaced because an agent reported a gap
  honestly instead of papering over it.
- **`apps/api_geteway` is a separate service and its own nested git repo.** You
  may read it freely — reading its actions, contracts and repos is often the
  fastest way to learn a payload's true shape. Change it **only** if a page is
  genuinely impossible otherwise, and if you do, say so prominently and keep the
  change additive and backward-compatible (the React admin must keep working).
- **UI strings are Russian**, in `config/locales/ru.yml`, always via `t()`.
  Code, comments and documentation are English. Edits to `ru.yml` are additive:
  it contains explanatory comment blocks that must survive.
- If the plan/spec documents
  (`docs/superpowers/{plans,specs}/2026-08-13-schedule-web-foundation*.md`)
  contradict the code, **the code is the truth** — those documents contain at
  least one layout instruction that was wrong enough to brick the container.

---

## 4. The architecture you must preserve

Request flow, strictly one direction:

```
Controller  →  Presenter  →  Gateway  →  Resilience::Call  →  HttpClient  →  api_geteway
   (thin)      (view model)   (one per     (timeout, retry,    (async-http)
                               resource)    breaker, cache)
```

### Layer contracts

| Layer | Location | Rule |
|---|---|---|
| Controllers | `app/controllers/` | Thin. Params → presenter → render. No HTTP, no business logic. Top-level class names. |
| Presenters | `app/lib/presenters/` | View models. Subclass `Presenters::Base` → exposes `data`, `stale?`, `cached_at`, `failed?`. ERB touches **only** presenters. |
| Gateways | `app/lib/gateways/` | One per upstream resource. Returns `Result<Outcome<…>>`. **The only code allowed to reach the network.** |
| Entities | `app/lib/entities/` | `Dry::Struct` value objects with `.from_json_api(hash)`. |
| Operations | `app/lib/operations/` | Multi-step use cases (see `Operations::RoomWeekSchedule` for concurrent fan-out). |
| Policies | `app/lib/policies/` | Authorization / decision logic, no I/O (`Policies::HomeFloors`, `Policies::WeekWindow`). |
| Session | `app/lib/session/` | `TokenStore` is the **sole** writer of `session[:auth]`. |
| Resilience | `app/lib/resilience/` | Breaker, stale cache, retry, timeouts. **Do not modify** — treat as infrastructure. |

### The recipe for adding a page (follow it exactly)

1. **Entity** (if a new resource): `app/lib/entities/x.rb`, `.from_json_api`.
   Verify field names against a **live** payload, not against the React types.
2. **Gateway**: `app/lib/gateways/portal/x_gateway.rb < BaseGateway`, with its own
   `Resilience::Call` key (e.g. `"portal.groups.schedule"`). Return entity
   **hashes**; callers rehydrate (the cache round-trips through JSON).
3. **Register** it in `config/initializers/container.rb` using the existing
   **lazy block** pattern.
4. **Presenter**: `app/lib/presenters/x.rb < Presenters::Base`.
5. **Controller** + route (named route, matching the `teacher`/`room` style).
6. **View** using the reference's class names, plus the shared partials
   `shared/_stale_banner` and `shared/_region_error` for degraded states.
7. **Stylesheet**: `app/assets/stylesheets/components/_x.css` + an
   `@import url("components/_x.css")` line in `application.css`.
8. **Locale keys** in `ru.yml`, additive.

If a step feels unnecessary for your page, that is a signal to re-read the
existing pages, not a licence to skip it.

---

## 5. Landmines — every one of these has already cost hours

These are not hypotheticals. Each was a real, diagnosed failure in this codebase.

### Loading and layout

1. **Namespaced code lives in `app/lib/<layer>/`, never `app/<layer>/`.** Every
   direct subdirectory of `app/` is a Zeitwerk autoload *root*, not a namespace:
   `app/gateways/foo.rb` must define top-level `Foo`, so a `module Gateways`
   wrapper raises `Zeitwerk::NameError`. Because production eager-loads, the
   container then boots **unhealthy and refuses every connection**. Controllers,
   models, views, helpers and javascript stay directly under `app/`.
2. **Initializers cannot eagerly reference `app/lib` constants.**
   `config/initializers/*.rb` run at index ~103–107; Zeitwerk's
   `setup_main_autoloader` runs at 144. Defer with
   `Rails.application.config.to_prepare` (index 149) or lazy blocks. This is why
   `rack_attack.rb` assigns its store inside `to_prepare`.
3. **Assets are precompiled into the image.** Propshaft only serves assets live
   in dev/test, so under `RAILS_ENV=production` a missing precompile means every
   CSS/JS 404s **while pages still return 200** — an unstyled, non-interactive
   site that looks healthy to `curl`. Consequence for you: **after any CSS, JS or
   ERB change you must `docker compose build schedule_web`, not just restart.**
4. **CSS `@import` must use the `url("components/_x.css")` form.** A bare
   `@import "…"` is not digest-rewritten by Propshaft and 404s.
5. **Never combine placement and `transform` on one element.** The reference
   pins its footer with `position: fixed; left: 50%; transform: translateX(-50%)`
   — porting that broke the footer. `_site_footer.css` must stay free of
   `position: fixed` and the substring `transform`; there is a verification grep
   for exactly that. The footer is pinned instead by `main { flex: 1 0 auto }`
   plus `.site-footer { margin-top: auto }` on a `min-height: 100svh` flex body.
6. **A wrapper element around `<main>` will unpin the footer**, because
   `main { flex: 1 0 auto }` then applies to a non-flex-child. If you must wrap
   `<main>` (e.g. for a Stimulus scope), give the wrapper
   `display: contents` — see `.floor-scope`.

### Markup and templating

7. **Assemble tag attributes before emitting the tag.** A `<body …>` built from
   inline `<% if %>` fragments once lost its closing bracket; the parser then
   read the following `<header>` as body attributes, `<body>` inherited
   `class="site-header"` and its `min-height: 88px`, and the footer silently
   unpinned on every page. Use `tag.attributes(hash)`.
8. **An ERB comment must not contain the sequence `%>`.** Writing `<% if %>`
   inside a `<%# … %>` comment terminates it early and leaks the remainder into
   the rendered HTML.
9. **Stimulus targets resolve only inside the controller element's own subtree.**
   A target outside it raises "Missing target element". If the action already
   called `event.preventDefault()`, the click is swallowed and the control is
   **silently dead** — worse than no JS at all. Guard: bail out *before*
   `preventDefault()` when a required target is missing.

### CSS behaviour

10. **`visibility` is a discrete property.** `transition: visibility 200ms` keeps
    an element hidden for the full duration, and a hidden element cannot receive
    focus — this silently broke focus-into-dialog. Use asymmetric timing:
    `visibility 0s linear <dur>` on the base, `visibility 0s linear 0s` on the
    open state.
11. **Any full-viewport overlay needs `pointer-events: none` when closed.** A
    stuck overlay at a high `z-index` swallows every click on the page. Also
    reset such overlays on `pageshow` — bfcache restores the DOM exactly as it
    was left.
12. **`width/height: 100%` does nothing on an inline element.** A hover fill
    rendered as a 0×0 invisible box because its span was never `display: block`.

### Data

13. **Verify entity field names against a live payload.** `Entities::Teacher`
    mapped `surname/name/patronymic` while the API ships
    `last_name/first_name/middle_name`; every teacher rendered blank, and the
    unit test passed because it used constructed data.
14. **`Resilience::StaleCache` round-trips through JSON**, so symbol keys come
    back as strings on a cache hit. Symbolize on read (`.symbolize_keys` /
    `.deep_symbolize_keys`) or your code works on a miss and breaks on a hit.
15. **Any authenticated call's cache key must include the account id**, or one
    user's data will be served to another. See
    `Gateways::Admin::RoomScheduleGateway`.
16. **`GET /public/teachers` search ignores pagination.** Its `search_by_fio`
    branch returns *all* matches, so `TeacherSearch` slices client-side. The
    rooms search (added later) paginates in SQL. Check which behaviour any new
    endpoint has before designing pagination on top of it.
17. **`POST /session` is rate-limited to 10/min per IP.** The 11th request
    returns 429 — do not misdiagnose that as an auth bug during testing.

---

## 6. Visual fidelity

- Design tokens are already aligned to the reference `:root`: `--ink`,
  `--velvet #4B262F`, `--almond #EED3BA`, `--paper`, `--line`, plus documented
  derived tokens. **Use tokens; introduce no new hardcoded colours.**
- Use the reference's **flat class vocabulary** (`.building`, `.floor`,
  `.entity-card`, `.teacher-record-*`). An earlier BEM rename made all 3658 lines
  of reference CSS unusable and forced a re-derivation that visibly diverged.
- Fonts (Manrope, Unbounded) are already loaded in the layout. Don't touch that.
- Interactive affordances must be honest: no hover lift or `cursor: pointer` on
  something that does not navigate.
- Must hold at **1280px and 375px**: no horizontal page overflow. Wide tables
  scroll inside their own wrapper (`overflow-x: auto`), never the body.

---

## 7. Availability and throughput — the reactor must never block

Falcon runs a **fiber reactor per worker process**. One blocking call parks every
concurrent request in that process. This is the single easiest way to destroy
this service's throughput.

- **Permitted:** `async-http` (via `Gateways::HttpClient`), `async-redis` (via
  `ScheduleWeb::Redis`). **Forbidden:** `Net::HTTP`, `HTTParty`, `Faraday`
  default adapter, the `redis` gem, `sleep`, blocking file or DNS I/O in a
  request path.
- **Every outbound call goes through `Resilience::Call`**, which enforces the
  timeout budget, jittered retry, the circuit breaker and the stale cache. Never
  bypass it — gateways receive it by injection precisely so a call site cannot
  forget.
- **Fan out concurrent upstream calls** rather than serializing them. See
  `Operations::RoomWeekSchedule` (6 concurrent `Async::Task`s merged into one
  `Outcome`). `/schedule` and `/group` are obvious candidates.
- **Degrade, never fail blank.** `Outcome::Stale` → render data *plus* the
  staleness banner with `cached_at`. `Failure` → render `shared/_region_error`
  with a working retry, header and footer intact. Redis being down must degrade
  the cache, never break a page.
- Keep `/readyz` honest: if you add an upstream, add its breaker key to
  `Resilience::Call::KNOWN_KEYS` so a cold process reports real state instead of
  an empty map.
- Prefer cacheable GETs with sensible `fresh_ttl`; think about what a cache key
  must include for correctness before you write it.

---

## 8. Security requirements

- **Strict nonce-based CSP, no `unsafe-inline`.** Inline `<script>`, `<style>`,
  `onclick=` and `style=` are silently dropped. Use Stimulus `data-action` and
  stylesheets. Zero console CSP violations is a release gate.
- **CSRF protection stays on.** Every mutating form goes through Rails' helpers.
- **Never place tokens, ids or personal data in URLs or logs.**
- **Staff-only data stays gated.** Room schedule data lives in the admin slice by
  deliberate decision; `/room*` is guarded by `before_action :require_account`.
  If a new page needs admin-scoped data, gate the page *and* scope the cache key.
- **Escape LIKE metacharacters** in any search term reaching a query.
- **Escape all upstream content on render** (Rails does this by default — do not
  defeat it with `html_safe`/`raw` on API-derived strings).
- Session cookies stay `httponly`, `same_site: :lax`, with `secure` driven by
  `ASSUME_SSL`/`FORCE_SSL` (both default false so plain-HTTP localhost works).
  Do not hardcode `secure: Rails.env.production?` — that conflates "named
  production" with "is HTTPS" and silently drops the cookie.

---

## 9. Extensibility — what "easy to change" means here

Judge your own work against these:

- **Adding the next page must not require editing an existing page.** Shared
  behaviour belongs in a partial, a presenter base, or a policy.
- **A page's data shape is described in exactly one place** (its presenter). If a
  view reaches into a raw hash, or a controller formats a date, that is a defect.
- **Authorization decisions live in `app/lib/policies/`**, never inline in a view
  or controller. `Policies::HomeFloors` is the model: it returns the list to
  render, so an ungranted option is **absent from the DOM** rather than
  CSS-hidden.
- **One component, one stylesheet, one root class.** No component restyles
  another; `application.css` positions nothing.
- Prefer extending an existing abstraction over adding a parallel one — but if
  extending would distort it, add a sibling and explain why in your report.

---

## 10. Environment

- Working dir: `C:\Users\Asus\Desktop\milk_schedule`. App:
  `apps/schedule_web` (its own nested git repo).
- **Bring the stack up first** — Docker Desktop was down when this prompt was
  written: `docker compose up -d`, then wait for `schedule_web` to answer.
  If the Docker daemon is unresponsive, check free disk space before anything
  else; `docker builder prune -af` has reclaimed ~25 GB here before.
- Rebuild after any change: `docker compose build schedule_web && docker compose up -d schedule_web` (~2–4 min).
- `HTTP_PROXY` is set: **curl to localhost needs `--noproxy '*'`**.
- API: `http://localhost:2300`. Public frontend: `http://localhost:3000`.
  React admin: `http://localhost:4173`.
- Staff credentials: `admin@milk-schedule.local` / `ChangeMe1!`.
- Run Ruby in-container:
  `docker compose exec -T schedule_web sh -c 'cat > /tmp/x.rb && bundle exec rails runner /tmp/x.rb' < script.rb`.
  Anything async must be wrapped in `Async do … end`.
- Seed data is small (5 teachers, 8 rooms). **Empty results are a normal case:
  every page needs a real empty state, not a blank screen.**
- **Playwright is installed and working** — use it. Write a node script, drive real
  Chromium, screenshot the reference beside the live page, read the console.
  Essentially every real defect in this project was invisible to `curl` and found
  this way.

---

## 11. Verification protocol — the definition of done

Run all of these and paste the real output.

1. Container healthy; `/up` and `/readyz` both 200.
2. Every route returns its expected status: the three new pages, plus `/`,
   `/teacher`, `/room`, `/group`, `/teacher/1`, `/room/1` unbroken. `/room*` must
   still redirect a guest to `/?auth=1`.
3. **Assets actually load**: extract every `/assets/*` URL from each rendered page
   and confirm each returns 200. A 200 page with 404 assets is the exact failure
   mode this project shipped once.
4. **Real browser pass at 1280 and 375**: zero console errors, zero CSP
   violations (listen for `securitypolicyviolation`), no horizontal overflow,
   footer at the bottom of short pages.
5. **Degraded states**: stop `api_geteway`, wait past `cache_fresh_ttl` (60s),
   then confirm a previously-visited page shows the staleness banner and a
   never-visited one shows the region error with header and footer intact.
   Restart it afterwards.
6. **Interaction**: every new control actually works in the browser — clicked,
   navigated, keyboard-reachable, focus visible. Not "the markup looks right".
7. No `translation missing` in any render.
8. Existing behaviour intact: dialogs open/close with focus handling, the home
   floor wipe plays and navigates, search and pagination still work.

---

## 12. Report

Finish with:

- **(a)** Files created/modified, grouped by layer.
- **(b)** Any deviation from this prompt or from the existing patterns, with the
  reasoning. Deviating for a good, stated reason is welcome; deviating silently
  is not.
- **(c)** Real verification output for every item in §11, and an explicit list of
  anything you could **not** verify.
- **(d)** Any upstream (`api_geteway`) limitation you hit and how you worked
  around it — plus what the clean fix would be if the API could change.
- **(e)** Anything needing a human decision.

If you hit a blocker you cannot resolve, **stop and report it with the real error
output**. Do not invent a workaround that hides it, and do not report a step as
passing that you did not run.
