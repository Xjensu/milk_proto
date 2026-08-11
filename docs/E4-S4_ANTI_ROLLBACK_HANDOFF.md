# E4-S4 Windows Secret-State Anti-Rollback Handoff

**Status:** deferred production gate; no protocol or wire change
**Scope:** `mmp-node` Windows vault and the provider-neutral secret-state seam
**Evidence date:** 2026-08-12

## Finding

The current vault journal has a durable generation floor inside the journal
itself. It correctly rejects torn records, gaps, stale generations, invalid
AEAD envelopes, and records newer than the reported floor. It does **not**
detect replacement of the complete journal with an older, previously valid
backup.

Concrete sequence:

1. Generation 1 is committed.
2. Generation 2 is committed and the journal is copied by an attacker or
   rollback mechanism.
3. The journal is replaced with the older complete generation-1 copy.
4. A fresh process opens the journal, observes floor 1, unwraps the valid
   generation-1 DEK, and restores the older authenticated state.

This is a local storage rollback capability, not a remote-peer exploit. DPAPI
authenticates the envelope and protects the DEK, but it does not provide a
monotonic freshness counter. The existing journal floor therefore protects
against partial writes and ordinary record loss, not against rollback of every
copy of the journal that the provider can see.

## Current verification

The bounded review found no defect in the implemented crash-safety paths:

- `cargo test -p mmp-node vault --all-targets` — 28 passed;
- `cargo test -p mmp-node secret_state --all-targets` — 3 passed;
- `cargo clippy -p mmp-node --all-targets -- -D warnings` — passed.

The session/RBSR review also passed independently; no related parser or
resource-bound regression was found.

## Required decision before a production claim

Choose and document one freshness authority for the Windows provider:

- an OS-backed monotonic value (for example, a protected registry/TPM-backed
  counter) that cannot be restored with the journal;
- a provider-specific initialized marker and rollback-detection policy, with
  an explicit statement of what filesystem snapshot attackers are out of
  scope; or
- fail-closed restore when the provider cannot prove freshness, accepting
  loss of restart continuity rather than restoring stale secret state.

A sidecar file alone is not a complete solution if the same rollback operation
can restore both files. A DPAPI-protected copy of the generation is likewise
not a monotonic counter by itself.

## Acceptance criteria for the next implementation

The chosen design must add tests for:

1. generation 1 → generation 2 → replacement with a valid generation-1
   journal;
2. loss of the current record while the freshness authority remains present;
3. crash/torn-write recovery that still accepts the newest valid generation;
4. atomic restore rejection without mutating the live `Node`;
5. provider unavailability and freshness-authority failure being fail-closed.

Until those tests pass against a real Windows provider, the Windows secret
state path is a verified implementation slice, but not a complete
anti-rollback production guarantee.
