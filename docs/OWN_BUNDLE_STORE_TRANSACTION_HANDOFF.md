# Own-origin bundle storage transaction handoff

Status: bounded security audit, 2026-08-09. This is an implementation
handoff only. No file under `specification/` was changed and no wire behavior
was altered.

## Finding

`NodeCore::store_own` currently calls the legacy infallible
`BundleStore::put` seam (`implementation/mesh-milk/crates/mmp-node/src/lib.rs`)
and then reports the bundle as created. `BundleStore::try_put` was added for
receive-side admission, but own-origin producers do not call it. A fixed-size
provider that rejects `try_put` can therefore expose an asymmetric failure:

```text
Node::send_text / group_send / send_file / publish_announcement
    -> Ok(id) or partial progress
    -> provider retained no bytes
```

The caller may believe that a message exists even though there is no durable
copy to forward. This is a local storage-integrity issue, not a wire
authentication issue.

## Why an immediate `try_put` replacement is unsafe

The own-origin producers do not all have the same rollback surface:

| Producer | State changed before/around storage | Failure consequence |
| --- | --- | --- |
| `gossip_claim`, `emit_receipt` | Claim/receipt bookkeeping is already updated or intentionally fire-and-forget | A rejected auxiliary bundle may be silently lost; changing the return type would alter internal callback contracts. |
| `publish_announcement` | No ratchet, but public API currently returns an ID after `store_own` | Must return a storage error and avoid `Created`/SeenIndex state on rejection. |
| `send_sealed` | Stores first, then creates `Conv` and `pending_first_contact` | Can be made transactional with a returned error, provided the store contract is explicit. |
| `send_app` | `SendChain::next()` advances before `store_own`; `pending_async` is removed before the write; DAG/pending state follows it | A write rejection after encryption requires restoring the consumed send key/index and the pending async marker. `SendChain` now has a tested snapshot/restore checkpoint seam, but no in-place transaction guard that coordinates the other NodeCore fields. |
| `send_group` | `LocalSenderKey::encrypt_with_ad` advances its private chain before `store_own` | A rejection would consume a group message index. The group sender key has no rollback/snapshot seam. |
| `send_file` symbols | Symbols are minted and indexed one by one before the manifest send | A capacity failure can leave a partial own-origin file and stale blob indexes unless the whole operation is transactional. |

Changing `store_own` to return `Result` without solving these transitions would
be worse than the current behavior: it would surface an error while leaving a
pairwise or group ratchet advanced, causing the next valid message to be
undecryptable by the peer. A preflight `total_bytes` check is also insufficient
for arbitrary providers because the `BundleStore` trait does not promise that
capacity remains stable between a check and the write.

## Evidence and residual risk

- Receive-side `mmp-core::ingest` already uses `BundleStore::try_put` and maps
  rejection to `DropReason::StoreRejected`; the node-level regression
  `node_store_rejection_prevents_delivery_and_post_store_side_effects`
  confirms that an inbound rejection does not deliver or index the bundle.
- Own-origin paths still call `store_own` at the producer sites listed above.
- The public `BundleStore::put` method is infallible and has no capacity/error
  result. `mmp-store::SnapshotStore` delegates to the in-memory implementation
  and currently cannot provide a fixed-capacity rejection contract.
- Existing oversized local-input tests prove that wire-size rejection does not
  poison first-contact state; they do not prove provider-capacity rejection.
- `mmp-crypto::SendChain::snapshot`/`restore` is now covered by a regression
  that replays the exact key, epoch, and index after a simulated rejected
  candidate. The checkpoint is zeroizing and provider-local; it is not an
  authenticated arbitrary-input rollback API.
- Group sends now perform an exact placeholder `Bundle` width check before
  `LocalSenderKey::encrypt_with_ad`; an oversized rejection therefore does not
  consume a group sender index. No group-key rollback API was added because
  duplicating the chain seed and private signing key would create key-reuse and
  persistence hazards.
- File sends now preflight a revoked destination before minting class-4 symbols,
  preventing durable symbol/index residue on that deterministic rejection. This
  does not solve provider-capacity failure after symbol minting.
- `ffe4975` hardens the host snapshot format independently of the provider
  transaction problem: loading rejects a checksummed bundle record whose
  `BundleMeta::size` disagrees with the persisted body, and persistence rejects
  record lengths that cannot fit the u32 framing field instead of truncating
  them. These checks protect quota/transfer accounting but do not reserve space
  for a future own-origin write.

## Safe follow-up gate

Before changing own-origin storage, agree on an additive provider contract with
explicit transactional semantics, for example:

1. `reserve(meta, byte_len)` returns an opaque reservation or a capacity error
   without exposing partially written bytes;
2. encryption/ratchet work is performed against a staged candidate state;
3. `commit(reservation, final_bytes)` is atomic, and failure releases the
   reservation without consuming a ratchet index; and
4. rollback/retry behavior is specified for pairwise chains, group sender
   keys, SeenIndex, delivery tracking, and file symbol indexes.

Alternatively, each cryptographic producer must gain a narrowly reviewed
`peek/commit` or snapshot/restore API before `store_own` can propagate a
capacity error. That work is larger than a safe one-file hardening and should
be accompanied by transactional tests for every producer in the table.

Until one of those contracts is approved, keep `store_own` on the legacy
`put` seam and treat fixed-capacity own-origin persistence as an explicit
provider limitation. Do not claim that `BundleStore::try_put` protects local
send APIs.

The audit also identified a separate crash-consistency residual in the current
host implementation: `SnapshotStore::persist` fsyncs the temporary image, then
removes the destination before renaming it. A process kill between those calls
can leave no primary; recovery handles a complete `.tmp` image, but not a
missing temporary file. A Unix-specific atomic-replace plus parent-directory
fsync design and a Windows-compatible replacement strategy require a separate
platform review; they are intentionally not approximated here.

## Transaction-readiness follow-up

The mmp-crypto audit (commit `be61df4`, wording correction `07b57ed`) proves
that a pairwise `SendChain` can be checkpointed before `next()` and restored
after a simulated rejected write without skipping a key. This is useful
building material, but it is not a complete storage transaction: group
`LocalSenderKey`, pending async markers, conversation DAG state, delivery
tracking, and file-symbol indexes still need coordinated commit/rollback.

No additional `BundleStore` preflight method was added. A read-only
`total_bytes`/`meta` check cannot reserve the final ciphertext/BundleID and
would be advisory for providers with different capacity or eviction rules;
adding it would create a false fail-closed claim. The smallest safe next API is
therefore an explicitly transactional provider reservation/commit seam, with
producer-specific staged state, rather than an unreviewed preflight boolean.

## Verification performed

```text
cargo test -p mmp-node --lib -- --test-threads=1  # 82 passed
cargo test -p mmp-crypto --all-targets -- --test-threads=1  # 41 passed
cargo test --workspace --all-targets -- --test-threads=1  # 426 listed, 420 active, 6 ignored
cargo clippy -p mmp-crypto -p mmp-node --all-targets --all-features -- -D warnings  # passed
```

The audit made no protocol, specification, routing, or FFI wire changes.
