# Research and design basis

Research documents explain why MMP uses DTN store-carry-forward, bounded replication, authenticated ingest, range reconciliation, and BLE-specific duty-cycle limits. They are evidence and hypotheses; they do not add wire requirements.

- [`design-basis.md`](design-basis.md): prior art and design choices.
- [`routing-research.md`](routing-research.md): Spray-and-Wait, Focus, utility, and copy-budget analysis.
- [`contact-window-scheduling.md`](contact-window-scheduling.md): short-contact scheduling and energy-model experiments.
- [`performance-model.md`](performance-model.md): cost, quota, battery, and scale models.
- [`failure-analysis.md`](failure-analysis.md): failure classes and mitigations.
- [`open-questions.md`](open-questions.md): unresolved technical decisions.
- [`bibliography.md`](bibliography.md): fetched sources and confidence levels.

The former numbered research reports were consolidated into these files; their Git history remains available for provenance.
