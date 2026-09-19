# Administrator browser audit — 2026-09-13

This is the current rendered baseline, captured through Playwright MCP against `http://localhost:3000`, approximately 15:19–15:27 Europe/Moscow. It supplements the earlier [source audit](AUDIT.md) and supersedes its current-state claims where explicitly stated below. The current request is documentation only. No application source, accounts, reservations, assignments or lessons were changed during this audit.

## Evidence boundary and runtime

Normal sign-in succeeded with the user-supplied administrator account. The Ruby account-creation snippet was not executed. Credentials are deliberately absent from this package. The observed role was super administrator; other roles were not verified.

Desktop inspections used 1280×900. Accounts and Workload were additionally inspected at 390px width. Screenshots, rendered DOM, computed styles, visible navigation and read-only editor opening informed this report. No create/save/delete, generated-password operation, failure injection, drag mutation, concurrency check or authoritative mutation read-back was performed. B00's complete functional anchor remains pending. No 320px, 768px, wide-desktop or print acceptance is claimed.

Rails was served by Docker container `schedule_web` (`decced7b71d4`), image `milk_schedule-schedule_web`, image SHA `5df274f28c303b360f78928626a0946a7194dd1e8197ef690a69029fba69ed7b`, published on port 3000 with no application bind mounts. Host/container SHA-256 matched for these sampled files:

| Rails file | SHA-256 |
| --- | --- |
| `app/assets/stylesheets/components/_admin_tokens.css` | `f8397a3b1e81bd204c4f213c9787a64845683c36df1d708e9d0d67d4986bfb96` |
| `app/assets/stylesheets/components/_admin_components.css` | `816e9c3d5c6e62f27a1b7ac917697202092e731a886a978d96c15a491fde1c3a` |
| `app/controllers/admin/accounts_controller.rb` | `59f9c798ca6c06cbec4ad660b853ad5aae2dbf969a9ed0b575c965c44618a933` |
| `app/views/admin/accounts/index.html.erb` | `7b8a8c54bd8bce15186433c11f4554ef759aa3bff6ee44c5c1767966e542d278` |

This is partial provenance, not a proof of every Rails file or the running API revision. All three repositories already contained changes at the start of this continuation; earlier statements that Rails was clean describe the initial source audit only.

The browser loaded `application-dba5515f.css` and the fingerprinted admin token, component, workspace, teacher, workload, resource, calendar, account and notice stylesheets with HTTP 200. Representative fingerprints: `_admin_tokens-82e9d7e5.css`, `_admin_components-fd6aa0bf.css`, `_admin_resources-f730a18d.css`, `_admin_calendar-984cb112.css`, `_admin_accounts-207fc5d3.css`. Manrope and Unbounded font resources loaded; browser font checks passed. The retrieved Schedule console output contained two stylesheet-preload warnings and no errors. This is not a whole-application console clearance.

**Conclusion:** visible inconsistencies persist with the new assets loaded. Missing CSS and missing font downloads are not an adequate explanation for the sampled pages. Investigate selector coverage, cascade and composition before adding more stylesheet layers.

## Preserve these verified improvements

- Admin token/component/page files now exist and load. Reuse them. The original undefined-token finding is historical; inspect remaining references individually rather than recreating the token system.
- Workload at 390px keeps a native five-column table: 850px minimum width inside a roughly 345px scrolling region, `overflow: auto`, with the Russian label “Таблица нагрузки с горизонтальной прокруткой”. Its old mobile-card conversion is resolved in this sample.
- Resource navigation now includes a shared, broader destination list. Its wrapping and hierarchy still need work; it is not simply missing navigation.
- The sampled Accounts teacher row displays a readable person name. Its edit form contains teacher and department searches and retained selection fields. Old claims that this editor only exposes raw IDs are obsolete. Search success, selection retention and writes remain unverified.
- The selected Schedule grid retains the time column and Monday–Saturday columns. An empty selected group is a valid empty state, not proof of failed scheduling.
- Loaded Workload data is available in an older semester. An empty current-semester scope must not be generalized to an unavailable workload service.

## Measured inconsistencies

Measurements are CSS pixels from the sampled state, not target values. Targets are in [DESIGN_SYSTEM.md](DESIGN_SYSTEM.md).

| Surface | Observed | Consequence |
| --- | --- | --- |
| Schedule / Workload / Notice heading | Unbounded approximately 21.76px / 26.112px | Shared header class does not produce the prescribed 24px / 32px |
| Resources / Calendar / Accounts heading | Unbounded 24px / 32px desktop | Useful shared size already present, but surrounding composition differs |
| Teachers heading | 64px, uppercase | Disproportionate to other administrator workspaces |
| Schedule faculty search | 34px control, 11px text | Smaller than common 40px controls and 14px form text |
| Schedule “Открыть расписание” | 30px control, 10px text | Essential action is undersized |
| Resource create/edit controls | Many raw 28px controls, 16px text | Font/control coverage remains inconsistent |
| Calendar faculty/semester selects | 34px control, 10px text | Essential context difficult to read |
| Accounts form controls | Mix of roughly 20px search input, 28px button, 26px role select; 10px/16px text | Shared control rules do not reach all fields |
| Populated Workload table | 11px text; begins around y=764 | Main work surface is delayed and undersized |
| Selected Schedule grid | Begins around y=695; 780px table in about 497px center region | Context and two side panels consume working space |
| Narrow shared shell | About 345px before main content in sampled Accounts/Workload | Expanded monitoring/navigation consumes the first screen |

Do not fix these by scaling the whole page or shrinking table columns. Fix the owning rules and context/editor layout.

## Findings and prescribed response

### V01 — shared styling exists but does not consistently reach controls

Loaded primitives coexist with page-specific rules and uncovered forms. Posts still looks like a raw browser form. Calendar has tiny selects; Accounts mixes font/control sizes. Header widths also differ: the Resources/Calendar header block stops around 720px while the content continues wider.

**Response:** map each visible control to one explicit shared field/button role, remove or narrow competing page declarations, and compare computed values after deployment. Use existing token names when semantically suitable; absence of a newly proposed alias is not itself a defect. Keep domain geometry in page CSS. Acceptance requires screenshots and measurements, not merely imported files.

### V02 — context choice consumes the workbench and can be ambiguous

Schedule exposes 19 semester choices. Workload shows 23 faculty chips in the sampled state, including indistinguishable repeated abbreviations, and orders semester history differently. Calendar uses faculty/semester selects with 25/20 options including placeholders. Group labels differ between Schedule and Calendar. Fixtures include odd or duplicate labels; no data cleanup was performed or authorized.

**Response:** one readable context summary after selection; a deliberate “Изменить контекст” control opens bounded searchable choices. Show the selected semester and a few adjacent choices with a reachable history panel. Use full faculty names or an abbreviation plus a distinguishing secondary line. Use the same group label formatter with course/year context. Preserve transport identity types. Collapse optional source/inspector panels when their combined width leaves an unusably small grid. Keep time/day columns intact.

### V03 — Schedule frame selection is not represented by the page URL

Reproduction, without writes:

1. Open `/admin/schedule?faculty_id=1&semester_id=9`.
2. Select `МК-3` and choose “Открыть расписание”.
3. The grid appears and `#schedule-shell.src` becomes `/admin/schedule?semester_id=9&faculty_id=1&groups%5B%5D=789`.
4. The browser address remains `/admin/schedule?faculty_id=1&semester_id=9`.
5. Navigate afresh to that unchanged address: no table, zero selected checkboxes, and “Выберите семестр, факультет и группы”.

This contradicts the page's statement that context is stored in its address. A fresh navigation to the displayed URL was tested; browser Back and native reload were not separately exercised.

**Response:** make committed context navigation update browser history using the existing Turbo conventions. Preserve group array, semester, faculty and applicable period/mode. Querying a picker must not commit a different context. Verify reload, copied URL, Back/Forward and multi-group selection. Resolve dirty drafts before applying scope changes. Do not make cosmetic context chips a substitute for URL correctness.

### V04 — Workload has a usable dense table, but editor and context need repair

The faculty-mode URL `/admin/workload?faculty_id=1&mode=faculty&page=1&period=semester&semester_id=1` showed 94 records, 30 on the first page. Its five headers are `Кафедра`, `Дисциплина и группа`, `Вид занятия`, `Часы`, `Назначение`. Current-semester mechanical faculty was empty. Department mode was opened, but its complete operation journey was not checked.

Read-only opening of record 8603 showed a fixed 520px section without dialog role/`aria-modal`; focus remained on BODY at the observation point. Keyboard trapping was not tested. The form presents 25 activity-type radios in a scrolling region, including fixture labels; bounded scrolling exists but does not provide search. Copy says the original subject is retained while an action offers “Найти другую дисциплину”.

**Response:** retain the table and its verified narrow scroll region. Finish a real accessible side sheet, with bounded body scrolling and reachable footer. Use a bounded searched list when the actual activity catalog is large; do not assume activity type is a small enumeration. Resolve the contradictory subject affordance against the write contract before rewriting copy or allowing changes. Do not remove fixture records to simplify the screenshot.

### V05 — Resources mixes creation and row editing with broken track alignment

`/admin/posts` has three logical columns. Header and inline row forms do not align; fields for a row's short name sit under the wrong visual track. Create controls are permanently above the list. Resource navigation wraps to three lines. The rendered page also contains two main landmarks.

**Response:** list-first composition, a compact create action and shared editor outside table row geometry. Define each resource's actual column tracks explicitly; do not reuse a generic three-column grid for all resources. Keep all fields and table columns. Use one main landmark, a labelled resource subnavigation and local table scrolling. Inspect Rooms and Subjects before accepting the whole Resources batch; Posts is only a representative anti-reference.

### V06 — Calendar needs five distinct layouts within one shared shell

- `/admin/operations?faculty_id=1&semester_id=9&group_id=789`: four period date forms in two columns, all unset in this scope. Keep their domain distinctions. The context is bulky and selects are tiny.
- `/admin/weekday_transfers`: five logical columns (date, followed weekday, group, reason, actions) wrap across a generic three-track grid. The screenshot confirms misalignment. A raw numeric `weekday_transfer[student_group_id]` field remains visible.
- `/admin/holidays`: three logical columns; two fixture rows and raw form controls. An independent date-based rule, not a recurring-schedule move.
- `/admin/operations/semesters`: three logical columns; 19 rows. The two-option semester number select is appropriate; the whole semester catalog should not become a long dropdown in other workspaces.
- `/admin/room_reservations`: a visible error says “Данные бронирований, аудиторий или временных слотов временно недоступны.” No table or editor is rendered in this state. The failing dependency and root cause are not established.

**Response:** preserve individual schemas and period meanings; use searched group context with the base-group identity for weekday transfers. Diagnose reservation reads narrowly before accepting reservation picker work: trace the actual controller dependencies, gateway results and API actions. A 200 HTML error page or a source-complete picker is not a usable reservation flow. Do not fabricate empty rows, fall back to ID entry or redesign API infrastructure without evidence.

### V07 — Accounts has better selectors inside an unusable row editor

The six-column account list is still present, and a sampled teacher label is readable. Opening edit displays teacher/department searches and safe ordinary fields. The absolute editor overlays the table, row role/status labels visually collide, and the lower save controls are outside the initial viewport. The permanently expanded create form pushes the list toward y=686. Two main landmarks are rendered.

At 390px, the table is approximately 900px wide inside the page's local overflow; no whole-document horizontal overflow was observed. This is not permission to remove columns. The page heading stays 24px, rather than the prescribed narrow 20px.

**Response:** preserve the new search/retained-selection work, move create/edit into the common sheet outside table geometry, and reserve a stable action cell. Show role-relevant help/fields only; preserve the contract's scope semantics when roles change. Verify existing safe-draft controller work through 422/409/network behavior rather than assuming it is absent or complete. Never retain password input or capture generated secrets. Audit password-related copy against the actual one-time credential flow without invoking it for visual inspection.

### V08 — adjacent pages and the narrow shell still need consistency

Teachers retains valuable visible search and constraints concepts, but its 64px heading and oversized black search panel should not become shared sizing. Notice retains its report-oriented empty state and teacher/semester controls. Only initial states were inspected; populated reports, print and teacher constraints remain pending.

On sampled narrow pages, always-expanded Grafana/Prometheus/Loki links and home navigation consume significant height. Active navigation can sit outside the initially visible horizontal strip.

**Response:** preserve dark identity and all destinations; put secondary monitoring links in an accessible collapsed disclosure on narrow screens and ensure the active destination is visible. Keep primary navigation reachable without a hover interaction. Normalize heading/control roles on Teachers and Notice without replacing their domain layouts.

## Screenshot index

These are baseline evidence, not desired mockups. Some include local fixture names and email addresses; they contain no deliberately captured passwords or tokens. Do not publish externally without a separate review.

| Evidence | State |
| --- | --- |
| [Schedule context](evidence/schedule-context-1280.png) | Initial selection workspace |
| [Schedule grid](evidence/schedule-grid-1280.png) | Selected empty group; full-page capture |
| [Workload](evidence/workload-1280.png) | Populated older semester |
| [Workload narrow](evidence/workload-390.png) | Shell/context viewport; table geometry separately inspected in DOM |
| [Workload editor](evidence/workload-editor-1280.png) | Existing requirement opened without saving |
| [Posts](evidence/resources-posts-1280.png) | Resource header/forms/row alignment |
| [Calendar](evidence/calendar-1280.png) | Initial period/context state |
| [Selected periods](evidence/calendar-periods-1280.png) | Four unset period forms |
| [Weekday transfers](evidence/weekday-transfers-1280.png) | Five-column content wrapping through wrong tracks |
| [Reservations](evidence/reservations-1280.png) | Unavailable-data state |
| [Holidays](evidence/holidays-1280.png) | Date-rule table |
| [Semesters](evidence/semesters-1280.png) | Semester catalog |
| [Accounts](evidence/accounts-1280.png) | Create block and six-column list |
| [Accounts narrow](evidence/accounts-390.png) | Shell and form density |
| [Account editor](evidence/account-editor-1280.png) | Overlapping row editor |
| [Teachers](evidence/teachers-1280.png) | Initial search workspace |
| [Notice](evidence/notice-1280.png) | Initial report selection |

## Next implementation gate

Use [BROWSER_BASED_CONTINUATION_PROMPT.md](BROWSER_BASED_CONTINUATION_PROMPT.md). Reconcile the dirty source with this snapshot, finish the functional B00 anchor, then close B01/B02 before expanding other pages. The historical handoff contains source-complete claims across later batches; none overrides the remaining rendered defects or missing write/failure acceptance recorded here.
