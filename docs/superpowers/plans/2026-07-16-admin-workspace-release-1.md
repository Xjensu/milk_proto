# Admin Workspace Release 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver the shared context foundation, readable admin data layer, operational Overview, and guided Group/Teacher/Room/Occupancy schedule workspace.

**Architecture:** Add small task-oriented gateway projections only where existing endpoints cannot satisfy the UI, then normalize all network data into typed frontend view models. A URL-synchronized workspace provider owns dependent selections; pages compose reusable semester rails, search pickers, and right-side inspectors.

**Tech Stack:** React 19, TypeScript, Vite, Vitest, GSAP, Lucide, Hanami/Ruby, PostgreSQL, Redis, RSpec.

---

## File structure

New frontend files:

- `apps/frontend/src/features/admin/domain.ts` — stable view-model types.
- `apps/frontend/src/features/admin/formatters.ts` — semester, group, teacher, room, and audit labels.
- `apps/frontend/src/features/admin/formatters.test.ts` — pure formatting tests.
- `apps/frontend/src/features/admin/workspace/WorkspaceContext.tsx` — dependent context state and URL/session synchronization.
- `apps/frontend/src/features/admin/workspace/WorkspaceContext.test.tsx` — context invalidation tests.
- `apps/frontend/src/features/admin/workspace/SemesterRail.tsx` — horizontal accessible semester selector.
- `apps/frontend/src/features/admin/workspace/EntityPicker.tsx` — bounded searchable picker.
- `apps/frontend/src/features/admin/workspace/Inspector.tsx` — focus-managed detail side panel.
- `apps/frontend/src/features/admin/workspace/workspace.test.tsx` — primitive interaction tests.
- `apps/frontend/src/features/admin/overview/normalize.ts` — dashboard projection normalization.
- `apps/frontend/src/features/admin/schedule/normalize.ts` — schedule and occupancy projection normalization.
- `apps/frontend/src/features/admin/schedule/ScheduleQuery.tsx` — guided mode-specific selection.
- `apps/frontend/src/features/admin/schedule/OccupancyGrid.tsx` — date/room occupancy comparison.
- `apps/frontend/src/features/admin/schedule/ScheduleInspector.tsx` — readable event detail panel.
- `apps/frontend/src/features/admin/README.md` — concise page/data-flow documentation.

Modified frontend files:

- `apps/frontend/src/lib/api.ts` — typed workspace/dashboard/schedule requests and abort support.
- `apps/frontend/src/lib/api.test.ts` — request construction coverage.
- `apps/frontend/src/features/admin/AdminPanel.tsx` — English navigation and workspace provider.
- `apps/frontend/src/features/admin/AdminOverview.tsx` — service status, active users, journal only.
- `apps/frontend/src/features/admin/AdminOverview.test.tsx` — partial-failure and readable-row coverage.
- `apps/frontend/src/features/admin/AdminSchedule.tsx` — compose the new schedule workspace.
- `apps/frontend/src/features/admin/AdminSchedule.test.tsx` — guided selection and date/semester modes.
- `apps/frontend/src/features/schedule/WeekGrid.tsx` — optional teacher/room labels and selection callback.
- `apps/frontend/src/styles.css` — tokens, context rail, inspector, schedule modes, responsive states.

Gateway files are added only for Release 1 gaps:

- `apps/api_geteway/config/routes.rb` — dashboard and batch occupancy routes.
- `apps/api_geteway/slices/admin/actions/dashboard/health.rb` — safe service-state projection.
- `apps/api_geteway/slices/admin/actions/dashboard/active_users.rb` — scoped Redis presence projection.
- `apps/api_geteway/slices/admin/actions/schedules/room_occupancy.rb` — bounded date/room batch read.
- matching contracts, serializers, and request specs under `apps/api_geteway/slices/admin` and `apps/api_geteway/spec/slices/admin/requests`.

### Task 1: Domain formatters

**Files:**
- Create: `apps/frontend/src/features/admin/domain.ts`
- Create: `apps/frontend/src/features/admin/formatters.ts`
- Test: `apps/frontend/src/features/admin/formatters.test.ts`

- [ ] **Step 1: Write failing formatter tests**

```ts
expect(formatSemester({ number: 3, startYear: 2026, endYear: 2027 }))
  .toBe("3 semester 2026/2027");
expect(calculateCourse({ admissionYear: 2023, semesterNumber: 5, semesterStartYear: 2025 }))
  .toBe(3);
expect(formatGroup({ code: "CS-21", course: 3 })).toBe("CS-21-3");
expect(formatTeacher(null)).toBe("Not assigned");
```

- [ ] **Step 2: Verify failure**

Run: `npm test -- --run src/features/admin/formatters.test.ts`
Expected: FAIL because the modules do not exist.

- [ ] **Step 3: Implement typed formatters**

```ts
export function formatSemester(value: SemesterSummary) {
  return `${value.number} semester ${value.startYear}/${value.endYear}`;
}

export function calculateCourse(value: CourseInput) {
  const academicOffset = value.semesterStartYear - value.admissionYear;
  return Math.max(1, academicOffset + (value.semesterNumber >= 1 ? 1 : 0));
}
```

- [ ] **Step 4: Run tests and typecheck**

Run: `npm test -- --run src/features/admin/formatters.test.ts && npm run lint`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/frontend/src/features/admin/domain.ts apps/frontend/src/features/admin/formatters.ts apps/frontend/src/features/admin/formatters.test.ts
git commit -m "Add readable admin domain formatters"
```

### Task 2: Typed API boundary

**Files:**
- Modify: `apps/frontend/src/lib/api.ts`
- Modify: `apps/frontend/src/lib/api.test.ts`
- Create: `apps/frontend/src/features/admin/overview/normalize.ts`
- Create: `apps/frontend/src/features/admin/schedule/normalize.ts`

- [ ] **Step 1: Add failing request and normalization tests**

Cover faculty groups with `semester_id`, teacher/room semester and date queries, bounded teacher search, dashboard endpoints, and occupancy POST payloads.

```ts
await api.listFacultyGroups("4", "12");
expect(fetch).toHaveBeenCalledWith(expect.stringContaining("/admin/faculties/4/groups?semester_id=12"), expect.anything());
```

- [ ] **Step 2: Verify failure**

Run: `npm test -- --run src/lib/api.test.ts`
Expected: FAIL for missing methods.

- [ ] **Step 3: Implement methods and normalizers**

Add `AbortSignal` support to safe GET/search requests and introduce typed methods without removing compatibility methods.

```ts
teacherScheduleQuery(teacherId: string, query: { semesterId?: string; date?: string }, signal?: AbortSignal)
roomOccupancy(date: string, roomIds: string[])
dashboardHealth()
dashboardActiveUsers()
```

- [ ] **Step 4: Run focused and full frontend tests**

Run: `npm test -- --run src/lib/api.test.ts && npm test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/frontend/src/lib/api.ts apps/frontend/src/lib/api.test.ts apps/frontend/src/features/admin/overview/normalize.ts apps/frontend/src/features/admin/schedule/normalize.ts
git commit -m "Add typed admin workspace API boundary"
```

### Task 3: Workspace context and selection primitives

**Files:**
- Create: `apps/frontend/src/features/admin/workspace/WorkspaceContext.tsx`
- Create: `apps/frontend/src/features/admin/workspace/WorkspaceContext.test.tsx`
- Create: `apps/frontend/src/features/admin/workspace/SemesterRail.tsx`
- Create: `apps/frontend/src/features/admin/workspace/EntityPicker.tsx`
- Create: `apps/frontend/src/features/admin/workspace/Inspector.tsx`
- Create: `apps/frontend/src/features/admin/workspace/workspace.test.tsx`
- Modify: `apps/frontend/src/features/admin/AdminPanel.tsx`

- [ ] **Step 1: Write failing dependency and accessibility tests**

Verify that changing faculty clears group, changing semester clears group, URL parameters update, semester cards expose radio semantics, picker search is keyboard operable, and Inspector returns focus on close.

- [ ] **Step 2: Verify failure**

Run: `npm test -- --run src/features/admin/workspace`
Expected: FAIL because components do not exist.

- [ ] **Step 3: Implement context and primitives**

Use a reducer for atomic dependent-state updates. Store only versioned identifiers in session storage. Use semantic buttons/listbox behavior and a focus-managed `<dialog>` or equivalent established primitive.

- [ ] **Step 4: Add GSAP inspector transition**

Animate only `x` and `autoAlpha`, kill the tween during cleanup, and make duration zero under reduced motion.

- [ ] **Step 5: Run tests and typecheck**

Run: `npm test -- --run src/features/admin/workspace && npm run lint`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add apps/frontend/src/features/admin/workspace apps/frontend/src/features/admin/AdminPanel.tsx
git commit -m "Add admin workspace context and inspectors"
```

### Task 4: Operational dashboard gateway projections

**Files:**
- Modify: `apps/api_geteway/config/routes.rb`
- Create: `apps/api_geteway/slices/admin/actions/dashboard/health.rb`
- Create: `apps/api_geteway/slices/admin/actions/dashboard/active_users.rb`
- Create matching serializers/contracts/specs.

- [ ] **Step 1: Write failing request specs**

Verify authenticated access, safe health payloads, empty active-user state, TTL-based presence, and scope filtering. Ensure responses never include infrastructure secrets.

- [ ] **Step 2: Run specs to verify failure**

Run: `bundle exec rspec spec/slices/admin/requests/dashboard_spec.rb`
Expected: FAIL with missing routes/actions.

- [ ] **Step 3: Implement minimal projections**

Health reports `operational`, `degraded`, or `unavailable` with measured latency. Presence reads Redis keys with expiry and resolves only safe account fields.

- [ ] **Step 4: Run focused gateway specs**

Run: `bundle exec rspec spec/slices/admin/requests/dashboard_spec.rb`
Expected: PASS.

- [ ] **Step 5: Commit gateway change in its nested repository**

```bash
git add config/routes.rb slices/admin/actions/dashboard slices/admin/serializers/dashboard spec/slices/admin/requests/dashboard_spec.rb
git commit -m "Add operational dashboard projections"
```

### Task 5: Rebuild Overview

**Files:**
- Modify: `apps/frontend/src/features/admin/AdminOverview.tsx`
- Modify: `apps/frontend/src/features/admin/AdminOverview.test.tsx`
- Modify: `apps/frontend/src/styles.css`

- [ ] **Step 1: Write failing Overview tests**

Assert that generic metrics and semester controls are absent; service status, active users, and event journal are present; one rejected request does not hide fulfilled sections; row selection opens an inspector.

- [ ] **Step 2: Verify failure**

Run: `npm test -- --run src/features/admin/AdminOverview.test.tsx`
Expected: FAIL against the old metric dashboard.

- [ ] **Step 3: Implement the operational Overview**

Fetch health, presence, and audit independently with `Promise.allSettled`. Render readable audit descriptions and localized section retries.

- [ ] **Step 4: Run focused tests, full tests, and build**

Run: `npm test -- --run src/features/admin/AdminOverview.test.tsx && npm test && npm run build`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/frontend/src/features/admin/AdminOverview.tsx apps/frontend/src/features/admin/AdminOverview.test.tsx apps/frontend/src/styles.css
git commit -m "Rebuild admin overview around operations"
```

### Task 6: Date-aware and batch occupancy gateway reads

**Files:**
- Modify: `apps/api_geteway/config/routes.rb`
- Modify teacher and room schedule contracts/actions/repos.
- Create: `apps/api_geteway/slices/admin/actions/schedules/room_occupancy.rb`
- Create matching contract/serializer/request specs.

- [ ] **Step 1: Write failing request and repo specs**

Cover exactly one of `semester_id` or `date`, group labels in room events, bounded room ID arrays, empty results, transfers, and reservations.

- [ ] **Step 2: Verify failure**

Run focused schedule request/repo specs and expect missing date mode and occupancy route failures.

- [ ] **Step 3: Implement indexed query paths and serializers**

Reuse the canonical schedule-grid serializer. Keep date filtering and transfer application in the repository/query layer, not the action.

- [ ] **Step 4: Run focused and complete gateway specs**

Run: `bundle exec rspec spec/slices/admin/requests/schedules_spec.rb spec/slices/admin/repos/schedule_event_repo_spec.rb`
Expected: PASS.

- [ ] **Step 5: Commit gateway change**

```bash
git add config/routes.rb slices/admin/actions slices/admin/contracts slices/admin/repos slices/admin/serializers spec/slices/admin
git commit -m "Add date-aware admin schedule projections"
```

### Task 7: Rebuild Schedule workspace

**Files:**
- Modify: `apps/frontend/src/features/admin/AdminSchedule.tsx`
- Modify: `apps/frontend/src/features/admin/AdminSchedule.test.tsx`
- Create: `apps/frontend/src/features/admin/schedule/ScheduleQuery.tsx`
- Create: `apps/frontend/src/features/admin/schedule/OccupancyGrid.tsx`
- Create: `apps/frontend/src/features/admin/schedule/ScheduleInspector.tsx`
- Modify: `apps/frontend/src/features/schedule/WeekGrid.tsx`
- Modify: `apps/frontend/src/styles.css`

- [ ] **Step 1: Write failing workflow tests**

Cover Faculty → Semester → Group dependency, teacher semester/date modes, room semester/date modes, multi-room occupancy, optional teacher/room text, readable group labels, and event inspector selection.

- [ ] **Step 2: Verify failure**

Run: `npm test -- --run src/features/admin/AdminSchedule.test.tsx`
Expected: FAIL against raw selects and missing modes.

- [ ] **Step 3: Implement guided query workflows**

Use the shared context and primitives. Fetch only after required parent context exists. Abort superseded searches and keep the prior successful schedule visible during refresh.

- [ ] **Step 4: Implement occupancy and event inspector**

Use a semantic table for room/time comparison with sticky labels. `Not assigned` is used for valid nulls.

- [ ] **Step 5: Run tests, typecheck, and build**

Run: `npm test && npm run lint && npm run build`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add apps/frontend/src/features/admin/AdminSchedule.tsx apps/frontend/src/features/admin/AdminSchedule.test.tsx apps/frontend/src/features/admin/schedule apps/frontend/src/features/schedule/WeekGrid.tsx apps/frontend/src/styles.css
git commit -m "Build guided admin schedule workspace"
```

### Task 8: Documentation and browser verification

**Files:**
- Create: `apps/frontend/src/features/admin/README.md`
- Modify tests/styles only for verified defects.

- [ ] **Step 1: Document page and method contracts**

Describe workspace dependencies, readable label rules, Overview projections, schedule query modes, inspector ownership, and nullable teacher/room behavior.

- [ ] **Step 2: Run the application and verify in the in-app browser**

Check desktop, tablet, and mobile widths. Exercise keyboard navigation, focus return, reduced motion, partial failures, empty data, teacher search, room search, and occupancy scrolling.

- [ ] **Step 3: Fix every observed regression and rerun checks**

Run: `npm test && npm run lint && npm run build`
Expected: all commands exit 0.

- [ ] **Step 4: Commit documentation and verification fixes**

```bash
git add apps/frontend/src/features/admin/README.md apps/frontend/src
git commit -m "Document and verify admin workspace release one"
```

## Release 1 completion evidence

- Every visible semester and group uses the mandated format.
- Overview contains only service state, active users, and journal.
- Group schedule requires faculty and semester before group.
- Teacher and room schedules support semester and exact-date modes.
- Occupancy supports a selected date and room set.
- Schedule events show readable group, subject, teacher, and room data with valid missing assignments.
- No newly built workflow asks users to enter raw IDs.
- Tests, typecheck, build, request specs, and browser checks pass.
- Commits include only scoped frontend or gateway files and exclude AI-related artifacts.
