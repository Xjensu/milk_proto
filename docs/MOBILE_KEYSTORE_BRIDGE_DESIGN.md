# Mobile Keystore Bridge: Bounded Design Review

**Status:** DEFERRED-WITH-REASON

**Scope:** Android Keystore and iOS Keychain integration for the Rust
ratchet/prekey snapshot vault

**Evidence date:** 2026-08-13

**Wire protocol:** unchanged; this note proposes no specification amendment

## Finding

The mobile shell exposes a lifecycle callback named `SecretStatePersistence`,
but the current FFI object has no path that connects that callback to the Rust
secret-state vault. The callback returns only a generation number; it cannot
load or commit a Rust `SealedRecord`, and a host can currently implement it
only as an independent bookkeeping operation. Reporting such a generation as
successful would falsely claim ratchet continuity.

The Rust implementation deliberately keeps the required capability private:

- `mmp-node::vault::SnapshotVault`, `SealedRecord`, and `SealedCandidate` are
  `pub(crate)`;
- `Node::restore_secret_state_from_vault` is Rust-internal;
- `mmp-ffi::MmpNode` exports the seed constructor and ordinary store
  persistence, but no secret-state load/save method;
- generated mobile bindings are checked to reject vault, ratchet, chain-key,
  skipped-key, and prekey-secret identifiers.

This is a real implementation gap, not a missing wire field. The normative
decision in `specification/08-crypto-spec.md §5.3` requires keystore-wrapped
ratchet/prekey persistence on phones and PCs, while the mobile integration is
currently only a fail-closed lifecycle seam.

## Secret boundaries

| Boundary | Current bytes | Assessment |
|---|---|---|
| Native secure storage → `MmpNode::new` | 32-byte Ed25519 identity seed, once, inward | Required by the existing API; never returned by FFI |
| Rust `secret_state::encode` | Local aggregate containing conversation chains, skipped keys, prekey state, pending first-contact state, and delivery bookkeeping | Rust-internal, bounded, zeroizing; not a mobile binding |
| Rust vault provider | Authenticated ciphertext envelope, bounded non-secret context, provider-wrapped DEK; the private `commit`/`unwrap_dek` seam also handles a transient DEK inside the provider implementation | Safe only while the provider remains an internal platform-specific bridge |
| Kotlin/Swift `SecretStatePersistence` | `restore() -> generation?`, `persist() -> generation` | No secret bytes, but currently disconnected from the Rust vault |
| React Native/JS | No seed, ciphertext, DEK, chain key, prekey, or vault handle | Correct and must remain so |

The lifecycle checks (`restore`/`persist` only while stopped) are necessary but
not sufficient. They do not provide a Rust node lock, provider transaction, or
generation CAS around the actual state swap.

## Threat and concurrency analysis

1. **Rollback.** The provider must retain a durable generation floor and reject
   a complete replacement with an older valid journal. The Windows handoff
   records that a filesystem-wide rollback remains undetectable without a
   platform freshness authority; a mobile adapter must make the same decision
   explicitly.
2. **Concurrent restore/save.** `SnapshotVault::with_restore_lock` is required
   to hold the provider transaction through decryption and the in-memory state
   installation. A host callback that only returns a generation cannot satisfy
   this invariant.
3. **Generation freshness.** A caller-supplied or host-maintained generation is
   not a freshness authority. Rust must read the provider floor, enforce the
   contiguous CAS, and re-check the floor after opening the record.
4. **Lifecycle.** Restoring after links, sessions, queued output, or conversation
   state exist is rejected by Rust. The platform shell's stopped/running guard
   must be paired with this Rust-side `Busy` check; it cannot replace it.
5. **Provider failure.** Locked, unavailable, malformed, stale, or rollback-
   rejected records must fail closed. Treating a failed restore as a fresh node
   loses continuity and can wedge existing conversations.

## Safe implementation boundary

Do **not** expose `SnapshotVault`, `SealedRecord`, plaintext snapshots, DEKs,
chain keys, skipped keys, or prekey secrets through UniFFI or React Native.
Do **not** add a public callback whose arguments or return values carry a DEK.
That would turn a private keystore operation into an application-visible secret
boundary.

The smallest future production design is a platform-specific Rust bridge,
compiled with the Android/iOS host library, that owns a non-copyable provider
handle and implements the existing private `SnapshotVault` contract. Its
host-facing operation should be opaque and generation-only, for example:

```text
restoreSecretState(providerHandle) -> generation?
persistSecretState(providerHandle) -> generation
```

The handle must not be serializable or exposed to JavaScript. Within the
Rust/native bridge, the provider may receive only bounded context and
ciphertext plus the transient DEK required by the keystore wrapping call; that
DEK must be zeroized and must never be returned to the application. If the
platform ABI cannot provide this internal bridge without exposing the DEK or a
private trait, the feature remains unavailable and the node must fail closed.

## Required implementation evidence

Before changing the public mobile API, the implementation must provide:

- Android and iOS provider adapters using real Keystore/Keychain operations;
- generation 1 → 2, torn-write, record-loss, complete-journal-rollback, and
  provider-unavailable tests;
- a concurrent restore/save test proving the provider lock covers state
  installation;
- a restart test proving a restored conversation can send and receive after
  the mandatory first-contact epoch bump;
- generated-binding and secret-surface checks proving no forbidden secret
  identifier or byte value crosses UniFFI/RN;
- native SDK/physical-device evidence. Windows tests alone do not close this
  mobile gate.

Until those artifacts exist, the correct status is **DEFERRED-WITH-REASON**:
the current fail-closed seam is safer than inventing a public API that cannot
enforce the vault invariants.
