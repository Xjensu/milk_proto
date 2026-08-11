# E4-S4 Freshness Authority Design

**Status:** proposed provider gate; not implemented
**Scope:** Rust-internal secret-state vault providers
**Protocol impact:** none; `specification/` and all MMP/1 wire bytes remain unchanged
**Evidence date:** 2026-08-12

## Decision

The generation floor stored inside the Windows journal is useful for torn-write
and partial-record recovery, but it is not an independent anti-rollback
authority. Replacing the complete journal with an older, previously valid copy
can also replace its floor. The current DPAPI adapter therefore remains a
verified implementation slice, not a full local-active anti-rollback
guarantee.

The next provider contract shall be a Rust-internal `FreshnessAuthority`.
It must be supplied by a platform-owned capability (for example, a service or
hardware-backed monotonic store) and must not be exposed through UniFFI,
React Native, or the protocol wire format.

## Evidence used

- `docs/E4-S4_ANTI_ROLLBACK_HANDOFF.md` records the reproducible valid-journal
  replacement sequence and the existing provider boundary.
- `mmp-node` vault verification passes 28 focused tests, including context,
  generation, corruption, lock, and atomic-restore cases; strict Clippy is
  clean.
- The delegated mobile review found that Kotlin/Swift coordinators and the
  UniFFI surface correctly exclude vault, ratchet, DEK, and provider-log data;
  native keystore wiring remains a platform gate.
- The delegated acceptance review adds fault-ordering, authority-loss,
  reset, concurrency, and whole-volume cases that are not yet covered by a
  real protected authority.

## Contract shape

The following is the proposed shape; it is deliberately not public API and is
not yet code:

```rust
struct FreshnessScope {
    kind: u8,
    identity: Key32,
    node_id: Id16,
}

enum FreshnessState {
    Uninitialized,
    Initialized {
        floor: u64,
        record_digest: [u8; 32],
    },
}

trait FreshnessAuthority {
    fn inspect(&mut self, scope: &FreshnessScope)
        -> Result<FreshnessState, VaultError>;

    fn advance_before_publish(
        &mut self,
        scope: &FreshnessScope,
        expected: u64,
        next: u64,
        record_digest: [u8; 32],
    ) -> Result<(), VaultError>;
}
```

The authority stores only a protected scope binding, a monotonic floor, and a
digest of the exact sealed record published at that floor. It does not receive
plaintext ratchet state or the data-encryption key. The record digest is
computed over a domain-separated canonical provider record (including the
generation, context, envelope, and opaque wrapped-DEK bytes), so two different
records at the same generation cannot be accepted interchangeably. The scope
must be immutable for the provider instance, or passed explicitly to every
authority operation; it must never be inferred from mutable conversation data.

The implementation may derive a provider-local scope digest with the existing
BLAKE3 dependency and domain separation. That digest is adapter metadata, not
a new signed or transmitted MMP field.

## Required ordering and failure policy

All operations below run under the existing identity-scoped provider lock:

1. Read and validate the journal and the authority state.
2. Construct the candidate's complete sealed record and its record digest.
3. Require exact agreement between the existing journal and authority floor
   plus record digest.
4. Advance the authority with a durable compare-and-swap to the candidate
   generation and digest.
5. Publish and flush the complete journal record.

If the process fails between steps 3 and 4, the authority is newer than the
journal. The next open must return a capability/generation failure. It must not
lower the authority, infer freshness from the journal, or restore the older
record. This intentionally prefers loss of restart availability over stale
secret-state restoration.

Freshness states have these semantics:

| Authority | Journal | Result |
|---|---|---|
| Uninitialized | missing | fresh optional vault (`Ok(None)`) |
| Initialized at `N` with digest `D` | exact record/floor `N` with digest `D` | usable |
| Initialized at `N` | missing, lower, higher, or different digest | fail closed |
| unavailable, locked, reset, or scope-mismatched | any | fail closed |

The authority accepts only `0 → 1` during explicit first initialization or
`N → N+1` thereafter. Gaps, equal values, decreases, and overflow are
rejected. There is no normal reset method. Destructive reprovisioning must be
an explicit provider lifecycle operation; clearing both stores silently would
make old journal images usable again.

## Threat-model boundary

This design detects replacement or deletion of the journal when the protected
authority remains outside the attacker's rollback operation. It does not claim
to defeat an administrator, kernel, or full-system snapshot attacker who can
restore both the journal and the authority. Such a provider must report
unavailable unless it has a stronger hardware or service trust boundary.

The current Windows DPAPI journal is therefore not upgraded by this document.
No registry sidecar, file lock, process-local clock, or DPAPI-protected copy of
the same generation is sufficient by itself.

## Current Windows target (not implementation approval)

The bounded platform review recommends a TPM 2.0 NV monotonic counter accessed
through Windows TBS behind a provider or service boundary. A scope must map to
an explicitly provisioned NV index (or the deployment must deliberately limit
one scope per vault), and read/increment operations must remain serialized by
the existing identity lock. Missing, blocked, reset, or unavailable TPM state
must fail closed. A service-owned registry/IPC authority is acceptable only as
an interim boundary against a narrower unprivileged file attacker; it is not a
full-volume rollback defense. CNG TPM-backed keys alone are not a freshness
counter.

This target is still a gate, not a shipped implementation. It requires a
Windows proof of concept, scope provisioning and reset policy, reboot and
power-loss tests, and separate physical-TPM and vTPM evidence. No new TPM Rust
crate is selected until its Windows TBS compatibility, MSRV, native dependency,
and supply-chain risk are reviewed.

## Acceptance matrix

The future provider implementation must add focused tests for these cases:

1. Fresh install: missing journal plus uninitialized authority returns an
   empty optional vault without attempting a key unwrap.
2. Full rollback: after generations 1 and 2, replacing the journal with a
   valid generation-1 image while the authority remains at 2 rejects before
   unwrap and leaves the live node unchanged.
3. Record loss: an initialized authority plus a missing or older journal never
   falls back to `Ok(None)` and never lowers the authority.
4. Torn write: a partial candidate before publication recovers the last
   authority-matching complete generation.
5. Commit-order crash: authority-newer/journal-old and journal-newer/
   authority-old states both fail closed; neither side is rolled back.
6. Corrupt, forked (including same-generation/different-digest), skipped,
   equal, or overflowing generations reject without mutating the node.
7. Authority unavailable, locked, reset, ACL-denied, or hardware-absent
   returns a coarse capability failure with no journal-floor fallback.
8. Restore concurrency keeps the provider transaction through validation and
   the in-memory state swap.
9. Busy-node restore, malformed records, and cross-node replay leave all live
   state and diagnostics unchanged.
10. Wall-clock rollback cannot make an older authority value acceptable.
11. A whole-volume rollback is either detected by the selected independent
   authority or explicitly classified outside the provider's threat model.

Windows CI can run fault-injected provider models, file replacement, torn-tail,
lock-contention, and unchanged-state assertions. A production claim additionally
requires a real service/TPM or equivalent authority, reboot/power-loss tests,
ACL/reset behavior, and separate physical and virtual-TPM evidence where
applicable. Android and iOS providers need the same contract-level tests plus
their native keystore lifecycle gates.

## Implementation gate

Do not add the trait or change `SnapshotVault` until one concrete platform
authority is selected and its ordering/durability behavior is reviewed. The
implementation must reuse the existing BLAKE3, AEAD, zeroization, and bounded
codec dependencies; no generic serializer, new wire field, or public secret
callback is justified by this design.

Until that gate is met, the correct terminal status is
`DEFERRED-WITH-REASON`: the Rust vault and Windows journal remain tested, while
platform-backed restart continuity and full anti-rollback remain unclaimed.
