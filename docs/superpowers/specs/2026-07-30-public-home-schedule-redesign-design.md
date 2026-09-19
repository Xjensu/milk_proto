# Public Home + Schedule Lookup Redesign — Design

Date: 2026-07-30
Scope: `apps/frontend` (separate nested git repo)

## Goal

Fully redesign the two public-facing surfaces of the schedule product — the home
screen and the schedule-lookup screen (group / teacher / room) — for a calmer,
more distinctive first impression than a typical university site. Keep the
underlying functionality (faculty → group schedule, teacher schedule, room
occupancy-by-day, all without login) and the admin panel untouched.

## Out of scope

- Admin panel (`AdminPanel.tsx` and everything under `features/admin`)
- Login / password-change / teacher self-service (`Login`, `PasswordChange`,
  `TeacherSelfService`) — visually unchanged, still reachable
- `WeekGrid.tsx`, `.button`/`.badge`/`.panel`/`.schedule-empty`/
  `.request-status`/`.request-error` base CSS — shared with admin and/or
  teacher self-service; not edited

## Reference direction

Synthesized from three references (screenshotted, not copied): BrightEdge
(oversized editorial type, numbered rows), Venture FZ (bold color-block hero,
ghost watermark type, connected-dot stat rail), State of AI Design (grainy
asymmetric photo collage, small mono pill labels, editorial restraint).

Deliberately avoids the current AI-generated-design default of
cream background + high-contrast serif + terracotta accent.

## Token system

- **Color**: paper `#EDEEEA` (cool "foggy stone", not cream), raised surface
  `#F7F7F3`, ink `#1B1D18` (soft warm-black), muted `#6E7268` (sage-grey),
  hairline `rgba(27,29,24,.12)`. Single accent: indigo-violet `#4B3FE0` +
  pale lavender tint `#DCE0F5`. No second accent — restraint over decoration.
- **Type**: Fraunces (new import, soft-curved serif, display headlines only)
  + Manrope (existing, body/UI) + DM Mono (existing, small labels/data).
- **Layout**: asymmetric editorial grid, hairline rules as the primary
  structural device instead of boxes/shadows, generous negative space —
  visually the opposite of the admin panel's dark glass-panel look.
- **Signature**: a thin animated "pulse line" (SVG heartbeat/waveform, gsap,
  respects `prefers-reduced-motion`) threading through the hero stats and
  reused as the divider between days in schedule results — a literal
  rendering of "ритм" (rhythm), the word already used in the product's copy.
- **Copy language**: all Russian. English references only shaped the visual
  language, not the on-page text.

## Home page

Header (wordmark + quiet "Войти" link; dead-end nav items "Возможности" /
"О системе" removed) → hero with an asymmetric grain-photo collage
(desaturated stock imagery, floating mono labels) beside a large Fraunces
headline and short subhead → **the three lookup paths (Группа /
Преподаватель / Аудитория) live directly in the hero as quiet arrow-links**,
not gated behind a scroll. Below: a short pulse-line stat strip, then a
minimal footer. No long marketing scroll.

## Schedule lookup page

Same quiet header with a back-link. Page title and selector labels are
**unchanged on purpose** — `"Расписание группы"` / `"Расписание
преподавателя"` / `"Занятость аудитории"`, `Факультет` / `Семестр` /
`Группа` / `Показать` — because `PublicSchedulePage.test.tsx` asserts these
exact strings and they're functional, not decorative. Visuals are fully
restyled (light selectors, generous spacing); results render as calm
day-cards with hairline dividers and the pulse-line motif, replacing the
current dark `week-day` treatment.

## Technical safety plan

- New files, nothing shared is edited in place:
  - `src/features/home/HomePage.tsx` (extracted out of `App.tsx`)
  - New `PublicHeader` / `PublicFooter`, rendered only for
    `home | group | teacher | room` views. Old `Header`/footer markup is
    untouched and still used for `login | password | self | admin`.
  - New results component for the schedule page instead of reusing
    `WeekGrid` (also depended on by `TeacherSelfService` and
    `AdminSchedule`).
- CSS is additive only: new `.public-*` / `.lookup-*` classes with their own
  locally-scoped custom properties. No edits to `:root` tokens or to
  `.button` / `.badge` / `.panel` / `.schedule-empty` / `.request-status`
  base rules.
- `api.ts` and the selection/fetch logic in `PublicSchedulePage.tsx`
  (cascading selectors, `toGroupOptions`, loading/error states) are
  unchanged — presentation only.
- Images: hosted placeholder stock-photo URLs (Unsplash), commented as
  swappable, no binary assets committed.

## Workflow

Build Home → self-critique against the `impeccable` skill checklist → spawn
one fresh-eyes review agent → apply its real feedback once → move to the
Schedule page → repeat. Verify both in a running dev server before calling
the work done.

## Testing

`PublicSchedulePage.test.tsx` and `toGroupOptions` tests must keep passing
unmodified (string assertions listed above). No new automated tests are
planned for the visual layer; verification is via the dev server + browser
+ the agent review pass.
