# Minimalist System and Full Admin CRUD Design

**Status:** Approved on 2026-07-16

## Objective

Redesign the complete client as a unified, highly readable minimalist system and expose every mutation supported by `apps/api_geteway/config/routes.rb` through an appropriate administrator workflow. All interface copy remains Russian. Engineering documentation remains English.

## Source of truth

The gateway routes and their Dry Validation contracts are authoritative. Documentation provides intent and examples, but the client must not invent unsupported actions. In particular:

- regular resources expose index/create/update/delete;
- operational resources such as holidays and reservations expose index/create/delete only;
- audit logs and sanitized media projections are read-only;
- nested schedule, group-semester, load, transfer, constraint, recommendation, and upload operations require dedicated workflows;
- map endpoints are read-only and therefore are not part of admin CRUD.

## Visual direction: Quiet Institutional Minimalism

The design is light, calm, and operational. It must remain understandable within five seconds for an administrator and readable during long university work sessions.

### Typography

- Use the native system sans-serif stack (`Inter`, `Segoe UI`, `Roboto`, sans-serif) for dependable Cyrillic rendering and no font-loading dependency.
- Body text is 1.125rem with a 1.55 line height.
- Controls and table content are at least 1rem.
- Secondary text is at least 0.875rem; no essential text may use the current 8–11px treatment.
- Page titles use `clamp(2.5rem, 5vw, 4rem)` and section titles use 1.5–2rem.
- Use normal sentence casing. Tiny all-caps eyebrow labels and decorative monospaced metadata are removed.
- Use `text-wrap: balance` only for short headings and `text-wrap: pretty` for longer descriptions as progressive enhancement.

### Color and surfaces

- Canvas: warm neutral `#f6f6f3`.
- Primary surface: `#ffffff`.
- Primary text: `#171717`; secondary text: `#646464`.
- Borders: `#deded9` with stronger interactive borders at `#b9b9b2`.
- Action accent: restrained blue `#2457d6`; destructive accent: `#b42318`.
- No gradients, glow, glass blur, decorative orbits, neon cyan/lime pairing, or ambient background graphics.
- Status is never communicated by color alone; text and icons accompany it.

### Geometry and spacing

- Corners are 8–12px, not pill-shaped except for compact statuses.
- Controls have a minimum 44px target; coarse pointers receive 48px.
- Content uses an 8px spacing scale and a maximum application width of 1440px.
- Tables own horizontal overflow. Page shells must never clip content.
- Motion is limited to 120–180ms opacity/background/border transitions and respects reduced motion.

### Public and self-service surfaces

- The landing page becomes content-first: one plain headline, one supporting paragraph, and direct schedule choices.
- Public schedule selectors and week grids use the same form, table, empty-state, and status components as administration.
- Teacher self-service uses readable tables and lists without decorative hero structures.
- Authentication and forced password change use a single-column form surface with visible labels and conventional actions.

## Admin information architecture

The primary navigation has six task-oriented areas:

1. **Обзор** — system status, current semester, recent audit activity, and direct task entry points.
2. **Расписание** — schedule grid, schedule events, teachers on events, transfers, constraints, and recommendations.
3. **Учебный процесс** — curriculum streams, load requirements, teaching assignments, room preferences, group semesters, periods, and bulk load distribution.
4. **Данные университета** — organizational, staff, subject, building, room, time, and dictionary resources.
5. **Календарь и аудитории** — holidays, weekday transfers, room reservations, availability, files, and media state.
6. **Доступ и система** — accounts, scopes, audit log, materialized views, and refresh controls.

On narrow screens the navigation becomes a horizontally scrollable tab row with hidden scrollbar chrome and keyboard accessibility.

## Resource capability matrix

### Standard collection CRUD

Each resource below receives list, create, edit, and delete in the shared resource workspace:

| Group | Resources |
|---|---|
| Organization | `faculties`, `kafedras`, `specialities`, `student_groups`, `semesters` |
| Staff | `teachers`, `teacher_on_kafedras`, `posts`, `academic_degrees`, `academic_titles` |
| Curriculum | `subjects`, `activity_types`, `curriculum_streams`, `study_load_requirements`, `teaching_assignments`, `assignment_room_references` |
| Space | `buildings`, `building_types`, `rooms`, `room_types`, `aviability_types`, `room_avialabilities` |
| Time | `time_slots`, `week_types` |
| Scheduling | `schedule_events` |
| Access | `/auth/accounts` |

The misspelled route identifiers `aviability_types` and `room_avialabilities` are preserved only in transport/configuration code. Russian UI labels use correct terminology.

### Limited-capability collections

The workspace exposes only supported actions:

| Resource | Supported admin actions |
|---|---|
| `holidays` | list, create, delete |
| `weekday_transfers` | list, create, delete |
| `room_reservations` | list, create, delete |
| `teacher_time_constraints` | create, delete; list through `/admin/views/teacher_constraints` |
| `event_teachers` | create, update, delete inside a schedule event |
| `event_transfers` | list/create from a schedule event, delete by transfer id |
| uploaded files | list, upload, request clean URL, delete |
| media files | list/show/status/URL only |
| audit logs | list/show/filter only |

### Nested workflows

Dedicated editors cover:

- add, generate, update, and delete student-group semesters;
- create/read/delete a group-semester load distribution;
- read and upsert group-semester periods;
- create schedule recommendations and materialize a selected recommendation;
- create/update/delete schedule events and their teacher bindings;
- create/delete teacher time constraints;
- create/list/delete event transfers;
- upload Excel and media files, poll media sanitation status, obtain clean URLs, and delete uploads;
- refresh materialized views.

## Shared CRUD architecture

### Resource registry

`adminResourceDefinitions.ts` contains one definition per standard or limited collection:

- Russian singular/plural labels and navigation group;
- endpoint resource name and payload envelope key;
- supported actions;
- table columns;
- field definitions for create and update;
- scalar type, required state, native constraints, and help text;
- relation target, option label strategy, and relationship-to-id conversion;
- read-only server-owned attributes;
- optional data normalization/serialization functions.

Definitions are typed and covered by a matrix test. A resource cannot appear in navigation without a valid endpoint, labels, columns, and action configuration.

### Generic API transport

The API client provides typed generic methods:

- `listAdminResource(resource, query)`;
- `createAdminResource(definition, values)`;
- `updateAdminResource(definition, id, values)`;
- `deleteAdminResource(resource, id)`.

Specialized route methods remain explicit. JSON:API collection responses retain `included` and `meta`. Mutation bodies use the envelope names required by each contract.

### Resource workspace

`AdminResourceWorkspace` provides:

- grouped resource navigation;
- title, description, count, search, filters, and pagination;
- semantic table with sticky header and readable rows;
- loading, empty, error, and retry states;
- create/edit drawer or native modal dialog;
- explicit delete confirmation naming the record;
- post-mutation refetch and visible success state;
- server validation errors at form and field level;
- capability-aware controls and role-aware access.

### Forms

`ResourceForm` uses semantic `form`, `fieldset`, `legend`, explicit `for`/`id` labels, native input types, and visible required indicators. Relationship fields load live options and display meaningful names rather than raw ids. Forms use a single column by default and at most two columns on wide screens. Submit remains available for native validation and is disabled only during an active request.

### Authorization

The client combines static route capability with `meta.actions` when present. Navigation and controls are hidden when definitely unavailable, but API `403` remains authoritative. A denied mutation keeps current data intact and displays a clear Russian error.

## State and error handling

- Collection state is scoped by resource, query, and page.
- Changing resource cancels or ignores stale responses.
- Mutations are pessimistic: data changes only after API success.
- Deletion requires a confirmation dialog and refreshes the current page.
- `422` errors are parsed into field messages; `401` uses the existing refresh flow; `403` explains missing permissions; `409` explains conflicts when supplied by the server.
- Read-only resources display a visible `Только просмотр` status.
- Empty data is a normal state, not an error.

## Accessibility and responsive behavior

- Document language is Russian.
- Every control has a visible programmatic label.
- Icon-only actions include resource-specific accessible names.
- Focus indicators meet contrast requirements and are never removed without replacement.
- Tables include captions or equivalent headings and retain headers during scrolling.
- Dynamic mutation outcomes use one polite announcement region; critical failures use `role="alert"`.
- At 200% zoom, functionality remains available without page-level horizontal clipping.
- Reduced-motion preference removes nonessential transitions.

## Testing and completion evidence

Completion requires all of the following:

1. A route-to-interface matrix proving every mutable gateway route has an admin workflow.
2. Unit tests validating every resource definition, payload envelope, field conversion, and supported action.
3. Component tests for list, create, edit, delete, limited capability, read-only state, validation error, and role restriction behavior.
4. Dedicated workflow tests for group semesters/load/periods, scheduling/teachers/transfers/constraints/recommendations, uploads/media, accounts/scopes, and view refresh.
5. Full TypeScript, Vitest, and production build success.
6. Docker Compose rebuild and live API verification with the supplied administrator account.
7. In-app browser QA at desktop, tablet, and mobile widths, including keyboard operation, 200% text zoom, console health, and screenshots.

## Explicit non-goals

- No editing for read-only map, audit, media-projection, search, or public endpoints.
- No invented update operation for create/delete-only routes.
- No raw JSON editor as a substitute for typed forms.
- No Cloudflare or production CDN work; existing caching remains local/development-only.
