# Administrator workbench design system

Status: prescribed implementation target, 2026-09-13. Derived from the incumbent Schedule/Workload identity and the user's constraints. Values below are decisions for future implementation, not a claim that the current app already uses them. Source audit and rendered evidence limits are in [AUDIT.md](AUDIT.md).

Current rendered evidence: [BROWSER_AUDIT.md](BROWSER_AUDIT.md). Its measurements describe actual defects; the decisions below describe the target. Existing token/component/page files now load, so extend the current system instead of recreating it. Prefer existing equivalent semantic aliases over renaming tokens merely to match this document's proposed vocabulary.

## Browser-derived implementation decisions

These decisions refine the original target and take precedence over stale assumptions about missing files or unstyled assets.

1. **Compact committed context.** After selection, show one summary band containing readable faculty, semester and group/department labels, with “Изменить контекст”. Expand bounded choices on demand. Keep the selected semester plus up to three adjacent choices visible; expose history through a labelled panel with search/year navigation. Never place all 19 semesters or 23 ambiguous faculty abbreviations above the main task. Keep identifiers hidden and labels unambiguous. Applying context updates the browser URL; searching does not.
2. **Working-space budget.** At 1280×900, target the first table header/grid row within the first 400px from the viewport top after context is committed and no editor is expanded. This is a design acceptance target, not a measured current value. Schedule currently starts around 695px and populated Workload around 764px. Reduce expanded context and redundant vertical chrome; do not shrink controls or table cells to meet the budget. If content forces an exception, record the exact state and reason.
3. **Responsive planner panels.** Keep the timetable's preserved minimum geometry. When source + inspector + grid cannot fit, collapse one secondary panel into an explicitly labelled, keyboard-operable panel; keep the selected workload and destination summary visible. At narrow widths use a sequential select/destination/edit flow with a scrollable unchanged table. Do not require simultaneous desktop sidebars at the cost of an unusable center.
4. **One shell, restrained secondary navigation.** Shared headers span the workspace rather than ending at an arbitrary 720px while tables continue wider. At narrow widths, place monitoring destinations in a closed “Мониторинг” disclosure and keep the active primary navigation item visible. Retain all destinations and dark sidebar identity. Aim for navigation before main content to occupy at most 160px at 390px width with secondary disclosures closed; verify actual labels rather than clipping them.
5. **Explicit control coverage.** Map every visible text/search/date/number/password input, select, textarea and button to shared visual roles. Default form text is 14px/20px, default controls 40px, compact row actions 36px; 10px text is not acceptable for context or primary actions. Page heading is 24px/32px desktop and 20px/28px narrow, including Teachers. Keep its search/constraints character but remove the 64px heading exception.
6. **Table-first CRUD.** Resources and Accounts show the existing table, filter/context and one create action before any new-record form. Open create/edit in the shared side sheet outside table geometry. Each schema owns its header/row tracks. Weekday transfers has five logical columns; a generic three-track grid is prohibited. Keep one main landmark. Do not add or remove columns to fit action controls.
7. **Real editor behavior.** Modal sheets require labelled dialog semantics, managed initial focus, contained keyboard focus, inert background, Escape/dirty-draft handling, focus return and one scroll-lock owner. Body scrolls while title and Save/Cancel remain reachable. A fixed section or an absolute action-cell popover is not completion. Schedule remains nonmodal. Reuse new account picker/draft work when changing the shell.
8. **Cardinality overrides field stereotypes.** The sampled workload activity catalog contains 25 entries. Use a bounded searchable radio/selection list when an activity catalog exceeds eight choices; keep two to four frequent choices as visible segments only when those are the complete appropriate set. Never delete fixture entries or hide real options to make an enum appear small. Two semester-number choices and stable role/status enums can remain native selects.
9. **Read errors are a distinct state.** Reservations currently fails before rendering its working table. Preserve context and provide safe Russian feedback/retry once the failing dependency is understood. Do not present a failed read as an empty catalog, expose numeric ID fallbacks, or claim picker acceptance from source alone.

For all targets, compare computed styles and screenshots with loaded asset provenance. Preserve browser-verified improvements (especially Workload local table scrolling and Accounts readable/searchable identity fields). A file import or a green syntax check is insufficient acceptance.

## 1. Product intent

The operator is a dispatcher or scoped academic administrator repeatedly distributing teaching load and scheduling classes, often under semester-start pressure. The interface should feel precise, readable and stable during long sessions. The timetable/table is the primary work surface; navigation, context and editing tools support it.

Domain vocabulary: semester, group, stream, teaching-load requirement, department membership, week parity, time slot, remaining hours, period and dated transfer. The signature interaction is moving a labelled workload item toward a timetable cell while both remaining hours and destination context stay visible.

Retain the existing color world: white paper, warm work surface, dark blue-black navigation, ink text, velvet selection, almond emphasis. Avoid a generic blue dashboard, metric-card wall, colorful Kanban board or oversized promotional headings. These choices follow the existing product, not a new brand exercise.

## 2. Fixed structural constraints

Never change table column count/order, header hierarchy, row grouping, merged cells, axes, totals, cell meaning or print-report semantics. Current structures are the minimum preservation baseline because the local specification does not provide an explicit schema appendix. If another authoritative specification is supplied later, compare and escalate a concrete discrepancy; do not silently redesign it.

Allowed: typography, alignment, wrapping, padding, row selection, focus, scroll containment, sticky headers/first columns where they do not obscure data, accessible cell markup and placement of editing tools outside the table. Do not add a drag column: place the handle inside the existing identity/action cell. Do not make rows reorderable without a real domain ordering operation.

The schedule retains time rows and day columns. Workload retains its current five columns: `Кафедра`, `Дисциплина и группа`, `Вид занятия`, `Часы`, `Назначение`. Notice retains `Число`, `Часы`, `Поток, группа`, `Вид занятий`, `Аудитория`. Accounts retains Email, role, status, teacher, scope and actions. Existing resource/calendar tables retain their own schemas; do not force all into three tracks.

## 3. Tokens and exact visual roles

Maintain admin tokens scoped to `.admin-body` in the existing `_admin_tokens.css`, imported before admin component/page CSS. Reuse root primitives through aliases. The table below specifies semantic roles; existing equivalent alias names may remain. Do not change public token meanings to fix admin problems.

| Token | Target | Meaning |
| --- | --- | --- |
| `--admin-text` | `var(--ink)` = `#151311` | Main content |
| `--admin-muted` | `#6d6a66` | Supporting copy; existing value |
| `--admin-canvas` | `#f7f6f3` | Existing warm workspace |
| `--admin-surface` | `var(--paper)` | Tables, panels, editors |
| `--admin-sidebar` | `#121a20` | Existing navigation |
| `--admin-action` | `var(--velvet)` = `#4B262F` | Primary actions and focused work |
| `--admin-nav-active` | `#8f1f36` | Existing active sidebar accent |
| `--admin-selection` | `rgba(238,211,186,.30)` | Selected record; must include non-color indication |
| `--admin-border` | `rgba(21,19,17,.18)` | Routine table/control boundary |
| `--admin-border-strong` | `var(--line-soft)` | Section/control emphasis |
| `--admin-focus` | `var(--velvet)` | Focus ring on light surface |
| `--admin-focus-on-dark` | `var(--almond)` | Focus ring on navigation/primary dark surface |
| `--admin-danger` | `var(--danger)` = `#B4392F` | Destructive action / error text |
| `--admin-success` | `#27643c` | Confirmed saved/available state |
| `--admin-warning` | `#805015` | Needs attention; always labelled |
| `--admin-control-bg` | `var(--paper)` | Form controls |
| `--admin-disabled-bg` | `#efeeeb` | Disabled control surface |
| `--admin-font-heading` | `var(--font-display)` | Unbounded |
| `--admin-font-body` | `var(--font-ui)` | Manrope |

New semantic colors are implementation targets. Verify text/background contrast in actual rendered combinations. Do not use root faint/placeholder colors for essential table data. Disabled is a labelled state, not a low-opacity substitute for explaining why an action is unavailable.

| Role | Target |
| --- | --- |
| Page heading | Unbounded 600, 24px/32px desktop; 20px/28px narrow; -0.02em tracking |
| Section heading / editor title | Manrope 700, 16px/24px |
| Body / label / form input | Manrope 500 (body 400 permitted), 14px/20px |
| Table content | Manrope 500, 13px/18px; content-driven row height |
| Table header | Manrope 700, 12px/16px; sentence case unless an established short label requires capitals |
| Supporting text | Manrope 400/500, 12px/18px |
| Kicker / small metadata | Manrope 700, 11px/16px; tracking at most .06em |
| Hours, dates and counters | Tabular numerals; preserve units and date meaning |

Do not put operational text below 11px. Do not use Unbounded for inputs, dense tables or long labels. Existing public typography is out of scope.

Spacing scale: 4, 8, 12, 16, 24, 32px. Table cells: start with 12px inline / 10px block; timetable geometry may retain existing padding where larger padding would distort the required grid. Main gutter: 24px desktop, 16px narrow. Panel/editor padding: 20px; related form fields 12px apart; field groups 24px apart. These are explicit exceptions to the basic scale, inherited from the current compact panel treatment.

Default controls: 40px minimum height; compact table buttons 36px; primary touch actions 44px. Icons: 16 or 20px, existing stroke style; icon-only controls require an accessible Russian name. Radius: tables/sheets 0, controls 4px, small panels 6px. Borders establish ordinary depth. Use one subtle shadow for an overlay only; no decorative card shadows on every surface.

## 4. Shared page grammar

1. Persistent role-aware sidebar.
2. Shared 82px-minimum page header: optional short kicker, page title, one task action. Height can grow for wrapping; never clip a long title.
3. Compact context strip immediately below the header where the task needs scope.
4. Local toolbar: search/filter/mode/pagination context for the current table.
5. Primary table/timetable and optional source/inspector regions.
6. Local feedback within the active frame/editor; no detached success banner above an unseen frame.

Do not insert all six pieces on pages that do not need them. Accounts does not need semester context. Notice needs teacher/semester/period rather than faculty/group. A common visual grammar does not mean shared irrelevant fields.

Context changes preserve valid parent values and clear invalid descendants atomically. Changing semester clears group-semester selection and the selected placement/load/event when incompatible. Changing a base person does not retain a membership from the previous person. Keep URL-backed selection and browser Back behavior, but never place a draft password or access token in the URL.

## 5. Selection matrix — make choices visible

| Choice | Required control | Notes |
| --- | --- | --- |
| Teacher/person | Search input + visible result list + selected summary | Name plus department/context; person ID for accounts/reservations |
| Teacher assignment | Search/filter within authorized memberships + checkboxes + selected chips | Membership ID for workload/event operations; do not substitute person ID |
| Subject / room / department / group / specialty | Searchable visible list; multi-select checkboxes where contract allows | No long native select, even with small fixtures |
| Faculty | Short visible list when <=8 authorized options; search/filter above it when larger | Count reflects full authorized collection, not a truncated first page |
| Semester | Visible current/recent choices; expanded searchable history when long | Keep academic year visible; no arbitrary limit that hides valid history |
| Period kind | Four visible links/segments with configured dates/status | Actual values: semester, lecture_week, credit_week, exam_session |
| Week parity / workload mode | Two–four visible links/radios/segments | Use links for navigation and radios for a form value |
| Account role | Native select permitted | Six stable choices, permission-filtered |
| Account status | Native select or two radios | Keep explicit save; no accidental activation on hover/drag |
| Weekday for dated rule | Native select permitted | Six stable weekdays; includes visible label |
| Time slot within editor | Visible short list or native select if <=8 | Show pair number and time, not an ID |
| Building/room/activity type | Native select only when <=8 complete choices and not a large identity catalog | Otherwise visible filtered list; preserve existing domain values |
| Date / time / numeric hours | Labelled native input | Keep date semantics; no custom date engine for this project |

The eight-option boundary is a product design rule, not an accessibility standard. A frequently compared set can still benefit from visible options below that count. Groups/teachers/rooms/subjects remain searchable regardless of fixture count.

Prefer ordinary labelled input plus list of buttons/links over a bespoke combobox. A popover that resembles a select still hides the choices; do not rename a dropdown and claim compliance with the user's preference. For a long set, a dedicated search panel can open while selected values remain visible outside it.

Search states: untouched hint, too-short query, loading, matching results, successful empty result, failed request, selected result. Existing shared admin query minimum is three characters and maximum 120. Preserve the current 500ms debounce initially. No search request per drag event. Explicit search submit must work without JavaScript. New search routes must use real authorized API capabilities.

Selected chips are labels with explicit remove buttons. Show the complete label on focus/expansion if truncated. Search updates must never remove selected items just because they are absent from the new result page.

## 6. Drag-and-drop contract

### Supported scope and rollout

Required: workload item -> schedule slot; teacher membership -> workload assignment; room -> workload assignment; existing lesson -> another slot through explicit edit staging. Existing lesson drag lands after its click-edit equivalent is proven. No drag for account permissions, resource ordering or printed reports. Calendar range-resizing, group bulk provisioning and multi-cell availability painting are deferred.

### State machine

`idle -> source selected -> dragging (optional) -> destination selected -> staged preview -> saving -> saved`

Alternative exits: Escape/cancel -> idle with original data; invalid target -> source remains selected; validation/conflict -> staged preview with error; uncertain network outcome -> reconciliation/read-back before retry. Switching context invalidates the drag payload, not the saved record.

### Source, target and feedback

- A handle inside the existing identity/action cell says `Перетащить` to assistive technology. Clicking the row's ordinary open/edit control must remain ordinary navigation.
- Source shows subject/activity, group context and remaining hours for a load; person+department for membership; room+building for a room; event identity/date/week context for a lesson.
- Original item stays in place during drag. Ghost is compact, plain and readable; no card rotation or large scale effect.
- Only same-operation/context targets highlight. Highlight means `Выбрать это место`, not “guaranteed available.” Show a textual destination preview and validate with the API.
- Use distinct payload types for requirement, membership, room and event. Include stable identifiers and current semantic context; never access tokens, whole API objects or trusted display labels. Reject arbitrary text/plain drops.
- Dragover does no network I/O and no repeated DOM geometry reads/writes. Cleanup hover/source states on dragend, Escape, drop, disconnect and Turbo navigation. Edge scrolling must be bounded to the intended source/table region; use animation-frame scheduling if needed and stop it on exit.

### Staging and save semantics

For workload -> slot, open the same placement editor as click selection. Display group, period, weekday/date, pair and source requirement. Preserve assigned teachers and optional room behavior. Save invokes the existing placement/create operation.

For membership/room -> assignment, open/update the assignment editor and add the dropped item to the retained selected set. Show `Добавлен в выбор` until Save; do not display `Назначено` before persistence. Duplicate drops are no-ops with readable feedback. Removing a selected item requires its explicit remove control. Save sends the complete selection to atomic replacement.

For lesson -> slot, stage the existing event PATCH and keep the original until confirmed. Label recurring edits `Изменить регулярное занятие`. A one-date change has a separate `Перенести на дату` operation and date preview. Occupied targets never automatically swap; `Обменять занятия` is explicit and uses the swap endpoint. Do not change load requirement or silently move all stream siblings.

Save disables duplicate local submission, reports `Сохранение…`, and completes only on server success. Reload authoritative data before declaring the final visual state. If the write may have succeeded but response/read-back failed, say `Не удалось подтвердить сохранение` and reconcile; do not automatically POST again.

### Equal non-drag operation

The user can select an item with click/tap or Enter, choose a destination with click/tap or keyboard-reachable controls, inspect the same preview and Save. Escape cancels the selection. Announce the selected source, chosen destination and save result using a restrained live region. No custom ARIA grid keyboard model unless the complete model is implemented; native table links/buttons are sufficient for the initial path.

Dragging needs a single-pointer alternative as well as keyboard access; keyboard-only fallback is insufficient for people who use a pointer but cannot drag accurately. See [W3C dragging movements guidance](https://www.w3.org/WAI/WCAG22/Understanding/dragging-movements.html).

## 7. Editors, drafts and errors

Schedule inspector is nonmodal so the timetable can remain a destination. Workload/Resources/Accounts use one common side-sheet treatment for multi-field edits, approximately 480px wide, up to 520px for complex assignment; width never exceeds viewport. Calendar period date forms stay inline with their existing four kinds.

On desktop, a side sheet may overlay only when explicitly modal; do not use a dim backdrop while leaving the page interactive. On narrow screens, the sheet becomes full width with its own visible Close/Save controls. Use native dialog semantics where suitable and the [W3C modal dialog pattern](https://www.w3.org/WAI/ARIA/apg/patterns/dialog-modal/) for focus containment and return. Never declare aria-modal without actually enforcing modality.

Sheet structure: labelled heading, selected record/context summary, field groups, error summary when needed, inline field errors, Save/Cancel footer. Do not nest sheets for entity lookup; put the lookup inside the editor or switch its body while retaining draft. Do not replace the entire draft form when only search results need updating.

Dirty state is explicit. Save/Discard/Continue editing is offered when closing or changing destination/context would lose a draft. Search and validation do not prompt. Browser reload/navigation warnings are scoped to dirty editors; do not install a universal navigation blocker. Never persist sensitive drafts in localStorage/sessionStorage or expose them in URLs.

| State | Required UI behavior / Russian copy example |
| --- | --- |
| Empty successful list | `Записей пока нет` plus relevant create action |
| Search no match | `Ничего не найдено` plus refine/reset action |
| Missing context | `Выберите группу и семестр` with actionable controls |
| Missing period | `Период не настроен` plus scoped calendar link |
| Validation | `Проверьте выделенные поля`; retain safe values and focus error/first invalid field |
| Conflict | `Место уже занято. Выберите другое время`; retain draft/original |
| Forbidden | `Недостаточно прав для этого действия`; preserve nonsensitive context, do not offer an invalid retry |
| Deleted selected record | `Запись больше недоступна`; return to current filtered list |
| Read/network failure | `Не удалось загрузить данные. Повторить`; never disguise as empty |
| Ambiguous write | `Не удалось подтвердить сохранение. Обновите данные перед повтором` |
| Success | `Изменения сохранены`; keep selected record visible |

These are examples mapped to safe categories, not literal translations of every upstream error. Field-specific messages require retaining structured validation details through the gateway. Do not fabricate an exact conflict cause if the API only returns a generic conflict.

## 8. Page-specific composition rules

### Schedule and Workload

Keep the existing source/timetable/inspector composition and workload faculty/department modes. Replace duplicated header/control styling through shared primitives. Avoid showing every context option in a tall wall: selected context remains compact; expanded entity lists open in a dedicated region. Remaining hours stay tied to the source requirement. Workload assignment tools sit beside or over the existing assignment column, never replace the table with a board.

### Resources

Use a stable secondary navigation shared across resource pages, grouped as `Учебный процесс` (subjects, groups, activity types, time slots, week types), `Преподаватели` (posts, titles, degrees), and `Аудиторный фонд` (buildings, building types, rooms, room types). This is navigation grouping only; do not alter data schemas or add unsupported entities. Desktop can use a compact category rail; narrow screens expose labelled sections, not a long unlabeled select.

The table is the main content. A single `Добавить…` action opens a labelled editor; row actions open the same editor with existing values. Default browsing shows values, not dozens of editable forms. Keep fields such as short name/code when required by the real contract. Search/pagination belong near the table. Deletion names the record and shows a dependency failure without removing it locally.

### Calendar

Preserve the five existing destinations: `Периоды`, `Семестры`, `Бронирования аудиторий`, `Нерабочие дни`, `Переносы дней`. Use consistent navigation and selection context. Do not make a new month calendar the main screen for four period date ranges.

Period kind + configured status + start/end fields + Save form one clear unit; its delete action belongs in that unit. Context uses readable entity selectors, not numeric fallbacks. Reservation and dated-rule screens keep existing tables and use shared editors. A weekday transfer's optional group is a base group, with explicit `Все группы` when blank. Do not expose unsupported holiday/transfer edit actions.

### Accounts

Open on search/filter/table, with `Создать учётную запись` as the primary action. Edit uses the same sheet as create, with role-specific fields in context. A person picker belongs in the teacher section; faculty/department scopes use searched labels and retained scope chips. Hide/disable irrelevant role fields and submit only intended changes; do not drop existing valid scopes on incidental search/navigation.

Keep role and status as small explicit controls. Teacher column should show a name if available; if the API lacks it, resolve through a bounded authorized read/serializer improvement rather than a query per row. Generated login is a suggestion: do not overwrite a manually changed login on every name keystroke. Password generation stays a deliberate separate operation; one-time credentials preserve no-store and must not appear in screenshots/evidence.

## 9. Responsive and accessibility floor

Keep existing sidebar thresholds: 224px desktop, 184px below 980px, compact horizontal navigation below 680px. Do not remove navigation accessible names when visual labels condense. Check 320px, 390px, 768px, 1280px and 1440px, plus 200% zoom. These are verification sizes, not five new breakpoint requirements.

Below the width needed for source + timetable + inspector, source and inspector become explicit expandable panels. Timetable retains readable column geometry and scrolls locally. Headers/first column may be sticky only if focus and cells remain visible. Main page must not overflow horizontally.

Focus ring: visible 2px with 2px offset, appropriate light/dark token. All inputs have persistent labels. Errors connect with aria-describedby/aria-invalid where appropriate. Status is text plus visual treatment. Maintain one main landmark. Native tables use scope/caption or equivalent accessible naming. Russian long names wrap; full identity must remain available without hover-only tooltips.

Motion: no decorative transitions in high-frequency actions; optional overlay fade/translation 120–180ms; no `transition: all`; reduced motion removes movement. Do not add drag libraries, animation libraries or table virtualization before measurement demonstrates a need.

## 10. Component ownership and completion

Shared primitives: page header, context item, button variants, field, feedback, entity search/list/selection, editor shell and pagination. Keep domain-specific table markup and business forms in their own views. Partial extraction requires real reuse and explicit locals; do not build a configuration-driven renderer for every page.

Each shared primitive is complete only when the actual consuming pages show default/hover/focus/disabled/pending/error states as applicable. The implementer captures computed type/control values and screenshots on at least two consuming pages. Documentation values are not a substitute for checking loaded assets.
