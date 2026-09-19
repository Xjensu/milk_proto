# Frontend Architecture (React + TypeScript)

## 1. App structure

Feature-folder layout, not type-folder (no global `components/`/`hooks/` dumping ground
for feature-specific code):

```
src/
  app/
    router.tsx              # role-aware route tree, see §2
    providers.tsx           # QueryClientProvider, AuthProvider, ThemeProvider
  shared/
    ui/                      # Radix-based primitives: Button, Select, Dialog, Table...
    api/
      client.ts               # fetch wrapper, see §6
      types.ts                 # generated/hand-maintained types mirroring backend DTOs
    lib/
      formatters.ts            # display-string helpers mirroring 05-backend-patterns.md §5 (client just renders server-sent display_name, never recomputes it)
  features/
    auth/
      api.ts  hooks.ts  store.ts  pages/LoginPage.tsx
    reference-data/            # activity_types, buildings, rooms, posts, etc. (§2 admin CRUD)
      pages/  components/  api.ts
    faculty-structure/         # faculties, kafedras, specialities, teachers
    student-groups/
    curriculum-workload/       # the pivot tables, cell editor, bulk-create wizard
    schedule/                  # student-group / teacher / room schedule pages, recommendation flow
    calendar/                  # holidays, weekday transfers, event transfers, semester periods
    reservations/
    users/                     # admin user & role management
  index.tsx
```

Each feature exports a small public surface (its `pages/`) — cross-feature imports go
through `shared/`, never reach into another feature's internals.

## 2. Routing & role gating

React Router, one route tree with a layout split:

```
/login                          (public)
/schedule/group                 (public — no AuthGuard)
/schedule/teacher                (public)
/schedule/room                   (public)
/admin/*                          (AuthGuard + RoleGuard)
/me/*                              (AuthGuard, any authenticated staff)
```

`RoleGuard` reads the resolved `role_assignments` from the auth store and hides/blocks
routes the user has no capability for, mirroring — not re-implementing — the backend's
`Auth::Capabilities` matrix (`05-backend-patterns.md §8`): the frontend matrix is a
convenience for UX (don't show a "Create Room" button to a `teacher`), the backend is the
actual enforcement boundary. Keep the two matrices in one shared JSON/TS file generated
from (or manually kept in sync with) `02-slice-authentication.md §2` to avoid drift.

## 3. Server state & client state

- **TanStack Query** owns everything that comes from the API: list/detail queries,
  mutations, cache invalidation. Query keys are structured
  (`["workload", "kafedra", kafedraId, semesterIds]`) so a mutation can invalidate
  precisely (e.g. assigning a workload cell invalidates only that
  `["workload", "kafedra", kafedraId, ...]` key, not the whole cache).
- **Zustand** owns UI-only state that must survive navigation within a feature but has no
  server representation: the current faculty/semester/group selection in the cascading
  selectors (§7), open modal state, the pivot table's expanded/collapsed subject rows.
- No Redux, no Context-as-a-store for server data — Context is reserved for genuinely
  cross-cutting concerns (auth session, theme).

## 4. Forms

React Hook Form + Zod. One Zod schema per backend Contract
(`05-backend-patterns.md §3`), colocated with the feature that owns the form. Field-level
error mapping: the backend's `422` envelope
(`{ error: { details: { "room.building_id": [...] } } }`) is mapped onto RHF's
`setError` by a shared `applyServerErrors(form, error.details)` helper in `shared/api` —
implemented once, used by every form's `onSubmit` failure handler.

Large reference-value fields (subject, teacher, room, kafedra pickers) use an async
combobox component (`shared/ui/AsyncSelect`) backed by the relevant `?q=` search endpoint
with debounced input, rather than a plain `<select>` dumping every row — this directly
answers the requirement for "more interactive UI where a field implies a large data set"
(e.g. picking a subject when assigning workload).

## 5. Auth flow

- On app boot, `AuthProvider` calls `GET /auth/me`. If it 401s, render the public routes
  only (no redirect loop — public schedule pages must work with zero auth calls
  succeeding).
- Access token lives **in memory only** (a module-level variable / Zustand store, not
  `localStorage`) — mitigates XSS token theft. Refresh token is the `httpOnly` cookie the
  browser sends automatically; the frontend never reads or stores it directly.
- The API client (§6) transparently retries a `401` once by calling
  `POST /auth/sessions/refresh`, then replays the original request; a second `401` clears
  auth state and routes to `/login`.
- `must_change_password: true` on the `/auth/me` response force-routes to a
  password-change screen before anything else renders.

## 6. API client

Thin `fetch` wrapper (no heavyweight HTTP library needed):

```ts
async function apiFetch<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`${API_BASE}${path}`, {
    ...init,
    credentials: "include", // sends the refresh cookie
    headers: { ...authHeader(), "Content-Type": "application/json", ...init?.headers },
  });
  if (res.status === 401 && !init?._retried) return retryAfterRefresh(path, init);
  if (!res.ok) throw await ApiError.fromResponse(res);
  return res.json();
}
```

**Materialized-view freshness**: the API auto-refreshes its materialized views after
writes to teacher constraints, holidays, weekday transfers, and room reservations
(`03-slice-admin.md §7`) — the frontend does **not** call `POST /admin/views/refresh`
itself. Mutation hooks in `features/calendar` and `features/reservations` only invalidate
the affected TanStack Query keys; because the server-side refresh is debounced, these
hooks use a short (~2s) delayed second invalidation so the occupancy/recommendation views
settle without a manual page reload.

## 7. Key reusable components

- **`CascadingSelector`** — the faculty→semester→group (and faculty→semester only, or
  semester+teacher-search, or date-only) pattern repeats across almost every page
  (`07-frontend-pages.md`). Implemented once as a composable set of linked `AsyncSelect`s
  driven by a small state machine (`{ facultyId, semesterId, groupId }`, each level reset
  when its parent changes), not copy-pasted per page.
- **`PivotTable`** — renders the kafedra/faculty workload tables: dynamic columns from
  `activity_types` in the response, grouped rows by subject, clickable empty/filled cells
  opening the cell-editor `Dialog`. Built generically enough to also render the faculty
  workload's flatter shape (same component, different column config).
- **`WeekGrid`** — renders a schedule as a day/week grid (used by student-group, teacher,
  and room-day views alike), with `is_transferred`/cancelled events visually
  distinguished (different background color, per the ТЗ's explicit highlighting
  requirement).
- **`DatePeriodBadge`** — shows which period (`lecture_week`/`credit_week`/`exam_session`)
  a given date/view falls into.

## 8. Responsiveness & accessibility

Tailwind breakpoints `sm/md/lg/xl`; admin pivot tables and week grids scroll horizontally
within their own container on narrow viewports (never the page body) rather than
collapsing into an unreadable stacked layout — schedules are inherently tabular and
staff/students expect to scan them as a grid even on tablets. Radix primitives provide
keyboard navigation and ARIA semantics for selects/dialogs/comboboxes out of the box;
custom components (`PivotTable`, `WeekGrid`) get explicit `role`/`aria-*` attributes since
they're bespoke grids, not native `<table>` semantics throughout (use real `<table>` markup
where the content is genuinely tabular — the pivot tables and week grids — so screen
readers get row/column semantics for free).
