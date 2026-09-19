# Local Nginx and API-Driven Scheduling Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver a locally cached Nginx frontend whose authentication, public schedules, teacher self-service, and core administration screens are driven by the existing API.

**Architecture:** Nginx serves Vite's hashed assets with immutable cache headers and proxies same-origin `/api` requests to Hanami without caching. React is split only along the features changed here: shared schedule normalization, public schedules, teacher self-service, and API-driven admin modules. The live route file and 2026-07-15 API reference remain authoritative.

**Tech Stack:** React 19, TypeScript, Vite, Nginx, Docker Compose, Vitest, Playwright CLI, Hanami API

---

## File Map

- Modify `apps/frontend/nginx.conf`: static cache policy, SPA fallback, compression, API proxy, stream buffering.
- Modify `apps/frontend/vite.config.ts`: same-origin development proxy and test configuration.
- Modify `apps/frontend/package.json`: unit-test scripts and Vitest dependency.
- Modify `apps/frontend/src/lib/api.ts`: `/api` base, typed errors, token lifecycle, public/teacher/admin endpoint methods.
- Create `apps/frontend/src/lib/api.test.ts`: transport and URL contract tests.
- Create `apps/frontend/src/features/schedule/types.ts`: shared JSON:API and normalized schedule types.
- Create `apps/frontend/src/features/schedule/normalize.ts`: live schedule response normalization.
- Create `apps/frontend/src/features/schedule/normalize.test.ts`: schedule fixture tests.
- Create `apps/frontend/src/features/schedule/WeekGrid.tsx`: accessible shared schedule table.
- Create `apps/frontend/src/features/schedule/PublicSchedulePage.tsx`: cascading API selectors and public schedules.
- Create `apps/frontend/src/features/teacher/TeacherSelfService.tsx`: structured teacher schedule, workload, and department views.
- Create `apps/frontend/src/features/admin/AdminOverview.tsx`: API-driven operational summary.
- Create `apps/frontend/src/features/admin/AdminResourcePage.tsx`: live JSON:API table and supported editors.
- Create `apps/frontend/src/features/admin/AdminAccounts.tsx`: real `/auth/accounts` list and account status view.
- Modify `apps/frontend/src/App.tsx`: compose extracted feature pages and role-aware navigation.
- Modify `apps/frontend/src/styles.css`: styles for new schedule tables, API states, teacher lists, and admin data tables.

### Task 1: Nginx cache policy and same-origin API

**Files:**
- Modify: `apps/frontend/nginx.conf`
- Modify: `apps/frontend/vite.config.ts`
- Modify: `apps/frontend/src/lib/api.ts`

- [ ] **Step 1: Add same-origin Vite development proxy**

Configure Vite so browser requests to `/api/*` reach the local API with the prefix removed:

```ts
server: {
  port: 4173,
  strictPort: true,
  proxy: {
    "/api": {
      target: "http://localhost:2300",
      changeOrigin: true,
      rewrite: (path) => path.replace(/^\/api/, ""),
    },
  },
},
```

- [ ] **Step 2: Make `/api` the production-safe default**

In `src/lib/api.ts`, use:

```ts
const API_BASE_URL = (import.meta.env.VITE_API_BASE_URL as string | undefined)?.replace(/\/$/, "") ?? "/api";
```

- [ ] **Step 3: Configure Nginx static delivery and API proxy**

Implement distinct locations for immutable hashed assets, uncached HTML, same-origin API, SSE, and SPA routes. The asset location must return `404` for missing files instead of serving `index.html`.

- [ ] **Step 4: Build and inspect generated filenames**

Run: `npm run build`

Expected: Vite emits content-hashed files under `dist/assets` and TypeScript exits successfully.

- [ ] **Step 5: Build Docker and verify cache headers**

Run: `docker-compose --env-file .env.local up -d --build frontend`

Check `http://localhost:4173/`, one emitted `/assets/*` URL, and `/api/public/faculties`.

Expected: HTML has `no-cache`; assets have `max-age=31536000, immutable`; API has `no-store` and returns JSON.

### Task 2: Typed API transport and schedule normalization

**Files:**
- Modify: `apps/frontend/package.json`
- Modify: `apps/frontend/src/lib/api.ts`
- Create: `apps/frontend/src/lib/api.test.ts`
- Create: `apps/frontend/src/features/schedule/types.ts`
- Create: `apps/frontend/src/features/schedule/normalize.ts`
- Create: `apps/frontend/src/features/schedule/normalize.test.ts`

- [ ] **Step 1: Add Vitest**

Add `"test": "vitest run"` to scripts and `"vitest": "^3.2.4"` to development dependencies, then run `npm install` to update the lockfile.

- [ ] **Step 2: Write failing normalization tests**

Use a JSON:API fixture containing `day_of_week`, `time_slot_start_time`, `subject_name`, `teachers`, and `is_manual_change`. Assert that `normalizeSchedule()` groups rows by day, sorts events by start time, and preserves the change flag. Add a legacy `data.days` fixture to protect compatibility.

- [ ] **Step 3: Run the normalization test and confirm failure**

Run: `npm test -- src/features/schedule/normalize.test.ts`

Expected: FAIL because the module does not exist.

- [ ] **Step 4: Implement shared schedule types and normalizer**

Define `ApiResource`, `ScheduleEvent`, and `ScheduleDay`. Implement pure helpers for day labels, nested/live field extraction, teacher/group display, time sorting, and changed-event detection.

- [ ] **Step 5: Run normalization tests**

Run: `npm test -- src/features/schedule/normalize.test.ts`

Expected: PASS.

- [ ] **Step 6: Write API URL and error tests**

Mock `fetch` and assert that room schedule includes `semester_id`, authentication uses `/api/auth/sessions`, an error response becomes a typed `ApiError`, and a `401` refresh rotates stored tokens before retrying once.

- [ ] **Step 7: Implement typed transport behavior**

Add `ApiError` with `status` and parsed body, session helpers, token clearing after failed refresh, and endpoint methods matching `routes.rb`. Change `roomSchedule(roomId, semesterId)` to include required semester context.

- [ ] **Step 8: Run transport tests and type checks**

Run: `npm test -- src/lib/api.test.ts && npm run lint`

Expected: PASS with no TypeScript errors.

### Task 3: API-driven public schedule pages

**Files:**
- Create: `apps/frontend/src/features/schedule/WeekGrid.tsx`
- Create: `apps/frontend/src/features/schedule/PublicSchedulePage.tsx`
- Modify: `apps/frontend/src/App.tsx`
- Modify: `apps/frontend/src/styles.css`

- [ ] **Step 1: Build the shared week grid**

Render normalized days in a semantic table with day columns and event rows. Display time, subject, activity, room, and teacher/group. Use a changed-event class and Russian label `Изменено` when the normalizer marks an event changed.

- [ ] **Step 2: Build cascading API selectors**

Load faculties and semesters with `Promise.all`. Load groups only after faculty and semester are selected. Load teacher and room choices from their public routes. Reset dependent selections when a parent changes.

- [ ] **Step 3: Add explicit request states**

Provide Russian loading, retryable error, validation, empty, and loaded states. Do not render seed schedule rows or sample metrics.

- [ ] **Step 4: Connect public routes in `App.tsx`**

Replace the inline public schedule implementation with the feature component while preserving the existing landing-page navigation and visual system.

- [ ] **Step 5: Verify public API behavior**

Run: `npm run build && npm test`

Expected: build and unit tests pass. Through Nginx, faculty/semester selectors populate from `/api/public/*`, and valid empty schedules render an empty state.

### Task 4: Structured teacher self-service

**Files:**
- Create: `apps/frontend/src/features/teacher/TeacherSelfService.tsx`
- Modify: `apps/frontend/src/App.tsx`
- Modify: `apps/frontend/src/styles.css`

- [ ] **Step 1: Render the teacher schedule through `WeekGrid`**

Fetch `/admin/me/schedule` when the schedule tab opens and normalize the live response.

- [ ] **Step 2: Replace raw workload JSON**

Render workload records as rows containing subject, activity type, assigned hours, group/stream context, and semester where provided. Unknown optional fields use a Russian em dash, not object stringification.

- [ ] **Step 3: Replace raw department JSON**

Render `/admin/me/kafedras` resources as department name, short name, faculty context, and assignment metadata when present.

- [ ] **Step 4: Add per-tab retry behavior**

Keep each tab's last successful response in component state, show a retry button after failure, and avoid refetching unrelated tabs.

- [ ] **Step 5: Connect teacher routing and verify**

Ensure JWT role `teacher` enters the teacher shell, logout uses the API, and no admin navigation is shown. Run `npm run build && npm test` and exercise a teacher-shaped mocked response in browser QA.

### Task 5: API-driven admin panel

**Files:**
- Create: `apps/frontend/src/features/admin/AdminOverview.tsx`
- Create: `apps/frontend/src/features/admin/AdminResourcePage.tsx`
- Create: `apps/frontend/src/features/admin/AdminAccounts.tsx`
- Modify: `apps/frontend/src/lib/api.ts`
- Modify: `apps/frontend/src/App.tsx`
- Modify: `apps/frontend/src/styles.css`

- [ ] **Step 1: Extend admin endpoint methods**

Add typed methods for paginated resource lists, audit logs, media files, holidays, weekday transfers, room reservations, accounts, nested schedule projections, and workload overview. Use only paths present in `routes.rb`.

- [ ] **Step 2: Build the live overview**

Load semesters, faculties, schedule load overview for the selected semester, and permitted operational collections in parallel. Render actual counts and status summaries only after responses arrive; otherwise show unavailable/empty states.

- [ ] **Step 3: Build reusable resource tables**

Render JSON:API resources with stable Russian column labels, pagination links when present, loading/error/empty states, and existing faculty/semester write dialogs. Preserve `meta.actions` for capability-aware controls.

- [ ] **Step 4: Connect account administration**

Load `/auth/accounts` only for `super_admin`, render email/role/status/scope, and retain API errors as visible states. Other roles must not render the navigation item or initiate the request.

- [ ] **Step 5: Connect schedule and workload selectors**

Require semester selection before admin projections. Send `semester_id` on faculty, department, teacher, room, and load requests. Render schedule results through `WeekGrid` and workload rows through a structured table.

- [ ] **Step 6: Connect calendar and operations lists**

Replace endpoint-description cards with real holidays, weekday transfers, reservations, media status, and audit lists for roles permitted to read them.

- [ ] **Step 7: Verify role-aware admin behavior**

Run `npm run build && npm test`. Login as the supplied administrator through Nginx, confirm dashboard requests resolve, open a reference resource, and confirm the account page loads real API data.

### Task 6: Docker and browser completion audit

**Files:**
- Modify only files implicated by discovered defects.

- [ ] **Step 1: Run the complete local verification suite**

Run: `npm run lint && npm test && npm run build`

Expected: all commands exit `0`.

- [ ] **Step 2: Rebuild the complete stack**

Run: `docker-compose --env-file .env.local up -d --build`

Expected: frontend, API, database, Redis, MinIO, media sanitizer, and Excel processor start; required health checks pass.

- [ ] **Step 3: Verify HTTP contracts**

Check HTML, asset, API, and authentication response headers/statuses without printing token values. Confirm Nginx does not cache API responses.

- [ ] **Step 4: Run desktop browser flow**

Test: landing -> public group/teacher schedule selectors -> staff login -> admin overview -> reference resource -> accounts. Confirm no framework overlay and no relevant console errors.

- [ ] **Step 5: Run mobile browser flow**

At approximately 390x844, check landing, public schedule selectors/grid, login, and admin navigation. Confirm horizontal schedule scrolling stays inside its container and the page body has no accidental overflow.

- [ ] **Step 6: Inspect screenshots and fix regressions**

Compare the rendered screens to the accepted dark operational design, inspect typography, spacing, palette, controls, schedule anatomy, loading/empty states, and responsive behavior, then rerun affected checks after fixes.

- [ ] **Step 7: Complete the requirement audit**

Map every verification item in the design specification to command, HTTP, DOM, console, interaction, or screenshot evidence. Leave the goal active if any required evidence remains missing.
