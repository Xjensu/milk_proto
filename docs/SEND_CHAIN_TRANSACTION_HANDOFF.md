# SendChain transaction checkpoint handoff

Status: implementation audit, 2026-08-09. This note is non-wire guidance; the
frozen `specification/` tree is unchanged.

## Result

`mmp_crypto::ratchet::SendChain` already exposes a narrowly sufficient
checkpoint/rollback primitive for a future own-origin storage transaction:

```text
checkpoint = send_chain.snapshot()?;
... derive key / build candidate ciphertext ...
if provider rejects:
    send_chain = SendChain::restore(&checkpoint)?;
```

The checkpoint is a `Zeroizing<Vec<u8>>`, `restore` performs strict version,
length, and counter validation, and no wire fields are involved. The new
regression `send_chain_snapshot_restore_replays_uncommitted_key_after_rejection`
proves that a rejected candidate can be rolled back without skipping the
message key, epoch, or index.

## Required use constraints

1. Take the checkpoint immediately before `SendChain::next()` (or before an
   equivalent chain mutation). Never checkpoint after consuming the key.
2. Treat the checkpoint as secret provider-local material. Keep it in the
   `Zeroizing` buffer, do not expose it through FFI, and authenticate/encrypt
   it if it crosses a persistence boundary.
3. Refuse the transaction if `snapshot()` fails at an epoch/index boundary;
   do not call `next()` and hope that rollback can recover a non-advancable
   state.
4. Restore only from the checkpoint created by the same in-memory chain. The
   byte parser is strict but does not authenticate arbitrary caller bytes.

This seam solves only the pairwise send-chain component. It does not make
`NodeCore::store_own` transactional: `pending_async`, `pending_first_contact`,
conversation DAG heads, delivery tracking, and SeenIndex need coordinated
rollback as well. `LocalSenderKey` for group messages has no equivalent
checkpoint/restore seam, and file-symbol indexes can be partially populated.
The reserve/commit provider contract described in
`docs/OWN_BUNDLE_STORE_TRANSACTION_HANDOFF.md` remains required before
propagating `BundleStore::try_put` errors through all own-origin producers.

## Verification

```text
cargo test -p mmp-crypto send_chain_snapshot_restore_replays_uncommitted_key_after_rejection
cargo test -p mmp-crypto --lib -- --test-threads=1
cargo check -p mmp-crypto --no-default-features
```
