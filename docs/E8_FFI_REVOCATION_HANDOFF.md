# E8 FFI Revocation Contract Handoff

**Status:** implemented 2026-08-10
**Scope:** additive FFI error reporting; no wire-format or specification change

## Problem

The node already had fallible self-revocation helpers, but the mobile FFI
surface used legacy wrappers. A provider rejection was therefore represented
as an empty byte vector, and the successor form incorrectly returned
`Ok(empty)`. A mobile caller could mistake that result for a published
revocation.

## Contract now implemented

- `MmpNode::try_revoke_identity(reason) -> Result<Vec<u8>, FfiError>` is an
  additive exported method. A successful result is the encoded, signed
  `REVOCATION` claim; `FfiError::Internal` means the local lifecycle/provider
  rejected publication.
- `MmpNode::revoke_identity_with_successor(...)` keeps its existing signature
  and now calls the typed node helper. Invalid 32/64-byte inputs still return
  `BadArg`; provider or lifecycle rejection returns `Internal`.
- The legacy `revoke_identity(...) -> Vec<u8>` remains unchanged for source
  and ABI compatibility. New mobile code should use `tryRevokeIdentity` and
  treat an error as non-publication.
- `check_mobile_api_surface.py` requires `tryRevokeIdentity` in both the
  generated method and throwing-method manifests.

## Safety evidence

The FFI regression suite verifies that rejected revocations do not mutate
claims, store length, or event output; successful revocation decodes as field
`0x09`, and a second attempt reports `Internal`. The successor regression
also verifies that a bounded provider rejection is not converted to an empty
successful result.

## Follow-up

Regenerate Kotlin/Swift bindings in the mobile release pipeline and update
the checked-in API reference and host migration notes atomically with those
generated artifacts. Do not add a new `FfiError` variant without a reviewed
host-language compatibility decision.
