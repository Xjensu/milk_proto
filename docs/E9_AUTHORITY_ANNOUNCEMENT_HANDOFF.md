# E9 Authority and Announcement Lifecycle Handoff

**Status:** implemented and verified
**Date:** 2026-08-10
**Scope:** receiver-local security hardening; no wire-format or frozen-specification change

## What changed

This slice closes three lifecycle gaps in the Rev-B authority-announcement implementation.

1. `mmp-core::authority::AuthorityStore::grant_for` and `official` now require a cached
   certificate's issuer to equal the currently pinned root for the scope. Re-pinning a
   scope therefore fences certificates issued by the former root immediately, without
   waiting for their original `not_after` horizon. A newly issued certificate under the
   replacement root remains usable after normal verification.

2. `Node::tick` now calls `AuthorityStore::expire` whenever a host wall-clock sample is
   available. Expired certificates and sticky revocation entries can no longer occupy the
   bounded authority maps indefinitely. If the host has not supplied wall time, the node
   remains fail-closed for time-dependent authority decisions; monotonic bundle aging is
   unchanged.

3. `NodeCore::handle_announcement` now evaluates a nonzero announcement `not_after` before
   adopting an attached certificate or emitting `Event::Announcement`. An expired body, or
   a nonzero body received before the wall clock is ready, remains stored and relayable but
   is not rendered locally. The existing local producer uses `not_after = 0` as its explicit
   no-expiry sentinel; this preserves the existing public API while nonzero values follow
   the receiver-local clock and 15-minute skew rule from specification document 18 §6.

## Regression evidence

- `mmp-core`:
  `repinning_scope_invalidates_cached_certificate_from_old_root` verifies old-root cache
  invalidation and replacement-root recovery.
- `mmp-node`:
  `tick_prunes_expired_authority_entries_for_capacity_recovery` verifies lifecycle pruning;
  `expired_announcement_is_relayed_but_not_rendered` verifies relay/storage versus display;
  `unclocked_expiring_announcement_is_not_rendered` verifies fail-closed clock readiness.
- Focused and full gates:

  ```text
  cargo test --locked -p mmp-node --lib --all-features -- --test-threads=1
  139 passed, 0 failed

  cargo test --locked -p mmp-core --lib --all-features -- --test-threads=1
  91 passed, 0 failed

  cargo clippy --locked -p mmp-node --lib --all-features -- -D warnings
  cargo clippy --locked -p mmp-core --lib --all-features -- -D warnings
  both passed
  ```

`cargo fmt --all -- --check` remains red because the workspace already contains broad,
unrelated formatting drift. No unrelated files were reformatted.

## Specification and interoperability boundary

The frozen files under `specification/` were not modified. The implementation follows the
receiver-local expiry rule already described in `specification/18-authority-announcements.md`;
it does not change bundle encoding, signatures, reconciliation domains, or legacy relay
behavior. A Rev-A node still stores and forwards class-5 bundles blindly.

One precision item remains for a future reviewed erratum: document 18 defines `not_after` as
a wall-clock display cutoff but does not explicitly name the `0` value as a no-expiry sentinel.
The current local producer has always emitted zero, so changing that interpretation here would
silently break the existing API and vectors. Treat the sentinel as an implementation convention
until a specification review decides whether to formalize or replace it; do not edit the frozen
specification implicitly.

## Adversarial review

- **Skeptical Professor — potential flaw:** wall-clock expiry can disagree across partitions.
  **Resolution:** expiry is receiver-local with the existing skew tolerance; monotonic bundle
  aging and causal state do not depend on wall time.
- **Security Auditor — potential flaw:** a stale certificate could survive root rotation.
  **Resolution:** grant and official predicates are both fenced by the current pinned issuer,
  and the regression exercises both denial and replacement-root recovery.
- **Performance Engineer — potential flaw:** pruning on every tick could add work.
  **Resolution:** the authority maps are bounded; pruning is a linear pass over bounded state
  and runs only when a wall-clock sample exists.
- **Distributed Systems Reviewer — potential flaw:** expired announcements might be dropped,
  harming eventual delivery. **Resolution:** expiry gates only local rendering and certificate
  adoption; the durable bundle remains relayable.
- **Battery Specialist — potential flaw:** host wall-clock plumbing could add wakeups.
  **Resolution:** no timer or wakeup was introduced; pruning is piggybacked on the existing
  host-driven `tick` call.
- **Inventive Scientist — potential flaw:** root rotation could be treated as a global revocation.
  **Resolution:** the fence is intentionally scope-local, matching the authority model; global
  convergence remains a deployment/re-pinning concern, not a new wire primitive.

## Next model instructions

1. Do not alter RBSR priority partitioning, D3 weighting, or file scheduling without an approved
   specification erratum and legacy-peer vectors.
2. Keep the `0` announcement sentinel caveat visible in conformance planning.
3. The next high-value host-only work is documentation/API-surface alignment for
   `tryRevokeIdentity`; the modified mobile guides are user-owned dirty files and must be
   reconciled carefully before staging.
4. Remaining release gates are external: Linux/BlueZ two-host exchange, Android/iOS runtime and
   battery measurements, native keystore persistence, hardware TAL tests, and external crypto
   review.
