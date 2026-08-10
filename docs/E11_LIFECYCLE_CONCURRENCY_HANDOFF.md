# E11 Lifecycle and Concurrency Handoff

**Status:** implemented and verified
**Date:** 2026-08-10
**Scope:** receiver-local authority replay and shared-session ratchet lifecycle; no wire or specification change

## Implemented

### Authority claims and clock readiness

An `AUTHCERT` claim can arrive before a host has supplied a wall-clock sample.
The claim is still retained by the ordinary claim store, but the authority
admission layer must not evaluate its expiry until the clock is available.
`Node::set_wall_clock` now reprocesses retained, subject-bound `AUTHCERT`
claims through `AuthorityStore::consider_cert`. This closes the one-shot claim
loss where a valid certificate remained unusable unless it was gossiped again.

The reprocessing is receiver-local and fail-closed: malformed claims,
subject/certificate mismatches, untrusted roots, expired certificates, and
revoked certificates remain rejected. No claim bytes, bundle fields, or wire
state changed.

### Parallel authenticated links

Conversation ratchet state is shared by all authenticated links to one peer.
Closing one link must not advance the conversation epoch while another live
link can still carry traffic. `LinkDown` now advances the repeat-contact epoch
only for the final authenticated link for that peer. This prevents one physical
link teardown from invalidating ciphertext still in flight on a parallel link.

The same authenticated-only bump is now applied when `SessionEngine` closes
internally because of a protocol error or outbox overflow. A Noise channel that
has reached `Binding` but has not accepted `ID_PROOF` is explicitly excluded:
it has session material, but it has not established a trusted conversation.

## Verification

- `authority_claim_received_before_wall_clock_is_reconsidered_after_clock_ready`: passed.
- Authority-focused node tests: 10 passed.
- `repeat_contact_defers_epoch_bump_until_last_parallel_link_down`: passed.
- `pre_identity_proof_internal_close_does_not_mark_conversation_established`: passed.
- Existing `ratchet_epoch_bumps_on_repeat_contact_and_messages_still_decrypt`: passed.
- Full `mmp-node` library suite: 145 passed, 0 failed, serial execution.
- Earlier repeated parallel runs covered the 142-test pre-auth-gate state and
  passed after the worktree settled. One earlier invocation was inconsistent,
  so the normal CI command remains explicitly serial; the authoritative current
  result is the 145-test serial run above.
- Strict `mmp-node` Clippy: passed.

The serial test mode remains the normal CI default because store-sensitive
tests are intentionally serialized. The repeated parallel runs are additional
evidence, not a replacement for that deterministic CI policy.

## Specification and interoperability boundary

The frozen files under `specification/` were not modified. Both changes are
local lifecycle behavior over existing claims, sessions, and ratchet state;
legacy peers see the same wire messages and frame encodings.

## Adversarial review

- **Security Auditor — potential flaw:** replaying all retained authority claims
  could re-admit a stale grant. **Resolution:** every replay goes through the
  existing root, signature, expiry, revocation, and capacity checks.
- **Distributed Systems Reviewer — potential flaw:** two links may disagree on
  whether the contact is still live. **Resolution:** the guard is local to the
  authenticated session table; the final local teardown is the only event that
  advances the shared epoch.
- **Performance Engineer — potential flaw:** clock readiness adds an O(n) pass.
  **Resolution:** the claim table is bounded by the existing authority limits,
  and the pass runs only when the host accepts a non-decreasing wall-clock
  sample.
- **Skeptical Professor — potential flaw:** this does not prove ratchet recovery
  after process restart. **Resolution:** platform-keystore persistence and the
  restored-epoch bump remain separate Phase B gates.
- **Battery Specialist — potential flaw:** no new wakeup or timer was added.
  **Resolution:** replay is piggybacked on an existing host clock update.
- **Inventive Scientist — potential flaw:** this is an optimization rather than
  a new wire primitive. **Resolution:** it is deliberately a correctness and
  lifecycle slice; experimental EDR/PCM/DoF work remains research-only.

## Next model instructions

1. Preserve the encrypted `ID_PROOF`, canonical AEAD AD, copy-budget, and time
   model boundaries documented by the existing errata; do not infer a wire
   change from this lifecycle work.
2. Keep native keystore ratchet persistence and restored-epoch bump wiring behind
   the Android/iOS platform gate.
3. Reconcile the existing dirty FFI/mobile documentation before adding release
   documentation-enforcement checks.
4. Keep RBSR priority partitioning, D3 weighting, origin-targeted blob
   scheduling, and Bluetooth hardware exchange deferred until their contracts
   and evidence are available.
