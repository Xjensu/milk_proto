# Implementation and Handoff Index

**Status:** current repository index, 2026-08-14
**Scope:** documentation organization only; no protocol or specification change

This index is the entry point for implementation notes that sit beside the
authoritative roadmap. It prevents a historical handoff from being mistaken
for a current task, while preserving every existing file in its original
location for traceability.

## Authority order

1. `specification/01-*.md` through `specification/18-*.md` — frozen wire and
   protocol contract.
2. `docs/ROADMAP_TO_PRODUCTION.md` — phase gates and current production status.
3. `docs/AGILE_BACKLOG.md` — historical story record and detailed acceptance
   evidence.
4. `docs/AGILE_REMAINING_WORK.md` — the current de-duplicated execution queue.
5. The handoffs, designs, decisions, and proposals below — implementation
   evidence or explicitly non-normative research; their own status line is
   binding for how they may be used.

The executive summary's “FINAL for v1 specification suite” label is read as
the release index for the core 01–15 suite. The per-document phase/draft
markers remain authoritative for document maturity in
`docs/SPEC_STATUS_MANIFEST.json`; documents 16–18 are final rev-B extensions.
This distinction is documentation governance only and does not modify the
frozen specification.

## Implemented or verified handoffs

These files record completed bounded slices. They are evidence, not open work
unless their “next steps” explicitly appear in `AGILE_REMAINING_WORK.md`:

- `E8_FFI_REVOCATION_HANDOFF.md`
- `E9_AUTHORITY_ANNOUNCEMENT_HANDOFF.md`
- `E10_BLOB_CI_TEST_TIER_HANDOFF.md`
- `E11_LIFECYCLE_CONCURRENCY_HANDOFF.md`
- `E12_ATTACHED_AUTHORITY_HANDOFF.md`
- `E7_JOURNAL_WRITER_HANDOFF.md`
- `E7_PRODUCER_CAPACITY_INCREMENT_HANDOFF.md`
- `E8_CLAIM_PRODUCER_CAPACITY_HANDOFF.md`
- `E7_DURABLE_FLOOR_CODEC_HANDOFF.md` (bounded codec slice only)
- `PERIPHERAL_LIFECYCLE_HANDOFF.md` (host-testable lifecycle only)
- `SESSION_CODEC_CONFORMANCE_HANDOFF.md` (bounded conformance audit)
- `OWN_BUNDLE_STORE_TRANSACTION_HANDOFF.md` (bounded storage audit)
- `MOBILE_BACKPRESSURE_HANDOFF.md`
- `MOBILE_THROWING_CONTRACT_HANDOFF.md`
- `SEND_CHAIN_TRANSACTION_HANDOFF.md`

## Designs and deferred implementation gates

These are not proof that a feature is production-ready:

- `E4-S4_ANTI_ROLLBACK_HANDOFF.md`, `E4-S4_FRESHNESS_AUTHORITY_DESIGN.md`,
  `KEYSTORE_SNAPSHOT_DESIGN.md`, and `MOBILE_KEYSTORE_BRIDGE_DESIGN.md` —
  platform persistence and restart gates.
- `E5-S4_CONFORMANCE_DESIGN.md`, `E5-S4_IMPLEMENTATION_PLAN.md`, and
  `E5-S4_CONFORMANCE_HANDOFF.md` — vector/conformance history; the last two
  contain historical checkpoints and must not be read as a clean current plan.
- `E7_FIXED_STORE_DESIGN.md`, `E7_FIXED_STORE_HANDOFF.md`,
  `E7_TRANSACTIONAL_API_DECISION.md`, `E7_COORDINATED_CHECKPOINT_DECISION.md`,
  `E7_FLASH_ADAPTER_REVIEW.md`, `E7_NRF52_PROVIDER_CONTRACT.md`, and
  `E7_NRF52840_DK_PROFILE.md` — embedded durability and provider decisions.
- `RBSR_D3_CONFORMANCE_HANDOFF.md` and `RBSR_PRIORITY_PARTITION_PROPOSAL.md` —
  deferred until an approved protocol meaning exists.

## Proposals and research-only notes

- `CONFORMANCE_002_FRIENDSHIP_PROPOSAL.md`
- `GROUP_CONTROL_PROTOCOL_PROPOSAL.md`
- `PROPOSAL_AUDIT_MMP_PHI.md`
- `MMP_PHI_EDR_HANDOFF.md`
- `E6_S1_SOCIAL_CLUSTER_HANDOFF.md`
- `AUTHORITY_ARITHMETIC_HANDOFF.md`
- `E7_EMBEDDED_HANDOFF.md`
- `CONTACT_SURVIVAL_IMPLEMENTATION_PLAN.md` (bounded research model; no
  production wire change)
- `MMP_PHI_SIMULATION.md` and `DOF_SPRAY_SIMULATION.md` (research models and
  measurement scaffolding)

## Supporting implementation and test documents

These are current reference material rather than handoff status records:

- `API_REFERENCE.md`, `BETA_TEST_APP_API.md`, and
  `BETA_TEST_APP_NICKNAME_API.md` — host/beta API contracts.
- `AEAD_AD_CONFORMANCE.md`, `GROUP_CONTROL_SECURITY_AUDIT.md`, and
  `INTEROPERABILITY_TEST_GUIDE.md` — conformance and security evidence.
- `E4-S5_BATTERY_RECORDING_TEMPLATE.md`, `MANUAL_BETA_TESTING.md`,
  `MANUAL_INTEROP_TESTING.md`, and `PC_MOBILE_MANUAL_TEST_GUIDE.md` —
  measurement and manual-test procedures.
- `USER_FEATURES.md` and `MOBILE_API_MIGRATION.md` — product-facing and API
  migration notes.

The MMP/Φ documents are research hypotheses. They do not alter MMP/1 and are
not production tasks until the baseline-and-candidate experiment gates in
`AGILE_REMAINING_WORK.md` are satisfied.

## Historical-source rule

References to `other/AUDIT.md`, `other/HANDOFF_MOBILE.md`, `spec_gaps.md`, and
`impl_audit.md` identify source material that is not present in this checkout
and has no recoverable Git history. Those names must not be used as links to
new work; the reconstructed history and current status are in the files named
above. Restoring the originals is deferred until an owner-supplied archive
exists.

No handoff/design/proposal/decision file was renamed or deleted in this cleanup.
