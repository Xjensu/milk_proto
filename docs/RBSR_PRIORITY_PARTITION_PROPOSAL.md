# RBSR priority-partition proposal

Status: proposal only, 2026-08-12. This document does not modify the frozen
`specification/` tree, add a wire field, or change the MMP/1 implementation.
It records a concrete interoperability blocker found while evaluating E5-S5.

## Decision boundary

The current implementation is intentionally **NO-CHANGE-JUSTIFIED** for the
existing MMP/1 wire contract. Full priority-class reconciliation is
**DEFERRED-WITH-REASON** until the protocol owner approves an erratum or a
new negotiated version.

The reason is not a missing Rust loop. The normative documents describe two
different executable contracts:

| Source | Normative statement | Implemented/frozen consequence |
| --- | --- | --- |
| `specification/03-protocol-spec.md` §5.2 | RBSR runs per priority class, highest first | Requires a peer-visible partition selector |
| `specification/06-sync-spec.md` §2 | D0–D4 domains; relay inventory is one D3 domain | D3 has no priority subdomain |
| `specification/10-message-format.md` §2.4 | RBSR frames carry `domain:u8` plus the reply flag | No priority-class field exists |
| `specification/18-authority-announcements.md` §§4, 5.2 | Class 5 stays in D3; partition key is declared priority, but no new domain/capability is added | Legacy peers must continue to understand D3 unchanged |

The Rust implementation matches the safe executable boundary: `DOMAINS` is
`[0, 1, 2, 3]`, `IdSource` is keyed by the domain byte, `SyncView` assigns
each bundle to one base domain, and all D3 frames retain base domain `3`.
Existing regressions explicitly reject using a later domain as an implicit
priority partition and lock the one-domain D3 transcript.

## Why an implicit implementation is unsafe

1. Reusing values `4..` for priority classes would make a legacy peer reject
   an otherwise valid `RECON_INIT`; the frame validator intentionally accepts
   only D0–D3 plus the reply flag.
2. Running several local `Reconciler`s while keeping the wire domain at `3`
   gives the peers no authenticated way to know which declared-priority set
   a fingerprint represents. The peers can compare different sets and waste
   the contact or request incorrect IDs.
3. Feeding `effective_priority` into the partition would be incorrect. It is
   receiver-local and can differ between honest peers because AuthCert views
   and token buckets are local. The signed declared priority is the only
   stable candidate, as §18.5.2.2 states.
4. Reordering `TRANSFER` alone would improve local scheduling but would not
   implement per-class RBSR reconciliation. It must not be reported as E5-S5
   completion.

## Candidate resolutions for an approved revision

### A. Negotiated priority-partition extension (recommended for a future
revision)

Define a new capability/version negotiation that explicitly identifies a
priority partition selector and its valid range. Only peers that negotiate it
would open the additional passes. The selector must be carried consistently
in `RECON_INIT`, `RECON_RANGE`, and `RECON_IDS`, and it must be included in
the state-machine ordering and transcript tests. Legacy peers continue using
the existing single D3 pass.

Required compatibility rules:

- no upgraded peer sends an extension frame to a peer that did not negotiate
  it;
- fallback to the current single D3 reconciliation is deterministic;
- declared priority, never receiver-local effective priority, selects a set;
- a failed or interrupted priority pass cannot skip the remaining legacy D3
  inventory unless the negotiated state machine explicitly permits it;
- the extension receives new golden vectors, malformed-selector rejection,
  downgrade/fallback tests, and mixed-version session tests.

This option requires an approved wire/version change. It is not implemented
in this proposal.

### B. Keep MMP/1 single-pass D3 and improve local transfer ordering

This is the current implementation boundary. A host may sort requested
handoffs using declared priority, destination, size, lifetime, and local
utility, while RBSR remains one D3 set. It is wire-compatible and useful, but
it is a scheduling optimization—not priority-class RBSR conformance.

### C. Add a new unnegotiated domain value

Rejected. It would create an interoperability failure with rev-A peers and
contradict the explicit zero-new-domain/capability decision in
`specification/18-authority-announcements.md` §4.

## Required evidence before implementation

An approved revision must provide:

1. exact frame layouts and capability/version negotiation rules;
2. a legacy fallback transcript;
3. declared-priority partition invariants and ordering examples;
4. malformed, duplicate, downgrade, and mixed-version security tests;
5. airtime and storage-cost measurements for the additional RBSR passes;
6. updated independent golden vectors and a conformance-reader change.

Until those artifacts exist, the correct production behavior is the current
single D3 pass with bounded RBSR state. The proposal deliberately preserves
the frozen wire protocol and prevents an implementation from silently
claiming a stronger interoperability guarantee than the bytes provide.

## Verification evidence for the current boundary

```text
cargo test -p mmp-core rbsr --lib --locked                         9 passed
cargo test -p mmp-core session::rx_bound_tests --lib --locked      21 passed
cargo clippy -p mmp-core --all-targets --locked -- -D warnings     passed
```

No file under `specification/` was changed for this proposal.
