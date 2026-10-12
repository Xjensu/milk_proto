# Administrator UX concepts — 2026-09-21

Status: initial analysis and generated concepts complete; implementation and authenticated verification pending. The user explicitly requested implementation based on generated images. This document does not reduce that request to documentation only or claim a finished redesign.

## Direction

Use Teachers as the interaction reference: a clearly labelled search, readable explicit selection, then controls for the selected record. Preserve the existing Teachers workspace. Apply the sequence to other pages without forcing them into one layout.

- Schedule: compact committed context, readable period/week controls, workload beside the unchanged timetable, and an editor that opens when a destination is chosen. At narrower widths, make secondary panels explicit disclosures while keeping all timetable columns in a labelled scrolling region.
- Workload: compact mode navigation and committed context; put the search and create action near the existing table. Assignment uses a searched checkbox list and retains all previously selected teachers/rooms. Keep save explicit and reachable.
- Resources: group the existing 15 destinations into a stable local navigation. Show the list first, one create action, and an editor on demand. Preserve each resource's actual columns.
- Calendar: retain the four distinct period forms and the five existing destinations. Use a compact selected-context summary and clear separation of scope selection from date editing. No invented month-calendar replacement.
- Accounts: select role before showing relevant identity/scope fields; keep the existing searched selectors and table. Place save/cancel consistently and never retain passwords in drafts.

## Concepts and interpretation

Generated using the built-in image_gen tool. Exact prompts are in [PROMPTS.md](PROMPTS.md).

| Concept | Useful composition | Corrections required before implementation |
| --- | --- | --- |
| [Schedule](concepts/schedule.png) | Compact context, primary timetable, workload source and explicit placement | Preserve actual pair times, table headers and existing period semantics; do not invent a placement endpoint or always-visible footer. Retain real navigation icons and branding. |
| [Workload](concepts/workload.png) | Table-first browsing and bounded assignment sheet with retained selection | Generated heading is oversized; use the existing design contract. Omit invented breadcrumbs, account bar, logo, version, page-size control and pagination semantics. The actual sheet overlays rather than compresses table columns. |
| [Resources](concepts/resources.png) | Categorized navigation and aligned three-column Posts list | The lower editor is a separate design specimen, not an always-expanded form to ship. Use the existing modal sheet on demand. Preserve real navigation icons and branding. |

Images are directional specimens, not pixel-perfect accepted specifications. Generated typography, colors and sample content contain incidental deviations. Existing product copy, identity, domain semantics and the September 13 design contract take precedence. Do not insert image mockups as functional UI. No account or session data was supplied to image generation.

## Evidence from this session

- Read the canonical September 13 browser audit, implementation plan and design system before inspecting current source.
- Root Git status initially contained unrelated untracked `.agents/`, `.playwright-mcp/`, `docs/TZ_COMPLIANCE_AUDIT_2026-09-20.md`, `package-lock.json`, and `storage/redis_data/`. Rails and API repositories were clean. Preserve all unrelated files.
- Rails HEAD was `16ec036` (`Use visible transfer context choices`). Current source already includes context summaries, history handling, searched choices and modal resource editors; historical audit defects must be reproduced before repair.
- Playwright MCP successfully navigated to `http://localhost:3000/admin/teachers`, which redirected to `/?auth=1`. The current browser session is unauthenticated. The sign-in form is visible. No credentials were supplied in this conversation, no account was inserted, and authentication was not bypassed.
- Docker serves Rails on port 3000 and the API on port 2300. Rails container has no bind mounts. Host/container SHA-256 matched for Teachers view (`0d13f9dfd375d7ef6101aa80797b6ddaf0cf4a125032d2dcb89220e25da89e5c`) and Workload CSS (`d60860b6232fbed672c9156575727803fcb1192147055edd71803085630273d0`). This is sampled provenance, not full application/API parity.
- Inspected the saved September 13 Teachers screenshot as a historical reference, not a current render.
- Current `_admin_studio.css` has broad late overrides, including 80px mode navigation, 20px table cell padding and 48px choices. These are source-level reasons to inspect density/cascade; their current computed effects are not yet measured.
- Current resource navigation is a flat 15-link sequence. Source already uses create/edit dialogs. Improve navigation and geometry rather than rebuilding those dialogs from historical assumptions.

## Next action and verification boundary

The user has been asked to sign in through the connected browser and report readiness. No application source was changed, no service rebuilt, no data mutated, and no tests added. This is an authentication dependency, not a request for design approval.

Once authenticated: capture current Teachers and target-page screenshots; verify source/assets; complete the authorized baseline journey (group/semester → workload → teacher assignment → placement → edit → reload/read-back). Then implement and verify one canonical batch at a time, beginning with Schedule/Workload shared controls and composition. Preserve already-fixed behavior. Before changing any request trace the full Rails → gateway → API contract and authorization chain. Do not skip the baseline gate merely because concept generation is complete.

Remaining: current rendered audit, full journey baseline, remaining page/state concepts where needed, implementation, direct interaction verification, responsive checks, concept/render comparison, and handoff update with actual results. No visual-fidelity or functional-completion claim is made.
