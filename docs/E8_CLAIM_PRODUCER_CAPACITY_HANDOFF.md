# E8 claim-producer capacity handoff

Status: implemented bounded increment; no wire-format or specification change.

## Scope

This increment makes ordinary local claim publication fail closed when the
bundle provider rejects the durable write. The implementation is deliberately
narrow: it does not claim that every producer in `mmp-node` is transactional.

## Implemented behavior

- Claim size is checked before signing, including opaque/unknown fields. The
  exact claim and bundle wire lengths must fit `MAX_BUNDLE`.
- `try_publish_claim_now` gossips the signed claim through `try_store_own`
  before adopting it in `ClaimStore`. A provider rejection returns
  `ClaimPublishError::StoreRejected`; claims, `SeenIndex`, diagnostics, and
  event state remain unchanged.
- Nickname publication maps malformed/oversized input to `TooLarge`, local
  identity revocation to `OwnerRevoked`, and durable capacity failure to
  `ResourceLimit`.
- Friendship acknowledgements publish the friendship claim before consuming
  `pending_offers`/`offered_since` or inserting patron state. A failed write
  leaves the offer retryable.
- FFI maps lifecycle and resource failures to `FfiError::Internal` rather than
  reporting them as caller-controlled `BadArg` input errors.

## Regression evidence

Focused tests cover:

1. regular claim publication rejected by a capacity provider;
2. opaque oversized claims rejected before signing or storage;
3. nickname error mapping at the FFI boundary, including a bounded snapshot;
4. friendship-ack rejection preserving retry state; and
5. receiver ingest rejection after authenticated bootstrap claims are already
   available.

The receiver test constructs the node with a normal provider, completes the
identity exchange, and only then swaps in a rejecting provider. This isolates
ingest admission from constructor bootstrap and avoids conflating two failure
conditions.

## Deliberate residuals

- `Node::new` keeps its source-compatible `()`-style construction and therefore
  cannot report a bootstrap claim persistence error. On a provider rejection,
  the fixed exchange/prekey claims remain available in the in-memory claim
  view for local discovery, but are not marked durable or inserted into
  `SeenIndex`; hosts requiring persistence must use a provider that can accept
  the fixed claims.
- `revoke_identity` and other legacy `Vec<u8>` claim wrappers can return an
  empty vector on provider rejection. A typed revocation API is still needed
  if callers must distinguish that failure without interpreting the bytes.
- The valid-prekey first-contact and established-conversation direct branches
  of `send_app` are now staged: candidate ratchet/conversation state commits
  only after `try_store_own` succeeds. A rejecting provider leaves the live
  sender chain, DAG, pending markers, SeenIndex, events, diagnostics, and sent
  history unchanged, and a retry reuses the original epoch/index.
- The self-originated `send_group` path now stages an opaque local sender-key
  candidate and commits it only after `try_store_own` succeeds. Stale or
  cross-key candidates are rejected by the crypto API.
- Receipt publication now calls `try_store_own` and is fail-closed at the
  provider boundary; a rejection does not undo authenticated delivery, but
  the current API has no durable pending-receipt retry journal.
- Per-member distribution/rekey transaction and crash semantics, receipt retry
  durability, and aggregate file-symbol/manifest batches still need their own
  reservation or batch-commit design; replacing every `store_own` call blindly
  would create new consistency bugs. The current file producer is safer but
  intentionally resumable: each accepted symbol is indexed only after durable
  admission, while an accepted prefix can remain after a later rejection.
- `SnapshotStore` admission covers the complete image, including auxiliary
  records. A stale auxiliary record can still make a valid bundle fail closed;
  this is intentional until provider reservation semantics are designed.

## Verification and handoff

Run the focused `mmp-node` and `mmp-ffi` tests before changing this increment,
then run the normal local fast check. The repository has pre-existing
rustfmt drift; `cargo fmt --all -- --check` may report that baseline debt even
when the affected crates compile and their tests pass. No file under
`specification/` was modified.

Recommended next step: design and stage group-key distributions/rekeys, a
bounded pending-receipt retry/reserved-control policy, and file symbol
producers separately. Their multi-object transaction boundaries remain
different; do not infer full storage atomicity or exactly-once behavior from the
pairwise/group message results.

## 2026-08-10 continuation: typed self-revocation result

Node::try_revoke_identity and
Node::try_revoke_identity_with_successor now expose
Result<Vec<u8>, ClaimPublishError>. The legacy Vec<u8> wrappers remain
source-compatible and delegate to the typed methods, preserving their
historical empty-vector-on-failure behavior. A focused regression proves that
provider rejection leaves claims, SeenIndex, events, and diagnostics
unchanged; a successful typed call decodes the revocation and a second call
fails with OwnerRevoked.

This additive node API does not change the FFI surface or the wire claim. The
FFI wrappers still require an explicit error-surface decision before exposing
typed failure to mobile callers.
