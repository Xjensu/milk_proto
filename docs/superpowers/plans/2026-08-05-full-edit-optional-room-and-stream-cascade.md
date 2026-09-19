# Full Cell Edit, Optional Room, and Stream Cascade — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let an operator fully edit an existing scheduled class (subject, teacher, room, day, slot, week type — not just day/slot/room/status), have that edit and any delete cascade correctly across the sibling classes of a shared ("поточное") lecture, and allow creating a class with no room and/or no teacher assigned yet.

**Architecture:** Every `schedule_events` row created together in one action (a stream lecture spanning N groups, or a lone class) now shares a DB-generated `event_batch_id`. Deleting any one row cascades to delete the whole batch. "Editing" a class is redefined as **delete the batch, then re-run the exact same creation path** (`POST /schedule_events/from_recommendation`) with the edited values — this is not a new code path, it is the existing stream-creation flow invoked again, so a class that's still part of the same stream comes back for every currently-matching group, and a class removed from the stream simply isn't recreated for that group. `schedule_events.room_id` becomes nullable end-to-end (DB, contracts, the core grid query's join, and both editor UIs). The frontend gets ONE shared editing experience (`OccurrenceEditor`, extended to work with or without a pinned date) instead of two divergent ones (`OccurrenceEditor` for the day-grid, the much thinner `ScheduleInspector` for the weekly builder grid).

**Tech Stack:** Ruby/Hanami 2.3 + ROM/Sequel (apps/api_geteway, ports 2300), React 18 + TypeScript + Vite (apps/frontend), RSpec, Vitest.

## Global Constraints

- apps/api_geteway and apps/frontend are each their OWN nested git repos — commit inside them, not the root repo.
- A class carries at most 2 teachers and at most 2 rooms (existing product rule, `MAX_TEACHERS_PER_EVENT` / `MAX_ROOMS_PER_EVENT` in `OccurrenceEditor.tsx`) — keep enforcing it in every place that builds a create/edit payload.
- Stream-linked classes (siblings created together for several groups) are always whole-group: `subgroup_number` is forced to `0` for every sibling when `streamSiblingGsids.length > 0`; lab sub-group splitting (`1`/`2`) only applies to a single group's own class.
- Match week-type periods on `week_types.code`, never a hardcoded id (differs per environment) — existing rule, keep following it in every new query.
- Docker: neither `api_geteway` nor `frontend` has a bind mount; `docker compose up -d` does not pick up code changes — use `docker compose build <svc> && docker compose up -d --force-recreate <svc>` to verify against a live container.
- Backend tests run with RSpec (doubles for injected deps — see `spec/slices/admin/scheduling/create_schedule_event_for_stream_spec.rb` for the house style). Frontend pure-function tests run with Vitest (see `slotEditor.test.ts`).

---

## File Structure

**Backend — `apps/api_geteway`:**
- New: `config/db/migrate/20260805130000_schedule_events_optional_room_and_batch.rb` — nullable `room_id`, new `event_batch_id` uuid column.
- Modify: `slices/admin/contracts/schedule_events/create_contract.rb`, `slices/admin/contracts/schedule_events/from_recommendation_contract.rb` — `room_id` becomes optional.
- Modify: `slices/admin/actions/schedule_events/from_recommendation.rb` — nil-safe `room_id` parsing.
- Modify: `slices/admin/scheduling/create_schedule_event_for_stream.rb` — skip the room-conflict guard when there's no room; stamp one shared `event_batch_id` across every sibling row created in the call.
- Modify: `lib/api_geteway/queries/schedule_grid.rb` — `rooms` join becomes a `LEFT JOIN` (an inner join would silently drop every roomless class from every grid/occupancy/notice view); expose `load_requirement_id` on every row.
- Modify: `slices/admin/presenters/schedule_event_presenter.rb` — the `:room` link must not build a URL with a nil id.
- Modify: `slices/admin/repos/schedule_event_repo.rb` — add `batch_events(id)`.
- Modify: `slices/admin/actions/schedule_events/destroy.rb` — cascades to the whole batch.
- New: `slices/admin/actions/schedule_events/batch.rb` — `GET /schedule_events/:id/batch`.
- Modify: `config/routes.rb` — register the new route.
- Test files alongside each (see individual tasks).

**Frontend — `apps/frontend`:**
- Modify: `src/features/admin/schedule/loadPanel.ts` — extract `makeSiblingResolver` (currently duplicated ad hoc inside `AdminSchedule.tsx`; `AdminOperations.tsx` needs the same logic).
- Modify: `src/features/admin/schedule/DayScheduleGrid.tsx` — `DayEvent` gains `loadRequirementId` and `groupSemesterId`.
- Modify: `src/features/admin/schedule/BuilderGrid.tsx` — `BuilderEvent` gains the same fields plus structured `teachers`, so it can drive the same editor as the day grid.
- Modify: `src/lib/api.ts` — `scheduleEventBatch`, nullable `room_id` in the two payload types that carry it.
- Modify: `src/features/admin/schedule/SlotEditor.tsx` — room becomes optional; subgroup forced to 0 for stream siblings; three helpers exported for reuse.
- Modify: `src/features/admin/schedule/OccurrenceEditor.tsx` — the "Параметры занятия" tab is rebuilt around the same load-based form as `SlotEditor`; save = delete batch + recreate; works with an empty `date` (builder-grid context hides the "Эта дата" tab).
- Modify: `src/features/admin/AdminSchedule.tsx` — builder-grid cell clicks open `OccurrenceEditor` instead of `ScheduleInspector`.
- Modify: `src/features/admin/AdminOperations.tsx` — wires the new `resolveSiblings`/`groupSemesterId` props into its existing `OccurrenceEditor` usage.
- Test files alongside each.

---

### Task 1: Migration — nullable `room_id`, batch id

**Files:**
- Create: `apps/api_geteway/config/db/migrate/20260805130000_schedule_events_optional_room_and_batch.rb`

**Interfaces:**
- Produces: `schedule_events.room_id` nullable; `schedule_events.event_batch_id` (uuid, `NOT NULL`, `DEFAULT gen_random_uuid()`, indexed) — every row, including pre-existing ones, gets a value automatically (the default is volatile, so Postgres computes one per existing row rather than reusing a single constant).

- [ ] **Step 1: Write the migration**

```ruby
# frozen_string_literal: true

# Two independent changes, bundled because both are needed before the create/edit
# flows can be touched:
#
# 1. room_id becomes nullable — a class can now be created before a room is
#    assigned. schedule_grid.rb's join on rooms must become a LEFT JOIN or every
#    roomless class silently vanishes from every grid/occupancy/notice view (see
#    that file's migration).
#
# 2. event_batch_id ties together every schedule_events row created in ONE
#    creation call (every group in a shared "поточное" lecture, or a lone class).
#    Deleting/editing any one row now finds its siblings via this column instead
#    of relying on same-room/same-time coincidence. The default is volatile
#    (gen_random_uuid()), so Postgres assigns a distinct value to every
#    pre-existing row too, not just new ones — each becomes its own batch of one.
ROM::SQL.migration do
  change do
    alter_table(:schedule_events) do
      set_column_allow_null :room_id
      add_column :event_batch_id, :uuid, null: false, default: Sequel.function(:gen_random_uuid)
      add_index :event_batch_id, name: :idx_schedule_events_batch_id
    end
  end
end
```

- [ ] **Step 2: Run the migration**

Run: `cd apps/api_geteway && bundle exec hanami db migrate` (or the project's equivalent migration command — check `package.json`/`Rakefile`/`bin/` for the exact invocation used elsewhere in this repo before assuming `hanami db migrate`).
Expected: migration applies with no error; `\d schedule_events` in `psql` shows `room_id` nullable and a new `event_batch_id uuid not null` column with a default.

- [ ] **Step 3: Commit**

```bash
cd apps/api_geteway
git add config/db/migrate/20260805130000_schedule_events_optional_room_and_batch.rb
git commit -m "feat(schedule_events): make room_id nullable, add event_batch_id"
```

---

### Task 2: Contracts — `room_id` optional on create paths

**Files:**
- Modify: `apps/api_geteway/slices/admin/contracts/schedule_events/create_contract.rb:11`
- Modify: `apps/api_geteway/slices/admin/contracts/schedule_events/from_recommendation_contract.rb:14`
- Test: `apps/api_geteway/spec/slices/admin/contracts/schedule_events/create_contract_spec.rb` (create if it doesn't already exist)
- Test: `apps/api_geteway/spec/slices/admin/contracts/schedule_events/from_recommendation_contract_spec.rb` (create if it doesn't already exist)

**Interfaces:**
- Consumes: nothing new.
- Produces: both contracts now accept a request with `room_id` omitted or `null`; `teacher_on_kafedra_ids` was already effectively optional (an empty array passes) — no change needed there, confirmed by reading both contracts.

- [ ] **Step 1: Write the failing tests**

```ruby
# frozen_string_literal: true

require_relative "../../../../../slices/admin/contracts/schedule_events/create_contract"

RSpec.describe Admin::Contracts::ScheduleEvents::CreateContract do
  subject(:contract) { described_class.new }

  def base_params(overrides = {})
    { schedule_event: {
      load_requirement_id: 1, time_slot_id: 2, week_type_id: 3,
      status: "scheduled", day_of_week: 1
    }.merge(overrides) }
  end

  it "accepts a request with room_id omitted" do
    result = contract.call(base_params)
    expect(result.errors.to_h).not_to have_key(:schedule_event)
  end

  it "accepts room_id: nil explicitly" do
    result = contract.call(base_params(room_id: nil))
    expect(result.errors.to_h.dig(:schedule_event, :room_id)).to be_nil
  end

  it "still rejects a non-integer room_id" do
    result = contract.call(base_params(room_id: "not-a-number"))
    expect(result.errors.to_h.dig(:schedule_event, :room_id)).not_to be_nil
  end
end
```

```ruby
# frozen_string_literal: true

require_relative "../../../../../slices/admin/contracts/schedule_events/from_recommendation_contract"

RSpec.describe Admin::Contracts::ScheduleEvents::FromRecommendationContract do
  subject(:contract) { described_class.new }

  def base_params(overrides = {})
    { recommendation: {
      day_of_week: 1, time_slot_id: 2, call_number: 1, week_type_id: 3,
      week_type_code: "both", teacher_on_kafedra_ids: [], load_requirement_ids: [10]
    }.merge(overrides) }
  end

  it "accepts a recommendation with room_id omitted" do
    result = contract.call(base_params)
    expect(result.errors.to_h).not_to have_key(:recommendation)
  end

  it "accepts room_id: nil explicitly" do
    result = contract.call(base_params(room_id: nil))
    expect(result.errors.to_h.dig(:recommendation, :room_id)).to be_nil
  end
end
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd apps/api_geteway && bundle exec rspec spec/slices/admin/contracts/schedule_events/create_contract_spec.rb spec/slices/admin/contracts/schedule_events/from_recommendation_contract_spec.rb`
Expected: FAIL — both contracts currently `required(:room_id)`, so omitting it produces a `:room_id` `is missing` error.

- [ ] **Step 3: Relax both contracts**

In `create_contract.rb`, change line 11 from:
```ruby
            required(:room_id).value(:integer)
```
to:
```ruby
            optional(:room_id).maybe(:integer)
```

In `from_recommendation_contract.rb`, change line 14 from:
```ruby
            required(:room_id).filled(:integer)
```
to:
```ruby
            optional(:room_id).maybe(:integer)
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd apps/api_geteway && bundle exec rspec spec/slices/admin/contracts/schedule_events/create_contract_spec.rb spec/slices/admin/contracts/schedule_events/from_recommendation_contract_spec.rb`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
cd apps/api_geteway
git add slices/admin/contracts/schedule_events/create_contract.rb slices/admin/contracts/schedule_events/from_recommendation_contract.rb spec/slices/admin/contracts/schedule_events/create_contract_spec.rb spec/slices/admin/contracts/schedule_events/from_recommendation_contract_spec.rb
git commit -m "feat(schedule_events): allow creating a class with no room"
```

---

### Task 3: `from_recommendation.rb` + `CreateScheduleEventForStream` — nil-safe room, skip the conflict check, stamp the batch id

**Files:**
- Modify: `apps/api_geteway/slices/admin/actions/schedule_events/from_recommendation.rb:55`
- Modify: `apps/api_geteway/slices/admin/scheduling/create_schedule_event_for_stream.rb`
- Test: `apps/api_geteway/spec/slices/admin/scheduling/create_schedule_event_for_stream_spec.rb` (extend the existing file)

**Interfaces:**
- Consumes: `Admin::Scheduling::Values::Recommendation#room_id` may now be `nil`.
- Produces: `CreateScheduleEventForStream#call` stamps every created row with the SAME `event_batch_id` (a fresh `SecureRandom.uuid` generated once per call) — later tasks (batch lookup, cascade delete) rely on this.

- [ ] **Step 1: Write the failing tests** (append to `create_schedule_event_for_stream_spec.rb`)

```ruby
  describe "no room assigned" do
    let(:recommendation) { build_recommendation(room_id: nil) }

    it "does not check room occupancy" do
      expect(rooms_rel).not_to receive(:has_occupied?)
      operation.call(recommendation: recommendation)
    end

    it "creates the events with room_id: nil" do
      expect(se_repo).to receive(:create).with(hash_including(room_id: nil)).twice.and_return(created_event)
      operation.call(recommendation: recommendation)
    end
  end

  describe "batch id" do
    let(:recommendation) { build_recommendation }

    it "stamps every created row in the same call with the same event_batch_id" do
      captured = []
      allow(se_repo).to receive(:create) { |attrs| captured << attrs[:event_batch_id]; created_event }
      operation.call(recommendation: recommendation)
      expect(captured.size).to eq(2)
      expect(captured.uniq.size).to eq(1)
      expect(captured.first).to be_a(String)
    end

    it "uses a different batch id across two separate calls" do
      first_batch = nil
      allow(se_repo).to receive(:create) { |attrs| first_batch ||= attrs[:event_batch_id]; created_event }
      operation.call(recommendation: recommendation)
      second_batch = nil
      allow(se_repo).to receive(:create) { |attrs| second_batch ||= attrs[:event_batch_id]; created_event }
      operation.call(recommendation: recommendation)
      expect(first_batch).not_to eq(second_batch)
    end
  end
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd apps/api_geteway && bundle exec rspec spec/slices/admin/scheduling/create_schedule_event_for_stream_spec.rb`
Expected: FAIL — `room_taken?` is currently called unconditionally, and no `event_batch_id` is passed to `create` at all.

- [ ] **Step 3: Update `create_schedule_event_for_stream.rb`**

Change the `call` method's transaction block from:
```ruby
        created = se_repo.transaction do
          # Re-check room availability inside the transaction to guard against races
          # between two concurrent accepts for the same slot.
          if room_taken?(recommendation)
            raise SlotTakenError, "Room #{recommendation.room_id} is no longer available " \
                                  "(day=#{recommendation.day_of_week}, " \
                                  "time_slot=#{recommendation.time_slot_id}, " \
                                  "week_type=#{recommendation.week_type_id})"
          end

          recommendation.load_requirement_ids.map do |lr_id|
            ev = se_repo.create(
              load_requirement_id: lr_id,
              time_slot_id:        recommendation.time_slot_id,
              room_id:             recommendation.room_id,
              week_type_id:        recommendation.week_type_id,
              day_of_week:         recommendation.day_of_week,
              specific_date:       recommendation.specific_date,
              subgroup_number:     recommendation.subgroup_number,
              status:              "scheduled",
              is_manual_change:    false
            )
```
to:
```ruby
        created = se_repo.transaction do
          # Re-check room availability inside the transaction to guard against races
          # between two concurrent accepts for the same slot. Skipped entirely when
          # no room is assigned yet — there is nothing to conflict on.
          if recommendation.room_id && room_taken?(recommendation)
            raise SlotTakenError, "Room #{recommendation.room_id} is no longer available " \
                                  "(day=#{recommendation.day_of_week}, " \
                                  "time_slot=#{recommendation.time_slot_id}, " \
                                  "week_type=#{recommendation.week_type_id})"
          end

          # One id shared by every row this call creates (every group in a shared
          # lecture, or the lone row for an unshared one) — Destroy and the new
          # /schedule_events/:id/batch endpoint use it to find siblings.
          batch_id = SecureRandom.uuid

          recommendation.load_requirement_ids.map do |lr_id|
            ev = se_repo.create(
              load_requirement_id: lr_id,
              time_slot_id:        recommendation.time_slot_id,
              room_id:             recommendation.room_id,
              week_type_id:        recommendation.week_type_id,
              day_of_week:         recommendation.day_of_week,
              specific_date:       recommendation.specific_date,
              subgroup_number:     recommendation.subgroup_number,
              status:              "scheduled",
              is_manual_change:    false,
              event_batch_id:      batch_id
            )
```
(The rest of the block — `event_teachers` creation, notification, re-fetch — is unchanged.)

- [ ] **Step 4: Nil-safe `room_id` in the action**

In `from_recommendation.rb`, change line 55 from:
```ruby
            room_id:                  params[:room_id].to_i,
```
to:
```ruby
            room_id:                  params[:room_id] ? params[:room_id].to_i : nil,
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cd apps/api_geteway && bundle exec rspec spec/slices/admin/scheduling/create_schedule_event_for_stream_spec.rb`
Expected: PASS (all tests, including the pre-existing ones — `hash_including(subgroup_number: 0)` etc. still match since `event_batch_id` is an additional key, not a replacement).

- [ ] **Step 6: Commit**

```bash
cd apps/api_geteway
git add slices/admin/actions/schedule_events/from_recommendation.rb slices/admin/scheduling/create_schedule_event_for_stream.rb spec/slices/admin/scheduling/create_schedule_event_for_stream_spec.rb
git commit -m "feat(schedule_events): skip room conflict check when roomless, stamp batch id"
```

---

### Task 4: `schedule_grid.rb` — `LEFT JOIN` rooms, expose `load_requirement_id`

**Files:**
- Modify: `apps/api_geteway/lib/api_geteway/queries/schedule_grid.rb`
- Test: `apps/api_geteway/spec/lib/api_geteway/queries/schedule_grid_spec.rb` (extend if it exists; if no spec file covers this query today, create one scoped to just these two behaviors — do not attempt to backfill full coverage of the whole class)

**Interfaces:**
- Produces: `ScheduleGridRow#room_id`/`#room_name`/`#room_number` are `nil` for a roomless class instead of the row being silently excluded; `ScheduleGridRow#load_requirement_id` is now available on every row.

- [ ] **Step 1: Write the failing test**

Check first whether `spec/lib/api_geteway/queries/schedule_grid_spec.rb` already exists and what fixtures/DB setup it uses (e.g. `FactoryBot` factories for `schedule_events`, `rooms`, etc. — grep the spec directory for existing factories before writing this). Using whatever the existing pattern is, add:

```ruby
  describe "a class with no room assigned" do
    it "is still returned, with room fields nil" do
      event = create(:schedule_event, room_id: nil) # adjust factory/trait to your fixtures
      rows = described_class.new(schedule_events_relation).for_group_semester(group_semester_id: event's group_semester_id)
      row = rows.find { |r| r.id == event.id }
      expect(row).not_to be_nil
      expect(row.room_id).to be_nil
      expect(row.room_name).to be_nil
    end
  end

  describe "load_requirement_id" do
    it "is present on every row" do
      event = create(:schedule_event)
      rows = described_class.new(schedule_events_relation).for_group_semester(group_semester_id: event's group_semester_id)
      row = rows.find { |r| r.id == event.id }
      expect(row.load_requirement_id).to eq(event.load_requirement_id)
    end
  end
```

Adapt the exact fixture-building calls to match whatever this repo's existing specs use (factories vs. plain `relation.command(:create).call`) — grep `spec/slices/admin/repos/schedule_event_repo_spec.rb` for the pattern already in use for `schedule_events` and mirror it exactly rather than guessing a `create(:schedule_event, ...)` FactoryBot call that may not exist.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd apps/api_geteway && bundle exec rspec spec/lib/api_geteway/queries/schedule_grid_spec.rb`
Expected: FAIL — a roomless class currently doesn't appear in the result set at all (inner join excludes it); `load_requirement_id` isn't on the struct.

- [ ] **Step 3: Add `load_requirement_id` to the struct**

In `schedule_grid.rb`, change the `ScheduleGridRow` definition (lines 7-24) from:
```ruby
    ScheduleGridRow = Struct.new(
      :id, :day_of_week, :specific_date, :subgroup_number, :status, :is_manual_change,
      :time_slot_id, :time_slot_start_time, :time_slot_end_time,
      :week_type_id, :week_type_name,
      :room_id, :room_name, :room_number,
      :subject_id, :subject_name, :subject_short_name,
```
to:
```ruby
    ScheduleGridRow = Struct.new(
      :id, :day_of_week, :specific_date, :subgroup_number, :status, :is_manual_change,
      :load_requirement_id,
      :time_slot_id, :time_slot_start_time, :time_slot_end_time,
      :week_type_id, :week_type_name,
      :room_id, :room_name, :room_number,
      :subject_id, :subject_name, :subject_short_name,
```

- [ ] **Step 4: `LEFT JOIN` rooms**

In `grid_query` (private method), change line 255 from:
```ruby
          .join(Sequel[:rooms].as(:rm),                      Sequel[:rm][:id]                    => Sequel[:schedule_events][:room_id])
```
to:
```ruby
          .left_join(Sequel[:rooms].as(:rm),                 Sequel[:rm][:id]                    => Sequel[:schedule_events][:room_id])
```

- [ ] **Step 5: Add `load_requirement_id` to the `GROUP BY` and `SELECT`**

In the same method, in the `.group(...)` call, add a line right after `Sequel[:schedule_events][:is_manual_change],`:
```ruby
            Sequel[:schedule_events][:load_requirement_id],
```

In the `.select(...)` call, add a line right after `Sequel[:schedule_events][:is_manual_change],`:
```ruby
            Sequel[:schedule_events][:load_requirement_id],
```

- [ ] **Step 6: Run the test to verify it passes**

Run: `cd apps/api_geteway && bundle exec rspec spec/lib/api_geteway/queries/schedule_grid_spec.rb`
Expected: PASS

- [ ] **Step 7: Run the full backend suite to catch any other spec asserting an exact column list**

Run: `cd apps/api_geteway && bundle exec rspec spec/lib/api_geteway/queries/ spec/slices/admin/repos/schedule_event_repo_spec.rb spec/slices/admin/actions/schedule_events/`
Expected: PASS. If any spec asserts a literal row shape (e.g. `expect(row.to_h.keys).to eq([...])`), update its expected list to include `load_requirement_id`.

- [ ] **Step 8: Commit**

```bash
cd apps/api_geteway
git add lib/api_geteway/queries/schedule_grid.rb spec/lib/api_geteway/queries/schedule_grid_spec.rb
git commit -m "fix(schedule_grid): left-join rooms so roomless classes stay visible, expose load_requirement_id"
```

---

### Task 5: Presenter — nil-safe room link

**Files:**
- Modify: `apps/api_geteway/slices/admin/presenters/schedule_event_presenter.rb:28`

**Interfaces:**
- Consumes: `e.room_id` may be `nil` (Task 1).
- Produces: the `:room` link is simply absent (not a broken `/admin/rooms/` URL) on a roomless event.

- [ ] **Step 1: Edit the link block**

Change line 28 from:
```ruby
      link(:room)             { |e, ctx| ctx.url("/admin/rooms/#{e.room_id}") }
```
to:
```ruby
      link(:room)             { |e, ctx| e.room_id ? ctx.url("/admin/rooms/#{e.room_id}") : nil }
```

- [ ] **Step 2: Manual verification (no existing presenter spec to extend)**

Run: `cd apps/api_geteway && bundle exec rspec spec/slices/admin/presenters/ -e "schedule_event"` to confirm nothing already covering this breaks. If no such spec exists, this is a one-line defensive change covered end-to-end by Task 8's action spec (which exercises a roomless event through the full stack) — no dedicated unit test needed here.

- [ ] **Step 3: Commit**

```bash
cd apps/api_geteway
git add slices/admin/presenters/schedule_event_presenter.rb
git commit -m "fix(schedule_event_presenter): don't build a broken room link when roomless"
```

---

### Task 6: `schedule_event_repo.rb` — `batch_events(id)`

**Files:**
- Modify: `apps/api_geteway/slices/admin/repos/schedule_event_repo.rb`
- Test: `apps/api_geteway/spec/slices/admin/repos/schedule_event_repo_spec.rb` (extend)

**Interfaces:**
- Produces: `ScheduleEventRepo#batch_events(id) -> Array<aggregate struct>` — every row (including `id` itself) sharing `id`'s `event_batch_id`, each fully hydrated via `fetch` (same shape `create`/`update` already return). Returns `[]` if `id` doesn't exist.

- [ ] **Step 1: Write the failing test** (append to `schedule_event_repo_spec.rb`; mirror whatever fixture-creation pattern the existing specs in that file already use)

```ruby
  describe "#batch_events" do
    it "returns every row created with the caller's event_batch_id" do
      first  = repo.create(load_requirement_id: lr1.id, time_slot_id: slot.id, room_id: room.id, week_type_id: week_type.id, day_of_week: 1, status: "scheduled", event_batch_id: "11111111-1111-1111-1111-111111111111")
      second = repo.create(load_requirement_id: lr2.id, time_slot_id: slot.id, room_id: room.id, week_type_id: week_type.id, day_of_week: 1, status: "scheduled", event_batch_id: "11111111-1111-1111-1111-111111111111")
      unrelated = repo.create(load_requirement_id: lr3.id, time_slot_id: slot.id, room_id: room.id, week_type_id: week_type.id, day_of_week: 2, status: "scheduled", event_batch_id: "22222222-2222-2222-2222-222222222222")

      result_ids = repo.batch_events(first.id).map(&:id)

      expect(result_ids).to contain_exactly(first.id, second.id)
      expect(result_ids).not_to include(unrelated.id)
    end

    it "returns an empty array for an id that doesn't exist" do
      expect(repo.batch_events(-1)).to eq([])
    end
  end
```

(Substitute this repo spec file's actual fixture-building helpers for `lr1`/`lr2`/`lr3`/`slot`/`room`/`week_type` — read the top of the existing spec file first; do not invent new factory calls.)

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd apps/api_geteway && bundle exec rspec spec/slices/admin/repos/schedule_event_repo_spec.rb -e "batch_events"`
Expected: FAIL — `NoMethodError: undefined method 'batch_events'`.

- [ ] **Step 3: Implement it**

Add to `ScheduleEventRepo`, near `load_one` (around line 75):

```ruby
      # Every row sharing id's event_batch_id, fully hydrated (fetch, not load_one) —
      # a stream lecture's siblings across groups, or just [id] for a lone class.
      # Empty array when id doesn't exist.
      def batch_events(id)
        target = load_one(id)
        return [] unless target

        ids = schedule_events.dataset.naked.where(event_batch_id: target[:event_batch_id]).select_map(:id)
        ids.map { |i| fetch(i) }.compact
      end
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd apps/api_geteway && bundle exec rspec spec/slices/admin/repos/schedule_event_repo_spec.rb`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
cd apps/api_geteway
git add slices/admin/repos/schedule_event_repo.rb spec/slices/admin/repos/schedule_event_repo_spec.rb
git commit -m "feat(schedule_event_repo): add batch_events to look up stream siblings"
```

---

### Task 7: `Destroy` — cascade to the whole batch

**Files:**
- Modify: `apps/api_geteway/slices/admin/actions/schedule_events/destroy.rb`
- Test: `apps/api_geteway/spec/slices/admin/actions/schedule_events/destroy_spec.rb` (create if none exists — check first)

**Interfaces:**
- Consumes: `repo.batch_events(id)` (Task 6).
- Produces: `DELETE /admin/schedule_events/:id` deletes every row in the same batch, notifying once per deleted row's own `load_requirement_id`/semester (a batch can theoretically span groups in different semesters, so don't collapse to one notification). Still 204 on success, 404 if the id doesn't exist. Response/audit behavior for a batch of one (the common case) is unchanged.

- [ ] **Step 1: Write the failing tests**

If `spec/slices/admin/actions/schedule_events/destroy_spec.rb` doesn't exist, create it using the same double-based style as `create_schedule_event_for_stream_spec.rb`:

```ruby
# frozen_string_literal: true

RSpec.describe Admin::Actions::ScheduleEvents::Destroy do
  subject(:action) { described_class.new(repo: repo, notify: notify, validation_contract: Admin::Contracts::ScheduleEvents::DestroyContract.new) }

  let(:repo)   { double("repo") }
  let(:notify) { double("notify") }

  def event_double(id, load_requirement_id:)
    double("event", id: id, load_requirement_id: load_requirement_id, to_h: { id: id })
  end

  before do
    allow(repo).to receive(:transaction) { |&b| b.call }
    allow(notify).to receive(:semester_id_for).and_return(3)
    allow(notify).to receive(:enqueue_for)
  end

  it "deletes every row in the batch, not just the requested id" do
    a = event_double(1, load_requirement_id: 10)
    b = event_double(2, load_requirement_id: 11)
    allow(repo).to receive(:batch_events).with(1).and_return([a, b])
    expect(repo).to receive(:delete).with(1)
    expect(repo).to receive(:delete).with(2)

    request  = double("request", params: { id: "1" })
    response = double("response", status: nil, body: nil)
    allow(request.params).to receive(:to_h).and_return(id: "1")
    allow(response).to receive(:status=)
    action.handle(request, response)
  end

  it "notifies once per deleted row's own load_requirement_id" do
    a = event_double(1, load_requirement_id: 10)
    b = event_double(2, load_requirement_id: 11)
    allow(repo).to receive(:batch_events).with(1).and_return([a, b])
    allow(repo).to receive(:delete)
    expect(notify).to receive(:enqueue_for).with(hash_including(load_requirement_id: 10))
    expect(notify).to receive(:enqueue_for).with(hash_including(load_requirement_id: 11))

    request  = double("request", params: { id: "1" })
    response = double("response", status: nil, body: nil)
    allow(request.params).to receive(:to_h).and_return(id: "1")
    allow(response).to receive(:status=)
    action.handle(request, response)
  end

  it "404s when the batch is empty (id doesn't exist)" do
    allow(repo).to receive(:batch_events).with(999).and_return([])
    request  = double("request", params: { id: "999" })
    response = double("response")
    allow(request.params).to receive(:to_h).and_return(id: "999")
    expect(response).to receive(:status=).with(404)
    expect(response).to receive(:body=)
    action.handle(request, response)
  end
end
```

Adjust these doubles once you see how `validate_or_render`/`Admin::Action` actually shapes `request`/`response` in this codebase's other action specs (search for an existing spec of a similarly-structured action, e.g. one testing `Update` or another `Destroy`, and match its request/response double style exactly rather than guessing).

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd apps/api_geteway && bundle exec rspec spec/slices/admin/actions/schedule_events/destroy_spec.rb`
Expected: FAIL — current implementation deletes only `id`, never calls `batch_events`.

- [ ] **Step 3: Rewrite the action**

Replace the body of `destroy.rb`'s `handle` method:

```ruby
        def handle(request, response)
          return unless validate_or_render(validation_contract, request.params.to_h, response)

          id    = request.params[:id].to_i
          batch = repo.batch_events(id)

          if batch.empty?
            response.status = 404
            response.body = { error: "Schedule event not found" }.to_json
            return
          end

          deleted_primary = repo.transaction do
            batch.each do |event|
              lr_id = event.respond_to?(:load_requirement_id) ? event.load_requirement_id : event[:load_requirement_id]
              repo.delete(event.id)
              next unless lr_id

              semester_id = notify.semester_id_for(lr_id)
              notify.enqueue_for(load_requirement_id: lr_id, semester_id: semester_id, change: :destroyed) if semester_id
            end
            batch.find { |event| event.id == id }
          end

          Thread.current[Admin::Actions::Concerns::Authorization::AUDIT_BEFORE_DATA_KEY] = deleted_primary.to_h

          response.status = 204
        end
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd apps/api_geteway && bundle exec rspec spec/slices/admin/actions/schedule_events/destroy_spec.rb`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
cd apps/api_geteway
git add slices/admin/actions/schedule_events/destroy.rb spec/slices/admin/actions/schedule_events/destroy_spec.rb
git commit -m "feat(schedule_events): deleting one class deletes its whole stream batch"
```

---

### Task 8: New `GET /schedule_events/:id/batch`

**Files:**
- New: `apps/api_geteway/slices/admin/actions/schedule_events/batch.rb`
- Modify: `apps/api_geteway/config/routes.rb:129` (insert after the `transfers_index` line)
- Test: `apps/api_geteway/spec/slices/admin/actions/schedule_events/batch_spec.rb`

**Interfaces:**
- Consumes: `repo.batch_events(id)` (Task 6).
- Produces: `GET /admin/schedule_events/:id/batch` → `200` with the presented list of every sibling (same JSON:API shape as `GET /schedule_events` — reuses `ScheduleEventPresenter`), including `id` itself. The frontend (Task 15) uses this to list affected groups in the delete confirmation.

- [ ] **Step 1: Write the failing test**

```ruby
# frozen_string_literal: true

RSpec.describe Admin::Actions::ScheduleEvents::Batch do
  subject(:action) { described_class.new(repo: repo, presenter: presenter) }

  let(:repo)      { double("repo") }
  let(:presenter) { double("presenter") }

  it "presents whatever batch_events returns for the requested id" do
    events = [double("event", id: 1), double("event", id: 2)]
    allow(repo).to receive(:batch_events).with(1).and_return(events)
    expect(presenter).to receive(:call).with(events, context: anything).and_return('{"data":[]}')

    request  = double("request", params: { id: "1" })
    response = double("response")
    allow(response).to receive(:status=)
    allow(response).to receive(:body=)
    action.handle(request, response)
  end
end
```

Match this to whichever `present(request, response, presenter, ...)` helper the other minimal GET actions (e.g. `TransfersIndex`) actually call under the hood — if that spec-writing pattern doesn't fit `present`'s real signature, base the double expectations on `TransfersIndex`'s own spec file instead of this sketch.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd apps/api_geteway && bundle exec rspec spec/slices/admin/actions/schedule_events/batch_spec.rb`
Expected: FAIL — the file doesn't exist yet.

- [ ] **Step 3: Implement the action**

```ruby
# frozen_string_literal: true

module Admin
  module Actions
    module ScheduleEvents
      # GET /admin/schedule_events/:id/batch
      # Every schedule_events row created together with :id (every group in a
      # shared "поточное" lecture, or just [id] for a lone class) — used by the
      # frontend to show which other groups a delete/edit will also affect.
      class Batch < Admin::Action
        include Deps[
          repo:      "repos.schedule_event_repo",
          presenter: "presenters.schedule_event_presenter"
        ]

        def handle(request, response)
          events = repo.batch_events(request.params[:id].to_i)
          present(request, response, presenter, events)
        end
      end
    end
  end
end
```

- [ ] **Step 4: Register the route**

In `config/routes.rb`, after line 129 (`get  "/schedule_events/:id/transfers", to: "schedule_events.transfers_index"`), add:
```ruby
      get  "/schedule_events/:id/batch",      to: "schedule_events.batch"
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `cd apps/api_geteway && bundle exec rspec spec/slices/admin/actions/schedule_events/batch_spec.rb`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
cd apps/api_geteway
git add slices/admin/actions/schedule_events/batch.rb config/routes.rb spec/slices/admin/actions/schedule_events/batch_spec.rb
git commit -m "feat(schedule_events): add GET /schedule_events/:id/batch"
```

---

### Task 9: Backend live smoke test (manual, no code changes)

**Files:** none — verification only.

- [ ] **Step 1: Rebuild and restart the API container**

Run: `cd /c/Users/Asus/Desktop/milk_schedule && docker compose build api_geteway && docker compose up -d --force-recreate api_geteway`

- [ ] **Step 2: Verify roomless creation, batch linking, and cascade delete against the live API**

Using the seed admin credentials (`admin@milk-schedule.local` / from `SEED_ADMIN_PASSWORD` or the seed default — check `config/db/seeds.rb`), log in and:
1. `POST /admin/schedule_events/from_recommendation` with `load_requirement_ids: [X, Y]` (two groups sharing a stream) and `room_id` omitted → expect `201`, two events created, both roomless.
2. `GET /admin/schedule_events/:id/batch` for one of the two ids → expect both ids back.
3. `DELETE /admin/schedule_events/:id` for one of the two → expect `204`, then `GET` the other id → expect `404` (cascaded).
4. Repeat creation with `room_id` set to a room already occupied by an unrelated event at the same slot → still expect the existing TOCTOU `409`, confirming the room-conflict guard still fires when a room IS given.

Expected: all four checks match. If any fails, stop and re-examine Tasks 1-8 before moving to the frontend — this is the checkpoint the frontend work depends on.

---

### Task 10: `loadPanel.ts` — extract `makeSiblingResolver`

**Files:**
- Modify: `apps/frontend/src/features/admin/schedule/loadPanel.ts`
- Modify: `apps/frontend/src/features/admin/AdminSchedule.tsx` (replace its inline `resolveSiblings` with the shared helper)
- Test: `apps/frontend/src/features/admin/schedule/loadPanel.test.ts` (new)

**Interfaces:**
- Produces: `makeSiblingResolver(cache: Record<string, LoadEntry[]>, fetchGroupLoad: (groupSemesterId: string) => Promise<LoadEntry[]>) => (entry: LoadEntry) => Promise<{ gsids: number[]; requirementIds: number[] }>` — same behavior `AdminSchedule.tsx`'s `resolveSiblings` already has (cache-first per-sibling-group fetch, matched via `matchSiblingRequirement`), just reusable. `AdminOperations.tsx` (Task 17) will use the same helper.

- [ ] **Step 1: Write the failing test**

```ts
import { describe, expect, it, vi } from "vitest";

import { makeSiblingResolver, type LoadEntry } from "./loadPanel";

function entry(overrides: Partial<LoadEntry> = {}): LoadEntry {
  return {
    id: 1, groupSemesterId: "10", subjectName: "S", subjectShort: "S", subjectId: 5,
    activityName: "A", activityShort: "A", activityTypeId: 7, kafedraId: 2,
    hoursPerEvent: 2, hoursAssigned: 0, actualHours: 0, controlSum: 10,
    weekBreakdown: { both: 0, even: 0, odd: 0 }, teachers: [], reservedRoom: null,
    streamSiblingGsids: [20],
    ...overrides,
  };
}

describe("makeSiblingResolver", () => {
  it("fetches and caches each sibling group's load once, matching by the stream tuple", async () => {
    const cache: Record<string, LoadEntry[]> = {};
    const siblingEntry = entry({ id: 99, groupSemesterId: "20" });
    const fetchGroupLoad = vi.fn().mockResolvedValue([siblingEntry]);
    const resolve = makeSiblingResolver(cache, fetchGroupLoad);

    const result = await resolve(entry());
    expect(result).toEqual({ gsids: [20], requirementIds: [99] });
    expect(fetchGroupLoad).toHaveBeenCalledTimes(1);

    await resolve(entry());
    expect(fetchGroupLoad).toHaveBeenCalledTimes(1); // cached on the second call
  });

  it("skips a sibling group whose load has no matching requirement", async () => {
    const cache: Record<string, LoadEntry[]> = {};
    const fetchGroupLoad = vi.fn().mockResolvedValue([]);
    const resolve = makeSiblingResolver(cache, fetchGroupLoad);

    expect(await resolve(entry())).toEqual({ gsids: [], requirementIds: [] });
  });

  it("returns empty when the entry has no stream siblings", async () => {
    const cache: Record<string, LoadEntry[]> = {};
    const fetchGroupLoad = vi.fn();
    const resolve = makeSiblingResolver(cache, fetchGroupLoad);

    expect(await resolve(entry({ streamSiblingGsids: [] }))).toEqual({ gsids: [], requirementIds: [] });
    expect(fetchGroupLoad).not.toHaveBeenCalled();
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd apps/frontend && npx vitest run src/features/admin/schedule/loadPanel.test.ts`
Expected: FAIL — `makeSiblingResolver` doesn't exist yet.

- [ ] **Step 3: Add the export to `loadPanel.ts`**

Add after `matchSiblingRequirement`:

```ts
// Shared by every page that can edit/create a stream class (AdminSchedule.tsx's
// builder grid, AdminOperations.tsx's day grid): resolves which OTHER groups
// share `entry`'s stream, and which of THEIR load requirements to include when
// building a create/edit payload. `cache` is a plain object the caller owns
// (typically a useRef) so repeated calls within one page session don't
// re-fetch a sibling group's whole workload every time.
export function makeSiblingResolver(
  cache: Record<string, LoadEntry[]>,
  fetchGroupLoad: (groupSemesterId: string) => Promise<LoadEntry[]>
) {
  return async function resolveSiblings(entry: LoadEntry): Promise<{ gsids: number[]; requirementIds: number[] }> {
    const gsids: number[] = [];
    const requirementIds: number[] = [];
    for (const siblingGsid of entry.streamSiblingGsids) {
      const key = String(siblingGsid);
      let entries = cache[key];
      if (!entries) {
        entries = await fetchGroupLoad(key).catch(() => []);
        cache[key] = entries;
      }
      const match = matchSiblingRequirement(entries, entry);
      if (match) { gsids.push(siblingGsid); requirementIds.push(match.id); }
    }
    return { gsids, requirementIds };
  };
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd apps/frontend && npx vitest run src/features/admin/schedule/loadPanel.test.ts`
Expected: PASS

- [ ] **Step 5: Use it from `AdminSchedule.tsx`**

In `AdminSchedule.tsx`, change the import on line 18 from:
```ts
import { normalizeLoadEntries, matchSiblingRequirement, type LoadEntry } from "./schedule/loadPanel";
```
to:
```ts
import { normalizeLoadEntries, makeSiblingResolver, type LoadEntry } from "./schedule/loadPanel";
```

Replace the `resolveSiblings` definition (lines 135-149):
```ts
  const resolveSiblings = useCallback(async (entry: LoadEntry) => {
    const gsids: number[] = [];
    const requirementIds: number[] = [];
    for (const siblingGsid of entry.streamSiblingGsids) {
      const key = String(siblingGsid);
      let entries = loadCache.current[key];
      if (!entries) {
        entries = normalizeLoadEntries(await api.groupWorkload(key).catch(() => null), key);
        loadCache.current[key] = entries;
      }
      const match = matchSiblingRequirement(entries, entry);
      if (match) { gsids.push(siblingGsid); requirementIds.push(match.id); }
    }
    return { gsids, requirementIds };
  }, []);
```
with:
```ts
  const resolveSiblings = useCallback(
    makeSiblingResolver(loadCache.current, (id) => api.groupWorkload(id).then((response) => normalizeLoadEntries(response, id))),
    []
  );
```

- [ ] **Step 6: Run the existing builder grid tests to confirm nothing broke**

Run: `cd apps/frontend && npx vitest run src/features/admin/schedule/builderGrid.test.tsx`
Expected: PASS (this refactor doesn't touch `BuilderGrid.tsx` itself, but confirms the page still imports/compiles cleanly).

Run: `cd apps/frontend && npx tsc --noEmit`
Expected: no new type errors.

- [ ] **Step 7: Commit**

```bash
cd apps/frontend
git add src/features/admin/schedule/loadPanel.ts src/features/admin/schedule/loadPanel.test.ts src/features/admin/AdminSchedule.tsx
git commit -m "refactor(schedule): extract makeSiblingResolver for reuse across pages"
```

---

### Task 11: `DayEvent` and `BuilderEvent` — carry `loadRequirementId` and `groupSemesterId`

**Files:**
- Modify: `apps/frontend/src/features/admin/schedule/DayScheduleGrid.tsx`
- Modify: `apps/frontend/src/features/admin/schedule/BuilderGrid.tsx`
- Test: `apps/frontend/src/features/admin/schedule/dayGrid.test.ts` (extend existing)
- Test: `apps/frontend/src/features/admin/schedule/builderGrid.test.tsx` (extend existing)

**Interfaces:**
- Produces: `DayEvent` gains `loadRequirementId: number | null` and `groupSemesterId: string`. `BuilderEvent` gains `loadRequirementId: number | null`, `groupSemesterId: string`, `subjectId: number | null`, `activityTypeId: number | null`, `kafedraId: number | null`, `weekTypeId: number | null`, and a structured `teacherList: DayEventTeacher[]` (the existing flattened `teachers: string` field for card display stays, so `BuilderGrid`'s own rendering is untouched). This is what Task 12/13 (`OccurrenceEditor` reuse, `AdminSchedule.tsx`/`AdminOperations.tsx` wiring) build on — both now expose the same identifiers the day grid already does, sourced from `attrs.load_requirement_id`/`attrs.group_semester_id`/etc., which Task 4 confirmed the backend now serializes on every grid row.

- [ ] **Step 1: Write the failing tests**

Read the existing `dayGrid.test.ts` and `builderGrid.test.tsx` first to match their exact fixture-row shape, then extend each with a case like:

`dayGrid.test.ts` addition:
```ts
it("parses load_requirement_id and group_semester_id", () => {
  const [event] = normalizeDayEvents({
    data: [{ id: "1", attributes: { time_slot_id: "5", load_requirement_id: 42, group_semester_id: "77" } }],
  });
  expect(event.loadRequirementId).toBe(42);
  expect(event.groupSemesterId).toBe("77");
});
```

`builderGrid.test.tsx` addition (mirror however that file currently builds its `source` fixture for `normalizeBuilderEvents`):
```ts
it("parses load_requirement_id, group_semester_id, and structured teacher/context ids", () => {
  const [event] = normalizeBuilderEvents({
    data: [{ id: "1", attributes: {
      time_slot_id: "5", load_requirement_id: 42, group_semester_id: "77",
      subject_id: 3, activity_type_id: 9, kafedra_id: 11, week_type_id: 6,
      teachers: [{ event_teacher_id: 1, teacher_on_kafedra_id: 2, last_name: "Иванов" }],
    } }]
  }, new Map());
  expect(event.loadRequirementId).toBe(42);
  expect(event.groupSemesterId).toBe("77");
  expect(event.subjectId).toBe(3);
  expect(event.activityTypeId).toBe(9);
  expect(event.kafedraId).toBe(11);
  expect(event.weekTypeId).toBe(6);
  expect(event.teacherList).toEqual([{ eventTeacherId: 1, teacherOnKafedraId: 2, name: "Иванов" }]);
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd apps/frontend && npx vitest run src/features/admin/schedule/dayGrid.test.ts src/features/admin/schedule/builderGrid.test.tsx`
Expected: FAIL — the new fields are `undefined`.

- [ ] **Step 3: Extend `DayEvent` and `normalizeDayEvents`**

In `DayScheduleGrid.tsx`, add to the `DayEvent` type (after `dayOfWeek?: number | null;`):
```ts
  // Which study_load_requirement this occurrence's series is scheduled against,
  // and which group-semester it belongs to — both needed to drive the unified
  // edit form (OccurrenceEditor) without the old subject+activity fuzzy match.
  loadRequirementId?: number | null;
  groupSemesterId?: string;
```

In `normalizeDayEvents`, add to the returned object (after `dayOfWeek: numberOrNull(attrs.day_of_week),`):
```ts
      loadRequirementId: numberOrNull(attrs.load_requirement_id),
      groupSemesterId: String(attrs.group_semester_id ?? ""),
```

- [ ] **Step 4: Extend `BuilderEvent` and `normalizeBuilderEvents`**

In `BuilderGrid.tsx`, add to the `BuilderEvent` type (after `additionalRooms?: OccurrenceRoom[];`):
```ts
  // Same identifiers DayEvent carries, needed so the builder grid's cell click
  // can drive the same edit form as the day grid instead of a thinner one.
  loadRequirementId: number | null;
  groupSemesterId: string;
  subjectId: number | null;
  activityTypeId: number | null;
  kafedraId: number | null;
  weekTypeId: number | null;
  teacherList: { eventTeacherId: number | null; teacherOnKafedraId: number | null; name: string }[];
```

In `normalizeBuilderEvents`, add to the returned object (after `additionalRooms: ...`):
```ts
      loadRequirementId: numberOrNull(attrs.load_requirement_id),
      groupSemesterId: String(attrs.group_semester_id ?? ""),
      subjectId: numberOrNull(attrs.subject_id),
      activityTypeId: numberOrNull(attrs.activity_type_id),
      kafedraId: numberOrNull(attrs.kafedra_id),
      weekTypeId: numberOrNull(attrs.week_type_id),
      teacherList: teachers.map((teacher) => ({
        eventTeacherId: Number(teacher.event_teacher_id) || null,
        teacherOnKafedraId: Number(teacher.teacher_on_kafedra_id) || null,
        name: [teacher.last_name, teacher.first_name].filter(Boolean).join(" ") || "Преподаватель",
      })),
```
This needs a `numberOrNull` helper in `BuilderGrid.tsx` (it doesn't have one today — `DayScheduleGrid.tsx` does). Add it next to the existing `text`/`initial` helpers at the bottom of `BuilderGrid.tsx`:
```ts
function numberOrNull(value: unknown): number | null { const n = Number(value); return Number.isFinite(n) && value !== null && value !== undefined && value !== "" ? n : null; }
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cd apps/frontend && npx vitest run src/features/admin/schedule/dayGrid.test.ts src/features/admin/schedule/builderGrid.test.tsx`
Expected: PASS

Run: `cd apps/frontend && npx tsc --noEmit`
Expected: no new errors (the two new optional/required fields don't break existing constructors of `DayEvent`/`BuilderEvent` literals — check `resolveDay`'s spread usages in `DayScheduleGrid.tsx` still type-check, since it spreads an existing `DayEvent` rather than building one from scratch, so this should be a non-issue).

- [ ] **Step 6: Commit**

```bash
cd apps/frontend
git add src/features/admin/schedule/DayScheduleGrid.tsx src/features/admin/schedule/BuilderGrid.tsx src/features/admin/schedule/dayGrid.test.ts src/features/admin/schedule/builderGrid.test.tsx
git commit -m "feat(schedule): expose load_requirement_id/group_semester_id on grid events"
```

---

### Task 12: `api.ts` — batch endpoint, nullable `room_id`

**Files:**
- Modify: `apps/frontend/src/lib/api.ts`
- Test: `apps/frontend/src/lib/api.test.ts` (extend existing)

**Interfaces:**
- Produces: `api.scheduleEventBatch(id: string | number)` → `GET /admin/schedule_events/:id/batch`. `updateScheduleEvent`'s and `createScheduleEvent`'s `room_id` types widen to `number | null` (matching the now-nullable DB column; `createEventFromRecommendation` already takes an untyped `Record<string, unknown>` so it needs no type change to carry `room_id: null`).

- [ ] **Step 1: Write the failing test** (append to `api.test.ts`, mirroring however the existing tests in that file assert on `fetch` calls — read the file first and match its mocking style exactly)

```ts
it("scheduleEventBatch requests GET /admin/schedule_events/:id/batch", async () => {
  await api.scheduleEventBatch("7");
  // assert the mocked fetch was called with the expected URL/method, matching
  // this file's existing assertion style for a similar GET call (e.g. listEventRooms's test).
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd apps/frontend && npx vitest run src/lib/api.test.ts`
Expected: FAIL — `api.scheduleEventBatch` doesn't exist.

- [ ] **Step 3: Add the method and widen the two types**

In `api.ts`, add right after `deleteScheduleEvent` (line 467-469):
```ts
  scheduleEventBatch(id: string | number) {
    return request<JsonApiCollection>(`/admin/schedule_events/${encodeURIComponent(String(id))}/batch`);
  },
```

Change line 464 from:
```ts
  updateScheduleEvent(id: string, scheduleEvent: { day_of_week?: number; time_slot_id?: number; room_id?: number; week_type_id?: number; subgroup_number?: number; load_requirement_id?: number; status?: "scheduled" | "cancelled" | "modified"; is_manual_change?: boolean; change_reason?: string }) {
```
to:
```ts
  updateScheduleEvent(id: string, scheduleEvent: { day_of_week?: number; time_slot_id?: number; room_id?: number | null; week_type_id?: number; subgroup_number?: number; load_requirement_id?: number; status?: "scheduled" | "cancelled" | "modified"; is_manual_change?: boolean; change_reason?: string }) {
```

Change line 473 from:
```ts
  createScheduleEvent(scheduleEvent: { load_requirement_id: number; time_slot_id: number; room_id: number; week_type_id: number; status: "scheduled" | "cancelled" | "modified"; day_of_week: number; teacher_on_kafedra_ids?: number[] }) {
```
to:
```ts
  createScheduleEvent(scheduleEvent: { load_requirement_id: number; time_slot_id: number; room_id: number | null; week_type_id: number; status: "scheduled" | "cancelled" | "modified"; day_of_week: number; teacher_on_kafedra_ids?: number[] }) {
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd apps/frontend && npx vitest run src/lib/api.test.ts`
Expected: PASS

Run: `cd apps/frontend && npx tsc --noEmit`
Expected: clean.

- [ ] **Step 5: Commit**

```bash
cd apps/frontend
git add src/lib/api.ts src/lib/api.test.ts
git commit -m "feat(api): add scheduleEventBatch, allow null room_id in event payloads"
```

---

### Task 13: `SlotEditor.tsx` — optional room, subgroup forced to 0 for stream siblings, export shared helpers

**Files:**
- Modify: `apps/frontend/src/features/admin/schedule/SlotEditor.tsx`
- Test: `apps/frontend/src/features/admin/schedule/slotEditor.test.ts` (extend existing)

**Interfaces:**
- Produces: submitting the manual-creation form no longer requires a room (`room_id: null` is sent when none is picked); the subgroup selector is hidden/forced to `0` once a load entry with `streamSiblingGsids.length > 0` is assigned. `attachExtras`, `extractCreatedEventId`, and `readableCreateError` become exported (`OccurrenceEditor`, Task 14, reuses them instead of duplicating the attach-extras/error-message logic).

- [ ] **Step 1: Write the failing tests** (append to `slotEditor.test.ts`)

```ts
import { readableCreateError } from "./SlotEditor";

describe("readableCreateError", () => {
  it("gives a stream-aware message for a 409", () => {
    expect(readableCreateError(new Error("409 slot_taken"))).toContain("одной из групп потока");
  });

  it("falls back to the raw message otherwise", () => {
    expect(readableCreateError(new Error("boom"))).toBe("boom");
  });
});
```

(`attachExtras`/`extractCreatedEventId` are exercised indirectly through `OccurrenceEditor`'s own tests in Task 14 rather than re-tested here — this task's test only needs to confirm the export compiles and the pure error-formatting behavior is unchanged.)

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd apps/frontend && npx vitest run src/features/admin/schedule/slotEditor.test.ts`
Expected: FAIL — `readableCreateError` isn't exported yet (TS/import error).

- [ ] **Step 3: Export the three helpers**

In `SlotEditor.tsx`, change:
```ts
async function attachExtras(response: Record<string, unknown>, loadRequirementId: number, extras: {
```
to:
```ts
export async function attachExtras(response: Record<string, unknown>, loadRequirementId: number, extras: {
```

Change:
```ts
function extractCreatedEventId(response: Record<string, unknown>, loadRequirementId: number): number | null {
```
to:
```ts
export function extractCreatedEventId(response: Record<string, unknown>, loadRequirementId: number): number | null {
```

Change:
```ts
function readableCreateError(caught: unknown) {
```
to:
```ts
export function readableCreateError(caught: unknown) {
```

- [ ] **Step 4: Make the room field optional**

Change the room `<label>` block from:
```tsx
        <label><span>Аудитория{roomLocked ? " (закреплена)" : ""}</span>
          <select required value={roomId} onChange={(event) => setRoomId(event.target.value)} disabled={roomLocked || !assignedEntry}>
            {roomLocked ? <option value={String(assignedEntry!.reservedRoom!.id)}>{assignedEntry!.reservedRoom!.name}</option> : <><option value="">Выберите аудиторию</option>{rooms.map((room) => <option value={room.id} key={room.id}>{room.label}</option>)}</>}
          </select>
        </label>
```
to:
```tsx
        <label><span>Аудитория{roomLocked ? " (закреплена)" : ""}</span>
          <select value={roomId} onChange={(event) => setRoomId(event.target.value)} disabled={roomLocked || !assignedEntry}>
            {roomLocked ? <option value={String(assignedEntry!.reservedRoom!.id)}>{assignedEntry!.reservedRoom!.name}</option> : <><option value="">Без аудитории</option>{rooms.map((room) => <option value={room.id} key={room.id}>{room.label}</option>)}</>}
          </select>
        </label>
```

Change the submit payload's `room_id` from:
```ts
        room_id: Number(roomId),
```
to:
```ts
        room_id: roomId ? Number(roomId) : null,
```

Change the submit button's `disabled` condition from:
```tsx
        <Button className="button-primary" type="submit" disabled={saving || !assignedEntry || !roomId || dateMissing || weekTypeMissing}>{saving ? "Создание…" : "Создать занятие"}</Button>
```
to:
```tsx
        <Button className="button-primary" type="submit" disabled={saving || !assignedEntry || dateMissing || weekTypeMissing}>{saving ? "Создание…" : "Создать занятие"}</Button>
```

- [ ] **Step 5: Force subgroup to 0 for stream siblings**

Change the subgroup `<label>` block from:
```tsx
        <label><span>Подгруппа</span><select value={subgroup} onChange={(event) => setSubgroup(Number(event.target.value))}><option value={0}>Вся группа</option><option value={1}>Подгруппа 1</option><option value={2}>Подгруппа 2</option></select></label>
```
to:
```tsx
        {assignedEntry && assignedEntry.streamSiblingGsids.length > 0
          ? <p className="inspector-note"><strong>Подгруппа: вся группа</strong><span>Поточное занятие охватывает несколько групп целиком — деление на подгруппы для него недоступно.</span></p>
          : <label><span>Подгруппа</span><select value={subgroup} onChange={(event) => setSubgroup(Number(event.target.value))}><option value={0}>Вся группа</option><option value={1}>Подгруппа 1</option><option value={2}>Подгруппа 2</option></select></label>}
```

And in `submitManual`, change the payload's `subgroup_number` from:
```ts
        subgroup_number: subgroup,
```
to:
```ts
        subgroup_number: assignedEntry.streamSiblingGsids.length > 0 ? 0 : subgroup,
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cd apps/frontend && npx vitest run src/features/admin/schedule/slotEditor.test.ts`
Expected: PASS

Run: `cd apps/frontend && npx tsc --noEmit`
Expected: clean.

- [ ] **Step 7: Commit**

```bash
cd apps/frontend
git add src/features/admin/schedule/SlotEditor.tsx src/features/admin/schedule/slotEditor.test.ts
git commit -m "feat(slot-editor): allow creating without a room, lock subgroup for stream classes"
```

---

### Task 14: `OccurrenceEditor.tsx` — unified load-based edit form, delete+recreate save, date-optional

**Files:**
- Modify: `apps/frontend/src/features/admin/schedule/OccurrenceEditor.tsx`
- Test: `apps/frontend/src/features/admin/schedule/occurrenceEditor.test.ts` (new — pure-function-level, mirroring `slotEditor.test.ts`'s scope; this task does not add a full component-render test suite, matching the codebase's existing convention of testing this file's siblings at the pure-function level)

This is the largest task. Read the CURRENT full contents of `OccurrenceEditor.tsx` and `SlotEditor.tsx` again immediately before starting (both were read in full during planning; nothing else in the repo touches either file between now and execution, but re-read anyway before editing — don't work from memory of this plan's quoted snippets alone).

**Interfaces:**
- Consumes: `SlotEditor`'s exported `attachExtras`/`extractCreatedEventId`/`readableCreateError` (Task 13); `LoadEntry`/`makeSiblingResolver` (Task 10); `DayEvent.loadRequirementId`/`.groupSemesterId` (Task 11); `api.scheduleEventBatch`/`api.deleteScheduleEvent`/`api.createEventFromRecommendation` (Tasks 8, 12, existing).
- Produces: new required props `groupSemesterId: string`, `groupLabel: string`, `resolveSiblings: (entry: LoadEntry) => Promise<{ gsids: number[]; requirementIds: number[] }>`. `date` may now be `""` (builder-grid context) — the `<div className="transfer-summary">` date line and the entire "Эта дата" tab (and its tab bar) are only rendered when `date` is non-empty. The "Параметры занятия" tab's save button now performs delete-the-batch-then-recreate instead of a partial PATCH, and can change subject/load, teacher(s), and room(s) — not just day/slot/week type/subgroup.

- [ ] **Step 1: Write the failing tests**

Create `occurrenceEditor.test.ts` covering the two pure helper functions this task extracts (see Step 3) — write these FIRST, confirm they fail, then implement:

```ts
import { describe, expect, it } from "vitest";

import { buildEditRecommendation } from "./OccurrenceEditor";
import type { LoadEntry } from "./loadPanel";

function entry(overrides: Partial<LoadEntry> = {}): LoadEntry {
  return {
    id: 42, groupSemesterId: "10", subjectName: "S", subjectShort: "S", subjectId: 5,
    activityName: "A", activityShort: "A", activityTypeId: 7, kafedraId: 2,
    hoursPerEvent: 2, hoursAssigned: 0, actualHours: 0, controlSum: 10,
    weekBreakdown: { both: 0, even: 0, odd: 0 }, teachers: [], reservedRoom: null,
    streamSiblingGsids: [],
    ...overrides,
  };
}

describe("buildEditRecommendation", () => {
  it("builds a from_recommendation payload with room_id null when none is picked", () => {
    const payload = buildEditRecommendation({
      assignedEntry: entry(), groupSemesterId: "10", dayOfWeek: "2", timeSlotId: "5",
      callNumber: 3, weekType: { id: 9, code: "both", name: "Каждую неделю" },
      roomId: "", teacherTokId: "", subgroup: 0, siblings: { gsids: [], requirementIds: [] },
    });
    expect(payload.room_id).toBeNull();
    expect(payload.load_requirement_ids).toEqual([42]);
    expect(payload.group_semester_ids).toEqual([10]);
    expect(payload.subgroup_number).toBe(0);
  });

  it("preserves a non-stream entry's chosen subgroup", () => {
    const payload = buildEditRecommendation({
      assignedEntry: entry(), groupSemesterId: "10", dayOfWeek: "2", timeSlotId: "5",
      callNumber: 3, weekType: { id: 9, code: "both", name: "Каждую неделю" },
      roomId: "", teacherTokId: "", subgroup: 2, siblings: { gsids: [], requirementIds: [] },
    });
    expect(payload.subgroup_number).toBe(2);
  });

  it("forces subgroup_number to 0 and merges siblings when the entry is a stream", () => {
    const payload = buildEditRecommendation({
      assignedEntry: entry({ streamSiblingGsids: [20] }), groupSemesterId: "10",
      dayOfWeek: "2", timeSlotId: "5", callNumber: 3,
      weekType: { id: 9, code: "both", name: "Каждую неделю" },
      roomId: "3", teacherTokId: "7", subgroup: 1,
      siblings: { gsids: [20], requirementIds: [99] },
    });
    expect(payload.subgroup_number).toBe(0);
    expect(payload.load_requirement_ids).toEqual([42, 99]);
    expect(payload.group_semester_ids).toEqual([10, 20]);
    expect(payload.room_id).toBe(3);
    expect(payload.teacher_on_kafedra_ids).toEqual([7]);
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd apps/frontend && npx vitest run src/features/admin/schedule/occurrenceEditor.test.ts`
Expected: FAIL — `buildEditRecommendation` doesn't exist.

- [ ] **Step 3: Rewrite `OccurrenceEditor.tsx`**

Replace the file's contents. The occurrence-level tab (cancel/undo transfer), the top summary line, and the overall `Inspector` wrapper stay conceptually the same; everything under "Параметры занятия" is rebuilt around `SlotEditor`'s manual-form pattern (load drop-zone/pick-list, teacher lock-to-load-entry with kafedra fallback, extra teacher/room queues, room optional) and the save handler becomes delete+recreate:

```tsx
import { useEffect, useMemo, useState } from "react";

import { Button } from "../../../components/ui";
import { api, PERIOD_LABELS, type EventRoom, type PeriodKind } from "../../../lib/api";
import type { EntityOption } from "../workspace/EntityPicker";
import { Inspector } from "../workspace/Inspector";
import type { BuilderSlot } from "./BuilderGrid";
import { dayLabel, type DayEvent, type DayEventTeacher } from "./DayScheduleGrid";
import type { LoadEntry } from "./loadPanel";
import { attachExtras, extractCreatedEventId, readableCreateError, type WeekTypeOption } from "./SlotEditor";
import { kafedraTeacherOptions } from "./teacherOptions";

const DAY_NAMES = ["", "Понедельник", "Вторник", "Среда", "Четверг", "Пятница", "Суббота"];

// A class may carry at most this many teachers and at most this many rooms —
// the primary one plus a single additional entry. The API itself imposes no such
// limit; it is a deliberate product rule, enforced here and mirrored in SlotEditor.
export const MAX_TEACHERS_PER_EVENT = 2;
export const MAX_ROOMS_PER_EVENT = 2;

export type BatchSibling = { id: string; groupLabel: string };

// Editor for one series (a builder-grid cell has no pinned date; a day-grid
// occurrence does — `date` is "" for the former, which hides the whole "Эта
// дата" tab since there is no single occurrence to act on).
//
// "Параметры занятия" now mirrors SlotEditor's creation form exactly, including
// the load-based subject/teacher/room picker, because editing IS re-creation:
// saving deletes the whole stream batch (every group this class was created
// for, found via DELETE's server-side cascade — see event_batch_id) and
// re-POSTs /schedule_events/from_recommendation with the edited values, the
// same call SlotEditor's manual form makes. A group still matching the stream
// gets its class recreated; a group no longer matching simply doesn't — this
// mirrors creation instead of ever partially PATCHing a batch member.
export function OccurrenceEditor({ event, date, groupSemesterId, groupLabel, rooms, slots, weekTypes, loadEntries, resolveSiblings, onClose, onChanged, onListChanged }: {
  event: DayEvent | null;
  date: string;
  groupSemesterId: string;
  groupLabel: string;
  rooms: EntityOption[];
  slots: BuilderSlot[];
  weekTypes: WeekTypeOption[];
  loadEntries: LoadEntry[];
  resolveSiblings: (entry: LoadEntry) => Promise<{ gsids: number[]; requirementIds: number[] }>;
  onClose: () => void;
  onChanged: () => void;
  onListChanged: () => void;
}) {
  const hasDate = Boolean(date);
  const [tab, setTab] = useState<"params" | "occurrence">("params");

  const [assignedEntry, setAssignedEntry] = useState<LoadEntry | null>(null);
  const [fullDay, setFullDay] = useState("");
  const [fullTimeSlot, setFullTimeSlot] = useState("");
  const [fullWeekTypeId, setFullWeekTypeId] = useState("");
  const [subgroup, setSubgroup] = useState(0);
  const [roomId, setRoomId] = useState("");
  const [teacherTokId, setTeacherTokId] = useState("");
  const [extraTeacherIds, setExtraTeacherIds] = useState<number[]>([]);
  const [extraRoomIds, setExtraRoomIds] = useState<string[]>([]);
  const [addExtraTeacherId, setAddExtraTeacherId] = useState("");
  const [addExtraRoomId, setAddExtraRoomId] = useState("");
  const [fallbackTeachers, setFallbackTeachers] = useState<EntityOption[]>([]);
  const [batchSiblings, setBatchSiblings] = useState<BatchSibling[]>([]);
  const [additionalRoomsPreview, setAdditionalRoomsPreview] = useState<EventRoom[]>([]);
  const [saving, setSaving] = useState("");
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");

  useEffect(() => {
    if (!event) return;
    setError(""); setMessage(""); setTab("params");
    const match = loadEntries.find((entry) => entry.id === event.loadRequirementId) ?? null;
    setAssignedEntry(match);
    setFullDay(String(event.dayOfWeek ?? isoWeekday(date) ?? ""));
    setFullTimeSlot(event.timeSlotId);
    setFullWeekTypeId(event.weekTypeId ? String(event.weekTypeId) : "");
    setSubgroup(event.subgroup);
    setRoomId(event.roomId);
    setTeacherTokId(event.teachers[0]?.teacherOnKafedraId ? String(event.teachers[0].teacherOnKafedraId) : "");
    setExtraTeacherIds(event.teachers.slice(1).map((teacher) => teacher.teacherOnKafedraId).filter((id): id is number => id !== null));
    setExtraRoomIds([]);
    setAddExtraTeacherId(""); setAddExtraRoomId("");
    setFallbackTeachers([]);
    setBatchSiblings([]);
    if (event.kafedraId) void kafedraTeacherOptions(event.kafedraId).then(setFallbackTeachers);
    void api.listEventRooms(Number(event.id)).then((response) => {
      setAdditionalRoomsPreview(response.data);
      setExtraRoomIds(response.data.map((room) => String(room.room_id)));
    }).catch(() => setAdditionalRoomsPreview([]));
    void api.scheduleEventBatch(event.id).then((response) => setBatchSiblings(normalizeBatchSiblings(response, event.id))).catch(() => setBatchSiblings([]));
  }, [event, date, loadEntries]);

  if (!event) return null;
  const current = event;

  const teacherLocked = Boolean(assignedEntry && assignedEntry.teachers.length > 0);
  const roomLocked = Boolean(assignedEntry?.reservedRoom);
  const isStream = Boolean(assignedEntry && assignedEntry.streamSiblingGsids.length > 0);
  const teacherPool = teacherLocked ? (assignedEntry?.teachers ?? []).map((teacher) => ({ id: String(teacher.teacherOnKafedraId), label: teacher.name })) : fallbackTeachers;
  const extraTeacherOptions = teacherPool.filter((option) => option.id !== teacherTokId && !extraTeacherIds.includes(Number(option.id)));
  const extraRoomOptions = rooms.filter((room) => room.id !== roomId && !extraRoomIds.includes(room.id));
  const paramsIncomplete = !assignedEntry || !fullDay || !fullTimeSlot || !fullWeekTypeId;
  const otherBatchGroups = batchSiblings.filter((sibling) => sibling.id !== current.id);

  function addExtraTeacher() {
    const id = Number(addExtraTeacherId);
    if (!id) return;
    setExtraTeacherIds((list) => list.includes(id) ? list : [...list, id]);
    setAddExtraTeacherId("");
  }
  function removeExtraTeacher(id: number) {
    setExtraTeacherIds((list) => list.filter((item) => item !== id));
  }
  function addExtraRoom() {
    if (!addExtraRoomId) return;
    setExtraRoomIds((list) => list.includes(addExtraRoomId) ? list : [...list, addExtraRoomId]);
    setAddExtraRoomId("");
  }
  function removeExtraRoom(id: string) {
    setExtraRoomIds((list) => list.filter((item) => item !== id));
  }

  async function saveParams() {
    if (paramsIncomplete || !assignedEntry) return;
    const weekType = weekTypes.find((item) => String(item.id) === fullWeekTypeId);
    if (!weekType) return;
    setSaving("params"); setError(""); setMessage("");
    try {
      const siblings = assignedEntry.streamSiblingGsids.length ? await resolveSiblings(assignedEntry) : { gsids: [], requirementIds: [] };
      const recommendation = buildEditRecommendation({
        assignedEntry, groupSemesterId, dayOfWeek: fullDay, timeSlotId: fullTimeSlot,
        callNumber: slots.find((slot) => slot.id === fullTimeSlot)?.callNumber ?? 0,
        weekType, roomId, teacherTokId, subgroup, siblings,
      });
      await api.deleteScheduleEvent(current.id); // cascades to the whole batch server-side
      const response = await api.createEventFromRecommendation(recommendation);
      const warning = await attachExtras(response, assignedEntry.id, {
        teacherIds: extraTeacherIds,
        teacherLabel: (id) => teacherPool.find((option) => Number(option.id) === id)?.label ?? `#${id}`,
        roomIds: extraRoomIds,
        roomLabel: (id) => rooms.find((room) => room.id === id)?.label ?? `#${id}`,
      });
      setMessage(warning ?? "Занятие обновлено.");
      onChanged();
    } catch (caught) {
      setError(readableCreateError(caught));
    } finally {
      setSaving("");
    }
  }

  async function run(key: string, action: () => Promise<unknown>, done: string) {
    setSaving(key); setError(""); setMessage("");
    try {
      await action();
      setMessage(done);
    } catch (caught) {
      setError(caught instanceof Error && caught.message ? caught.message : "Изменение не удалось сохранить.");
    } finally {
      setSaving("");
    }
  }

  function deleteEvent() {
    const groupsNote = otherBatchGroups.length ? ` Также будет удалено у групп: ${otherBatchGroups.map((sibling) => sibling.groupLabel).join(", ")}.` : "";
    if (!window.confirm(`Удалить занятие целиком?${groupsNote} Действие необратимо.`)) return;
    return run("delete", () => api.deleteScheduleEvent(current.id).then(() => onChanged()), "Занятие удалено.");
  }

  function undoTransfer() {
    if (!current.transferId) return;
    return run("undo", () => api.deleteEventTransfer(current.transferId!).then(() => onChanged()), "Перенос отменён.");
  }

  function cancelOccurrence() {
    return run("cancel", () => api.createEventTransfer(current.id, { original_date: date, transfer_type: "cancelled" }).then(() => onChanged()), "Занятие отменено на эту дату.");
  }

  return <Inspector title={current.subject} open onClose={onClose}>
    <div className="transfer-summary">
      <strong>{current.subject}</strong>
      {hasDate ? <span>{current.startTime}{current.endTime ? `–${current.endTime}` : ""} · {dayLabel(date)}</span> : <span>{current.startTime}{current.endTime ? `–${current.endTime}` : ""}</span>}
      <small>{current.activity} · {current.room}{current.specificDate ? " · разовое занятие" : ""}</small>
    </div>

    {hasDate ? <div className="editor-tabbar" role="tablist" aria-label="Редактирование занятия">
      <button type="button" role="tab" aria-selected={tab === "params"} className={tab === "params" ? "active" : ""} onClick={() => setTab("params")}>Параметры занятия</button>
      <button type="button" role="tab" aria-selected={tab === "occurrence"} className={tab === "occurrence" ? "active" : ""} onClick={() => setTab("occurrence")}>Эта дата</button>
    </div> : null}

    {!hasDate || tab === "params" ? <>
    <section className="editor-section">
      <h3>Изменить занятие</h3>
      <p className="inspector-help">Эти поля описывают ВСЮ серию занятий, а не одну дату — сохранение пересоздаёт занятие с новыми значениями{otherBatchGroups.length ? `, включая ${otherBatchGroups.length === 1 ? "группу" : "группы"} потока: ${otherBatchGroups.map((sibling) => sibling.groupLabel).join(", ")}` : ""}.</p>
      <div className="inspector-form">
        <div
          className={assignedEntry ? "load-drop-zone has-entry" : "load-drop-zone"}
          onDragOver={(dragEvent) => { dragEvent.preventDefault(); dragEvent.dataTransfer.dropEffect = "copy"; }}
          onDrop={(dragEvent) => {
            dragEvent.preventDefault();
            const id = Number(dragEvent.dataTransfer.getData("application/x-load-entry"));
            const found = loadEntries.find((item) => item.id === id);
            if (found) setAssignedEntry(found);
          }}
        >
          {assignedEntry
            ? <><strong>{assignedEntry.subjectName}</strong><span>{assignedEntry.activityName} · {assignedEntry.actualHours}/{assignedEntry.controlSum} ч{assignedEntry.streamSiblingGsids.length ? ` · поток из ${assignedEntry.streamSiblingGsids.length + 1} групп` : ""}</span><button type="button" onClick={() => setAssignedEntry(null)}>Очистить</button></>
            : <>
              <span>Перетащите строку нагрузки сюда или выберите ниже.</span>
              {loadEntries.length
                ? <ul className="load-pick-list">{loadEntries.map((entry) => <li key={entry.id}><button type="button" onClick={() => setAssignedEntry(entry)}><strong>{entry.subjectShort}</strong><span>{entry.activityName} · {entry.actualHours}/{entry.controlSum} ч</span></button></li>)}</ul>
                : <p className="inspector-help">Нет распределения нагрузки для этой группы.</p>}
            </>}
        </div>
        <label><span>День недели</span>
          <select value={fullDay} onChange={(input) => setFullDay(input.target.value)}>
            <option value="">Выберите день</option>
            {DAY_NAMES.slice(1).map((label, index) => <option value={index + 1} key={label}>{label}</option>)}
          </select>
        </label>
        <label><span>Пара</span>
          <select value={fullTimeSlot} onChange={(input) => setFullTimeSlot(input.target.value)}>
            <option value="">Выберите пару</option>
            {slots.map((slot) => <option value={slot.id} key={slot.id}>{slot.label}</option>)}
          </select>
        </label>
        <label><span>Тип недели</span>
          <select value={fullWeekTypeId} onChange={(input) => setFullWeekTypeId(input.target.value)}>
            <option value="">Выберите тип недели</option>
            {weekTypes.map((item) => <option value={item.id} key={item.id}>{weekTypeLabel(item)}</option>)}
          </select>
        </label>
        {isStream
          ? <p className="inspector-note"><strong>Подгруппа: вся группа</strong><span>Поточное занятие охватывает несколько групп целиком — деление на подгруппы для него недоступно.</span></p>
          : <label><span>Подгруппа</span>
            <select value={subgroup} onChange={(input) => setSubgroup(Number(input.target.value))}>
              <option value={0}>Вся группа</option><option value={1}>Подгруппа 1</option><option value={2}>Подгруппа 2</option>
            </select>
          </label>}
        <label><span>Преподаватель{teacherLocked ? " (из нагрузки)" : ""}</span>
          {teacherLocked
            ? <select value={teacherTokId} onChange={(input) => setTeacherTokId(input.target.value)} disabled={assignedEntry!.teachers.length === 1}>{assignedEntry!.teachers.map((teacher) => <option value={teacher.teacherOnKafedraId} key={teacher.teacherOnKafedraId}>{teacher.name}</option>)}</select>
            : <select value={teacherTokId} onChange={(input) => setTeacherTokId(input.target.value)} disabled={!assignedEntry}><option value="">Без преподавателя</option>{fallbackTeachers.map((teacher) => <option value={teacher.id} key={teacher.id}>{teacher.label}</option>)}</select>}
        </label>
        {assignedEntry ? <div className="assignment-add-block">
          <span className="inspector-field-hint">Дополнительные преподаватели (кроме основного)</span>
          {extraTeacherIds.length ? <ul className="assignment-list">{extraTeacherIds.map((id) => <li className="assignment-row" key={id}><span>{teacherPool.find((option) => Number(option.id) === id)?.label ?? `#${id}`}</span><Button type="button" className="button-small button-outline danger-button" onClick={() => removeExtraTeacher(id)}>Убрать</Button></li>)}</ul> : null}
          {extraTeacherIds.length >= MAX_TEACHERS_PER_EVENT - 1
            ? <p className="inspector-field-hint">Максимум {MAX_TEACHERS_PER_EVENT} преподавателя на занятие (включая основного).</p>
            : <div className="assignment-add">
            <select value={addExtraTeacherId} onChange={(input) => setAddExtraTeacherId(input.target.value)}>
              <option value="">Добавить преподавателя…</option>
              {extraTeacherOptions.map((option) => <option value={option.id} key={option.id}>{option.label}</option>)}
            </select>
            <Button type="button" className="button-small" disabled={!addExtraTeacherId} onClick={addExtraTeacher}>Добавить</Button>
          </div>}
        </div> : null}
        <label><span>Аудитория{roomLocked ? " (закреплена)" : ""}</span>
          <select value={roomId} onChange={(input) => setRoomId(input.target.value)} disabled={roomLocked || !assignedEntry}>
            {roomLocked ? <option value={String(assignedEntry!.reservedRoom!.id)}>{assignedEntry!.reservedRoom!.name}</option> : <><option value="">Без аудитории</option>{rooms.map((room) => <option value={room.id} key={room.id}>{room.label}</option>)}</>}
          </select>
        </label>
        {assignedEntry ? <div className="assignment-add-block">
          <span className="inspector-field-hint">Дополнительные аудитории (кроме основной)</span>
          {extraRoomIds.length ? <ul className="assignment-list">{extraRoomIds.map((id) => <li className="assignment-row" key={id}><span>{rooms.find((room) => room.id === id)?.label ?? additionalRoomsPreview.find((room) => String(room.room_id) === id)?.room_name ?? `#${id}`}</span><Button type="button" className="button-small button-outline danger-button" onClick={() => removeExtraRoom(id)}>Убрать</Button></li>)}</ul> : null}
          {extraRoomIds.length >= MAX_ROOMS_PER_EVENT - 1
            ? <p className="inspector-field-hint">Максимум {MAX_ROOMS_PER_EVENT} аудитории на занятие (включая основную).</p>
            : <div className="assignment-add">
            <select value={addExtraRoomId} onChange={(input) => setAddExtraRoomId(input.target.value)}>
              <option value="">Добавить аудиторию…</option>
              {extraRoomOptions.map((room) => <option value={room.id} key={room.id}>{room.label}</option>)}
            </select>
            <Button type="button" className="button-small" disabled={!addExtraRoomId} onClick={addExtraRoom}>Добавить</Button>
          </div>}
        </div> : null}
        <div className="inspector-actions">
          <Button className="button-primary" disabled={Boolean(saving) || paramsIncomplete} onClick={saveParams}>{saving === "params" ? "Сохраняем…" : "Сохранить изменения"}</Button>
        </div>
      </div>
    </section>

    <section className="editor-section">
      <h3>Удалить занятие</h3>
      <p className="inspector-help">Удаляет занятие целиком{otherBatchGroups.length ? `, включая ${otherBatchGroups.length === 1 ? "группу" : "группы"} потока: ${otherBatchGroups.map((sibling) => sibling.groupLabel).join(", ")}` : ""}.</p>
      <Button className="button-outline danger-button" disabled={Boolean(saving)} onClick={deleteEvent}>{saving === "delete" ? "Удаляем…" : "Удалить занятие"}</Button>
    </section>
    </> : null}

    {hasDate && tab === "occurrence" ? <>
    <section className="editor-section">
      <h3>Отмена</h3>
      <div className="inspector-actions">
        <Button className="button-outline danger-button" disabled={Boolean(saving)} onClick={cancelOccurrence}>{saving === "cancel" ? "Отменяем…" : "Отменить занятие на эту дату"}</Button>
        {current.transferId ? <Button className="button-outline" disabled={Boolean(saving)} onClick={undoTransfer}>{saving === "undo" ? "Возвращаем…" : "Отменить перенос"}</Button> : null}
      </div>
    </section>
    </> : null}

    {error ? <div className="form-error" role="alert">{error}</div> : null}
    {message ? <p className="request-status" role="status">{message}</p> : null}
  </Inspector>;
}

// Pure builder for the from_recommendation payload an edit-save sends — split
// out so it's testable without mounting the component (mirrors SlotEditor's
// submitManual, minus the React state).
export function buildEditRecommendation({ assignedEntry, groupSemesterId, dayOfWeek, timeSlotId, callNumber, weekType, roomId, teacherTokId, subgroup, siblings }: {
  assignedEntry: LoadEntry;
  groupSemesterId: string;
  dayOfWeek: string;
  timeSlotId: string;
  callNumber: number;
  weekType: { id: number; code: string };
  roomId: string;
  teacherTokId: string;
  subgroup: number;
  siblings: { gsids: number[]; requirementIds: number[] };
}): Record<string, unknown> {
  const isStream = assignedEntry.streamSiblingGsids.length > 0;
  return {
    score: 0,
    day_of_week: Number(dayOfWeek),
    time_slot_id: Number(timeSlotId),
    call_number: callNumber,
    week_type_id: weekType.id,
    week_type_code: weekType.code,
    room_id: roomId ? Number(roomId) : null,
    teacher_on_kafedra_ids: teacherTokId ? [Number(teacherTokId)] : [],
    group_semester_ids: [Number(groupSemesterId), ...siblings.gsids],
    load_requirement_ids: [assignedEntry.id, ...siblings.requirementIds],
    is_day_edge: false,
    week_type_recommendation: weekType.code,
    // Stream siblings are always whole-group (confirmed product rule) — the
    // non-stream case preserves whatever subgroup the operator picked.
    subgroup_number: isStream ? 0 : subgroup,
  };
}

function normalizeBatchSiblings(response: { data: Array<{ id: string; attributes?: Record<string, unknown>; relationships?: Record<string, unknown> }> }, primaryId: string): BatchSibling[] {
  return response.data.map((row) => {
    const attrs = row.attributes ?? {};
    const label = String(attrs.group_display_name ?? "");
    return { id: row.id, groupLabel: label || (row.id === primaryId ? "эта группа" : `группа #${row.id}`) };
  });
}

function weekTypeLabel(item: WeekTypeOption): string {
  if (item.code === "both") return "Каждую неделю";
  if (item.code === "odd") return "Нечётные недели";
  if (item.code === "even") return "Чётные недели";
  if (item.code === "lecture_week" || item.code === "credit_week" || item.code === "exam_session") return PERIOD_LABELS[item.code];
  return item.name;
}

function isoWeekday(date: string): number | null {
  if (!date) return null;
  const parsed = new Date(`${date}T00:00:00Z`);
  if (Number.isNaN(parsed.getTime())) return null;
  return parsed.getUTCDay() || 7;
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd apps/frontend && npx vitest run src/features/admin/schedule/occurrenceEditor.test.ts`
Expected: PASS

Run: `cd apps/frontend && npx tsc --noEmit`
Expected: clean — pay particular attention to `DayEventTeacher` import (used only in the type position for `event.teachers`; if unused after the rewrite, remove the import rather than leaving an unused-import error) and to `date` being typed `string` (not `string | null`) at every call site (Task 15/16 must pass `""`, not `null`).

- [ ] **Step 5: Commit**

```bash
cd apps/frontend
git add src/features/admin/schedule/OccurrenceEditor.tsx src/features/admin/schedule/occurrenceEditor.test.ts
git commit -m "feat(occurrence-editor): unify editing with the creation form; edit = delete batch + recreate"
```

---

### Task 15: `AdminSchedule.tsx` — builder grid uses `OccurrenceEditor` instead of `ScheduleInspector`

**Files:**
- Modify: `apps/frontend/src/features/admin/AdminSchedule.tsx`

**Interfaces:**
- Consumes: `OccurrenceEditor`'s new props (Task 14), `BuilderEvent.loadRequirementId`/`.groupSemesterId`/etc. (Task 11).
- Produces: clicking an existing builder-grid cell opens the SAME editor the day-grid uses, in date-less mode. The `mode !== "group"` (teacher/room query result) click path keeps using `ScheduleInspector` unchanged — out of scope for this plan (the user's complaint was specifically about the schedule-builder grid).

- [ ] **Step 1: Add state and a `BuilderEvent → DayEvent` adapter**

Near the existing `selectedEvent`/`selectedEventGroup` state, add a parallel piece of state for the builder-grid editor (keep `selectedEvent`/`ScheduleInspector` for the other, untouched, click path):
```ts
const [builderEditorEvent, setBuilderEditorEvent] = useState<{ event: DayEvent; groupSemesterId: string; groupLabel: string } | null>(null);
```
Add the import for `DayEvent` from `./schedule/DayScheduleGrid` (and `OccurrenceEditor` from `./schedule/OccurrenceEditor`) at the top of the file alongside the other schedule/* imports.

Add an adapter function near `builderEventToScheduleEvent` (which stays, still used by the teacher/room result path):
```ts
// Bridges a builder grid cell into the shape OccurrenceEditor expects — the two
// event shapes carry the same fields (Task 11 added load_requirement_id/
// group_semester_id/teacher structure to BuilderEvent specifically for this).
function builderEventToDayEvent(event: BuilderEvent, slots: BuilderSlot[]): DayEvent {
  const slot = slots.find((item) => item.id === event.timeSlotId);
  return {
    id: event.id,
    timeSlotId: event.timeSlotId,
    startTime: slot?.startTime ?? "",
    endTime: (slot?.label ?? "").split("–")[1] ?? "",
    subject: event.subject,
    activity: event.activity,
    roomId: event.roomId,
    room: event.room,
    kafedraId: event.kafedraId,
    status: event.status,
    specificDate: null,
    subgroup: event.subgroup,
    teachers: event.teacherList,
    additionalRooms: event.additionalRooms,
    loadRequirementId: event.loadRequirementId,
    groupSemesterId: event.groupSemesterId,
    weekTypeId: event.weekTypeId,
    dayOfWeek: event.dayOfWeek,
    movedFrom: null,
    transferId: null,
  };
}
```

- [ ] **Step 2: Change the builder grid's `onEventClick`**

Change (line 238):
```tsx
              onEventClick={(event) => { setEditorContext(null); setSelectedEventGroup(groupSemesterId); setSelectedEvent(builderEventToScheduleEvent(event, slots, groupLabelById.get(groupSemesterId) ?? "")); }}
```
to:
```tsx
              onEventClick={(event) => { setEditorContext(null); setBuilderEditorEvent({ event: builderEventToDayEvent(event, slots), groupSemesterId, groupLabel: groupLabelById.get(groupSemesterId) ?? "" }); }}
```

- [ ] **Step 3: Render `OccurrenceEditor` instead of a second `ScheduleInspector`**

After the existing `<ScheduleInspector ... />` block (around line 249-258), add:
```tsx
    <OccurrenceEditor
      event={builderEditorEvent?.event ?? null}
      date=""
      groupSemesterId={builderEditorEvent?.groupSemesterId ?? ""}
      groupLabel={builderEditorEvent?.groupLabel ?? ""}
      rooms={rooms}
      slots={slots}
      weekTypes={weekTypes}
      loadEntries={builderEditorEvent ? loadByGroup[builderEditorEvent.groupSemesterId] ?? [] : []}
      resolveSiblings={resolveSiblings}
      onClose={() => setBuilderEditorEvent(null)}
      onChanged={() => { const group = builderEditorEvent?.groupSemesterId; setBuilderEditorEvent(null); if (group) void refreshGroup(group); }}
      onListChanged={() => { const group = builderEditorEvent?.groupSemesterId; if (group) void refreshGroup(group); }}
    />
```
(`rooms`, `slots`, `weekTypes`, `loadByGroup`, `resolveSiblings`, `refreshGroup` all already exist in this component's scope — this is prop wiring only, no new data fetching.)

- [ ] **Step 4: Type-check and run existing tests**

Run: `cd apps/frontend && npx tsc --noEmit`
Expected: clean.

Run: `cd apps/frontend && npx vitest run src/features/admin/schedule/builderGrid.test.tsx src/features/admin/schedule/loadPanel.test.ts`
Expected: PASS (no behavior in these files changed by this task, but confirms nothing broke).

- [ ] **Step 5: Manual verification against the live app** (this task has no automated component test — see Global Constraints on Docker rebuild)

Run: `cd /c/Users/Asus/Desktop/milk_schedule && docker compose build frontend api_geteway && docker compose up -d --force-recreate frontend api_geteway`

In the admin panel: open Конструктор расписания → group mode → click an existing cell. Confirm the drawer now shows the full load-based form (subject/teacher/room all editable, not just day/pair/room/status), change the teacher, save, and confirm the change persists after a refresh.

- [ ] **Step 6: Commit**

```bash
cd apps/frontend
git add src/features/admin/AdminSchedule.tsx
git commit -m "feat(admin-schedule): builder grid cells open the full load-based editor"
```

---

### Task 16: `AdminOperations.tsx` — wire `resolveSiblings`/`groupSemesterId` into its existing `OccurrenceEditor`

**Files:**
- Modify: `apps/frontend/src/features/admin/AdminOperations.tsx`

**Interfaces:**
- Consumes: `makeSiblingResolver` (Task 10), `OccurrenceEditor`'s new required props (Task 14).
- Produces: the day-grid ("Операции") editor gains the same full editing capability, batch-aware delete, and roomless support as the builder grid — this page already renders `OccurrenceEditor`, it just needs the two new required props supplied.

- [ ] **Step 1: Add a load cache ref and `resolveSiblings`**

Near the existing `const [loadEntries, setLoadEntries] = useState<LoadEntry[]>([]);` (line 35), add:
```ts
const loadCache = useRef<Record<string, LoadEntry[]>>({});
const resolveSiblings = useCallback(
  makeSiblingResolver(loadCache.current, (id) => api.groupWorkload(id).then((response) => normalizeLoadEntries(response, id))),
  []
);
```
Update the import line (currently `import { normalizeLoadEntries, type LoadEntry } from "./schedule/loadPanel";`) to also bring in `makeSiblingResolver`.

- [ ] **Step 2: Find the group's own `groupSemesterId` in scope at the `<OccurrenceEditor>` call site**

Read the surrounding code at line 253 to find the variable already holding the current single group's id (this page operates on ONE group at a time, unlike `AdminSchedule.tsx`'s multi-group builder — the id is almost certainly already a `groupId`/`selectedGroupId`-style variable in scope; use whatever that variable is actually named rather than inventing a new one).

- [ ] **Step 3: Pass the two new props**

At the `<OccurrenceEditor ... />` call (lines 253-259), add:
```tsx
      groupSemesterId={groupId}
      groupLabel={groupLabelInScope}
      resolveSiblings={resolveSiblings}
```
(substituting the actual in-scope variable names found in Step 2 for `groupId`/`groupLabelInScope`).

- [ ] **Step 4: Type-check**

Run: `cd apps/frontend && npx tsc --noEmit`
Expected: clean — this should surface immediately if `OccurrenceEditor`'s required props aren't all supplied.

- [ ] **Step 5: Manual verification against the live app**

Using the same rebuilt containers from Task 15's Step 5: open Операции (day-grid) for a group, click an existing occurrence, confirm the "Параметры занятия" tab now has the same load-based fields, and confirm "Эта дата" still works (cancel/undo transfer) since this page always has a real `date`.

- [ ] **Step 6: Commit**

```bash
cd apps/frontend
git add src/features/admin/AdminOperations.tsx
git commit -m "feat(admin-operations): wire stream-sibling resolution into the day-grid editor"
```

---

### Task 17: End-to-end verification against the user's four reported gaps

**Files:** none — verification only, closes out the plan.

- [ ] **Step 1: Rebuild both containers one more time with everything applied**

Run: `cd /c/Users/Asus/Desktop/milk_schedule && docker compose build frontend api_geteway && docker compose up -d --force-recreate frontend api_geteway`

- [ ] **Step 2: Re-check each of the four original complaints**

1. **Edit an existing cell, change teacher and subject.** Builder grid AND day grid — both should now let every field (subject/load, teacher, room, day, slot, week type) be changed, not just day/pair/room/status.
2. **Stream cascade.** Create a class shared by two groups (drag a load entry whose `streamSiblingGsids` is non-empty), confirm it appears in both groups' grids. Edit it in one group (e.g. change the room) → confirm the other group's grid shows the SAME new room. Delete it from one group → confirm it disappears from the other group's grid too. Edit it to a DIFFERENT subject that is no longer shared with the sibling group → confirm the sibling group's class is gone (not recreated) while the edited group's new class exists.
3. **No room/teacher required.** Create a class via the manual form with both left blank → confirm `201` and the class renders as "Без аудитории"/"Не назначено" in the grid instead of failing to save.
4. **Извещение** — re-confirm it still renders correctly (no code in this plan touches it, but Task 4's `schedule_grid.rb`/presenter changes are shared infrastructure — worth a quick re-check that the notice report, which reads from `lib/api_geteway/queries/teacher_notice.rb`, still returns the same rows it did before).

Expected: all four confirmed. If any fails, do NOT patch around it ad hoc — go back to the specific task above whose interface contract it violates and fix it there, per this plan's TDD steps.

- [ ] **Step 3: Run each app's full test suite one last time**

Run: `cd apps/api_geteway && bundle exec rspec`
Run: `cd apps/frontend && npx vitest run && npx tsc --noEmit`
Expected: both green.
