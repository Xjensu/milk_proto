# Task Report — `/guide` page (interactive instructional book)

Brief: `docs/superpowers/plans/2026-08-16-task-guide-brief.md`
Mission: `docs/superpowers/plans/2026-08-16-remaining-public-pages-migration-prompt.md`
App: `apps/schedule_web` (Rails 8 + Hotwire + Falcon, port 3000)

Status: **DONE_WITH_CONCERNS** — the page is complete and verified, but building it
surfaced a real rendering defect in the design reference *and* in the React port
(half the guide was unreachable behind a mirror image of the other half). I fixed
it rather than porting it; see §(b).4. That fix is the one thing in this task that
a human may want to look at, and it is worth back-porting to `apps/frontend`.

---

## (a) Files created / modified, by layer

### New — exclusively mine

| Layer | File | Lines |
|---|---|---|
| Controller | `apps/schedule_web/app/controllers/guides_controller.rb` | 18 |
| Presenter | `apps/schedule_web/app/lib/presenters/guide_pages.rb` | 63 |
| View | `apps/schedule_web/app/views/guides/index.html.erb` | 94 |
| View | `apps/schedule_web/app/views/guides/_page_0.html.erb` | 30 |
| View | `apps/schedule_web/app/views/guides/_page_1.html.erb` | 54 |
| View | `apps/schedule_web/app/views/guides/_page_2.html.erb` | 35 |
| View | `apps/schedule_web/app/views/guides/_page_3.html.erb` | 29 |
| View | `apps/schedule_web/app/views/guides/_page_4.html.erb` | 45 |
| View | `apps/schedule_web/app/views/guides/_page_5.html.erb` | 39 |
| Stimulus | `apps/schedule_web/app/javascript/controllers/guide_book_controller.js` | 465 |
| Stylesheet | `apps/schedule_web/app/assets/stylesheets/components/_guide_page.css` | 712 |

### Modified — shared files, single-purpose additions only

| File | Change |
|---|---|
| `apps/schedule_web/config/routes.rb` | +3 lines: one comment + `get "guide" => "guides#index", as: :guide`, appended after the `schedule` route. Nothing reordered. |
| `apps/schedule_web/app/assets/stylesheets/application.css` | +3 lines: one comment + `@import url("components/_guide_page.css");` appended after `_schedule.css`. |
| `apps/schedule_web/config/locales/ru.yml` | +~130 lines: one contiguous top-level `guide_page:` block inserted between the existing `guide:` and `floors:` sections, plus its explanatory comment header. No existing key touched. |

### Deliberately NOT created

`app/lib/entities/*`, `app/lib/gateways/portal/*`, a `config/initializers/container.rb`
registration, and a `Resilience::Call` key. Reasoning in §(b).1.

### Deliberately NOT touched

`app/views/shared/_guide_dialog.html.erb` (md5 `2522c96078ea04092cc7ce52c478aa30`,
unmodified), the `guide:` / `floors:` locale sections it depends on, and
`app/views/home/index.html.erb`. Reasoning in §(b).2.

---

## (b) Deviations from the brief / mission doc, with reasoning

### 1. Recipe steps 1–4 (Entity / Gateway / container / Presenter) — skipped, as the brief predicted

Confirmed independently by reading `Guide.tsx`, `guidePages.tsx` and
`schedule_home/guide.html` in full: there is no upstream resource anywhere in this
page. Every string is fixed instructional copy and every table is an illustrative
mockup. An Entity would model nothing, a Gateway would call nothing, and a
`Resilience::Call` key would be a breaker on a circuit that does not exist — which
would also have made `/readyz` report a permanently-untested key. So:

- **No entity, no gateway, no container entry, no breaker key.** Genuinely skippable.
- **`Resilience::Call::KNOWN_KEYS` deliberately untouched**, so `/readyz` stays honest.
- **A presenter *was* written**, but not a `Presenters::Base` subclass. `Presenters::GuidePages`
  wraps no `Result`, so inheriting `data` / `stale?` / `failed?` would advertise
  states this page cannot reach. What it does own is the thing the view must not
  re-derive: the 3-sheet / 6-page physical pairing (front `2i`, back `2i+1`) that
  the flip transform depends on, plus the two-digit numeral formatting. That keeps
  mission §9's "a page's data shape is described in exactly one place" true even
  for a page whose "data" is structural.

**Empirical confirmation that no network call happens** (mission §7 / brief DoD 3):
with `api_geteway` stopped, `/guide` still answers 200 with all six leaflets and
no degraded region:

```
=== stopping api_geteway ===
 Container api_geteway  Stopping
 Container api_geteway  Stopped
guide: 200
6
(should be 6 heads)
0
(should be 0)          <- region-error / stale-banner / translation-missing count
 Container api_geteway  Starting
 Container api_geteway  Started
readyz: 200
```

Playwright also confirms zero non-asset requests — see §(c).3.

### 2. Relationship to `shared/_guide_dialog` — kept fully separate, and its trigger left alone

Read both `app/views/shared/_guide_dialog.html.erb` and
`apps/frontend/src/features/public/GuideDialog.tsx` in full. The brief's ruling is
correct and I followed it: **no shared partial, no shared controller, no shared
stylesheet, no shared locale keys.** The dialog is a three-row floor-picker
(`3 этаж → /group`, `2 этаж → /teacher`, `1 этаж → /room` when signed in) driven by
a `floors:` local; the new page is six leaflets of instructional copy. Zero content
overlap, so there is nothing that could drift.

**Correction to one of the brief's suggestions.** The brief said to verify whether
the dialog's own "Открыть гайд" trigger currently does anything and, *if it is a
dead link/no-op*, wire it to `guide_path`. **It is not a no-op.** In
`app/views/home/index.html.erb` it is:

```erb
<button class="guide-trigger" type="button" aria-label="<%= t("guide.open") %>"
        data-action="dialog#open" data-dialog-name-param="guide">
```

— it opens the floor-picker dialog, and it works (verified in the browser, §(c).8).
So I did **not** rewire it: doing so would have deleted a working feature to make
room for an unrelated one. Consequence, flagged for a human in §(d): **`/guide` is
currently reachable only by typing the URL.** That is exactly the state the React app
is in too — `grep` over `apps/frontend/src/features/public` finds `/guide` only in
`PublicApp.tsx`'s route table and in `PublicLayout.tsx`'s back-button map; nothing
links to it. So this is not a regression, but it is a product gap.

### 3. Locale strings: words go through `t()`, positional numerals are derived

Mission §3 requires UI strings in `ru.yml` via `t()`. Every Russian word, label,
caption and mockup cell on all six leaflets does. Three categories are derived from
structure instead, and I want this called out rather than discovered:

- the two-digit page numerals `01`..`06` in each `.leaflet-page-head`,
- the closed book's range label `01—06`,
- leaflet 01's `01`/`02`/`03` menu marks.

These are positional numbering, not translatable copy. `Presenters::GuidePages`
formats them from each page's index, so a printed numeral cannot disagree with the
page's real position in the book. Likewise, *which* item in leaflet 02's / 05's
selector rows is highlighted is positional (second course, even week, first week
control) and lives in the partial as a named local, not as a duplicate of the
highlighted string in `ru.yml` where it could silently stop matching the list beside
it. Leaflet 06 is the exception: its `added`/`removed` state is what each row *means*
(it must agree with the legend), so that one *is* a value in `ru.yml`.

Decorative glyphs (`?`, `→`, `⌕`, `‹`, `›`) stay as literals in ERB, consistent with
the rest of this app (`×`, `↗`, `‹` are literals in the existing partials).

`ru.yml` is additive and self-contained: several words in `guide_page:` also exist
under `floors:`, `group_picker:`, `schedule.week:` and `schedule.subgroup:` and are
**not** reused from there. Those keys are the live UI's wording; these are a drawing
of the live UI. Sharing them would let a rewording of a real control silently edit
an illustration. This is documented in the comment block above the section.

### 4. **A real defect in the reference, fixed rather than ported**

This is the significant finding of the task.

`.book-sheet` in `schedule_home/guide.css` carries both
`filter: drop-shadow(...)` and `will-change: transform, filter`, two lines below its
own `transform-style: preserve-3d`. Both `filter` and `will-change: filter` are
*grouping* properties: they force the element's **used** `transform-style` to `flat`,
so the declared `preserve-3d` is discarded. With the sheet flattened, its two faces
stop participating in `.book-sheets`' 3D rendering context, and therefore
`backface-visibility: hidden` on them does nothing.

The visible result: after the first turn, the sheet's **front** face stays painted —
mirrored — where its **back** should be. Leaflets 02, 04 and 06 are never reachable;
the visitor sees a mirror image of 01, 03 and 05 instead. Half the guide's content
is inaccessible.

**Verified as inherited, not introduced.** I loaded `schedule_home/guide.html` in the
same Chromium build, pressed ArrowRight, and it reproduces the artifact exactly
(`ref-p2.png` — "Как найти расписание" rendered mirror-reversed on the left page,
with `#bookPageIndex` reading `02—03`). The React port has the same CSS and the same
bug. Mission §2 asks me to report where the visual and behavioural specs disagree;
here they *agree*, and both are wrong, so mission priority 1 (correctness) governs.

**Fix**, documented in-file as deviation 11 at the `.book-sheet` rule:

- `.book-sheet` keeps `preserve-3d` and drops `filter` entirely; `will-change` is
  narrowed to `transform` (the property actually animated every frame).
- The sheet's cast shadow moves onto `.book-sheet-face` as a `box-shadow`, still
  driven by the same `--shadow-x` / `--shadow-y` / `--shadow-blur` custom properties
  the engine already sets. `box-shadow` paints the same shape without grouping.
- `.book-sheet.is-dragging` becomes `.book-sheet.is-dragging .book-sheet-face` with
  the reference's deeper alpha.

Isolated by bisection in the live browser via a constructable stylesheet (see §(c).4 —
`page.addStyleTag` is itself blocked by this app's CSP, which is incidental proof the
CSP is real): removing `filter` alone was **not** enough (`fixA` still mirrored);
`will-change: filter` had to go too (`fixB` correct). Screenshots before/after are in
the scratchpad (`base-spread2.png`, `fixA-spread2.png`, `fixB-spread2.png`).

**Recommendation for a human:** back-port the same two-line change to
`apps/frontend/src/features/public/guide.css` and to `schedule_home/guide.css`. I did
not touch either — `schedule_home/` is the design reference and `apps/frontend` is
outside this task's scope.

### 5. Keyboard and pointer affordances added (mission §11.6 requires them; the reference has none)

The reference book is operable by pointer drag, page-edge click and dblclick only.
None of those is reachable by keyboard or by assistive tech, and mission §11.6 plus
the brief's DoD 2 require every control to be keyboard-reachable with visible focus.
Additions, all documented as deviation 9 in the stylesheet:

- `.book` is `tabindex="0" role="group" aria-live="polite" aria-keyshortcuts="ArrowLeft ArrowRight"`
  with a `:focus-visible` outline.
- Keyboard handling is bound to **the book element, not `window`** (the React port
  binds `window`, which hijacks Arrow keys page-wide while a visitor is trying to
  scroll or move a caret). `Home`/`End` added — the only way to reach leaflet 06
  without six presses.
- Two real `<button class="book-nav">` prev/next controls in the meta row, disabled
  at the ends rather than hidden so the row keeps its shape.

### 6. Progressive-enhancement gating (`is-interactive` / `is-live`)

The reference declares `cursor: grab` and `touch-action: none` on `.book`
unconditionally. Without JS that promises a drag that does not exist, and
`touch-action: none` would additionally kill touch scrolling on a page whose book
cannot be turned. Both are gated behind `.is-interactive`, added by the controller on
`connect()`. Likewise `.book-guide:not(.is-live)` hides the two nav buttons and the
"перетащите край страницы" hint (`display: none`, not `opacity: 0` — invisible-but-
focusable is the trap `_site_header.css` documents for the overlay-state back link),
and collapses the meta grid to match.

Complementing that, `.book-sheet:nth-child(1..3)` get **declarative** z-index
defaults (45/44/43) mirroring the engine's `updateStacking()` at spread 0. The
reference relies entirely on JS for this, so with JS unavailable the last sheet in
DOM order paints on top and the book opens on leaflet 05. Verified: with JS disabled
the book opens on leaflet 01 (`guide-nojs.png`).

`aria-hidden` on the faces is applied **by the controller, not server-side**, on
purpose — with the engine absent no face is hidden and all six leaflets stay in the
accessibility tree, which is the whole point of server-rendering them.

### 7. Shell-integration deviations from `guide.css` (all forced, all documented in-file)

Deviations 1–8 and 10 in the stylesheet header. The load-bearing ones:

- **`body.manual-page { min-height: 100% }` not ported.** `application.css` pins the
  footer with `min-height: 100svh` + flex on body; re-declaring min-height as a
  percentage would collapse that and unpin the footer — landmine #5's failure class.
- **`.book-guide { padding-top: var(--manual-top) }` not ported.** The reference pins
  `.site-header` with `position: fixed`; this app's header is in normal flow, so the
  padding would offset the book twice. `--manual-top` is kept (the `--book-w` clamp
  and `.book-viewport`'s min-height need it) and re-scoped from `:root` to
  `.book-guide`, with its breakpoint moved from the reference's 820px to
  `_site_header.css`'s actual 720px.
- **The reference's `.manual-page .site-header` / `.schedule-back` / `.profile-button` /
  `.contacts-list` overrides dropped** — they restyle other components, which
  `application.css`'s header comment forbids, and the z-index bump existed only
  because the reference header is fixed.
- **`html { scroll-behavior: smooth }` and the reference's global
  `@media (prefers-reduced-motion) { *, *::before, *::after }` reset dropped** as
  out-of-scope for a component file. Reduced motion is honoured by a `.book-guide`-scoped
  block *plus* the controller collapsing its rAF durations — a CSS
  `transition-duration` override cannot slow a rAF animation. Verified in §(c).9.
- **`.book-turn-zone*` not ported** — it styles an element that exists in neither the
  reference markup nor the React port.
- **Six new component-local custom properties** (`--book-paper`, `--book-paper-2`,
  `--paper-edge`, `--paper-edge-lit`, `--book-stack-dark`, `--book-stack-light`,
  `--book-cover-edge`). The book's aged-paper family is genuinely absent from
  `application.css`'s token set. Declared on `.book-guide`, **not** `:root`, so a
  page-specific palette cannot leak into the global tokens. The reference's
  `--book-cover: #4b262f` is byte-identical to `--velvet` and uses the token; every
  other colour is either a token or an `rgba()` reusing a token's exact channels at
  the reference's own alpha — the convention `_teacher_record.css` already documents.

### 8. No wrapper around `<main>` (landmine #6 avoided by construction)

`Guide.tsx` wraps `<main>` in a `<div ref>` purely for React's ref mechanics. As the
brief predicted, Rails needs no wrapper: every target the engine uses lives inside
the book, so `data-controller="guide-book"` sits directly on
`<main class="book-guide">`, matching how `schedules/show.html.erb` scopes its own
controller. Footer stays pinned — measured, §(c).4.

### 9. Landmine compliance

- **#3/#4** — full `docker compose build schedule_web` before every visual check
  (three builds total). Import uses the `url("...")` form; Propshaft's digest rewrite
  and a 200 on the served file are proved in §(c).3.
- **#7** — every conditional attribute is assembled into a hash before the tag is
  emitted (`tag.attributes`): progress dots, `.faculty-mini.is-selected`, `.ms-grid i.on`,
  `.leaflet-week-controls span.is-active`, `.lt-row i.event`, `.change-item.is-*`.
- **#8** — no `%>` inside any `<%# … %>` comment; checked mechanically, §(c).7.
- **#9** — `connect()` returns early, before wiring any listener, if any required
  target is missing or the sheet count is wrong; `beginDrag` bails out **before**
  `preventDefault()` when there is nothing to turn; the nav actions call no
  `preventDefault()` at all.
- **#10** — grepped: `guide.css` uses no `visibility` transition, so the asymmetric-timing
  fix has nothing to apply to here.
- **#11** — `pageshow` (with `event.persisted`) resets sheet progress, the `is-dragging`
  classes and `body.book-dragging`, so a bfcache restore mid-drag cannot leave a
  half-turned sheet or a body stuck in `grabbing`.
- **§8 security** — no inline `<script>`/`<style>`/`style=`/`onclick=`; all state changes
  go through Stimulus `data-action` and CSSOM. No `html_safe`/`raw` on any string:
  the two places that compose markup use `safe_join` + `tag.em`, and the NBSP
  separators are literal U+00A0 characters rather than `&nbsp;` entities precisely so
  no `html_safe` is needed. No `require_account` because nothing here is
  account-scoped, so nothing can leak between accounts. Zero CSP violations measured.

---

## (c) Verification — real output

Environment: Docker up, `schedule_web` rebuilt and recreated after every CSS/JS/ERB
change. `curl` always with `--noproxy '*'`. Playwright 1.62.1, real Chromium.

### 1. Container healthy, `/up` and `/readyz`

```
 milk_schedule-schedule_web  Built
 Container schedule_web  Started
up=200 (try 11)
readyz=200
```

### 2. Route sweep (Playwright, following redirects)

```
===== regression sweep =====
/ -> 200 final=/
/teacher -> 200 final=/teacher
/group -> 200 final=/group
/schedule -> 404 final=/schedule
/teacher/1 -> 200 final=/teacher/1
/room -> 200 final=/?auth=1
/room/1 -> 200 final=/?auth=1
/guide -> 200 final=/guide
sweep console: ["[error] Failed to load resource: the server responded with a status of 404 (Not Found)"]
sweep CSP: []
```

`/room` and `/room/1` still redirect a guest to `/?auth=1`. The single console error
in the sweep is that `/schedule` 404 — `/schedule` with no `group` parameter is the
sibling task's page answering "not found", pre-existing and outside this task; the
error is the document's own status, not a missing asset.

### 3. Assets actually load

Every `/assets/*` URL in the rendered `/guide`:

```
200 /assets/application-83ffb75b.css
200 /assets/application-bfcdf840.js
200 /assets/controllers/application-3affb389.js
200 /assets/controllers/dialog_controller-d13ee4c6.js
200 /assets/controllers/entity_search_controller-7fdf7984.js
200 /assets/controllers/faculty_controller-16d54519.js
200 /assets/controllers/floor_controller-6e4c1be4.js
200 /assets/controllers/guide_book_controller-b2b76876.js
200 /assets/controllers/hello_controller-708796bd.js
200 /assets/controllers/index-ee64e1f1.js
200 /assets/controllers/schedule_controller-9f636332.js
200 /assets/stimulus-loading-1fc53fe7.js
200 /assets/stimulus.min-4b1e420e.js
200 /assets/turbo.min-9fd88cd5.js
```

The new stylesheet arrives through `application.css`'s `@import` chain, so it is not
in the page source. Propshaft rewrote it and it serves 200:

```
32:@import url("/assets/components/_guide_page-e6e2a6c4.css");
...
200 /assets/components/_guide_page-e6e2a6c4.css
```

(all 13 component stylesheets 200; full list in the transcript)

### 4. Real browser pass at 1280 and 375

Identical results at both viewports. 1280 shown; 375 differed only in the numbers.

```
===== /guide @ 1280px =====
console errors/warnings: []
CSP violations: []
failed requests: []
responses >=400: []
non-asset/doc requests (should be none): []      <- brief DoD 3
all requests count: 33
overflow: {"doc":1280,"win":1280,"bodyScroll":1280}
footer: {"found":true,"footerBottom":972,"bodyHeight":972,"viewport":900,"gapBelowFooter":0}
leaflet pages in DOM: 6
```

```
===== /guide @ 375px =====
console errors/warnings: []
CSP violations: []
failed requests: []
responses >=400: []
non-asset/doc requests (should be none): []
all requests count: 33
overflow: {"doc":375,"win":375,"bodyScroll":375}
footer: {"found":true,"footerBottom":812,"bodyHeight":812,"viewport":812,"gapBelowFooter":0}
leaflet pages in DOM: 6
```

`doc scrollWidth == innerWidth` at both sizes → no horizontal page overflow.
`gapBelowFooter: 0` at both → footer is the last thing in the document, correctly
pinned on this page specifically (brief DoD 4).

Incidental proof the CSP is genuinely strict, from the bisection run:

```
page.addStyleTag: Applying inline style violates the following Content Security Policy
directive 'style-src 'self' https://fonts.googleapis.com 'nonce-4NCIzABU4zNDDIVmgUsYeA=='.
```

(I switched the probe to a constructable stylesheet — CSSOM — to work around it.
That also confirms the engine's own `element.style.setProperty` calls are CSSOM and
therefore not CSP-blocked: zero violations across every run.)

### 5. Interaction — every control actually works

```
initial book state:      {"index":"01",   "activeDots":[0],  "prevDisabled":true, "nextDisabled":false,"interactive":true,"visibleFaces":["0"],  "zIndexes":["45","44","43"]}
after Next click:        {"index":"02—03","activeDots":[1,2],"prevDisabled":false,"nextDisabled":false,"interactive":true,"visibleFaces":["1","2"],"zIndexes":["18","44","43"]}
after 2nd Next click:    {"index":"04—05","activeDots":[3,4],"prevDisabled":false,"nextDisabled":false,"interactive":true,"visibleFaces":["3","4"],"zIndexes":["18","19","43"]}
after 3rd Next click:    {"index":"06",   "activeDots":[5],  "prevDisabled":false,"nextDisabled":true, "interactive":true,"visibleFaces":["5"],  "zIndexes":["18","19","20"]}
after Prev click:        {"index":"04—05","activeDots":[3,4],"prevDisabled":false,"nextDisabled":false,...}
after ArrowRight:        {"index":"06",   "activeDots":[5],  ...,"nextDisabled":true}
after End:               {"index":"06",   "activeDots":[5],  ...}
after Home:              {"index":"01",   "activeDots":[0],  "prevDisabled":true, ...}
after drag-to-flip:      {"index":"02—03","activeDots":[1,2],...}
after edge click:        {"index":"04—05","activeDots":[3,4],...}
body still has book-dragging?: false
```

- Page-index readout and progress dots track the visible spread exactly (`01`,
  `02—03`, `04—05`, `06`), and `aria-hidden` bookkeeping matches (`visibleFaces`).
- `prev` disabled only at spread 0, `next` only at the last spread.
- Drag-to-flip **and** page-edge click **and** dblclick **and** the buttons **and**
  Arrow/Page/Home/End all advance the book (brief DoD 2). Every one of the three
  sheets was turned in both directions during the run, not just the first.
- `body.book-dragging` is cleaned up after the drag ends.

Keyboard reachability and visible focus:

```
book focused: {"tag":"DIV","cls":"book is-interactive","outline":"2px solid rgb(75, 38, 47)"}
Home while body focused (should be unchanged - book not focused): {"index":"04—05",...}
focusable order: ["A.back-button","BUTTON.profile-button","DIV.book","BUTTON.book-nav", ...]
```

The book is a tab stop with a real focus ring, its keys are scoped to it (pressing
Home with focus on `body` correctly does nothing), and the nav button follows it in
tab order.

### 6. No `translation missing` anywhere — full six-page pass

Rendered HTML: `grep -c "translation missing\|Translation missing"` → **0**.
And an exhaustive in-container key check, plus proof the dialog's keys survived:

```
pages: [0, 1, 2, 3, 4, 5] numerals: ["01", "02", "03", "04", "05", "06"] range: 01—06
sheets: [[0, 0, 1], [1, 2, 3], [2, 4, 5]]
missing keys: []
guide (dialog) keys: [:title, :close, :floor, :open]
p4 rows: 3 rows, cells/row: [5, 5, 5]
p5 states: ["added", "removed"]
p2 cells: ["", "ПИ-21", "", "ИВТ-31", "", "ИВТ-31", "", "ПИ-21", "", "ЭС-22"]
```

All 66 pieces of expected copy verified present in the server-rendered HTML
(brief DoD 1 and 5) — headings, sections, captions, and every mockup cell:

```
missing copy: NONE
page-head numerals: ['01', '02', '03', '04', '05', '06']
menu marks: ['01', '02', '03']
range label: ['01—06']
page index readout: ['01']
progress dots: 6 total, 1 is-active
nav buttons: 2
change items: ['is-added', 'is-removed']
event cells: 7 | blank lt cells: 18 | ms on cells: 5 | desks: 8
body tag: ['<body data-controller="dialog" class="manual-page">']
main tag: ['<main class="book-guide" data-controller="guide-book">']
stray %>: 0
```

Note `stray %>: 0` and `<main …>` being a single well-formed tag: landmines #7/#8 clear
in the actual output, not just by inspection.

### 7. Landmine spot-checks

```
=== ERB comment blocks containing '%>' (landmine #8) ===
offending comments: NONE

=== landmine #5 guard on _site_footer.css ===
exit=1 (1 = no match = pass)
```

### 8. Existing behaviour intact

```
guide DIALOG after trigger click: {"exists":true,"ariaHidden":"false","visible":"visible",
  "rows":["/group 3 этаж Группа ↗","/teacher 2 этаж Преподаватель ↗"],"bodyModal":true}
guide DIALOG after Escape: {"ariaHidden":"true","visible":"hidden","bodyModal":false}
```

The floor-picker dialog still opens from the home page's `.guide-trigger`, renders
its rows (two for a guest — `Policies::HomeFloors` correctly omits the room floor),
takes the scroll lock, and closes on Escape releasing it. Its file is byte-identical
to before this task (§(a)). `/`, `/teacher`, `/group`, `/teacher/1` and the
`/room*` guest redirect all still behave as before (§(c).2).

### 9. Reduced motion

```
reduced-motion turn settled in ms: 194 index: 02—03
pageerrors: [] CSP: []
is-live present: 1
nav visible: true | hint visible: true
```

A turn that normally animates for 720ms settles effectively instantly, with no
errors and the controls still present.

### 10. JavaScript disabled

```
===== /guide with JavaScript disabled =====
leaflet pages present: 6
aria-hidden faces (should be 0): 0
book has is-interactive class: 0 (expected 0)
```

Screenshot `guide-nojs.png` confirms: the book opens on **leaflet 01** (declarative
z-index defaults working), all six leaflets are in the document and in the
accessibility tree, and the inert nav buttons and drag hint are absent rather than
dead.

### 11. Reference comparison

`ref-p1.png` / `ref-p2.png` are `schedule_home/guide.html` in the same Chromium, and
`guide-1280-p1.png` / `guide-1280-p2.png` are the live page. Leaflet 01 matches the
reference closely (type scale, rules, menu marks, the decorative flow row pinned to
the bottom). Leaflet 02 onward is where the live page is deliberately **better** than
the reference — see §(b).4. Screenshots for all four spreads at both viewports are in
the session scratchpad: `final-{1280,375}-spread{0..3}.png`,
`guide-{1280,375}-{p1,p2,p6,mid-drag}.png`, `guide-nojs.png`,
`guide-reduced-motion.png`, `ref-p1.png`, `ref-p2.png`,
`base-spread2.png`, `fixA-spread2.png`, `fixB-spread2.png`, `fixB-midflip.png`.

---

## (d) Human decisions needed, and what I could not verify

### Needs a human decision

1. **`/guide` has no entry point in the UI.** Reachable only by typing the URL, in
   this app and in the React app alike (§(b).2). The `.guide-trigger` on the home
   page is *not* free — it opens the floor-picker dialog and I left it working. If
   the guide page should be discoverable, someone needs to decide where the link
   goes. The two options I would weigh: a fourth row inside the floor-picker dialog
   (cheap, but it means editing `_guide_dialog`, which the brief ring-fenced), or a
   separate small control in the site footer (touches a shared component). I did not
   pick one, because both are product decisions rather than migration work.

2. **Back-port the backface fix (§(b).4).** The same two-line change should go into
   `apps/frontend/src/features/public/guide.css` and `schedule_home/guide.css`; until
   it does, the React `/guide` and the design reference both hide half their content.
   I did not touch either file — out of scope, and `schedule_home/` is the reference
   of record.

3. **Mobile whitespace is faithful, not accidental.** At 375px the book occupies the
   top ~30% of the viewport with white space down to the footer. That is what the
   reference does (`.book-viewport { min-height: auto }` at ≤820px, with the
   reference's footer fixed over the same gap), so I ported it. Dropping the
   `min-height: auto` override would vertically centre the book and fill the space —
   a genuine improvement, but a design change, so I left it alone.

### Could not verify

- **Real touch and pen input.** Playwright drives synthetic mouse pointer events;
  `pointerType === "pen"/"touch"` paths, `getCoalescedEvents()` batching under real
  touch, and `setPointerCapture` on a touchscreen were not exercised on hardware.
  The code paths are a direct port of already-shipped React logic, but I did not
  observe them.
- **Screen-reader output.** No AT available in this environment. I verified the
  mechanics (`aria-live="polite"` on the book, `aria-hidden` correctly toggled to
  `false`/`true` per visible spread, decorative regions `aria-hidden`, all six
  leaflets in the tree when JS is absent) but not what NVDA/JAWS/VoiceOver actually
  announce on a turn. `aria-live` on a container holding all six pages is inherited
  from the reference and could plausibly be chatty.
- **Mission §11.5's degraded-state protocol is not applicable** and I did not fake
  it: this page has no upstream, so there is no staleness banner or region error to
  produce. What I did verify instead is stronger for this page — `/guide` serves 200
  with complete content while `api_geteway` is stopped (§(b).1).
- **Firefox and Safari.** Chromium only. The fix in §(b).4 depends on
  `backface-visibility` interacting with grouping properties, which is spec'd
  behaviour but historically inconsistent; WebKit in particular deserves a look.

### Shared-file edits — explicit confirmation

I re-read each shared file immediately before editing it and re-checked all three
after finishing. **`ru.yml` had in fact been modified on disk by the sibling task
between my read and my edit** (the Edit tool reported it); my insertion applied
cleanly on top and clobbered nothing.

- **`config/routes.rb`** — clean. One comment + one `get "guide" …` line appended
  after the existing `schedule` route. Full file re-read at the end: all 12 pre-existing
  routes present, in their original order, with their original comments.
- **`app/assets/stylesheets/application.css`** — clean. One comment + one
  `@import url("components/_guide_page.css");` appended after `_schedule.css`. All
  12 pre-existing imports present and in their original order, including the
  "shared before the pages that place it" comment and its ordering guarantee.
- **`config/locales/ru.yml`** — clean. One contiguous top-level `guide_page:` block
  between `guide:` and `floors:`; `grep '^  guide_page:\|^  guide:'` finds exactly
  one of each (lines 65 and 100). Parses as valid YAML. All 19 top-level sections
  intact. The sibling's `/schedule` work is intact and *visible* in the final state —
  `schedule:` now carries `out_of_period`, a key that did **not** exist when I first
  read the file, so the sibling's addition survived my edit:

```
top-level: ['app','auth','contacts','cookies','date','degraded','entity_search','floors',
            'group_picker','guide','guide_page','home','nav','record_sheet','room_search',
            'schedule','teacher_search','time']
schedule subkeys: ['controls_aria','dash','day_empty','day_selector_aria','empty','event',
            'glance','legend','mobile_aria','not_found','out_of_period','page_aria','subgroup',
            'subgroup_selector_aria','table_aria','time_header','transfers','week',
            'week_selector_aria','weekdays_full','weekdays_short']
guide (dialog) keys: [:title, :close, :floor, :open]
```

I touched none of the sibling's files (`_schedule.css`, `_schedule_selector.css`,
`presenters/group_schedule.rb`, `app/views/schedules/*`).

### Git

No `git add` and no `git commit` were run anywhere in this repository. All work is
uncommitted working-tree state. `apps/schedule_web` still has zero commits on
`master`, as expected.
