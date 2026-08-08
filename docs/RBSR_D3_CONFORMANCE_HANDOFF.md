# RBSR Priority Partitioning and D3 Admission: Conformance Handoff

**Status:** Deferred pending an approved protocol erratum or amendment
**Scope:** E5-S5 follow-up (authority announcements, RBSR, and relay admission)
**Wire compatibility:** No specification or wire-format change was made in this checkpoint.

## Executive decision

The two open E5-S5 items cannot be implemented safely as a zero-wire-change
patch from the frozen documents:

1. **RBSR priority-class partitioning** is required by the protocol phases, but
   the frozen suite does not define how the priority sub-partitions are encoded
   in the existing `domain` byte or negotiated with a peer.
2. **D3 admission weighting for subscribed scopes** is required as a local
   policy hook, but no weighting function, quota interaction, or starvation
   rule is normative.

Guessing either behavior would create two plausible implementations that can
disagree while still accepting the same wire bytes. That is a conformance and
availability risk, not an optimization opportunity. The correct next step is
to obtain a protocol decision, then add vectors and interop tests before
enabling the behavior.

## Evidence from the frozen specification

The following statements are normative or explicitly constraining:

| Source | Frozen requirement | Consequence |
| --- | --- | --- |
| `specification/03-protocol-spec.md` §5.2 | RECONCILE is RBSR over BundleID sets **per priority class**, highest priority first. | An implementation must eventually expose stable priority-class reconciliation. |
| `specification/06-sync-spec.md` §2 | The domains are D0 control, D1 direct, D2 group, D3 relay spray, and D4 blob symbols. | The current domain vocabulary has no declared priority sub-domain. |
| `specification/18-authority-announcements.md` §4 | Class-5 announcements reuse D3; no new RBSR domain or HELLO capability bit. | Adding a new numeric domain or capability would violate the zero-wire extension constraint. |
| `specification/18-authority-announcements.md` §5.2 | RBSR partitioning uses the signed **declared** priority, never receiver-local effective priority. | The local authority clamp cannot be used as a reconciliation key. |
| `specification/18-authority-announcements.md` §5.2.2 | Wire ordering follows declared priority; the admission-aware sort-key fix is an errata candidate. | The document intentionally records the queue-jump residual instead of defining a new key. |
| `specification/05-routing-spec.md` §6, §8 | Transfer order is class/destination/declared priority/size/lifetime; quota and origin buckets are described without an admission-weight formula. | There is no normative definition of “weight D3 for a subscribed scope.” |

The authority-announcement document therefore describes the intended seam but
does not supply the missing algorithm. Its explicit choice to reuse D3 is
important: it keeps rev-A peers interoperable, but it also means that any
priority-class implementation must be representable inside D3 without
inventing a new negotiated domain.

## Evidence from the current Rust implementation

The implementation deliberately reflects the currently executable subset:

- `mmp-core/src/session.rs` defines `DOMAINS` as `[0, 1, 2, 3]`, validates only
  those values (apart from the reply flag), and advances one `Reconciler` at a
  time. There is no priority partition cursor or domain encoding.
- `mmp-node/src/lib.rs` maps each stored bundle to exactly one domain:
  classes 0/3 -> D0, eligible direct class 1 -> D1, class 2 -> D2, and all
  remaining classes (including class 5) -> D3. The `SyncView` key is `(BundleID,
  domain)` only.
- `mmp-router/src/lib.rs::transfer_order` correctly sorts by the signed,
  declared `priority`. `eviction_order` uses `effective_priority` only for
  receiver-local storage pressure, matching the authority-announcement
  distinction. Neither function implements RBSR priority partitions.
- `NodeCore::on_bundle` has explicit friendship and solicited-symbol admission
  paths, then charges the ordinary origin/WoT bucket. It does not contain a
  class-5 subscribed-scope multiplier or reserved D3 quota.

This is not a missing one-line branch. The data model and session grammar
would need a shared interpretation of the partition key before an implementation
could be interoperable.

## Ambiguity A: RBSR priority-class partitioning

The frozen text gives two facts: priority partitions exist conceptually, and
the key is the declared byte. It does **not** say which of these possible
representations is the wire truth:

1. Encode `(D3, declared_priority)` as new numeric domain values.
2. Run several reconcilers under the existing D3 value, with an implicit
   priority field in the RBSR range namespace.
3. Reuse a priority value as the domain and derive D0-D4 from another field.
4. Keep one D3 reconciler and order returned IDs by priority outside RBSR.

These choices are not equivalent. (1) changes the accepted `domain` space and
legacy negotiation. (2) requires a new range/partition grammar. (3) collides
with the existing D0-D4 meaning. (4) does not satisfy “RBSR over sets per
priority class” because a range fingerprint would mix classes. A receiver that
chooses a different mapping can produce different fingerprints, reject a
range, or starve a class while both peers remain otherwise valid.

The effective priority is not an escape hatch: it is local, may differ between
honest receivers, and is expressly forbidden as an RBSR key. Using it would
reintroduce the cross-peer mismatch that the frozen text calls out.

## Ambiguity B: D3 admission weighting

“Weight D3 relay-spray admission” has several materially different meanings:

- multiply the per-origin bucket cost for a subscribed scope;
- reserve a fraction of the D3 quota for subscribed announcements;
- change queue order only, without changing admission volume;
- apply a class-5/authority bucket in addition to the ordinary origin bucket;
- use a probability of admission based on scope, age, or declared priority.

The documents specify none of the multiplier, reserve, floor, refill, ordering,
or starvation behavior. Each option changes security and availability:

- a multiplier can make legitimate announcements disappear under quota;
- a reserve can starve ordinary relay traffic;
- queue-only weighting leaves admission abuse unchanged;
- a second bucket can accidentally create an unbounded bypass if duplicates,
  paths, or scope aliases are counted differently;
- probability-based admission needs deterministic testability and a replay-safe
  random source.

The existing authority bucket bounds elevated admissions per subject, while the
ordinary origin/WoT bucket bounds relay resources. Combining them without a
normative rule could invalidate those bounds or make behavior differ by node.

## Non-spec-changing implementation plan

After the protocol decision is recorded, implement in this order:

1. **Freeze the partition key.** Add a small internal `SyncPartition` type that
   carries `(base_domain, declared_priority)` and a single conversion function.
   Keep `effective_priority` out of this type. Do not change `Frame` or accepted
   domain values until an erratum defines the representation.
2. **Make the session engine data-driven.** Replace the implicit `0..3` cursor
   with an ordered partition list supplied by the host. The default list must
   remain exactly the current D0-D3 behavior until the wire decision is
   approved; D4 remains a blob availability phase as specified.
3. **Define D3 admission semantics.** Record whether weighting is order-only,
   a bucket multiplier, a reserved quota, or a second bounded bucket. Specify
   units, refill, duplicate accounting, scope matching, authority failure,
   and starvation guarantees. The rule must be local and must never mutate the
   signed priority byte.
4. **Add golden vectors and interop fixtures.** For every base domain and
   priority 0-3, assert the exact partition key, RBSR range bytes, phase order,
   and legacy behavior. Include a rev-A peer that knows only D0-D4.
5. **Add adversarial/property tests.** Cover effective-priority divergence,
   self-declared priority-0 junk, duplicate relay paths, invalid/expired
   certificates, unsubscribed scope, subscribed scope, token exhaustion,
   and quota pressure. Assert that no local policy changes the wire key and
   that D0/D1/D2 cannot be starved by D3 weighting.
6. **Run cross-implementation tests before enabling production defaults.** Two
   independent engines must produce identical partition/fingerprint transcripts
   from the same store, and a legacy peer must continue to complete a session
   without a new HELLO capability.

## Acceptance criteria for reopening this work

This follow-up is ready for implementation only when all of the following are
available in an approved document or erratum:

- a byte-level definition of the priority partition key and its negotiation;
- the exact phase order and behavior when a peer supports only D0-D4;
- an admission-weight formula with constants and quota/refill semantics;
- explicit duplicate, expiry, certificate, and scope-mismatch accounting;
- starvation and abuse bounds that do not rely on receiver-local effective
  priority being globally consistent;
- golden vectors covering legacy and upgraded peers.

Until then, the current Rust behavior is the safest conforming behavior: class
5 remains in D3, transfer ordering uses declared priority, local admission may
use effective priority, and no unapproved wire interpretation is introduced.

## Files and changes in this checkpoint

- Added this handoff only.
- `specification/` was not modified.
- No existing dirty file was staged or edited.
- No Rust behavior was changed because the missing semantics are normative,
  not an implementation bug that can be fixed without guessing.
