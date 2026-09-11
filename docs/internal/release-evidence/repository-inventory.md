# Repository documentation inventory

Date: 2026-09-11

This inventory records the classification used for the documentation reboot.
The classification is based on engineering purpose and source evidence, not on
file names alone.

| Classification | Current location | Contents |
|---|---|---|
| PUBLIC_SPEC | `docs/spec/` | Normative MMP/1 objects, wire rules, state machines, registries, security, and conformance. |
| PUBLIC_RESEARCH | `docs/research/` | Prior art, routing rationale, models, experiments, failure analysis, open questions, and bibliography. |
| PUBLIC_IMPLEMENTATION_GUIDE | `docs/implementation/` | Crate boundaries, Rust layout, integration constraints, fixed-store seams, and test strategy. |
| PUBLIC_DEPLOYMENT | `docs/deployment/` | Deployment profiles and platform validation procedures. |
| PUBLIC_PROJECT_DOC | `README.md`, `docs/README.md` | Project scope, entry points, maturity, build, and test commands. |
| INTERNAL_HANDOFF | `docs/internal/handoff/` | Current engineering state, integration sequence, release gates, and research measurement records. |
| INTERNAL_DECISION | `docs/internal/decisions/` | Open protocol decisions and implementation design gates. |
| INTERNAL_AUDIT | `docs/internal/audits/` | Security and design findings with evidence and residual risk. |
| INTERNAL_CONFORMANCE | `docs/internal/conformance/` | Claim and vector conformance records. |
| INTERNAL_RELEASE_EVIDENCE | `docs/internal/release-evidence/` | Implementation state, test guides, release records, inventory, and this reboot report. |
| AI_PROCESS_DEBRIS | deleted | Root instruction files, local workflow directories, prompt-oriented plans, and model-process records with no independent engineering role. |
| OBSOLETE_DUPLICATE | deleted | Numbered specification/course entry points replaced by the purpose-based tree. |

The original files remain available in Git history. No protocol history or tag
was rewritten.
