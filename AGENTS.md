# Project working instructions

Latest user direction, 2026-09-08: work directly. Do not spawn agents for implementation or review. This supersedes older handoff requirements for reviewer agents and three review cycles.

Chat and documentation: English. Product interface: Russian. Preserve the existing visual system.

## Current assignment and precedence — 2026-09-13

The user now reports that functionality works and requests a consistent administrator design system and substantially better UX. This supersedes the older instruction to assume schedule creation/editing is broken. The September 13 assignment is analysis and documentation only; implementation starts when the user invokes the saved implementation prompt or otherwise requests it.

Canonical package: [Administrator workbench plan](docs/design/admin-workbench-2026-09-13/IMPLEMENTATION_PLAN.md). Read its companion AUDIT.md, DESIGN_SYSTEM.md and HANDOFF.md selectively. The copy-ready implementation prompt is IMPLEMENTATION_PROMPT.md in that directory. These documents supersede conflicting older design/handoff instructions for this initiative. They do not erase the broader migration backlog.

Latest evidence: [Browser audit](docs/design/admin-workbench-2026-09-13/BROWSER_AUDIT.md), captured through authenticated Playwright MCP on September 13. Read it before acting on the historical source audit or appended source-complete handoff entries. The focused restart prompt is [Browser-based continuation](docs/design/admin-workbench-2026-09-13/BROWSER_BASED_CONTINUATION_PROMPT.md). This continuation updates documentation only; existing UI changes predate it.

Playwright MCP is available in the audited session and normal sign-in succeeded. Do not repeat an old browser-unavailable claim without attempting the currently available tool. Do not execute the user's account insertion snippet: the supplied account already worked. Never copy credentials into prompts, evidence or AGENTS files.

The new admin CSS files already exist and load; the sampled Workload mobile table retains all columns; Accounts has searched edit selectors and readable teacher labels. Preserve this progress. Remaining verified defects include inconsistent computed control sizes, excessive context height, lost Schedule group context on fresh navigation to its displayed URL, overlapping account editing, five-field weekday-transfer content using three tracks, and unavailable reservation data. Treat findings V01–V08 as concrete acceptance work, not authorization for a rewrite.

Preserve the recognizable Schedule, Workload, Teachers and Notice workspaces. Resources, Calendar and Accounts are anti-references for composition and interaction, not templates to copy. Preserve the dark sidebar, Manrope/Unbounded typography, ink/velvet/almond identity and required table structures. Unification means shared controls, spacing, hierarchy and state behavior; it does not mean making every page the same layout.

Do not change table columns, their order, header hierarchy, grouping, row meaning, axes, merged cells, totals or report semantics. Do not turn tables into cards or hide columns on small screens. Use an explicitly labelled scrolling region. The local specification does not supply an explicit column schema; preserve the current structures and record any discrepancy rather than inventing one.

## Priority and completion

During implementation, first capture the running baseline and verify the core journey: select group/semester, create workload, assign a teacher, place a lesson, edit it, reload and read back the saved result. A reproduced regression takes priority over visual work on that journey. Historical bug reports alone do not justify reopening unrelated functionality.

Do not substitute source review, documentation commits or green syntax checks for a usable flow. Verify the source revision and assets actually running. Defer unrelated infrastructure, search-engine, database and React migration experiments during the administrator design initiative.

Implement in the order defined by the plan: baseline; shared primitives plus Schedule/Workload consistency; complete assignment/placement/editing flow; Resources; Calendar; Accounts; adjacent-page and responsive verification. Finish and verify one batch before starting the next. Do not add new tests or test files.

## Fixed UX rules

- High-cardinality entities use visible, bounded searchable lists with explicit selection and readable secondary context. Do not put teachers, subjects, rooms, departments or groups in long select menus. No raw database ID fields in normal administrator flows.
- Small stable enumerations may use native selects; prefer visible segments for frequently switched 2–4 choices. Use DESIGN_SYSTEM.md's control matrix. Do not replace all selects with custom widgets.
- Drag and drop is frequent on scheduling/assignment surfaces, with an equally capable click/tap and keyboard path. It stages a clearly labelled operation. It must not silently save, delete, change scope or replace a collection.
- Preserve all existing selections when adding one teacher/room to a workload assignment. The API assignment PUT replaces the entire collection; sending just the dropped item would erase other assignments.
- Keep the original record and safe form input on failures. Distinguish recurring edits from dated transfers and swaps. Never simulate an edit with delete/create.
- A dirty editor survives search, validation and recoverable failures. Switching context must resolve the draft explicitly. Passwords are never reflected back or saved in drafts.
- Use the same admin header, control heights, focus treatment, button meanings, feedback and editor structure across pages. Page CSS must not restyle unrelated pages or public UI.
- Keep all product copy Russian and all implementation documentation English. Do not expose backend error symbols as user messages.

## Repository boundaries and working method

The workspace root, apps/schedule_web and apps/api_geteway have independent Git state. Inspect each before work. Rails was clean only at the initial source audit; all three repositories had pre-existing changes by the browser continuation. Never attribute those changes to the current task or stage them wholesale.

Use the existing Rails ERB/Hotwire/importmap/Propshaft stack. No React, Tailwind, shadcn, frontend rewrite, database ownership transfer or new component framework for this initiative. Small shared ERB partials and scoped CSS are the default. Use contract/operation/presenter layers when behavior changes require them; do not refactor every older controller as a prerequisite to styling.

Before a changed request, trace Rails route -> controller/contract/operation -> gateway -> API route/action/contract -> repository/authorization. Do not infer API names from UI labels. Search results must preserve the identifier type expected by the write contract.

## Architecture and implementation

- Rails owns UI, typed forms/operations, presenters and API gateways. Hanami API owns authorization, transactions, repositories and database business rules. Preserve the completed Excel worker.
- Read the actual request contract before writing a client call. Normalize identifiers once; authorization and persistence must use the same identities.
- Editing must preserve existing data on validation, conflict or network failure. Do not implement editing by deleting and recreating a lesson.
- Authorize every affected record; protect concurrent writes and keep related mutations and Outbox records atomic.
- Prefer small explicit operations and existing conventions. Avoid generic frameworks, speculative abstractions, unnecessary dependencies and excessive comments.
- Use searchable selectors for large data, with keyboard-accessible controls. Drag and drop is an enhancement to the same server operation.

## Verification and work discipline

- No new tests or test files, per user. Use relevant syntax/lint/loading checks plus direct runtime and browser verification. Inspect errors and authoritative read-back after mutations.
- Work through one complete user journey before switching to another. Track only concrete findings, changes, evidence and remaining blockers.
- Preserve dirty work in all nested repositories. Commit only attributable changes. Never reset storage or delete React until full parity is demonstrated.
- Keep credentials, tokens, session data and proxy authentication out of logs, committed files and tool output.
- Select skills only when they help the current task. User instructions override conflicting skill workflows. No permission or delegation ceremony inferred from a skill.

Setup rationale and sources: apps/schedule_web/docs/handoff/ASTRA_WORKING_SETUP.md.
