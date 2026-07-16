# Minimalist System and Full Admin CRUD Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the current neon/glass client with a unified readable minimalist interface and expose every gateway-supported administrator mutation through typed resource or dedicated workflow UI.

**Architecture:** A typed resource registry drives ordinary and limited-capability collections through one CRUD workspace and semantic form renderer. Nested group-load, scheduling, file, account, and system operations remain explicit feature modules. A route-coverage manifest and tests make missing admin surfaces detectable.

**Tech Stack:** React 19, TypeScript, Vite, Vitest, native HTML forms/dialogs, lucide-react, Nginx, Docker Compose.

---

## File map

- `apps/frontend/src/features/admin/resources/types.ts` — CRUD registry types and field/value contracts.
- `apps/frontend/src/features/admin/resources/definitions.ts` — every standard and limited collection definition.
- `apps/frontend/src/features/admin/resources/coverage.ts` — mutable route-to-interface coverage matrix.
- `apps/frontend/src/features/admin/resources/values.ts` — JSON:API relationship extraction and form serialization.
- `apps/frontend/src/features/admin/resources/ResourceForm.tsx` — semantic typed create/edit form.
- `apps/frontend/src/features/admin/resources/AdminResourceWorkspace.tsx` — list/search/page/create/edit/delete shell.
- `apps/frontend/src/features/admin/AdminDataNavigation.tsx` — grouped resource selection.
- `apps/frontend/src/features/admin/AdminCurriculum.tsx` — group semesters, periods, and load distribution.
- `apps/frontend/src/features/admin/AdminScheduling.tsx` — events, teacher bindings, transfers, constraints, recommendations.
- `apps/frontend/src/features/admin/AdminFiles.tsx` — Excel/media upload, sanitation status, URL, deletion.
- `apps/frontend/src/features/admin/AdminSystem.tsx` — audit, materialized views, refresh.
- `apps/frontend/src/lib/api.ts` — generic CRUD and explicit nested transport.
- `apps/frontend/src/styles.css` — replacement minimalist design system.
- `apps/frontend/src/App.tsx` — removal of legacy duplicate inline screens and unified shell.

### Task 1: Define and prove admin route coverage

**Files:**
- Create: `apps/frontend/src/features/admin/resources/types.ts`
- Create: `apps/frontend/src/features/admin/resources/coverage.ts`
- Create: `apps/frontend/src/features/admin/resources/coverage.test.ts`

- [ ] **Step 1: Write the failing coverage test**

```ts
import { describe, expect, it } from "vitest";
import { adminMutationCoverage } from "./coverage";

describe("admin mutation coverage", () => {
  it("maps every gateway mutation to an administrator surface", () => {
    expect(adminMutationCoverage.filter((route) => !route.surface)).toEqual([]);
    expect(new Set(adminMutationCoverage.map((route) => `${route.method} ${route.path}`)).size)
      .toBe(adminMutationCoverage.length);
  });
});
```

- [ ] **Step 2: Run the test and verify RED**

Run: `npm test -- src/features/admin/resources/coverage.test.ts`

Expected: FAIL because `coverage.ts` does not exist.

- [ ] **Step 3: Add the shared types and complete coverage manifest**

```ts
export type AdminMutationMethod = "POST" | "PATCH" | "DELETE";
export type AdminMutationCoverage = {
  method: AdminMutationMethod;
  path: string;
  surface: "resource" | "accounts" | "curriculum" | "scheduling" | "calendar" | "files" | "system";
};

export type AdminResourceName =
  | "semesters" | "buildings" | "faculties" | "kafedras" | "student_groups"
  | "subjects" | "teachers" | "teacher_on_kafedras" | "teaching_assignments"
  | "time_slots" | "week_types" | "study_load_requirements" | "room_types"
  | "activity_types" | "building_types" | "rooms" | "aviability_types"
  | "room_avialabilities" | "posts" | "academic_titles" | "academic_degrees"
  | "curriculum_streams" | "schedule_events" | "specialities"
  | "assignment_room_references" | "holidays" | "weekday_transfers"
  | "room_reservations" | "media_files" | "audit_logs";
```

Populate `adminMutationCoverage` with every POST/PATCH/DELETE route in `config/routes.rb`, including accounts, uploads, student-group semesters/load/periods, event teachers/transfers, recommendations, constraints, reservations, holidays, weekday transfers, files, and view refresh.

- [ ] **Step 4: Run the coverage test and verify GREEN**

Run: `npm test -- src/features/admin/resources/coverage.test.ts`

Expected: PASS with no uncovered or duplicate mutation route.

- [ ] **Step 5: Commit**

```powershell
git add apps/frontend/src/features/admin/resources/types.ts apps/frontend/src/features/admin/resources/coverage.ts apps/frontend/src/features/admin/resources/coverage.test.ts
git commit -m "test: define complete admin mutation coverage"
```

### Task 2: Build the typed resource definition registry

**Files:**
- Create: `apps/frontend/src/features/admin/resources/definitions.ts`
- Create: `apps/frontend/src/features/admin/resources/definitions.test.ts`
- Create: `apps/frontend/src/features/admin/resources/values.ts`
- Create: `apps/frontend/src/features/admin/resources/values.test.ts`

- [ ] **Step 1: Write failing registry invariants**

```ts
import { describe, expect, it } from "vitest";
import { adminResourceDefinitions } from "./definitions";

describe("adminResourceDefinitions", () => {
  it("defines labels, payload keys, columns, fields, and honest actions", () => {
    for (const definition of adminResourceDefinitions) {
      expect(definition.label.length).toBeGreaterThan(0);
      expect(definition.singular.length).toBeGreaterThan(0);
      expect(definition.columns.length).toBeGreaterThan(0);
      if (definition.actions.includes("create")) expect(definition.createFields.length).toBeGreaterThan(0);
      if (definition.actions.includes("update")) expect(definition.updateFields.length).toBeGreaterThan(0);
    }
    expect(adminResourceDefinitions.find((item) => item.resource === "holidays")?.actions)
      .toEqual(["index", "create", "delete"]);
    expect(adminResourceDefinitions.find((item) => item.resource === "audit_logs")?.actions)
      .toEqual(["index", "show"]);
  });
});
```

- [ ] **Step 2: Run registry tests and verify RED**

Run: `npm test -- src/features/admin/resources/definitions.test.ts`

Expected: FAIL because definitions do not exist.

- [ ] **Step 3: Implement registry types and definitions**

The registry must contain these exact field sets from gateway contracts:

- faculties: `name`, `short_name`;
- semesters: `start_year`, `end_year`, `number`;
- kafedras: `faculty_id`, `name`, `short_name`;
- specialities: `faculty_id`, `name`, `duration_of_study`;
- student_groups create: `speciality_id`, `year_of_admission`, `auto_provision`, `students_now`, `semester_duration`; update: `name`, `students_now`, `year_of_admission`, `faculty_id`;
- teachers create: `post_id`, `academic_degree_id`, `academic_title_id`, `first_name`, `middle_name`, `last_name`, `sovmestitel`; update: same scalar fields;
- teacher_on_kafedras: `teacher_id`, `kafedra_id`, `pochasovoy`;
- posts, academic degrees, academic titles, subjects, activity types, building types: `name`, `short_name`;
- buildings: `building_type_id`, `number`;
- rooms: `building_id`, `room_type_id`, `name`, `number`;
- room_types: `activity_type_id`, `name`;
- aviability_types: `name`;
- room_avialabilities: `room_id`, `aviability_type_id`, `name`, `number`;
- time_slots: `call_number`, `start_time`, `end_time`;
- week_types: `name`, `code`;
- curriculum_streams: `group_semester_id`, `subject_id`, `kafedra_id`, `course`;
- study_load_requirements: `stream_id`, `activity_type_id`, `hours_per_event`, `hours_assigned`, `requires_consecutive_pairs`, `split_strategy`;
- teaching_assignments: `load_requirement_id`, `teacher_on_kafedra_id`;
- assignment_room_references: `teaching_assignment_id`, `room_id`, `priority_level`;
- schedule_events: `load_requirement_id`, `time_slot_id`, `room_id`, `week_type_id`, `status`, `subgroup_number`, `is_manual_change`, `change_reason`, `day_of_week`, `sovmestitel`;
- holidays: `holiday_date`, `name` with no update action;
- weekday_transfers: `transfer_date`, `follows_weekday`, `student_group_id`, `reason` with no update action;
- room_reservations: `room_id`, `teacher_id`, `reserved_date`, `time_slot_id`, `purpose` with no update action;
- media_files and audit_logs: index/show only.

Relationship ids use select fields; booleans use checkboxes; dates and times use native controls; numeric constraints match contracts. Payload keys use the singular contract envelope, including the API spellings `aviability_type` and `room_avialability`.

- [ ] **Step 4: Add value conversion tests and implementation**

```ts
expect(serializeResourceValues(definition, { faculty_id: "2", pochasovoy: true }))
  .toEqual({ faculty_id: 2, pochasovoy: true });
expect(readResourceField(resource, "faculty_id"))
  .toBe("2");
```

Implement numeric conversion, nullable optional strings, booleans, direct attributes, and JSON:API relationship ids.

- [ ] **Step 5: Run registry/value tests and verify GREEN**

Run: `npm test -- src/features/admin/resources`

Expected: PASS for every definition and conversion invariant.

- [ ] **Step 6: Commit**

```powershell
git add apps/frontend/src/features/admin/resources
git commit -m "feat: define admin CRUD resource registry"
```

### Task 3: Generalize API transport and validation errors

**Files:**
- Modify: `apps/frontend/src/lib/api.ts`
- Modify: `apps/frontend/src/lib/api.test.ts`

- [ ] **Step 1: Write failing generic CRUD transport tests**

```ts
await api.createAdminResource("kafedras", "kafedra", { faculty_id: 2, name: "ИТ", short_name: "ИТ" });
await api.updateAdminResource("kafedras", "kafedra", "9", { name: "ИТ" });
await api.deleteAdminResource("kafedras", "9");

expect(fetchMock.mock.calls.map(([url, init]) => [url, init.method, init.body])).toEqual([
  ["/api/admin/kafedras", "POST", JSON.stringify({ kafedra: { faculty_id: 2, name: "ИТ", short_name: "ИТ" } })],
  ["/api/admin/kafedras/9", "PATCH", JSON.stringify({ kafedra: { name: "ИТ" } })],
  ["/api/admin/kafedras/9", "DELETE", undefined],
]);
```

- [ ] **Step 2: Run transport tests and verify RED**

Run: `npm test -- src/lib/api.test.ts`

Expected: FAIL because generic methods and structured errors are absent.

- [ ] **Step 3: Implement generic transport and `ApiError`**

`ApiError` carries `status`, `message`, and `fields: Record<string, string[]>`. Generic methods use encoded ids and `{ [payloadKey]: attributes }`. Keep token refresh behavior unchanged.

- [ ] **Step 4: Add explicit nested methods**

Add transport for group semesters/generation/update/delete, group load create/read/delete, period read/upsert, event teachers, event transfers, teacher constraints, recommendations/materialization, multipart uploads, file list/URL/delete, audit detail/filter, view reads/refresh.

- [ ] **Step 5: Run transport tests and verify GREEN**

Run: `npm test -- src/lib/api.test.ts`

Expected: all route, envelope, multipart, query, and error tests PASS.

- [ ] **Step 6: Commit**

```powershell
git add apps/frontend/src/lib/api.ts apps/frontend/src/lib/api.test.ts
git commit -m "feat: add complete admin API transport"
```

### Task 4: Implement the shared CRUD form and workspace

**Files:**
- Create: `apps/frontend/src/features/admin/resources/ResourceForm.tsx`
- Create: `apps/frontend/src/features/admin/resources/ResourceForm.test.tsx`
- Create: `apps/frontend/src/features/admin/resources/AdminResourceWorkspace.tsx`
- Create: `apps/frontend/src/features/admin/resources/AdminResourceWorkspace.test.tsx`
- Create: `apps/frontend/src/features/admin/resources/RelationSelect.tsx`

- [ ] **Step 1: Write failing form behavior tests**

Render faculty and room definitions. Assert visible labels, required/native types, live relationship select, create/update submit labels, field error association, and no raw id input for configured relations.

- [ ] **Step 2: Run tests and verify RED**

Run: `npm test -- src/features/admin/resources/ResourceForm.test.tsx`

Expected: FAIL because the form components do not exist.

- [ ] **Step 3: Implement semantic forms**

Use `<form>`, `<fieldset>`, `<legend>`, `<label htmlFor>`, stable ids, native inputs, `aria-describedby`, and an error summary. Load relationship options using the relation definition and `api.listAdminCollection`. Keep submit enabled until a valid submit begins.

- [ ] **Step 4: Write failing workspace tests**

Test list loading, search query, create opening, edit prefill, named delete confirmation, post-mutation refetch, limited capability, read-only state, `403`, and `422` field errors.

- [ ] **Step 5: Implement the workspace**

Use a semantic table, sticky header wrapper, page controls, native `<dialog>` where supported through a React ref, and pessimistic mutation state. Resource changes ignore stale request completion.

- [ ] **Step 6: Run workspace tests and verify GREEN**

Run: `npm test -- src/features/admin/resources`

Expected: all form, relation, workspace, registry, and coverage tests PASS.

- [ ] **Step 7: Commit**

```powershell
git add apps/frontend/src/features/admin/resources
git commit -m "feat: build reusable admin CRUD workspace"
```

### Task 5: Replace admin navigation and expose every regular resource

**Files:**
- Create: `apps/frontend/src/features/admin/AdminDataNavigation.tsx`
- Create: `apps/frontend/src/features/admin/AdminDataNavigation.test.tsx`
- Modify: `apps/frontend/src/features/admin/AdminPanel.tsx`
- Replace: `apps/frontend/src/features/admin/AdminResourcePage.tsx`
- Replace: `apps/frontend/src/features/admin/AdminOperations.tsx`

- [ ] **Step 1: Write failing navigation tests**

Assert all six task groups, every registry resource label, role-gated accounts, and read-only labels for media/audit.

- [ ] **Step 2: Run tests and verify RED**

Run: `npm test -- src/features/admin/AdminDataNavigation.test.tsx src/features/admin/AdminPanel.test.tsx`

Expected: FAIL because current navigation exposes only a small subset.

- [ ] **Step 3: Implement grouped navigation and shared workspace routing**

The selected resource renders `AdminResourceWorkspace`. Existing overview, schedule, and workload entries remain direct tasks. Limited collections use the same workspace with their configured action set.

- [ ] **Step 4: Run navigation tests and verify GREEN**

Run: `npm test -- src/features/admin`

Expected: every regular/limited/read-only resource is discoverable and role controls pass.

- [ ] **Step 5: Commit**

```powershell
git add apps/frontend/src/features/admin
git commit -m "feat: expose complete admin resource navigation"
```

### Task 6: Implement curriculum and group-semester workflows

**Files:**
- Create: `apps/frontend/src/features/admin/AdminCurriculum.tsx`
- Create: `apps/frontend/src/features/admin/AdminCurriculum.test.tsx`
- Create: `apps/frontend/src/features/admin/curriculum/GroupSemesterEditor.tsx`
- Create: `apps/frontend/src/features/admin/curriculum/GroupLoadEditor.tsx`
- Create: `apps/frontend/src/features/admin/curriculum/PeriodEditor.tsx`

- [ ] **Step 1: Write failing workflow tests**

Assert group selection, semester list, add/generate/update/delete, load read/create/delete, period read/upsert, required relation controls, and visible API error states.

- [ ] **Step 2: Run tests and verify RED**

Run: `npm test -- src/features/admin/AdminCurriculum.test.tsx`

Expected: FAIL because workflows do not exist.

- [ ] **Step 3: Implement dedicated curriculum workflows**

Use separate panels for semester lifecycle, periods, and bulk load. The load editor renders nested streams, requirements, assignments, and room references as repeatable semantic fieldsets and serializes the exact `streams` contract.

- [ ] **Step 4: Run tests and verify GREEN**

Run: `npm test -- src/features/admin/AdminCurriculum.test.tsx`

Expected: all nested route/envelope/state tests PASS.

- [ ] **Step 5: Commit**

```powershell
git add apps/frontend/src/features/admin/AdminCurriculum.tsx apps/frontend/src/features/admin/AdminCurriculum.test.tsx apps/frontend/src/features/admin/curriculum
git commit -m "feat: add curriculum and group load administration"
```

### Task 7: Implement complete scheduling workflows

**Files:**
- Create: `apps/frontend/src/features/admin/AdminScheduling.tsx`
- Create: `apps/frontend/src/features/admin/AdminScheduling.test.tsx`
- Create: `apps/frontend/src/features/admin/scheduling/EventEditor.tsx`
- Create: `apps/frontend/src/features/admin/scheduling/EventTeachersEditor.tsx`
- Create: `apps/frontend/src/features/admin/scheduling/TransferEditor.tsx`
- Create: `apps/frontend/src/features/admin/scheduling/ConstraintEditor.tsx`
- Create: `apps/frontend/src/features/admin/scheduling/RecommendationEditor.tsx`

- [ ] **Step 1: Write failing scheduling tests**

Assert event list/create/edit/delete, teacher binding create/edit/delete, transfer list/create/delete, constraint view/create/delete, recommendation creation, and materialization into an event.

- [ ] **Step 2: Run tests and verify RED**

Run: `npm test -- src/features/admin/AdminScheduling.test.tsx`

Expected: FAIL because the complete workflow does not exist.

- [ ] **Step 3: Implement scheduling editors**

All day/week/status choices use visible Russian labels. Relations select live load requirements, slots, rooms, week types, and teacher-department links. Manual changes require a reason. Recommendation results show score and placement context before materialization.

- [ ] **Step 4: Run tests and verify GREEN**

Run: `npm test -- src/features/admin/AdminScheduling.test.tsx`

Expected: every scheduling mutation and error-state test PASS.

- [ ] **Step 5: Commit**

```powershell
git add apps/frontend/src/features/admin/AdminScheduling.tsx apps/frontend/src/features/admin/AdminScheduling.test.tsx apps/frontend/src/features/admin/scheduling
git commit -m "feat: add complete scheduling administration"
```

### Task 8: Complete file, account, audit, and system operations

**Files:**
- Create: `apps/frontend/src/features/admin/AdminFiles.tsx`
- Create: `apps/frontend/src/features/admin/AdminFiles.test.tsx`
- Create: `apps/frontend/src/features/admin/AdminSystem.tsx`
- Create: `apps/frontend/src/features/admin/AdminSystem.test.tsx`
- Modify: `apps/frontend/src/features/admin/AdminAccounts.tsx`
- Modify: `apps/frontend/src/features/admin/AdminAccounts.test.tsx`

- [ ] **Step 1: Write failing operation tests**

Assert Excel/media upload controls, media status polling display, clean URL action, file deletion, account create/edit/status/scope/delete, audit filtering/detail, teaching-load/constraint/reservation views, and refresh action.

- [ ] **Step 2: Run tests and verify RED**

Run: `npm test -- src/features/admin/AdminFiles.test.tsx src/features/admin/AdminAccounts.test.tsx src/features/admin/AdminSystem.test.tsx`

Expected: FAIL for missing workflows and incomplete account edit/scope behavior.

- [ ] **Step 3: Implement file, account, and system modules**

Use multipart `FormData` without forcing JSON content type. Poll only quarantined/scanning files while the page is active. Preserve one-time generated passwords only in transient state. Account scope controls change with role. Audit before/after JSON strings are parsed for readable diff display without becoming editable.

- [ ] **Step 4: Run tests and verify GREEN**

Run: `npm test -- src/features/admin/AdminFiles.test.tsx src/features/admin/AdminAccounts.test.tsx src/features/admin/AdminSystem.test.tsx`

Expected: all operation tests PASS.

- [ ] **Step 5: Commit**

```powershell
git add apps/frontend/src/features/admin/AdminFiles.tsx apps/frontend/src/features/admin/AdminFiles.test.tsx apps/frontend/src/features/admin/AdminSystem.tsx apps/frontend/src/features/admin/AdminSystem.test.tsx apps/frontend/src/features/admin/AdminAccounts.tsx apps/frontend/src/features/admin/AdminAccounts.test.tsx
git commit -m "feat: complete admin files accounts and system tools"
```

### Task 9: Replace the visual system with readable minimalism

**Files:**
- Replace: `apps/frontend/src/styles.css`
- Modify: `apps/frontend/src/App.tsx`
- Modify: `apps/frontend/src/components/ui.tsx`
- Create: `apps/frontend/src/styles.test.ts`

- [ ] **Step 1: Write failing visual-token tests**

```ts
const css = readFileSync(new URL("./styles.css", import.meta.url), "utf8");
expect(css).toContain("--canvas: #f6f6f3");
expect(css).toContain("font-size: 1.125rem");
expect(css).not.toMatch(/backdrop-filter|radial-gradient|linear-gradient|box-shadow:\s*0 0/);
expect(css).not.toContain("DM Mono");
```

- [ ] **Step 2: Run the test and verify RED**

Run: `npm test -- src/styles.test.ts`

Expected: FAIL because the current CSS is dark, neon, gradient-heavy, and uses tiny type.

- [ ] **Step 3: Replace global tokens and component styles**

Implement the approved palette, type scale, spacing, controls, surfaces, tables, dialogs, statuses, responsive navigation, 200% zoom behavior, focus states, coarse-pointer targets, reduced motion, and optional system dark scheme. Remove ambient/orbit/floating decorative markup and duplicate legacy inline admin/public/self-service components from `App.tsx`.

- [ ] **Step 4: Run visual-token and component tests**

Run: `npm test -- src/styles.test.ts && npm test`

Expected: token test and all component tests PASS.

- [ ] **Step 5: Commit**

```powershell
git add apps/frontend/src/styles.css apps/frontend/src/styles.test.ts apps/frontend/src/App.tsx apps/frontend/src/components/ui.tsx
git commit -m "feat: redesign client with readable minimalism"
```

### Task 10: Perform completion audit and rendered verification

**Files:**
- Modify: `docs/superpowers/plans/2026-07-16-minimalist-system-full-admin-crud.md`

- [ ] **Step 1: Run fresh automated verification**

Run:

```powershell
cd apps/frontend
npm test
npm run lint
npm run build
cd ../..
docker-compose --env-file .env.local up -d --build
```

Expected: zero test failures, TypeScript errors, build errors, or container startup failures.

- [ ] **Step 2: Verify the route coverage matrix**

Compare `adminMutationCoverage` to `apps/api_geteway/config/routes.rb` and confirm every POST/PATCH/DELETE route has a named interface surface. Confirm read-only routes have no mutation controls.

- [ ] **Step 3: Exercise live API safely**

Log in with the supplied administrator credentials. For each standard resource, perform create/update/delete using uniquely prefixed QA records where constraints permit, then remove the record. Exercise create/delete-only operations with disposable future-dated records. Do not mutate existing records.

- [ ] **Step 4: Run in-app browser QA**

Verify page identity, meaningful DOM, no framework overlay, console health, navigation, one full standard CRUD cycle, one limited create/delete cycle, one nested workflow, desktop 1440×900, tablet 768×1024, mobile 390×844, keyboard navigation, and 200% text zoom. Capture desktop and mobile evidence outside the repository.

- [ ] **Step 5: Mark each checkbox only from evidence**

Update this plan’s checkboxes after commands and browser observations prove each item. Keep the goal active if any resource or workflow remains unimplemented or unverified.

---

## Plan self-review result

- The plan covers every resource and nested mutation category in the approved specification.
- API spellings, payload envelope names, and method names are consistent across tasks.
- Read-only resources are explicitly excluded from mutation controls.
- No placeholder implementation steps remain.
