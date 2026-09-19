# Schedule Builder Drawer Overlay Fix

> Completed work log (Phase D of the admin-panel epic).

**Goal:** Fix the reported bug where opening the SlotEditor drawer makes the "Use" button (and the rest of the external load-distribution panel) unreachable, breaking the drop-zone's own on-screen instruction to "press Use on a load row of this group."

**Files changed:** `apps/frontend/src/features/admin/schedule/SlotEditor.tsx`, `apps/frontend/src/styles.css`

## Root cause (corrected from the initial research pass)

The earlier codebase research attributed this to a CSS rule scoping the drawer to `width: 100vw` at narrow viewports via `.admin-product-shell .admin-inspector`. That rule can never match: `Inspector` (`workspace/Inspector.tsx`) renders its content through `createPortal(content, document.body)`, so `.admin-inspector` becomes a sibling of `#root` in the actual DOM, not a descendant of `.admin-product-shell` — the CSS descendant selector has no ancestry to match against, portal or no portal, regardless of viewport.

The real mechanism is simpler: the base rule alone (`.admin-inspector { position: fixed; inset: 0 0 0 auto; width: min(380px, 100vw); z-index: 20; }`) already covers nearly the entire width of any viewport ≤ ~400px, and being `position: fixed` it stays pinned to that screen region regardless of how far the underlying page is scrolled. The load panel sits in normal document flow to its left/above; at ≤1100px `.builder-layout` collapses to a single column, so the load panel and the grids stack vertically and both end up, at some scroll position, inside the same horizontal band the fixed drawer permanently occupies. There is no scroll position where a narrow-viewport user can see both the open drawer and the "Use" button.

Confirmed with a live before/after occlusion test (Playwright's `isVisible()` is not enough — it doesn't detect one element being visually covered by another; used `document.elementFromPoint()` at the same viewport coordinate before and after opening the drawer instead):
- Before opening the drawer: the coordinate resolves to `.load-row-use` (the button itself).
- After opening the drawer, same coordinate: resolves to `.load-pick-list` inside `.admin-inspector` — the drawer, not the button underneath it.

## Fix

Rather than resize the drawer or redesign the mobile layout, added a fallback picker *inside* `SlotEditor`'s existing drop zone (`.load-drop-zone`): when no load entry is assigned yet, it now renders the same `loadEntries` list (already passed into the component, already scoped to the current group) as a scrollable, clickable in-drawer list (`.load-pick-list`), alongside the existing drag-and-drop target. This makes the "pick a load row" action reachable from *inside* the drawer at any viewport width, without depending on the external panel being visible at all — the drag-and-drop path and the external "Use" button both still work unchanged at desktop widths where nothing is covered.

Verified live: the in-drawer list renders real data (46 entries for a test group), and a targeted point-based occlusion check confirms the new list button is genuinely the topmost, clickable element at its screen position at a 390px-wide viewport — the same width where the old external "Use" button had just been proven unreachable.

## Verification performed

- `npm run lint` (tsc -b) clean, `npm run test` (vitest): 23 files / 63 tests green.
- Live Playwright verification against the docker-compose stack at both desktop (1400px) and narrow mobile (390px) viewports, including the before/after `elementFromPoint` occlusion proof above.
