# Public Period Exposure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expose the already-computed `period_kind` (semester / lecture_week / credit_week / exam_session) read-only through the public portal API, plus a new endpoint listing a group-semester's configured period date ranges — unblocking the frontend redesign's real period-jump control.

**Architecture:** `APIGeteway::Queries::SemesterResolver` (lib/) gains one batched query method and threads `period_kind` onto its `SemesterContext` struct at the two places that build one. Two portal serializers pick up the new field for free once it's on the struct. A new `GET /public/groups/:group_semester_id/periods` action/contract/serializer, following the existing `groups.now`/`groups.schedule` pattern exactly, exposes the full period list.

**Tech Stack:** Ruby 3.4 / Hanami 2.3 / Sequel (raw dataset queries against ROM's underlying `db`) / RSpec + database_cleaner-sequel.

## Global Constraints

- Repo root: `C:\Users\Asus\Desktop\milk_schedule\apps\api_geteway` (separate nested git repo — commit here, not in the outer repo).
- No schema/migration changes. No new gem dependencies.
- No auth required on the new endpoint (same trust level as the rest of `/public`).
- Every file this plan touches or creates is listed explicitly below with its full path.

## How to run tests (read this before Task 1)

There is no bind-mounted dev container and the shipped `.env.test` is dead
(its `localhost:5434`/`localhost:6381` values never apply — the `api_geteway`
container's docker-compose `env_file` already sets `DATABASE_URL`/`REDIS_URL`
as real process env vars, and dotenv never overrides an already-set var, so
`HANAMI_ENV=test` silently resolves to the **dev** database inside that
container). Also, `.dockerignore` excludes `spec/` entirely, so `spec/` does
not exist inside the built image at all — tests cannot run inside the
`api_geteway`/`media_sanitizer` containers as built.

Run tests **on the host** (Ruby 3.4 is installed at `C:\Ruby34-x64`,
`bundle check` already reports the Gemfile satisfied) against an isolated
`database_test` database on the same dockerized Postgres, and Redis logical
DB 1 (separate namespace from dev's DB 0, same `redis` container) — this
never touches dev data:

```bash
# one-time per session: bring the stack up, then create+migrate the isolated test DB
cd C:/Users/Asus/Desktop/milk_schedule
docker compose up -d db redis minio
cd apps/api_geteway
DATABASE_URL="postgresql://xjensu:7yu7ukmaj3@localhost:5432/database_test" \
REDIS_URL="redis://:7yu7ukmaj3@localhost:6379/1" \
HANAMI_ENV=test bundle exec hanami db create
DATABASE_URL="postgresql://xjensu:7yu7ukmaj3@localhost:5432/database_test" \
REDIS_URL="redis://:7yu7ukmaj3@localhost:6379/1" \
HANAMI_ENV=test bundle exec hanami db migrate

# every test run:
cd apps/api_geteway
DATABASE_URL="postgresql://xjensu:7yu7ukmaj3@localhost:5432/database_test" \
REDIS_URL="redis://:7yu7ukmaj3@localhost:6379/1" \
HANAMI_ENV=test bundle exec rspec <path>
```

(`hanami db create` is a no-op if `database_test` already exists — safe to
re-run. Both commands were already verified working in this environment.)

---

### Task 1: `SemesterResolver` — thread `period_kind` through `SemesterContext`

**Files:**
- Modify: `lib/api_geteway/queries/semester_resolver.rb`
- Test: `spec/lib/api_geteway/queries/semester_resolver_spec.rb` (new)

**Interfaces:**
- Consumes: nothing new — same `SemesterResolver.new(schedule_events_relation)` constructor.
- Produces: `SemesterContext#period_kind` (String: `"semester"` / `"lecture_week"` / `"credit_week"` / `"exam_session"`), always populated on every `SemesterContext` the class returns. New public method `special_period_kinds_for(group_semester_ids:, date:)` → `Hash[Integer, String]` (batched, one query, missing key means no special period active — caller should treat that as `"semester"`). Task 2 and Task 3 both read `ctx.period_kind` off contexts this task produces.

- [ ] **Step 1: Write the failing spec**

Create `spec/lib/api_geteway/queries/semester_resolver_spec.rb`:

```ruby
# frozen_string_literal: true

require_relative "../../../../lib/api_geteway/queries/semester_resolver"

# Scoped spec: only the period_kind behavior this task adds. SemesterResolver's
# broader query surface (active_for_faculty/active_for_teacher/active_for_kafedra)
# is exercised indirectly through the portal request specs — see
# spec/slices/portal/requests/groups_periods_spec.rb, whose fixture-seeding style
# (raw SQL via db.run, chained through the FK graph) this spec mirrors, following
# the precedent in spec/lib/api_geteway/queries/schedule_grid_spec.rb.
RSpec.describe APIGeteway::Queries::SemesterResolver, type: :feature do
  subject(:resolver) { described_class.new(schedule_events_relation) }

  let(:schedule_events_relation) { Portal::Slice["relations.schedule_events"] }
  let(:db) { Hanami.app["db.rom"].gateways[:default].connection }

  before do
    db.run <<~SQL
      INSERT INTO faculties (id, name, short_name) VALUES (501, 'Test Faculty', 'TF')
      ON CONFLICT (id) DO NOTHING;
    SQL

    db.run <<~SQL
      INSERT INTO specialities (id, faculty_id, name, duration_of_study)
        VALUES (501, 501, 'Computer Science', 4)
      ON CONFLICT (id) DO NOTHING;
    SQL

    db.run <<~SQL
      INSERT INTO student_groups (id, speciality_id, year_of_admission)
        VALUES (501, 501, '2022-09-01'), (502, 501, '2022-09-01')
      ON CONFLICT (id) DO NOTHING;
    SQL

    db.run <<~SQL
      INSERT INTO semesters (id, number, start_year, end_year)
        VALUES (501, 1, 2026, 2027)
      ON CONFLICT (id) DO NOTHING;
    SQL

    db.run <<~SQL
      INSERT INTO student_group_semesters (id, group_id, semester_id, students_now, duration)
        VALUES (501, 501, 501, 30, 18), (502, 502, 501, 25, 18)
      ON CONFLICT (id) DO NOTHING;
    SQL

    # Group 501: semester + lecture_week + a gap (ordinary semester days,
    # no special period) + credit_week + exam_session.
    db.run <<~SQL
      INSERT INTO student_group_semester_periods
        (student_group_semester_id, period_kind, start_date, end_date)
        VALUES
          (501, 'semester',     '2026-09-01', '2027-01-25'),
          (501, 'lecture_week', '2026-09-01', '2026-11-30'),
          (501, 'credit_week',  '2026-12-01', '2026-12-07'),
          (501, 'exam_session', '2026-12-22', '2027-01-25')
      ON CONFLICT (student_group_semester_id, period_kind) DO NOTHING;
    SQL

    # Group 502: only configured with an exam_session that overlaps the SAME
    # date group 501 is in lecture_week on — proves the batched lookup
    # resolves each id independently rather than sharing one result.
    db.run <<~SQL
      INSERT INTO student_group_semester_periods
        (student_group_semester_id, period_kind, start_date, end_date)
        VALUES
          (502, 'semester',     '2026-09-01', '2027-01-25'),
          (502, 'exam_session', '2026-10-01', '2026-10-31')
      ON CONFLICT (student_group_semester_id, period_kind) DO NOTHING;
    SQL
  end

  describe "#special_period_kinds_for" do
    it "returns the active special period_kind per group-semester, batched in one call" do
      result = resolver.special_period_kinds_for(group_semester_ids: [501, 502], date: "2026-10-01")

      expect(result).to eq(501 => "lecture_week", 502 => "exam_session")
    end

    it "omits an id with no special period covering the date" do
      # 2026-12-10 is inside group 501's overall semester but in the gap
      # between credit_week (ends 12-07) and exam_session (starts 12-22).
      result = resolver.special_period_kinds_for(group_semester_ids: [501], date: "2026-12-10")

      expect(result).to eq({})
    end

    it "returns an empty hash without querying when given no ids" do
      expect(resolver.special_period_kinds_for(group_semester_ids: [], date: "2026-10-01")).to eq({})
    end
  end

  describe "#resolve_group_semester" do
    it "sets period_kind to the active special period on a date inside one" do
      context = resolver.resolve_group_semester(group_semester_id: 501, date: "2026-10-01")

      expect(context.period_kind).to eq("lecture_week")
    end

    it "falls back to the ordinary semester kind on a date in the gap between special periods" do
      context = resolver.resolve_group_semester(group_semester_id: 501, date: "2026-12-10")

      expect(context.period_kind).to eq("semester")
    end
  end
end
```

- [ ] **Step 2: Run it to verify it fails**

```bash
cd apps/api_geteway
DATABASE_URL="postgresql://xjensu:7yu7ukmaj3@localhost:5432/database_test" \
REDIS_URL="redis://:7yu7ukmaj3@localhost:6379/1" \
HANAMI_ENV=test bundle exec rspec spec/lib/api_geteway/queries/semester_resolver_spec.rb
```

Expected: `NoMethodError: undefined method 'special_period_kinds_for'` (and/or `NoMethodError: undefined method 'period_kind' for an instance of APIGeteway::Queries::SemesterContext`).

- [ ] **Step 3: Implement**

In `lib/api_geteway/queries/semester_resolver.rb`:

Replace the `SemesterContext` struct definition:

```ruby
    SemesterContext = Struct.new(
      :group_semester_id, :semester_id, :semester_number, :start_year, :end_year,
      :start_date, :end_date, :label, :period_kind,
      keyword_init: true
    )
```

Add a new public method, placed after `periods_for` (before the `resolve_group_semester` method):

```ruby
      # Which SPECIAL period_kind (lecture_week/credit_week/exam_session) is
      # active, for a batch of group-semesters, on a single date — one query
      # regardless of how many ids are passed. An id with no covering special
      # period is absent from the result; callers treat a missing key as the
      # ordinary "semester" kind.
      def special_period_kinds_for(group_semester_ids:, date:)
        return {} if group_semester_ids.empty?

        d = to_date(date)
        db[:student_group_semester_periods]
          .where(student_group_semester_id: group_semester_ids.map(&:to_i))
          .where(period_kind: APIGeteway::WeekParity::SPECIAL_PERIOD_KINDS)
          .where(Sequel.lit("start_date <= ? AND end_date >= ?", d, d))
          .select(:student_group_semester_id, :period_kind)
          .all
          .each_with_object({}) { |row, acc| acc[row[:student_group_semester_id]] = row[:period_kind] }
      end
```

Replace `resolve_group_semester`'s body:

```ruby
      def resolve_group_semester(group_semester_id:, date:)
        d = to_date(date)
        row = db[:student_group_semester_periods]
          .join(Sequel[:student_group_semesters].as(:sgs),
                Sequel[:sgs][:id] => Sequel[:student_group_semester_periods][:student_group_semester_id])
          .join(Sequel[:semesters].as(:sem), Sequel[:sem][:id] => Sequel[:sgs][:semester_id])
          .where(Sequel[:student_group_semester_periods][:period_kind] => SEMESTER_PERIOD_KIND)
          .where(Sequel[:student_group_semester_periods][:student_group_semester_id] => group_semester_id.to_i)
          .where(Sequel.lit(
            "? BETWEEN student_group_semester_periods.start_date AND student_group_semester_periods.end_date", d
          ))
          .select(
            Sequel[:sgs][:id].as(:group_semester_id),
            Sequel[:sgs][:semester_id],
            Sequel[:sem][:number].as(:semester_number),
            Sequel[:sem][:start_year],
            Sequel[:sem][:end_year],
            Sequel[:student_group_semester_periods][:start_date],
            Sequel[:student_group_semester_periods][:end_date]
          )
          .first
        return nil unless row

        kind = special_period_kinds_for(group_semester_ids: [row[:group_semester_id]], date: d)[row[:group_semester_id]] ||
               SEMESTER_PERIOD_KIND
        build_context(row, period_kind: kind)
      end
```

(Only the last 4 lines change — the query itself is untouched.)

Replace `active_for_faculty`'s final two lines (`rows = ...all` stays; only what follows changes):

```ruby
          .order(Sequel.lit("course"), Sequel[:sp][:name])
          .all

        kinds = special_period_kinds_for(group_semester_ids: rows.map { |r| r[:group_semester_id] }, date: d)
        rows.map { |row| build_faculty_row(row, kinds) }
      end
```

Replace `build_context` and `build_faculty_row`:

```ruby
      def build_context(row, period_kind:)
        SemesterContext.new(
          group_semester_id: row[:group_semester_id],
          semester_id: row[:semester_id],
          semester_number: row[:semester_number],
          start_year: row[:start_year],
          end_year: row[:end_year],
          start_date: row[:start_date],
          end_date: row[:end_date],
          label: "#{row[:semester_number]} семестр #{row[:start_year]}/#{row[:end_year]}",
          period_kind: period_kind
        )
      end

      def build_faculty_row(row, kinds)
        {
          group_semester_id: row[:group_semester_id],
          group_id: row[:group_id],
          course: row[:course],
          speciality_name: row[:speciality_name],
          duration_of_study: row[:duration_of_study],
          students_now: row[:students_now],
          year_of_admission: row[:year_of_admission],
          semester: build_context(row, period_kind: kinds[row[:group_semester_id]] || SEMESTER_PERIOD_KIND)
        }
      end
```

- [ ] **Step 4: Run it to verify it passes**

```bash
cd apps/api_geteway
DATABASE_URL="postgresql://xjensu:7yu7ukmaj3@localhost:5432/database_test" \
REDIS_URL="redis://:7yu7ukmaj3@localhost:6379/1" \
HANAMI_ENV=test bundle exec rspec spec/lib/api_geteway/queries/semester_resolver_spec.rb
```

Expected: `6 examples, 0 failures`.

- [ ] **Step 5: Commit**

```bash
cd apps/api_geteway
git add lib/api_geteway/queries/semester_resolver.rb spec/lib/api_geteway/queries/semester_resolver_spec.rb
git commit -m "feat: thread period_kind onto SemesterContext"
```

---

### Task 2: Serialize `period_kind` on the public semester object

**Files:**
- Modify: `slices/portal/serializers/occurrence_window_serializer.rb`
- Modify: `slices/portal/serializers/faculties/groups.rb`
- Test: `spec/slices/portal/serializers/occurrence_window_serializer_spec.rb` (new)

**Interfaces:**
- Consumes: `APIGeteway::Queries::SemesterContext#period_kind` (Task 1), `APIGeteway::Queries::OccurrenceWindow` (existing struct, unchanged — `anchor_date`, `start_date`, `end_date`, `weeks`, `occurrences`, `transfers`, `holidays`).
- Produces: `meta.semester.period_kind` / `meta.semesters[].period_kind` in the JSON from `Portal::Serializers::OccurrenceWindowSerializer.call`, and `groups[].semester.period_kind` in `Portal::Serializers::Faculties::Groups#call`'s JSON. Both are plain-JSON string fields consumed by the frontend (Spec B).

- [ ] **Step 1: Write the failing spec**

Create `spec/slices/portal/serializers/occurrence_window_serializer_spec.rb`:

```ruby
# frozen_string_literal: true

# No DB needed: OccurrenceWindow/SemesterContext are plain Structs, built
# directly here to exercise the serializer in isolation.
RSpec.describe Portal::Serializers::OccurrenceWindowSerializer do
  let(:window) do
    APIGeteway::Queries::OccurrenceWindow.new(
      anchor_date: Date.parse("2026-10-01"),
      start_date: Date.parse("2026-09-29"),
      end_date: Date.parse("2026-10-12"),
      weeks: [
        { start_date: Date.parse("2026-09-29"), end_date: Date.parse("2026-10-05"),
          parity: "even", iso_week: 40, is_current: true },
        { start_date: Date.parse("2026-10-06"), end_date: Date.parse("2026-10-12"),
          parity: "odd", iso_week: 41, is_current: false }
      ],
      occurrences: [],
      transfers: [],
      holidays: []
    )
  end

  let(:semester) do
    APIGeteway::Queries::SemesterContext.new(
      group_semester_id: 501, semester_id: 501, semester_number: 1,
      start_year: 2026, end_year: 2027,
      start_date: Date.parse("2026-09-01"), end_date: Date.parse("2027-01-25"),
      label: "1 семестр 2026/2027", period_kind: "lecture_week"
    )
  end

  it "includes period_kind on meta.semester" do
    body = JSON.parse(described_class.call(window, semester: semester))

    expect(body.dig("meta", "semester", "period_kind")).to eq("lecture_week")
  end

  it "includes period_kind on each entry of meta.semesters" do
    body = JSON.parse(described_class.call(window, semesters: [semester]))

    expect(body.dig("meta", "semesters", 0, "period_kind")).to eq("lecture_week")
  end

  it "keeps meta.semester nil when no semester is passed" do
    body = JSON.parse(described_class.call(window))

    expect(body.dig("meta", "semester")).to be_nil
  end
end
```

- [ ] **Step 2: Run it to verify it fails**

```bash
cd apps/api_geteway
DATABASE_URL="postgresql://xjensu:7yu7ukmaj3@localhost:5432/database_test" \
REDIS_URL="redis://:7yu7ukmaj3@localhost:6379/1" \
HANAMI_ENV=test bundle exec rspec spec/slices/portal/serializers/occurrence_window_serializer_spec.rb
```

Expected: first two examples FAIL (`expected: "lecture_week", got: nil`); third passes already.

- [ ] **Step 3: Implement**

In `slices/portal/serializers/occurrence_window_serializer.rb`, replace `semester_hash`:

```ruby
        def semester_hash(ctx)
          {
            group_semester_id: ctx.group_semester_id,
            semester_id: ctx.semester_id,
            number: ctx.semester_number,
            label: ctx.label,
            start_date: ctx.start_date.to_s,
            end_date: ctx.end_date.to_s,
            period_kind: ctx.period_kind
          }
        end
```

In `slices/portal/serializers/faculties/groups.rb`, replace `semester_hash`:

```ruby
  def semester_hash(ctx)
    return nil unless ctx

    {
      semester_id: ctx.semester_id,
      number: ctx.semester_number,
      label: ctx.label,
      start_date: ctx.start_date.to_s,
      end_date: ctx.end_date.to_s,
      period_kind: ctx.period_kind
    }
  end
```

- [ ] **Step 4: Run it to verify it passes**

```bash
cd apps/api_geteway
DATABASE_URL="postgresql://xjensu:7yu7ukmaj3@localhost:5432/database_test" \
REDIS_URL="redis://:7yu7ukmaj3@localhost:6379/1" \
HANAMI_ENV=test bundle exec rspec spec/slices/portal/serializers/occurrence_window_serializer_spec.rb
```

Expected: `3 examples, 0 failures`.

- [ ] **Step 5: Commit**

```bash
cd apps/api_geteway
git add slices/portal/serializers/occurrence_window_serializer.rb slices/portal/serializers/faculties/groups.rb spec/slices/portal/serializers/occurrence_window_serializer_spec.rb
git commit -m "feat: serialize period_kind on the public semester object"
```

---

### Task 3: `GET /public/groups/:group_semester_id/periods`

**Files:**
- Modify: `slices/portal/repos/schedule_occurrence_repo.rb`
- Modify: `config/routes.rb`
- Create: `slices/portal/contracts/groups/periods_contract.rb`
- Create: `slices/portal/actions/groups/periods.rb`
- Create: `slices/portal/serializers/groups/periods.rb`
- Test: `spec/slices/portal/requests/groups_periods_spec.rb` (new)

**Interfaces:**
- Consumes: `repo.periods_for(group_semester_id:)` (new repo method, this task) → delegates to `APIGeteway::Queries::SemesterResolver#periods_for(group_semester_id:)` (already existed pre-Task-1, returns `Array<{id:, period_kind:, start_date:, end_date:}>`, unchanged by Task 1).
- Produces: `GET /public/groups/:group_semester_id/periods` → `200 {"group_semester_id": Integer, "periods": [{"period_kind": String, "start_date": "YYYY-MM-DD", "end_date": "YYYY-MM-DD"}, ...]}`. Consumed by the frontend (Spec B) to build the period-jump control.

- [ ] **Step 1: Write the failing request spec**

Create `spec/slices/portal/requests/groups_periods_spec.rb`:

```ruby
# frozen_string_literal: true

# GET /public/groups/:group_semester_id/periods
RSpec.describe "GET /public/groups/:group_semester_id/periods", type: :request, db: true do
  let(:db) { Hanami.app["db.rom"].gateways[:default].connection }

  before do
    db.run <<~SQL
      INSERT INTO faculties (id, name, short_name) VALUES (601, 'Test Faculty', 'TF')
      ON CONFLICT (id) DO NOTHING;
    SQL

    db.run <<~SQL
      INSERT INTO specialities (id, faculty_id, name, duration_of_study)
        VALUES (601, 601, 'Computer Science', 4)
      ON CONFLICT (id) DO NOTHING;
    SQL

    db.run <<~SQL
      INSERT INTO student_groups (id, speciality_id, year_of_admission)
        VALUES (601, 601, '2022-09-01')
      ON CONFLICT (id) DO NOTHING;
    SQL

    db.run <<~SQL
      INSERT INTO semesters (id, number, start_year, end_year)
        VALUES (601, 1, 2026, 2027)
      ON CONFLICT (id) DO NOTHING;
    SQL

    db.run <<~SQL
      INSERT INTO student_group_semesters (id, group_id, semester_id, students_now, duration)
        VALUES (601, 601, 601, 30, 18)
      ON CONFLICT (id) DO NOTHING;
    SQL

    db.run <<~SQL
      INSERT INTO student_group_semester_periods
        (student_group_semester_id, period_kind, start_date, end_date)
        VALUES
          (601, 'semester',     '2026-09-01', '2027-01-25'),
          (601, 'lecture_week', '2026-09-01', '2026-11-30'),
          (601, 'credit_week',  '2026-12-01', '2026-12-07'),
          (601, 'exam_session', '2026-12-22', '2027-01-25')
      ON CONFLICT (student_group_semester_id, period_kind) DO NOTHING;
    SQL
  end

  context "with a group-semester that has configured periods" do
    it "returns every period ordered by start_date" do
      get "/public/groups/601/periods"

      expect(last_response.status).to eq(200)
      body = JSON.parse(last_response.body)
      expect(body["group_semester_id"]).to eq(601)
      expect(body["periods"].map { |p| p["period_kind"] }).to eq(
        %w[semester lecture_week credit_week exam_session]
      )
      expect(body["periods"].first).to eq(
        "period_kind" => "semester", "start_date" => "2026-09-01", "end_date" => "2027-01-25"
      )
    end
  end

  context "with a group-semester that has no configured periods" do
    it "returns 200 with an empty array" do
      get "/public/groups/999999/periods"

      expect(last_response.status).to eq(200)
      body = JSON.parse(last_response.body)
      expect(body).to eq("group_semester_id" => 999_999, "periods" => [])
    end
  end

  context "with group_semester_id = 0 (not > 0)" do
    it "returns 422" do
      get "/public/groups/0/periods"

      expect(last_response.status).to eq(422)
    end
  end

  context "period_kind end-to-end on the schedule endpoint (Task 1 + Task 2 integration)" do
    it "reflects the real active period on a date inside a special period" do
      get "/public/groups/601/schedule?date=2026-10-01"

      expect(last_response.status).to eq(200)
      body = JSON.parse(last_response.body)
      expect(body.dig("meta", "semester", "period_kind")).to eq("lecture_week")
    end

    it "falls back to the ordinary semester kind on a date with no special period active" do
      get "/public/groups/601/schedule?date=2026-12-10"

      expect(last_response.status).to eq(200)
      body = JSON.parse(last_response.body)
      expect(body.dig("meta", "semester", "period_kind")).to eq("semester")
    end
  end
end
```

- [ ] **Step 2: Run it to verify it fails**

```bash
cd apps/api_geteway
DATABASE_URL="postgresql://xjensu:7yu7ukmaj3@localhost:5432/database_test" \
REDIS_URL="redis://:7yu7ukmaj3@localhost:6379/1" \
HANAMI_ENV=test bundle exec rspec spec/slices/portal/requests/groups_periods_spec.rb
```

Expected: the two `period_kind end-to-end` examples PASS already (Task 1 + 2 done); the three `/periods` examples FAIL with 404 (no such route yet).

- [ ] **Step 3: Implement**

Add to `slices/portal/repos/schedule_occurrence_repo.rb`, in the public section (right after `semesters_for`, before `empty_window`):

```ruby
      # Every configured period (kind + date range) for a group-semester, for
      # the public period-jump control. Does not check whether a 'semester'
      # period covers any particular date — this lists everything configured,
      # same as the admin builder's period buttons. An id with nothing
      # configured gets an empty array, not a 404 (D2 philosophy: missing
      # configuration is visible, not an error the client special-cases).
      def periods_for(group_semester_id:)
        semester_resolver.periods_for(group_semester_id: group_semester_id.to_i)
      end
```

Create `slices/portal/contracts/groups/periods_contract.rb`:

```ruby
# frozen_string_literal: true

module Portal
  module Contracts
    module Groups
      class PeriodsContract < Dry::Validation::Contract
        params do
          required(:group_semester_id).value(:integer, gt?: 0)
        end
      end
    end
  end
end
```

Create `slices/portal/actions/groups/periods.rb`:

```ruby
# frozen_string_literal: true

module Portal
  module Actions
    module Groups
      # GET /public/groups/:group_semester_id/periods
      #
      # Read-only list of every period configured for the group-semester
      # (period_kind + its date range): semester/lecture_week/credit_week/
      # exam_session. Unlike /schedule, this does not resolve against an
      # anchor date — it lists everything configured so the client can build
      # a period-jump control. An id with no configured periods gets back an
      # empty array, not a 404.
      class Periods < Portal::Action
        include Deps[
          repo:                "repos.schedule_occurrence_repo",
          validation_contract: "contracts.groups.periods_contract",
          serializer:          "serializers.groups.periods"
        ]

        def handle(request, response)
          return unless validate_or_render(validation_contract, request.params.to_h, response)

          group_semester_id = request.params[:group_semester_id].to_i
          periods = repo.periods_for(group_semester_id: group_semester_id)

          render_serialized(response, serializer, { group_semester_id: group_semester_id, periods: periods })
        end
      end
    end
  end
end
```

Create `slices/portal/serializers/groups/periods.rb`:

```ruby
# frozen_string_literal: true

class Portal::Serializers::Groups::Periods
  def call(resource)
    {
      group_semester_id: resource[:group_semester_id],
      periods: resource[:periods].map { |p| period_hash(p) }
    }.to_json
  end

  private

  def period_hash(p)
    { period_kind: p[:period_kind], start_date: p[:start_date].to_s, end_date: p[:end_date].to_s }
  end
end
```

In `config/routes.rb`, inside the `slice :portal, at: "/public"` block, add a line right after the `groups.schedule` route (which currently reads `get "/groups/:group_semester_id/schedule",  to: "groups.schedule"`):

```ruby
      get "/groups/:group_semester_id/periods",   to: "groups.periods"
```

- [ ] **Step 4: Run it to verify it passes**

```bash
cd apps/api_geteway
DATABASE_URL="postgresql://xjensu:7yu7ukmaj3@localhost:5432/database_test" \
REDIS_URL="redis://:7yu7ukmaj3@localhost:6379/1" \
HANAMI_ENV=test bundle exec rspec spec/slices/portal/requests/groups_periods_spec.rb
```

Expected: `6 examples, 0 failures`.

- [ ] **Step 5: Run the full portal + lib test suites to confirm nothing else broke**

```bash
cd apps/api_geteway
DATABASE_URL="postgresql://xjensu:7yu7ukmaj3@localhost:5432/database_test" \
REDIS_URL="redis://:7yu7ukmaj3@localhost:6379/1" \
HANAMI_ENV=test bundle exec rspec spec/slices/portal spec/lib/api_geteway/queries
```

Expected: 0 failures. (`spec/slices/portal/requests/groups_spec.rb`'s pre-existing assertions are unrelated to this change and are not touched by this plan; if it was already failing before Task 1, it's a pre-existing issue, not a regression introduced here — confirm by checking whether its failures, if any, are about the `data` key shape rather than about period_kind/periods.)

- [ ] **Step 6: Manual smoke check of `active_for_faculty`'s period_kind threading**

`active_for_faculty` (used by `/public/faculties/:id/groups`) was mechanically
updated in Task 1 in the same shape as the tested `resolve_group_semester`,
but isn't covered by an automated test (it additionally requires a
schedule-presence fixture chain unrelated to this feature). Confirm it
doesn't crash with a read-only request against the real dev container
(already running, this is a GET with no side effects):

```bash
curl -s "http://localhost:2300/public/faculties/1/groups" | head -c 2000
```

Expected: 200 response; if `groups` is non-empty, confirm each entry's
`semester.period_kind` is present and is one of the four valid values.

- [ ] **Step 7: Commit**

```bash
cd apps/api_geteway
git add slices/portal/repos/schedule_occurrence_repo.rb slices/portal/contracts/groups/periods_contract.rb slices/portal/actions/groups/periods.rb slices/portal/serializers/groups/periods.rb config/routes.rb spec/slices/portal/requests/groups_periods_spec.rb
git commit -m "feat: add public GET /groups/:id/periods endpoint"
```
