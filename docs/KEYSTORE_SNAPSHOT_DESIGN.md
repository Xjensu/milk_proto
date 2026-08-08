# Keystore-backed secret-state design (implementation plan)

Status: partially implemented primitive, no wire change, not yet wired into
`mmp-node` or UniFFI.
The frozen `specification/` directory is intentionally unchanged.

## Current boundary

Commit `51e00eb` adds provider-local, plaintext codecs for `SendChain`,
`RecvChain`, and `PrekeyStore`. They are strict and bounded, but they are not
encryption, freshness, rollback, or atomicity mechanisms. They must never be
written directly to `SnapshotStore` or exposed through UniFFI/React Native.

The existing `MMPSNAP2` store is an integrity/checksum container for
receiver-local non-secret state. A checksum does not provide confidentiality or
rollback protection. Secret ratchet/prekey continuity therefore needs a
separate vault transaction.

Commit `c6685c0` adds `mmp-crypto::snapshot`: a bounded ChaCha20-Poly1305
envelope with a fresh injected nonce, context-hash AAD, version/kind checks,
exact framing, and caller-supplied trusted-clock freshness validation. It does
not enforce durable generation rollback, provider key wrapping, or atomic
multi-record recovery; those remain mandatory responsibilities of the next
layer.

Commit `c2edc90` adds the next Rust-internal slice, `mmp-node::secret_state`.
It canonically aggregates pairwise conversation state (peer identity, derived
receipt key, send/receive ratchets, skipped-key windows, and conversation DAG),
first-contact/pending markers, BundleID delivery tracking,
established-conversation bookkeeping, and the current prekey secrets. The
decoder is bounded, rejects trailing or non-canonical input, validates
cross-record references, and constructs replacement state before `Node`
mutates anything. Restore is refused while a live session or queued wire
output exists. The aggregate is still plaintext by design and must be sealed
under the envelope above before any persistence.

This slice deliberately does not claim complete Node restart continuity. Group
sender-key ratchets, one-time-prekey consumption/tombstone metadata, durable
generation/rollback state, provider transactions, and crash recovery remain
unimplemented gates. Group membership/epoch records may continue through the
existing non-secret snapshot path, but group cryptographic material must not be
silently regenerated as if it were restored secret state.

## Required secret-state inventory

One atomic generation must cover all state whose mismatch can break delivery or
causal behavior:

- conversation ID and authenticated peer identity;
- current send and receive chain snapshots, including skipped message keys;
- the derived receipt-MAC key (the implementation derives it once from CK₀ and
  retains it; CK₀ itself is never persisted);
- conversation DAG state needed to preserve outgoing parent heads;
- first-contact/pending markers and BundleID-to-receipt tracking;
- `established_convs`, so the first live post-restore contact performs the
  required epoch bump exactly once;
- signed-prekey/one-time-prekey secrets plus generation, expiry, and consumed
  or tombstoned OTK metadata.

Persisting only chain keys is insufficient: receipts may stop resolving,
queued first-contact messages may overtake root establishment, and causal
parents may disappear. Persisting only prekeys is also unsafe because replaying
an older valid prekey generation can re-expose a previously used OTK.

## Proposed layering

`mmp-node` owns the typed aggregate encoder/decoder and transaction semantics.
`mmp-crypto` owns the typed chain/prekey primitives and the authenticated
envelope implementation. A Rust-internal `SnapshotVault` owns only sealed
records or a platform capability; it is not a UniFFI data type.

The preferred native operation is:

1. Rust creates a fresh random data-encryption key (DEK) and a fresh nonce.
2. Rust AEAD-seals the complete typed plaintext aggregate with AAD containing
   the schema, state kind, local identity public key, node ID, and every
   conversation identity/context covered by the generation.
3. The native provider wraps/unwraps the DEK (or returns an opaque sealed
   handle). It never receives the plaintext ratchet/prekey aggregate.
4. The persisted record contains only a versioned envelope, wrapped DEK,
   nonce, authenticated context, ciphertext, tag, written-at metadata, and a
   monotonic generation.

The existing `mmp-crypto::aead::seal/open` helper uses a deliberately fixed
zero nonce for a single-use message key and must not be reused for snapshots.
The envelope needs an explicit randomized nonce contract and tests proving two
successful writes under the same provider key cannot reuse a nonce.

## Mandatory restore invariants

- Validate the expected identity/node/conversation context before decryption or
  before accepting the plaintext.
- Enforce a durable monotonic generation per identity and state kind. An older
  valid envelope must not roll a ratchet or prekey store backward.
- Enforce freshness using a trusted durable platform clock or a rollback-safe
  provider timestamp. `Node::tick`/`now_ms` resets after process death and is
  not sufficient for a 72-hour restart TTL.
- Commit conversation, prekey, pending, receipt, and causal state atomically;
  a crash must restore either the previous complete generation or the next
  complete generation, never a mixture.
- Reject malformed, oversized, truncated, tampered, wrong-context, stale,
  future/rollback-invalid, or partially committed records without mutating live
  state. There is no cleartext fallback.
- On a valid restore, the first subsequent live session performs one epoch bump
  for each restored established conversation. A fresh first-contact conversation
  must not bump prematurely.
- Missing, locked, invalidated, or rollback-unsafe platform storage is a
  capability failure. Non-secret records may still persist, but secret state is
  treated as unavailable.

## API boundary rules

The existing `MmpNode::new(seed, tier, store_path)` remains source-compatible.
Any vault-enabled construction is additive and Rust-internal until native
Android/iOS adapters exist. No UniFFI or JavaScript method may accept or return
CK₀, chain keys, skipped keys, prekey secrets, plaintext snapshots, wrapping
keys, or provider logs containing them. The host may receive only non-secret
capability/status and ordinary protocol events.

## Acceptance matrix before wiring

The implementation must add tests for:

- typed aggregate round-trip with receipt verification and DAG parent
  continuity;
- first-contact pending state and `Delivered` transition after restart;
- skipped-key/out-of-order decryption after restart;
- prekey generation, OTK consumption/tombstones, and rollback rejection;
- fresh nonce, AAD/context binding, wrong identity, tamper, truncation, size,
  version, stale/future timestamp, and generation rollback;
- atomic crash points around write, fsync, replace, and recovery;
- one post-restore epoch bump, no bump for a new conversation, and no state
  mutation after any failed restore;
- native provider unavailable/locked/invalidation behavior;
- generated UniFFI/React Native surfaces containing no secret-state types.

Only after this matrix passes should an opaque sealed record be added to the
host persistence flow. This design is intentionally a gate, not a claim that
restart continuity is already implemented.
