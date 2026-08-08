# Candidate Group-Control Security Audit

## Status

The candidate `implementation/mesh-milk/crates/mmp-types/src/group_control.rs`
is not production protocol code and must not be staged or integrated as-is.
The frozen specification does not define a wire schema for these structs.
This audit is evidence for the implementation backlog; it does not amend the
specification.

Normative boundaries checked:

- `specification/06-sync-spec.md` §8 defines membership operations, encrypted
  channel placement, epoch anchors, and all-admin checkpoints, but no byte
  encoding for a control batch or bootstrap invite.
- `specification/08-crypto-spec.md` §6 requires sender-key distribution over
  pairwise channels and fresh epoch keys; removed members receive no new keys.
- `specification/10-message-format.md` §3.1 registers nine signing domains and
  does not register `GroupControl`, `GroupGrant`, or `GroupBootstrap`.
- `specification/03-protocol-spec.md` requires authenticated input before
  durable state changes and AEAD protection for post-auth control.

## Findings

1. **Critical — unauthenticated bootstrap invite.** `BootstrapInvite` carries
   no authenticated outer envelope, replay state, or recipient-bound
   signature/AEAD. `validate_bindings` checks field equality only, so forged
   recipient, nonce, and zero-signature values can pass structural validation.
2. **Critical — signatures are parsed but never verified.** Membership ops,
   anchors, grants, batches, and bootstrap fields accept arbitrary `Sig64`
   bytes. No production consumer currently verifies these candidate types.
3. **High — plaintext, unbound sender-key seed.** `SenderKeyDist` contains a
   raw chain seed, while `SenderKeyGrant::signed_object` does not bind that
   seed. A modified seed can preserve the grant and cause decryption or
   availability failures; an unwrapped invite exposes key material.
4. **High — signer and admin-slot binding is absent.** Slot numbers are not
   bound to the sorted admin key set, coordinator slots are unconstrained, and
   batch signer identity is not encoded. Any listed admin can be represented as
   another slot.
5. **High — anchor signature and acknowledgement role confusion.** The two
   signed objects are byte-identical and have no role discriminator or separate
   registered domain, allowing replay as a different checkpoint message.
6. **High — batch replay and canonical-order checks are missing.** There is no
   duplicate operation check, per-signer replay/frontier state, ordering rule,
   or anchor validation despite comments claiming canonical order.
7. **High — aggregate resource and length risks.** Per-item limits allow a
   batch of hundreds of large nested operations without an aggregate bound;
   several encoders cast lengths to `u16` without returning an error. This can
   create non-decodable local encodings and memory/CPU pressure from remote
   input.
8. **High — anchor root semantics are disconnected.** The candidate root is
   not computed or verified over members, tags, tombstones, and frontier. The
   current production OR-Set root hashes member IDs only, which is insufficient
   for the checkpoint semantics in §8.3–§8.5.
9. **High — bootstrap completeness is not enforced.** Initial state, grant
   uniqueness/coverage, coordinator selection, previous-anchor rules, and one
   sender key per member are not validated.

## Safe remediation boundary

No exported candidate wire type is safe to integrate as a conformance surface.
Keep the file and the added candidate domain variants uncommitted. The current
pairwise sender-key path may remain as an implementation compatibility path,
but it needs explicit session binding, sender/epoch authorization, and replay
state before it is described as group-control conformance.

If a protocol amendment is approved, implement in this order:

1. Define one canonical control schema and register its signing domain(s).
2. Bind signer key, admin slot, operation ID/sequence, group, epoch, and
   recipient in an authenticated envelope.
3. Define replay, ordering, duplicate, frontier, and aggregate-size limits
   with a hard authenticated bound no larger than the bundle/frame limits.
4. Separate anchor and acknowledgement roles with distinct preimages/domains;
   define and verify the complete member/tag/tombstone/frontier root.
5. Add cryptographic vectors, malformed-wire negatives, and state-machine tests
   before wiring the messages into `mmp-core` or `mmp-node`.

Until those decisions exist, changing the specification or promoting the
candidate code would create a second, incompatible protocol surface.
