# Final Whole-Branch Review — Remaining Public Pages Migration

Covers `/group`, `/schedule`, `/guide` together, after each passed its own
task review and fix loop. See the individual reports for per-page detail:
`2026-08-16-task-group-report.md`, `2026-08-16-task-schedule-report.md`,
`2026-08-16-task-guide-report.md`.

**Verdict: ready to ship.** No blocking issue found from the whole-branch
view. Three small, non-blocking items were found and fixed in one final
polish pass (below).

---

## (a) Rolled-up files touched, by layer

| Layer | On disk | From this migration |
|---|---|---|
| Entities | 6 | `faculty.rb` new; `schedule_event.rb` extended (+4 attrs, additive) |
| Gateways (portal) | 8 | 5 new: faculties, groups, group_periods, group_schedule, group_now |
| Operations | 5 | 2 new: `faculty_catalog_with_groups`, `group_schedule_window` |
| Presenters | 13 | 5 new: faculty_catalog, faculty_detail, group_periods, group_schedule, guide_pages |
| Policies | 2 | none (both reused as-is per brief ruling) |
| Controllers | 11 | `groups` rewritten, `schedules` + `guides` new |
| Views | — | `groups/` 5, `schedules/` 10, `guides/` 7; `shared/_week_selector` + `shared/_region_error` modified |
| Stimulus | 9 | 3 new (faculty, schedule, guide_book); `dialog_controller` modified |
| Stylesheets | 13 components | 4 new; `_region_error`, `_teacher_record`, `_site_header`, `_site_footer`, `_dialog`, `application.css` modified |
| Routes | 15 | 4 new: `group_faculty`, `group_periods`, `schedule`, `guide` |
| Container | 10 registrations | 5 new, lazy-block pattern |
| `KNOWN_KEYS` | 7 | 5 new, 0 from `/guide` (correct) |

`apps/api_geteway` and `apps/frontend` confirmed read-only throughout.

## (b) Cross-task observations (fixed or noted)

- **B1 (fixed)** — `_schedule.css`'s z-index 330 (dialog backdrops) wasn't recorded in `application.css`'s documented layer table. Non-exploitable, but drift in the one place meant to prevent it. **Fixed.**
- **B2 (noted, no action)** — `/group` tokenized every ported color; `/schedule` kept two hexes verbatim, faithfully matching the reference with no existing token for either shade. Defensible, just inconsistently disclosed between the two reports.
- **B3 (fixed)** — `/guide`'s `.book` rule still pairs `preserve-3d` with `filter`, the same grouping-property pattern fixed on `.book-sheet`. Harmless today (`.book-sheets` supplies the real 3D context) but a future-editor trap. **Comment added, no CSS value changed.**
- **B4 — the global constraint held.** "Adding the next page must not require editing an existing page" — verified in aggregate: every shared-file edit across all three tasks was extraction/repair (fixing a real bug, building a named reusable contract), never accretion. `/guide`, built last, touched zero existing page files and only the three files the recipe requires every page to append to. Caveat: the shell edits that made this true landed in `/group`'s *fix round*, after a reviewer caught it — the constraint held because it was enforced, not because it was automatic.
- **B5** — `ru.yml` survived two concurrent edits (`/schedule` fix round + `/guide`, running in parallel) intact: 19 top-level sections, zero duplicate keys at any depth, all 123 statically-referenced `t()` keys resolve.

## (c) Whole-branch verification (condensed; full output in the review transcript)

- `/readyz` breaker map: all 5 new keys present, 0 stray, 0 from `/guide`.
- `application.css` import chain: 13/13 component stylesheets 200, all `url(...)` form, coherent order.
- `/group` → `/schedule` handoff walked live end-to-end: lands on a real, correct, in-period schedule (not a 404, not out-of-period).
- Out-of-period page's own exits (both period links, picker link, header back) all verified working via real navigation.
- Shared `overlay-open` contract: `/schedule` and `/guide` correctly ignore it (no page overlay of their own); `/group` correctly participates. No page reintroduced the per-component restyling pattern `/group`'s fix round eliminated.
- Full regression sweep, 10 pages × 2 viewports: zero console errors, zero CSP violations, zero horizontal overflow, footer pinned (`gapBelowFooter=0`) everywhere, zero `translation missing`.
- Degraded-state sweep across all three pages together with `api_geteway` stopped: correct independent stale/region-error behavior everywhere; `/schedule`'s out-of-period state confirmed unable to mask a real failure; `/guide` unaffected (no upstream at all).
- Nothing staged or committed in any of the four repos (root, `schedule_web`, `api_geteway`, `frontend`).

## (d) Needs a human decision (complete list, carried from all sources)

1. **`/guide` has no UI entry point** — reachable only by URL, same as the React app. Options: a 4th row in the floor-picker dialog, or a control in the site footer. Neither implementer nor reviewer should pick this — it's yours.
2. **Back-port the backface-visibility CSS fix** to `apps/frontend/src/features/public/guide.css` and `schedule_home/guide.css` — both still ship a bug where leaflets 2/4/6 of the guide render as mirrored duplicates of 1/3/5. Confirmed still present in both files (untouched, out of this migration's scope). Two lines each, same fix already verified working in the Rails port.
3. **Host disk pressure on drive C:** — Docker died mid-migration at 0.2GB free, recovered to ~6.6GB by clearing caches. Will recur on the next build; `wsl --manage docker-desktop-data --set-sparse true` (WSL VHDX compaction) is the likely real fix, a host-level decision.
4. **Two `/schedule` states unverified end-to-end against a live payload**: `state: "cancelled"` and `subgroup_number` 1/2. No such row in seed data; a DB write to construct one was refused by the permission system (not worked around). Verified instead at presenter + real-view-partial level (86/86 checks). SQL to construct + verify + revert is in the `/schedule` report, section (d).
5. **No test suite exists** for `apps/schedule_web` (no `test/`/`spec/`, no test gems). All verification in this migration was live/Playwright-driven. Introducing a framework is a call for you.
6. **Cross-browser verification gap** — two behaviors (the `/group` `auto-fit` grid, `/guide`'s backface-visibility fix) verified in Chromium only; both are spec'd but historically inconsistent across engines, worth one Firefox/Safari pass.
7. **`Resilience::Call::KNOWN_KEYS`** still omits two pre-existing keys unrelated to this migration (`portal.teachers.schedule`, `admin.rooms.schedule`) — demonstrated live that a cold process under-reports them at `/readyz` until something exercises them. Cheap 2-line fix, correctly left out of scope by both tasks that touched the file.
8. **Focus containment across 5 new overlay surfaces** (faculty panel, event dialog, transfers dialog, plus the 2 pages' shared auth/contacts dialogs) — none traps focus; inherited from the pre-existing shared `dialog_controller`, not a regression, but the surface tripled. Worth one follow-up rather than three separate tickets. (The one instance that WAS fixed in this final pass: the faculty grid behind the open panel, via `inert` — see below.)

## Final polish pass (applied after the whole-branch review, before handoff)

1. **`application.css`** — registered the missing `330` z-index in the documented layer table (schedule/transfers dialog backdrops), between the existing 260 and 340 entries.
2. **`_guide_page.css`** — added an explanatory comment on `.book`'s harmless-but-confusing `preserve-3d` + `filter` pairing, cross-referencing the fuller diagnosis already written for `.book-sheet`, so a future editor doesn't re-diagnose it.
3. **`inert` on the faculty grid behind the open overlay panel** (`app/javascript/controllers/faculty_controller.js`, `app/views/groups/_scene.html.erb`) — the grid was `aria-hidden="true"` but its cards stayed keyboard-reachable, a real ARIA violation matching the exact bug class already fixed elsewhere in this branch (footer, back-button). Verified via Playwright: before the fix, Tab #1 from the open panel landed inside the hidden grid; after, none of the first 6 tab stops do, and closing the panel correctly restores the grid to the tab order (both JS-on and no-JS paths confirmed).

All three fixes verified live post-rebuild. Nothing staged or committed.
