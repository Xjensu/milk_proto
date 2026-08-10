# E12 Attached Authority Certificate Handoff

**Status:** implemented and verified
**Date:** 2026-08-10
**Scope:** receiver-local authority lifecycle; no wire-format or frozen-specification change

## Problem closed

An announcement can carry a self-contained `AuthCert` in TLV `0x81`. Before
the host supplied a wall-clock sample, the receiver correctly refused to use
the certificate, but the old implementation discarded it. A later clock
sample or root restoration therefore required the sender to retransmit the
announcement.

## Implementation

- Validates the attached certificate's suite, signature, and subject binding to
  the announcement origin before retaining it.
- Retains at most 256 pending `(subject, scope)` certificates. A full map does
  not evict existing entries; a longer-lived certificate for an existing key
  may replace it. This is fail-closed under attachment flooding.
- Re-evaluates pending certificates after a non-decreasing wall-clock sample,
  root pin, authority snapshot restore, and normal authority expiry.
- Root rotation fences pending certificates from the former issuer for that
  scope and rebuilds only certificates matching the new pinned root, so a
  longer-lived stale grant cannot hide a shorter-lived valid replacement.
- Drops malformed, expired, revoked, display-expired, and otherwise invalid
  entries; untrusted-root and authority-capacity cases remain pending.
- Rebuilds the pending set from durable class-5 bundles at startup after
  validating stored metadata, bundle signature, announcement scope/signature,
  and the attached certificate. Startup reconstruction emits no application
  events, so it cannot duplicate an earlier announcement notification.

The existing fail-closed rendering rule remains unchanged: an unclocked or
expired announcement is not rendered official or elevated. Once the clock and
trusted root are available, a subsequent announcement can use the recovered
certificate without carrying it again.

## Verification

- `attached_authority_cert_received_before_clock_is_replayed_after_clock_ready`: passed.
- `attached_authority_cert_is_reconstructed_from_store_after_restart`: passed.
- `rotated_authority_root_accepts_shorter_lived_pending_certificate`: passed.
- `restored_authority_root_accepts_shorter_lived_pending_certificate`: passed.
- Authority-focused node tests: 14 passed.
- Full `mmp-node` library suite: 149 passed, 0 failed, serial execution.
- `mmp-core` authority suite: 16 passed.
- Strict `mmp-node` Clippy: passed.
- Focused nonzero display-cutoff regression: passed.
- `git diff --check` for the scoped Rust files: passed.

## Adversarial review

- **Security Auditor — potential flaw:** a hostile peer can fill the pending
  map. **Resolution:** the map is bounded and never evicts; later valid keys
  are rejected locally until normal expiry/recovery frees capacity.
- **Distributed Systems Reviewer — potential flaw:** a restart could lose a
  pending attachment. **Resolution:** startup reconstructs only from durable,
  cryptographically revalidated class-5 bundles.
- **Performance Engineer — potential flaw:** startup scans stored announcements.
  **Resolution:** the store is already quota-bounded; the scan runs once and
  performs no application callbacks.
- **Skeptical Professor — potential flaw:** existing unclocked announcements
  are not replayed as events after clock readiness. **Resolution:** this avoids
  duplicate user notifications; the recovered certificate is available for
  future authority decisions, while durable relay behavior is unchanged.
- **Battery Specialist — potential flaw:** clock updates might cause work.
  **Resolution:** re-evaluation is bounded to 256 entries and piggybacks on an
  existing host clock/root/expiry operation.
- **Inventive Scientist — potential flaw:** this is not a new wire primitive.
  **Resolution:** it is intentionally a lifecycle correctness repair; EDR/PCM/
  DoF research remains separate and non-normative.

## Boundary and next steps

The `specification/` directory is unchanged. Native keystore ratchet
persistence, mobile runtime gates, real BLE exchange, and external crypto
review remain release blockers. API/mobile prose reconciliation is also still
pending because those files contain existing user-owned dirty edits.
