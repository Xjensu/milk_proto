# Future Authenticated Group-Control Envelope

**Status: PROPOSAL-ONLY / non-normative.** This document does not amend
`docs/spec/`, promote `mmp-types::group_control`, or claim production
readiness. It records a candidate design for review before any wire or state
machine change is authorized.

## 1. Scope and evidence

The proposal addresses three open implementation gates identified in
[`../audits/group-control.md`](../audits/group-control.md):

1. late-joiner confidentiality;
2. confidentiality of messages sent by a remaining non-admin after removal;
3. bounded admission of authenticated future-epoch state.

The evidence baseline is the frozen text of sync section 8, crypto section 6, and message
format section 3, together with the current `mmp-node` group path. Sync section 8 requires
admin-signed membership operations, an epoch anchor, and an epoch increase for
every effective membership change. Crypto section 6 requires fresh sender keys for a
new epoch and no distribution to a removed member. Message format section 3 currently
registers no group-control, grant, or bootstrap signing domain. The current
pairwise `SenderKeyDist` body contains a group, epoch, chain seed, and signing
key, but no authenticated admin anchor or recipient binding.

## 2. Confirmed current defects

These are implementation/specification gaps, not merely hypothetical risks:

- The quarantined bootstrap candidate has structural checks but no implemented
  signature verification, recipient-bound AEAD, or replay state. It must not be
  used as a conformance surface.
- The current invite path distributes the inviter's current-epoch key before
  adding the member and does not perform the epoch transition required by
  crypto section 6.3. A late joiner can therefore receive current-epoch material
  without a defined non-retroactive bootstrap boundary.
- A current member can send a syntactically valid future-epoch distribution.
  The receiver can learn it without an authenticated anchor proving the
  membership transition; retaining one entry per `(member, epoch)` also has no
  protocol-level future-epoch budget.
- After accepting an administrator's future-epoch distribution, the current
  non-admin path does not itself advance its local epoch and sender chain.
  A later message from that member can consequently remain in the old epoch.
- Candidate anchor and acknowledgement objects currently share the same signed
  object shape and do not have a role-separated signing preimage. Candidate
  operations also lack a production consumer that verifies signer authorization,
  sequence/replay state, and aggregate limits.

Existing local fail-closed checks (authenticated origin binding, stale/non-member
rejection, duplicate `(member, epoch)` rejection, and bounded unknown-group
bootstrap) remain useful compatibility hardening. They do not solve the three
gates above.

## 3. Requirements for a future control protocol

Any approved design MUST:

1. authenticate the administrator and bind the administrator key to the
   immutable admin set and slot rules in sync section 8.4;
2. bind every membership operation to one group, base epoch/anchor, operation
   identifier, signer sequence, and canonical operation bytes;
3. make an effective membership change and its resulting epoch unambiguous;
4. deliver sender-key material only to an authorized recipient and only for the
   resulting epoch (never to a removed member or backward to a late joiner);
5. let a non-admin atomically install an authenticated future epoch before it
   can publish a new-epoch group message;
6. reject stale, duplicate, skipped, conflicting, or unauthenticated controls
   without mutating durable membership or key state;
7. impose aggregate bounds on operations, recipients, nested lengths, and
   future-epoch admission before allocation or storage;
8. preserve the sync section 8 partition behavior: removal takes effect as knowledge
   reconverges, not by pretending that disconnected nodes have global time;
9. provide canonical bytes and negative vectors before implementation wiring.

## 4. Candidate envelope and schema (not normative)

The preferred shape is one authenticated control envelope carried inside the
already-authenticated pairwise channel. The channel supplies confidentiality
for control traffic; the envelope supplies durable authorization, replay, and
cross-session binding.

Candidate fixed fields, in canonical order:

```text
version                 u8
kind                    u8  (bootstrap, membership batch, epoch anchor, key grant, ack)
group_id                id16
recipient_node_id       id16 or zero for a group-wide control
base_epoch              u16
result_epoch            u16
base_anchor_hash        hash32
control_sequence        u64    (per authorized signer)
control_id              id16   (hash of unsigned canonical object)
signer_slot              u16
signer_identity_pubkey   key32
body_len                 u16
body                    canonical kind-specific object
signature               sig64
```

The body would be one of:

- `ADD`: member identity/public-key binding and one OR-Set add tag;
- `REMOVE`: strictly ordered observed add tags;
- `ANCHOR`: result epoch, complete membership/tombstone/frontier root,
  previous-anchor hash, and a role-specific anchor signature;
- `GRANT`: recipient, result epoch, anchor hash, sender signature key, and a
  pairwise-encrypted sender-chain seed; the seed MUST NOT be covered only by a
  detached grant that leaves the seed unbound;
- `BOOTSTRAP`: immutable admin set, initial membership state, initial anchor,
  recipient-bound grants, and a fresh bootstrap nonce;
- `ACK`: an explicitly different acknowledgement preimage referring to an
  anchor hash, not an alias of the anchor signature.

The envelope is a candidate schema, not a declaration of exact widths or frame
type. A future protocol revision must decide whether batches are one signed
object or independently signed operations plus a signed batch manifest. It must
also decide whether grant ciphertext uses the existing pairwise AEAD context or
an additional recipient-bound wrapper. No decision is made here by implication.

## 5. Cryptographic, replay, and resource invariants

### 5.1 Signing and channel binding

Every signed object needs a distinct, prefix-free domain tag (for example,
separate tags for control, grant, anchor, acknowledgement, and bootstrap).
The signed preimage MUST include group, epochs, anchor hash, signer identity,
signer slot, recipient where applicable, sequence, control ID, and the exact
canonical body. Verifiers MUST check strict Ed25519 canonicality as required by
crypto section 10 and verify that the public key derives the claimed node identifier.

The envelope is not a replacement for Noise or `ID_PROOF`: it is accepted only
after the pairwise session is authenticated. The pairwise AEAD associated data
should bind the envelope version, kind, group, epochs, control ID, and recipient
to prevent ciphertext relocation. If a separate bootstrap wrapper is chosen,
its nonce and recipient must be included in both AEAD associated data and the
signed authorization object.

### 5.2 Replay and ordering

Receivers maintain, per group and authorized signer, the highest accepted
sequence plus a bounded set of recently accepted control IDs. A control is
accepted only if its base anchor is known (or is an explicitly bounded bootstrap
case), its sequence is not replayed, its operation ID recomputes correctly, and
its signer is authorized for the immutable admin set. Anchor and ACK roles have
different domains and state transitions. State mutation is transactional:
verify, validate all operations, then commit membership/epoch/key changes.

Future epochs are admitted only through a verified anchor/transition whose
result epoch is the next valid epoch for that control history. Missing epochs
are fetched as bounded control history; arbitrary unanchored jumps are rejected.

### 5.3 Size and admission bounds

Before allocation, implementations must enforce the existing frame/bundle
reassembly limit and tighter control-specific limits: maximum admins,
operations, remove tags, distributions, nested body length, and total encoded
batch size. The aggregate limit, not just each item limit, is authoritative.
Per-group pending controls, future epochs, and replay IDs require finite quotas
with deterministic eviction/rejection behavior. A quota failure must not
partially mutate membership or consume a sender-chain index.

## 6. Migration and compatibility

This proposal cannot be silently deployed as MMP/1. A compatible rollout would
need one of the following approved paths:

- **New protocol version/profile:** register a new control frame/domain set and
  require peers to negotiate it; MMP/1 nodes continue the existing limited
  pairwise path and must not interpret the new controls.
- **Opaque extension under a future bundle class:** only if the frozen registry
  explicitly authorizes the class and defines relay behavior; unknown nodes may
  relay but must not apply state.
- **Out-of-band application migration:** keep groups created by MMP/1 in their
  documented limitations and create new groups under the reviewed control
  profile.

No downgrade may cause a member that has seen a removal to resume publishing
  under an old epoch. A node must never treat a legacy `SenderKeyDist` as proof
  of an admin transition. Existing MMP/1 groups therefore remain a documented
  compatibility mode until all required control semantics and vectors are
  available.

## 7. Required vectors and verification plan

Before any implementation change, the approval package should contain:

- exact encode/decode vectors for every control kind, including empty and
  maximum-size legal bodies;
- signature vectors for each domain and cross-domain rejection vectors;
- AEAD vectors showing recipient, group, epoch, anchor, and control-ID binding;
- replay vectors: duplicate sequence, duplicate control ID, stale anchor,
  reordered batch, conflicting signer, and bounded missing-epoch catch-up;
- confidentiality vectors: late joiner cannot decrypt prior epoch, removed
  member cannot decrypt result epoch, and remaining non-admin can decrypt only
  after installing the verified new epoch;
- malformed-wire/property/fuzz cases for truncation, overlong lengths,
  duplicate fields, non-canonical order, aggregate overflow, and allocation
  failure;
- crash-recovery vectors proving verify-before-commit and atomic persistence.

The existing mandatory test classes (wire/golden, crypto, security/regression,
property/fuzzing, storage invariants, and key end-to-end scenarios) should be
reused and parameterized rather than duplicated across platform wrappers.

## 8. Threat-model limits

The envelope does not provide global consensus, instant removal during a
partition, anonymity, traffic-flow confidentiality, or protection from RF
jamming. A compromised current admin can authorize harmful membership changes;
that is an authority compromise, not a serialization failure. A relay can
withhold valid controls and delay convergence. Offline double-spending of a
routing budget is outside this proposal. These limits must remain explicit in
the security documentation.

## 9. Rejected alternatives for this proposal

- **Promote the quarantined `group_control.rs` unchanged:** rejected because
  structural codecs do not implement signature verification, role binding,
  replay state, or aggregate admission.
- **Require only `dist.epoch == local_epoch + 1`:** rejected because it breaks
  legitimate partition catch-up and still does not authenticate the transition.
- **Accept any higher epoch and prune old state:** rejected because a malicious
  current member can advance state without an anchor and induce memory,
  confidentiality, or availability failures.
- **Use MLS as a transparent replacement:** deferred; sync section 8 explicitly
  documents concurrent partitioned membership changes and the lack of an
  arbiter. A decentralized group-key design may be revisited separately.
- **Add an application-side signature without a registered protocol domain:**
  rejected because it creates an unreviewed second wire/security contract.

## 10. Approval gate

This proposal is ready only for architecture review. Before implementation or
any specification edit, reviewers must approve:

1. the exact envelope/frame placement and version negotiation;
2. the domain-tag registry and canonical preimages;
3. anchor/root and admin-slot semantics;
4. recipient-bound grant encryption and late-joiner policy;
5. replay/frontier rules and finite aggregate quotas;
6. migration behavior for existing MMP/1 groups; and
7. the complete vector, fuzz, crash-recovery, and end-to-end test matrix.

The approval record should identify the accepted wire revision, compatibility
profile, and production gates. Until that record exists, the implementation must
continue to quarantine the candidate module and document the current MMP/1
limitations rather than pretending that this proposal is implemented.
