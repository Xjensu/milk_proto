# Mobile API Migration: `setNickname` Error Handling

**Status:** implementation migration note (2026-08-09)  
**Scope:** UniFFI consumers on Android/Kotlin and iOS/Swift, plus React Native
wrappers. This note does not change the MMP wire format or any file under
`docs/spec/`.

## What changed

Commit `343cbc5` changed the exported Rust FFI method
`MmpNode::set_nickname` in
`implementation/mesh-milk/crates/mmp-ffi/src/lib.rs` from an infallible
unit-returning method to:

```rust
pub fn set_nickname(&self, name: String, seq: u32) -> Result<(), FfiError>
```

The method now rejects a nickname whose UTF-8 representation is longer than
`NICKNAME_MAX` before taking the node lock, signing a claim, adopting it, or
queueing gossip. `NICKNAME_MAX` is currently 32 bytes in
`implementation/mesh-milk/crates/mmp-types/src/consts.rs`. The cap is measured
in bytes, not Unicode scalar values or grapheme clusters; an exact 32-byte
name is valid, while 33 bytes is rejected.

The direct Rust node API was subsequently hardened by commit `4ca76a2`:
`Node::try_set_nickname` returns `Result<(), SendError>`, and the legacy
unit-returning `Node::set_nickname` delegates to it and fails closed. This
preserves Rust source compatibility while ensuring that the invalid legacy
call has no claim or gossip side effect. The mobile migration concerns the
fallible `mmp-ffi` surface.

## Language-facing behavior

UniFFI projects the Rust `Result<(), FfiError>` into each target language; it
does not expose a Rust `Result` struct verbatim.

| Consumer | Previous contract | New contract | Required caller action |
| --- | --- | --- | --- |
| Rust FFI | `set_nickname(...)` returned `()` | `Result<(), FfiError>` | Handle `Err(FfiError::BadArg)`; do not retry unchanged input. |
| Kotlin generated binding | `Unit` with no declared throwing annotation | `Unit` plus `@Throws(FfiException::class)` on the generated implementation | Call inside the generated UniFFI exception boundary and handle the `BadArg` variant. |
| Swift generated binding | non-throwing `Void` method | `func setNickname(...) throws` (still no success value) | Call with `try` and handle the generated `FfiError` case. |
| React Native TypeScript | `Promise<void>` with no documented nickname rejection | `Promise<void>` that rejects when the native UniFFI call throws | Catch the rejection in JS/TS; the native module must reject rather than resolve on error. |

The Kotlin and Swift source-level requirements are enforced by
`implementation/mesh-milk/scripts/check_mobile_api_surface.py`. Its throwing
manifest currently contains 21 exported `Result<_, FfiError>` methods,
including `setNickname`; it checks Kotlin `@Throws(FfiException::class)` and
Swift `throws` independently of method-name presence.

## Handling `FfiError::BadArg`

`FfiError::BadArg` is the deliberately coarse FFI error for caller-controlled
invalid lengths or malformed encoded input. Its Rust definition and display
mapping are in `implementation/mesh-milk/crates/mmp-ffi/src/lib.rs`.

For a nickname, treat `BadArg` as a validation result:

1. Measure the candidate in UTF-8 bytes before calling native code.
2. If it is over 32 bytes, ask the user to shorten it (or choose a shorter
   replacement). Do not silently truncate: truncation could change the
   user-visible name and is not specified by the wire contract.
3. If the native call still returns `BadArg`, show the same validation state;
   do not perform an automatic retry with the same bytes.
4. Preserve the sequence number until a successful claim publication. A
   rejected call must not consume a sequence number in the host's LWW
   bookkeeping.

The language-neutral intent is equivalent to:

```text
try setNickname(name, seq)
catch BadArg:
    show "nickname must be at most 32 UTF-8 bytes"
```

For client-side preflight, use the platform's UTF-8 byte count rather than a
character-count helper: `TextEncoder().encode(name).length` in JavaScript,
`name.toByteArray(Charsets.UTF_8).size` on Kotlin/Android, and
`name.utf8.count` on Swift. The native check remains authoritative.

## React Native bridge requirement

The checked-in React Native guide (`implementation/mesh-milk/REACT_NATIVE_INTEGRATION.md`)
still shows the historical `setNickname(...): Promise<void>` declaration and
omits several repetitive native methods. This migration note intentionally
does not edit that user-owned document. The TypeScript return type may remain
`Promise<void>`; the semantic change is that the Promise rejects on
`FfiError::BadArg`.

The Android and iOS native modules must keep the existing drive-queue pattern:

```text
post work to the native drive queue
call the generated setNickname
resolve() only after success
catch the generated error and promise.reject(...)
```

Do not call `resolve()` after catching the error, and do not let an exception
escape the native Promise callback. Choose a stable host error code (for
example, a project-specific `MMP_BAD_ARG`) only in the bridge layer; the Rust
FFI enum remains the source of truth and is still `BadArg`.

## Regenerating Kotlin and Swift bindings

Run these commands from `implementation/mesh-milk`. They are the same
pipeline used by the `mobile-bindings` job in `.github/workflows/ci.yml` and
the platform packaging scripts. The library suffix/path must match the build
target (`.so` in the Linux CI example; use the platform-appropriate artifact
for local Android or iOS work).

```text
cargo build -p mmp-ffi --release --locked

cargo run -p mmp-ffi --features cli --release --locked --bin uniffi-bindgen -- \
  generate --library --language kotlin --out-dir bindings/kotlin \
  target/release/libmmp_ffi.so

cargo run -p mmp-ffi --features cli --release --locked --bin uniffi-bindgen -- \
  generate --library --language swift --out-dir bindings/swift \
  target/release/libmmp_ffi.so
```

Run the source-level gates against the freshly generated directories:

```text
python scripts/check_mobile_secret_surface.py bindings/kotlin bindings/swift
python scripts/check_mobile_api_surface.py bindings/kotlin bindings/swift
python scripts/check_ios_lifecycle_contract.py
python -m unittest discover -s scripts/tests -p 'test_*.py'
```

For release artifacts, `scripts/build-android-aar.sh` and
`scripts/build-ios-xcframework.sh` perform the corresponding one-language
generation and `check_mobile_binding.py` checks. These scanners prove source
surface, API annotations, and secret-surface constraints; they do **not** prove
Kotlin/Swift compilation, Android/iOS background execution, BLE behavior, or
physical-device interoperability.

## Evidence in the repository

- `343cbc5`: FFI rejection and no-side-effect regression for an oversized
  nickname.
- `4ca76a2`: direct Rust `try_set_nickname` validation and legacy fail-closed
  behavior.
- `1adffd3`: source-level Kotlin/Swift throwing-ABI checks.
- `1bf6401`: source-derived test that keeps the throwing manifest aligned with
  the Rust `Result` exports.
- `implementation/mesh-milk/crates/mmp-ffi/src/tests.rs`: FFI regression
  `oversized_nickname_is_rejected_before_claim_or_gossip_side_effects`.
- `implementation/mesh-milk/scripts/tests/test_mobile_api_surface.py`: tests
  that accept a current generated surface and reject stale/non-throwing
  `setNickname` output.

The source-level checks intentionally reject a stale pre-`343cbc5` binding;
passing only the old method-name check is insufficient because it would miss
the error-ABI change.

## Rollout checklist

- [ ] Regenerate Kotlin and Swift bindings from the same `mmp-ffi` commit that
      the native libraries were built from.
- [ ] Run the API, secret-surface, lifecycle, and Python test gates above; keep
      their output with the release evidence.
- [ ] Update Android call sites to handle the generated throwing error before
      enabling arbitrary nickname input.
- [ ] Update Swift call sites to use `try`/`do`/`catch` and handle `BadArg`.
- [ ] Update React Native native modules to reject the Promise on the native
      exception; update JS/TS callers to catch it.
- [ ] Add host-level tests for 32-byte ASCII, 33-byte ASCII, and multi-byte
      UTF-8 boundary cases. Confirm the rejected call leaves claims and
      pending gossip unchanged.
- [ ] Validate with the platform's UTF-8 byte count; never truncate silently.
- [ ] Regenerate or republish any checked-in/generated mobile artifact only
      after reviewing its source diff and verifying it contains the throwing
      annotation/signature.
- [ ] Run Android/iOS compilation and physical-device tests separately. The
      repository's source-level gates are necessary but not sufficient for
      that release decision.

This is an application/FFI compatibility migration only. No protocol field,
wire encoding, cryptographic primitive, or specification document was changed.
