# Local Nginx Caching and API-Driven Schedules

## Objective

Turn the current frontend into a same-origin, API-driven application for public users,
teachers, and administrators. Nginx will provide local/development static delivery and
browser-cache policy without relying on Cloudflare or another external CDN.

The interface remains Russian. Source code and project documentation remain English.

## Authoritative Sources

- `apps/api_geteway/config/routes.rb` defines the routes the frontend may call.
- `docs/17-api-frontend-reference-2026-07-15.md` defines current response contracts.
- `docs/ТЕХЗАДАНИЕ (2) (1).docx` defines the product goals: schedule management,
  teacher workload, room occupancy, visible schedule changes, access separation,
  responsive operation, and a two-second response target under normal conditions.
- Older endpoint examples in `docs/04-slice-main.md`, `docs/06-frontend-architecture.md`,
  and `docs/07-frontend-pages.md` are product guidance only when they disagree with the
  live route file or the 2026-07-15 API reference.

## Delivery Architecture

### Nginx

Nginx is the single browser-facing origin in Docker development.

- `/assets/*` serves Vite content-hashed files with `Cache-Control: public, max-age=31536000, immutable`.
- `/index.html` and SPA fallbacks use `Cache-Control: no-cache` so a rebuild is visible immediately.
- `/api/*` proxies to `http://api_geteway:2300/*`, removes the `/api` prefix, forwards standard proxy headers,
  and sends `Cache-Control: no-store`.
- API streaming is not buffered for `/api/admin/stream`.
- Gzip is enabled for HTML, CSS, JavaScript, JSON, SVG, and common text formats.
- Security headers are added at the frontend origin without introducing local TLS requirements.
- Unknown asset paths return `404`; only application routes fall back to `index.html`.

This is intentionally local/development caching, not a simulated external CDN. A future
CDN can use the same cache headers without changing React code.

### Frontend API Base

Production Docker builds use the relative base URL `/api`. Vite development proxies
`/api` to `http://localhost:2300`, preserving the same browser request paths in both
environments. An explicit `VITE_API_BASE_URL` remains available for exceptional setups.

Authentication and all domain data come from the API. The built bundle contains no
credentials and no preloaded schedule, account, teacher, room, faculty, or semester data.

## Data and Authentication Flow

The existing fetch wrapper remains lightweight but gains clear request contracts:

- login: `POST /auth/sessions`;
- refresh: `POST /auth/sessions/refresh`, rotating both tokens;
- logout: `DELETE /auth/sessions`;
- password change: `POST /auth/password` when `must_change_password` requires it;
- public selectors and schedules: current `/public/*` routes;
- teacher self-service: `/admin/me`, `/admin/me/schedule`, `/admin/me/load`, and `/admin/me/kafedras`;
- administration: only existing `/admin/*` and `/auth/accounts` routes.

Access and refresh tokens continue to use session-scoped browser storage because the
current API returns both tokens in JSON rather than an HTTP-only refresh cookie. The API
client retries one failed authorized request after a successful refresh and clears the
session after a second authorization failure.

Read-only reference queries may be cached in memory for a short period and deduplicated.
Authentication, current schedule data, mutations, and user-specific teacher data are
always requested from the API and are never served from Nginx cache.

## Public Schedule Experience

The public shell exposes three Russian-language workflows:

1. Group schedule: faculty -> semester -> group -> `/public/groups/:group_semester_id/schedule`.
2. Teacher schedule: semester + teacher -> `/public/teachers/:teacher_id/schedule?semester_id=`.
3. Room schedule: semester + room -> `/public/rooms/:room_id/schedule?semester_id=`.

Selectors load their options from public API endpoints. Parent changes clear dependent
selections. Independent initial requests run in parallel. Every page has loading,
validation, error, empty, and loaded states.

A shared week grid groups live JSON:API schedule rows by day and renders time, subject,
activity type, teacher or group, and room. Manual changes and transferred events receive
a distinct accent treatment. Missing rooms are shown as an intentional absence rather
than a rendering error. The grid keeps tabular scanability and scrolls inside its own
container on narrow screens.

## Teacher Experience

Teacher accounts enter a focused self-service shell instead of the admin dashboard.

- “Моё расписание” renders `/admin/me/schedule` through the shared week grid.
- “Моя нагрузка” renders structured workload rows from `/admin/me/load`, including
  subject, activity type, assigned hours, and available group/semester context.
- “Мои кафедры” renders readable department membership from `/admin/me/kafedras`.
- Profile identity and authorization state come from `/admin/me` where available, with
  JWT claims used only as the bootstrap fallback required by the current login response.

Raw JSON is not part of the user-facing success state. It may appear only in development
diagnostics outside production UI.

## Component Boundaries

The current monolithic `App.tsx` will be reduced along the paths touched by this work:

- `src/lib/api.ts`: transport, authentication refresh, and typed endpoint methods;
- `src/features/schedule/`: public selectors, response normalization, and week grid;
- `src/features/teacher/`: teacher schedule, workload, and department views;
- `src/features/auth/`: login and password-change behavior;
- `src/components/`: existing shared primitives only.

The refactor is limited to functionality required by this design. Existing visual tokens
and the approved dark operational design remain unchanged.

## Error Handling

- Network failures retain the current page and offer a retry action.
- `401` triggers one token refresh attempt.
- `403 password_change_required` routes to the password-change form.
- `403` for other operations is shown as insufficient access.
- `422` displays field or query validation messages in Russian.
- Public rate limiting (`429`) displays a wait-and-retry state.
- Empty API collections render a purposeful empty state, not sample content.

## Verification

Completion requires all of the following evidence:

1. `npm run build` and the TypeScript check pass.
2. `docker-compose --env-file .env.local up -d --build` succeeds.
3. Hashed assets return immutable one-year cache headers.
4. `index.html` returns `no-cache`; proxied API responses return `no-store`.
5. The browser loads public faculties through `/api/public/faculties` on the Nginx origin.
6. Supplied administrator credentials complete the real login flow through `/api/auth/sessions`.
7. Public group and teacher schedule flows render live or valid empty API states.
8. Teacher self-service components render structured API responses without raw JSON.
9. Desktop and mobile browser checks show no framework overlay, relevant console errors,
   page-level overflow, or broken primary interactions.

## Out of Scope

- Cloudflare, CloudFront, DNS, certificates, or external edge provisioning.
- New backend routes or changes to API response contracts.
- Offline schedule storage or service-worker caching.
- Replacing the existing visual design system.
