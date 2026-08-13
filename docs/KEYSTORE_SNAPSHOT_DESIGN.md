# Keystore-backed secret-state design (implementation plan)

Status: Rust-internal primitive and restore seam implemented; a Windows-only
DPAPI adapter now covers the provider transaction contract and the PC CLI can
restore/save it through an opaque host-only facade. Android/iOS adapters,
mobile wiring, and restart/hardware evidence remain open; there is no wire
change and no UniFFI/React Native exposure.
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

The prekey component now uses the metadata-bearing provider-local state codec
added in the follow-up implementation: it carries the private SPK/OTK batch,
generation, local expiry metadata, and idempotent OTK-consumption tombstones.
The frozen signed prekey claim grammar and duplicate-replay behavior are
unchanged; expiry enforcement and key rotation still require a trusted clock
and an explicit lifecycle policy.

### Verification disposition (2026-08-13)

The aggregate restore regression now proves that prekey private material,
generation, expiry, and idempotent OTK tombstones survive a Node snapshot
roundtrip. The tombstone is durable bookkeeping, not a receiver-side replay
fence: frozen MMP/1 §4.2 explicitly permits reuse of an unexpired published
prekey because serverless peers cannot enforce global one-time consumption;
duplicate first-contact handling remains scoped by BundleID deduplication.
Adding a `recipient_agree` rejection for a tombstoned ID would therefore be a
wire-compatible-looking but behavior-incompatible liveness change and is
intentionally recorded as `NO-CHANGE-JUSTIFIED` for this cycle.

This Rust-internal slice does not claim complete platform restart continuity.
The aggregate, vault transaction, prekey tombstone, generation, and rollback
paths are covered by host tests; native Android/iOS provider wiring, group
sender-key secret persistence, crash-point evidence, and physical kill/restart
validation remain open gates. Group membership/epoch records may continue
through the existing non-secret snapshot path, but group cryptographic material
must not be silently regenerated as if it were restored secret state. The
implementation marks a decoded group as requiring one local sender-key
re-gossip; the first post-restore group send schedules distributions before
publishing ciphertext. If that distribution cannot be committed, the marker
remains set and the send fails closed rather than advertising false continuity.

Commit `9357a49` adds the Rust-internal vault seam around that aggregate. A
canonical context binds the pairwise/prekey coverage version, local Ed25519
public key, node ID, and sorted `(conversation ID, peer key)` entries. The
sealed candidate carries its generation; `save_core` generates and zeroizes a
DEK, seals the aggregate, checks the next-generation rule, and calls one
provider commit operation that must atomically wrap the DEK and publish the
record. Loading validates the fixed 72-hour TTL, context, identity-bound
conversation IDs, envelope generation, and a caller-supplied durable
generation floor. The provider trait remains an adapter contract: no durable
high-water mark, crash recovery, trusted platform clock, or native keystore
implementation exists yet.

Commit `9e3a408` adds a test-only reference provider with an explicit durable
high-water floor and atomic failure behavior. Commit `772a59c` then makes the
production-facing seam consume the provider's generation floor and trusted
clock through `save_core_from_vault`/`restore_from_vault`; callers no longer
need to supply an arbitrary wall-clock timestamp or remember the rollback
floor. The reference provider is deliberately confined to tests and stores a
raw DEK only as a test fixture. A real adapter must wrap the DEK in Android
Keystore, iOS Keychain, or an equivalent capability and persist the floor and
record transactionally.

Commit `f6c4c68` adds the Rust-internal Node restore adapter. It checks that
the node is quiescent before asking the provider to load/unlock a generation,
then applies the already validated aggregate in one in-memory swap. A busy
node returns `VaultError::Busy` without touching state; no provider secret or
plaintext crosses the FFI boundary.

The follow-up vault hardening adds a required `SnapshotVault::with_restore_lock`
callback and `restore_from_vault_into`. The provider must hold an
identity-scoped exclusive transaction or lease from the first record read
through the callback's in-memory state application; a default/no-op lock is
not permitted. The reference provider and a lock-observing regression test
prove the callback scope, but they do not provide a cross-process lock. Native
Android Keystore, iOS Keychain, or equivalent adapters must still implement
that guarantee transactionally before restart continuity can be claimed in
production.

The follow-up restore regression covers the lifecycle invariant that matters
after a process restart: identity claims are reintroduced through the ordinary
non-secret bootstrap path, the restored established conversation remains at
its persisted epoch, and the first subsequent authenticated contact advances
both ratchet sides exactly once. This is evidence for the Rust state machine,
not evidence of a native keystore or mobile lifecycle implementation.

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

## Acceptance matrix before complete platform wiring

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

### Host verification update (2026-08-13)

The Rust host slice now covers two restart-sensitive rows explicitly:

- `mmp-node::tests::restored_sender_receives_delivery_receipt_after_first_contact`
  performs a real first-contact transfer, restores the sender aggregate into a
  fresh node, and verifies the subsequent authenticated receipt reaches
  `Delivered`.
- `mmp-crypto::tests::recv_chain_snapshot_roundtrip_preserves_skipped_keys`
  restores a receive chain, recovers the same out-of-order skipped keys, and
  verifies each consumed key remains one-shot after restore.

These are host regression gates, not evidence for native Android/iOS keystore
adapters, crash-point durability, or physical kill/restart behavior. The
opaque sealed-record path may therefore be exercised by the Rust/Windows
provider slice, but complete platform persistence remains gated on the
unverified acceptance items above.

## Windows adapter evidence (2026-08-11)

`mmp-node/src/vault_windows.rs` is a target-gated implementation of the
existing crate-private `SnapshotVault` seam. It uses the maintained
`windows-sys` 0.61.2 bindings only for user-scope DPAPI and the required
handle/time APIs; no generic serializer or key/value store was added. DPAPI
receives a fresh 32-byte DEK and BLAKE3(context) as optional entropy, while the
plaintext aggregate remains inside the Rust seal/open path.

The adapter stores bounded, checksummed journal entries, calls `sync_all`,
retains a durable generation floor during journal compaction, and holds an
exclusive Windows file handle across `with_restore_lock` callbacks. Focused
Windows-host evidence is six adapter tests plus the complete 163-test
`mmp-node` library suite and strict Clippy. The Windows target also passes
`cargo check -p mmp-node --target x86_64-pc-windows-msvc --locked`.

The PC integration is deliberately narrower than a general host API: `mmp-cli`
accepts a separate `--vault <path>` for `connect`, `send`, and `listen`, opens
and restores it before clock priming or traffic, and saves a fresh generation
on every drive-loop shutdown path. The public `mmp-node::WindowsVault` facade
exposes only coarse status/errors; the provider trait, journal records, DEK,
and plaintext aggregate remain private. A fresh vault is treated as an empty
optional capability, while provider failures fail closed rather than being
reported as a successful run.

This does not close E4-S4: the adapter is not wired into the mobile/FFI restart
path, and Android Keystore/iOS Keychain implementations plus kill/restart and
physical-device evidence remain required. The adapter's bounded journal,
provider transaction, and CLI bootstrap/shutdown path are implementation
evidence, not a claim that all production persistence acceptance cases have
passed.
