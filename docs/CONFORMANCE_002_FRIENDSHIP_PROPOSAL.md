# CONFORMANCE-002 — Friendship claim and signing-domain proposal

Status: **PROPOSAL — no normative specification change applied**

This document records the demonstrated mismatch between the frozen MMP/1
documents and the current Rust implementation. It is an implementation and
interoperability decision record, not a replacement for
`specification/10-message-format.md`, `specification/06-sync-spec.md`, or
`specification/08-crypto-spec.md`.

## 1. Finding

The frozen text is not byte-complete:

* `specification/10-message-format.md` §3.1 defines the `mmp1/friend`
  object as `friend ‖ expiry_hint ‖ peer_role`, but does not define the width or
  enum of `peer_role`.
* The same document §4.4 describes the field-`0x05` body as
  `friend key32 ‖ expiry_hint u32 ‖ both sigs`, without defining the placement
  of the two signatures or how they relate to the outer Claim signature.
* `specification/06-sync-spec.md` identifies field `0x05` as a countersigned
  friendship claim but does not add a complete body grammar.
* `specification/08-crypto-spec.md` requires identity signatures but does not
  resolve the missing byte layout.

The current Rust implementation is deterministic but therefore not compatible
with that unresolved grammar:

```text
Friendship body       = friend[32] ‖ since:u64 LE ‖ countersig[64]  (104 B)
Friend domain object  = owner[32] ‖ friend[32] ‖ since:u64 BE       (72 B)
Claim signed object   = owner[32] ‖ field:u8 ‖ seq:u32 LE ‖ body
Claim wire            = owner ‖ field ‖ seq ‖ u16 body_len LE ‖ body ‖ sig[64]
```

The outer Claim signature is the dependent's `mmp1/claim` signature. The
`countersig` is the patron's `mmp1/friend` signature. This binds the patron
approval to the dependent owner and the friendship timestamp, but it is a
provisional interpretation of the documents.

## 2. Consequence

Two independent implementations cannot currently produce or verify one
canonical Friendship record. A dual parser would not solve the problem:

1. the frozen form has no version/discriminator;
2. several body lengths are consistent with the prose;
3. accepting multiple encodings would violate the one-valid-encoding rule and
   make signature-domain selection ambiguous.

The security boundary also matters. A patron approval that omits the dependent
owner could be replayed for a different dependent. Any adopted grammar must
keep an explicit owner binding in either the signed object or the outer claim
signature.

## 3. Decision options

### Option A — clarify the shipped Rust representation (recommended if no
external MMP/1 implementation has deployed)

Approve an erratum that makes the existing bytes normative:

```text
field 0x05 body = friend key32 ‖ since u64 LE ‖ patron countersig sig64
mmp1/friend object = owner key32 ‖ friend key32 ‖ since u64 BE
outer Claim signature = dependent signature over mmp1/claim(owner ‖ field ‖ seq ‖ body)
```

This preserves existing stored claims and implementation behavior. It still
requires the exact wording, signature ownership, and endianness to be added to
the normative documents before Friendship golden vectors become normative.

### Option B — introduce a versioned migration

Keep the current representation as a provisional deployed form and define a
new version or explicitly allocated field in a separately approved protocol
revision. The new grammar must include a discriminator, exact widths, both
signature positions, owner binding, and a migration/expiry policy. No new field
or version is selected by this document.

### Option C — implement the prose literally

Do not select this option yet. The prose is missing enough information that a
literal implementation would require inventing `peer_role`, signature
placement, and the relationship between the outer and inner signatures.

## 4. Required approval and follow-up

Before changing the normative specification or publishing a normative vector,
the project owner must choose Option A or Option B. After approval:

1. update the affected normative documents in one reviewed change;
2. add positive and negative byte-exact vectors, including cross-domain and
   owner-substitution failures;
3. define whether old provisional claims are accepted, migrated, or rejected;
4. update the independent conformance reader and the status manifest; and
5. run the mandatory wire, cryptographic, security/regression, property, and
   end-to-end tests.

Until then, the current Rust representation remains a **provisional local
contract**. It is tested for internal stability, but must not be advertised as
cross-implementation Friendship interoperability.

## 5. Evidence

The finding is tracked as `CONFORMANCE-002` in
`docs/E5-S4_CONFORMANCE_DESIGN.md`. The current implementation tests cover
strict round-trip, truncation/trailing-byte rejection, and the exact
owner/friend/timestamp signing preimage. Those tests prove the Rust behavior;
they do not prove agreement with the unresolved frozen prose.

**State:** `DEFERRED-WITH-REASON` — external decision is required before a
normative wire change.
