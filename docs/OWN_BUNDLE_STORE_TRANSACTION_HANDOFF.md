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
| `send_app` | `SendChain::next()` advances before `store_own`; `pending_async` is removed before the write; DAG/pending state follows it | A write rejection after encryption requires restoring the consumed send key/index and the pending async marker. `SendChain` has a snapshot reader but no in-place rollback transaction. |
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

## Verification performed

```text
cargo test -p mmp-node --lib -- --test-threads=1  # 80 passed
cargo clippy -p mmp-node --all-targets -- -D warnings  # passed
```

The audit made no protocol, specification, routing, or FFI wire changes.
