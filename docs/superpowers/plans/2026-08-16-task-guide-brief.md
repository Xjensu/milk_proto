# Task Brief — `/guide` page (interactive instructional book)

> Self-contained. You have no other context beyond this file, the mission
> doc it was derived from (`docs/superpowers/plans/2026-08-16-remaining-public-pages-migration-prompt.md`
> — read it first for ground rules, architecture, landmines and the
> verification protocol; this brief only fills in specifics for this page),
> and the repository itself.

## What this page is — and why most of the recipe doesn't apply

This is a **fully static, content-only page**: six fixed instructional
"leaflet" pages laid out as a book you drag-flip through. There is no
upstream API call anywhere in it — confirmed by reading `Guide.tsx`,
`guidePages.tsx` (all six `GuidePage0`..`GuidePage5` components, full file),
and `schedule_home/guide.html` (whose six `<template>` blocks are **verbatim
identical markup and copy** to the six React components — compared side by
side, they match exactly, down to the Russian text).

That means recipe steps 1-4 of mission §4 (**Entity, Gateway, container
registration, Presenter**) genuinely do not apply to this page — there is no
upstream resource to wrap. This is a deliberate, stated deviation from "if a
step feels unnecessary, that's a signal to re-read the existing pages" (mission
§4): every *other* page in this app has upstream data: this one doesn't, and
forcing an empty Gateway/Presenter pair into existence would be inventing
structure with nothing to hold. Say so plainly in your report; don't silently
skip the steps without a controller/view — you still need those.

## Relationship to `shared/_guide_dialog` — do NOT share content

Read `app/views/shared/_guide_dialog.html.erb` and
`apps/frontend/src/features/public/GuideDialog.tsx`. **These are a
completely different feature that happens to share the word "guide":** the
dialog is a three-row floor-picker shortcut (Группа→3F / Преподаватель→2F /
Аудитория→1F) that jumps to `/group`, `/teacher`, `/room` — it takes a
`floors:` local (array of `{index:, target:, path:}`) and has zero content
overlap with the six-page instructional book this task builds. Confirmed by
reading both files in full. **Keep them fully separate** — no shared
partial, no shared controller, no shared stylesheet. The only thing they
should have in common is that the dialog's own "Открыть гайд" trigger
(`guide.open` in `ru.yml`, and `.guide-trigger` in
`schedule_home/styles.css` ~line 1191, which is explicitly documented there
as "intentionally static — no hover relief") may reasonably link to this new
`/guide` page — verify whether that trigger currently does anything, and if
it's currently a dead link/no-op, wire it to `guide_path`.

## Content — port verbatim, into `ru.yml`

The six pages' full Russian copy (headings, labels, captions) is already
written and reviewed — in both `guidePages.tsx` and `guide.html`'s
templates. Port it verbatim. Per mission ground rules ("UI strings are
Russian, in `config/locales/ru.yml`, always via `t()`" — no exception stated
for content pages), **every string on this page goes through `t()`**, added
under a new `guide_page:` section in `ru.yml`, additive, nested by page
number to keep it navigable:

```yaml
guide_page:
  title: "Гайд"
  page_index_aria: "Положение в гайде"
  drag_hint: "перетащите край страницы"
  pages:
    "0":
      kicker: "01"
      section: "Расписание"
      heading_html: "Как найти<br />расписание"
      menu:
        - { mark: "01", label: "Группа" }
        - { mark: "02", label: "Преподаватель" }
        - { mark: "03", label: "Аудитория" }
    "1":
      kicker: "02"
      section: "Группа"
      heading_html: "Факультет<br />Курс<br />Группа"
      # ... course/subgroup/week selector labels, faculty-stack caption
    # pages 2-5 similarly — see guidePages.tsx / guide.html for exact copy
```
(This is a sketch of the shape, not a literal requirement to use
`_html` suffixed keys or this exact nesting — Rails' `t()` with `<br />`
inside a string needs either `_html`-suffixed keys with
`html_safe`-by-convention interpolation, or splitting the heading into two
lines rendered with a real `<br>` in the ERB and two separate `t()` calls.
**Prefer the latter** — two `t()` calls plus a literal `<br>` in the template
avoids relying on Rails' magic `_html` suffix convention and keeps escaping
obviously correct for API-derived-content safety habits elsewhere in this
app (mission §8: never defeat escaping with `html_safe`/`raw`; this content
isn't API-derived, but consistency avoids a "did they mean to do that"
review question). Structure the six pages' locale keys however reads
cleanest once you're actually writing the ERB — the sketch above is a
starting point, not a contract.

Also port page 4's mini-timetable (`.leaflet-table`) and page 2's
mini-schedule (`.leaflet-mini-schedule`) — these are static illustrative
mockups (fake subject codes like "ПИ"/"БД"/"ОС", fake room numbers), **not**
live data. Their cell contents count as UI copy too — `t()` them the same
way, don't wire them to a real schedule call (that would contradict "no
upstream resource" above and add a fifth network dependency for decoration).

## Interaction — port the book engine as a Stimulus controller

Reference behavior lives in two places, both worth reading before you start
(they're substantial — 330 and 476 lines respectively):
- `apps/frontend/src/features/public/guideBookEngine.ts` (the React port —
  closer to what you're doing, since it already operates on a raw DOM ref
  rather than React state, per `Guide.tsx`'s `useEffect(() => {
  return attachBookEngine(rootRef.current); }, [])` — this is really just
  "vanilla JS attached once," which is exactly the shape a Stimulus
  `connect()`/`disconnect()` pair wants).
- `schedule_home/guide.js` (the original static-mockup version — 476 lines,
  slightly more because it also builds the six `<template>` clones at
  runtime; you don't need that part since ERB renders all six pages
  server-side).

Port `guideBookEngine.ts`'s logic into
`app/javascript/controllers/guide_book_controller.js`. Key behaviors to
preserve (read the source for exact mechanics — this is a summary, not a
spec): drag-to-flip page corners, click/tap a page edge to advance, current
page index indicator (`#bookPageIndex`), progress dots (`#bookProgress`,
one per page, active state tracks current page), left/right placeholder
covers before page 1 / after page 6, `aria-live="polite"` on the book root
so page changes are announced.

Since Rails renders all six `<article>` pages directly in the DOM (unlike
the React version's `<template>`-based lazy instantiation), **the content is
always present and readable even if JS fails to load or the drag interaction
breaks** — a real accessibility improvement over the SPA version. Preserve
that: don't hide pages with `display: none` in a way that depends on JS
running to reveal them; the reference's own CSS handles the 3D book
positioning declaratively (verify this by reading the CSS before assuming
you need a JS-driven initial-state class).

`Guide.tsx` wraps `<main>` in an extra `<div ref={rootRef}>` only because
React refs need a DOM node to attach to before `<main>` exists in some
render paths — **you don't need this wrapper in Rails.** Attach the Stimulus
controller directly to `<main data-controller="guide-book">` (or a scoped
child if `<main>` itself needs to stay controller-free per this app's
conventions — check how other pages with Stimulus controllers scope theirs).
Avoids landmine #6 (a wrapper around `<main>` unpins the footer) entirely by
not introducing one.

## Visual spec

`schedule_home/guide.html` (structure — the `<template>` blocks map 1:1 to
your six ERB partials/sections) and `schedule_home/guide.css` (478 lines,
the full stylesheet for the book — read it in full, it's dense 3D-transform
CSS for the page-flip effect and isn't meaningfully summarizable into a line
range like the other pages' blocks). `schedule_home/styles.css` line 3554
(`/* The guide entry stays flat; the interactive paper object lives on
guide.html. */`) confirms `styles.css` itself has no guide-page-specific
rules beyond the trigger button — everything else is in `guide.css`.

Key classes: `.book-guide` (root, becomes `body`'s `main` class — reference
uses it directly on `<main>`), `.book-viewport`, `.book-scene`, `.book`,
`.book-cover--left`/`--right`, `.book-paper-stack--left`/`--right`,
`.book-placeholder--left`/`--right`, `.book-sheets`, `.book-sheet`,
`.book-sheet-face` / `.book-sheet-front` / `.book-sheet-back`,
`.book-sheet-edge`, `.book-spine`, `.book-meta`, `.book-page-index`,
`.book-progress`, `.book-drag-hint`, and per-page classes
`.leaflet-page`/`.leaflet-page--intro`/`--schedule`/`--changes`.

The React version's sheet/page pairing (`SHEET_COUNT = 3`, `PAGE_COUNT = 6`,
front = `sheetIndex*2`, back = `sheetIndex*2+1`) is the book's physical
structure (3 double-sided sheets = 6 pages) — replicate this pairing exactly
in the ERB loop, not a flat list of 6 independent pages, or the 3D flip
transform (which targets `.book-sheet`, not individual pages) will animate
the wrong element.

## Architecture — the recipe, filled in

**1-4. Entity/Gateway/container/Presenter** — skipped, see above. A trivial
"presenter" is fine if it makes the controller/view boundary cleaner (e.g.
`Presenters::GuidePages` that just exposes the six pages' locale-key
prefixes as an array, if that reduces ERB repetition) but is not required by
the architecture — use your judgment, note the choice.

**5. Controller** — `app/controllers/guides_controller.rb` (new):
```ruby
class GuidesController < ApplicationController
  def index
  end
end
```
Route: `get "guide" => "guides#index", as: :guide` in `config/routes.rb`
(append to the existing route list, don't reorder).

**6. View** — `app/views/guides/index.html.erb`, structured per the visual
spec above. Consider a partial per page (`_leaflet_page.html.erb` taking a
`page:` local keyed to the locale prefix) to avoid six near-identical ERB
blocks, since the six pages share a `<header class="leaflet-page-head">` +
`<h1>` shape even though their bodies differ — your call on how much to
factor out versus keep explicit; six pages with meaningfully different
bodies (menu list / selector row / search-field / table / change-list) may
not compress cleanly into one partial without a worse "what does this
even render" readability cost. Don't force it if it doesn't fit.

**7. Stylesheet** — `app/assets/stylesheets/components/_guide_page.css`
(new), the full port of `schedule_home/guide.css`. Note: the reference only
`<link>`s `guide.css` on the guide page itself (conditional load), but this
app's established pattern (`application.css`'s header comment: "one file per
component... nothing here may position a component") is to **always**
import every component stylesheet globally regardless of which page uses
it — follow that existing pattern (`@import
url("components/_guide_page.css");` in `application.css`) rather than
reintroducing the reference's conditional-load approach; this app has no
per-page `<link>` mechanism and building one just for this page would be a
parallel loading strategy the rest of the app doesn't have.

**8. Locale keys** — see the Content section above; this page has more
locale keys than any other in this app (six pages of instructional copy) —
budget real time for getting this right rather than rushing it, it's the
bulk of the actual content work here.

## Landmines that apply here (mission §5)

- **#3/#4** — you will be iterating on `guide.css` (a large, unfamiliar
  3D-transform file) and the Stimulus controller heavily; every visual
  check needs a real rebuild (`docker compose build schedule_web`), and the
  new stylesheet import needs the `url(...)` form.
- **#8** — this page has the most ERB comments of any page in this app
  (six pages' worth of ported structure) — double-check none contain a
  literal `%>` sequence inside a `<%# %>` comment.
- **#9** — the drag-to-flip interaction has many targets (each page's own
  edge, the progress dots, potentially prev/next affordances) inside one
  controller scope; verify every one still works after a full page load,
  not just the first page you happen to test.
- **#10** — if any part of the book uses a `visibility` transition for
  page-turn state (check `guide.css` for this pattern, it's used elsewhere
  in the reference for dialogs), use the same asymmetric-timing fix, not a
  naive `transition: visibility <dur>`.
- **#7** — if you build any tag with conditional classes (e.g. the active
  progress dot), assemble the attributes hash before emitting the tag
  (`tag.attributes(...)`), not via inline `<% if %>` fragments inside an
  open tag.

## Definition of done (subset of mission §11)

1. `GET /guide` returns 200, all six pages' content is present in the
   rendered HTML (view source / `curl`, not just "looks right in the
   browser" — this is the accessibility win of SSR-ing static content,
   verify it's real).
2. Drag-to-flip and click-to-advance both work in a real browser; page
   index and progress dots update correctly; keyboard-reachable (tab to a
   control that advances the page, not just mouse drag).
3. No live API calls originate from this page (check the Network tab —
   zero requests beyond the page load itself and its own assets).
4. Assets 200, zero console/CSP errors at 1280/375, no horizontal overflow,
   footer pinned (the book's `<main>` is not wrapped, per the landmine #6
   note above — verify the footer is still correctly at the bottom on this
   page specifically, since it's visually the most complex layout so far).
5. No `translation missing` anywhere on the page — given the volume of new
   locale keys here, this is the most likely place to miss one; a full
   render pass through all six pages is required, not a spot check.
6. The `_guide_dialog` partial (floor-picker) still renders and functions
   correctly on whatever page(s) trigger it — confirm this task's changes
   didn't touch its file or the `guide:`/`floors:` locale keys it depends on.
