# schedule_home Frontend Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace `apps/frontend`'s public (unauthenticated) surface — home, group/teacher/room lookup, schedule display — with the exact visual design, copy, and interaction feel of the `schedule_home/` static mockup, driven by the real API instead of the mockup's hardcoded mock data.

**Architecture:** Add `react-router-dom`, mounted only for the public subtree (unauthenticated visitors); `App.tsx`'s existing `view` state machine keeps driving `password`/`admin`/`self` for authenticated staff exactly as today. A shared `PublicLayout` route wrapper (header, footer, and the four shared dialogs — Auth/Contacts/Cookies/Guide-picker, built once) wraps eight route components, each a faithful port of one `schedule_home/*.html` page, wired to the real `api` client instead of mock arrays.

**Tech Stack:** React 19, TypeScript, Vite, `react-router-dom` (new), `gsap`, `lucide-react`, vitest (no jsdom — tests use `react-dom/server`'s `renderToStaticMarkup` plus `vi.stubGlobal`/`vi.spyOn`, never `@testing-library/*`, which is not installed).

## Governing spec

Full approved scope, the deltas-from-mockup table, and rationale live in
`docs/superpowers/specs/2026-08-08-schedule-home-redesign-design.md`. This
plan implements that spec exactly — do not re-litigate any decision in its
"Deltas from the mockup" table. Where this plan makes a call the spec left
open (there is exactly one: how `TeacherDetail`/`RoomDetail` get their
entity's data — see Task 8), the reasoning is spelled out inline.

## Path conventions

- Paths with no leading segment (e.g. `src/lib/api.ts`) are relative to
  `apps/frontend/` — a **separate nested git repo**
  (`C:\Users\Asus\Desktop\milk_schedule\apps\frontend`), currently on branch
  `feature/date-resolved-schedule`. Work directly on that branch; do not
  create a new one. `apps/frontend` has its own git history — commands run
  from inside it, not the outer repo.
- Paths starting `schedule_home/` are relative to the outer repo root
  (`C:\Users\Asus\Desktop\milk_schedule\schedule_home`) — the static mockup,
  read-only reference material. Never edit it.
- All shell commands below assume the working directory is
  `apps/frontend` unless stated otherwise.

## Global Constraints

- `react-router-dom` (latest), mounted only for the public subtree. Routes:
  `/`, `/group`, `/teacher`, `/room`, `/schedule`, `/teacher/:id`,
  `/room/:id`, `/guide` — one component per mockup page, DOM/classes
  reproduced faithfully so the copied CSS applies unmodified. No new visual
  elements, no reordering, no invented markup.
- `schedule_home/styles.css` and `schedule_home/guide.css` are copied
  byte-for-byte into `src/features/public/` and imported only by the public
  route tree. Google Fonts `<link>`s for Manrope + Unbounded are added to
  `index.html`.
- Decorative pixel-art (`teacher-pixel-head` / room floor-plan) is ported as
  static, non-data-driven decoration — the exact hardcoded pixel pattern
  strings from `teacher-detail.js`/`room-detail.js`, never data-driven.
- The Contacts dialog's 3 icons are local `lucide-react` icons (matching the
  rest of this codebase's icon convention), never the mockup's
  `unpkg.com/reicon` CDN script tag.
- Room search and room schedule stay gated behind login
  (`isAuthenticated`/`hasAccessToken()` check) — never made public, unlike
  the mockup.
- There is no "±2 weeks" pager anywhere in the new design. The week toggle
  ("Нечётная"/"Чётная") binds to the real two weeks in
  `OccurrenceWindow.meta.weeks` — never more, never fewer.
- The group picker's dropped 4th step (schedule type) becomes a period-jump
  control on `/schedule` only, group mode only, via the new
  `api.publicGroupPeriods` method (backed by the already-implemented
  `GET /public/groups/:group_semester_id/periods`).
- Teacher/room detail pages show only real fields (teacher
  `post`/`academic_degree`/`academic_title`; room
  `number`/`building.number`/`room_type.name`) — every fabricated mockup
  field (subjects, capacity, equipment, kafedra/department, hardcoded
  profiles) is dropped, not replaced with something invented.
- Every mockup page duplicates the same Auth/Contacts/Cookie dialog HTML +
  JS. That duplication is collapsed into `PublicLayout`, built once in
  Task 2. No later task re-ports that markup or its open/close JS.
- Agents must **not** run `git commit` — this project's convention is that
  the user commits manually. Every task ends with "Stop here, changes are
  ready for review", never a commit step.
- Verification commands (run from `apps/frontend`): `npm run lint` (runs
  `tsc -b --pretty false`) and `npm test` (runs `vitest run`). Do not rely on
  a live browser check as a pass/fail gate in these steps — the plan's
  automated verification is lint + test.
- This repo's vitest setup has **no jsdom and no `@testing-library/*`**.
  Every existing component test uses `renderToStaticMarkup` from
  `react-dom/server` (renders once, synchronously — `useEffect` never runs)
  plus `vi.stubGlobal("sessionStorage", ...)` / `vi.stubGlobal("fetch", ...)`
  where a DOM global is needed. New tests in this plan follow the same
  pattern: assert on synchronous initial markup, and test data-fetching
  logic as **plain exported functions** (never by rendering + waiting for an
  effect to resolve, which does not work under `renderToStaticMarkup`).

---

## Task 1: `api.ts` additions, session/JSON:API helpers, stylesheet copy

**Files:**
- Modify: `src/features/schedule/types.ts` (add `period_kind` to `SemesterContext`)
- Modify: `src/lib/api.ts` (new `publicGroupPeriods` method + `GroupPeriodsResponse` type; widen `listPublicTeachers`/`listPublicRooms` return types and change their signatures)
- Modify: `src/lib/api.test.ts` (new assertions for the above)
- Create: `src/lib/session.ts` (`hasAccessToken()`, extracted so both the old and new public pages can share it)
- Create: `src/lib/session.test.ts`
- Create: `src/features/public/entityFields.ts` (JSON:API relationship-resolution helpers shared by `EntitySearch`, `TeacherDetail`, `RoomDetail`)
- Create: `src/features/public/entityFields.test.ts`
- Create: `src/features/public/styles.css` (byte-for-byte copy of `schedule_home/styles.css`)
- Create: `src/features/public/guide.css` (byte-for-byte copy of `schedule_home/guide.css`)
- Modify: `index.html` (add Manrope/Unbounded font `<link>`s)
- Modify: `package.json` (add `react-router-dom`)

**Interfaces:**
- Produces: `hasAccessToken(): boolean` (from `src/lib/session.ts`) — used by every later task that gates room content.
- Produces: `resolveRelationshipAttributes(resource: JsonApiResource, included: JsonApiResource[] | undefined, key: string): Record<string, unknown> | undefined`, `teacherFullName(attributes): string`, `findResourceById(collection: JsonApiCollection, id: string): JsonApiResource | undefined`, `stringAttribute(attributes, key, fallback?): string` (from `src/features/public/entityFields.ts`).
- Produces: `api.publicGroupPeriods(groupSemesterId: string): Promise<GroupPeriodsResponse>`, `GroupPeriodsResponse = { group_semester_id: number; periods: Array<{ period_kind: PeriodKind; start_date: string; end_date: string }> }` (from `src/lib/api.ts`).
- Produces: `api.listPublicTeachers(query?: string, perPage?: number): Promise<JsonApiCollection>`, `api.listPublicRooms(perPage?: number): Promise<JsonApiCollection>` — **signature changed**: `listPublicRooms` no longer takes a `query` (the server has no `q` filter for rooms — verified against `apps/api_geteway/slices/portal/contracts/rooms/index_contract.rb`, which only allows `page`/`per_page`). Both now default `perPage` to 100 (the server's max, per `IndexContract`'s `lteq?: 100`) so callers get the fullest practical browse list to filter/paginate client-side.
- Consumes: existing `request<T>()`, `JsonApiResource`, `JsonApiCollection`, `PeriodKind` from `src/lib/api.ts`.

### Step 1: Install react-router-dom

Run: `npm install react-router-dom` (from `apps/frontend`).

### Step 2: Write the failing test for `api.publicGroupPeriods`

In `src/lib/api.test.ts`, inside the existing `describe("API transport", ...)` block (which already has a `beforeEach` stubbing `sessionStorage` and restoring mocks), add:

```ts
  it("fetches every configured period for a group-semester from the public (not admin) endpoint", async () => {
    const fetchMock = vi.fn().mockImplementation(() => Promise.resolve(new Response(
      JSON.stringify({ group_semester_id: 513, periods: [{ period_kind: "semester", start_date: "2026-09-01", end_date: "2027-01-01" }] }),
      { status: 200, headers: { "Content-Type": "application/json" } },
    )));
    vi.stubGlobal("fetch", fetchMock);

    const result = await api.publicGroupPeriods("513");

    expect(fetchMock).toHaveBeenCalledWith("/api/public/groups/513/periods", expect.any(Object));
    expect(result.periods[0].period_kind).toBe("semester");
  });

  it("filters public teacher search server-side and defaults to a full page", async () => {
    const fetchMock = vi.fn().mockImplementation(() => Promise.resolve(new Response(JSON.stringify({ data: [] }), { status: 200, headers: { "Content-Type": "application/json" } })));
    vi.stubGlobal("fetch", fetchMock);

    await api.listPublicTeachers();
    await api.listPublicTeachers("Иванов");

    const [firstUrl, secondUrl] = fetchMock.mock.calls.map(([url]) => new URL(String(url), "http://x"));
    expect(firstUrl.pathname).toBe("/api/public/teachers");
    expect(firstUrl.searchParams.get("per_page")).toBe("100");
    expect(firstUrl.searchParams.has("q")).toBe(false);
    expect(secondUrl.searchParams.get("q")).toBe("Иванов");
    expect(secondUrl.searchParams.get("per_page")).toBe("100");
  });

  it("requests every room up to the server's page cap, since /public/rooms has no query filter", async () => {
    const fetchMock = vi.fn().mockImplementation(() => Promise.resolve(new Response(JSON.stringify({ data: [] }), { status: 200, headers: { "Content-Type": "application/json" } })));
    vi.stubGlobal("fetch", fetchMock);

    await api.listPublicRooms();

    expect(fetchMock).toHaveBeenCalledWith("/api/public/rooms?per_page=100", expect.any(Object));
  });
```

### Step 3: Run the tests to see them fail

Run: `npm test -- src/lib/api.test.ts`
Expected: FAIL — `api.publicGroupPeriods is not a function`, and the teacher/room URL assertions fail against the current `?q=` implementation.

### Step 4: Add `period_kind` to `SemesterContext`

In `src/features/schedule/types.ts`, change:

```ts
export type SemesterContext = {
  group_semester_id?: number;
  semester_id: number;
  number: number;
  label: string;
  start_date: string;
  end_date: string;
};
```

to:

```ts
export type SemesterContext = {
  group_semester_id?: number;
  semester_id: number;
  number: number;
  label: string;
  start_date: string;
  end_date: string;
  period_kind: PeriodKind;
};
```

This file doesn't import `PeriodKind` today — the type actually lives in `src/lib/api.ts`. Add the import: `import type { OccurrenceState, PeriodKind, SemesterContext as _unused, TransferNotice } from "../../lib/api";` — actually simpler: the existing top-of-file import is `import type { OccurrenceState, SemesterContext, TransferNotice } from "../../lib/api";` in `WeekGrid.tsx`, but `SemesterContext` itself is **defined** in `src/lib/api.ts` at lines 48-55 (re-read the file to confirm before editing — this plan was written against that layout). Add `PeriodKind` is already exported from `src/lib/api.ts` (it's defined there, not in `types.ts`), so edit `src/lib/api.ts` directly, not `types.ts` — the `SemesterContext` type this task modifies lives in `src/lib/api.ts`, not `src/features/schedule/types.ts` (that file only imports `SemesterContext`, it doesn't declare it). Correct file to modify: `src/lib/api.ts`.

### Step 5: Add `publicGroupPeriods` and widen teacher/room list methods

In `src/lib/api.ts`, near the existing `PERIOD_LABELS` constant, add:

```ts
export type GroupPeriodsResponse = {
  group_semester_id: number;
  periods: Array<{ period_kind: PeriodKind; start_date: string; end_date: string }>;
};
```

Replace the existing `listPublicTeachers`/`listPublicRooms` methods:

```ts
  listPublicTeachers(query = "") {
    return request<{ data: Array<{ id: string; attributes?: Record<string, unknown> }> }>(`/public/teachers${query ? `?q=${encodeURIComponent(query)}` : ""}`);
  },
  listPublicRooms(query = "") {
    return request<{ data: Array<{ id: string; attributes?: Record<string, unknown> }> }>(`/public/rooms${query ? `?q=${encodeURIComponent(query)}` : ""}`);
  },
```

with:

```ts
  /**
   * `perPage` defaults to the server's max (100) so the caller gets the
   * fullest practical browse list; pass `query` to filter server-side via
   * trigram search (rejects a 1-character `q` with 422).
   */
  listPublicTeachers(query = "", perPage = 100) {
    const params = new URLSearchParams({ per_page: String(perPage) });
    if (query) params.set("q", query);
    return request<JsonApiCollection>(`/public/teachers?${params.toString()}`);
  },
  /**
   * /public/rooms has no server-side `q` filter — only `page`/`per_page`
   * (see apps/api_geteway/slices/portal/contracts/rooms/index_contract.rb).
   * Callers filter the returned list client-side (see EntitySearch, Task 5).
   */
  listPublicRooms(perPage = 100) {
    return request<JsonApiCollection>(`/public/rooms?per_page=${perPage}`);
  },
```

Every existing call site of `listPublicTeachers`/`listPublicRooms` is inside `src/features/schedule/PublicSchedulePage.tsx`, which Task 10 deletes entirely — no other call sites exist today, so this is a safe signature change. Confirm with: `grep -rn "listPublicTeachers\|listPublicRooms" src` before editing, and if any other call site turns up, update it to match the new signature.

Add the new method near the existing (admin-scoped) `groupPeriods` method — give it a distinct name since `groupPeriods` is already taken by a different, JSON:API-shaped, `/admin`-scoped method:

```ts
  /**
   * Every configured period (semester/lecture_week/credit_week/exam_session)
   * for one group-semester, unfiltered by date — powers the public schedule
   * page's period-jump control. NOT the same as the admin-scoped
   * `groupPeriods` above (different URL, different response shape: plain
   * JSON here, JSON:API there).
   */
  publicGroupPeriods(groupSemesterId: string) {
    return request<GroupPeriodsResponse>(`/public/groups/${encodeURIComponent(groupSemesterId)}/periods`);
  },
```

### Step 6: Run the tests to see them pass

Run: `npm test -- src/lib/api.test.ts`
Expected: PASS.

### Step 7: Write the failing test for `session.ts`

Create `src/lib/session.test.ts`:

```ts
import { afterEach, describe, expect, it, vi } from "vitest";
import { hasAccessToken } from "./session";

class MemoryStorage implements Storage {
  private values = new Map<string, string>();
  get length() { return this.values.size; }
  clear() { this.values.clear(); }
  getItem(key: string) { return this.values.get(key) ?? null; }
  key(index: number) { return [...this.values.keys()][index] ?? null; }
  removeItem(key: string) { this.values.delete(key); }
  setItem(key: string, value: string) { this.values.set(key, value); }
}

afterEach(() => { vi.unstubAllGlobals(); });

describe("hasAccessToken", () => {
  it("is false with no token in sessionStorage", () => {
    vi.stubGlobal("sessionStorage", new MemoryStorage());
    expect(hasAccessToken()).toBe(false);
  });

  it("is true once a token is stored", () => {
    const storage = new MemoryStorage();
    storage.setItem("milk_schedule_access_token", "token-value");
    vi.stubGlobal("sessionStorage", storage);
    expect(hasAccessToken()).toBe(true);
  });
});
```

### Step 8: Run it to see it fail

Run: `npm test -- src/lib/session.test.ts`
Expected: FAIL — cannot find module `./session`.

### Step 9: Implement `session.ts`

Create `src/lib/session.ts`:

```ts
const ACCESS_TOKEN_KEY = "milk_schedule_access_token";

export function hasAccessToken(): boolean {
  if (typeof sessionStorage === "undefined") return false;
  try { return Boolean(sessionStorage.getItem(ACCESS_TOKEN_KEY)); } catch { return false; }
}
```

### Step 10: Run it to see it pass

Run: `npm test -- src/lib/session.test.ts`
Expected: PASS.

### Step 11: Write the failing test for `entityFields.ts`

Create `src/features/public/entityFields.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { findResourceById, resolveRelationshipAttributes, stringAttribute, teacherFullName } from "./entityFields";
import type { JsonApiCollection, JsonApiResource } from "../../lib/api";

describe("resolveRelationshipAttributes", () => {
  it("follows a belongs_to relationship into the included array", () => {
    const resource: JsonApiResource = { id: "41", type: "teacher", relationships: { post: { data: { id: "3", type: "post" } } } };
    const included: JsonApiResource[] = [{ id: "3", type: "post", attributes: { name: "доцент", short_name: "доц." } }];
    expect(resolveRelationshipAttributes(resource, included, "post")).toEqual({ name: "доцент", short_name: "доц." });
  });

  it("returns undefined when the relationship has no data or included is missing", () => {
    const resource: JsonApiResource = { id: "41", type: "teacher", relationships: { post: { data: null } } };
    expect(resolveRelationshipAttributes(resource, [], "post")).toBeUndefined();
    expect(resolveRelationshipAttributes(resource, undefined, "post")).toBeUndefined();
  });
});

describe("teacherFullName", () => {
  it("joins last, first, middle name and skips blanks", () => {
    expect(teacherFullName({ last_name: "Иванов", first_name: "Пётр", middle_name: "Ильич" })).toBe("Иванов Пётр Ильич");
    expect(teacherFullName({ last_name: "Иванов", first_name: "Пётр", middle_name: null })).toBe("Иванов Пётр");
    expect(teacherFullName(undefined)).toBe("");
  });
});

describe("findResourceById", () => {
  it("finds a resource by id in a collection", () => {
    const collection: JsonApiCollection = { data: [{ id: "1", type: "room" }, { id: "2", type: "room" }] };
    expect(findResourceById(collection, "2")?.id).toBe("2");
    expect(findResourceById(collection, "9")).toBeUndefined();
  });
});

describe("stringAttribute", () => {
  it("stringifies numbers and falls back for missing values", () => {
    expect(stringAttribute({ number: 214 }, "number")).toBe("214");
    expect(stringAttribute({}, "number", "—")).toBe("—");
  });
});
```

### Step 12: Run it to see it fail

Run: `npm test -- src/features/public/entityFields.test.ts`
Expected: FAIL — cannot find module `./entityFields`.

### Step 13: Implement `entityFields.ts`

Create `src/features/public/entityFields.ts`:

```ts
import type { JsonApiCollection, JsonApiResource } from "../../lib/api";

type RelationshipRef = { id: string; type: string } | null;
type RelationshipsShape = Record<string, { data?: RelationshipRef } | undefined>;

/**
 * Resolves a JSON:API `belongs_to` relationship (e.g. a teacher's `post`)
 * against the response's `included` array. Returns the related resource's
 * `attributes`, or undefined when the relationship is absent or not
 * sideloaded.
 */
export function resolveRelationshipAttributes(resource: JsonApiResource, included: JsonApiResource[] | undefined, key: string): Record<string, unknown> | undefined {
  const relationships = resource.relationships as RelationshipsShape | undefined;
  const ref = relationships?.[key]?.data;
  if (!ref || !included) return undefined;
  return included.find((item) => item.id === ref.id && item.type === ref.type)?.attributes;
}

export function teacherFullName(attributes: Record<string, unknown> | undefined): string {
  if (!attributes) return "";
  return [attributes.last_name, attributes.first_name, attributes.middle_name].filter(Boolean).join(" ");
}

export function findResourceById(collection: JsonApiCollection, id: string): JsonApiResource | undefined {
  return collection.data.find((item) => item.id === id);
}

export function stringAttribute(attributes: Record<string, unknown> | undefined, key: string, fallback = ""): string {
  const value = attributes?.[key];
  return typeof value === "string" || typeof value === "number" ? String(value) : fallback;
}
```

### Step 14: Run it to see it pass

Run: `npm test -- src/features/public/entityFields.test.ts`
Expected: PASS.

### Step 15: Copy the mockup stylesheets verbatim

Run (from the outer repo root, `C:\Users\Asus\Desktop\milk_schedule`):

```
cp schedule_home/styles.css apps/frontend/src/features/public/styles.css
cp schedule_home/guide.css apps/frontend/src/features/public/guide.css
```

Do not hand-retype these files — copy them exactly. Do not import either file yet; Task 2 wires the import.

### Step 16: Add the Google Fonts links

In `index.html`, inside `<head>`, add (matching every `schedule_home/*.html` file's `<head>` verbatim):

```html
    <link rel="preconnect" href="https://fonts.googleapis.com" />
    <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin />
    <link href="https://fonts.googleapis.com/css2?family=Manrope:wght@400;500;600;700;800&family=Unbounded:wght@500;600;700;800&display=swap" rel="stylesheet" />
```

### Step 17: Run the full test suite and lint to confirm nothing else broke

Run: `npm test` and `npm run lint`
Expected: both PASS (the deleted-method-signature grep in Step 5 should have caught any stray call sites; if `npm run lint` reports a type error at a `listPublicTeachers`/`listPublicRooms` call site, fix that call site now).

### Step 18: Stop here, changes are ready for review.

---

## Task 2: `PublicLayout` + shared dialogs + router mount in `App.tsx`

This is the foundational shell every later page task builds on: the header,
footer, and the four dialogs (Auth/Contacts/Cookies/Guide-picker) that every
mockup page duplicates get built **once** here. No later task re-ports that
dialog markup or its open/close JS.

**Files:**
- Create: `src/features/public/bodyClass.ts` (DOM body-class hooks every page needs — `<body>` classes drive the mockup's own CSS, e.g. `body.modal-open { overflow: hidden; }`, `body.schedule-page.footer-visible .site-footer { ... }`)
- Create: `src/features/public/cookieConsent.ts` + `src/features/public/cookieConsent.test.ts` (ported from `schedule_home/script.js` lines 132-171)
- Create: `src/features/public/AuthDialog.tsx` + `src/features/public/AuthDialog.test.tsx`
- Create: `src/features/public/ContactsDialog.tsx`
- Create: `src/features/public/CookieDialog.tsx`
- Create: `src/features/public/GuideDialog.tsx`
- Create: `src/features/public/PublicLayout.tsx` + `src/features/public/PublicLayout.test.tsx`
- Create: `src/features/public/PublicApp.tsx` (router mount; gets one new `<Route>` added per later task)
- Modify: `src/App.tsx` (mount `PublicApp` in place of the old `isPublicView` branch; reset the URL to `/` on logout)

**Interfaces:**
- Consumes: `hasAccessToken` (unused here, but every later task that touches `PublicLayout`'s siblings will), `api.login`, `accountFromAccessToken`, `Account` from Task 1 / `src/lib/api.ts`.
- Produces: `useBodyClass(className: string | null)`, `useFooterVisibility()`, `useModalOpenBodyClass(isAnyOpen: boolean)` (from `bodyClass.ts`) — every subsequent page component task calls `useBodyClass` with its own page-type class and `useFooterVisibility()` once.
- Produces: `PublicOutletContext = { openGuide: () => void }` — page components read this via `useOutletContext<PublicOutletContext>()` when they need to open the guide-picker dialog (only `PublicHome`, Task 3, does).
- Produces: `<PublicApp onLoggedIn={(account: Account) => void} />` — mounted from `App.tsx`.

### Step 1: Write the failing test for `cookieConsent.ts`'s pure helpers

Create `src/features/public/cookieConsent.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { buildCookiePreferences, buildCookieRemoval, buildCookieString } from "./cookieConsent";

describe("buildCookieString", () => {
  it("encodes the value and sets a one-year max-age by default", () => {
    expect(buildCookieString("schedule_functional", "1")).toBe("schedule_functional=1; max-age=31536000; path=/; SameSite=Lax");
  });
});

describe("buildCookieRemoval", () => {
  it("zeroes max-age to delete the cookie", () => {
    expect(buildCookieRemoval("schedule_analytics")).toBe("schedule_analytics=; max-age=0; path=/; SameSite=Lax");
  });
});

describe("buildCookiePreferences", () => {
  it("always marks necessary true and stamps the given time", () => {
    const now = new Date("2026-08-08T12:00:00.000Z");
    expect(buildCookiePreferences(true, false, now)).toEqual({ necessary: true, functional: true, analytics: false, updatedAt: "2026-08-08T12:00:00.000Z" });
  });
});
```

### Step 2: Run it to see it fail

Run: `npm test -- src/features/public/cookieConsent.test.ts`
Expected: FAIL — cannot find module `./cookieConsent`.

### Step 3: Implement `cookieConsent.ts`

Create `src/features/public/cookieConsent.ts` — the pure string-building logic is separated from the `document.cookie`/`localStorage` writes so the logic stays unit-testable without jsdom (this codebase has none):

```ts
export type CookiePreferences = { necessary: true; functional: boolean; analytics: boolean; updatedAt: string };

const COOKIE_PREFERENCES_KEY = "schedule-cookie-preferences";

export function buildCookiePreferences(functional: boolean, analytics: boolean, now = new Date()): CookiePreferences {
  return { necessary: true, functional, analytics, updatedAt: now.toISOString() };
}

export function buildCookieString(name: string, value: string, days = 365): string {
  const maxAge = days * 24 * 60 * 60;
  return `${name}=${encodeURIComponent(value)}; max-age=${maxAge}; path=/; SameSite=Lax`;
}

export function buildCookieRemoval(name: string): string {
  return `${name}=; max-age=0; path=/; SameSite=Lax`;
}

export function readCookiePreferences(): Partial<CookiePreferences> {
  try {
    const raw = localStorage.getItem(COOKIE_PREFERENCES_KEY);
    return raw ? (JSON.parse(raw) as Partial<CookiePreferences>) : {};
  } catch {
    return {};
  }
}

/**
 * Persists the visitor's choice: localStorage (source of truth for the
 * dialog's checkboxes) plus three real cookies, mirroring
 * schedule_home/script.js's persistCookiePreferences exactly —
 * schedule_cookie_consent is always set; schedule_functional/
 * schedule_analytics are set or removed per choice.
 */
export function persistCookiePreferences(functional: boolean, analytics: boolean): void {
  const preferences = buildCookiePreferences(functional, analytics);
  localStorage.setItem(COOKIE_PREFERENCES_KEY, JSON.stringify(preferences));
  document.cookie = buildCookieString("schedule_cookie_consent", "saved");
  document.cookie = functional ? buildCookieString("schedule_functional", "1") : buildCookieRemoval("schedule_functional");
  document.cookie = analytics ? buildCookieString("schedule_analytics", "1") : buildCookieRemoval("schedule_analytics");
}
```

### Step 4: Run it to see it pass

Run: `npm test -- src/features/public/cookieConsent.test.ts`
Expected: PASS.

### Step 5: Implement `bodyClass.ts` (no dedicated test)

Create `src/features/public/bodyClass.ts`. This is pure `useEffect` DOM glue with no branching logic worth asserting on in a jsdom-less suite — same precedent as `src/components/PulseLine.tsx`, which also has no test file:

```ts
import { useEffect } from "react";

/**
 * Adds `className` to <body> for the lifetime of the calling component and
 * removes it on unmount. Every schedule_home page pins a page-type class to
 * <body> (e.g. body.schedule-page) that its own CSS selectors key off of —
 * see schedule_home/styles.css lines 1580, 2129, 2780, 3287.
 */
export function useBodyClass(className: string | null) {
  useEffect(() => {
    if (!className) return;
    document.body.classList.add(className);
    return () => { document.body.classList.remove(className); };
  }, [className]);
}

/**
 * Toggles body.footer-visible once the page has scrolled past 2px, matching
 * every schedule_home page's own updateFooterVisibility() — the footer
 * starts off-screen and slides in only once the visitor scrolls.
 */
export function useFooterVisibility() {
  useEffect(() => {
    function update() { document.body.classList.toggle("footer-visible", window.scrollY > 2); }
    update();
    window.addEventListener("scroll", update, { passive: true });
    return () => {
      window.removeEventListener("scroll", update);
      document.body.classList.remove("footer-visible");
    };
  }, []);
}

/** Toggles body.modal-open — every schedule_home dialog backdrop locks
 * background scroll while open (schedule_home/styles.css line 26). */
export function useModalOpenBodyClass(isAnyOpen: boolean) {
  useEffect(() => {
    document.body.classList.toggle("modal-open", isAnyOpen);
    return () => { document.body.classList.remove("modal-open"); };
  }, [isAnyOpen]);
}
```

### Step 6: Implement the four dialog components

Create `src/features/public/AuthDialog.tsx`. DOM/classes ported from `schedule_home/index.html` lines 62-80 (`#authDialog`); the submit handler is ported from `src/App.tsx`'s current `Login` component (lines 126-137: `api.login`, session storage writes, `accountFromAccessToken`) instead of the mockup's `event.preventDefault()`-only stub. The visible copy stays "Логин"/"Пароль" verbatim even though the value is submitted as an email — this is a visual, not semantic, port:

```tsx
import { useState } from "react";
import type { FormEvent } from "react";
import { accountFromAccessToken, api, type Account } from "../../lib/api";

export function AuthDialog({ open, onClose, onLoggedIn }: { open: boolean; onClose: () => void; onLoggedIn: (account: Account) => void }) {
  const [login, setLogin] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setError("");
    setLoading(true);
    try {
      const response = await api.login(login, password);
      sessionStorage.setItem("milk_schedule_access_token", response.access_token);
      sessionStorage.setItem("milk_schedule_refresh_token", response.refresh_token);
      sessionStorage.setItem("milk_schedule_email", login);
      onLoggedIn(accountFromAccessToken(response.access_token, login));
    } catch (requestError) {
      setError(requestError instanceof Error ? requestError.message : "Не удалось войти");
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className={`auth-backdrop ${open ? "is-open" : ""}`} id="authDialog" aria-hidden={!open} onClick={(event) => { if (event.target === event.currentTarget) onClose(); }}>
      <section className="auth-panel" role="dialog" aria-modal="true" aria-label="Авторизация">
        <button className="auth-close" type="button" aria-label="Закрыть форму авторизации" onClick={onClose}>×</button>
        <form className="auth-form" onSubmit={submit} autoComplete="on">
          <label className="field">
            <span className="sr-only">Логин</span>
            <input type="text" name="login" placeholder="Логин" autoComplete="username" required value={login} onChange={(event) => setLogin(event.target.value)} />
          </label>
          <label className="field">
            <span className="sr-only">Пароль</span>
            <input type="password" name="password" placeholder="Пароль" autoComplete="current-password" required value={password} onChange={(event) => setPassword(event.target.value)} />
          </label>
          {error ? <p className="auth-error" role="alert">{error}</p> : null}
          <button className="auth-submit" type="submit" disabled={loading}>{loading ? "Проверяем…" : "Войти"}</button>
        </form>
      </section>
    </div>
  );
}
```

Create `src/features/public/ContactsDialog.tsx`. DOM ported from `schedule_home/index.html` lines 99-120, with the three `<re-icon>` CDN icons replaced by local `lucide-react` icons (matching this codebase's existing icon convention — see `src/App.tsx`'s `lucide-react` imports):

```tsx
import { Mail, Phone, Send } from "lucide-react";

export function ContactsDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  return (
    <div className={`contacts-backdrop ${open ? "is-open" : ""}`} id="contactsDialog" aria-hidden={!open} onClick={(event) => { if (event.target === event.currentTarget) onClose(); }}>
      <section className="contacts-panel" role="dialog" aria-modal="true" aria-label="Контакты">
        <button className="plain-close contacts-close" type="button" aria-label="Закрыть контакты" onClick={onClose}>×</button>
        <div className="contacts-list">
          <a className="contact-item" href="https://t.me/telegram" target="_blank" rel="noreferrer" aria-label="Telegram">
            <span className="contact-item-icon" aria-hidden="true"><Send size={22} /></span>
            <span>@telegram</span>
            <span aria-hidden="true">↗</span>
          </a>
          <a className="contact-item" href="tel:+000000000000" aria-label="Телефон">
            <span className="contact-item-icon" aria-hidden="true"><Phone size={22} /></span>
            <span>+000 000 00 00</span>
            <span aria-hidden="true">↗</span>
          </a>
          <a className="contact-item" href="mailto:hello@example.com" aria-label="Email">
            <span className="contact-item-icon" aria-hidden="true"><Mail size={22} /></span>
            <span>hello@example.com</span>
            <span aria-hidden="true">↗</span>
          </a>
        </div>
      </section>
    </div>
  );
}
```

Create `src/features/public/CookieDialog.tsx`. DOM ported from `schedule_home/index.html` lines 122-163; the three action buttons call the real `persistCookiePreferences` from Step 3 instead of the mockup's DOM-only toggle:

```tsx
import { useEffect, useState } from "react";
import { persistCookiePreferences, readCookiePreferences } from "./cookieConsent";

export function CookieDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const [functional, setFunctional] = useState(false);
  const [analytics, setAnalytics] = useState(false);

  useEffect(() => {
    if (!open) return;
    const preferences = readCookiePreferences();
    setFunctional(Boolean(preferences.functional));
    setAnalytics(Boolean(preferences.analytics));
  }, [open]);

  function save(nextFunctional: boolean, nextAnalytics: boolean) {
    persistCookiePreferences(nextFunctional, nextAnalytics);
    onClose();
  }

  return (
    <div className={`cookie-backdrop ${open ? "is-open" : ""}`} id="cookieDialog" aria-hidden={!open} onClick={(event) => { if (event.target === event.currentTarget) onClose(); }}>
      <section className="cookie-panel" role="dialog" aria-modal="true" aria-label="Cookies">
        <div className="cookie-head cookie-head--plain">
          <button className="cookie-close" type="button" aria-label="Закрыть настройки cookies" onClick={onClose}>×</button>
        </div>
        <div className="cookie-options">
          <div className="cookie-row">
            <div><strong>Необходимые</strong></div>
            <span className="cookie-required" aria-label="Всегда включено">ON</span>
          </div>
          <label className="cookie-row" htmlFor="functionalCookies">
            <div><strong>Функциональные</strong></div>
            <span className="switch"><input id="functionalCookies" type="checkbox" checked={functional} onChange={(event) => setFunctional(event.target.checked)} /><span className="switch-track" aria-hidden="true"><span></span></span></span>
          </label>
          <label className="cookie-row" htmlFor="analyticsCookies">
            <div><strong>Аналитика</strong></div>
            <span className="switch"><input id="analyticsCookies" type="checkbox" checked={analytics} onChange={(event) => setAnalytics(event.target.checked)} /><span className="switch-track" aria-hidden="true"><span></span></span></span>
          </label>
        </div>
        <div className="cookie-actions">
          <button type="button" className="cookie-action cookie-action--ghost" onClick={() => save(false, false)}>Только необходимые</button>
          <button type="button" className="cookie-action cookie-action--ghost" onClick={() => save(true, true)}>Принять все</button>
          <button type="button" className="cookie-action cookie-action--primary" onClick={() => save(functional, analytics)}>Сохранить</button>
        </div>
      </section>
    </div>
  );
}
```

Create `src/features/public/GuideDialog.tsx`. DOM ported from `schedule_home/index.html` lines 82-97; each step both closes the dialog and navigates (ported from `script.js`'s `guideSteps.forEach` handler):

```tsx
export function GuideDialog({ open, onClose, onPick }: { open: boolean; onClose: () => void; onPick: (target: "group" | "teacher" | "room") => void }) {
  const steps: Array<{ target: "group" | "teacher" | "room"; floor: string; label: string }> = [
    { target: "group", floor: "3F", label: "Группа" },
    { target: "teacher", floor: "2F", label: "Преподаватель" },
    { target: "room", floor: "1F", label: "Аудитория" },
  ];

  return (
    <div className={`guide-backdrop ${open ? "is-open" : ""}`} id="guideDialog" aria-hidden={!open} onClick={(event) => { if (event.target === event.currentTarget) onClose(); }}>
      <section className="guide-panel" role="dialog" aria-modal="true" aria-label="Подсказка">
        <button className="plain-close guide-close" type="button" aria-label="Закрыть подсказку" onClick={onClose}>×</button>
        <div className="guide-steps">
          {steps.map((step) => (
            <button className="guide-step" type="button" key={step.target} aria-label={step.label} onClick={() => onPick(step.target)}>
              <span>{step.floor}</span><span>{step.label}</span><span>↗</span>
            </button>
          ))}
        </div>
      </section>
    </div>
  );
}
```

### Step 7: Write the failing test for `PublicLayout`

Create `src/features/public/PublicLayout.test.tsx`:

```tsx
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { PublicLayout } from "./PublicLayout";

function renderAt(path: string) {
  return renderToStaticMarkup(
    <MemoryRouter initialEntries={[path]}>
      <Routes>
        <Route element={<PublicLayout onLoggedIn={() => undefined} />}>
          <Route path="/" element={<div>home content</div>} />
        </Route>
      </Routes>
    </MemoryRouter>,
  );
}

describe("PublicLayout", () => {
  it("renders the header, footer, and every dialog backdrop closed by default", () => {
    const markup = renderAt("/");
    expect(markup).toContain("home content");
    expect(markup).toContain('id="authDialog"');
    expect(markup).toContain('id="contactsDialog"');
    expect(markup).toContain('id="cookieDialog"');
    expect(markup).toContain('id="guideDialog"');
    expect(markup).not.toContain("is-open");
  });
});
```

### Step 8: Run it to see it fail

Run: `npm test -- src/features/public/PublicLayout.test.tsx`
Expected: FAIL — cannot find module `./PublicLayout`.

### Step 9: Implement `PublicLayout.tsx`

Create `src/features/public/PublicLayout.tsx`. Header/footer DOM ported from `schedule_home/index.html` lines 16-23 and 56-60 (the profile-button SVG is copied verbatim — it is identical across every mockup page):

```tsx
import { useState } from "react";
import { Outlet, useNavigate } from "react-router-dom";
import type { Account } from "../../lib/api";
import { useModalOpenBodyClass } from "./bodyClass";
import { AuthDialog } from "./AuthDialog";
import { ContactsDialog } from "./ContactsDialog";
import { CookieDialog } from "./CookieDialog";
import { GuideDialog } from "./GuideDialog";
import "./styles.css";

type DialogName = "auth" | "contacts" | "cookies" | "guide" | null;

export type PublicOutletContext = { openGuide: () => void };

export function PublicLayout({ onLoggedIn }: { onLoggedIn: (account: Account) => void }) {
  const [openDialog, setOpenDialog] = useState<DialogName>(null);
  const navigate = useNavigate();
  useModalOpenBodyClass(openDialog !== null);

  function toggle(name: Exclude<DialogName, null>) {
    setOpenDialog((current) => (current === name ? null : name));
  }
  function close() { setOpenDialog(null); }

  return (
    <>
      <header className="site-header">
        <button className="profile-button" type="button" aria-label="Открыть форму авторизации" aria-controls="authDialog" aria-expanded={openDialog === "auth"} onClick={() => toggle("auth")}>
          <svg className="profile-icon" aria-hidden="true" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
            <path fillRule="evenodd" clipRule="evenodd" d="M8.25 9C8.25 6.92893 9.92893 5.25 12 5.25C14.0711 5.25 15.75 6.92893 15.75 9C15.75 11.0711 14.0711 12.75 12 12.75C9.92893 12.75 8.25 11.0711 8.25 9ZM12 6.75C10.7574 6.75 9.75 7.75736 9.75 9C9.75 10.2426 10.7574 11.25 12 11.25C13.2426 11.25 14.25 10.2426 14.25 9C14.25 7.75736 13.2426 6.75 12 6.75Z" />
            <path fillRule="evenodd" clipRule="evenodd" d="M1.25 12C1.25 6.06294 6.06294 1.25 12 1.25C17.9371 1.25 22.75 6.06294 22.75 12C22.75 17.9371 17.9371 22.75 12 22.75C6.06294 22.75 1.25 17.9371 1.25 12ZM12 2.75C6.89137 2.75 2.75 6.89137 2.75 12C2.75 14.5456 3.77827 16.851 5.4421 18.5235C5.6225 17.5504 5.97694 16.6329 6.68837 15.8951C7.75252 14.7915 9.45416 14.25 12 14.25C14.5457 14.25 16.2474 14.7915 17.3115 15.8951C18.023 16.6329 18.3774 17.5505 18.5578 18.5236C20.2217 16.8511 21.25 14.5456 21.25 12C21.25 6.89137 17.1086 2.75 12 2.75ZM17.1937 19.6554C17.0918 18.4435 16.8286 17.5553 16.2318 16.9363C15.5823 16.2628 14.3789 15.75 12 15.75C9.62099 15.75 8.41761 16.2628 7.76815 16.9363C7.17127 17.5553 6.90811 18.4434 6.80622 19.6553C8.28684 20.6618 10.0747 21.25 12 21.25C13.9252 21.25 15.7131 20.6618 17.1937 19.6554Z" />
          </svg>
        </button>
      </header>

      <Outlet context={{ openGuide: () => setOpenDialog("guide") } satisfies PublicOutletContext} />

      <footer className="site-footer" aria-label="Служебные ссылки">
        <button type="button" aria-controls="contactsDialog" aria-expanded={openDialog === "contacts"} onClick={() => toggle("contacts")}>Контакты</button>
        <button type="button" aria-controls="cookieDialog" aria-expanded={openDialog === "cookies"} onClick={() => toggle("cookies")}>Cookies</button>
        <a href="https://example.com" target="_blank" rel="noreferrer">Ссылка</a>
      </footer>

      <AuthDialog open={openDialog === "auth"} onClose={close} onLoggedIn={onLoggedIn} />
      <ContactsDialog open={openDialog === "contacts"} onClose={close} />
      <CookieDialog open={openDialog === "cookies"} onClose={close} />
      <GuideDialog open={openDialog === "guide"} onClose={close} onPick={(target) => { close(); navigate(`/${target}`); }} />
    </>
  );
}
```

### Step 10: Run it to see it pass

Run: `npm test -- src/features/public/PublicLayout.test.tsx`
Expected: PASS.

### Step 11: Create `PublicApp.tsx`

Create `src/features/public/PublicApp.tsx`. Only the `/` route exists so far — Task 3 replaces the placeholder, and Tasks 4-9 each add exactly one `<Route>` line as their own page component becomes available. No dedicated test file (pure router wiring, matching the precedent that `src/App.tsx` itself has none):

```tsx
import { BrowserRouter, Route, Routes } from "react-router-dom";
import type { Account } from "../../lib/api";
import { PublicLayout } from "./PublicLayout";

export function PublicApp({ onLoggedIn }: { onLoggedIn: (account: Account) => void }) {
  return (
    <BrowserRouter>
      <Routes>
        <Route element={<PublicLayout onLoggedIn={onLoggedIn} />}>
          <Route path="/" element={<div>Home placeholder — replaced in Task 3</div>} />
        </Route>
      </Routes>
    </BrowserRouter>
  );
}
```

### Step 12: Wire `PublicApp` into `App.tsx`

Modify `src/App.tsx`:

1. Remove the imports `PublicSchedulePage as ApiPublicSchedulePage` (from `./features/schedule/PublicSchedulePage`), `HomePage` (from `./features/home/HomePage`), and `PublicHeader, PublicFooter, PublicMain` (from `./features/home/PublicShell`) — these three files are deleted wholesale in Task 10; `App.tsx` simply stops referencing them now. Add `import { PublicApp } from "./features/public/PublicApp";`.
2. Delete the `PUBLIC_VIEWS` constant and the `isPublicView` local — both become dead once the ternary that used them is replaced below.
3. Replace the return statement's public/non-public split. The old code (lines 80-105) is:

```tsx
  return (
    <div className={`app-shell ${view === "admin" ? "admin-app-shell" : ""} ${isPublicView ? "public-app-shell" : ""}`} ref={stageRef}>
      <div className="ambient ambient-one" />
      <div className="ambient ambient-two" />
      {isPublicView ? (
        <PublicHeader onHome={() => setView("home")} onLogin={() => setView(staffView(account))} />
      ) : view !== "admin" ? (
        <Header account={account} currentLabel={currentLabel} onLogin={() => setView(staffView(account))} onHome={() => setView("home")} onMenu={() => setMobileMenuOpen((value) => !value)} />
      ) : null}
      {mobileMenuOpen && <MobileMenu onClose={() => setMobileMenuOpen(false)} onLogin={() => { setMobileMenuOpen(false); setView("login"); }} />}
      {isPublicView ? (
        <PublicMain>
          {view === "home" && <HomePage onOpenSchedule={(mode) => setView(mode)} isAuthenticated={Boolean(account)} onRequireLogin={() => setView("login")} />}
          {(view === "group" || view === "teacher" || view === "room") && <ApiPublicSchedulePage mode={view} onBack={() => setView("home")} onRequireLogin={() => setView("login")} />}
        </PublicMain>
      ) : (
        <main>
          {view === "login" && <Login onBack={() => setView("home")} onLoggedIn={handleLogin} />}
          {view === "password" && account ? <PasswordChange email={account.email} onChanged={handleLogin} onLogout={handleLogout} /> : null}
          {view === "admin" && <ApiAdminPanel account={account} onLogout={handleLogout} onBack={() => setView("home")} />}
          {view === "self" && <ApiTeacherSelfService onLogout={handleLogout} />}
        </main>
      )}
      {isPublicView ? <PublicFooter /> : <footer className="site-footer"><span>Milk Schedule · 2026</span><span>Расписание, которое держит ритм университета</span></footer>}
    </div>
  );
```

Replace it with:

```tsx
  const showPublicRouter = !account;

  return (
    <div className={`app-shell ${view === "admin" ? "admin-app-shell" : ""} ${showPublicRouter ? "public-app-shell" : ""}`} ref={stageRef}>
      <div className="ambient ambient-one" />
      <div className="ambient ambient-two" />
      {showPublicRouter ? (
        <PublicApp onLoggedIn={handleLogin} />
      ) : (
        <>
          {view !== "admin" && <Header account={account} currentLabel={currentLabel} onLogin={() => setView(staffView(account))} onHome={() => setView("home")} onMenu={() => setMobileMenuOpen((value) => !value)} />}
          {mobileMenuOpen && <MobileMenu onClose={() => setMobileMenuOpen(false)} onLogin={() => { setMobileMenuOpen(false); setView("login"); }} />}
          <main>
            {view === "login" && <Login onBack={() => setView("home")} onLoggedIn={handleLogin} />}
            {view === "password" && account ? <PasswordChange email={account.email} onChanged={handleLogin} onLogout={handleLogout} /> : null}
            {view === "admin" && <ApiAdminPanel account={account} onLogout={handleLogout} onBack={() => setView("home")} />}
            {view === "self" && <ApiTeacherSelfService onLogout={handleLogout} />}
          </main>
          {view !== "admin" && <footer className="site-footer"><span>Milk Schedule · 2026</span><span>Расписание, которое держит ритм университета</span></footer>}
        </>
      )}
    </div>
  );
```

`.app-shell.public-app-shell` (existing, untouched CSS at `src/styles.css` lines 714-721) already paints a light background and hides the `.ambient` blur circles — reused as-is so the mockup's light-themed pages don't sit on the dark admin gradient. `view === "login"` inside the non-public branch is now unreachable in practice: that branch only renders once `account` is truthy, and no code path sets `view` to `"login"` while `account` is truthy (`staffView` never returns `"login"` for a non-null account). This matches the design spec's "Login as a page is removed for the public flow" — the branch is left in place, not deleted, per "otherwise untouched" in the spec's "Out of scope" section.

4. In `handleLogout`, reset the URL to `/` before clearing `account`, so the freshly-mounted `PublicApp`'s `BrowserRouter` (which reads `window.location` once, at mount) starts at the home route instead of wherever the router happened to be:

```tsx
  async function handleLogout() {
    const refreshToken = sessionStorage.getItem("milk_schedule_refresh_token");
    if (refreshToken) { try { await api.logout(refreshToken); } catch { /* Clear local session even when the API is unavailable. */ } }
    sessionStorage.removeItem("milk_schedule_access_token");
    sessionStorage.removeItem("milk_schedule_refresh_token");
    sessionStorage.removeItem("milk_schedule_email");
    sessionStorage.removeItem(VIEW_STORAGE_KEY);
    if (typeof window !== "undefined") window.history.replaceState(null, "", "/");
    setAccount(null);
    setView("home");
  }
```

### Step 13: Run the full suite and lint

Run: `npm test` and `npm run lint`
Expected: both PASS. `HomePage.test.tsx`/`PublicSchedulePage.test.tsx` (the old ones) still exist and still pass at this point — they're only deleted in Task 10, once nothing in the new tree needs the old components for reference.

### Step 14: Stop here, changes are ready for review.

---

## Task 3: `PublicHome` (`index.html` port)

**Files:**
- Create: `src/features/public/PublicHome.tsx` + `src/features/public/PublicHome.test.tsx`
- Modify: `src/features/public/PublicApp.tsx` (replace the `/` placeholder route)

**Interfaces:**
- Consumes: `PublicOutletContext` (Task 2) via `useOutletContext()`.
- Produces: `<PublicHome />`, mounted at `/`.

`index.html` has no `updateFooterVisibility()` in its own `script.js` (unlike every other mockup page) and no extra `<body>` class — `PublicHome` does **not** call `useBodyClass`/`useFooterVisibility` from Task 2.

### Step 1: Write the failing test

Create `src/features/public/PublicHome.test.tsx`:

```tsx
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { MemoryRouter, Outlet, Route, Routes } from "react-router-dom";
import { PublicHome } from "./PublicHome";

function renderHome() {
  return renderToStaticMarkup(
    <MemoryRouter initialEntries={["/"]}>
      <Routes>
        <Route element={<Outlet context={{ openGuide: () => undefined }} />}>
          <Route path="/" element={<PublicHome />} />
        </Route>
      </Routes>
    </MemoryRouter>,
  );
}

describe("PublicHome", () => {
  it("renders the three floor buttons and the guide trigger, none active yet", () => {
    const markup = renderHome();
    expect(markup).toContain('aria-label="Группа"');
    expect(markup).toContain('aria-label="Преподаватель"');
    expect(markup).toContain('aria-label="Аудитория"');
    expect(markup).toContain('aria-label="Открыть гайд"');
    expect(markup).not.toContain("is-open");
    expect(markup).not.toContain("is-active");
  });
});
```

### Step 2: Run it to see it fail

Run: `npm test -- src/features/public/PublicHome.test.tsx`
Expected: FAIL — cannot find module `./PublicHome`.

### Step 3: Implement `PublicHome.tsx`

DOM ported from `schedule_home/index.html` lines 25-54 (`.scene`/`.building`/`.shaft`/3 floor buttons/guide trigger) and line 165 (`.transition-layer`). Behavior ported from `schedule_home/script.js`'s `chooseFloor()` (lines 42-58) — the 540ms delay before navigating is the mockup's own transition timing, kept as-is:

```tsx
import { useState } from "react";
import { useNavigate, useOutletContext } from "react-router-dom";
import type { PublicOutletContext } from "./PublicLayout";

type FloorTarget = "group" | "teacher" | "room";

const ROUTE_LABELS: Record<FloorTarget, string> = { group: "Группа", teacher: "Преподаватель", room: "Аудитория" };

const FLOORS: Array<{ target: FloorTarget; index: number; label: string }> = [
  { target: "group", index: 3, label: "Группа" },
  { target: "teacher", index: 2, label: "Преподаватель" },
  { target: "room", index: 1, label: "Аудитория" },
];

export function PublicHome() {
  const navigate = useNavigate();
  const { openGuide } = useOutletContext<PublicOutletContext>();
  const [activeFloor, setActiveFloor] = useState<FloorTarget | null>(null);
  const [transitioning, setTransitioning] = useState(false);

  function chooseFloor(target: FloorTarget) {
    setActiveFloor(target);
    setTransitioning(true);
    window.setTimeout(() => navigate(`/${target}`), 540);
  }

  return (
    <>
      <main className="scene" aria-label="Выбор типа расписания">
        <section className="building" aria-label="Три варианта выбора">
          <div className="shaft" aria-hidden="true">
            <div className="shaft-line shaft-line--left"></div>
            <div className="shaft-line shaft-line--right"></div>
          </div>

          <button className="guide-trigger" type="button" aria-label="Открыть гайд" onClick={openGuide}>
            <span className="guide-mark" aria-hidden="true">?</span>
          </button>

          {FLOORS.map((floor) => (
            <button key={floor.target} className={`floor floor--${floor.index} ${activeFloor === floor.target ? "is-active" : ""}`} type="button" aria-label={floor.label} onClick={() => chooseFloor(floor.target)}>
              <span className="floor-index"><span className="floor-number">{floor.index}</span><span className="floor-f">F</span></span>
              <span className="floor-label">{floor.label}</span>
              <span className="floor-arrow" aria-hidden="true">↗</span>
            </button>
          ))}
        </section>
      </main>

      <div className={`transition-layer ${transitioning ? "is-open" : ""}`} aria-hidden={!transitioning}>
        <span>{activeFloor ? ROUTE_LABELS[activeFloor] : ""}</span>
      </div>
    </>
  );
}
```

### Step 4: Run it to see it pass

Run: `npm test -- src/features/public/PublicHome.test.tsx`
Expected: PASS.

### Step 5: Wire the route

In `src/features/public/PublicApp.tsx`, replace the placeholder route with the real one:

```tsx
import { PublicHome } from "./PublicHome";
// ...
          <Route path="/" element={<PublicHome />} />
```

### Step 6: Run the full suite and lint

Run: `npm test` and `npm run lint`
Expected: both PASS.

### Step 7: Stop here, changes are ready for review.

---

## Task 4: `GroupPicker` (`group.html`/`group.js` port)

Keeps the mockup's faculty-card grid, carousel detail overlay, and
flyer-clone transition; wires faculty cards to `api.listFaculties()` and the
course/group step to `api.listPublicGroups(facultyId)`; drops the mockup's
4th step (`facultySchedules`, procedurally-generated fake group codes)
entirely — the period-jump control that step's spirit maps to lives on
`SchedulePage` instead (Task 6), since periods are per-group-semester and
the group isn't known until this step completes.

**Files:**
- Create: `src/features/public/GroupPicker.tsx` + `src/features/public/GroupPicker.test.tsx`
- Modify: `src/features/public/PublicApp.tsx` (add the `/group` route)

**Interfaces:**
- Consumes: `api.listFaculties()`, `api.listPublicGroups(facultyId, date?)`, `DateResolvedGroup` (from `src/lib/api.ts`); `useBodyClass`, `useFooterVisibility` (Task 2).
- Produces: `groupByCourse(groups: DateResolvedGroup[]): Array<{ course: number; groups: DateResolvedGroup[] }>`, `mapFacultyOptions(response): FacultyOption[]` — both exported for the tests below and reusable if a later task needs the same bucketing (none currently does).

### Step 1: Write the failing tests

Create `src/features/public/GroupPicker.test.tsx`:

```tsx
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { MemoryRouter } from "react-router-dom";
import { GroupPicker, groupByCourse, mapFacultyOptions } from "./GroupPicker";
import type { DateResolvedGroup } from "../../lib/api";

describe("GroupPicker", () => {
  it("shows the faculty loading state and a closed detail overlay on first render", () => {
    const markup = renderToStaticMarkup(<MemoryRouter><GroupPicker /></MemoryRouter>);
    expect(markup).toContain("Загружаем факультеты");
    expect(markup).toContain('aria-label="Выбор курса и группы"');
  });
});

describe("mapFacultyOptions", () => {
  it("uppercases the short name into a monogram and the name into a label", () => {
    const options = mapFacultyOptions({ data: [{ id: "1", attributes: { name: "Строительный", short_name: "стф" } }] });
    expect(options).toEqual([{ id: "1", monogram: "СТФ", label: "СТРОИТЕЛЬНЫЙ" }]);
  });
});

describe("groupByCourse", () => {
  const group = (overrides: Partial<DateResolvedGroup>): DateResolvedGroup => ({
    group_semester_id: 1, group_id: 1, course: 1, speciality_name: "МК", students_now: 20,
    year_of_admission: "2024-09-01", duration_of_study: 4,
    semester: { group_semester_id: 1, semester_id: 1, number: 1, label: "1 семестр", start_date: "2026-09-01", end_date: "2027-01-01", period_kind: "semester" },
    ...overrides,
  });

  it("buckets groups by course and sorts courses ascending", () => {
    const groups = [group({ group_semester_id: 3, course: 3, speciality_name: "СИ" }), group({ group_semester_id: 1, course: 1 })];
    const result = groupByCourse(groups);
    expect(result.map((entry) => entry.course)).toEqual([1, 3]);
    expect(result[1].groups[0].speciality_name).toBe("СИ");
  });

  it("returns an empty list for an empty groups array", () => {
    expect(groupByCourse([])).toEqual([]);
  });
});
```

### Step 2: Run it to see it fail

Run: `npm test -- src/features/public/GroupPicker.test.tsx`
Expected: FAIL — cannot find module `./GroupPicker`.

### Step 3: Implement `GroupPicker.tsx`

DOM ported from `schedule_home/group.html` lines 26-76 (faculty grid + detail
overlay) — **omit** the `.faculty-detail-schedules` block entirely (dropped
per the locked delta above). The flyer-clone transition is ported from
`schedule_home/group.js`'s `animateProductToDetail` (lines 94-124):

```tsx
import { useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { api, type DateResolvedGroup } from "../../lib/api";
import { useBodyClass, useFooterVisibility } from "./bodyClass";

type FacultyOption = { id: string; monogram: string; label: string };

export function mapFacultyOptions(response: { data: Array<{ id: string; attributes?: Record<string, unknown> }> }): FacultyOption[] {
  return response.data.map((item) => ({
    id: item.id,
    monogram: String(item.attributes?.short_name ?? "").toUpperCase(),
    label: String(item.attributes?.name ?? "").toUpperCase(),
  }));
}

export function groupByCourse(groups: DateResolvedGroup[]): Array<{ course: number; groups: DateResolvedGroup[] }> {
  const byCourse = new Map<number, DateResolvedGroup[]>();
  for (const group of groups) {
    const bucket = byCourse.get(group.course) ?? [];
    bucket.push(group);
    byCourse.set(group.course, bucket);
  }
  return [...byCourse.entries()].sort(([left], [right]) => left - right).map(([course, courseGroups]) => ({ course, groups: courseGroups }));
}

function animateProductToDetail(cardEl: HTMLElement, detailEl: HTMLElement) {
  const source = cardEl.querySelector<HTMLElement>(".faculty-product");
  if (!source) return;
  const from = source.getBoundingClientRect();
  const to = detailEl.getBoundingClientRect();
  const clone = source.cloneNode(true) as HTMLElement;
  clone.classList.add("faculty-product-flyer");
  Object.assign(clone.style, { left: `${from.left}px`, top: `${from.top}px`, width: `${from.width}px`, height: `${from.height}px` });
  document.body.appendChild(clone);
  source.style.opacity = "0";
  detailEl.style.opacity = "0";
  const animation = clone.animate([
    { left: `${from.left}px`, top: `${from.top}px`, width: `${from.width}px`, height: `${from.height}px` },
    { left: `${to.left}px`, top: `${to.top}px`, width: `${to.width}px`, height: `${to.height}px` },
  ], { duration: 560, easing: "cubic-bezier(.76,0,.24,1)", fill: "forwards" });
  animation.finished.finally(() => {
    clone.remove();
    detailEl.style.opacity = "";
    source.style.opacity = "";
  });
}

export function GroupPicker() {
  useBodyClass("faculty-page");
  useFooterVisibility();
  const navigate = useNavigate();

  const [faculties, setFaculties] = useState<FacultyOption[]>([]);
  const [loadingFaculties, setLoadingFaculties] = useState(true);
  const [error, setError] = useState("");

  const [detailOpen, setDetailOpen] = useState(false);
  const [activeIndex, setActiveIndex] = useState(0);
  const [course, setCourse] = useState<number | null>(null);
  const [groupId, setGroupId] = useState<string | null>(null);
  const [groups, setGroups] = useState<DateResolvedGroup[]>([]);
  const [groupsLoading, setGroupsLoading] = useState(false);

  const detailProductRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let active = true;
    api.listFaculties()
      .then((response) => { if (active) setFaculties(mapFacultyOptions(response)); })
      .catch(() => active && setError("Не удалось загрузить список факультетов."))
      .finally(() => active && setLoadingFaculties(false));
    return () => { active = false; };
  }, []);

  useEffect(() => {
    document.body.classList.toggle("faculty-detail-open", detailOpen);
    return () => { document.body.classList.remove("faculty-detail-open"); };
  }, [detailOpen]);

  const activeFaculty = faculties[activeIndex];

  useEffect(() => {
    if (!detailOpen || !activeFaculty) return;
    let active = true;
    setGroupsLoading(true);
    setCourse(null);
    setGroupId(null);
    api.listPublicGroups(activeFaculty.id)
      .then((response) => { if (active) setGroups(response.groups); })
      .catch(() => active && setError("Не удалось загрузить группы."))
      .finally(() => active && setGroupsLoading(false));
    return () => { active = false; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [detailOpen, activeFaculty?.id]);

  const byCourse = groupByCourse(groups);
  const courses = byCourse.map((entry) => entry.course);
  const activeCourseGroups = byCourse.find((entry) => entry.course === course)?.groups ?? [];

  function openFaculty(index: number, cardEl: HTMLButtonElement) {
    setActiveIndex(index);
    setDetailOpen(true);
    if (detailProductRef.current) animateProductToDetail(cardEl, detailProductRef.current);
  }

  function shiftFaculty(direction: 1 | -1) {
    setActiveIndex((current) => (current + direction + faculties.length) % faculties.length);
  }

  function confirm() {
    if (!groupId) return;
    navigate(`/schedule?group=${encodeURIComponent(groupId)}`);
  }

  return (
    <>
      <main className="faculty-scene" aria-label="Выбор факультета">
        {loadingFaculties ? <p className="lookup-status" role="status">Загружаем факультеты…</p> : null}
        {error ? <p className="lookup-status">{error}</p> : null}
        <section className="faculty-grid" aria-hidden={detailOpen}>
          {faculties.map((faculty, index) => (
            <button key={faculty.id} className="faculty-card" type="button" aria-label={faculty.label} onClick={(event) => openFaculty(index, event.currentTarget)}>
              <span className="faculty-product"><span className="faculty-monogram">{faculty.monogram}</span></span>
              <span className="faculty-card-label">{faculty.label}</span>
            </button>
          ))}
        </section>
      </main>

      <section className="faculty-detail" aria-hidden={!detailOpen} aria-label="Выбор курса и группы">
        <button className="faculty-detail-close" type="button" aria-label="Вернуться к факультетам" onClick={() => setDetailOpen(false)}>‹</button>
        <button className="faculty-detail-nav faculty-detail-prev" type="button" aria-label="Предыдущий факультет" onClick={() => shiftFaculty(-1)}>‹</button>
        <button className="faculty-detail-nav faculty-detail-next" type="button" aria-label="Следующий факультет" onClick={() => shiftFaculty(1)}>›</button>

        <div className="faculty-detail-product" ref={detailProductRef} aria-hidden="true">
          <span className="faculty-monogram">{activeFaculty?.monogram}</span>
        </div>

        <div className="faculty-detail-options">
          <div className="faculty-detail-dots" aria-hidden="true">
            {faculties.map((faculty, index) => <span key={faculty.id} className={index === activeIndex ? "is-active" : ""}></span>)}
          </div>

          <div className="faculty-detail-courses" role="group" aria-label="Курс">
            {groupsLoading ? <span className="lookup-status">Загружаем курсы…</span> : courses.map((courseNumber) => (
              <button key={courseNumber} type="button" className={course === courseNumber ? "is-active" : ""} aria-label={`${courseNumber} курс`} onClick={() => { setCourse(courseNumber); setGroupId(null); }}>{courseNumber}</button>
            ))}
            {!groupsLoading && courses.length === 0 ? <span className="lookup-status">Нет групп с расписанием на эту дату.</span> : null}
          </div>

          <div className="faculty-detail-groups" role="group" aria-label="Группа">
            {activeCourseGroups.map((group) => (
              <button key={group.group_semester_id} type="button" className={groupId === String(group.group_semester_id) ? "is-active" : ""} onClick={() => setGroupId(String(group.group_semester_id))}>
                {group.speciality_name}-{group.course}
              </button>
            ))}
          </div>

          <button className="faculty-detail-confirm" type="button" aria-label="Выбрать расписание" disabled={!groupId} onClick={confirm}>+</button>
        </div>
      </section>
    </>
  );
}
```

### Step 4: Run it to see it pass

Run: `npm test -- src/features/public/GroupPicker.test.tsx`
Expected: PASS.

### Step 5: Wire the route

In `src/features/public/PublicApp.tsx`, add:

```tsx
import { GroupPicker } from "./GroupPicker";
// ...
          <Route path="/group" element={<GroupPicker />} />
```

### Step 6: Run the full suite and lint

Run: `npm test` and `npm run lint`
Expected: both PASS.

### Step 7: Stop here, changes are ready for review.

---

## Task 5: `EntitySearch` (`teacher.html`/`room.html`/`search.js` port)

One component, `mode="teacher" | "room"`, covers both routes (mirroring how
`search.js` already branches on `document.body.dataset.searchMode`). Teacher
search filters server-side via `api.listPublicTeachers(query)`; room search
has **no server-side filter** — confirmed against
`apps/api_geteway/slices/portal/contracts/rooms/index_contract.rb`, which
only accepts `page`/`per_page` — so `EntitySearch` fetches up to 100 rooms
once and filters client-side, porting `search.js`'s own `normalize()`/
`searchableText()` logic verbatim for that purpose. Room search+detail stay
gated behind login.

**Files:**
- Modify: `src/features/public/PublicLayout.tsx` (extend `PublicOutletContext` with `openAuth`, so a page can trigger the login dialog directly — e.g. this task's room gate, and Task 8's `RoomDetail` gate)
- Create: `src/features/public/EntitySearch.tsx` + `src/features/public/EntitySearch.test.tsx`
- Modify: `src/features/public/PublicApp.tsx` (add `/teacher` and `/room` routes)

**Interfaces:**
- Consumes: `hasAccessToken` (Task 1), `resolveRelationshipAttributes`/`stringAttribute`/`teacherFullName` (Task 1), `useBodyClass`/`useFooterVisibility` (Task 2), `api.listPublicTeachers`/`api.listPublicRooms` (Task 1, already returning `JsonApiCollection`).
- Produces: `paginate<T>(items: T[], page: number, pageSize: number): { pageItems: T[]; totalPages: number; page: number }`, `roomSearchableText(resource, included): string` — both exported and reusable (no other task currently needs them).
- Produces: navigation state contract consumed by Task 8 — `navigate(\`/${mode}/${id}\`, { state: { resource: JsonApiResource, included: JsonApiResource[] | undefined } })`. `TeacherDetail`/`RoomDetail` read `location.state` first and fall back to a fresh fetch when it's absent (direct link / refresh) — see Task 8 for the full rationale.

### Step 1: Extend `PublicOutletContext` with `openAuth`

In `src/features/public/PublicLayout.tsx`, change:

```tsx
export type PublicOutletContext = { openGuide: () => void };
```

to:

```tsx
export type PublicOutletContext = { openGuide: () => void; openAuth: () => void };
```

and change the `<Outlet context={...} />` value from:

```tsx
      <Outlet context={{ openGuide: () => setOpenDialog("guide") } satisfies PublicOutletContext} />
```

to:

```tsx
      <Outlet context={{ openGuide: () => setOpenDialog("guide"), openAuth: () => setOpenDialog("auth") } satisfies PublicOutletContext} />
```

Run `npm test -- src/features/public/PublicLayout.test.tsx` — still PASS (the existing assertions don't check context shape).

### Step 2: Write the failing tests

Create `src/features/public/EntitySearch.test.tsx`:

```tsx
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter, Outlet, Route, Routes } from "react-router-dom";
import { EntitySearch, paginate, roomSearchableText } from "./EntitySearch";
import type { JsonApiResource } from "../../lib/api";

class MemoryStorage implements Storage {
  private values = new Map<string, string>();
  get length() { return this.values.size; }
  clear() { this.values.clear(); }
  getItem(key: string) { return this.values.get(key) ?? null; }
  key(index: number) { return [...this.values.keys()][index] ?? null; }
  removeItem(key: string) { this.values.delete(key); }
  setItem(key: string, value: string) { this.values.set(key, value); }
}

afterEach(() => { vi.unstubAllGlobals(); });

function renderAt(mode: "teacher" | "room", path: string) {
  return renderToStaticMarkup(
    <MemoryRouter initialEntries={[path]}>
      <Routes>
        <Route element={<Outlet context={{ openGuide: () => undefined, openAuth: () => undefined }} />}>
          <Route path={path} element={<EntitySearch mode={mode} />} />
        </Route>
      </Routes>
    </MemoryRouter>,
  );
}

describe("EntitySearch", () => {
  it("shows the teacher search field and starts loading", () => {
    const markup = renderAt("teacher", "/teacher");
    expect(markup).toContain("Фамилия, имя или кафедра");
    expect(markup).toContain("Ищем варианты");
  });

  it("gates room search behind login when no token is present", () => {
    const markup = renderAt("room", "/room");
    expect(markup).toContain("Войдите, чтобы искать аудитории");
    expect(markup).not.toContain("Номер аудитории или корпус");
  });

  it("shows the room search once a token is present", () => {
    const storage = new MemoryStorage();
    storage.setItem("milk_schedule_access_token", "token-value");
    vi.stubGlobal("sessionStorage", storage);
    const markup = renderAt("room", "/room");
    expect(markup).not.toContain("Войдите, чтобы искать аудитории");
    expect(markup).toContain("Номер аудитории или корпус");
  });
});

describe("paginate", () => {
  it("slices items into pages and clamps an out-of-range page", () => {
    const items = Array.from({ length: 10 }, (_, index) => index);
    expect(paginate(items, 1, 4)).toEqual({ pageItems: [0, 1, 2, 3], totalPages: 3, page: 1 });
    expect(paginate(items, 5, 4)).toEqual({ pageItems: [8, 9], totalPages: 3, page: 3 });
  });
});

describe("roomSearchableText", () => {
  it("joins number, name, building number, and room type into one searchable string", () => {
    const resource: JsonApiResource = { id: "1", type: "room", attributes: { number: "2-312" }, relationships: { building: { data: { id: "2", type: "building" } }, room_type: { data: { id: "3", type: "room_type" } } } };
    const included: JsonApiResource[] = [{ id: "2", type: "building", attributes: { number: "2" } }, { id: "3", type: "room_type", attributes: { name: "Лекционная аудитория" } }];
    expect(roomSearchableText(resource, included)).toBe("2-312 2 Лекционная аудитория");
  });
});
```

### Step 3: Run it to see it fail

Run: `npm test -- src/features/public/EntitySearch.test.tsx`
Expected: FAIL — cannot find module `./EntitySearch`.

### Step 4: Implement `EntitySearch.tsx`

DOM ported from `schedule_home/teacher.html`/`room.html` lines 26-37 (the two files are identical past the `<body data-search-mode>` attribute and the input placeholder) and card/pagination markup from `search.js`'s `cardMarkup`/`renderPagination` (lines 49-64, 95-116). The card→detail flyer transition is ported from `search.js`'s `openSchedule` (lines 127-150):

```tsx
import { useEffect, useState } from "react";
import { useNavigate, useOutletContext } from "react-router-dom";
import { Lock } from "lucide-react";
import { api, type JsonApiCollection, type JsonApiResource } from "../../lib/api";
import { hasAccessToken } from "../../lib/session";
import { resolveRelationshipAttributes, stringAttribute, teacherFullName } from "./entityFields";
import { useBodyClass, useFooterVisibility } from "./bodyClass";
import type { PublicOutletContext } from "./PublicLayout";

type Mode = "teacher" | "room";

const PAGE_SIZE: Record<Mode, number> = { teacher: 8, room: 9 };
const PLACEHOLDER: Record<Mode, string> = { teacher: "Фамилия, имя или кафедра", room: "Номер аудитории или корпус" };

function normalizeSearchText(value: string): string {
  return value.toLocaleLowerCase("ru-RU").replace(/ё/g, "е").trim();
}

export function roomSearchableText(resource: JsonApiResource, included: JsonApiResource[] | undefined): string {
  const attrs = resource.attributes ?? {};
  const building = resolveRelationshipAttributes(resource, included, "building");
  const roomType = resolveRelationshipAttributes(resource, included, "room_type");
  return [attrs.number, attrs.name, building?.number, roomType?.name].filter(Boolean).join(" ");
}

export function paginate<T>(items: T[], page: number, pageSize: number): { pageItems: T[]; totalPages: number; page: number } {
  const totalPages = Math.max(1, Math.ceil(items.length / pageSize));
  const clampedPage = Math.min(Math.max(page, 1), totalPages);
  const start = (clampedPage - 1) * pageSize;
  return { pageItems: items.slice(start, start + pageSize), totalPages, page: clampedPage };
}

function animateCardToCenter(cardEl: HTMLElement): Promise<Animation> {
  const rect = cardEl.getBoundingClientRect();
  const clone = cardEl.cloneNode(true) as HTMLElement;
  clone.classList.add("entity-card-flyer");
  Object.assign(clone.style, { left: `${rect.left}px`, top: `${rect.top}px`, width: `${rect.width}px`, height: `${rect.height}px` });
  document.body.appendChild(clone);
  const targetW = Math.min(window.innerWidth * 0.3, 360);
  const targetH = Math.min(window.innerHeight * 0.28, 240);
  const targetX = (window.innerWidth - targetW) / 2;
  const targetY = (window.innerHeight - targetH) / 2;
  const animation = clone.animate([
    { left: `${rect.left}px`, top: `${rect.top}px`, width: `${rect.width}px`, height: `${rect.height}px` },
    { left: `${targetX}px`, top: `${targetY}px`, width: `${targetW}px`, height: `${targetH}px` },
  ], { duration: 390, easing: "cubic-bezier(.76,0,.24,1)", fill: "forwards" });
  return animation.finished.finally(() => clone.remove());
}

export function EntitySearch({ mode }: { mode: Mode }) {
  useBodyClass("entity-search-page");
  useFooterVisibility();
  const navigate = useNavigate();
  const { openAuth } = useOutletContext<PublicOutletContext>();
  const isAuthenticated = hasAccessToken();
  const gated = mode === "room" && !isAuthenticated;

  const [query, setQuery] = useState("");
  const [page, setPage] = useState(1);
  const [collection, setCollection] = useState<JsonApiCollection | null>(null);
  const [loading, setLoading] = useState(!gated);
  const [error, setError] = useState("");

  useEffect(() => {
    if (gated) return;
    if (mode === "teacher" && query.length === 1) return; // server rejects a 1-char q
    let active = true;
    setLoading(true);
    setError("");
    const request = mode === "teacher" ? api.listPublicTeachers(query) : api.listPublicRooms();
    request
      .then((response) => { if (active) setCollection(response); })
      .catch(() => active && setError(mode === "teacher" ? "Не удалось загрузить список преподавателей." : "Не удалось загрузить список аудиторий."))
      .finally(() => active && setLoading(false));
    return () => { active = false; };
    // Room mode intentionally omits `query` from the fetch dependency — the
    // server has no room filter; filtering happens client-side below.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [mode, gated, mode === "teacher" ? query : null]);

  useEffect(() => { setPage(1); }, [query]);

  if (gated) {
    return (
      <main className="entity-search-main">
        <div className="entity-search-empty" role="status">
          <Lock size={28} aria-hidden="true" />
          <p>Войдите, чтобы искать аудитории.</p>
          <button className="entity-page-arrow" type="button" onClick={openAuth}>Войти</button>
        </div>
      </main>
    );
  }

  const allItems = collection?.data ?? [];
  const filtered = mode === "room"
    ? allItems.filter((item) => normalizeSearchText(roomSearchableText(item, collection?.included)).includes(normalizeSearchText(query)))
    : allItems;
  const { pageItems, totalPages, page: currentPage } = paginate(filtered, page, PAGE_SIZE[mode]);

  async function openDetail(item: JsonApiResource, cardEl: HTMLButtonElement) {
    cardEl.classList.add("is-selected");
    await animateCardToCenter(cardEl).catch(() => undefined);
    navigate(`/${mode}/${item.id}`, { state: { resource: item, included: collection?.included } });
  }

  return (
    <main className="entity-search-main">
      <form className="entity-search-form" role="search" onSubmit={(event) => event.preventDefault()}>
        <label className="sr-only" htmlFor="entitySearchInput">Поиск</label>
        <span className="entity-search-icon" aria-hidden="true">⌕</span>
        <input id="entitySearchInput" className="entity-search-input" type="search" autoComplete="off" placeholder={PLACEHOLDER[mode]} value={query} onChange={(event) => setQuery(event.target.value)} />
        <button className={`entity-search-clear ${query ? "is-visible" : ""}`} type="button" aria-label="Очистить поиск" onClick={() => setQuery("")}>×</button>
      </form>

      {loading ? <p className="lookup-status" role="status">Ищем варианты…</p> : null}
      {error ? <p className="lookup-status">{error}</p> : null}

      <section className="entity-card-grid" aria-live="polite">
        {pageItems.map((item) => mode === "teacher"
          ? <TeacherCard key={item.id} resource={item} included={collection?.included} onOpen={openDetail} />
          : <RoomCard key={item.id} resource={item} included={collection?.included} onOpen={openDetail} />)}
      </section>

      {!loading && filtered.length === 0 ? <div className="entity-search-empty">—</div> : null}

      {totalPages > 1 && filtered.length > 0 ? (
        <nav className="entity-pagination" aria-label="Страницы результатов">
          <button className="entity-page-arrow" type="button" aria-label="Предыдущая страница" disabled={currentPage === 1} onClick={() => setPage(currentPage - 1)}>‹</button>
          <div className="entity-page-numbers">
            {Array.from({ length: totalPages }, (_, index) => index + 1).map((pageNumber) => (
              <button key={pageNumber} className={`entity-page-number${pageNumber === currentPage ? " is-active" : ""}`} type="button" aria-current={pageNumber === currentPage ? "page" : undefined} onClick={() => setPage(pageNumber)}>{pageNumber}</button>
            ))}
          </div>
          <button className="entity-page-arrow" type="button" aria-label="Следующая страница" disabled={currentPage === totalPages} onClick={() => setPage(currentPage + 1)}>›</button>
        </nav>
      ) : null}
    </main>
  );
}

function TeacherCard({ resource, included, onOpen }: { resource: JsonApiResource; included: JsonApiResource[] | undefined; onOpen: (item: JsonApiResource, cardEl: HTMLButtonElement) => void }) {
  const name = teacherFullName(resource.attributes);
  const post = resolveRelationshipAttributes(resource, included, "post");
  return (
    <button className="entity-card teacher-entity-card" type="button" aria-label={name} onClick={(event) => onOpen(resource, event.currentTarget)}>
      <span className="teacher-card-name">{name}</span>
      <span className="teacher-card-department">{stringAttribute(post, "name", "—")}</span>
    </button>
  );
}

function RoomCard({ resource, included, onOpen }: { resource: JsonApiResource; included: JsonApiResource[] | undefined; onOpen: (item: JsonApiResource, cardEl: HTMLButtonElement) => void }) {
  const number = stringAttribute(resource.attributes, "number");
  const roomType = resolveRelationshipAttributes(resource, included, "room_type");
  return (
    <button className="entity-card room-entity-card" type="button" aria-label={number} onClick={(event) => onOpen(resource, event.currentTarget)}>
      <span className="room-card-number">{number}</span>
      <span className="room-card-type">{stringAttribute(roomType, "name", "—")}</span>
    </button>
  );
}
```

The teacher card's subtitle line is `post.name` (e.g. "доцент") rather than the mockup's fabricated `departments` line — `Portal::Serializers::TeacherSerializer` exposes no kafedra/department field at all (verified against `apps/api_geteway/slices/portal/serializers/teacher_serializer.rb`), so there is nothing real to put there; `post` is the closest real per-teacher attribute already sideloaded by the same search response.

### Step 5: Run it to see it pass

Run: `npm test -- src/features/public/EntitySearch.test.tsx`
Expected: PASS.

### Step 6: Wire the routes

In `src/features/public/PublicApp.tsx`, add:

```tsx
import { EntitySearch } from "./EntitySearch";
// ...
          <Route path="/teacher" element={<EntitySearch mode="teacher" />} />
          <Route path="/room" element={<EntitySearch mode="room" />} />
```

### Step 7: Run the full suite and lint

Run: `npm test` and `npm run lint`
Expected: both PASS.

### Step 8: Stop here, changes are ready for review.

---

## Task 6: `SchedulePage` core — desktop grid, week/subgroup selectors, period-jump, transfers

The biggest task, split in two: this task delivers the desktop experience —
matrix table, real week toggle, subgroup filter, transfers panel, period-jump
control, event modal. Task 7 adds the mobile day-tabs/swipe/glance card on
top of the same `OccurrenceWeekGrid` component.

`OccurrenceWeekGrid` (in `src/features/schedule/WeekGrid.tsx`) is rewritten
from a day-list into a real weekday×time-slot matrix, matching
`schedule.html`'s `.schedule-table`. The plain `WeekGrid` function in the
same file (used by `AdminSchedule`/`TeacherSelfService`) is **not touched**.
Because there is no server-exposed enumerable time-slot list on the public
API (`OccurrenceWindow` only carries per-occurrence `time_slot_start_time`/
`_end_time`), the matrix's rows are derived from the **unique start times
actually present in the given week**, sorted ascending — the data-driven
equivalent of `schedule.js`'s hardcoded `times` array.

**Architecture note — why `OccurrenceWeekGrid` takes a single week, not a
window:** the old signature took the whole `OccurrenceWindow` and picked a
week internally via its own tab state. Three different pages now need a
"pick one of the two real weeks" control (`SchedulePage`, `TeacherDetail`,
`RoomDetail` — Task 8), each with different surrounding chrome (subgroup
selector + transfers only on `SchedulePage`). Splitting `WeekSelector` out as
a shared, page-owned component and handing `OccurrenceWeekGrid` an
already-resolved `week: DateScheduleWeek | null` keeps the grid a pure
presentational renderer reusable by all three pages without it knowing which
page it's in.

**Files:**
- Create: `src/features/public/WeekSelector.tsx` + `src/features/public/WeekSelector.test.tsx`
- Modify: `src/features/schedule/WeekGrid.tsx` (rewrite `OccurrenceWeekGrid`; leave `WeekGrid` untouched)
- Modify: `src/features/schedule/WeekGrid.test.tsx` (replace the `OccurrenceWeekGrid` describe block; leave the `WeekGrid` describe block untouched)
- Create: `src/features/public/SchedulePage.tsx` + `src/features/public/SchedulePage.test.tsx`
- Modify: `src/features/public/PublicApp.tsx` (add the `/schedule` route)

**Interfaces:**
- Consumes: `normalizeOccurrenceWindow` (unchanged, `src/features/schedule/normalize.ts`), `DateScheduleWeek`/`DateScheduleEvent` (unchanged, `src/features/schedule/types.ts`), `api.groupSchedule`/`api.teacherSchedule`/`api.roomOccupancyWindow`/`api.publicGroupPeriods`/`PERIOD_LABELS` (`src/lib/api.ts`), `hasAccessToken` (Task 1), `useBodyClass`/`useFooterVisibility` (Task 2).
- Produces: `WeekSelector({ weeks, activeIndex, onSelect })` (Task 8 reuses this directly).
- Produces (from `WeekGrid.tsx`, both exported): `OccurrenceWeekGrid({ week: DateScheduleWeek | null, loading?: boolean, subgroupFilter?: number | "all" })` — **signature changed** from the old `{ window: OccurrenceWindow | null, loading?: boolean }`. Task 8 consumes this new signature directly.
- Produces: `<SchedulePage />`, mounted at `/schedule`, reading `?group=`/`?teacher=`/`?room=`.

### Step 1: Write the failing test for `WeekSelector`

Create `src/features/public/WeekSelector.test.tsx`:

```tsx
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { DateScheduleWeek } from "../schedule/types";
import { WeekSelector } from "./WeekSelector";

describe("WeekSelector", () => {
  it("always shows Нечётная before Чётная regardless of which week is chronologically current", () => {
    const weeks: DateScheduleWeek[] = [
      { start_date: "2026-11-09", end_date: "2026-11-15", parity: "even", iso_week: 46, is_current: true, label: "", days: [] },
      { start_date: "2026-11-16", end_date: "2026-11-22", parity: "odd", iso_week: 47, is_current: false, label: "", days: [] },
    ];
    const markup = renderToStaticMarkup(<WeekSelector weeks={weeks} activeIndex={0} onSelect={() => undefined} />);
    expect(markup.indexOf("Нечётная")).toBeLessThan(markup.indexOf("Чётная"));
    expect(markup).toContain("is-current");
  });
});
```

### Step 2: Run it to see it fail

Run: `npm test -- src/features/public/WeekSelector.test.tsx`
Expected: FAIL — cannot find module `./WeekSelector`.

### Step 3: Implement `WeekSelector.tsx`

DOM ported from `schedule_home/schedule.html` lines 39-42 (`#weekSelector`) — the mockup always renders exactly two fixed-order buttons, so this maps each button to whichever `weeks[]` entry has the matching `parity`, not to array position:

```tsx
import type { DateScheduleWeek } from "../schedule/types";

export function WeekSelector({ weeks, activeIndex, onSelect }: { weeks: DateScheduleWeek[]; activeIndex: number; onSelect: (index: number) => void }) {
  const buttons: Array<{ parity: "odd" | "even"; label: string }> = [
    { parity: "odd", label: "Нечётная" },
    { parity: "even", label: "Чётная" },
  ];

  return (
    <div className="schedule-selector" role="group" aria-label="Неделя">
      {buttons.map((button) => {
        const index = weeks.findIndex((week) => week.parity === button.parity);
        const week = index >= 0 ? weeks[index] : null;
        return (
          <button
            key={button.parity}
            type="button"
            data-week={button.parity}
            disabled={index < 0}
            className={`${index === activeIndex ? "is-active" : ""} ${week?.is_current ? "is-current" : ""}`}
            title={week?.is_current ? `Текущая неделя · ${week.iso_week}` : ""}
            onClick={() => index >= 0 && onSelect(index)}
          >
            {button.label}
          </button>
        );
      })}
    </div>
  );
}
```

### Step 4: Run it to see it pass

Run: `npm test -- src/features/public/WeekSelector.test.tsx`
Expected: PASS.

### Step 5: Write the failing test for the rewritten `OccurrenceWeekGrid`

In `src/features/schedule/WeekGrid.test.tsx`, **replace the entire
`OccurrenceWeekGrid` describe block** (and its `occurrenceWindow`/
`occurrence` fixture helpers) — the `WeekGrid` describe block above it stays
exactly as-is. Replace from `function occurrenceWindow(...)` through the end
of the `describe("OccurrenceWeekGrid", ...)` block with:

```tsx
function week(overrides: Partial<DateScheduleWeek> = {}): DateScheduleWeek {
  return {
    start_date: "2026-11-09", end_date: "2026-11-15", parity: "even", iso_week: 46, is_current: true,
    label: "Тек. неделя 09.11–15.11 · чётная",
    days: Array.from({ length: 7 }, (_, index) => ({
      date: `2026-11-${String(9 + index).padStart(2, "0")}`, label: "", weekday: index + 1, events: [], isHoliday: false,
    })),
    ...overrides,
  };
}

function withDayEvents(base: DateScheduleWeek, date: string, events: DateScheduleEvent[]): DateScheduleWeek {
  return { ...base, days: base.days.map((day) => (day.date === date ? { ...day, events } : day)) };
}

function event(overrides: Partial<DateScheduleEvent> = {}): DateScheduleEvent {
  return {
    id: "1", date: "2026-11-09", startTime: "08:30", endTime: "09:55", subject: "Философия", activityType: "лекция",
    rooms: ["ауд. 1315"], teachers: ["Рябцева Наталья Адамовна"], group: "СИ 3 курс", course: 3, speciality: "СИ",
    kafedra: "Философия, история и политология", subgroupNumber: 0, state: "scheduled", originalDate: null, transferReason: null,
    ...overrides,
  };
}

describe("OccurrenceWeekGrid", () => {
  it("renders a desktop matrix cell for a scheduled class, with its weekday short label", () => {
    const markup = renderToStaticMarkup(<OccurrenceWeekGrid week={withDayEvents(week(), "2026-11-09", [event()])} />);
    expect(markup).toContain("Философия");
    expect(markup).toContain("ауд. 1315");
    expect(markup).toContain("ПН");
  });

  it("distinguishes 'nothing selected yet' from a null week", () => {
    expect(renderToStaticMarkup(<OccurrenceWeekGrid week={null} />)).toContain("Здесь появится расписание");
  });

  it("shows a loading state instead of the table while fetching", () => {
    expect(renderToStaticMarkup(<OccurrenceWeekGrid week={null} loading />)).toContain("Загружаем расписание");
  });

  it("filters by subgroup, always keeping subgroup-0 (shared) events visible", () => {
    const filled = withDayEvents(week(), "2026-11-09", [
      event({ id: "a", subgroupNumber: 1, subject: "Группа 1" }),
      event({ id: "b", subgroupNumber: 2, subject: "Группа 2" }),
      event({ id: "c", subgroupNumber: 0, subject: "Общее" }),
    ]);
    const markup = renderToStaticMarkup(<OccurrenceWeekGrid week={filled} subgroupFilter={1} />);
    expect(markup).toContain("Группа 1");
    expect(markup).toContain("Общее");
    expect(markup).not.toContain("Группа 2");
  });
});
```

Update the file's imports: remove `OccurrenceWindow` (no longer used by this file) and add `DateScheduleEvent, DateScheduleWeek` from `./types`.

### Step 6: Run it to see it fail

Run: `npm test -- src/features/schedule/WeekGrid.test.tsx`
Expected: FAIL — `OccurrenceWeekGrid` still expects a `window` prop, not `week`.

### Step 7: Rewrite `OccurrenceWeekGrid` in `WeekGrid.tsx`

Replace everything from the file's `// ── Date-resolved grid ...` comment
(the `OccurrenceWeekGrid` export and its `OccurrenceCard` helper) through
the end of the file with:

```tsx
// ── Date-resolved grid ──────────────────────────────────────────────────
// The public portal's grid: a real weekday × time-slot matrix for ONE
// already-resolved week (see src/features/public/SchedulePage.tsx and
// src/features/public/TeacherDetail.tsx/RoomDetail.tsx, which each own a
// WeekSelector and hand this component the chosen week). Kept in this file
// alongside WeekGrid (rather than replacing it) because WeekGrid's
// day-of-week/JSON:API contract is still what AdminSchedule and
// TeacherSelfService feed it from the unchanged admin schedule endpoints.
const WEEKDAY_SHORT_UPPER = ["ПН", "ВТ", "СР", "ЧТ", "ПТ", "СБ", "ВС"];

const STATE_LABEL: Record<DateScheduleEvent["state"], string | null> = {
  scheduled: null,
  moved_in: "Перенесено",
  cancelled: "Отменено",
  reserved: "Резерв",
};

const WEEKDAY_FULL = ["Воскресенье", "Понедельник", "Вторник", "Среда", "Четверг", "Пятница", "Суббота"];

function weekdayFullName(isoDate: string): string {
  const [year, month, day] = isoDate.split("-").map(Number);
  return WEEKDAY_FULL[new Date(Date.UTC(year, month - 1, day)).getUTCDay()];
}

// Cosmetic "is-today" column highlight only — never used for parity or any
// scheduling decision, which always come from the server (see api.weekParity's
// own doc comment: "Never recompute this in the client").
function todayIso(): string {
  return new Date().toISOString().slice(0, 10);
}

type TimeSlot = { startTime: string; endTime: string };

function timeSlotsForWeek(week: DateScheduleWeek): TimeSlot[] {
  const seen = new Map<string, TimeSlot>();
  for (const day of week.days) {
    for (const occurrenceEvent of day.events) {
      seen.set(occurrenceEvent.startTime, { startTime: occurrenceEvent.startTime, endTime: occurrenceEvent.endTime });
    }
  }
  return [...seen.values()].sort((left, right) => left.startTime.localeCompare(right.startTime));
}

function visibleEvents(day: DateScheduleWeek["days"][number], slot: TimeSlot, subgroupFilter: number | "all"): DateScheduleEvent[] {
  return day.events.filter((occurrenceEvent) => occurrenceEvent.startTime === slot.startTime && (subgroupFilter === "all" || occurrenceEvent.subgroupNumber === 0 || occurrenceEvent.subgroupNumber === subgroupFilter));
}

function EventButton({ event: occurrenceEvent, onSelect }: { event: DateScheduleEvent; onSelect: (event: DateScheduleEvent) => void }) {
  const stateClass = occurrenceEvent.state === "cancelled" ? "is-removed" : occurrenceEvent.state === "moved_in" ? "is-added" : "";
  return (
    <button className={`schedule-event ${stateClass}`} type="button" aria-label={occurrenceEvent.subject} onClick={() => onSelect(occurrenceEvent)}>
      <span className="schedule-event-top"><span className="schedule-event-type">{occurrenceEvent.activityType}</span><span className="schedule-event-room">{occurrenceEvent.rooms.join(" + ") || "—"}</span></span>
      <span className="schedule-event-name">{occurrenceEvent.subject}</span>
      <span className="schedule-event-foot"><span>{occurrenceEvent.group ?? (occurrenceEvent.subgroupNumber ? `${occurrenceEvent.subgroupNumber} ПГ` : "")}</span><span>↗</span></span>
    </button>
  );
}

function DesktopTable({ week, subgroupFilter, onSelectEvent }: { week: DateScheduleWeek; subgroupFilter: number | "all"; onSelectEvent: (event: DateScheduleEvent) => void }) {
  const slots = timeSlotsForWeek(week);
  const days = week.days.filter((day) => day.weekday <= 6); // Пн-Сб — schedule_home never shows Sunday as a school day.
  const today = todayIso();

  return (
    <section className="schedule-table-wrap" aria-label="Таблица расписания">
      <div className="schedule-table">
        <div className="schedule-head schedule-head--time">{week.iso_week}</div>
        {days.map((day) => <div className={`schedule-head ${day.date === today ? "is-today" : ""}`} key={day.date}>{WEEKDAY_SHORT_UPPER[day.weekday - 1]}</div>)}
        {slots.map((slot) => (
          <Fragment key={slot.startTime}>
            <div className="schedule-time"><div>{slot.startTime}<span>{slot.endTime}</span></div></div>
            {days.map((day) => {
              const cellEvents = visibleEvents(day, slot, subgroupFilter);
              return (
                <div className={`schedule-slot ${day.date === today ? "is-today" : ""}`} key={day.date}>
                  {cellEvents.length ? cellEvents.map((occurrenceEvent) => <EventButton key={occurrenceEvent.id} event={occurrenceEvent} onSelect={onSelectEvent} />) : <span className="schedule-empty-mark">·</span>}
                </div>
              );
            })}
          </Fragment>
        ))}
      </div>
    </section>
  );
}

function EventDetailModal({ event: occurrenceEvent, onClose }: { event: DateScheduleEvent | null; onClose: () => void }) {
  const open = Boolean(occurrenceEvent);
  const statusLabel = occurrenceEvent ? (STATE_LABEL[occurrenceEvent.state] ?? occurrenceEvent.activityType) : "";
  const rooms = occurrenceEvent?.rooms ?? [];
  const teachers = occurrenceEvent?.teachers ?? [];

  return (
    <div className={`schedule-event-backdrop ${open ? "is-open" : ""}`} aria-hidden={!open} onClick={(domEvent) => { if (domEvent.target === domEvent.currentTarget) onClose(); }}>
      <section className="schedule-event-panel" role="dialog" aria-modal="true" aria-label="Занятие">
        <button className="schedule-panel-close" type="button" aria-label="Закрыть" onClick={onClose}>×</button>
        {occurrenceEvent ? <>
          <div className={`event-detail-status ${occurrenceEvent.state !== "scheduled" ? `is-${occurrenceEvent.state}` : ""}`}>{rooms.length ? `${statusLabel} · ${rooms.join(" + ")}` : statusLabel}</div>
          <div className="event-detail-body">
            <div className="event-detail-name">{occurrenceEvent.subject}</div>
            <div className="event-detail-grid">
              <div className="event-detail-item"><small>{occurrenceEvent.group ? "Группа" : "Курс"}</small><strong>{occurrenceEvent.group ?? "—"}</strong></div>
              <div className="event-detail-item"><small>{teachers.length > 1 ? "Преподаватели" : "Преподаватель"}</small><strong>{teachers.length ? teachers.join(", ") : "—"}</strong></div>
              <div className="event-detail-item"><small>{rooms.length > 1 ? "Аудитории" : "Аудитория"}</small><strong>{rooms.length ? rooms.join(", ") : "—"}</strong></div>
              <div className="event-detail-item"><small>Время</small><strong>{weekdayFullName(occurrenceEvent.date)} · {occurrenceEvent.startTime}–{occurrenceEvent.endTime}</strong></div>
              <div className="event-detail-item"><small>Подгруппа</small><strong>{occurrenceEvent.subgroupNumber ? String(occurrenceEvent.subgroupNumber) : "Все"}</strong></div>
              {occurrenceEvent.state === "moved_in" && occurrenceEvent.originalDate ? <div className="event-detail-item"><small>Перенос</small><strong>с {formatShortDate(occurrenceEvent.originalDate)}</strong></div> : null}
              {occurrenceEvent.state === "cancelled" && occurrenceEvent.transferReason ? <div className="event-detail-item"><small>Причина</small><strong>{occurrenceEvent.transferReason}</strong></div> : null}
            </div>
          </div>
        </> : null}
      </section>
    </div>
  );
}

export function OccurrenceWeekGrid({ week, loading = false, subgroupFilter = "all" }: { week: DateScheduleWeek | null; loading?: boolean; subgroupFilter?: number | "all" }) {
  const [selectedEvent, setSelectedEvent] = useState<DateScheduleEvent | null>(null);

  if (loading) return <div className="lookup-empty" aria-busy="true"><PulseLine className="lookup-day-pulse" /><h2>Загружаем расписание…</h2></div>;
  if (!week) return <div className="lookup-empty"><h2>Здесь появится расписание</h2><p>Выберите параметры выше.</p></div>;

  return (
    <>
      <DesktopTable week={week} subgroupFilter={subgroupFilter} onSelectEvent={setSelectedEvent} />
      <EventDetailModal event={selectedEvent} onClose={() => setSelectedEvent(null)} />
    </>
  );
}
```

Update the top-of-file imports: `import { Fragment, useState } from "react";` (was `useEffect, useState`), and change `import { formatShortDate, normalizeOccurrenceWindow, normalizeSchedule } from "./normalize";` to `import { formatShortDate, normalizeSchedule } from "./normalize";` (`normalizeOccurrenceWindow` moves to the page components in this task). Also run `grep -n "SemesterContext\|TransferNotice\|OccurrenceState" src/features/schedule/WeekGrid.tsx` after editing — the file's top `import type { OccurrenceState, SemesterContext, TransferNotice } from "../../lib/api";` line references types this rewrite no longer uses directly (`DateScheduleEvent["state"]` replaces the `OccurrenceState` usage); trim that import line down to whatever it still needs (likely nothing — delete the line if the grep comes back empty).

The one shared modal, `EventDetailModal`, deliberately uses `schedule.html`'s
class names (`schedule-event-backdrop`/`schedule-event-panel`) for **every**
caller, including Task 8's `TeacherDetail`/`RoomDetail` — the mockup's
`teacher-event-*` classes wrap an identical `event-detail-grid` field list
with only cosmetic differences; consolidating to one modal keeps the field
logic (teachers/rooms/time/subgroup) in exactly one place. This is the same
DRY consolidation the design spec already blessed for the four shared
dialogs, applied to a fifth actually-identical piece of markup.

### Step 8: Run it to see it pass

Run: `npm test -- src/features/schedule/WeekGrid.test.tsx`
Expected: PASS (both the untouched `WeekGrid` describe block and the rewritten `OccurrenceWeekGrid` one).

### Step 9: Write the failing test for `SchedulePage`

Create `src/features/public/SchedulePage.test.tsx`:

```tsx
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter, Outlet, Route, Routes } from "react-router-dom";
import { SchedulePage } from "./SchedulePage";

class MemoryStorage implements Storage {
  private values = new Map<string, string>();
  get length() { return this.values.size; }
  clear() { this.values.clear(); }
  getItem(key: string) { return this.values.get(key) ?? null; }
  key(index: number) { return [...this.values.keys()][index] ?? null; }
  removeItem(key: string) { this.values.delete(key); }
  setItem(key: string, value: string) { this.values.set(key, value); }
}

afterEach(() => { vi.unstubAllGlobals(); });

function renderAt(path: string) {
  return renderToStaticMarkup(
    <MemoryRouter initialEntries={[path]}>
      <Routes>
        <Route element={<Outlet context={{ openGuide: () => undefined, openAuth: () => undefined }} />}>
          <Route path="/schedule" element={<SchedulePage />} />
        </Route>
      </Routes>
    </MemoryRouter>,
  );
}

describe("SchedulePage", () => {
  it("shows the subgroup selector for group mode but not teacher/room mode", () => {
    expect(renderAt("/schedule?group=513")).toContain('aria-label="Подгруппа"');
    expect(renderAt("/schedule?teacher=41")).not.toContain('aria-label="Подгруппа"');
  });

  it("gates room mode behind login when no token is present", () => {
    const markup = renderAt("/schedule?room=14");
    expect(markup).toContain("Войдите, чтобы смотреть занятость аудиторий");
  });

  it("shows room mode content once a token is present", () => {
    const storage = new MemoryStorage();
    storage.setItem("milk_schedule_access_token", "token-value");
    vi.stubGlobal("sessionStorage", storage);
    const markup = renderAt("/schedule?room=14");
    expect(markup).not.toContain("Войдите, чтобы смотреть занятость аудиторий");
  });
});
```

### Step 10: Run it to see it fail

Run: `npm test -- src/features/public/SchedulePage.test.tsx`
Expected: FAIL — cannot find module `./SchedulePage`.

### Step 11: Implement `SchedulePage.tsx`

DOM ported from `schedule_home/schedule.html` lines 26-63 (`.schedule-controls`/`.schedule-table-wrap`/`.schedule-legend`) and the transfers dialog from lines 82-87. The mobile section (`.mobile-schedule`, lines 49-53) is added in Task 7 — omit it here:

```tsx
import { useEffect, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { Lock } from "lucide-react";
import { api, PERIOD_LABELS, type OccurrenceWindow, type PeriodKind } from "../../lib/api";
import { hasAccessToken } from "../../lib/session";
import { normalizeOccurrenceWindow, formatShortDate } from "../schedule/normalize";
import { OccurrenceWeekGrid } from "../schedule/WeekGrid";
import { WeekSelector } from "./WeekSelector";
import { useBodyClass, useFooterVisibility } from "./bodyClass";

type Mode = "group" | "teacher" | "room";

export function SchedulePage() {
  useBodyClass("schedule-page");
  useFooterVisibility();
  const [searchParams] = useSearchParams();
  const isAuthenticated = hasAccessToken();

  const groupId = searchParams.get("group");
  const teacherId = searchParams.get("teacher");
  const roomId = searchParams.get("room");
  const mode: Mode = groupId ? "group" : teacherId ? "teacher" : "room";
  const entityId = groupId ?? teacherId ?? roomId ?? "";
  const gated = mode === "room" && !isAuthenticated;

  const [date, setDate] = useState<string | undefined>(undefined);
  const [result, setResult] = useState<OccurrenceWindow | null>(null);
  const [loading, setLoading] = useState(true);
  const [subgroup, setSubgroup] = useState<number | "all">("all");
  const [activeWeekIndex, setActiveWeekIndex] = useState(0);
  const [transfersOpen, setTransfersOpen] = useState(false);
  const [periods, setPeriods] = useState<Array<{ period_kind: PeriodKind; start_date: string; end_date: string }>>([]);

  useEffect(() => {
    if (!entityId || gated) { setLoading(false); return; }
    let active = true;
    setLoading(true);
    const request = mode === "group" ? api.groupSchedule(entityId, date)
      : mode === "teacher" ? api.teacherSchedule(entityId, date)
      : api.roomOccupancyWindow(entityId, date);
    request.then((response) => { if (active) setResult(response); }).finally(() => active && setLoading(false));
    return () => { active = false; };
  }, [mode, entityId, date, gated]);

  useEffect(() => {
    if (mode !== "group" || !groupId) { setPeriods([]); return; }
    let active = true;
    api.publicGroupPeriods(groupId).then((response) => { if (active) setPeriods(response.periods); }).catch(() => undefined);
    return () => { active = false; };
  }, [mode, groupId]);

  const schedule = normalizeOccurrenceWindow(result);

  useEffect(() => {
    const currentIndex = schedule.weeks.findIndex((week) => week.is_current);
    setActiveWeekIndex(currentIndex >= 0 ? currentIndex : 0);
    // Only re-sync when a new window has actually loaded, not on every render.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [result]);

  if (gated) {
    return (
      <main className="schedule-main">
        <div className="entity-search-empty" role="status">
          <Lock size={28} aria-hidden="true" />
          <p>Войдите, чтобы смотреть занятость аудиторий.</p>
        </div>
      </main>
    );
  }

  const identity = mode === "group" ? (result?.meta.semester?.label ?? "") : mode === "teacher" ? "Преподаватель" : `Ауд. ${roomId}`;
  const activeWeek = schedule.weeks[activeWeekIndex] ?? null;

  return (
    <main className="schedule-main">
      <section className="schedule-controls" aria-label="Параметры расписания">
        <div className="schedule-identity">
          <span className="schedule-group-code">{identity}</span>
        </div>

        {mode === "group" ? (
          <div className="schedule-selector" role="group" aria-label="Подгруппа">
            <button type="button" className={subgroup === "all" ? "is-active" : ""} onClick={() => setSubgroup("all")}>Все</button>
            <button type="button" className={subgroup === 1 ? "is-active" : ""} onClick={() => setSubgroup(1)}>1</button>
            <button type="button" className={subgroup === 2 ? "is-active" : ""} onClick={() => setSubgroup(2)}>2</button>
          </div>
        ) : null}

        <WeekSelector weeks={schedule.weeks} activeIndex={activeWeekIndex} onSelect={setActiveWeekIndex} />

        {result && result.transfers.length > 0 ? (
          <button className="transfers-button" type="button" aria-expanded={transfersOpen} onClick={() => setTransfersOpen(true)}>
            <span>Переносы</span><span className="transfers-count">{result.transfers.length}</span>
          </button>
        ) : null}
      </section>

      {mode === "group" && periods.length > 0 ? (
        <section className="schedule-selector" role="group" aria-label="Период">
          {periods.map((period) => (
            <button key={period.period_kind} type="button" onClick={() => setDate(period.start_date)}>{PERIOD_LABELS[period.period_kind]}</button>
          ))}
        </section>
      ) : null}

      <OccurrenceWeekGrid week={activeWeek} loading={loading} subgroupFilter={mode === "group" ? subgroup : "all"} />

      {result && result.transfers.length > 0 ? <TransfersDialog open={transfersOpen} onClose={() => setTransfersOpen(false)} transfers={result.transfers} /> : null}
    </main>
  );
}

function TransfersDialog({ open, onClose, transfers }: { open: boolean; onClose: () => void; transfers: OccurrenceWindow["transfers"] }) {
  return (
    <div className={`transfers-backdrop ${open ? "is-open" : ""}`} aria-hidden={!open} onClick={(event) => { if (event.target === event.currentTarget) onClose(); }}>
      <section className="transfers-panel" role="dialog" aria-modal="true" aria-label="Переносы">
        <button className="schedule-panel-close" type="button" aria-label="Закрыть" onClick={onClose}>×</button>
        <div className="transfers-list">
          {transfers.map((transfer) => (
            <div className={`transfer-row is-${transfer.transfer_type === "moved" ? "added" : "removed"}`} key={transfer.id}>
              <span className="transfer-sign">{transfer.transfer_type === "moved" ? "+" : "−"}</span>
              <div><div className="transfer-subject">{transfer.subject_name ?? "Занятие"}</div><div className="transfer-meta">{transfer.reason ?? ""}</div></div>
              <div className="transfer-where">{formatShortDate(transfer.original_date)}{transfer.new_date ? ` → ${formatShortDate(transfer.new_date)}` : ""}<br />{transfer.time_slot_start_time}</div>
            </div>
          ))}
        </div>
      </section>
    </div>
  );
}
```

### Step 12: Run it to see it pass

Run: `npm test -- src/features/public/SchedulePage.test.tsx`
Expected: PASS.

### Step 13: Wire the route

In `src/features/public/PublicApp.tsx`, add:

```tsx
import { SchedulePage } from "./SchedulePage";
// ...
          <Route path="/schedule" element={<SchedulePage />} />
```

### Step 14: Run the full suite and lint

Run: `npm test` and `npm run lint`
Expected: both PASS.

### Step 15: Stop here, changes are ready for review.

---

## Task 7: `SchedulePage` mobile — day tabs, glance card, swipe, agenda

Adds `schedule.html`'s mobile section (`.mobile-schedule`, lines 49-53) into
`OccurrenceWeekGrid` alongside the desktop table built in Task 6 — both
render unconditionally in the DOM; the copied CSS's media queries decide
which is visible, matching the mockup's own approach (it never conditionally
renders either block in JS, only via CSS). Scoped to the **single already-
selected week** `OccurrenceWeekGrid` receives — "next event" look-ahead for
the glance card only searches the rest of that week's days, never into the
other week (there are only ever two weeks shown at all, per the "no ±2-weeks
pager" constraint — reaching into the other week would need a second
`WeekSelector` click anyway).

**Files:**
- Modify: `src/features/schedule/WeekGrid.tsx` (add `MobileSchedule`/`MobileGlance` to `OccurrenceWeekGrid`'s render)
- Modify: `src/features/schedule/WeekGrid.test.tsx` (extend the `OccurrenceWeekGrid` describe block)

**Interfaces:**
- Consumes: everything already in `WeekGrid.tsx` from Task 6 (`DateScheduleWeek`, `DateScheduleEvent`, `STATE_LABEL`, `WEEKDAY_SHORT_UPPER`, `todayIso`, `formatShortDate`, `plural` — the last already exists in this file for the admin `WeekGrid` export, reused here).
- Produces: no new exports — `OccurrenceWeekGrid`'s public signature from Task 6 is unchanged; this task only adds to what it renders internally.

### Step 1: Write the failing tests

In `src/features/schedule/WeekGrid.test.tsx`, add two more `it` blocks inside the existing `describe("OccurrenceWeekGrid", ...)` block (after the subgroup-filter test from Task 6):

```tsx
  it("renders the mobile day tabs and agenda for the same week data", () => {
    const filled = withDayEvents(week(), "2026-11-09", [event()]);
    const markup = renderToStaticMarkup(<OccurrenceWeekGrid week={filled} />);
    expect(markup).toContain('role="tablist"');
    expect(markup).toContain("mobile-agenda-event");
  });

  it("shows the empty agenda state for a day with no classes", () => {
    const markup = renderToStaticMarkup(<OccurrenceWeekGrid week={week()} />);
    expect(markup).toContain("Нет занятий");
  });
```

### Step 2: Run it to see it fail

Run: `npm test -- src/features/schedule/WeekGrid.test.tsx`
Expected: FAIL — neither `role="tablist"` nor `mobile-agenda-event` appear yet (Task 6 only rendered the desktop table).

### Step 3: Add the mobile components to `WeekGrid.tsx`

Add `useRef` and `type { TouchEvent as ReactTouchEvent }` to the file's React import (`import { Fragment, useRef, useState } from "react"; import type { TouchEvent as ReactTouchEvent } from "react";`). Insert the following two components right before `export function OccurrenceWeekGrid` (DOM ported from `schedule_home/schedule.html` lines 49-53; behavior ported from `schedule.js`'s `renderMobileGlance`/`renderMobileSchedule`/the `touchstart`/`touchend` listeners, lines 204-326):

```tsx
function isoTimeToMinutes(value: string): number {
  const [hours, minutes] = value.split(":").map(Number);
  return hours * 60 + minutes;
}

function MobileGlance({ week, day, now }: { week: DateScheduleWeek; day: DateScheduleWeek["days"][number]; now: Date }) {
  const isViewingToday = week.is_current && day.date === todayIso();

  if (!isViewingToday) {
    const first = day.events[0];
    return (
      <div className={`mobile-glance ${day.events.length === 0 ? "is-finished" : ""}`}>
        <div className="mobile-glance-copy">
          <span className="mobile-glance-kicker">{formatShortDate(day.date)}</span>
          <strong className="mobile-glance-main">{day.events.length ? `${day.events.length} ${plural(day.events.length, "занятие", "занятия", "занятий")}` : "Нет занятий"}</strong>
          {first ? <span className="mobile-glance-meta">Первое · {first.subject}</span> : <span className="mobile-glance-meta">{week.parity === "even" ? "Чётная" : "Нечётная"} неделя</span>}
        </div>
        <span className="mobile-glance-time">{first ? first.startTime : "—"}</span>
      </div>
    );
  }

  const currentMinutes = now.getUTCHours() * 60 + now.getUTCMinutes();
  const current = day.events.find((event) => currentMinutes >= isoTimeToMinutes(event.startTime) && currentMinutes <= isoTimeToMinutes(event.endTime));
  if (current) {
    return (
      <div className="mobile-glance is-now">
        <div className="mobile-glance-copy"><span className="mobile-glance-kicker">Сейчас</span><strong className="mobile-glance-main">{current.subject}</strong><span className="mobile-glance-meta">{current.activityType} · {current.rooms.join(" + ") || "—"}</span></div>
        <span className="mobile-glance-time">{current.endTime}</span>
      </div>
    );
  }

  const next = day.events.find((event) => isoTimeToMinutes(event.startTime) > currentMinutes);
  if (next) {
    return (
      <div className="mobile-glance">
        <div className="mobile-glance-copy"><span className="mobile-glance-kicker">Далее</span><strong className="mobile-glance-main">{next.subject}</strong><span className="mobile-glance-meta">{next.activityType} · {next.rooms.join(" + ") || "—"}</span></div>
        <span className="mobile-glance-time">{next.startTime}</span>
      </div>
    );
  }

  return (
    <div className="mobile-glance is-finished">
      <div className="mobile-glance-copy"><span className="mobile-glance-kicker">Сегодня</span><strong className="mobile-glance-main">На сегодня всё</strong><span className="mobile-glance-meta">{week.iso_week} неделя</span></div>
      <span className="mobile-glance-time">✓</span>
    </div>
  );
}

function MobileSchedule({ week, subgroupFilter, onSelectEvent }: { week: DateScheduleWeek; subgroupFilter: number | "all"; onSelectEvent: (event: DateScheduleEvent) => void }) {
  const days = week.days.filter((day) => day.weekday <= 6);
  const now = new Date();
  const today = todayIso();
  const defaultIndex = Math.max(0, days.findIndex((day) => day.date === today));
  const [selectedIndex, setSelectedIndex] = useState(defaultIndex);
  const touchStartX = useRef<number | null>(null);

  const day = days[Math.min(selectedIndex, days.length - 1)];
  const filteredEvents = day.events.filter((event) => subgroupFilter === "all" || event.subgroupNumber === 0 || event.subgroupNumber === subgroupFilter);

  function handleTouchStart(event: ReactTouchEvent) { touchStartX.current = event.changedTouches[0]?.clientX ?? null; }
  function handleTouchEnd(event: ReactTouchEvent) {
    if (touchStartX.current == null) return;
    const endX = event.changedTouches[0]?.clientX ?? touchStartX.current;
    const delta = endX - touchStartX.current;
    touchStartX.current = null;
    if (Math.abs(delta) < 58) return;
    const nextIndex = delta < 0 ? Math.min(days.length - 1, selectedIndex + 1) : Math.max(0, selectedIndex - 1);
    setSelectedIndex(nextIndex);
  }

  return (
    <section className="mobile-schedule" aria-label="Расписание по дням">
      <div className="mobile-day-tabs" role="tablist" aria-label="День недели">
        {days.map((tabDay, index) => (
          <button key={tabDay.date} type="button" role="tab" aria-selected={index === selectedIndex} className={`mobile-day-tab ${index === selectedIndex ? "is-active" : ""} ${week.is_current && tabDay.date === today ? "is-today" : ""}`} onClick={() => setSelectedIndex(index)}>
            <strong>{WEEKDAY_SHORT_UPPER[tabDay.weekday - 1]}</strong><span>{tabDay.date.slice(-2)}</span>
          </button>
        ))}
      </div>

      <MobileGlance week={week} day={{ ...day, events: filteredEvents }} now={now} />

      <div className="mobile-agenda" onTouchStart={handleTouchStart} onTouchEnd={handleTouchEnd}>
        {filteredEvents.length ? filteredEvents.map((event) => (
          <button className={`mobile-agenda-event ${event.state === "moved_in" ? "is-added" : event.state === "cancelled" ? "is-removed" : ""}`} type="button" key={event.id} onClick={() => onSelectEvent(event)}>
            <span className="mobile-agenda-time">{event.startTime}<small>{event.endTime}</small></span>
            <span className="mobile-agenda-body">
              <span className="mobile-agenda-top"><span>{event.activityType}</span><span>{event.rooms.join(" + ") || "—"}</span></span>
              <span className="mobile-agenda-name">{event.subject}</span>
              {STATE_LABEL[event.state] ? <span className="mobile-agenda-note">{STATE_LABEL[event.state]}</span> : null}
            </span>
            <span className="mobile-agenda-arrow" aria-hidden="true">↗</span>
          </button>
        )) : <div className="mobile-empty"><strong>—</strong><span>Нет занятий</span></div>}
      </div>
    </section>
  );
}
```

### Step 4: Wire `MobileSchedule` into `OccurrenceWeekGrid`

Change `OccurrenceWeekGrid`'s return (from Task 6) from:

```tsx
  return (
    <>
      <DesktopTable week={week} subgroupFilter={subgroupFilter} onSelectEvent={setSelectedEvent} />
      <EventDetailModal event={selectedEvent} onClose={() => setSelectedEvent(null)} />
    </>
  );
```

to:

```tsx
  return (
    <>
      <MobileSchedule week={week} subgroupFilter={subgroupFilter} onSelectEvent={setSelectedEvent} />
      <DesktopTable week={week} subgroupFilter={subgroupFilter} onSelectEvent={setSelectedEvent} />
      <EventDetailModal event={selectedEvent} onClose={() => setSelectedEvent(null)} />
    </>
  );
```

### Step 5: Run it to see it pass

Run: `npm test -- src/features/schedule/WeekGrid.test.tsx`
Expected: PASS.

### Step 6: Run the full suite and lint

Run: `npm test` and `npm run lint`
Expected: both PASS.

### Step 7: Stop here, changes are ready for review.

---

## Task 8: `TeacherDetail` + `RoomDetail` (dossier pages)

**The one open design judgment call in this plan** (everything else follows
the spec's locked deltas table directly): there is no single-entity endpoint
for a teacher or room (confirmed — `grep -rn "public/teachers\|public/rooms"
apps/api_geteway/config/routes.rb` shows only `teachers.index`/`rooms.index`,
both list actions), so `/teacher/:id` and `/room/:id` need a way to get one
entity's data from an id-only route param. Two options: (a) refetch the
whole list and filter by id, or (b) pass the entity through React Router
navigation state from `EntitySearch`'s card click (Task 5 already sets this
up: `navigate(..., { state: { resource, included } })`). This plan uses
**both, layered**: navigation state is the primary path (zero extra
round-trip for the common click-through case, and it carries the exact
`included` array needed to resolve `post`/`academic_degree`/`academic_title`
or `building`/`room_type`), with a refetch-by-id fallback for direct links,
page refreshes, and shared URLs where `location.state` is empty. The
fallback is reliable in practice because `listPublicTeachers`/
`listPublicRooms` cap `per_page` at 100 server-side (Task 1) — comfortably
covering one institution's teacher/room count — so "fetch up to 100, find by
id" is a real, not approximate, substitute for a single-entity endpoint.

**Files:**
- Create: `src/features/public/pixelArt.tsx` (the two hardcoded pixel patterns + a shared renderer)
- Create: `src/features/public/TeacherDetail.tsx` + `src/features/public/TeacherDetail.test.tsx`
- Create: `src/features/public/RoomDetail.tsx` + `src/features/public/RoomDetail.test.tsx`
- Modify: `src/features/public/PublicApp.tsx` (add `/teacher/:id` and `/room/:id` routes)

**Interfaces:**
- Consumes: `resolveRelationshipAttributes`/`stringAttribute`/`teacherFullName`/`findResourceById` (Task 1), `hasAccessToken` (Task 1), `useBodyClass`/`useFooterVisibility` (Task 2), `PublicOutletContext` (Task 2/5), `OccurrenceWeekGrid`/`WeekSelector` (Task 6), `normalizeOccurrenceWindow` (unchanged), `api.teacherSchedule`/`api.roomOccupancyWindow`/`api.listPublicTeachers`/`api.listPublicRooms` (`src/lib/api.ts`).
- Produces: `renderPixelPattern(pattern: string[])`, `PIXEL_HEAD_PATTERN`, `PIXEL_ROOM_PATTERN` (from `pixelArt.tsx`) — used only by these two pages.

### Step 1: Write the failing tests

Create `src/features/public/pixelArt.test.tsx`:

```tsx
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { renderPixelPattern } from "./pixelArt";

describe("renderPixelPattern", () => {
  it("emits one <i class=\"pixel-N\"> per character, row by row", () => {
    const markup = renderToStaticMarkup(<div>{renderPixelPattern(["01", "10"])}</div>);
    expect(markup).toBe('<div><i class="pixel-0"></i><i class="pixel-1"></i><i class="pixel-1"></i><i class="pixel-0"></i></div>');
  });
});
```

Create `src/features/public/TeacherDetail.test.tsx`:

```tsx
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { TeacherDetail } from "./TeacherDetail";
import type { JsonApiResource } from "../../lib/api";

function renderTeacherDetail(resource: JsonApiResource, included: JsonApiResource[]) {
  return renderToStaticMarkup(
    <MemoryRouter initialEntries={[{ pathname: "/teacher/41", state: { resource, included } }]}>
      <Routes>
        <Route path="/teacher/:id" element={<TeacherDetail />} />
      </Routes>
    </MemoryRouter>,
  );
}

describe("TeacherDetail", () => {
  it("renders real fields from navigation state instead of fetching, and drops the fabricated Кафедра field", () => {
    const resource: JsonApiResource = {
      id: "41", type: "teacher",
      attributes: { last_name: "Иванов", first_name: "Пётр", middle_name: "Ильич" },
      relationships: { post: { data: { id: "3", type: "post" } }, academic_degree: { data: null }, academic_title: { data: null } },
    };
    const included: JsonApiResource[] = [{ id: "3", type: "post", attributes: { name: "доцент", short_name: "доц." } }];
    const markup = renderTeacherDetail(resource, included);
    expect(markup).toContain("Иванов Пётр Ильич");
    expect(markup).toContain("доцент");
    expect(markup).toContain("TCH-041");
    expect(markup).not.toContain("Кафедра");
  });
});
```

Create `src/features/public/RoomDetail.test.tsx`:

```tsx
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter, Outlet, Route, Routes } from "react-router-dom";
import { RoomDetail } from "./RoomDetail";
import type { JsonApiResource } from "../../lib/api";

class MemoryStorage implements Storage {
  private values = new Map<string, string>();
  get length() { return this.values.size; }
  clear() { this.values.clear(); }
  getItem(key: string) { return this.values.get(key) ?? null; }
  key(index: number) { return [...this.values.keys()][index] ?? null; }
  removeItem(key: string) { this.values.delete(key); }
  setItem(key: string, value: string) { this.values.set(key, value); }
}

afterEach(() => { vi.unstubAllGlobals(); });

function renderRoomDetail(entries: Array<string | { pathname: string; state?: unknown }>) {
  return renderToStaticMarkup(
    <MemoryRouter initialEntries={entries}>
      <Routes>
        <Route element={<Outlet context={{ openGuide: () => undefined, openAuth: () => undefined }} />}>
          <Route path="/room/:id" element={<RoomDetail />} />
        </Route>
      </Routes>
    </MemoryRouter>,
  );
}

describe("RoomDetail", () => {
  it("gates the room record behind login when unauthenticated", () => {
    const markup = renderRoomDetail(["/room/14"]);
    expect(markup).toContain("Войдите, чтобы смотреть карточку аудитории");
  });

  it("shows real room fields from navigation state once authenticated, dropping the fabricated Вместимость/Оснащение fields", () => {
    const storage = new MemoryStorage();
    storage.setItem("milk_schedule_access_token", "token-value");
    vi.stubGlobal("sessionStorage", storage);
    const resource: JsonApiResource = {
      id: "14", type: "room", attributes: { number: "2-312" },
      relationships: { building: { data: { id: "2", type: "building" } }, room_type: { data: { id: "3", type: "room_type" } } },
    };
    const included: JsonApiResource[] = [{ id: "2", type: "building", attributes: { number: "2" } }, { id: "3", type: "room_type", attributes: { name: "Лекционная аудитория" } }];
    const markup = renderRoomDetail([{ pathname: "/room/14", state: { resource, included } }]);
    expect(markup).not.toContain("Войдите, чтобы смотреть карточку аудитории");
    expect(markup).toContain("2-312");
    expect(markup).toContain("Лекционная аудитория");
    expect(markup).not.toContain("Вместимость");
    expect(markup).not.toContain("Оснащение");
  });
});
```

### Step 2: Run them to see them fail

Run: `npm test -- src/features/public/pixelArt.test.tsx src/features/public/TeacherDetail.test.tsx src/features/public/RoomDetail.test.tsx`
Expected: FAIL — none of the three modules exist yet.

### Step 3: Implement `pixelArt.tsx`

The two patterns are copied verbatim from `schedule_home/teacher-detail.js` lines 150-173 (`pixelPattern`) and `schedule_home/room-detail.js` lines 38-45 (`planPattern`) — static, non-data-driven decoration per the design spec:

```tsx
export const PIXEL_HEAD_PATTERN = [
  "00000000111100000000",
  "00000011111111000000",
  "00000111111111100000",
  "00001111111111110000",
  "00011122222221111000",
  "00111222222222111100",
  "00112221111222211100",
  "00112210000122211100",
  "00112211001122211100",
  "00112222222222211100",
  "00112222112222211100",
  "00111222222222111100",
  "00011122222221111000",
  "00001112222111110000",
  "00000111221111100000",
  "00000011111111000000",
  "00000001111110000000",
  "00000001111110000000",
  "00000111111111100000",
  "00011111111111111000",
  "00111111111111111100",
  "01111111111111111110",
];

export const PIXEL_ROOM_PATTERN = [
  "00000000000000000000", "00111111111111111100", "00100000000000000100", "00102220222022200100",
  "00100000000000000100", "00102220222022200100", "00100000000000000100", "00102220222022200100",
  "00100000000000000100", "00100000000000000100", "00122222222222222100", "00120000000000002100",
  "00120000000000002100", "00120001111000002100", "00120001111000002100", "00120000000000002100",
  "00122222222222222100", "00100000000000000100", "00111111111111111100", "00000000000000000000",
  "00000001111110000000", "00000001111110000000",
];

export function renderPixelPattern(pattern: string[]) {
  return pattern.flatMap((row, rowIndex) => [...row].map((cell, cellIndex) => <i className={`pixel-${cell}`} key={`${rowIndex}-${cellIndex}`} />));
}
```

### Step 4: Implement `TeacherDetail.tsx`

DOM ported from `schedule_home/teacher-detail.html` lines 26-62, dropping the mockup's fabricated `subjects`/`groups` bio and its `Кафедра` field (no backing data — `Portal::Serializers::TeacherSerializer` has no kafedra relationship). The schedule section reuses `WeekSelector` + `OccurrenceWeekGrid` from Task 6 instead of re-porting `teacher-detail.js`'s own table/mobile-agenda builder a second time (identical markup shape to `schedule.html`'s, so the same components apply):

```tsx
import { useEffect, useState } from "react";
import { useLocation, useParams } from "react-router-dom";
import { api, type JsonApiResource, type OccurrenceWindow } from "../../lib/api";
import { findResourceById, resolveRelationshipAttributes, stringAttribute, teacherFullName } from "./entityFields";
import { normalizeOccurrenceWindow } from "../schedule/normalize";
import { OccurrenceWeekGrid } from "../schedule/WeekGrid";
import { WeekSelector } from "./WeekSelector";
import { useBodyClass, useFooterVisibility } from "./bodyClass";
import { PIXEL_HEAD_PATTERN, renderPixelPattern } from "./pixelArt";

type LocationState = { resource?: JsonApiResource; included?: JsonApiResource[] } | null;

export function TeacherDetail() {
  useBodyClass("teacher-record-page");
  useFooterVisibility();
  const { id = "" } = useParams();
  const location = useLocation();
  const state = location.state as LocationState;

  const [resource, setResource] = useState<JsonApiResource | undefined>(state?.resource);
  const [included, setIncluded] = useState<JsonApiResource[] | undefined>(state?.included);
  const [loadingEntity, setLoadingEntity] = useState(!state?.resource);

  useEffect(() => {
    if (state?.resource) return; // Came from EntitySearch's card click — no refetch needed.
    let active = true;
    api.listPublicTeachers("", 100).then((response) => {
      if (!active) return;
      setResource(findResourceById(response, id));
      setIncluded(response.included);
    }).finally(() => active && setLoadingEntity(false));
    return () => { active = false; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [id]);

  const [result, setResult] = useState<OccurrenceWindow | null>(null);
  const [scheduleLoading, setScheduleLoading] = useState(true);
  const [activeWeekIndex, setActiveWeekIndex] = useState(0);

  useEffect(() => {
    if (!id) return;
    let active = true;
    setScheduleLoading(true);
    api.teacherSchedule(id).then((response) => { if (active) setResult(response); }).finally(() => active && setScheduleLoading(false));
    return () => { active = false; };
  }, [id]);

  const schedule = normalizeOccurrenceWindow(result);
  useEffect(() => {
    const currentIndex = schedule.weeks.findIndex((week) => week.is_current);
    setActiveWeekIndex(currentIndex >= 0 ? currentIndex : 0);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [result]);

  const name = teacherFullName(resource?.attributes);
  const fallbackResource: JsonApiResource = resource ?? { id, type: "teacher" };
  const post = resolveRelationshipAttributes(fallbackResource, included, "post");
  const academicDegree = resolveRelationshipAttributes(fallbackResource, included, "academic_degree");
  const academicTitle = resolveRelationshipAttributes(fallbackResource, included, "academic_title");
  const code = `TCH-${String(id).padStart(3, "0")}`;

  return (
    <main className="teacher-record-main">
      <section className="teacher-record-sheet" aria-label="Карточка преподавателя">
        <div className="teacher-record-copy">
          <div className="teacher-record-number">{code}</div>
          <div className="teacher-record-name">{loadingEntity ? "Загружаем…" : (name || "—")}</div>
          <div className="teacher-record-fields">
            {[
              ["ФИО", name || "—"],
              ["Учёная степень", stringAttribute(academicDegree, "name", "—")],
              ["Учёное звание", stringAttribute(academicTitle, "name", "—")],
              ["Должность", stringAttribute(post, "name", "—")],
            ].map(([label, value]) => (
              <div className="teacher-record-field" key={label}><span>{label}</span><strong>{value}</strong></div>
            ))}
          </div>
        </div>

        <div className="teacher-record-portrait" aria-hidden="true">
          <div className="teacher-record-fileline"><span>FILE</span><strong>{code}</strong></div>
          <div className="teacher-pixel-stage">
            <div className="teacher-height-lines"></div>
            <div className="teacher-pixel-head">{renderPixelPattern(PIXEL_HEAD_PATTERN)}</div>
          </div>
        </div>
      </section>

      <section className="teacher-schedule-section" aria-label="Расписание преподавателя">
        <div className="teacher-schedule-toolbar">
          <div className="teacher-schedule-person">{name || "—"}</div>
          <WeekSelector weeks={schedule.weeks} activeIndex={activeWeekIndex} onSelect={setActiveWeekIndex} />
        </div>

        <OccurrenceWeekGrid week={schedule.weeks[activeWeekIndex] ?? null} loading={scheduleLoading} />
      </section>
    </main>
  );
}
```

### Step 5: Implement `RoomDetail.tsx`

DOM ported from `schedule_home/room-detail.html` lines 26-62, dropping the mockup's fabricated `capacity`/`equipment` fields (no backing data — `Portal::Serializers::RoomSerializer` exposes only `name`/`number` plus the `building`/`room_type` relationships). Gated behind login like every other room surface:

```tsx
import { useEffect, useState } from "react";
import { useLocation, useOutletContext, useParams } from "react-router-dom";
import { Lock } from "lucide-react";
import { api, type JsonApiResource, type OccurrenceWindow } from "../../lib/api";
import { hasAccessToken } from "../../lib/session";
import { findResourceById, resolveRelationshipAttributes, stringAttribute } from "./entityFields";
import { normalizeOccurrenceWindow } from "../schedule/normalize";
import { OccurrenceWeekGrid } from "../schedule/WeekGrid";
import { WeekSelector } from "./WeekSelector";
import { useBodyClass, useFooterVisibility } from "./bodyClass";
import { PIXEL_ROOM_PATTERN, renderPixelPattern } from "./pixelArt";
import type { PublicOutletContext } from "./PublicLayout";

type LocationState = { resource?: JsonApiResource; included?: JsonApiResource[] } | null;

export function RoomDetail() {
  useBodyClass("teacher-record-page");
  useBodyClass("room-record-page");
  useFooterVisibility();
  const { id = "" } = useParams();
  const location = useLocation();
  const state = location.state as LocationState;
  const { openAuth } = useOutletContext<PublicOutletContext>();
  const isAuthenticated = hasAccessToken();

  const [resource, setResource] = useState<JsonApiResource | undefined>(state?.resource);
  const [included, setIncluded] = useState<JsonApiResource[] | undefined>(state?.included);
  const [loadingEntity, setLoadingEntity] = useState(!state?.resource);

  useEffect(() => {
    if (!isAuthenticated || state?.resource) return;
    let active = true;
    api.listPublicRooms(100).then((response) => {
      if (!active) return;
      setResource(findResourceById(response, id));
      setIncluded(response.included);
    }).finally(() => active && setLoadingEntity(false));
    return () => { active = false; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [id, isAuthenticated]);

  const [result, setResult] = useState<OccurrenceWindow | null>(null);
  const [scheduleLoading, setScheduleLoading] = useState(true);
  const [activeWeekIndex, setActiveWeekIndex] = useState(0);

  useEffect(() => {
    if (!id || !isAuthenticated) return;
    let active = true;
    setScheduleLoading(true);
    api.roomOccupancyWindow(id).then((response) => { if (active) setResult(response); }).finally(() => active && setScheduleLoading(false));
    return () => { active = false; };
  }, [id, isAuthenticated]);

  const schedule = normalizeOccurrenceWindow(result);
  useEffect(() => {
    const currentIndex = schedule.weeks.findIndex((week) => week.is_current);
    setActiveWeekIndex(currentIndex >= 0 ? currentIndex : 0);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [result]);

  if (!isAuthenticated) {
    return (
      <main className="teacher-record-main">
        <div className="entity-search-empty" role="status">
          <Lock size={28} aria-hidden="true" />
          <p>Войдите, чтобы смотреть карточку аудитории.</p>
          <button className="entity-page-arrow" type="button" onClick={openAuth}>Войти</button>
        </div>
      </main>
    );
  }

  const number = stringAttribute(resource?.attributes, "number", "—");
  const fallbackResource: JsonApiResource = resource ?? { id, type: "room" };
  const building = resolveRelationshipAttributes(fallbackResource, included, "building");
  const roomType = resolveRelationshipAttributes(fallbackResource, included, "room_type");
  const code = `ROOM-${String(id).padStart(3, "0")}`;

  return (
    <main className="teacher-record-main">
      <section className="teacher-record-sheet" aria-label="Карточка аудитории">
        <div className="teacher-record-copy">
          <div className="teacher-record-number">{code}</div>
          <div className="teacher-record-name">{loadingEntity ? "Загружаем…" : number}</div>
          <div className="teacher-record-fields">
            {[
              ["Аудитория", number],
              ["Тип", stringAttribute(roomType, "name", "—")],
              ["Корпус", stringAttribute(building, "number", "—")],
            ].map(([label, value]) => (
              <div className="teacher-record-field" key={label}><span>{label}</span><strong>{value}</strong></div>
            ))}
          </div>
        </div>

        <div className="teacher-record-portrait" aria-hidden="true">
          <div className="teacher-record-fileline"><span>FILE</span><strong>{code}</strong></div>
          <div className="teacher-pixel-stage">
            <div className="teacher-height-lines"></div>
            <div className="teacher-pixel-head">{renderPixelPattern(PIXEL_ROOM_PATTERN)}</div>
          </div>
        </div>
      </section>

      <section className="teacher-schedule-section" aria-label="Расписание аудитории">
        <div className="teacher-schedule-toolbar">
          <div className="teacher-schedule-person">{number} · {stringAttribute(roomType, "name", "—")}</div>
          <WeekSelector weeks={schedule.weeks} activeIndex={activeWeekIndex} onSelect={setActiveWeekIndex} />
        </div>

        <OccurrenceWeekGrid week={schedule.weeks[activeWeekIndex] ?? null} loading={scheduleLoading} />
      </section>
    </main>
  );
}
```

### Step 6: Run them to see them pass

Run: `npm test -- src/features/public/pixelArt.test.tsx src/features/public/TeacherDetail.test.tsx src/features/public/RoomDetail.test.tsx`
Expected: PASS.

### Step 7: Wire the routes

In `src/features/public/PublicApp.tsx`, add:

```tsx
import { TeacherDetail } from "./TeacherDetail";
import { RoomDetail } from "./RoomDetail";
// ...
          <Route path="/teacher/:id" element={<TeacherDetail />} />
          <Route path="/room/:id" element={<RoomDetail />} />
```

### Step 8: Run the full suite and lint

Run: `npm test` and `npm run lint`
Expected: both PASS.

### Step 9: Stop here, changes are ready for review.

---

## Task 9: `Guide` (`guide.html`/`guide.js` port)

Fully static — no API calls, no data wiring. This is the heaviest single
mockup script (custom pointer-drag page-flip physics), ported faithfully
rather than simplified. Split into three files: the six page contents
(declarative JSX, replacing the mockup's `<template>` elements), the physics
engine (an imperative DOM/pointer-event module operating on refs, since a
physics simulation driving raw CSS custom properties every animation frame
does not map onto React's render cycle — the same reason `PulseLine.tsx`
already drives its animation imperatively via a ref instead of React state),
and the component that wires them together. The dialog-handling block at the
end of `guide.js` (lines 397-476) is dropped entirely — `PublicLayout`
(Task 2) already owns the Auth/Contacts/Cookie dialogs shared by every
public page.

**Files:**
- Create: `src/features/public/guidePages.tsx` (the six `<template>` contents, ported to JSX components)
- Create: `src/features/public/guideBookEngine.ts` (the page-flip physics)
- Create: `src/features/public/Guide.tsx` + `src/features/public/Guide.test.tsx`
- Modify: `src/features/public/PublicApp.tsx` (add the `/guide` route)

**Interfaces:**
- Produces: `GUIDE_PAGES: Array<() => JSX.Element>` (from `guidePages.tsx`).
- Produces: `attachBookEngine(root: HTMLElement): () => void` (from `guideBookEngine.ts`) — attaches every pointer/keyboard listener scoped under `root` and returns a cleanup function; `Guide.tsx` calls it from a `useEffect`.
- Produces: `<Guide />`, mounted at `/guide`.

### Step 1: Write the failing test

Create `src/features/public/Guide.test.tsx`:

```tsx
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { Guide } from "./Guide";

describe("Guide", () => {
  it("renders all six pages across three sheets, plus the progress markers", () => {
    const markup = renderToStaticMarkup(<Guide />);
    expect(markup).toContain("Как найти");
    expect(markup).toContain("Факультет");
    expect(markup).toContain("Изменения");
    expect((markup.match(/book-sheet-face/g) ?? []).length).toBe(6);
  });
});
```

### Step 2: Run it to see it fail

Run: `npm test -- src/features/public/Guide.test.tsx`
Expected: FAIL — cannot find module `./Guide`.

### Step 3: Implement `guidePages.tsx`

Ported verbatim from `schedule_home/guide.html`'s six `<template>` blocks (lines 60-165):

```tsx
export function GuidePage0() {
  return (
    <article className="leaflet-page leaflet-page--intro">
      <header className="leaflet-page-head"><span>01</span><span>Расписание</span></header>
      <h1>Как найти<br />расписание</h1>
      <div className="leaflet-menu">
        <div><span className="menu-mark">01</span><b>Группа</b><i>→</i></div>
        <div><span className="menu-mark">02</span><b>Преподаватель</b><i>→</i></div>
        <div><span className="menu-mark">03</span><b>Аудитория</b><i>→</i></div>
      </div>
      <div className="leaflet-flow" aria-hidden="true">
        <div className="flow-box"><span></span><span></span><span></span></div><i>→</i>
        <div className="flow-card"><span></span><span></span><span></span></div><i>→</i>
        <div className="flow-grid"><span></span><span></span><span></span><span></span><span></span><span></span></div>
      </div>
    </article>
  );
}

export function GuidePage1() {
  return (
    <article className="leaflet-page">
      <header className="leaflet-page-head"><span>02</span><span>Группа</span></header>
      <h1>Факультет<br />Курс<br />Группа</h1>
      <div className="leaflet-selector-row">
        <div><small>Курс</small><b>1&nbsp;&nbsp;<em>2</em>&nbsp;&nbsp;3&nbsp;&nbsp;4&nbsp;&nbsp;5</b></div>
        <div><small>Подгруппа</small><b>Все&nbsp;&nbsp;/&nbsp;&nbsp;1&nbsp;&nbsp;/&nbsp;&nbsp;2</b></div>
        <div><small>Неделя</small><b><em>Чётная</em>&nbsp;&nbsp;/&nbsp;&nbsp;Нечётная</b></div>
      </div>
      <div className="leaflet-faculty-stack" aria-hidden="true">
        <div className="faculty-mini is-selected"><b>Факультет</b><span>Выбрать</span><i>→</i></div>
        <div className="faculty-mini"><b>Факультет</b><span>Выбрать</span><i>→</i></div>
        <div className="faculty-mini"><b>Факультет</b><span>Выбрать</span><i>→</i></div>
      </div>
      <p className="leaflet-caption">Выберите факультет, курс и группу</p>
    </article>
  );
}

export function GuidePage2() {
  return (
    <article className="leaflet-page">
      <header className="leaflet-page-head"><span>03</span><span>Преподаватель</span></header>
      <h1>Поиск<br />по ФИО</h1>
      <div className="leaflet-search-field"><span>Иванов Илья Сергеевич</span><i>⌕</i></div>
      <div className="leaflet-result-card">
        <div><small>ФИО</small><b>Иванов Илья Сергеевич</b></div>
        <div><small>Кафедра</small><b>Кафедра информатики</b></div>
      </div>
      <div className="leaflet-mini-schedule" aria-hidden="true">
        <div className="ms-head"><span>Пн</span><span>Вт</span><span>Ср</span><span>Чт</span><span>Пт</span></div>
        <div className="ms-grid">
          <i></i><i className="on">ПИ-21</i><i></i><i className="on">ИВТ-31</i><i></i>
          <i className="on">ИВТ-31</i><i></i><i className="on">ПИ-21</i><i></i><i className="on">ЭС-22</i>
        </div>
      </div>
    </article>
  );
}

export function GuidePage3() {
  return (
    <article className="leaflet-page">
      <header className="leaflet-page-head"><span>04</span><span>Аудитория</span></header>
      <h1>Поиск<br />по номеру</h1>
      <div className="leaflet-search-field"><span>203</span><i>⌕</i></div>
      <div className="leaflet-room-card">
        <div><small>Аудитория</small><b>203</b></div>
        <div><small>Тип</small><b>Лекционная</b></div>
        <div><small>Корпус</small><b>2</b></div>
      </div>
      <div className="room-plan" aria-hidden="true">
        <span className="board"></span>
        <span className="desk d1"></span><span className="desk d2"></span><span className="desk d3"></span><span className="desk d4"></span>
        <span className="desk d5"></span><span className="desk d6"></span><span className="desk d7"></span><span className="desk d8"></span>
        <span className="door"></span>
      </div>
    </article>
  );
}

export function GuidePage4() {
  return (
    <article className="leaflet-page leaflet-page--schedule">
      <header className="leaflet-page-head"><span>05</span><span>Расписание</span></header>
      <h1>Неделя</h1>
      <div className="leaflet-week-controls">
        <span className="is-active">Чётная</span><span>Нечётная</span><span>Все</span><span>1</span><span>2</span>
      </div>
      <div className="leaflet-table">
        <div className="lt-head"><span></span><span>Пн</span><span>Вт</span><span>Ср</span><span>Чт</span><span>Пт</span></div>
        <div className="lt-row"><b>08:30</b><i></i><i className="event"><strong>ПИ</strong><small>203 · Лаб.</small></i><i></i><i className="event"><strong>БД</strong><small>305 · Лек.</small></i><i></i></div>
        <div className="lt-row"><b>10:15</b><i className="event"><strong>ОС</strong><small>101 · Лек.</small></i><i></i><i className="event"><strong>ПИ</strong><small>203 · Лаб.</small></i><i></i><i className="event"><strong>БД</strong><small>305 · Пр.</small></i></div>
        <div className="lt-row"><b>12:00</b><i></i><i className="event"><strong>Сети</strong><small>214 · Лаб.</small></i><i></i><i className="event"><strong>ОС</strong><small>101 · Пр.</small></i><i></i></div>
      </div>
      <p className="leaflet-caption">Нажмите на занятие — откроются подробности</p>
    </article>
  );
}

export function GuidePage5() {
  return (
    <article className="leaflet-page leaflet-page--changes">
      <header className="leaflet-page-head"><span>06</span><span>Переносы</span></header>
      <h1>Изменения<br />в расписании</h1>
      <div className="change-legend"><span className="added"></span><b>Добавлено</b><span className="removed"></span><b>Отменено</b></div>
      <div className="change-list">
        <div className="change-item is-added"><time>10:15</time><b>Программирование</b><span>203</span><small>добавлено</small></div>
        <div className="change-item is-removed"><time>12:00</time><b>Базы данных</b><span>305</span><small>отменено</small></div>
      </div>
      <div className="detail-links">
        <div><small>Преподаватель</small><b>Иванов И.С.</b><i>→</i></div>
        <div><small>Аудитория</small><b>203</b><i>→</i></div>
      </div>
      <p className="leaflet-caption">Из подробностей можно перейти к преподавателю или аудитории</p>
    </article>
  );
}

export const GUIDE_PAGES = [GuidePage0, GuidePage1, GuidePage2, GuidePage3, GuidePage4, GuidePage5];
```

### Step 4: Implement `guideBookEngine.ts`

Ported from `schedule_home/guide.js` lines 1-395 (the physics: sheet-fold
math, drag/pointer handling, keyboard nav, stacking z-index). Because React
already renders the sheets (Step 3) instead of `guide.js`'s own
`makeSheet`/`cloneTemplate`/`renderProgress` DOM-creation functions, this
port **drops those three** and instead queries the already-rendered
`.book-sheet` elements under `root`:

```ts
const PAGE_COUNT = 6;
const SHEET_COUNT = Math.ceil(PAGE_COUNT / 2);

const clamp01 = (value: number) => Math.max(0, Math.min(1, value));
const easeInOutCubic = (t: number) => (t < 0.5 ? 4 * t * t * t : 1 - (-2 * t + 2) ** 3 / 2);
const easeOutQuart = (t: number) => 1 - (1 - t) ** 4;

type SheetState = { progress: number; animationFrame: number; animationToken: number };

type DragState = {
  direction: "forward" | "backward"; sheetIndex: number; rect: DOMRect; x: number; center: number;
  sheet: HTMLElement; pointerId: number; startX: number; lastX: number; lastTime: number;
  velocity: number; progress: number; pendingProgress: number; moved: number;
};

/**
 * Ports schedule_home/guide.js's page-flip physics (pointer-drag with
 * velocity/commit thresholds, CSS-custom-property-driven fold rendering,
 * keyboard + dblclick navigation) onto React-rendered sheets, scoped to
 * `root` instead of the global `document`. The mockup's dialog-handling
 * block (lines 397-476) is intentionally dropped — see Task 2's PublicLayout.
 */
export function attachBookEngine(root: HTMLElement): () => void {
  const book = root.querySelector<HTMLElement>("#guideBook");
  const pageIndexEl = root.querySelector<HTMLElement>("#bookPageIndex");
  const progressRoot = root.querySelector<HTMLElement>("#bookProgress");
  const sheets = [...root.querySelectorAll<HTMLElement>(".book-sheet")];
  const sheetState: SheetState[] = sheets.map(() => ({ progress: 0, animationFrame: 0, animationToken: 0 }));

  if (!book || !pageIndexEl || !progressRoot || sheets.length !== SHEET_COUNT) return () => undefined;

  let spread = 0;
  let active: DragState | null = null;
  let locked = false;
  let dragFrame = 0;

  function visiblePages(): number[] {
    if (spread === 0) return [0];
    if (spread === SHEET_COUNT) return [PAGE_COUNT - 1];
    return [spread * 2 - 1, spread * 2];
  }

  function updateMeta() {
    const visible = visiblePages();
    pageIndexEl!.textContent = visible.length === 1
      ? String(visible[0] + 1).padStart(2, "0")
      : `${String(visible[0] + 1).padStart(2, "0")}—${String(visible[1] + 1).padStart(2, "0")}`;

    const visibleSet = new Set(visible);
    [...progressRoot!.querySelectorAll<HTMLElement>("i")].forEach((mark, index) => {
      mark.classList.toggle("is-active", visibleSet.has(index));
    });

    sheets.forEach((sheet) => {
      const front = sheet.querySelector<HTMLElement>(".book-sheet-front");
      const back = sheet.querySelector<HTMLElement>(".book-sheet-back");
      front?.setAttribute("aria-hidden", visibleSet.has(Number(front.dataset.page)) ? "false" : "true");
      back?.setAttribute("aria-hidden", visibleSet.has(Number(back.dataset.page)) ? "false" : "true");
    });
  }

  function applyBookFold(progress = 0) {
    const p = clamp01(progress);
    const arc = Math.sin(Math.PI * p);
    book!.style.setProperty("--book-fold", arc.toFixed(4));
    book!.style.setProperty("--book-fold-side", p < 0.5 ? "1" : "-1");
    book!.style.setProperty("--spine-width", `${(18 + arc * 10).toFixed(2)}px`);
    book!.style.setProperty("--spine-z", `${(5 + arc * 3).toFixed(2)}px`);
    book!.style.setProperty("--spine-blur", `${(arc * 0.35).toFixed(2)}px`);
  }

  function applySheetVisual(sheetIndex: number, progress: number) {
    const sheet = sheets[sheetIndex];
    if (!sheet) return;
    const p = clamp01(progress);
    const arc = Math.sin(Math.PI * p);
    const phase = Math.sin(Math.PI * 2 * p);
    const edgeCurl = Math.sin(Math.PI * Math.min(1, p * 1.35)) * Math.sin(Math.PI * Math.min(1, (1 - p) * 1.35));
    const angle = -180 * p;
    const lift = arc * 24;
    const skew = phase * 1.15;
    const tilt = phase * 0.42;
    const scaleX = 1 - arc * 0.018;
    const shiftY = -arc * 2.4;
    const shade = arc ** 0.82;
    const foldX = 100 - p * 100;
    const contentShift = phase * 1.7;
    const shadowX = -6 - arc * 22;
    const shadowY = 7 + arc * 6;
    const shadowBlur = 9 + arc * 18;

    sheetState[sheetIndex].progress = p;
    sheet.style.setProperty("--angle", `${angle.toFixed(3)}deg`);
    sheet.style.setProperty("--lift", `${lift.toFixed(2)}px`);
    sheet.style.setProperty("--skew", `${skew.toFixed(3)}deg`);
    sheet.style.setProperty("--tilt", `${tilt.toFixed(3)}deg`);
    sheet.style.setProperty("--scale-x", scaleX.toFixed(5));
    sheet.style.setProperty("--shift-y", `${shiftY.toFixed(2)}px`);
    sheet.style.setProperty("--shade", shade.toFixed(4));
    sheet.style.setProperty("--curl", edgeCurl.toFixed(4));
    sheet.style.setProperty("--fold-x", `${foldX.toFixed(2)}%`);
    sheet.style.setProperty("--content-shift", `${contentShift.toFixed(2)}px`);
    sheet.style.setProperty("--content-shift-back", `${(-contentShift).toFixed(2)}px`);
    sheet.style.setProperty("--content-scale", (1 - arc * 0.008).toFixed(5));
    sheet.style.setProperty("--face-overlay-opacity", (0.1 + shade * 0.72).toFixed(4));
    sheet.style.setProperty("--edge-overlay-opacity", (0.08 + shade * 0.84).toFixed(4));
    sheet.style.setProperty("--corner-radius", `${(edgeCurl * 13).toFixed(2)}px`);
    sheet.style.setProperty("--corner-radius-small", `${(edgeCurl * 10).toFixed(2)}px`);
    sheet.style.setProperty("--edge-width", `${(2 + edgeCurl * 4).toFixed(2)}px`);
    sheet.style.setProperty("--edge-blur", `${(edgeCurl * 8).toFixed(2)}px`);
    sheet.style.setProperty("--cast-shift", `${(-edgeCurl * 8).toFixed(2)}px`);
    sheet.style.setProperty("--cast-blur", `${(5 + shade * 13).toFixed(2)}px`);
    sheet.style.setProperty("--cast-opacity", (shade * 0.55).toFixed(4));
    sheet.style.setProperty("--shadow-x", `${shadowX.toFixed(2)}px`);
    sheet.style.setProperty("--shadow-y", `${shadowY.toFixed(2)}px`);
    sheet.style.setProperty("--shadow-blur", `${shadowBlur.toFixed(2)}px`);
    applyBookFold(p);
  }

  function cancelSheetAnimation(sheetIndex: number) {
    const state = sheetState[sheetIndex];
    if (!state) return;
    state.animationToken += 1;
    if (state.animationFrame) cancelAnimationFrame(state.animationFrame);
    state.animationFrame = 0;
  }

  function animateSheet(sheetIndex: number, target: number, options: { duration?: number; easing?: (t: number) => number } = {}): Promise<boolean> {
    const state = sheetState[sheetIndex];
    const sheet = sheets[sheetIndex];
    if (!state || !sheet) return Promise.resolve(false);

    cancelSheetAnimation(sheetIndex);
    const token = state.animationToken;
    const from = state.progress;
    const distance = Math.abs(target - from);
    const duration = options.duration ?? Math.max(280, 630 * distance + 160);
    const easing = options.easing ?? easeInOutCubic;
    const start = performance.now();

    sheet.classList.remove("is-dragging");
    sheet.classList.add("is-settling");
    book!.classList.add("is-turning");

    return new Promise((resolve) => {
      const tick = (now: number) => {
        if (state.animationToken !== token) { resolve(false); return; }
        const t = clamp01((now - start) / duration);
        applySheetVisual(sheetIndex, from + (target - from) * easing(t));
        if (t < 1) { state.animationFrame = requestAnimationFrame(tick); return; }
        state.animationFrame = 0;
        applySheetVisual(sheetIndex, target);
        sheet.classList.remove("is-settling");
        book!.classList.remove("is-turning");
        applyBookFold(0);
        resolve(true);
      };
      state.animationFrame = requestAnimationFrame(tick);
    });
  }

  function updateStacking(activeIndex = -1) {
    sheets.forEach((sheet, index) => {
      if (index === activeIndex) { sheet.style.zIndex = "60"; return; }
      sheet.style.zIndex = index < spread ? String(18 + index) : String(45 - index);
    });
  }

  async function turnForward() {
    if (locked || active || spread >= SHEET_COUNT) return;
    locked = true;
    const sheetIndex = spread;
    updateStacking(sheetIndex);
    await animateSheet(sheetIndex, 1, { duration: 720, easing: easeOutQuart });
    spread += 1;
    updateStacking();
    updateMeta();
    locked = false;
  }

  async function turnBackward() {
    if (locked || active || spread <= 0) return;
    locked = true;
    const sheetIndex = spread - 1;
    updateStacking(sheetIndex);
    await animateSheet(sheetIndex, 0, { duration: 720, easing: easeOutQuart });
    spread -= 1;
    updateStacking();
    updateMeta();
    locked = false;
  }

  function pointerMode(event: PointerEvent) {
    if (!book || locked) return null;
    const rect = book.getBoundingClientRect();
    const x = event.clientX - rect.left;
    const center = rect.width / 2;
    if (x >= center && spread < SHEET_COUNT) return { direction: "forward" as const, sheetIndex: spread, rect, x, center };
    if (x < center && spread > 0) return { direction: "backward" as const, sheetIndex: spread - 1, rect, x, center };
    return null;
  }

  function beginDrag(event: PointerEvent) {
    if ((event.pointerType === "mouse" && event.button !== 0) || locked || active) return;
    const mode = pointerMode(event);
    if (!mode) return;

    cancelSheetAnimation(mode.sheetIndex);
    const sheet = sheets[mode.sheetIndex];
    const initialProgress = mode.direction === "forward" ? 0 : 1;
    applySheetVisual(mode.sheetIndex, initialProgress);

    active = { ...mode, sheet, pointerId: event.pointerId, startX: event.clientX, lastX: event.clientX, lastTime: performance.now(), velocity: 0, progress: initialProgress, pendingProgress: initialProgress, moved: 0 };

    updateStacking(mode.sheetIndex);
    sheet.classList.add("is-dragging");
    book!.classList.add("is-dragging");
    document.body.classList.add("book-dragging");
    book!.setPointerCapture?.(event.pointerId);
    event.preventDefault();
  }

  function flushDragFrame() {
    dragFrame = 0;
    if (!active) return;
    active.progress = active.pendingProgress;
    applySheetVisual(active.sheetIndex, active.progress);
  }

  function moveDrag(event: PointerEvent) {
    if (!active || event.pointerId !== active.pointerId) return;
    const samples = event.getCoalescedEvents?.() ?? [event];
    const sample = samples[samples.length - 1] ?? event;
    const now = performance.now();
    const dt = Math.max(1, now - active.lastTime);
    active.velocity = active.velocity * 0.68 + ((sample.clientX - active.lastX) / dt) * 0.32;
    active.lastX = sample.clientX;
    active.lastTime = now;

    const dx = sample.clientX - active.startX;
    active.moved = Math.max(active.moved, Math.abs(dx));

    if (active.direction === "forward") {
      const travel = Math.max(120, active.startX - (active.rect.left + active.center));
      active.pendingProgress = clamp01(-dx / travel);
    } else {
      const travel = Math.max(120, active.rect.left + active.center - active.startX);
      active.pendingProgress = clamp01(1 - dx / travel);
    }

    if (!dragFrame) dragFrame = requestAnimationFrame(flushDragFrame);
    event.preventDefault();
  }

  async function finishDrag(event: PointerEvent | null, cancelled = false) {
    if (!active || (event && event.pointerId !== active.pointerId)) return;

    if (dragFrame) {
      cancelAnimationFrame(dragFrame);
      dragFrame = 0;
      active.progress = active.pendingProgress;
      applySheetVisual(active.sheetIndex, active.progress);
    }

    const state = active;
    active = null;
    book!.classList.remove("is-dragging");
    document.body.classList.remove("book-dragging");
    try { book!.releasePointerCapture?.(state.pointerId); } catch { /* already released */ }

    const rect = state.rect;
    const clickForward = state.direction === "forward" && state.moved < 7 && state.x > rect.width * 0.88;
    const clickBackward = state.direction === "backward" && state.moved < 7 && state.x < rect.width * 0.12;

    let commit = false;
    if (!cancelled) {
      commit = state.direction === "forward"
        ? state.progress > 0.36 || state.velocity < -0.34 || clickForward
        : state.progress < 0.64 || state.velocity > 0.34 || clickBackward;
    }

    const target = state.direction === "forward" ? (commit ? 1 : 0) : (commit ? 0 : 1);

    if (commit) {
      spread += state.direction === "forward" ? 1 : -1;
      updateMeta();
    }

    locked = true;
    updateStacking(state.sheetIndex);
    await animateSheet(state.sheetIndex, target, { duration: Math.max(260, 610 * Math.abs(target - state.progress) + 120), easing: easeOutQuart });
    updateStacking();
    locked = false;
  }

  updateStacking();
  updateMeta();

  const onPointerDown = (event: PointerEvent) => beginDrag(event);
  const onPointerMove = (event: PointerEvent) => moveDrag(event);
  const onPointerUp = (event: PointerEvent) => { void finishDrag(event, false); };
  const onPointerCancel = (event: PointerEvent) => { void finishDrag(event, true); };
  const onDblClick = (event: MouseEvent) => {
    if (locked || active || !book) return;
    const rect = book.getBoundingClientRect();
    const x = event.clientX - rect.left;
    if (x >= rect.width / 2) void turnForward(); else void turnBackward();
  };
  const onKeyDown = (event: KeyboardEvent) => {
    if (event.key === "ArrowRight" || event.key === "PageDown") { event.preventDefault(); void turnForward(); }
    else if (event.key === "ArrowLeft" || event.key === "PageUp") { event.preventDefault(); void turnBackward(); }
  };

  book.addEventListener("pointerdown", onPointerDown);
  book.addEventListener("pointermove", onPointerMove);
  book.addEventListener("pointerup", onPointerUp);
  book.addEventListener("pointercancel", onPointerCancel);
  book.addEventListener("dblclick", onDblClick);
  window.addEventListener("keydown", onKeyDown);

  return () => {
    book.removeEventListener("pointerdown", onPointerDown);
    book.removeEventListener("pointermove", onPointerMove);
    book.removeEventListener("pointerup", onPointerUp);
    book.removeEventListener("pointercancel", onPointerCancel);
    book.removeEventListener("dblclick", onDblClick);
    window.removeEventListener("keydown", onKeyDown);
    sheetState.forEach((_, index) => cancelSheetAnimation(index));
  };
}
```

### Step 5: Implement `Guide.tsx`

DOM ported from `schedule_home/guide.html` lines 26-52 (`.book-guide`/`.book-viewport`/`.book-scene`/`.book`/`.book-meta`):

```tsx
import { useEffect, useRef } from "react";
import { GUIDE_PAGES } from "./guidePages";
import { attachBookEngine } from "./guideBookEngine";
import { useBodyClass, useFooterVisibility } from "./bodyClass";

const SHEET_COUNT = 3;
const PAGE_COUNT = 6;

export function Guide() {
  useBodyClass("manual-page");
  useFooterVisibility();
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!rootRef.current) return;
    return attachBookEngine(rootRef.current);
  }, []);

  return (
    <div ref={rootRef}>
      <main className="book-guide">
        <section className="book-viewport" aria-label="Интерактивный мануал">
          <div className="book-scene">
            <div className="book" id="guideBook" aria-live="polite">
              <div className="book-cover book-cover--left" aria-hidden="true"></div>
              <div className="book-cover book-cover--right" aria-hidden="true"></div>
              <div className="book-paper-stack book-paper-stack--left" aria-hidden="true"></div>
              <div className="book-paper-stack book-paper-stack--right" aria-hidden="true"></div>
              <div className="book-placeholder book-placeholder--left" aria-hidden="true"><span>?</span><small>01—06</small></div>
              <div className="book-placeholder book-placeholder--right" aria-hidden="true"><span>конец</span></div>
              <div className="book-sheets">
                {Array.from({ length: SHEET_COUNT }, (_, sheetIndex) => {
                  const FrontPage = GUIDE_PAGES[sheetIndex * 2];
                  const backPageIndex = sheetIndex * 2 + 1;
                  const BackPage = backPageIndex < PAGE_COUNT ? GUIDE_PAGES[backPageIndex] : null;
                  return (
                    <div className="book-sheet" key={sheetIndex}>
                      <section className="book-sheet-face book-sheet-front" data-page={sheetIndex * 2}><FrontPage /></section>
                      <section className="book-sheet-face book-sheet-back" data-page={backPageIndex}>{BackPage ? <BackPage /> : null}</section>
                      <i className="book-sheet-edge" aria-hidden="true"></i>
                    </div>
                  );
                })}
              </div>
              <div className="book-spine" aria-hidden="true"></div>
            </div>
          </div>

          <div className="book-meta" aria-label="Положение в гайде">
            <span className="book-page-index" id="bookPageIndex">01</span>
            <div className="book-progress" id="bookProgress" aria-hidden="true">
              {Array.from({ length: PAGE_COUNT }, (_, index) => <i key={index}></i>)}
            </div>
            <span className="book-drag-hint">перетащите край страницы</span>
          </div>
        </section>
      </main>
    </div>
  );
}
```

### Step 6: Run it to see it pass

Run: `npm test -- src/features/public/Guide.test.tsx`
Expected: PASS.

### Step 7: Wire the route

In `src/features/public/PublicApp.tsx`, add:

```tsx
import { Guide } from "./Guide";
// ...
          <Route path="/guide" element={<Guide />} />
```

### Step 8: Run the full suite and lint

Run: `npm test` and `npm run lint`
Expected: both PASS.

### Step 9: Stop here, changes are ready for review.

---

## Task 10: Cleanup — delete the superseded old public surface, final verification

Removes the components this redesign replaces (now unreferenced since
Task 2 stopped importing them from `App.tsx`), confirms `PublicApp.tsx`
wires all eight routes, and runs the full verification suite end to end.

**Files:**
- Delete: `src/features/home/HomePage.tsx`
- Delete: `src/features/home/PublicShell.tsx`
- Delete: `src/features/schedule/PublicSchedulePage.tsx`
- Delete: `src/features/schedule/PublicSchedulePage.test.tsx`

**Interfaces:** none — this task removes code, it doesn't add any.

### Step 1: Confirm nothing still references the files being deleted

Run: `grep -rn "HomePage\|PublicShell\|PublicSchedulePage\|PublicHeader\|PublicFooter\|PublicMain\b" src --include=*.tsx --include=*.ts`

Expected: the only matches left are inside the four files being deleted
themselves (Task 2 already removed `App.tsx`'s imports of all of these).
If anything else matches, stop and fix that reference first — it means an
earlier task's edit didn't fully land.

### Step 2: Delete the four files

```
rm src/features/home/HomePage.tsx
rm src/features/home/PublicShell.tsx
rm src/features/schedule/PublicSchedulePage.tsx
rm src/features/schedule/PublicSchedulePage.test.tsx
```

`src/features/home/` is now empty — that's fine, git doesn't track empty directories, no further action needed.

### Step 3: Confirm `PublicApp.tsx` wires all eight routes

Read `src/features/public/PublicApp.tsx` and confirm it matches (route order doesn't matter, but every path must be present exactly once):

```tsx
import { BrowserRouter, Route, Routes } from "react-router-dom";
import type { Account } from "../../lib/api";
import { PublicLayout } from "./PublicLayout";
import { PublicHome } from "./PublicHome";
import { GroupPicker } from "./GroupPicker";
import { EntitySearch } from "./EntitySearch";
import { SchedulePage } from "./SchedulePage";
import { TeacherDetail } from "./TeacherDetail";
import { RoomDetail } from "./RoomDetail";
import { Guide } from "./Guide";

export function PublicApp({ onLoggedIn }: { onLoggedIn: (account: Account) => void }) {
  return (
    <BrowserRouter>
      <Routes>
        <Route element={<PublicLayout onLoggedIn={onLoggedIn} />}>
          <Route path="/" element={<PublicHome />} />
          <Route path="/group" element={<GroupPicker />} />
          <Route path="/teacher" element={<EntitySearch mode="teacher" />} />
          <Route path="/room" element={<EntitySearch mode="room" />} />
          <Route path="/schedule" element={<SchedulePage />} />
          <Route path="/teacher/:id" element={<TeacherDetail />} />
          <Route path="/room/:id" element={<RoomDetail />} />
          <Route path="/guide" element={<Guide />} />
        </Route>
      </Routes>
    </BrowserRouter>
  );
}
```

If any route is missing or the file still has the Task 2 placeholder route, fix it now.

### Step 4: Run the full test suite

Run: `npm test`
Expected: PASS, with no reference to any of the four deleted files anywhere in the output.

### Step 5: Run lint (typecheck)

Run: `npm run lint`
Expected: PASS — no dangling imports of the deleted files, no type errors anywhere in `src/features/public/**` or the modified `src/App.tsx`/`src/lib/api.ts`/`src/features/schedule/WeekGrid.tsx`.

### Step 6: Spot-check the route table against the design spec

Re-read the "Navigation architecture" table in
`docs/superpowers/specs/2026-08-08-schedule-home-redesign-design.md` and
confirm every row has a corresponding route in `PublicApp.tsx` from Step 3
above, and that no extra routes exist beyond those eight.

### Step 7: Stop here, changes are ready for review.

---

## Self-Review

**Spec coverage:**
- Navigation architecture (router mounted only for the public subtree, `App.tsx`'s state machine untouched for staff) — Task 2.
- `PublicLayout` + four shared dialogs, built once — Task 2.
- Visual/asset porting (stylesheets copied verbatim, fonts added, pixel-art static) — Task 1 (stylesheets/fonts), Task 8 (pixel-art).
- Contacts dialog icons inlined as local `lucide-react` icons, not the CDN script — Task 2.
- Delta: room search/schedule stays gated — Task 5 (`EntitySearch`), Task 6 (`SchedulePage`), Task 8 (`RoomDetail`).
- Delta: no ±2-weeks pager — never implemented anywhere in Tasks 6-8; `WeekSelector` only ever exposes the two real weeks.
- Delta: real week toggle bound to `OccurrenceWindow.meta.weeks` — Task 6 (`WeekSelector`).
- Delta: group picker's dropped 4th step → period-jump control on `/schedule` — Task 4 (dropped), Task 6 (`api.publicGroupPeriods` + period buttons).
- Delta: teacher/room detail real-fields-only — Task 8.
- Delta: Auth dialog calls real `api.login` — Task 2.
- Delta: cookie consent persists for real — Task 2.
- Delta: guide book ported as-is — Task 9.
- Data wiring summary: `PublicHome` (Task 3), `GroupPicker` (Task 4), `EntitySearch` (Task 5), `SchedulePage` (Tasks 6-7), `TeacherDetail`/`RoomDetail` (Task 8), `Guide` (Task 9) — every row covered.
- Testing section: `PublicSchedulePage.test.tsx`/`WeekGrid.test.tsx` updated/split (Tasks 5, 6, 7, 10), `GroupPicker` real-flow test (Task 4), `EntitySearch` search+pagination test (Task 5), route-gating tests for `/room` and `/room/:id` (Tasks 5 and 8).

**Placeholder scan:** every task's implementation steps contain complete,
compilable code (no `TODO`/`fill in`/"similar to Task N" references) except
where a step explicitly says "port verbatim from `schedule_home/X` lines
A-B" for pure markup/CSS the reader must copy from the referenced mockup
file rather than have re-transcribed a second time in this document — the
"No Placeholders" rule's documented exception.

**Type consistency:** `OccurrenceWeekGrid`'s new signature
(`{ week: DateScheduleWeek | null; loading?: boolean; subgroupFilter?:
number | "all" }`, introduced Task 6) is used identically in Task 6
(`SchedulePage`) and Task 8 (`TeacherDetail`/`RoomDetail`). `WeekSelector`'s
signature (`{ weeks, activeIndex, onSelect }`, Task 6) is likewise identical
across Tasks 6 and 8. `PublicOutletContext` grows from `{ openGuide }`
(Task 2) to `{ openGuide, openAuth }` (Task 5) — every consumer after Task 5
(`EntitySearch`, `RoomDetail`) uses the two-field version; `PublicHome`
(Task 3, written before the `openAuth` addition) only destructures
`openGuide`, which stays valid against the widened type. `api.listPublicRooms`'s
signature change (Task 1: drops `query`, adds `perPage`) is consumed
correctly by both callers that exist after Task 1 lands (`EntitySearch`
Task 5, `RoomDetail`/`TeacherDetail`'s room fallback Task 8) — no caller
anywhere still passes a query string positionally.
