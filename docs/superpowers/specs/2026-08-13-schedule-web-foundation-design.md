# schedule_web — Foundation, Shell and Home (Spec 1)

Date: 2026-08-13
Status: Approved (design), pending spec review
Supersedes: nothing. Complements `2026-08-08-schedule-home-redesign-design.md`, which
describes the visual design this service ports.

---

## 1. Purpose

Introduce `apps/schedule_web`, a Rails 8 + Hotwire service that becomes the public
face of Milk Schedule. It replaces the public half of the React SPA in
`apps/frontend`. It owns no data: every read and write goes to the Hanami API in
`apps/api_geteway`.

This spec covers the platform, the shared page shell, and the home page. The
remaining public pages are Spec 2.

### Boundary with apps/frontend

`apps/frontend` keeps the admin panel and teacher self-service and stays
reachable at its current port. The two services are **independent**: separate
logins, separate sessions, no shared state, no handoff protocol. Both
authenticate against the same `/auth` endpoints of `api_geteway`. React's public
pages are retired only when Spec 2 ships; nothing in this spec modifies
`apps/frontend`.

Rationale for independence: the React admin holds its JWT pair in
`sessionStorage` (`apps/frontend/src/App.tsx:58`), which a different origin
cannot write. Every single-sign-on option therefore requires either a shared
proxy origin or a ticket-exchange protocol — code that exists only to be deleted
when React is retired.

### Non-goals

- No admin panel, no teacher self-service. Those stay in React.
- No database. No ActiveRecord.
- No background jobs.
- No automated tests (see §13).
- No changes to `apps/api_geteway`.

---

## 2. Decisions

| # | Decision | Rationale |
|---|---|---|
| D1 | Independent services, separate logins | No cross-origin session code to unwind later |
| D2 | Falcon + Async, fiber-per-request | Non-blocking IO for an entirely IO-bound BFF |
| D3 | No ActiveRecord | Owns no data; also removes the thread-pool hazard that makes Falcon+Rails risky |
| D4 | Layered failover: stale cache → per-region degradation → circuit breaker | Maximum availability, honest about staleness |
| D5 | Code, comments and docs in English; UI strings Russian in `config/locales/ru.yml` | Matches `PRODUCT.md` and the `schedule_home/` design reference |
| D6 | No tests | Explicit user instruction; consequences recorded in §13 |

---

## 3. Runtime

- Ruby 3.4 (matches `apps/api_geteway`'s `ruby:3.4.2` base image).
- Rails 8, generated with `--skip-active-record --skip-active-storage
  --skip-action-mailer --skip-action-mailbox --skip-action-text
  --skip-active-job --skip-test --skip-system-test`.
- Falcon 0.52 as the server, replacing Puma. Forked one process per core; each
  process runs an async reactor; each request is a Fiber.
- `config.active_support.isolation_level = :fiber` so `CurrentAttributes` and
  `ActiveSupport::IsolatedExecutionState` key off the Fiber, not the Thread.
- `async-http` for all outbound HTTP and `async-redis` for all Redis, so a
  request in flight yields the reactor rather than parking it.

**Hard prohibition.** Nothing reachable from a request may use a blocking
client: not the `redis` gem, not `ActiveSupport::Cache::RedisCacheStore`, not
`Net::HTTP`, not Faraday's default adapter, not `rack-timeout`. Each parks the
entire reactor for the duration of its IO. The failure mode is counterintuitive
and therefore dangerous: adding a Redis-backed cache or rate limiter "for
resilience" would serialize every request in the process, making the service
slower the more resilience is added to it.

### Concurrency model

```
Falcon process (× CPU cores)
  reactor
    ├ Fiber  GET /teacher
    │    Async::Barrier
    │      ├ portal.teachers.index    ┐ concurrent; both yield
    │      └ portal.kafedras.index    ┘ while awaiting IO
    ├ Fiber  GET /assets/application.css   ← served during the above
    └ Fiber  GET /assets/application.js    ← served during the above
```

`Async::Barrier` fans out the calls a page needs; `Async::Semaphore` caps
in-flight upstream calls per process so a traffic spike cannot open more
connections to `api_geteway` than it can serve. Both are wrapped by the
aggregator described in §6 — controllers never touch `Async` directly.

---

## 4. Directory layout

Zeitwerk-managed, one directory per layer.

**Namespaced layers live under `app/lib/`, never directly under `app/`.** Every
direct subdirectory of `app/` is a Zeitwerk autoload *root*, not a namespace, so
`app/resilience/breaker.rb` would have to define a top-level `Breaker`; wrapping
it in `module Resilience` raises `Zeitwerk::NameError` and, because production
eager-loads, the container boots unhealthy and refuses every connection.
`app/lib/resilience/breaker.rb` correctly resolves to `Resilience::Breaker`, and
`app/lib` is picked up automatically — it needs no `config.autoload_paths` entry.
Controllers, views, models, helpers and javascript stay directly under `app/`:
those are legitimate roots whose files define top-level constants.

A related loading rule: `config/initializers/*.rb` run at initializer index
~103-107, but Zeitwerk's `setup_main_autoloader` runs at 144, so **no initializer
may eagerly reference an `app/lib` constant**. Defer with
`Rails.application.config.to_prepare` (index 149) or lazy registration blocks.

```
apps/schedule_web/
├── Dockerfile
├── Gemfile
├── config/
│   ├── application.rb
│   ├── falcon.rb
│   ├── initializers/{settings,container,redis,rack_attack,security_headers,logging}.rb
│   ├── locales/ru.yml
│   └── routes.rb
└── app/
    ├── controllers/          thin: contract → operation → presenter → render
    ├── models/               Current (ActiveSupport::CurrentAttributes)
    ├── lib/                  namespaced layers — see the note above
    │   ├── contracts/        dry-validation param schemas
    │   ├── entities/         dry-struct value objects
    │   ├── gateways/         one adapter per api_geteway resource
    │   ├── operations/       dry-operation, one class per use case
    │   ├── policies/         authorization predicates
    │   ├── presenters/       view models; ERB touches only these
    │   ├── resilience/       Breaker, StaleCache, RetryPolicy, Call, Outcome,
    │   │                     RackAttackStore
    │   └── session/          TokenStore, Refresh, Consent
    ├── views/
    │   ├── layouts/application.html.erb
    │   ├── shared/           _site_header, _site_footer, _dialog, _stale_banner,
    │   │                     _region_error
    │   └── home/
    ├── javascript/controllers/    Stimulus
    └── assets/stylesheets/
        ├── application.css
        └── components/       one file per component
```

### Layer contract

Each layer may depend only on layers below it.

```
controllers → operations → gateways → resilience
     ↓             ↓           ↓
 presenters    entities    entities
     ↓
   views
```

- **Controllers** parse params through a contract, call one operation, wrap the
  result in a presenter, render. Target ≤ 10 lines per action. No business
  logic, no `Async`, no HTTP.
- **Operations** (`dry-operation`) express one use case as a sequence of steps
  returning `Result`. They own concurrency fan-out.
- **Gateways** map one api_geteway resource to Ruby. They parse JSON:API into
  entities and return `Result`. They never rescue — `Resilience::Call` does.
- **Entities** (`dry-struct` + `dry-types`) are immutable and framework-free.
- **Presenters** are the only objects views see. They expose formatted strings
  and booleans, never raw entities or Results.

### Dependency injection

`dry-container` + `dry-auto_inject`, registered in
`config/initializers/container.rb`. Gateways and operations are resolved, never
instantiated inline, so an upstream can be swapped for a stub without touching a
call site.

`dry-operation` is chosen over hand-rolled service objects for consistency:
`apps/api_geteway` already depends on it, so both Ruby services read the same
way.

---

## 5. Request lifecycle

```
Falcon fiber
 └ Rack middleware (§8)
   └ Controller#action
     ├ Contracts::X.call(params)        → Success(input) | Failure(errors)
     ├ Operations::Y.call(input)        → Success(data)  | Failure(reason)
     │    └ Aggregator fan-out → Gateways → Resilience::Call
     └ Presenters::Z.new(...)
       └ layout + partials
```

A `Failure` from a contract renders 422 with the form redisplayed. A `Failure`
from an operation renders the region error partial described in §6.

---

## 6. Resilience

Every outbound call passes through `Resilience::Call`. There is no other path to
the API; gateways receive it by injection.

```ruby
Resilience::Call.new(
  key:      "portal.teachers.index",
  timeouts: { connect: 1.0, read: 3.0, total: 5.0 },
  retry:    { attempts: 2, backoff: :exponential_jitter, on: [Timeout, ServerError] },
  breaker:  { threshold: 5, window: 30, cooldown: 15 },
  cache:    { fresh: 60, stale_for: 86_400 }
)
```

### Outcome states

`Resilience::Call#call` returns exactly one of three, and every presenter must
handle all three:

| Outcome | Meaning | Rendering |
|---|---|---|
| `Success(Outcome::Fresh(data))` | upstream answered | normal |
| `Success(Outcome::Stale(data, cached_at))` | upstream failed, cache had a copy | normal + `_stale_banner` showing `cached_at` |
| `Failure(reason)` | upstream failed, no cached copy | `_region_error` inside the region's Turbo Frame, with a retry control |

Degradation is **per region**, never per page. A failed schedule fetch must not
prevent the header, footer, and teacher card from rendering.

### Timeouts

Enforced with `Async::Task#with_timeout` at the IO boundary. `Rack::Timeout` is
deliberately **not** used: it enforces deadlines by raising at an arbitrary
point in application code, which under a fiber reactor can fire mid-write and
leave connection state inconsistent. Per-call timeouts interrupt only a socket
read.

### Circuit breaker

State per `key`, stored in Redis so all Falcon processes share it. `threshold`
consecutive failures inside `window` seconds opens the circuit; while open,
calls fail immediately without touching the network for `cooldown` seconds, then
a single probe request decides whether to close. Breaker state per key is
reported by `/readyz`.

### Stale cache

Redis, keyed by `key` plus a digest of the request parameters. Successful
responses are written with the serialized body and a `cached_at` timestamp.
Within `fresh` seconds the cached copy is served without an upstream call at
all; between `fresh` and `stale_for` it is served **only** when the upstream
call fails. Past `stale_for` it is discarded.

Authenticated responses are cached per account, never globally, to prevent one
visitor's data reaching another.

---

## 7. Sessions, authentication, cookie consent

### Session

`cookie_store`, key `_schedule_web_session`, `httponly: true`,
`same_site: :lax`, `secure` in production, encrypted with `SECRET_KEY_BASE`.

Contents: access token, refresh token, access-token expiry, and a minimal
account record (id, email, role). Nothing else.

**Size constraint.** A cookie is capped at 4KB, and encryption plus Base64
inflates the payload roughly 1.4×. Two JWTs plus an account record is expected
to fit, but it is not guaranteed for long-lived tokens with large claim sets.
Boot-time assertion: `Session::TokenStore` measures the serialized session on
write and logs a warning past 3KB. If real tokens exceed the budget, the
fallback is a Redis-backed session store keyed by an opaque cookie id — a
contained change behind `Session::TokenStore`, which is why no other layer is
allowed to read `session` directly.

### Login

`POST /session` → `Operations::SignIn` → `POST /auth/sessions` on api_geteway.
On success the tokens are written to the session and the visitor is returned to
the path stashed by `require_account`, or `/`.

`Rack::Attack` throttles this path specifically (§8).

### Token refresh

`Session::Refresh` wraps every authenticated gateway call. On a `401` it rotates
once via `POST /auth/sessions/refresh`, replays the original call, and on a
second failure clears the session and redirects to `/` with the auth dialog
open. Refresh is idempotent per request — a page fanning out four calls that all
see `401` performs one refresh, not four. This is enforced with a per-request
memo held in `ActiveSupport::CurrentAttributes` (fiber-isolated per §3).

### Logout

`DELETE /session` → `DELETE /auth/sessions` upstream, then `reset_session`. The
local session is cleared even when the upstream call fails.

### Cookie consent

A separate cookie, `cookie_consent`, long-lived and **versioned**. Deliberately
not stored in the session so a decision survives logout, and bumping the version
invalidates old grants when the policy changes.

Consent is **per category, not binary** — the design reference
(`schedule_home/index.html`) presents three independent switches: necessary
(always on), functional, and analytics. The cookie therefore stores a category
list, e.g. `v1:necessary,functional`. `Session::Consent#allows?(:analytics)`
is the only supported way to test a grant.

Before a decision is recorded, only strictly-necessary cookies are set: the
session cookie and the CSRF token. The consent dialog is a Turbo Frame; both
buttons `POST` and re-render the frame without a full navigation.

---

## 8. Middleware

### Removed

`ActiveRecord`, `ActiveStorage`, `ActionMailer`, `ActionMailbox`, `ActionText`,
`ActiveJob`. None have work in a stateless BFF; each is boot time and attack
surface.

### Kept and added

| Middleware | Configuration |
|---|---|
| `Rack::Attack` | Per-IP request throttle (assets exempt); a hard, much lower limit on `POST /session`. Backed by a **custom async-redis store**, `Resilience::RackAttackStore` — every store Rack::Attack ships with uses a blocking client, which would park the reactor on every single request including asset fetches. |
| Security headers | Nonce-based CSP with no `unsafe-inline`, HSTS, `X-Frame-Options: DENY`, `X-Content-Type-Options: nosniff`, `Referrer-Policy: strict-origin-when-cross-origin`, `Permissions-Policy` denying camera/microphone/geolocation. |
| `Rack::Deflater` | gzip for HTML/CSS/JS/JSON. |
| `ActionDispatch::RemoteIp` | Trusted proxy chain declared explicitly, so `Rack::Attack` throttles the real client and not the proxy. |
| CSRF | Rails default, `protect_from_forgery with: :exception`. |
| Request ID | Generated per request, propagated to api_geteway as `X-Request-Id`, included in every log line, so a trace spans both services. |
| Structured logging | One JSON line per request: method, path, status, duration, request id, and per-upstream-call timing plus outcome state (fresh/stale/failed). |

### Health

- `/up` — Rails 8 built-in liveness. Process is alive. No upstream check.
- `/readyz` — readiness. Reports Redis reachability and circuit-breaker state
  per upstream key. Returns 503 when Redis is unreachable, because both the
  session store's rate limiting and the stale cache depend on it.

The distinction matters for orchestration: a failing `/readyz` should stop
traffic, a failing `/up` should restart the container.

---

## 9. View layer — Hotwire component structure

### Component convention

One component is one name across three files sharing a single root class:

```
app/views/shared/_site_footer.html.erb
app/assets/stylesheets/components/_site_footer.css
app/javascript/controllers/site_footer_controller.js   (only when behaviour is needed)
```

A component's stylesheet may style only selectors nested under its root class.
No page stylesheet may reposition another component.

### Layout

`layouts/application.html.erb` renders `_site_header`, `yield`, `_site_footer`
for every page in the service. Pages cannot opt out. This is what makes the
shell consistent by construction rather than by discipline.

### Footer rules

Two defects in the React implementation are designed out rather than patched:

1. **Positioning and animation are separated onto different elements.** An outer
   element owns placement; an inner element owns `transform`. In the React
   version both intents shared one `transform`, so
   `.entity-search-page .site-footer` overriding `transform` for a slide-in
   silently discarded the `translateX(-50%)` half of the base centring while
   `left: 50%` survived — under `position: fixed` that re-based against the
   viewport, shifting the footer right and stretching it
   (`apps/frontend/src/features/public/styles.css:242` and `:2774`).
2. **Visibility does not depend on scroll.** The React `useFooterVisibility`
   hook (`apps/frontend/src/features/public/bodyClass.ts:22`) only revealed the
   footer past `scrollY > 2`, so pages short enough not to scroll never showed
   one. The footer is part of normal document flow here and is always present.

### Empty states

Empty results render a labelled empty state from `ru.yml`, not a decorative
glyph. The React empty state rendered a literal `—` at 64px in a 260px box
(`EntitySearch.tsx:125`, `styles.css:2759`), which reads as a stray black line.

---

## 10. Home page and authorization

### Floor selection

```ruby
Policies::HomeFloors.call(account:)
# nil     → [:group, :teacher]
# present → [:group, :teacher, :room]
```

The room control is **absent from the DOM** for guests, not hidden with CSS.

### Route protection

`require_account` guards `/room` and `/room/:id` (the latter in Spec 2). On
rejection it stashes the intended path in the session, redirects to `/`, and
opens the auth dialog. After a successful login the visitor lands on the stashed
path.

This mirrors the API, which already treats room detail as non-public:
`config/routes.rb` moved `rooms.schedule` and `rooms.occupancy` out of the
`portal` slice into `admin`, leaving only the rooms index public.

### Dialogs

Auth, contacts, cookies and guide dialogs live in the shell and are driven by
Stimulus with `body.modal-open` for scroll locking, matching the
`schedule_home/` reference. Each is its own component per §9.

---

## 11. Configuration

All settings via environment, validated at boot with `dry-configurable` +
`dry-types` so a missing or malformed variable fails fast rather than at first
request.

| Variable | Purpose |
|---|---|
| `API_GETEWAY_URL` | Base URL of the Hanami API |
| `REDIS_URL` | Breaker state, stale cache, Rack::Attack counters |
| `SECRET_KEY_BASE` | Session encryption |
| `RAILS_ENV`, `RAILS_LOG_LEVEL` | Standard |
| `UPSTREAM_TIMEOUT_*` | Connect / read / total defaults |
| `BREAKER_*` | Threshold, window, cooldown |
| `CACHE_FRESH_TTL`, `CACHE_STALE_TTL` | Stale-cache windows |
| `RATE_LIMIT_*` | Rack::Attack limits |
| `TRUSTED_PROXIES` | Comma-separated CIDRs for RemoteIp |

---

## 12. Docker

A new service in the root `docker-compose.yml`, alongside the existing eight
(`frontend`, `api_geteway`, `media_sanitizer`, `excel_processor`, `db`, `redis`,
`minio`, `minio_init`):

- Built from `apps/schedule_web`, Ruby 3.4 base.
- `depends_on: [api_geteway, redis]`.
- Falcon bound to `0.0.0.0:3000`, published on host **3000**.
- Healthcheck polls `/up`.
- Shares the existing `.env.local` plus the variables in §11.
- `restart: unless-stopped`, same json-file logging anchor as the other services.

**Assets are precompiled into the image.** Propshaft only serves assets live in
development and test, so under `RAILS_ENV=production` an image without a
precompile step serves 404 for every stylesheet and Stimulus controller while
pages still return 200 — an unstyled, non-interactive site that looks healthy to
a `curl` status check. The `RUN` step needs `RAILS_ENV=production` set
explicitly, because compose's `environment:` block applies at `up` and not at
`build`; it also needs `SECRET_KEY_BASE_DUMMY=1` and placeholder values for the
two settings validated at boot (`API_GETEWAY_URL`, `REDIS_URL`), since
precompiling loads the initializers.

Two consequences worth stating plainly: **changing any CSS or JS requires
`docker compose build`, not merely a restart**, and CSS `@import` must use the
`url("components/_name.css")` form — a bare `@import "…"` is not digest-rewritten
by Propshaft and 404s.

`apps/frontend` stays on **4173**, unchanged and reachable, per D1.

---

## 13. Verification

No automated tests, per D6. Verification is manual:

1. `docker compose up schedule_web` reaches healthy.
2. `/up` returns 200; `/readyz` returns 200 with Redis up and 503 with Redis stopped.
3. Home page renders for a guest with **two** floors; the room control is absent
   from the HTML source.
4. `GET /room` as a guest redirects to `/` with the auth dialog open.
5. Login as staff; the room floor appears; the stashed-path return works.
6. Stop `api_geteway`; a previously visited page still renders from the stale
   cache with the staleness banner. A never-visited page renders the region
   error with the rest of the shell intact.
7. Repeated failures open the circuit; `/readyz` reflects it; recovery closes it.
8. `POST /session` beyond the limit returns 429.
9. Footer renders identically on every page, at every viewport, on both a short
   page and a long one.

### Recorded risk

A fiber reactor, a circuit breaker, and a token-refresh path are the three
components here most likely to fail in ways invisible until production —
specifically: a blocking call accidentally introduced into the reactor,
breaker state that never closes, and refresh storms under concurrent fan-out.
Without tests, only manual exercise of steps 6–8 stands between those and a
user-visible outage. This should be revisited before the service takes real
traffic.

---

## 14. Delivery

Five Sonnet-5 subagents in `.claude/agents/`, each owning a non-overlapping set
of paths so they can run without stepping on one another.

| Agent | Owns |
|---|---|
| `rails-platform` | app skeleton, Falcon config, middleware, initializers, Dockerfile, compose entry |
| `rails-domain` | contracts, entities, gateways, operations, policies, container |
| `resilience-engineer` | `app/lib/resilience/*`, Rack::Attack, breaker, stale cache, `/readyz` |
| `hotwire-views` | layouts, partials, Stimulus controllers, Turbo Frames |
| `css-architect` | component stylesheets, the design port from `schedule_home/` |

## 15. Follow-on

**Spec 2 — public pages.** `/group`, `/teacher`, `/room`, `/schedule`,
`/teacher/:id`, `/room/:id`, `/guide`; server-side pagination for the teacher
and room listings; retirement of the React public routes. Depends on this spec.
