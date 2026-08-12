# MMP/1 Group-Control Protocol Proposal

**Status:** non-normative proposal; not implemented and not approved for v1

**Protocol impact:** would require an explicit specification amendment. This
document does not change `specification/`, existing wire bytes, or the
quarantined `mmp-types::group_control` candidate.

**Evidence basis:** `docs/GROUP_CONTROL_SECURITY_AUDIT.md`,
`specification/06-sync-spec.md` §8, `specification/08-crypto-spec.md` §6,
`specification/10-message-format.md` §3.1, and the current
`mmp-node` sender-key implementation.

## 1. Why a proposal is required

The current pairwise sender-key path has a concrete late-joiner defect:
`Node::group_invite` sends the inviter's current distribution and only then
adds the member, without advancing the epoch. A new member therefore receives
the old epoch key and can read earlier same-epoch traffic. This conflicts with
the late-joiner rule in doc 08 §6.3.

There are two related correctness hazards:

1. Existing members have no authenticated transition that tells them to mint
   fresh local keys after an add or remove.
2. `SenderKeyDist` has no administrator/anchor proof. Accepting a higher epoch
   from any authenticated member permits unauthorised epoch jumps; requiring
   exact equality breaks legitimate partitioned re-key convergence.

The candidate `group_control.rs` file is not a solution. The audit found
missing signature verification, signer/slot binding, replay frontiers,
aggregate limits, complete membership-root semantics, and safe bootstrap
validation.

## 2. Required security and convergence properties

Any approved design MUST provide all of the following before it is wired into
`mmp-node`:

- Every effective membership change has one unambiguous operation identity and
  advances the group epoch exactly once.
- A late joiner receives only the new epoch's sender-key material and cannot
  derive earlier epochs from the welcome.
- A removed member receives no new-epoch key material after the removal is
  observed by an honest sender.
- Every accepted epoch transition is authorised by the administrator set and
  bound to the resulting membership state.
- Duplicate, stale, skipped, concurrent, and replayed operations are handled
  deterministically without resetting a sender-key receive chain.
- The membership root commits to the exact OR-Set state relevant to the
  checkpoint, including add-tags, removal tombstones, and operation frontier;
  hashing member IDs alone is insufficient.
- All remote lengths, operation counts, grants, and nested payloads have
  aggregate bounds no larger than the existing bundle/session limits.
- A failed decode, signature check, replay check, or provider write leaves live
  group state unchanged.

## 3. Candidate wire architecture

The recommended direction is a new, versioned **group-control envelope** with
two transport forms. The names below are proposal labels, not assigned v1
registries.

### 3.1 Authenticated control commit

An administrator publishes a signed commit containing:

```text
control_version      u8
group_id             Id16
admin_key_id         Key32 or registered admin slot
operation_id         16 or 32 bytes, unique under the admin key
operation_sequence   u64
previous_epoch       u16
new_epoch            u16
operation_kind       ADD or REMOVE
subject              NodeId
result_members_root  [32]byte
result_frontier      bounded canonical digest/frontier
signature            Sig64
```

The signature preimage must domain-separate group control from bundle,
AuthCert, announcement, and ordinary claim signatures. `operation_id`,
sequence, group, both epochs, subject, root, and signer identity must all be
covered. An acknowledgement must use a distinct role and signing preimage;
the same bytes must never be valid as both a commit and an acknowledgement.

The commit is delivered through an authenticated channel. Relays may store and
forward it, but only a member that verifies the administrator authorization,
the previous-root relationship, and the replay frontier may apply it.

### 3.2 Recipient-bound welcome/key package

An ADD requires a recipient-bound welcome encrypted to the invitee's pairwise
conversation. It contains the new epoch, the post-commit membership root, and
only the sender-key grants permitted for that recipient. Each grant is bound to
the group, epoch, recipient, sender identity, and the commit operation. Raw
sender-chain seeds must never be accepted without authenticated encryption and
recipient binding.

After applying a commit, each surviving member generates a fresh local sender
key for `new_epoch` and distributes it through the existing pairwise channel,
but the distribution is accepted only when it references the verified commit.
The removed member is excluded from the grant set.

### 3.3 Bootstrap

The first invite cannot be authorized by a bare epoch-1 sender-key record.
Bootstrap must carry either:

- an administrator-signed initial membership commit plus a recipient-bound
  welcome; or
- an out-of-band verified administrator/membership checkpoint whose encoding
  and trust ceremony are explicitly specified.

TOFU may remain an application policy, but it must not be described as an
administrator authorization primitive.

## 4. Replay, ordering, and resource rules

The receiver should maintain bounded per-group state containing the last
accepted administrator frontier, the current epoch/root, and a bounded set of
recent operation IDs. Rules should be:

1. Reject an operation whose group or signer binding is invalid.
2. Reject a duplicate operation ID without mutating state.
3. Reject a sequence rollback or an operation that skips the authenticated
   frontier unless the protocol explicitly defines a bounded merge proof.
4. Accept a higher epoch only with a verified administrator commit and a
   valid membership-root transition.
5. Retain only a bounded number of previous epochs needed for in-flight
   decryption; define the exact retention horizon before implementation.
6. Enforce both per-item and aggregate limits before allocation, signature
   verification of unbounded nested data, or key installation.

Concurrent administrator changes are the hardest case. The v1 proposal should
choose one explicit policy: deterministic conflict rejection until a common
frontier is exchanged, or a formally specified OR-Set merge with a new signed
result root. Silent last-writer-wins is not acceptable for membership security.

## 5. Compatibility and migration

This proposal is not wire-compatible with the current bare `SENDER_KEY`
bootstrap semantics. A safe migration therefore needs an explicit version and
interoperability policy, for example:

- keep the current path as a legacy mode that cannot claim full group-control
  conformance; or
- introduce a new control version and require peers to negotiate/support it
  before allowing membership changes; or
- freeze group membership changes while allowing legacy read-only epoch traffic
  during a transition.

An implementation must not infer the new semantics from an old `SENDER_KEY`
body, and an old peer must never be tricked into accepting a new control object
as an ordinary application payload.

## 6. Required vectors and tests before approval

Before any code or specification amendment is approved, add exact byte vectors
and negative tests for:

- valid ADD, REMOVE, bootstrap, welcome, and per-member grant packages;
- wrong signer, wrong admin slot, wrong group, wrong recipient, and wrong epoch;
- cross-domain signature and role confusion attempts;
- duplicate, replayed, stale, skipped, and conflicting operations;
- removed-member grant requests and late-joiner attempts to open old traffic;
- malformed lengths, aggregate-limit boundaries, duplicate grants, and trailing
  bytes;
- crash/failure atomicity during commit, key installation, and persistence;
- partition/merge traces with deterministic convergence or explicit rejection.

The mandatory test classes are wire/golden vectors, cryptography,
security/regression, property/fuzzing, storage invariants, and key end-to-end
group scenarios. No test result may be used to claim the proposal is
implemented until two independent implementations agree on the vectors.

## 7. Rejected shortcuts

- **Rotate only the administrator's key:** leaves surviving members on the old
  epoch and does not provide future secrecy from a removed member.
- **Accept any higher epoch:** permits a malicious current member to flood
  arbitrary future epochs and consume bounded receiver state.
- **Require exact epoch equality:** breaks legitimate post-removal convergence
  across a partition.
- **Promote the quarantined `group_control.rs` types:** the security audit found
  unverified signatures, missing replay/role bindings, incomplete roots, and
  unbounded aggregate encodings.
- **Use a distributed ledger:** introduces a coordination requirement that the
  disconnected MMP/1 architecture intentionally does not provide.

## 8. Approval gate

This proposal should remain documentation-only until the project owner
approves, in writing:

1. the control envelope and registered signing domains;
2. the concurrency/partition policy;
3. the migration behavior for legacy peers;
4. the epoch-retention and aggregate-size constants; and
5. the required vector and external cryptographic review scope.

Only after that approval may `mmp-types`, `mmp-crypto`, `mmp-core`, and
`mmp-node` be changed in dependency order. Until then, the correct production
status is **deferred**: the current group path remains an explicitly tracked
compatibility limitation, not a silently upgraded protocol.
