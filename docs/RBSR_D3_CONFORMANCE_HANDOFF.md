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

## Re-audit evidence (2026-08-09)

This gate was rechecked against the current tree rather than relying on the
earlier review. The ambiguity remains, and the following observations make the
boundary executable for the next implementer:

* `mmp-core/src/session.rs` keeps a private `DOMAINS: [u8; 4] = [0, 1, 2, 3]`
  (lines 20–23). `valid_recon_domain` accepts only those four values plus the
  reply flag (lines 45–47), and `start_domain`/`maybe_advance_domain` advance a
  single cursor through that array (lines 621–647). There is no priority byte
  in `ReconInit`, `ReconRange`, or `ReconIds` state. This is a faithful
  implementation of the currently executable D0–D3 subset, not evidence that
  a hidden priority partition already exists.
* `mmp-core/src/rbsr.rs` passes only `self.domain` to `IdSource::ids_in` (the
  trait at lines 173–180 and calls at lines 206, 232, and 316). The RBSR
  fingerprint itself is over the selected IDs and count; it has no place to
  carry declared priority. Adding a second reconciler without defining a
  range/partition identifier would therefore make two implementations emit
  different transcripts for the same wire bytes.
* `mmp-node/src/lib.rs` builds `SyncView` entries as `(BundleID, domain)` and
  maps class 0/3 to D0, direct class 1 to D1, class 2 to D2, and all remaining
  classes (including class 5) to D3 (the `SessionHost::ids` implementation,
  around lines 2808–2821). Declared priority is intentionally not part of this
  key. This is the only safe mapping while doc 18 §4 mandates D3 reuse.
* Class-5 authority handling is present, but it is not the missing D3 weight:
  `on_bundle` charges the ordinary origin bucket before storage (around lines
  2898–2970), while `handle_announcement` performs a post-store, receiver-local
  AuthCert token-bucket decision and updates `effective_priority` (around lines
  1628–1690). No reserved D3 quota, multiplier, or deterministic scope-weight
  formula is applied. Treating this existing local elevation as RBSR
  partitioning would violate doc 18 §5.2's declared-priority rule.
* `mmp-router/src/lib.rs::transfer_order` sorts by declared `priority`, while
  `eviction_order` sorts by receiver-local `effective_priority` (around lines
  99–145). The existing regression test
  `transfer_order_uses_declared_priority_eviction_order_uses_effective` locks
  this asymmetry. It is valuable evidence, but it does not resolve how RBSR
  sets are partitioned.

### Reopening checklist (must be answered by an approved protocol decision)

The gate may be reopened only after the decision records all of the following
in one normative source (or an approved erratum), with byte-level examples:

1. A canonical partition identifier for `(base domain, declared priority)` or
   an explicit statement that priority partitions are advisory and not an RBSR
   key. If the existing D3 byte is reused, define how a peer distinguishes the
   partitions without adding an unnegotiated value.
2. The exact legacy-peer behavior: transcript and phase order when the peer
   implements only D0–D4 and does not know the new partition rule. Include the
   downgrade/ignore behavior, not just the upgraded path.
3. A deterministic D3 admission rule, including whether “weight” changes
   ordering, admission probability, a bounded reserve, or a bucket charge;
   specify units, duplicate accounting, refill, scope matching, certificate
   failure/expiry, and the maximum state per subject/scope.
4. A starvation invariant for D0/D1/D2 and a per-receiver abuse bound that
   remains true when peers lie about declared priority or relay the same
   BundleID through many paths. The invariant must not depend on
   `effective_priority` being equal on two receivers.
5. Golden vectors for every base domain and declared priority 0–3, including
   exact `RECON_*` bytes, order, fingerprint inputs, and a legacy/upgraded
   interop transcript. Add property tests for duplicate, expired, revoked, and
   unsubscribed announcements before enabling a new default.

Until these answers exist, no production Rust change is justified. A test-only
assertion of current D0–D3 behavior or another resource bound is safe; a new
partition cursor, domain value, or D3 quota policy is not.

## External release gate (manual PC/mobile procedure re-audit, 2026-08-09)

The manual procedure in `docs/MANUAL_INTEROP_TESTING.md` is useful for the
Phase-A radio gate, but it cannot close this RBSR/D3 item. Its successful path
observes handshake, message delivery, receipt return, and reconnect behavior;
it does not expose the per-priority RBSR sets or a negotiated partition key.
Therefore a report MUST NOT label a successful PC BLE or mobile foreground run
as “RBSR/D3 conformant.” Keep the results as separate gates:

| Gate | Required external evidence | What it proves | What it does **not** prove |
| --- | --- | --- | --- |
| Host loopback | `mmp-cli demo` exits 0 and contains both fail-closed assertions | Rust stack can deliver a message and receipt without radio | BLE, mobile callbacks, RBSR priority partitions |
| PC BLE | Two Linux/BlueZ hosts run `listen` and `connect` with distinct seeds; logs, `inspect` output, adapter/MTU/RSSI, and reconnect trace are retained | Phase-A real-radio message/receipt path | A normative priority partition or D3 admission weight |
| Mobile bridge | Android/iOS consuming app compiles generated bindings and records callback/permission/background evidence | API bridge and platform lifecycle behavior | Background delivery parity, RBSR/D3 semantics, or production battery target |
| RBSR/D3 conformance | Approved erratum/amendment, byte-level vectors, and two independent transcript producers/consumers (one legacy D0–D4 peer) | Interoperable partition and admission behavior | Hardware radio performance (a separate gate) |

The current diagnostic output is not a substitute for the final RBSR/D3
artifact. `mmp-node::DiagLog::dump` is a local, line-oriented journey log with
truncated BundleIDs and Rust `Debug` event formatting; it does not record a
stable domain/priority transcript, and adding such a field before the wire
decision could accidentally make a local policy look normative. A future
release may add a versioned, local-only trace schema and validator, but that
work should begin only after the partition decision fixes the fields to be
validated. Until then, retain raw CLI output and stores as observational
evidence, not as conformance vectors.

### Exact hardware evidence bundle for reopening the external gate

For each real-radio run, attach all of the following to the release record:

1. Commit ID and toolchain (`rustc --version`, `cargo --version`), host OS and
   kernel, adapter model/firmware, and phone model/OS when applicable.
2. The exact commands from `MANUAL_INTEROP_TESTING.md`, with distinct seed-file
   fingerprints (never the seed bytes), timestamps, and process exit codes.
3. Complete `mmp-cli listen`/`connect`/`send` logs, both `mmp-cli inspect`
   outputs, negotiated MTU if available, RSSI samples, elapsed delivery time,
   and a controlled disconnect/reconnect attempt.
4. A gate classification for every run: advertisement, GATT connection,
   identity proof, bundle transfer, receipt, or restart persistence. “BLE
   failed” without the failed gate is insufficient.
5. If a wire/spec discrepancy appears, preserve the raw frame bytes and map it
   to the relevant specification section; do not edit `specification/` to make
   the observed implementation pass.

The release decision remains **blocked on the protocol decision and the
Linux/BlueZ hardware evidence independently**. A successful hardware run may
close only the radio row above; it cannot authorize a guessed RBSR partition or
D3 weighting rule.

## Wire-neutral negative-test audit (2026-08-09)

The current session fencing was rechecked after the D3 transcript regression.
One missing executable boundary was found and added as
`session::rx_bound_tests::recon_init_for_later_domain_closes_without_priority_partition`.
It drives an established responder with an unresolved D0 reconciler, injects a
syntactically valid `RECON_INIT` for D3, and asserts a protocol close. This
prevents an implementation from treating a later base domain as an implicit
declared-priority partition while D0 is unresolved. It does not add a domain,
priority field, or negotiation capability and therefore changes no wire bytes.

The companion test `recon_init_is_role_and_order_fenced` still verifies that a
later base domain is accepted only after the current reconciler reports done;
the existing RBSR D3 transcript test verifies that generated D3 frames retain
base domain `3` and vary only by the existing reply flag. Focused mmp-core tests
and Clippy pass for this checkpoint.

The normative blocker is unchanged: the frozen documents require RBSR per
declared priority class but do not define a byte-level partition key or
negotiation, while the authority-announcement document requires class-5 traffic
to reuse D3. Until an approved erratum states whether priority is advisory or
wire-partitioned (and defines legacy-peer behavior, D3 admission weighting,
starvation and abuse bounds, and golden transcripts), no further production
RBSR/D3 implementation is justified.
