# schedule_web Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stand up `apps/schedule_web` — a Rails 8 + Hotwire BFF running on a Falcon fiber reactor — with its platform layers, shared page shell, and the auth-gated home page, reachable in Docker.

**Architecture:** A stateless Rails app with no database. Controllers parse params through dry-validation contracts, delegate to dry-operation use cases, and render presenters. Every outbound call to `apps/api_geteway` passes through a single `Resilience::Call` object providing timeouts, retry, a Redis circuit breaker, and a stale-response cache. All IO is non-blocking so a page's API fan-out never parks the reactor that is concurrently serving CSS and JS.

**Tech Stack:** Ruby 3.4, Rails 8 (no ActiveRecord), Falcon 0.52, async 2.42, async-http 0.92, async-redis, dry-configurable, dry-container, dry-auto_inject, dry-types, dry-struct, dry-validation, dry-monads, dry-operation, Hotwire (Turbo + Stimulus), Rack::Attack.

Spec: `docs/superpowers/specs/2026-08-13-schedule-web-foundation-design.md`

---

## Global Constraints

- **No tests.** Explicit user instruction. Every task ends with a manual verification command whose expected output is stated. Do not create `spec/` or `test/`.
- **No commits by agents.** The user stages and commits manually. Never run `git commit`, `git add`, or `git push`. End each task by listing changed files.
- **Code, comments, identifiers, logs and docs in English.** Visitor-facing strings in Russian, and only in `config/locales/ru.yml`. No Russian string literals in `.rb`, `.erb` or `.js` files.
- **No blocking IO anywhere reachable from a request.** Use `async-http` for HTTP and `async-redis` for Redis. The `redis` gem, `ActiveSupport::Cache::RedisCacheStore`, `Net::HTTP`, `Faraday` (default adapter) and `rack-timeout` are forbidden — each blocks the fiber reactor.
- **No changes to `apps/api_geteway` or `apps/frontend`** in this plan.
- **No ActiveRecord, ActiveStorage, ActionMailer, ActionMailbox, ActionText, ActiveJob.**
- **Layer dependency direction is one-way:** `controllers → operations → gateways → resilience`, with `presenters` and `entities` as leaves. A gateway must never reference a controller, presenter or Rails helper.
- **Controllers contain no business logic, no `Async` blocks, and no HTTP.** Target ≤ 10 lines per action.
- **A component is one name across three files** sharing a single root CSS class: `app/views/shared/_name.html.erb`, `app/assets/stylesheets/components/_name.css`, and optionally `app/javascript/controllers/name_controller.js`. A component stylesheet may only style selectors nested under its own root class.
- **Positioning and `transform` never share an element.** An outer element owns placement; an inner element owns `transform`. This is what prevents the footer regression documented in spec §9.
- Service port **3000**. `apps/frontend` keeps **4173** and is not touched.
- Ruby base image `ruby:3.4.2`, matching `apps/api_geteway/Dockerfile`.

### Upstream API facts (verified, do not re-derive)

- Base path for public data: `{API_GETEWAY_URL}/public`, for auth: `{API_GETEWAY_URL}/auth`.
- `POST /auth/sessions` — body `{"email": "...", "password": "..."}` → `201 {"access_token": "...", "refresh_token": "..."}`.
- `POST /auth/sessions/refresh` — body `{"refresh_token": "..."}` → same shape.
- `DELETE /auth/sessions` — body `{"refresh_token": "..."}`.
- Authenticated calls use `Authorization: Bearer <access_token>`.
- `GET /public/teachers?q=&page=&per_page=` — JSON:API collection. `q` requires ≥ 2 chars; `per_page` ≤ 100. Serializer emits **no `meta` totals** (`slices/portal/serializers/teachers/index.rb`).
- `GET /public/rooms?page=&per_page=` — JSON:API collection, no `q` filter, no `meta` totals.
- Access tokens are JWTs; the role claim is decoded client-side without verification for display only. Authorization is always enforced upstream.

---

## File Structure

> **Namespaced layers live under `app/lib/`, never directly under `app/`.**
> Every direct subdirectory of `app/` is a Zeitwerk autoload *root*, not a
> namespace, so `app/resilience/breaker.rb` would have to define a top-level
> `Breaker`; the `module Resilience` wrapper raises
> `Zeitwerk::NameError: expected file ... to define constant Breaker`, and
> because production eager-loads, the container boots unhealthy and refuses
> every connection. `app/lib/resilience/breaker.rb` correctly resolves to
> `Resilience::Breaker`, and `app/lib` is picked up automatically -- it needs no
> `config.autoload_paths` entry. Controllers, models, views, helpers and
> javascript stay directly under `app/`: those are legitimate Rails roots whose
> files define top-level constants. An earlier draft of this plan specified the
> `app/<layer>/` form and it bricked the container during Task 3.

```
apps/schedule_web/
├── Dockerfile                              Task 1
├── Gemfile                                 Task 1
├── config.ru                               Task 1
├── config/
│   ├── application.rb                      Task 1   framework opt-outs, fiber isolation
│   ├── environments/{development,production}.rb   Task 1
│   ├── falcon.rb                           Task 1   forked reactor config
│   ├── routes.rb                           Tasks 1, 5, 9
│   ├── locales/ru.yml                      Tasks 7-9  ALL visitor-facing strings
│   └── initializers/
│       ├── settings.rb                     Task 2   dry-configurable, boot-time validation
│       ├── container.rb                    Task 2   dry-container registrations
│       ├── redis.rb                        Task 2   async-redis pool
│       ├── rack_attack.rb                  Task 6
│       ├── security_headers.rb             Task 6   CSP + headers
│       └── logging.rb                      Task 6   structured JSON logs
└── app/
    ├── lib/                                 all domain layers live here — see below
    │   ├── resilience/                     Task 3
    │   │   ├── outcome.rb                    Fresh | Stale | Failed
    │   │   ├── breaker.rb                    Redis circuit breaker
    │   │   ├── stale_cache.rb                Redis fresh/stale cache
    │   │   ├── retry_policy.rb               attempts + jittered backoff
    │   │   ├── call.rb                       the single outbound path
    │   │   └── rack_attack_store.rb          Task 6   non-blocking store adapter
    │   ├── gateways/                        Task 4
    │   │   ├── http_client.rb                async-http, persistent, per-host
    │   │   ├── json_api.rb                   JSON:API document → entities
    │   │   ├── base_gateway.rb
    │   │   ├── portal/{teachers,rooms}_gateway.rb
    │   │   └── auth_gateway.rb
    │   ├── entities/                        Task 4
    │   │   ├── types.rb, teacher.rb, room.rb, account.rb
    │   ├── session/                         Task 5
    │   │   ├── token_store.rb, refresh.rb, consent.rb
    │   ├── contracts/                       Task 5
    │   │   └── sign_in.rb
    │   ├── operations/                      Task 5
    │   │   ├── sign_in.rb, sign_out.rb
    │   ├── policies/                        Task 9
    │   │   └── home_floors.rb
    │   └── presenters/                      Tasks 7, 9
    │       ├── base.rb, home_page.rb
    ├── controllers/
    │   ├── application_controller.rb        Tasks 1, 5, 9
    │   ├── health_controller.rb             Task 3   /readyz
    │   ├── sessions_controller.rb           Task 5
    │   ├── consents_controller.rb           Task 5
    │   └── home_controller.rb               Task 9
    ├── views/
    │   ├── layouts/application.html.erb     Task 7
    │   ├── shared/                          Tasks 7, 8
    │   │   ├── _site_header.html.erb, _site_footer.html.erb
    │   │   ├── _stale_banner.html.erb, _region_error.html.erb
    │   │   ├── _auth_dialog.html.erb, _contacts_dialog.html.erb
    │   │   ├── _cookie_dialog.html.erb, _guide_dialog.html.erb
    │   └── home/index.html.erb              Task 9
    ├── javascript/controllers/              Tasks 7-9
    │   ├── dialog_controller.js, cookie_consent_controller.js
    │   └── floor_controller.js
    └── assets/stylesheets/
        ├── application.css                  Task 7   tokens + reset only
        └── components/                      Tasks 7-9  one file per component
```

**Parallelisation.** Tasks 1→2 are sequential. Then Task 3 and Task 7 are independent and may run concurrently. Task 4 needs 2+3. Task 5 needs 4. Task 6 needs 2 (and 3 for the store adapter). Task 8 needs 7. Task 9 needs 5+7+8. Task 10 is last.

---

## Task 1: Rails skeleton, Falcon, Docker

**Files:**
- Create: `apps/schedule_web/{Gemfile,config.ru,Dockerfile,.dockerignore}`
- Create: `apps/schedule_web/config/{application.rb,boot.rb,environment.rb,routes.rb,falcon.rb}`
- Create: `apps/schedule_web/config/environments/{development.rb,production.rb}`
- Create: `apps/schedule_web/app/controllers/application_controller.rb`
- Modify: `docker-compose.yml` (root) — add the `schedule_web` service

**Interfaces:**
- Consumes: nothing.
- Produces: a bootable Rails app named `ScheduleWeb`; `ApplicationController < ActionController::Base`; `/up` returning 200.

- [x] **Step 1: Generate the app**

Run from the repo root. The skips are load-bearing — see Global Constraints.

```bash
gem install rails
rails new apps/schedule_web \
  --skip-active-record --skip-active-storage --skip-action-mailer \
  --skip-action-mailbox --skip-action-text --skip-active-job \
  --skip-test --skip-system-test --skip-jbuilder --skip-kamal \
  --skip-solid --skip-devcontainer --css=tailwind=false --javascript=importmap
```

If `--css=tailwind=false` is rejected by the installed Rails version, omit the flag entirely; this service uses hand-written CSS and must not have Tailwind.

- [x] **Step 2: Replace the Gemfile**

`apps/schedule_web/Gemfile`:

```ruby
# frozen_string_literal: true

source "https://rubygems.org"
ruby "3.4.2"

gem "rails", "~> 8.0"
gem "propshaft"
gem "importmap-rails"
gem "turbo-rails"
gem "stimulus-rails"

# Fiber reactor. Falcon replaces Puma; async-http and async-redis keep every
# outbound call non-blocking so one request never parks the reactor.
gem "falcon", "~> 0.52"
gem "async", "~> 2.42"
gem "async-http", "~> 0.92"
gem "async-redis"

# dry-rb. Mirrors apps/api_geteway so both Ruby services read the same way.
gem "dry-configurable"
gem "dry-container"
gem "dry-auto_inject"
gem "dry-types"
gem "dry-struct"
gem "dry-validation"
gem "dry-monads"
gem "dry-operation", ">= 1.0.1"

gem "rack-attack"

group :development do
  gem "dotenv"
end
```

Then `cd apps/schedule_web && bundle install`.

- [x] **Step 3: Configure the application**

`apps/schedule_web/config/application.rb`:

```ruby
# frozen_string_literal: true

require_relative "boot"

require "rails"
require "action_controller/railtie"
require "action_view/railtie"
require "rails/health_controller"

Bundler.require(*Rails.groups)

module ScheduleWeb
  class Application < Rails::Application
    config.load_defaults 8.0

    # Every request is a Fiber under Falcon, so per-request isolated state must
    # key off the Fiber rather than the Thread. Without this, CurrentAttributes
    # set in one request can leak into another running on the same thread.
    config.active_support.isolation_level = :fiber

    config.api_only = false
    config.time_zone = "Europe/Moscow"
    config.i18n.default_locale = :ru
    config.i18n.available_locales = [:ru]

    # Domain layers. Zeitwerk autoloads every app/* directory by default; these
    # are listed to document the layering, not to add paths.
    config.autoload_paths += %W[
      #{root}/app/resilience
      #{root}/app/gateways
      #{root}/app/entities
      #{root}/app/session
      #{root}/app/contracts
      #{root}/app/operations
      #{root}/app/policies
      #{root}/app/presenters
    ]

    config.generators do |g|
      g.assets false
      g.helper false
    end
  end
end
```

- [x] **Step 4: Configure Falcon**

`apps/schedule_web/config/falcon.rb`:

```ruby
# frozen_string_literal: true

# One forked process per core, each running its own reactor. Fibers give
# concurrency within a process; forking gives parallelism across cores.
service "schedule_web" do
  include Falcon::Environment::Rack

  count Integer(ENV.fetch("WEB_CONCURRENCY", Etc.nprocessors))
  endpoint { Async::HTTP::Endpoint.parse("http://0.0.0.0:3000") }
end
```

- [x] **Step 5: Routes and base controller**

`apps/schedule_web/config/routes.rb`:

```ruby
# frozen_string_literal: true

Rails.application.routes.draw do
  get "up" => "rails/health#show", as: :rails_health_check
  root "home#index"
end
```

`apps/schedule_web/app/controllers/application_controller.rb`:

```ruby
# frozen_string_literal: true

class ApplicationController < ActionController::Base
  protect_from_forgery with: :exception
end
```

Create a placeholder `app/controllers/home_controller.rb` with an empty `index` and `app/views/home/index.html.erb` containing `<p>schedule_web</p>` so the root route resolves. Task 9 replaces both.

- [x] **Step 6: Dockerfile**

`apps/schedule_web/Dockerfile`:

```dockerfile
FROM ruby:3.4.2

RUN apt-get update -qq && apt-get install -y --no-install-recommends \
      build-essential git curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY Gemfile Gemfile.lock ./
RUN bundle install

COPY . .

EXPOSE 3000

CMD ["bundle", "exec", "falcon", "host", "config/falcon.rb"]
```

`apps/schedule_web/.dockerignore`:

```
.git
log
tmp
node_modules
```

- [x] **Step 7: Add the compose service**

In the root `docker-compose.yml`, add after the `frontend` service. `SECRET_KEY_BASE` must be added to `.env.local` — generate with `bundle exec rails secret`.

```yaml
  schedule_web:
    build:
      context: ./apps/schedule_web
      dockerfile: Dockerfile
    container_name: schedule_web
    env_file:
      - .env.local
    environment:
      RAILS_ENV: production
      API_GETEWAY_URL: http://api_geteway:2300
      REDIS_URL: redis://:${REDIS_PASSWORD}@redis:6379/1
      WEB_CONCURRENCY: "2"
    ports:
      - "3000:3000"
    depends_on:
      - api_geteway
      - redis
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:3000/up"]
      interval: 15s
      timeout: 5s
      retries: 5
      start_period: 20s
    restart: unless-stopped
    logging: *default-logging
```

- [x] **Step 8: Verify**

```bash
cd apps/schedule_web && bundle exec falcon host config/falcon.rb
```

Expected: process starts, no exception. In another shell:

```bash
curl -s -o /dev/null -w "%{http_code}\n" --noproxy '*' http://localhost:3000/up
```

Expected: `200`. (`--noproxy '*'` is required — this shell has `HTTP_PROXY` set and would otherwise get a proxy 502.)

Then:

```bash
docker compose build schedule_web && docker compose up -d schedule_web
docker compose ps schedule_web
```

Expected: state `healthy` within ~40s.

- [x] **Step 9: Report changed files** (do not commit)

---

## Task 2: Settings, container, Redis

**Files:**
- Create: `apps/schedule_web/config/initializers/{settings.rb,redis.rb,container.rb}`

**Interfaces:**
- Consumes: Task 1's application.
- Produces:
  - `ScheduleWeb::Settings` — a `Dry::Configurable` singleton. Readers: `api_geteway_url`, `redis_url`, `timeout_connect/read/total` (Float), `breaker_threshold/window/cooldown` (Integer), `cache_fresh_ttl/cache_stale_ttl` (Integer), `rate_limit_general/rate_limit_login` (Integer), `trusted_proxies` (Array<String>).
  - `ScheduleWeb::Redis.call { |client| ... }` — yields an `Async::Redis::Client` from a capped pool. Returns the block's value.
  - `ScheduleWeb::Container` — a `Dry::Container` instance.
  - `ScheduleWeb::Import = Dry::AutoInject(ScheduleWeb::Container)`.

- [x] **Step 1: Settings with boot-time validation**

`config/initializers/settings.rb`. Fail fast: a malformed env var must stop boot, not surface as a 500 on the first request.

```ruby
# frozen_string_literal: true

require "dry/configurable"
require "dry/types"

module ScheduleWeb
  module Types
    include Dry.Types()
  end

  # Reads and coerces every environment variable the service depends on.
  # Raises at boot when one is missing or malformed.
  class Settings
    extend Dry::Configurable

    setting :api_geteway_url,    default: ENV["API_GETEWAY_URL"],  constructor: Types::String.constrained(filled: true)
    setting :redis_url,          default: ENV["REDIS_URL"],        constructor: Types::String.constrained(filled: true)

    setting :timeout_connect,    default: ENV.fetch("UPSTREAM_TIMEOUT_CONNECT", "1.0"),  constructor: Types::Coercible::Float
    setting :timeout_read,       default: ENV.fetch("UPSTREAM_TIMEOUT_READ", "3.0"),     constructor: Types::Coercible::Float
    setting :timeout_total,      default: ENV.fetch("UPSTREAM_TIMEOUT_TOTAL", "5.0"),    constructor: Types::Coercible::Float

    setting :breaker_threshold,  default: ENV.fetch("BREAKER_THRESHOLD", "5"),  constructor: Types::Coercible::Integer
    setting :breaker_window,     default: ENV.fetch("BREAKER_WINDOW", "30"),    constructor: Types::Coercible::Integer
    setting :breaker_cooldown,   default: ENV.fetch("BREAKER_COOLDOWN", "15"),  constructor: Types::Coercible::Integer

    setting :cache_fresh_ttl,    default: ENV.fetch("CACHE_FRESH_TTL", "60"),      constructor: Types::Coercible::Integer
    setting :cache_stale_ttl,    default: ENV.fetch("CACHE_STALE_TTL", "86400"),   constructor: Types::Coercible::Integer

    setting :rate_limit_general, default: ENV.fetch("RATE_LIMIT_GENERAL", "300"), constructor: Types::Coercible::Integer
    setting :rate_limit_login,   default: ENV.fetch("RATE_LIMIT_LOGIN", "10"),    constructor: Types::Coercible::Integer

    setting :redis_pool_size,    default: ENV.fetch("REDIS_POOL_SIZE", "16"),     constructor: Types::Coercible::Integer

    setting :trusted_proxies,    default: ENV.fetch("TRUSTED_PROXIES", ""),
            constructor: ->(value) { value.to_s.split(",").map(&:strip).reject(&:empty?) }
  end
end

# Touch every setting so a bad value raises here rather than mid-request.
ScheduleWeb::Settings.config.values.each_key { |key| ScheduleWeb::Settings.config.public_send(key) }
```

- [x] **Step 2: Non-blocking Redis pool**

`config/initializers/redis.rb`. `async-redis` is mandatory: the `redis` gem blocks the reactor.

```ruby
# frozen_string_literal: true

require "async/redis"

module ScheduleWeb
  # A capped pool of async-redis clients. Blocking Redis clients would park the
  # whole reactor on every round-trip, so every Redis access goes through here.
  module Redis
    ENDPOINT = Async::Redis::Endpoint.parse(ScheduleWeb::Settings.config.redis_url)

    LIMIT = Async::Semaphore.new(ScheduleWeb::Settings.config.redis_pool_size)

    module_function

    # Yields a client. Returns the block's value, or nil when Redis is
    # unreachable — callers treat Redis as an optimisation, never a dependency
    # for correctness (see Resilience::StaleCache).
    def call
      LIMIT.acquire do
        client = Async::Redis::Client.new(ENDPOINT)
        begin
          yield client
        ensure
          client.close
        end
      end
    rescue StandardError => error
      Rails.logger.warn({ event: "redis_unavailable", error: error.class.name, message: error.message }.to_json)
      nil
    end

    # True when a PING round-trips. Used by /readyz.
    def available?
      call { |client| client.call("PING") } == "PONG"
    end
  end
end
```

- [x] **Step 3: Container and injector**

`config/initializers/container.rb`. Registrations for later tasks are added by those tasks.

```ruby
# frozen_string_literal: true

require "dry/container"
require "dry/auto_inject"

module ScheduleWeb
  class Container
    extend Dry::Container::Mixin
  end

  Import = Dry::AutoInject(Container)
end
```

- [x] **Step 4: Verify**

```bash
cd apps/schedule_web && bundle exec rails runner 'puts ScheduleWeb::Settings.config.timeout_read.class; puts ScheduleWeb::Settings.config.breaker_threshold'
```

Expected: `Float` then `5`.

Fail-fast check — expect a raise, not a nil:

```bash
API_GETEWAY_URL= bundle exec rails runner 'puts :boot' 2>&1 | tail -3
```

Expected: a `Dry::Types::ConstraintError` mentioning `filled?`.

- [x] **Step 5: Report changed files** (do not commit)

---

## Task 3: Resilience layer and /readyz

**Files:**
- Create: `apps/schedule_web/app/lib/resilience/{outcome.rb,breaker.rb,stale_cache.rb,retry_policy.rb,call.rb}`
- Create: `apps/schedule_web/app/controllers/health_controller.rb`
- Modify: `apps/schedule_web/config/routes.rb`
- Modify: `apps/schedule_web/config/initializers/container.rb`

**Interfaces:**
- Consumes: `ScheduleWeb::Redis.call`, `ScheduleWeb::Settings`.
- Produces:
  - `Resilience::Outcome::Fresh.new(data:)`, `Resilience::Outcome::Stale.new(data:, cached_at:)` — `cached_at` is a `Time`.
  - `Resilience::Breaker.new(key:)` with `#allow?` → Boolean, `#record_success`, `#record_failure`, `#state` → `:closed | :open`.
  - `Resilience::StaleCache.new(key:)` with `#fresh(params)` → data or nil, `#stale(params)` → `[data, cached_at]` or nil, `#write(params, data)`.
  - `Resilience::Call.new(key:, cacheable: true)` with `#call(params: {}, &block)` → `Dry::Monads::Result`. `Success` wraps an `Outcome`; `Failure` wraps a Symbol reason (`:circuit_open`, `:timeout`, `:upstream_error`, `:transport_error`).
  - `Resilience.breaker_states` → `Hash[String => Symbol]` for `/readyz`.

- [x] **Step 1: Outcome**

`app/lib/resilience/outcome.rb`:

```ruby
# frozen_string_literal: true

module Resilience
  # The three states every presenter must handle. Modelling staleness in the
  # type — rather than as a nullable timestamp beside the data — makes it
  # impossible to render cached data without deciding what to say about its age.
  module Outcome
    Fresh = Struct.new(:data, keyword_init: true) do
      def stale? = false
      def cached_at = nil
    end

    Stale = Struct.new(:data, :cached_at, keyword_init: true) do
      def stale? = true
    end
  end
end
```

- [x] **Step 2: Circuit breaker**

`app/lib/resilience/breaker.rb`. State lives in Redis so all forked Falcon processes share one view.

```ruby
# frozen_string_literal: true

module Resilience
  # Counts consecutive failures per key. Past the threshold the circuit opens
  # and calls fail immediately for `cooldown` seconds, sparing a struggling
  # upstream the additional load of a retry storm.
  class Breaker
    def initialize(key:, settings: ScheduleWeb::Settings.config)
      @key = key
      @threshold = settings.breaker_threshold
      @window = settings.breaker_window
      @cooldown = settings.breaker_cooldown
    end

    def allow?
      state != :open
    end

    def state
      opened = ScheduleWeb::Redis.call { |client| client.call("GET", open_key) }
      opened ? :open : :closed
    end

    def record_success
      ScheduleWeb::Redis.call do |client|
        client.call("DEL", failure_key)
        client.call("DEL", open_key)
      end
    end

    def record_failure
      ScheduleWeb::Redis.call do |client|
        count = client.call("INCR", failure_key).to_i
        client.call("EXPIRE", failure_key, @window) if count == 1
        if count >= @threshold
          client.call("SET", open_key, "1", "EX", @cooldown)
          client.call("DEL", failure_key)
        end
      end
    end

    private

    def failure_key = "breaker:#{@key}:failures"
    def open_key    = "breaker:#{@key}:open"
  end
end
```

- [x] **Step 3: Stale cache**

`app/lib/resilience/stale_cache.rb`:

```ruby
# frozen_string_literal: true

require "digest"

module Resilience
  # Stores the last good response per key+params. Within `fresh_ttl` the copy is
  # served without touching the network; between `fresh_ttl` and `stale_ttl` it
  # is served only when the upstream call fails.
  class StaleCache
    def initialize(key:, settings: ScheduleWeb::Settings.config)
      @key = key
      @fresh_ttl = settings.cache_fresh_ttl
      @stale_ttl = settings.cache_stale_ttl
    end

    def fresh(params)
      entry = read(params)
      return nil unless entry
      return nil if Time.now - entry.fetch(:cached_at) > @fresh_ttl

      entry.fetch(:data)
    end

    def stale(params)
      entry = read(params)
      return nil unless entry

      [entry.fetch(:data), entry.fetch(:cached_at)]
    end

    def write(params, data)
      payload = JSON.generate({ "cached_at" => Time.now.to_i, "data" => data })
      ScheduleWeb::Redis.call { |client| client.call("SET", cache_key(params), payload, "EX", @stale_ttl) }
    end

    private

    def read(params)
      raw = ScheduleWeb::Redis.call { |client| client.call("GET", cache_key(params)) }
      return nil unless raw

      parsed = JSON.parse(raw)
      { data: parsed.fetch("data"), cached_at: Time.at(parsed.fetch("cached_at")) }
    rescue JSON::ParserError, KeyError
      nil
    end

    # Authenticated responses include the account id in `params`, so one
    # visitor's cached data can never be served to another.
    def cache_key(params)
      "cache:#{@key}:#{Digest::SHA256.hexdigest(JSON.generate(params))}"
    end
  end
end
```

- [x] **Step 4: Retry policy**

`app/lib/resilience/retry_policy.rb`:

```ruby
# frozen_string_literal: true

module Resilience
  # Exponential backoff with full jitter. Jitter matters more than the base
  # delay: without it, every fiber that failed at the same instant retries at
  # the same instant, reproducing the spike that caused the failure.
  class RetryPolicy
    BASE_DELAY = 0.1

    def initialize(attempts: 2)
      @attempts = attempts
    end

    def each_attempt
      attempt = 0
      begin
        attempt += 1
        yield attempt
      rescue Resilience::RetryableError
        raise if attempt >= @attempts

        sleep(rand * BASE_DELAY * (2**(attempt - 1)))
        retry
      end
    end
  end

  # Raised by Resilience::Call's block for conditions worth another attempt:
  # timeouts, connection resets, and 5xx responses. 4xx is never retryable.
  class RetryableError < StandardError; end
end
```

Note: `sleep` inside an `Async` task yields the reactor rather than blocking the thread — `Async` patches `Kernel#sleep` via the fiber scheduler. No `Async::Task.current.sleep` needed.

- [x] **Step 5: The single outbound path**

`app/lib/resilience/call.rb`:

```ruby
# frozen_string_literal: true

require "dry/monads"

module Resilience
  # Every outbound call goes through here. Gateways receive an instance by
  # injection and have no other way to reach the network, so timeouts, retry,
  # breaker and cache cannot be forgotten at a call site.
  class Call
    include Dry::Monads[:result]

    REGISTRY = {}
    REGISTRY_MUTEX = Mutex.new

    def self.register(key)
      REGISTRY_MUTEX.synchronize { REGISTRY[key] = true }
    end

    def self.breaker_states
      REGISTRY_MUTEX.synchronize { REGISTRY.keys }.to_h { |key| [key, Breaker.new(key: key).state] }
    end

    def initialize(key:, cacheable: true, settings: ScheduleWeb::Settings.config)
      @key = key
      @cacheable = cacheable
      @settings = settings
      @breaker = Breaker.new(key: key)
      @cache = StaleCache.new(key: key)
      @retry_policy = RetryPolicy.new
      self.class.register(key)
    end

    # `params` identifies the request for caching and MUST include the account
    # id for authenticated calls. The block performs the actual request and
    # returns a parsed Ruby value, or raises RetryableError / StandardError.
    def call(params: {})
      if @cacheable && (hit = @cache.fresh(params))
        return Success(Outcome::Fresh.new(data: hit))
      end

      return degraded(params, :circuit_open) unless @breaker.allow?

      data = nil
      @retry_policy.each_attempt do
        data = Async::Task.current.with_timeout(@settings.timeout_total) { yield }
      end

      @breaker.record_success
      @cache.write(params, data) if @cacheable
      Success(Outcome::Fresh.new(data: data))
    rescue Async::TimeoutError
      @breaker.record_failure
      degraded(params, :timeout)
    rescue RetryableError
      @breaker.record_failure
      degraded(params, :upstream_error)
    rescue StandardError => error
      @breaker.record_failure
      log_failure(error)
      degraded(params, :transport_error)
    end

    private

    # A cached copy, however old, beats an error region — as long as the view
    # says how old it is.
    def degraded(params, reason)
      if @cacheable && (cached = @cache.stale(params))
        data, cached_at = cached
        return Success(Outcome::Stale.new(data: data, cached_at: cached_at))
      end

      Failure(reason)
    end

    def log_failure(error)
      Rails.logger.warn({ event: "upstream_failure", key: @key, error: error.class.name, message: error.message }.to_json)
    end
  end
end
```

- [x] **Step 6: Readiness endpoint**

`app/controllers/health_controller.rb`:

```ruby
# frozen_string_literal: true

# /up (Rails built-in) answers "is the process alive" — a failure there should
# restart the container. /readyz answers "can this instance serve correctly" —
# a failure there should remove it from rotation without restarting it.
class HealthController < ApplicationController
  skip_forgery_protection

  def readyz
    redis_ok = ScheduleWeb::Redis.available?
    breakers = Resilience::Call.breaker_states

    render json: {
      status: redis_ok ? "ready" : "degraded",
      redis: redis_ok,
      breakers: breakers
    }, status: redis_ok ? :ok : :service_unavailable
  end
end
```

Add to `config/routes.rb`, above `root`:

```ruby
  get "readyz" => "health#readyz"
```

- [x] **Step 7: Verify**

```bash
docker compose up -d redis
cd apps/schedule_web && bundle exec falcon host config/falcon.rb &
curl -s --noproxy '*' http://localhost:3000/readyz
```

Expected: `{"status":"ready","redis":true,"breakers":{}}` with HTTP 200.

Now stop Redis and re-check:

```bash
docker compose stop redis
curl -s -w " [%{http_code}]\n" --noproxy '*' http://localhost:3000/readyz
docker compose start redis
```

Expected: `{"status":"degraded","redis":false,...}` with `[503]`.

- [x] **Step 8: Report changed files** (do not commit)

---

## Task 4: HTTP transport, entities, gateways

**Files:**
- Create: `apps/schedule_web/app/lib/gateways/{http_client.rb,json_api.rb,base_gateway.rb,auth_gateway.rb}`
- Create: `apps/schedule_web/app/lib/gateways/portal/{teachers_gateway.rb,rooms_gateway.rb}`
- Create: `apps/schedule_web/app/lib/entities/{types.rb,teacher.rb,room.rb,account.rb}`
- Modify: `apps/schedule_web/config/initializers/container.rb`

**Interfaces:**
- Consumes: `Resilience::Call`, `ScheduleWeb::Settings`.
- Produces:
  - `Gateways::HttpClient#request(method:, path:, headers: {}, body: nil)` → `Struct(status:, body:)`; raises `Resilience::RetryableError` on 5xx and transport faults.
  - `Gateways::JsonApi.collection(document)` → `Array<Hash>` with relationships resolved from `included` and merged under symbol keys.
  - `Entities::Teacher`, `Entities::Room`, `Entities::Account` — `Dry::Struct`s with `.from_json_api(hash)`.
  - `Gateways::Portal::TeachersGateway#index(query: nil, page: 1, per_page: 20)` → `Result<Outcome<Array<Entities::Teacher>>>`.
  - `Gateways::Portal::RoomsGateway#index(page: 1, per_page: 20)` → `Result<Outcome<Array<Entities::Room>>>`.
  - `Gateways::AuthGateway#sign_in(email:, password:)` / `#refresh(refresh_token:)` / `#sign_out(refresh_token:)` → `Result<Hash>` with `"access_token"` / `"refresh_token"` string keys.
  - Container keys: `"gateways.teachers"`, `"gateways.rooms"`, `"gateways.auth"`.

- [x] **Step 1: Non-blocking HTTP client**

`app/lib/gateways/http_client.rb`. One persistent client per host, created lazily per process.

```ruby
# frozen_string_literal: true

require "async/http"
require "protocol/http/headers"

module Gateways
  # The only outbound HTTP in the service. Uses async-http so a request in
  # flight yields the reactor, letting sibling fibers serve assets and other
  # pages meanwhile.
  class HttpClient
    Response = Struct.new(:status, :body, keyword_init: true)

    def initialize(base_url: ScheduleWeb::Settings.config.api_geteway_url)
      @endpoint = Async::HTTP::Endpoint.parse(base_url)
      @base_path = @endpoint.path.to_s.chomp("/")
    end

    def request(method:, path:, headers: {}, body: nil)
      full_path = "#{@base_path}#{path}"
      encoded = body.nil? ? nil : [JSON.generate(body)]

      response = client.public_send(
        method.to_s.downcase,
        full_path,
        ::Protocol::HTTP::Headers[default_headers.merge(headers).to_a],
        encoded
      )

      begin
        raw = response.read
        raise Resilience::RetryableError, "upstream #{response.status}" if response.status >= 500

        Response.new(status: response.status, body: raw)
      ensure
        response.close
      end
    rescue Errno::ECONNREFUSED, Errno::ECONNRESET, Errno::EPIPE, IOError, SocketError => error
      raise Resilience::RetryableError, error.message
    end

    private

    # Per-fiber-scheduler client reuse. Falcon forks, so each process gets its
    # own connection pool to the upstream.
    def client
      @client ||= Async::HTTP::Client.new(@endpoint)
    end

    def default_headers
      {
        "accept" => "application/json",
        "content-type" => "application/json",
        "x-request-id" => (::Current.request_id if defined?(::Current) && ::Current.respond_to?(:request_id)).to_s
      }.compact_blank
    end
  end
end
```

- [x] **Step 2: JSON:API flattening**

`app/lib/gateways/json_api.rb`:

```ruby
# frozen_string_literal: true

module Gateways
  # Flattens a JSON:API document into plain hashes, resolving each
  # relationship from `included` and merging it under a symbol key. Entities
  # never see JSON:API structure.
  module JsonApi
    module_function

    def collection(document)
      data = document["data"] || []
      included = index_included(document["included"])
      data.map { |resource| flatten(resource, included) }
    end

    def resource(document)
      return nil unless document["data"]

      flatten(document["data"], index_included(document["included"]))
    end

    def flatten(resource, included)
      base = { id: resource["id"].to_s }.merge((resource["attributes"] || {}).symbolize_keys)

      (resource["relationships"] || {}).each do |name, relationship|
        ref = relationship.dig("data")
        next if ref.nil? || ref.is_a?(Array)

        related = included[[ref["type"], ref["id"].to_s]]
        base[name.to_sym] = related ? flatten(related, included) : nil
      end

      base
    end

    def index_included(included)
      (included || []).to_h { |item| [[item["type"], item["id"].to_s], item] }
    end
  end
end
```

- [x] **Step 3: Entities**

`app/lib/entities/types.rb`:

```ruby
# frozen_string_literal: true

module Entities
  module Types
    include Dry.Types()
  end
end
```

`app/lib/entities/teacher.rb`:

```ruby
# frozen_string_literal: true

module Entities
  class Teacher < Dry::Struct
    transform_keys(&:to_sym)

    attribute :id, Types::String
    attribute :surname,     Types::String.optional.default(nil)
    attribute :name,        Types::String.optional.default(nil)
    attribute :patronymic,  Types::String.optional.default(nil)
    attribute :post_name,   Types::String.optional.default(nil)

    def self.from_json_api(hash)
      new(
        id: hash[:id],
        surname: hash[:surname], name: hash[:name], patronymic: hash[:patronymic],
        post_name: hash.dig(:post, :name)
      )
    end

    # "Иванов И. И." — surname in full, initials abbreviated.
    def display_name
      initials = [name, patronymic].compact.reject(&:empty?).map { |part| "#{part[0]}." }
      [surname, *initials].compact.join(" ").strip
    end
  end
end
```

`app/lib/entities/room.rb`:

```ruby
# frozen_string_literal: true

module Entities
  class Room < Dry::Struct
    transform_keys(&:to_sym)

    attribute :id, Types::String
    attribute :number,          Types::String.optional.default(nil)
    attribute :name,            Types::String.optional.default(nil)
    attribute :building_number, Types::String.optional.default(nil)
    attribute :room_type_name,  Types::String.optional.default(nil)

    def self.from_json_api(hash)
      new(
        id: hash[:id], number: hash[:number]&.to_s, name: hash[:name],
        building_number: hash.dig(:building, :number)&.to_s,
        room_type_name: hash.dig(:room_type, :name)
      )
    end

    def display_number = number.presence || name.to_s
  end
end
```

`app/lib/entities/account.rb`:

```ruby
# frozen_string_literal: true

require "base64"

module Entities
  class Account < Dry::Struct
    STAFF_ROLES = %w[super_admin faculty_admin kafedra_admin scheduler].freeze

    transform_keys(&:to_sym)

    attribute :id,    Types::String
    attribute :email, Types::String
    attribute :role,  Types::String
    attribute :must_change_password, Types::Bool.default(false)
    attribute :expires_at, Types::Integer.default(0)

    # Decodes the JWT payload WITHOUT verifying the signature. This is display
    # only — every authorization decision is made upstream by api_geteway,
    # which does verify. Never gate a security-relevant action on this.
    #
    # Verified claim set (decoded from a live token 2026-08-13):
    #   sub, role, teacher_id, must_change_password, scopes, jti, iat, exp
    def self.from_access_token(token, email)
      payload = JSON.parse(Base64.urlsafe_decode64(pad(token.split(".")[1].to_s)))
      new(
        id: payload["sub"].to_s,
        email: email,
        role: payload["role"].to_s.presence || "unknown",
        must_change_password: payload["must_change_password"] == true,
        expires_at: payload["exp"].to_i
      )
    rescue StandardError
      new(id: "", email: email, role: "unknown")
    end

    def self.pad(segment) = segment + "=" * ((4 - segment.length % 4) % 4)

    def staff? = STAFF_ROLES.include?(role)

    # Access tokens live 900s. Treat a token within 60s of expiry as already
    # expired so a page's fan-out refreshes once up front rather than having
    # several concurrent calls each discover a 401 mid-render.
    def expiring_soon? = expires_at.positive? && Time.now.to_i >= (expires_at - 60)
  end
end
```

- [x] **Step 4: Base gateway**

`app/lib/gateways/base_gateway.rb`:

```ruby
# frozen_string_literal: true

module Gateways
  class BaseGateway
    def initialize(http: HttpClient.new)
      @http = http
    end

    private

    attr_reader :http

    def get_json(path, headers: {})
      JSON.parse(http.request(method: :get, path: path, headers: headers).body)
    end

    def query_string(params)
      filtered = params.compact.reject { |_, value| value.to_s.empty? }
      filtered.empty? ? "" : "?#{URI.encode_www_form(filtered)}"
    end
  end
end
```

- [x] **Step 5: Portal gateways**

`app/lib/gateways/portal/teachers_gateway.rb`:

```ruby
# frozen_string_literal: true

module Gateways
  module Portal
    class TeachersGateway < BaseGateway
      KEY = "portal.teachers.index"

      def initialize(http: HttpClient.new, call: Resilience::Call.new(key: KEY))
        super(http: http)
        @call = call
      end

      # `q` shorter than 2 characters is rejected upstream, so it is dropped here.
      def index(query: nil, page: 1, per_page: 20)
        normalized = query.to_s.strip
        normalized = nil if normalized.length < 2
        params = { q: normalized, page: page, per_page: per_page }

        @call.call(params: params) do
          document = get_json("/public/teachers#{query_string(params)}")
          JsonApi.collection(document).map { |row| Entities::Teacher.from_json_api(row).to_h }
        end
      end
    end
  end
end
```

`app/lib/gateways/portal/rooms_gateway.rb`:

```ruby
# frozen_string_literal: true

module Gateways
  module Portal
    # /public/rooms has page/per_page but no `q` — filtering is the caller's job.
    class RoomsGateway < BaseGateway
      KEY = "portal.rooms.index"

      def initialize(http: HttpClient.new, call: Resilience::Call.new(key: KEY))
        super(http: http)
        @call = call
      end

      def index(page: 1, per_page: 20)
        params = { page: page, per_page: per_page }

        @call.call(params: params) do
          document = get_json("/public/rooms#{query_string(params)}")
          JsonApi.collection(document).map { |row| Entities::Room.from_json_api(row).to_h }
        end
      end
    end
  end
end
```

Entities are cached as hashes (`.to_h`) rather than objects because the stale cache serializes to JSON. Callers rehydrate with `Entities::Teacher.new(hash)`.

- [x] **Step 6: Auth gateway**

`app/lib/gateways/auth_gateway.rb`. Not cacheable and not breaker-guarded for writes — a login must never return a cached token pair.

```ruby
# frozen_string_literal: true

module Gateways
  class AuthGateway < BaseGateway
    include Dry::Monads[:result]

    def sign_in(email:, password:)
      post("/auth/sessions", { email: email, password: password }, success: 201)
    end

    def refresh(refresh_token:)
      post("/auth/sessions/refresh", { refresh_token: refresh_token }, success: 200)
    end

    def sign_out(refresh_token:)
      http.request(method: :delete, path: "/auth/sessions", body: { refresh_token: refresh_token })
      Success(true)
    rescue StandardError
      # The local session is cleared regardless; a failed upstream revoke must
      # not strand the visitor in a signed-in state they cannot leave.
      Success(true)
    end

    private

    def post(path, body, success:)
      response = http.request(method: :post, path: path, body: body)
      return Failure(:invalid_credentials) unless response.status == success

      Success(JSON.parse(response.body))
    rescue Resilience::RetryableError
      Failure(:upstream_unavailable)
    rescue StandardError
      Failure(:upstream_unavailable)
    end
  end
end
```

- [x] **Step 7: Register in the container**

Append to `config/initializers/container.rb`:

```ruby
ScheduleWeb::Container.register("gateways.teachers") { Gateways::Portal::TeachersGateway.new }
ScheduleWeb::Container.register("gateways.rooms")    { Gateways::Portal::RoomsGateway.new }
ScheduleWeb::Container.register("gateways.auth")     { Gateways::AuthGateway.new }
```

- [x] **Step 8: Verify concurrency and correctness**

Start the API and Redis, then run. The whole script must sit inside `Async` because `Resilience::Call` uses `Async::Task.current`.

```bash
docker compose up -d api_geteway redis
cd apps/schedule_web && bundle exec rails runner '
require "async"
Async do
  started = Time.now
  barrier = Async::Barrier.new
  results = {}
  barrier.async { results[:teachers] = ScheduleWeb::Container["gateways.teachers"].index(per_page: 5) }
  barrier.async { results[:rooms]    = ScheduleWeb::Container["gateways.rooms"].index(per_page: 5) }
  barrier.wait
  results.each { |name, result| puts "#{name}: #{result.success? ? result.value!.data.length : result.failure} stale=#{result.success? && result.value!.stale?}" }
  puts "elapsed=#{(Time.now - started).round(3)}s"
end'
```

Expected: both lines report a count (e.g. `teachers: 5`), `stale=false`, and an elapsed time close to the slower single call rather than their sum — that is the proof the fan-out ran concurrently.

Now the failover path. Stop the API and re-run the same command:

```bash
docker compose stop api_geteway
```

Expected: both report counts again with `stale=true` — served from the Redis cache written by the first run. Restart with `docker compose start api_geteway`.

Finally, prove the breaker opens. Stop the API, run the command six times, then check:

```bash
curl -s --noproxy '*' http://localhost:3000/readyz
```

Expected: `"portal.teachers.index":"open"`.

- [x] **Step 9: Report changed files** (do not commit)

---

## Task 5: Session, authentication, cookie consent

**Files:**
- Create: `apps/schedule_web/app/lib/session/{token_store.rb,refresh.rb,consent.rb}`
- Create: `apps/schedule_web/app/lib/contracts/sign_in.rb`
- Create: `apps/schedule_web/app/lib/operations/{sign_in.rb,sign_out.rb}`
- Create: `apps/schedule_web/app/controllers/{sessions_controller.rb,consents_controller.rb}`
- Modify: `apps/schedule_web/app/controllers/application_controller.rb`
- Modify: `apps/schedule_web/config/{routes.rb,initializers/container.rb}`

**Interfaces:**
- Consumes: `Gateways::AuthGateway`, `Entities::Account`.
- Produces:
  - `Session::TokenStore.new(session)` with `#write(access_token:, refresh_token:, account:)`, `#clear`, `#account` → `Entities::Account | nil`, `#access_token`, `#refresh_token`, `#signed_in?`.
  - `Session::Consent.new(cookies)` with `#decided?`, `#categories` → `Array<Symbol>`, `#record(functional:, analytics:)`, `#allows?(category)`.
  - `Contracts::SignIn.new.call(params)` → `Dry::Validation::Result`.
  - `Operations::SignIn#call(email:, password:, session:)` → `Result<Entities::Account>`.
  - `ApplicationController#current_account`, `#signed_in?`, `#require_account` (before_action), helper methods `current_account` / `signed_in?` / `cookie_consent`.
  - Routes: `POST /session`, `DELETE /session`, `POST /consent`.

- [x] **Step 1: Token store**

`app/lib/session/token_store.rb`. The only object permitted to touch `session` — see spec §7 on the 4KB budget and the Redis fallback this isolation buys.

```ruby
# frozen_string_literal: true

module Session
  class TokenStore
    KEY = "auth"
    SIZE_WARN_BYTES = 3_072

    def initialize(session)
      @session = session
    end

    def write(access_token:, refresh_token:, account:)
      payload = {
        "access_token" => access_token,
        "refresh_token" => refresh_token,
        "account" => { "id" => account.id, "email" => account.email, "role" => account.role }
      }
      warn_if_large(payload)
      @session[KEY] = payload
    end

    def clear = @session.delete(KEY)

    def signed_in? = data.present?

    def access_token  = data&.dig("access_token")
    def refresh_token = data&.dig("refresh_token")

    def account
      raw = data&.dig("account")
      return nil unless raw

      Entities::Account.new(id: raw["id"], email: raw["email"], role: raw["role"])
    end

    private

    def data = @session[KEY]

    # A cookie is capped at 4KB and encryption inflates the payload ~1.4x. An
    # oversized Set-Cookie is silently dropped by the browser, which looks
    # exactly like a successful login that did not stick.
    def warn_if_large(payload)
      size = JSON.generate(payload).bytesize
      return if size < SIZE_WARN_BYTES

      Rails.logger.warn({ event: "session_payload_large", bytes: size }.to_json)
    end
  end
end
```

- [x] **Step 2: Cookie consent**

`app/lib/session/consent.rb`. Three categories, matching `schedule_home/index.html`'s cookie dialog: necessary (always on), functional, analytics.

```ruby
# frozen_string_literal: true

module Session
  # Stored outside the session so a decision survives sign-out, and versioned so
  # a policy change can invalidate previous grants.
  class Consent
    COOKIE = :cookie_consent
    VERSION = "v1"
    MAX_AGE = 1.year

    def initialize(cookies)
      @cookies = cookies
    end

    def decided? = raw.present? && raw.start_with?("#{VERSION}:")

    def categories
      return [:necessary] unless decided?

      raw.split(":", 2).last.split(",").map(&:to_sym)
    end

    def allows?(category) = categories.include?(category.to_sym)

    def record(functional:, analytics:)
      granted = [:necessary]
      granted << :functional if functional
      granted << :analytics if analytics

      @cookies[COOKIE] = {
        value: "#{VERSION}:#{granted.join(',')}",
        expires: MAX_AGE.from_now,
        httponly: true,
        same_site: :lax,
        secure: Rails.env.production?
      }
    end

    private

    def raw = @cookies[COOKIE].to_s
  end
end
```

- [x] **Step 3: Token refresh**

`app/lib/session/refresh.rb`:

```ruby
# frozen_string_literal: true

module Session
  # Rotates an expired access token exactly once per request. A page that fans
  # out four calls which all see 401 must perform one refresh, not four —
  # otherwise concurrent rotations invalidate one another and sign the visitor
  # out mid-render.
  class Refresh
    def initialize(store:, auth: ScheduleWeb::Container["gateways.auth"])
      @store = store
      @auth = auth
    end

    def call
      return false unless @store.refresh_token
      return Current.refresh_result unless Current.refresh_result.nil?

      Current.refresh_result = perform
    end

    private

    def perform
      result = @auth.refresh(refresh_token: @store.refresh_token)
      return clear_and_fail unless result.success?

      tokens = result.value!
      account = Entities::Account.from_access_token(tokens["access_token"], @store.account&.email.to_s)
      @store.write(access_token: tokens["access_token"], refresh_token: tokens["refresh_token"], account: account)
      true
    end

    def clear_and_fail
      @store.clear
      false
    end
  end
end
```

Add `app/models/current.rb` (Rails' `CurrentAttributes`, fiber-isolated per Task 1):

```ruby
# frozen_string_literal: true

class Current < ActiveSupport::CurrentAttributes
  attribute :request_id, :refresh_result
end
```

- [x] **Step 4: Sign-in contract and operations**

`app/lib/contracts/sign_in.rb`:

```ruby
# frozen_string_literal: true

require "dry/validation"

module Contracts
  class SignIn < Dry::Validation::Contract
    params do
      required(:email).filled(:string)
      required(:password).filled(:string)
    end
  end
end
```

`app/lib/operations/sign_in.rb`:

```ruby
# frozen_string_literal: true

module Operations
  class SignIn
    include Dry::Monads[:result]

    def initialize(auth: ScheduleWeb::Container["gateways.auth"])
      @auth = auth
    end

    def call(email:, password:, session:)
      result = @auth.sign_in(email: email, password: password)
      return Failure(result.failure) if result.failure?

      tokens = result.value!
      account = Entities::Account.from_access_token(tokens["access_token"], email)
      Session::TokenStore.new(session).write(
        access_token: tokens["access_token"], refresh_token: tokens["refresh_token"], account: account
      )
      Success(account)
    end
  end
end
```

`app/lib/operations/sign_out.rb`:

```ruby
# frozen_string_literal: true

module Operations
  class SignOut
    include Dry::Monads[:result]

    def initialize(auth: ScheduleWeb::Container["gateways.auth"])
      @auth = auth
    end

    def call(session:)
      store = Session::TokenStore.new(session)
      @auth.sign_out(refresh_token: store.refresh_token) if store.refresh_token
      store.clear
      Success(true)
    end
  end
end
```

- [x] **Step 5: Application controller**

Replace `app/controllers/application_controller.rb`:

```ruby
# frozen_string_literal: true

class ApplicationController < ActionController::Base
  protect_from_forgery with: :exception

  before_action :assign_request_id

  helper_method :current_account, :signed_in?, :cookie_consent

  private

  def assign_request_id
    Current.request_id = request.request_id
  end

  def token_store = @token_store ||= Session::TokenStore.new(session)

  def current_account = token_store.account

  def signed_in? = token_store.signed_in?

  def cookie_consent = @cookie_consent ||= Session::Consent.new(cookies)

  # Guards routes that expose non-public data. Stashes where the visitor was
  # heading so the post-login redirect lands there instead of the home page.
  def require_account
    return if signed_in?

    session[:return_to] = request.fullpath
    redirect_to root_path(auth: 1)
  end

  def consume_return_to
    session.delete(:return_to) || root_path
  end
end
```

- [x] **Step 6: Controllers and routes**

`app/controllers/sessions_controller.rb`:

```ruby
# frozen_string_literal: true

class SessionsController < ApplicationController
  def create
    validation = Contracts::SignIn.new.call(sign_in_params)
    return redirect_to(root_path(auth: 1), alert: t("auth.invalid")) if validation.failure?

    result = Operations::SignIn.new.call(**validation.to_h, session: session)
    return redirect_to(root_path(auth: 1), alert: t("auth.#{result.failure}")) if result.failure?

    redirect_to consume_return_to
  end

  def destroy
    Operations::SignOut.new.call(session: session)
    redirect_to root_path
  end

  private

  def sign_in_params = params.permit(:email, :password).to_h.symbolize_keys
end
```

`app/controllers/consents_controller.rb`:

```ruby
# frozen_string_literal: true

class ConsentsController < ApplicationController
  def create
    cookie_consent.record(functional: params[:functional] == "1", analytics: params[:analytics] == "1")
    redirect_back fallback_location: root_path
  end
end
```

Add to `config/routes.rb`:

```ruby
  post   "session" => "sessions#create"
  delete "session" => "sessions#destroy"
  post   "consent" => "consents#create"
```

Add to `config/locales/ru.yml` (create the file if absent):

```yaml
ru:
  auth:
    invalid: "Неверный логин или пароль"
    invalid_credentials: "Неверный логин или пароль"
    upstream_unavailable: "Сервис временно недоступен. Попробуйте позже"
```

- [x] **Step 6b: Handle `must_change_password`**

The upstream JWT carries a `must_change_password` claim. An account flagged with
it can authenticate but must not be treated as fully signed in — `api_geteway`
will reject its calls until the password is rotated. This service does not
implement the password-change form (that lives in the React app), so it must
refuse the session rather than hand the visitor a broken signed-in state.

In `Operations::SignIn`, after decoding the account and before writing the
session:

```ruby
      if account.must_change_password
        return Failure(:must_change_password)
      end
```

Add to `config/locales/ru.yml` under `auth:`:

```yaml
    must_change_password: "Требуется смена пароля. Обратитесь к администратору"
```

Without this the visitor appears logged in, sees the room floor, and then every
room request fails upstream with no explanation — a state that looks like a bug
in this service rather than an account condition.

- [x] **Step 7: Session cookie configuration**

Create `config/initializers/session_store.rb`:

```ruby
# frozen_string_literal: true

Rails.application.config.session_store :cookie_store,
  key: "_schedule_web_session",
  httponly: true,
  same_site: :lax,
  secure: Rails.env.production?
```

- [x] **Step 8: Verify**

With `api_geteway` running and a known account, using a cookie jar:

```bash
cd apps/schedule_web && bundle exec falcon host config/falcon.rb &
JAR=$(mktemp)
TOKEN=$(curl -s --noproxy '*' -c $JAR http://localhost:3000/ | grep -o 'name="authenticity_token" value="[^"]*"' | head -1 | sed 's/.*value="//;s/"//')
curl -s -o /dev/null -w "%{http_code} %{redirect_url}\n" --noproxy '*' -b $JAR -c $JAR \
  -d "authenticity_token=$TOKEN" -d "email=<known-email>" -d "password=<known-password>" \
  http://localhost:3000/session
```

Expected: `302 http://localhost:3000/`. Confirm the session cookie is set and httpOnly:

```bash
grep _schedule_web_session $JAR
```

Expected: a line containing `TRUE` in the httpOnly column (leading `#HttpOnly_` prefix in the jar).

Wrong credentials must not set an auth payload — repeat with a bad password and confirm the redirect carries `auth=1`.

- [x] **Step 9: Report changed files** (do not commit)

---

## Task 6: Middleware, security headers, rate limiting, logging

**Files:**
- Create: `apps/schedule_web/app/lib/resilience/rack_attack_store.rb`
- Create: `apps/schedule_web/config/initializers/{rack_attack.rb,security_headers.rb,logging.rb}`
- Modify: `apps/schedule_web/config/application.rb`

**Interfaces:**
- Consumes: `ScheduleWeb::Redis`, `ScheduleWeb::Settings`.
- Produces: `Resilience::RackAttackStore` satisfying Rack::Attack's store contract (`read`, `write`, `increment`, `delete`); a nonce-based CSP; one JSON log line per request.

- [x] **Step 1: Non-blocking Rack::Attack store**

`app/lib/resilience/rack_attack_store.rb`. Every store Rack::Attack ships with is blocking; using one would park the reactor on every request, including asset fetches.

```ruby
# frozen_string_literal: true

module Resilience
  # Rack::Attack requires a store responding to read/write/increment/delete.
  # Its bundled stores all use blocking clients, which under a fiber reactor
  # would serialize every request in the process — so this adapter wraps
  # async-redis instead.
  class RackAttackStore
    def read(key)
      ScheduleWeb::Redis.call { |client| client.call("GET", key) }
    end

    def write(key, value, expires_in: nil)
      ScheduleWeb::Redis.call do |client|
        expires_in ? client.call("SET", key, value.to_s, "EX", expires_in.to_i) : client.call("SET", key, value.to_s)
      end
    end

    def increment(key, amount = 1, expires_in: nil)
      ScheduleWeb::Redis.call do |client|
        count = client.call("INCRBY", key, amount).to_i
        client.call("EXPIRE", key, expires_in.to_i) if expires_in && count == amount
        count
      end
    end

    def delete(key)
      ScheduleWeb::Redis.call { |client| client.call("DEL", key) }
    end
  end
end
```

- [x] **Step 2: Rate limiting**

`config/initializers/rack_attack.rb`:

```ruby
# frozen_string_literal: true

Rack::Attack.cache.store = Resilience::RackAttackStore.new

# General per-IP ceiling. Static assets are exempt so a normal page load, which
# fetches several files at once, cannot exhaust a visitor's own budget.
Rack::Attack.throttle("requests/ip", limit: ScheduleWeb::Settings.config.rate_limit_general, period: 60) do |request|
  request.ip unless request.path.start_with?("/assets")
end

# Login is throttled far harder than general traffic: it is the one endpoint
# where an attacker gets something valuable per attempt.
Rack::Attack.throttle("login/ip", limit: ScheduleWeb::Settings.config.rate_limit_login, period: 60) do |request|
  request.ip if request.path == "/session" && request.post?
end

Rack::Attack.throttled_responder = lambda do |request|
  retry_after = (request.env["rack.attack.match_data"] || {})[:period].to_i
  [429, { "content-type" => "text/plain", "retry-after" => retry_after.to_s }, ["Too Many Requests\n"]]
end

ActiveSupport::Notifications.subscribe("throttle.rack_attack") do |_name, _start, _finish, _id, payload|
  Rails.logger.warn({ event: "rate_limited", ip: payload[:request].ip, path: payload[:request].path }.to_json)
end
```

- [x] **Step 3: Security headers**

`config/initializers/security_headers.rb`. The `schedule_home` design loads Manrope and Unbounded from Google Fonts, so those two hosts are allowed explicitly; everything else is same-origin only.

```ruby
# frozen_string_literal: true

Rails.application.configure do
  config.content_security_policy do |policy|
    policy.default_src :self
    policy.base_uri    :self
    policy.form_action :self
    policy.frame_ancestors :none
    policy.object_src  :none
    policy.img_src     :self, :data
    policy.script_src  :self
    policy.style_src   :self, "https://fonts.googleapis.com"
    policy.font_src    :self, "https://fonts.gstatic.com"
    policy.connect_src :self
  end

  # A nonce beats 'unsafe-inline': inline <script> and <style> emitted by Rails
  # helpers still work, but an injected one without the per-response nonce does not.
  config.content_security_policy_nonce_generator = ->(request) { request.session.id.to_s.presence || SecureRandom.base64(16) }
  config.content_security_policy_nonce_directives = %w[script-src style-src]

  config.action_dispatch.default_headers.merge!(
    "X-Frame-Options" => "DENY",
    "X-Content-Type-Options" => "nosniff",
    "Referrer-Policy" => "strict-origin-when-cross-origin",
    "Permissions-Policy" => "camera=(), microphone=(), geolocation=(), interest-cohort=()"
  )
end
```

In `config/environments/production.rb` ensure:

```ruby
  config.force_ssl = ENV.fetch("FORCE_SSL", "false") == "true"
  config.assume_ssl = true
```

`force_ssl` is opt-in because the service runs behind plain HTTP inside the compose network; enabling it unconditionally would redirect-loop the healthcheck.

- [x] **Step 4: Structured logging and proxy trust**

`config/initializers/logging.rb`:

```ruby
# frozen_string_literal: true

# One JSON line per request. Includes the request id that is forwarded to
# api_geteway as X-Request-Id, so a single trace spans both services.
ActiveSupport::Notifications.subscribe("process_action.action_controller") do |*args|
  event = ActiveSupport::Notifications::Event.new(*args)
  payload = event.payload

  Rails.logger.info({
    event: "request",
    method: payload[:method],
    path: payload[:path],
    status: payload[:status],
    duration_ms: event.duration.round(2),
    request_id: payload[:headers]&.env&.dig("action_dispatch.request_id"),
    controller: payload[:controller],
    action: payload[:action]
  }.compact.to_json)
end
```

In `config/application.rb`, inside the `Application` class:

```ruby
    config.middleware.use Rack::Deflater
    config.middleware.use Rack::Attack

    # Without a declared proxy chain, Rack::Attack throttles the proxy's IP and
    # one noisy visitor takes down everyone behind the same hop.
    config.action_dispatch.trusted_proxies =
      ScheduleWeb::Settings.config.trusted_proxies.map { |proxy| IPAddr.new(proxy) }
      .presence || ActionDispatch::RemoteIp::TRUSTED_PROXIES
```

Note this reads `ScheduleWeb::Settings`, which is defined in an initializer that runs *after* `application.rb`. Move the `trusted_proxies` assignment into `config/initializers/logging.rb` (or a dedicated `proxies.rb`) rather than `application.rb` if boot raises `NameError`.

- [x] **Step 5: Verify**

```bash
curl -sI --noproxy '*' http://localhost:3000/ | grep -iE "content-security-policy|x-frame|x-content-type|referrer-policy|permissions-policy"
```

Expected: all five headers present; the CSP contains `script-src 'self' 'nonce-` and no `unsafe-inline`.

Rate limiting:

```bash
for i in $(seq 1 15); do
  curl -s -o /dev/null -w "%{http_code} " --noproxy '*' -X POST http://localhost:3000/session
done; echo
```

Expected: the first responses are `422`/`302` (CSRF rejection is fine — the throttle counts them), then `429` once past `RATE_LIMIT_LOGIN`.

Logging:

```bash
curl -s -o /dev/null --noproxy '*' http://localhost:3000/ && docker compose logs --tail=5 schedule_web | grep '"event":"request"'
```

Expected: a JSON line with `path`, `status`, `duration_ms`.

- [x] **Step 6: Report changed files** (do not commit)

---

## Task 7: Page shell — layout, header, footer, CSS foundation

**Files:**
- Create: `apps/schedule_web/app/views/layouts/application.html.erb`
- Create: `apps/schedule_web/app/views/shared/{_site_header.html.erb,_site_footer.html.erb,_stale_banner.html.erb,_region_error.html.erb}`
- Create: `apps/schedule_web/app/assets/stylesheets/application.css`
- Create: `apps/schedule_web/app/assets/stylesheets/components/{_site_header.css,_site_footer.css,_stale_banner.css,_region_error.css}`
- Create: `apps/schedule_web/app/lib/presenters/base.rb`
- Modify: `apps/schedule_web/config/locales/ru.yml`

**Interfaces:**
- Consumes: `current_account`, `signed_in?` helpers from Task 5.
- Produces: a layout rendering `_site_header`, `yield`, `_site_footer` on every page; `Presenters::Base` with `#stale?`, `#cached_at_label`, `#failed?`; CSS custom properties `--ink`, `--paper`, `--line`, `--muted` on `:root`.

**Design source:** `schedule_home/index.html` (markup) and `schedule_home/styles.css` (visual values). Port values, not structure — the CSS below is authoritative where they disagree.

- [x] **Step 1: Design tokens and reset**

`app/assets/stylesheets/application.css`. This file holds tokens, reset and layout primitives **only** — no component styling.

```css
/*
 * Tokens and reset. Component styling lives in components/, one file per
 * component, each scoped to its own root class. Nothing here may position a
 * component; nothing in components/ may restyle another component.
 */
@import "components/site_header.css";
@import "components/site_footer.css";
@import "components/stale_banner.css";
@import "components/region_error.css";

:root {
  --ink: #151311;
  --paper: #ffffff;
  --line: #e4e0da;
  --muted: #8a8279;
  --font-ui: 'Manrope', system-ui, sans-serif;
  --font-display: 'Unbounded', var(--font-ui);
  color-scheme: light;
}

*, *::before, *::after { box-sizing: border-box; }

html, body { margin: 0; padding: 0; }

body {
  min-height: 100svh;
  display: flex;
  flex-direction: column;
  color: var(--ink);
  background: var(--paper);
  font-family: var(--font-ui);
  -webkit-font-smoothing: antialiased;
}

/* The single most important rule in this file. `main` grows to absorb slack,
   so the footer sits at the bottom of a short page and below the content of a
   long one — with no scroll listener, no fixed positioning, and no per-page
   override. The React version reproduced this per page with position:fixed
   plus a scroll-gated class and got it wrong on every page that did not
   happen to scroll. */
main { flex: 1 0 auto; }

body.modal-open { overflow: hidden; }

.sr-only {
  position: absolute;
  width: 1px; height: 1px;
  margin: -1px; padding: 0;
  overflow: hidden;
  clip: rect(0 0 0 0);
  white-space: nowrap;
  border: 0;
}
```

- [x] **Step 2: Footer component**

`app/views/shared/_site_footer.html.erb`:

```erb
<footer class="site-footer" aria-label="<%= t("footer.aria_label") %>">
  <div class="site-footer__bar">
    <button class="site-footer__item" type="button"
            data-action="dialog#open" data-dialog-name-param="contacts"><%= t("footer.contacts") %></button>
    <button class="site-footer__item" type="button"
            data-action="dialog#open" data-dialog-name-param="cookies"><%= t("footer.cookies") %></button>
    <a class="site-footer__item" href="https://example.com" target="_blank" rel="noreferrer"><%= t("footer.link") %></a>
  </div>
</footer>
```

`app/assets/stylesheets/components/_site_footer.css`:

```css
/*
 * Two elements, two jobs. `.site-footer` owns placement and never declares
 * `transform`; `.site-footer__bar` owns appearance and is the only element an
 * animation may transform. Collapsing these into one element is what broke the
 * React footer: a page rule overrode `transform` for a slide-in and silently
 * discarded the `translateX(-50%)` that was doing the centring, while the
 * `left: 50%` beside it survived and pushed the footer into the right half of
 * the viewport.
 */
.site-footer {
  flex: 0 0 auto;
  display: flex;
  justify-content: center;
  width: 100%;
  padding: 0 16px 28px;
}

.site-footer__bar {
  display: inline-flex;
  max-width: 100%;
  border: 1px solid var(--ink);
  background: var(--paper);
}

.site-footer__item {
  min-width: 112px;
  height: 42px;
  display: grid;
  place-items: center;
  margin: 0;
  padding: 0 18px;
  border: 0;
  border-right: 1px solid var(--ink);
  color: var(--ink);
  background: transparent;
  text-decoration: none;
  text-transform: uppercase;
  font-family: var(--font-ui);
  font-size: 10px;
  font-weight: 800;
  line-height: 1;
  letter-spacing: .085em;
  cursor: pointer;
  transition: color 170ms ease, background 170ms ease;
}

.site-footer__bar > :last-child { border-right: 0; }

.site-footer__item:hover,
.site-footer__item:focus-visible {
  color: var(--paper);
  background: var(--ink);
  outline: none;
}

@media (max-width: 620px) {
  .site-footer { padding-bottom: 12px; }
  .site-footer__item { min-width: 0; flex: 1; padding-inline: 8px; font-size: 9px; }
}
```

- [x] **Step 3: Header component**

`app/views/shared/_site_header.html.erb`. `back_path` is a local; when absent no back control renders.

```erb
<header class="site-header">
  <% if local_assigns[:back_path].present? %>
    <a class="site-header__back" href="<%= back_path %>" aria-label="<%= t("nav.back") %>">‹</a>
  <% end %>

  <button class="site-header__profile" type="button"
          aria-label="<%= t("auth.open_dialog") %>"
          data-action="dialog#open" data-dialog-name-param="auth">
    <svg class="site-header__profile-icon" aria-hidden="true" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
      <path fill-rule="evenodd" clip-rule="evenodd" d="M8.25 9C8.25 6.92893 9.92893 5.25 12 5.25C14.0711 5.25 15.75 6.92893 15.75 9C15.75 11.0711 14.0711 12.75 12 12.75C9.92893 12.75 8.25 11.0711 8.25 9ZM12 6.75C10.7574 6.75 9.75 7.75736 9.75 9C9.75 10.2426 10.7574 11.25 12 11.25C13.2426 11.25 14.25 10.2426 14.25 9C14.25 7.75736 13.2426 6.75 12 6.75Z"/>
      <path fill-rule="evenodd" clip-rule="evenodd" d="M1.25 12C1.25 6.06294 6.06294 1.25 12 1.25C17.9371 1.25 22.75 6.06294 22.75 12C22.75 17.9371 17.9371 22.75 12 22.75C6.06294 22.75 1.25 17.9371 1.25 12ZM12 2.75C6.89137 2.75 2.75 6.89137 2.75 12C2.75 14.5456 3.77827 16.851 5.4421 18.5235C5.6225 17.5504 5.97694 16.6329 6.68837 15.8951C7.75252 14.7915 9.45416 14.25 12 14.25C14.5457 14.25 16.2474 14.7915 17.3115 15.8951C18.023 16.6329 18.3774 17.5505 18.5578 18.5236C20.2217 16.8511 21.25 14.5456 21.25 12C21.25 6.89137 17.1086 2.75 12 2.75ZM17.1937 19.6554C17.0918 18.4435 16.8286 17.5553 16.2318 16.9363C15.5823 16.2628 14.3789 15.75 12 15.75C9.62099 15.75 8.41761 16.2628 7.76815 16.9363C7.17127 17.5553 6.90811 18.4434 6.80622 19.6553C8.28684 20.6618 10.0747 21.25 12 21.25C13.9252 21.25 15.7131 20.6618 17.1937 19.6554Z"/>
    </svg>
  </button>
</header>
```

`app/assets/stylesheets/components/_site_header.css`:

```css
.site-header {
  flex: 0 0 auto;
  position: relative;
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
  min-height: 64px;
  padding: 16px 20px;
}

.site-header__back,
.site-header__profile {
  display: grid;
  place-items: center;
  width: 44px;
  height: 44px;
  padding: 0;
  border: 1px solid var(--line);
  color: var(--ink);
  background: var(--paper);
  text-decoration: none;
  font-size: 22px;
  line-height: 1;
  cursor: pointer;
  transition: border-color 170ms ease, background 170ms ease;
}

.site-header__back { order: 0; }
.site-header__profile { order: 1; margin-left: auto; }

.site-header__back:hover,
.site-header__profile:hover,
.site-header__back:focus-visible,
.site-header__profile:focus-visible {
  border-color: var(--ink);
  outline: none;
}

.site-header__profile-icon { width: 24px; height: 24px; fill: currentColor; }
```

- [x] **Step 4: Degradation partials**

`app/views/shared/_stale_banner.html.erb`:

```erb
<div class="stale-banner" role="status">
  <span class="stale-banner__mark" aria-hidden="true">⚠</span>
  <span><%= t("degraded.stale", time: l(cached_at, format: :short)) %></span>
</div>
```

`app/views/shared/_region_error.html.erb`:

```erb
<div class="region-error" role="alert">
  <p class="region-error__message"><%= t("degraded.failed") %></p>
  <% if local_assigns[:retry_path].present? %>
    <a class="region-error__retry" href="<%= retry_path %>" data-turbo-frame="_self"><%= t("degraded.retry") %></a>
  <% end %>
</div>
```

`app/assets/stylesheets/components/_stale_banner.css`:

```css
.stale-banner {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  padding: 10px 20px;
  border-bottom: 1px solid var(--line);
  color: var(--muted);
  font-size: 12px;
  font-weight: 600;
}

.stale-banner__mark { font-size: 14px; }
```

`app/assets/stylesheets/components/_region_error.css`:

```css
/*
 * Sized to occupy the region it replaces, so a failed fetch does not collapse
 * the layout around it and shift everything else on the page.
 */
.region-error {
  display: grid;
  place-items: center;
  gap: 12px;
  min-height: 200px;
  padding: 24px;
  border: 1px dashed var(--line);
  text-align: center;
}

.region-error__message { margin: 0; color: var(--muted); font-size: 13px; font-weight: 600; }

.region-error__retry {
  padding: 10px 18px;
  border: 1px solid var(--ink);
  color: var(--ink);
  background: var(--paper);
  text-decoration: none;
  text-transform: uppercase;
  font-size: 10px;
  font-weight: 800;
  letter-spacing: .085em;
}
```

- [x] **Step 5: Layout**

`app/views/layouts/application.html.erb`:

```erb
<!DOCTYPE html>
<html lang="ru">
  <head>
    <title><%= content_for(:title) || t("app.title") %></title>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width,initial-scale=1">
    <meta name="color-scheme" content="light">
    <meta name="theme-color" content="#FFFFFF">
    <%= csrf_meta_tags %>
    <%= csp_meta_tag %>
    <link rel="preconnect" href="https://fonts.googleapis.com">
    <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
    <link href="https://fonts.googleapis.com/css2?family=Manrope:wght@400;500;600;700;800&family=Unbounded:wght@500;600;700;800&display=swap" rel="stylesheet">
    <%= stylesheet_link_tag "application" %>
    <%= javascript_importmap_tags %>
  </head>

  <body data-controller="dialog">
    <%= render "shared/site_header", back_path: content_for(:back_path) %>
    <%= yield %>
    <%= render "shared/site_footer" %>
    <%= yield :dialogs %>
  </body>
</html>
```

- [x] **Step 6: Presenter base**

`app/lib/presenters/base.rb`:

```ruby
# frozen_string_literal: true

module Presenters
  # Wraps a Resilience Result so views ask `stale?` / `failed?` instead of
  # pattern-matching monads in ERB.
  class Base
    def initialize(result)
      @result = result
    end

    def failed? = @result.failure?

    def failure_reason = @result.failure

    def stale? = @result.success? && @result.value!.stale?

    def cached_at = @result.success? ? @result.value!.cached_at : nil

    def data = @result.success? ? @result.value!.data : nil
  end
end
```

- [x] **Step 7: Locale strings**

Merge into `config/locales/ru.yml`:

```yaml
ru:
  app:
    title: "Выбор расписания"
  nav:
    back: "Назад"
  footer:
    aria_label: "Служебные ссылки"
    contacts: "Контакты"
    cookies: "Cookies"
    link: "Ссылка"
  degraded:
    stale: "Данные от %{time}, возможно устарели"
    failed: "Не удалось загрузить данные"
    retry: "Повторить"
  time:
    formats:
      short: "%H:%M"
```

- [x] **Step 8: Verify**

```bash
curl -s --noproxy '*' http://localhost:3000/ | grep -c "site-footer"
```

Expected: at least `1`.

Visual check — the footer must sit at the bottom of the viewport on a short page and below content on a long one, centred in both cases, at 1280px and 375px widths. Confirm no `position: fixed` and no `transform` on `.site-footer`:

```bash
grep -nE "position:\s*fixed|transform" apps/schedule_web/app/assets/stylesheets/components/_site_footer.css
```

Expected: no output.

- [x] **Step 9: Report changed files** (do not commit)

---

## Task 8: Dialogs

**Files:**
- Create: `apps/schedule_web/app/views/shared/{_auth_dialog.html.erb,_contacts_dialog.html.erb,_cookie_dialog.html.erb,_guide_dialog.html.erb}`
- Create: `apps/schedule_web/app/assets/stylesheets/components/_dialog.css`
- Create: `apps/schedule_web/app/javascript/controllers/dialog_controller.js`
- Modify: `apps/schedule_web/app/views/layouts/application.html.erb`, `config/locales/ru.yml`, `app/assets/stylesheets/application.css`

**Interfaces:**
- Consumes: `signed_in?`, `current_account`, `cookie_consent` helpers; routes `POST /session`, `DELETE /session`, `POST /consent`.
- Produces: a `dialog` Stimulus controller with actions `open` (param `name`), `close`, and `closeOnBackdrop`; dialogs identified by `data-dialog-target="panel"` plus `data-dialog-name`.

- [x] **Step 1: Dialog controller**

`app/javascript/controllers/dialog_controller.js`:

```javascript
import { Controller } from "@hotwired/stimulus"

// One controller on <body> owns every dialog, so only one can be open at a
// time and the scroll lock has a single owner. Per-dialog controllers would
// each need to know about the others to enforce that.
export default class extends Controller {
  static targets = ["panel"]

  connect() {
    this.escHandler = (event) => { if (event.key === "Escape") this.close() }
    document.addEventListener("keydown", this.escHandler)

    // ?auth=1 is how require_account asks for the login dialog after bouncing
    // an unauthenticated visitor off a protected route.
    if (new URLSearchParams(window.location.search).get("auth") === "1") {
      this.show("auth")
    }
  }

  disconnect() { document.removeEventListener("keydown", this.escHandler) }

  open(event) {
    event.preventDefault()
    this.show(event.params.name)
  }

  close() {
    this.panelTargets.forEach((panel) => {
      panel.classList.remove("is-open")
      panel.setAttribute("aria-hidden", "true")
    })
    document.body.classList.remove("modal-open")
  }

  closeOnBackdrop(event) {
    if (event.target === event.currentTarget) this.close()
  }

  show(name) {
    const panel = this.panelTargets.find((target) => target.dataset.dialogName === name)
    if (!panel) return

    this.close()
    panel.classList.add("is-open")
    panel.setAttribute("aria-hidden", "false")
    document.body.classList.add("modal-open")
    panel.querySelector("input, button, a")?.focus()
  }
}
```

- [x] **Step 2: Dialog styling**

`app/assets/stylesheets/components/_dialog.css`. Add `@import "components/dialog.css";` to `application.css`.

```css
.dialog {
  position: fixed;
  z-index: 100;
  inset: 0;
  display: grid;
  place-items: center;
  padding: 24px;
  background: rgba(255, 255, 255, .82);
  backdrop-filter: blur(5px);
  -webkit-backdrop-filter: blur(5px);
  opacity: 0;
  visibility: hidden;
  transition: opacity 200ms ease, visibility 200ms ease;
}

.dialog.is-open { opacity: 1; visibility: visible; }

.dialog__panel {
  position: relative;
  width: min(100%, 380px);
  padding: 28px 24px 24px;
  border: 1px solid var(--ink);
  background: var(--paper);
}

.dialog__close {
  position: absolute;
  top: 8px; right: 8px;
  width: 32px; height: 32px;
  border: 0;
  background: transparent;
  color: var(--ink);
  font-size: 22px;
  line-height: 1;
  cursor: pointer;
}

.dialog__field { display: block; margin-bottom: 12px; }

.dialog__input {
  width: 100%;
  height: 46px;
  padding: 0 14px;
  border: 1px solid var(--line);
  background: var(--paper);
  color: var(--ink);
  font-family: var(--font-ui);
  font-size: 14px;
}

.dialog__input:focus-visible { border-color: var(--ink); outline: none; }

.dialog__submit {
  width: 100%;
  height: 46px;
  border: 1px solid var(--ink);
  background: var(--ink);
  color: var(--paper);
  text-transform: uppercase;
  font-family: var(--font-ui);
  font-size: 11px;
  font-weight: 800;
  letter-spacing: .085em;
  cursor: pointer;
}

.dialog__error { margin: 0 0 12px; color: #b4392f; font-size: 12px; font-weight: 600; }

.dialog__row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 12px 0;
  border-bottom: 1px solid var(--line);
}

.dialog__actions { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 16px; }

.dialog__action {
  flex: 1;
  min-height: 40px;
  padding: 0 12px;
  border: 1px solid var(--ink);
  background: var(--paper);
  color: var(--ink);
  text-transform: uppercase;
  font-size: 10px;
  font-weight: 800;
  letter-spacing: .06em;
  cursor: pointer;
}

.dialog__action--primary { background: var(--ink); color: var(--paper); }
```

- [x] **Step 3: Auth dialog**

`app/views/shared/_auth_dialog.html.erb`. Signed in it becomes a sign-out panel — the same control should not open a login form for someone already logged in.

```erb
<div class="dialog" data-dialog-target="panel" data-dialog-name="auth"
     aria-hidden="true" data-action="click->dialog#closeOnBackdrop">
  <section class="dialog__panel" role="dialog" aria-modal="true" aria-label="<%= t("auth.title") %>">
    <button class="dialog__close" type="button" aria-label="<%= t("auth.close") %>" data-action="dialog#close">×</button>

    <% if signed_in? %>
      <p class="dialog__error" style="color: var(--muted)"><%= current_account.email %></p>
      <% if current_account.staff? %>
        <a class="dialog__action" href="<%= ENV.fetch("ADMIN_PANEL_URL", "http://localhost:4173") %>"
           target="_blank" rel="noreferrer"><%= t("auth.admin_panel") %> ↗</a>
      <% end %>
      <%= form_with url: session_path, method: :delete, class: "dialog__form" do %>
        <button class="dialog__submit" type="submit"><%= t("auth.sign_out") %></button>
      <% end %>
    <% else %>
      <% if flash[:alert].present? %>
        <p class="dialog__error" role="alert"><%= flash[:alert] %></p>
      <% end %>
      <%= form_with url: session_path, method: :post, class: "dialog__form" do %>
        <label class="dialog__field">
          <span class="sr-only"><%= t("auth.login") %></span>
          <input class="dialog__input" type="text" name="email" placeholder="<%= t("auth.login") %>" autocomplete="username" required>
        </label>
        <label class="dialog__field">
          <span class="sr-only"><%= t("auth.password") %></span>
          <input class="dialog__input" type="password" name="password" placeholder="<%= t("auth.password") %>" autocomplete="current-password" required>
        </label>
        <button class="dialog__submit" type="submit"><%= t("auth.submit") %></button>
      <% end %>
    <% end %>
  </section>
</div>
```

- [x] **Step 4: Cookie dialog**

`app/views/shared/_cookie_dialog.html.erb`:

```erb
<div class="dialog" data-dialog-target="panel" data-dialog-name="cookies"
     aria-hidden="true" data-action="click->dialog#closeOnBackdrop">
  <section class="dialog__panel" role="dialog" aria-modal="true" aria-label="<%= t("cookies.title") %>">
    <button class="dialog__close" type="button" aria-label="<%= t("cookies.close") %>" data-action="dialog#close">×</button>

    <%= form_with url: consent_path, method: :post do %>
      <div class="dialog__row">
        <strong><%= t("cookies.necessary") %></strong>
        <span aria-label="<%= t("cookies.always_on") %>">ON</span>
      </div>
      <label class="dialog__row">
        <strong><%= t("cookies.functional") %></strong>
        <input type="checkbox" name="functional" value="1" <%= "checked" if cookie_consent.allows?(:functional) %>>
      </label>
      <label class="dialog__row">
        <strong><%= t("cookies.analytics") %></strong>
        <input type="checkbox" name="analytics" value="1" <%= "checked" if cookie_consent.allows?(:analytics) %>>
      </label>

      <div class="dialog__actions">
        <button class="dialog__action" type="submit" name="functional" value="0"><%= t("cookies.necessary_only") %></button>
        <button class="dialog__action dialog__action--primary" type="submit"><%= t("cookies.save") %></button>
      </div>
    <% end %>
  </section>
</div>
```

- [x] **Step 5: Contacts and guide dialogs**

`app/views/shared/_contacts_dialog.html.erb`:

```erb
<div class="dialog" data-dialog-target="panel" data-dialog-name="contacts"
     aria-hidden="true" data-action="click->dialog#closeOnBackdrop">
  <section class="dialog__panel" role="dialog" aria-modal="true" aria-label="<%= t("contacts.title") %>">
    <button class="dialog__close" type="button" aria-label="<%= t("contacts.close") %>" data-action="dialog#close">×</button>
    <div class="dialog__row"><a href="https://t.me/telegram" target="_blank" rel="noreferrer">@telegram</a><span aria-hidden="true">↗</span></div>
    <div class="dialog__row"><a href="tel:+000000000000">+000 000 00 00</a><span aria-hidden="true">↗</span></div>
    <div class="dialog__row"><a href="mailto:hello@example.com">hello@example.com</a><span aria-hidden="true">↗</span></div>
  </section>
</div>
```

`app/views/shared/_guide_dialog.html.erb`. `floors` is a local supplied by the home page so the guide lists exactly the destinations the visitor may reach.

```erb
<div class="dialog" data-dialog-target="panel" data-dialog-name="guide"
     aria-hidden="true" data-action="click->dialog#closeOnBackdrop">
  <section class="dialog__panel" role="dialog" aria-modal="true" aria-label="<%= t("guide.title") %>">
    <button class="dialog__close" type="button" aria-label="<%= t("guide.close") %>" data-action="dialog#close">×</button>
    <% floors.each do |floor| %>
      <a class="dialog__row" href="<%= floor[:path] %>">
        <span><%= floor[:index] %>F</span>
        <span><%= t("floors.#{floor[:target]}") %></span>
        <span aria-hidden="true">↗</span>
      </a>
    <% end %>
  </section>
</div>
```

- [x] **Step 6: Render dialogs from the layout**

In `app/views/layouts/application.html.erb`, replace `<%= yield :dialogs %>` with:

```erb
    <%= render "shared/auth_dialog" %>
    <%= render "shared/contacts_dialog" %>
    <%= render "shared/cookie_dialog" %>
    <%= yield :dialogs %>
```

The guide dialog stays page-supplied via `content_for :dialogs` because it needs the floor list.

- [x] **Step 7: Locale strings**

Merge into `config/locales/ru.yml`:

```yaml
ru:
  auth:
    title: "Авторизация"
    open_dialog: "Открыть форму авторизации"
    close: "Закрыть форму авторизации"
    login: "Логин"
    password: "Пароль"
    submit: "Войти"
    sign_out: "Выйти"
    admin_panel: "Панель администратора"
  cookies:
    title: "Cookies"
    close: "Закрыть настройки cookies"
    necessary: "Необходимые"
    functional: "Функциональные"
    analytics: "Аналитика"
    always_on: "Всегда включено"
    necessary_only: "Только необходимые"
    save: "Сохранить"
  contacts:
    title: "Контакты"
    close: "Закрыть контакты"
  guide:
    title: "Подсказка"
    close: "Закрыть подсказку"
  floors:
    group: "Группа"
    teacher: "Преподаватель"
    room: "Аудитория"
```

- [x] **Step 8: Verify**

Load `http://localhost:3000/` in a browser. Confirm: the profile button opens the auth dialog; `Escape` and a backdrop click close it; `body` gains `modal-open` while open; the footer's Contacts and Cookies buttons open their dialogs. Submitting the cookie form sets the cookie:

```bash
curl -s -D - -o /dev/null --noproxy '*' -b $JAR -c $JAR -X POST \
  -d "authenticity_token=$TOKEN" -d "functional=1" http://localhost:3000/consent | grep -i set-cookie
```

Expected: a `cookie_consent=v1%3Anecessary%2Cfunctional` cookie with `HttpOnly`.

- [x] **Step 9: Report changed files** (do not commit)

---

## Task 9: Home page and auth gating

**Files:**
- Create: `apps/schedule_web/app/lib/policies/home_floors.rb`
- Create: `apps/schedule_web/app/lib/presenters/home_page.rb`
- Create: `apps/schedule_web/app/controllers/rooms_controller.rb`
- Create: `apps/schedule_web/app/views/home/index.html.erb`
- Create: `apps/schedule_web/app/assets/stylesheets/components/{_scene.css,_transition_layer.css}`
- Create: `apps/schedule_web/app/javascript/controllers/floor_controller.js`
- Modify: `apps/schedule_web/app/controllers/home_controller.rb`, `config/routes.rb`, `config/locales/ru.yml`, `app/assets/stylesheets/application.css`

**Interfaces:**
- Consumes: `require_account`, `current_account`, `signed_in?`; the dialog controller from Task 8.
- Produces:
  - `Policies::HomeFloors.call(account:)` → `Array<Hash>` of `{target: Symbol, index: Integer, path: String}`, ordered top floor first.
  - `Presenters::HomePage.new(account:)` with `#floors`.
  - Routes `GET /room`, `GET /group`, `GET /teacher` (the latter two are Spec 2 placeholders returning 404 until then; `/room` is guarded now).

- [x] **Step 1: Floor policy**

`app/lib/policies/home_floors.rb`:

```ruby
# frozen_string_literal: true

module Policies
  # Decides which lookups a visitor may reach. Room data is staff-only, matching
  # api_geteway, which moved rooms.schedule and rooms.occupancy out of the
  # public portal slice into admin — only the rooms index stayed public.
  #
  # The view renders exactly what this returns, so an ungranted floor is absent
  # from the DOM rather than hidden with CSS.
  module HomeFloors
    PUBLIC = [
      { target: :group,   index: 3, path: "/group" },
      { target: :teacher, index: 2, path: "/teacher" }
    ].freeze

    PROTECTED = [
      { target: :room, index: 1, path: "/room" }
    ].freeze

    module_function

    def call(account:)
      account ? PUBLIC + PROTECTED : PUBLIC
    end
  end
end
```

- [x] **Step 2: Presenter and controllers**

`app/lib/presenters/home_page.rb`:

```ruby
# frozen_string_literal: true

module Presenters
  class HomePage
    def initialize(account:)
      @account = account
    end

    def floors = Policies::HomeFloors.call(account: @account)
  end
end
```

`app/controllers/home_controller.rb`:

```ruby
# frozen_string_literal: true

class HomeController < ApplicationController
  def index
    @page = Presenters::HomePage.new(account: current_account)
  end
end
```

`app/controllers/rooms_controller.rb`. The gate lives here so it applies to every room action Spec 2 adds.

```ruby
# frozen_string_literal: true

class RoomsController < ApplicationController
  before_action :require_account

  # Spec 2 replaces this with the room search page.
  def index
    render plain: "", status: :no_content
  end
end
```

Add to `config/routes.rb`:

```ruby
  get "room" => "rooms#index"
```

- [x] **Step 3: Home view**

`app/views/home/index.html.erb`:

```erb
<% content_for :dialogs do %>
  <%= render "shared/guide_dialog", floors: @page.floors %>
<% end %>

<main class="scene" aria-label="<%= t("home.aria_label") %>" data-controller="floor">
  <section class="scene__building" aria-label="<%= t("home.options_label") %>">
    <div class="scene__shaft" aria-hidden="true">
      <div class="scene__shaft-line scene__shaft-line--left"></div>
      <div class="scene__shaft-line scene__shaft-line--right"></div>
    </div>

    <button class="scene__guide" type="button" aria-label="<%= t("guide.open") %>"
            data-action="dialog#open" data-dialog-name-param="guide">
      <span class="scene__guide-mark" aria-hidden="true">?</span>
    </button>

    <% @page.floors.each do |floor| %>
      <a class="scene__floor scene__floor--<%= floor[:index] %>"
         href="<%= floor[:path] %>"
         data-action="floor#enter"
         data-floor-label-param="<%= t("floors.#{floor[:target]}") %>"
         aria-label="<%= t("floors.#{floor[:target]}") %>">
        <span class="scene__floor-index">
          <span class="scene__floor-number"><%= floor[:index] %></span>
          <span class="scene__floor-f">F</span>
        </span>
        <span class="scene__floor-label"><%= t("floors.#{floor[:target]}") %></span>
        <span class="scene__floor-arrow" aria-hidden="true">↗</span>
      </a>
    <% end %>
  </section>
</main>

<div class="transition-layer" data-floor-target="layer" aria-hidden="true"><span data-floor-target="label"></span></div>
```

The floors are `<a>` elements, not `<button>`: navigation that works without JavaScript, is middle-clickable, and shows its destination on hover. The Stimulus controller only adds the transition flourish.

- [x] **Step 4: Floor transition controller**

`app/javascript/controllers/floor_controller.js`:

```javascript
import { Controller } from "@hotwired/stimulus"

// Plays the wipe transition, then follows the link. Progressive enhancement:
// without JS the anchor navigates immediately and nothing is lost.
export default class extends Controller {
  static targets = ["layer", "label"]

  enter(event) {
    if (event.metaKey || event.ctrlKey || event.shiftKey || event.button !== 0) return

    event.preventDefault()
    const href = event.currentTarget.getAttribute("href")

    this.labelTarget.textContent = event.params.label
    this.layerTarget.classList.add("is-open")
    this.layerTarget.setAttribute("aria-hidden", "false")

    window.setTimeout(() => { window.location.href = href }, 420)
  }
}
```

- [x] **Step 5: Scene styling**

`app/assets/stylesheets/components/_scene.css`. Port dimensions from `schedule_home/styles.css`; the rules below are authoritative.

```css
.scene {
  display: grid;
  place-items: center;
  width: 100%;
  padding: 24px 20px;
}

.scene__building {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: 14px;
  width: min(100%, 520px);
  padding: 28px 0;
}

.scene__shaft { position: absolute; inset: 0; pointer-events: none; }
.scene__shaft-line { position: absolute; top: 0; bottom: 0; width: 1.5px; background: var(--line); }
.scene__shaft-line--left { left: 0; }
.scene__shaft-line--right { right: 0; }

.scene__floor {
  position: relative;
  display: grid;
  grid-template-columns: 64px minmax(0, 1fr) 32px;
  align-items: center;
  gap: 12px;
  min-height: 96px;
  padding: 0 18px;
  border: 1px solid var(--ink);
  background: var(--paper);
  color: var(--ink);
  text-decoration: none;
  cursor: pointer;
  transition: background 200ms ease, color 200ms ease;
}

.scene__floor:hover,
.scene__floor:focus-visible { background: var(--ink); color: var(--paper); outline: none; }

.scene__floor-index { display: flex; align-items: baseline; gap: 2px; }
.scene__floor-number { font-family: var(--font-display); font-size: 34px; font-weight: 700; line-height: 1; }
.scene__floor-f { font-size: 12px; font-weight: 800; }

.scene__floor-label {
  font-family: var(--font-display);
  font-size: 18px;
  font-weight: 600;
  letter-spacing: -.01em;
}

.scene__floor-arrow { justify-self: end; font-size: 20px; }

.scene__guide {
  position: absolute;
  top: -6px; right: -6px;
  z-index: 2;
  width: 34px; height: 34px;
  display: grid;
  place-items: center;
  border: 1px solid var(--ink);
  background: var(--paper);
  color: var(--ink);
  cursor: pointer;
}

.scene__guide-mark { font-family: var(--font-display); font-size: 15px; font-weight: 700; }

@media (max-width: 620px) {
  .scene__floor { min-height: 78px; grid-template-columns: 52px minmax(0, 1fr) 24px; padding: 0 12px; }
  .scene__floor-number { font-size: 26px; }
  .scene__floor-label { font-size: 15px; }
}
```

`app/assets/stylesheets/components/_transition_layer.css`:

```css
/*
 * `.transition-layer` owns position; the inner span owns transform. Same
 * separation as the footer, for the same reason.
 */
.transition-layer {
  position: fixed;
  z-index: 200;
  inset: 0;
  display: grid;
  place-items: center;
  background: var(--ink);
  opacity: 0;
  visibility: hidden;
  transition: opacity 380ms ease, visibility 380ms ease;
}

.transition-layer.is-open { opacity: 1; visibility: visible; }

.transition-layer span {
  color: var(--paper);
  font-family: var(--font-display);
  font-size: 28px;
  font-weight: 600;
  transform: translateY(8px);
  transition: transform 380ms cubic-bezier(.2, .8, .2, 1);
}

.transition-layer.is-open span { transform: translateY(0); }
```

Add both imports to `application.css`.

- [x] **Step 6: Locale strings**

Merge into `config/locales/ru.yml`:

```yaml
ru:
  home:
    aria_label: "Выбор типа расписания"
    options_label: "Варианты выбора"
  guide:
    open: "Открыть гайд"
```

- [x] **Step 7: Verify the gate**

Signed out — the room floor must be **absent from the HTML**, not hidden:

```bash
curl -s --noproxy '*' http://localhost:3000/ | grep -c 'href="/room"'
```

Expected: `0`.

```bash
curl -s -o /dev/null -w "%{http_code} %{redirect_url}\n" --noproxy '*' http://localhost:3000/room
```

Expected: `302 http://localhost:3000/?auth=1`.

Signed in, reusing the cookie jar from Task 5:

```bash
curl -s --noproxy '*' -b $JAR http://localhost:3000/ | grep -c 'href="/room"'
curl -s -o /dev/null -w "%{http_code}\n" --noproxy '*' -b $JAR http://localhost:3000/room
```

Expected: `1`, then `204`.

Return-path check: signed out, request `/room`, then log in through the dialog. Expected: landing on `/room`, not `/`.

- [x] **Step 8: Report changed files** (do not commit)

---

## Task 10: Integration and acceptance

**Files:**
- Modify: `docker-compose.yml`, `.env.example`
- Create: `apps/schedule_web/README.md`

**Interfaces:**
- Consumes: everything above.
- Produces: a verified running service.

- [x] **Step 1: Document environment variables**

Append to the root `.env.example` (names only, no secrets):

```
# schedule_web
SECRET_KEY_BASE=
API_GETEWAY_URL=http://api_geteway:2300
ADMIN_PANEL_URL=http://localhost:4173
WEB_CONCURRENCY=2
UPSTREAM_TIMEOUT_CONNECT=1.0
UPSTREAM_TIMEOUT_READ=3.0
UPSTREAM_TIMEOUT_TOTAL=5.0
BREAKER_THRESHOLD=5
BREAKER_WINDOW=30
BREAKER_COOLDOWN=15
CACHE_FRESH_TTL=60
CACHE_STALE_TTL=86400
RATE_LIMIT_GENERAL=300
RATE_LIMIT_LOGIN=10
REDIS_POOL_SIZE=16
TRUSTED_PROXIES=
FORCE_SSL=false
```

- [x] **Step 2: Write the README**

`apps/schedule_web/README.md` covering: purpose and the boundary with `apps/frontend`; the layer diagram from the spec; why Falcon and why no blocking clients are permitted; how to add a gateway (register a `Resilience::Call` key, return entity hashes); the component convention; and the environment variables above.

- [x] **Step 3: Full-stack acceptance run**

```bash
docker compose build schedule_web
docker compose up -d
docker compose ps
```

Then work the spec §13 checklist in order:

| # | Command / action | Expected |
|---|---|---|
| 1 | `docker compose ps schedule_web` | `healthy` |
| 2 | `curl -s -w " [%{http_code}]" --noproxy '*' localhost:3000/up` | `[200]` |
| 3 | `curl -s --noproxy '*' localhost:3000/readyz` | `"status":"ready"` |
| 4 | `docker compose stop redis` then re-run 3 | `[503]`, `"redis":false`. Restart redis. |
| 5 | `curl -s --noproxy '*' localhost:3000/ \| grep -c 'href="/room"'` | `0` |
| 6 | `curl -s -o /dev/null -w "%{http_code}" --noproxy '*' localhost:3000/room` | `302` |
| 7 | Sign in via the dialog in a browser | room floor appears |
| 8 | `docker compose stop api_geteway`, reload a page visited while it was up | renders with the staleness banner |
| 9 | Reload a page never visited | region error, header and footer intact |
| 10 | Repeat 9 six times, then `curl localhost:3000/readyz` | that key reports `open` |
| 11 | `for i in $(seq 1 15); do curl -s -o /dev/null -w "%{http_code} " --noproxy '*' -X POST localhost:3000/session; done` | ends in `429` |
| 12 | Resize 1280px → 375px on every page, short and long | footer centred, at the bottom, never right-shifted or stretched |

Restart `api_geteway` when finished.

- [x] **Step 4: Report changed files and the checklist outcome** (do not commit)

---

## Self-Review

**Spec coverage.** §1 boundary → Task 1 compose + Task 8 admin link. §3 runtime → Task 1. §4 layout → File Structure + Tasks 2-9. §5 lifecycle → Tasks 4-5, 9. §6 resilience → Task 3. §7 sessions/consent → Task 5. §8 middleware → Task 6. §9 components/footer → Tasks 7-8. §10 home/gating → Task 9. §11 configuration → Tasks 2, 10. §12 Docker → Tasks 1, 10. §13 verification → Task 10. §14 agents → dispatch, outside the plan. §15 follow-on → below.

**Amendments to the spec discovered while planning.** Both are recorded here and must be folded back into the spec document:

1. **Cookie consent is three categories, not a binary.** `schedule_home/index.html` presents necessary / functional / analytics with independent switches. `Session::Consent` stores a comma-separated category list, and spec §7's "granted/denied" wording is superseded.
2. **`async-redis` is mandatory, and Rack::Attack needs a custom store.** Spec §8 named Rack::Attack without noting that all its bundled stores block. `Resilience::RackAttackStore` (Task 6) closes this. Recorded in Global Constraints as a hard prohibition because it is the single easiest way to silently destroy the reactor.

**Placeholder scan.** No TBD/TODO. Two deliberate stubs, both explicitly scoped: `RoomsController#index` returns 204 pending Spec 2, and `/group` + `/teacher` are unrouted until Spec 2.

**Type consistency.** `Resilience::Call#call(params:)` returns `Result<Outcome>` throughout. `Outcome::Fresh`/`Stale` expose `data`, `stale?`, `cached_at` and are consumed by `Presenters::Base` with those exact names. Gateways cache entity **hashes** and callers rehydrate — stated in Task 4 Step 5 and relied on by Spec 2. `Policies::HomeFloors.call` returns `{target:, index:, path:}` and is consumed with those keys in Task 9 Step 3 and the guide dialog in Task 8 Step 5. `Session::TokenStore` is the sole writer of `session[:auth]`.

## Follow-on: Spec 2

Pages `/group`, `/teacher`, `/room`, `/schedule`, `/teacher/:id`, `/room/:id`, `/guide`, plus retirement of the React public routes.

**Blocking prerequisite discovered during planning.** Server-side pagination cannot render numbered pages as the design shows, because `/public/teachers` and `/public/rooms` return no total count — `all_recent` returns a bare array and the serializers emit no `meta` (`slices/portal/serializers/{teachers,rooms}/index.rb`). Spec 2 must begin with an additive change to `apps/api_geteway` adding `meta: { total, page, per_page }` to both index serializers. It is backward-compatible: the React client ignores unknown `meta`.

Related, and worth stating plainly: React has been fetching a single page and paginating it client-side (`EntitySearch.tsx:98`), so **rooms beyond the first page have never been reachable in the UI**. This is a pre-existing data-visibility bug, not a regression introduced by any of this work.

---

## Implementation Notes

Written after all ten tasks were built and verified. These are deviations from
the plan as drafted, recorded with their reasoning because most of them were
found the expensive way.

**Layout and loading**

1. **Namespaced layers moved to `app/lib/<layer>/`.** See the note under File
   Structure. The `app/<layer>/` form in the original draft made each layer an
   autoload root rather than a namespace; it raised `Zeitwerk::NameError` at
   boot and, because production eager-loads, left the container unhealthy and
   refusing every connection.
2. **Initializers cannot eagerly reference `app/lib` constants.**
   `config/initializers/*.rb` run at initializer index ~103-107, but Zeitwerk's
   `setup_main_autoloader` runs at 144 — the autoload stubs do not exist yet.
   Defer with `Rails.application.config.to_prepare` (index 149) or lazy blocks.
   `rack_attack.rb` assigns its store inside `to_prepare` for exactly this
   reason; `container.rb` is safe because its registration blocks are not
   evaluated at load time.
3. **`trusted_proxies` moved from `config/application.rb` to
   `config/initializers/logging.rb`.** The application class body evaluates
   before `settings.rb` runs, so `ScheduleWeb::Settings` is not yet defined.

**Resilience layer (Task 3)**

4. **`RetryableError` must be defined before `RetryPolicy`**, which references
   it in a constant. The drafted order raised `NameError` on load.
5. **Timeouts are retryable.** As drafted, `Async::TimeoutError` escaped the
   retry loop and failed after a single attempt, contradicting the stated
   policy that timeouts, connection faults and 5xx are all retryable.
6. **`Resilience::Call::KNOWN_KEYS`** declares the upstreams this service
   depends on, so `/readyz` reports real breaker state on a cold process. The
   registry is populated lazily on first use, so without this the endpoint
   returned an empty map — which reads as "nothing is broken" when the truth is
   "nothing has been looked at". A readiness probe must not tell that lie.
7. **Every Redis access is time-bounded.** `ScheduleWeb::Redis.call` takes a
   `timeout:` (default `timeout_connect`) and wraps the call in
   `Async::Task#with_timeout`. A *stopped* Redis container neither connects nor
   refuses — it stalls — so `/readyz` hung indefinitely instead of reporting
   `redis:false`. Bounding it inside `call` rather than `available?` means
   `StaleCache` inherits the protection, honouring the rule that Redis is an
   optimisation and never a correctness dependency.

**Assets and Docker**

8. **The Dockerfile precompiles assets.** Propshaft only serves assets live in
   development and test, so under `RAILS_ENV=production` every stylesheet and
   Stimulus controller returned 404 while pages still returned 200 — an
   unstyled, non-interactive site that looks healthy to `curl`. The step needs
   `RAILS_ENV=production` set explicitly (compose's `environment:` applies at
   `up`, not at `build`), `SECRET_KEY_BASE_DUMMY=1`, and placeholder
   `API_GETEWAY_URL`/`REDIS_URL` because precompile boots the initializers.
   **Consequence: changing any CSS or JS requires `docker compose build`, not
   just a restart.**
9. **CSS `@import` must use the `url("components/_name.css")` form.** A bare
   `@import "…"` is not digest-rewritten by Propshaft and 404s.
10. **Binstubs shipped with `#!/usr/bin/env ruby.exe`**, generated on Windows
    and unusable in the Linux container; corrected to `ruby`.
11. **`docker-compose.yml` shares Redis db 0** rather than using a dedicated
    db 1: `${REDIS_PASSWORD}` cannot be expanded in a service's `environment:`
    block when the value arrives via another service's `env_file`.

**Sessions, cookies, TLS**

12. **`assume_ssl` and `force_ssl` are env-driven and default false**, rather
    than `assume_ssl = true` as drafted. `assume_ssl` marks session cookies
    `secure`, and browsers silently drop secure cookies over plain HTTP — which
    would break sign-in on localhost with no visible error.
13. **`Session::Consent` keys its cookie's `secure` flag to the same signal.**
    It was `Rails.env.production?`, which conflates "environment is named
    production" with "connection is HTTPS"; `CookieJar#write_cookie?` then
    dropped the consent cookie without any error, so `POST /consent` returned
    302 and set nothing.
14. **No `session_store.rb` initializer was needed** — Rails 8's defaults
    already provide `httponly` and `same_site: :lax`.

**Middleware**

15. **Rate limiting fails open when Redis is unavailable.** `Redis.call`
    returns nil, and rack-attack treats a nil `increment` as a fresh count of
    1. This matches `Resilience::Breaker`'s documented fail-open choice: a
    monitoring outage must not block the whole service.

**Front end**

16. **Dialog focus never entered the panel.** `transition: visibility 200ms`
    keeps the panel `visibility: hidden` for the full duration, and a hidden
    element cannot take focus, so the controller's synchronous `.focus()`
    silently did nothing. Fixed with asymmetric timing: visibility flips
    instantly on open, and the flip back to hidden is deferred until the
    opacity fade completes on close. Only a real browser surfaces this — the
    markup, the JS and the CSS are each individually correct.
17. **Fixing 16 exposed a second bug.** `open()` set `lastTrigger`, then
    `show()` called `close()`, which cleared it — so Escape had nothing to
    focus and dropped the visitor on `<body>`. `close()` was split into
    `close()` (hide + restore focus) and `hideAll()` (hide only), with `show()`
    calling the latter. Before the fix the symptom was masked: focus had never
    left the trigger, so a naive check reported success.
18. **Task 9's "expected 1 `href="/room"`" is 2 in practice**, because
    `@page.floors` feeds both the scene grid and the guide dialog. The
    guest-side invariant is the one that matters and holds: 0 occurrences, the
    floor absent from the DOM entirely rather than hidden with CSS.

**Verification gaps worth knowing**

19. Checklist items 8-10 (staleness banner, region error, breaker-open via a
    data page) are **not applicable until Spec 2**: no Spec 1 route reaches a
    portal gateway. The home page renders static floors and
    `RoomsController#index` is a 204 stub. The underlying mechanisms were
    driven directly instead, and breaker state was confirmed to round-trip
    through Redis into a live `/readyz` across process boundaries.
20. **`curl -w '%{http_code}'` on a page proves the document rendered, not that
    the page works.** Item 8 above survived seven tasks of verification because
    every check asserted 200 on the HTML and none walked the asset references.
