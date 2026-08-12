# Beta-test application API foundation

Status: implementation note, 2026-08-12. This document describes the additive
capability-discovery surface for the existing MMP/1 host API. It does not change
the frozen wire protocol or claim native-radio support.

## Capability report

`mmp-ffi::MmpNode::capabilities()` returns a secret-free record:

```text
capabilities: [DirectMessaging, GroupMessaging, FileTransfer,
               Announcements, Diagnostics, TransportInjection]
persistent_store: bool
```

The list order is stable. `persistent_store` is true only for a node created
with a file-backed snapshot path; it is false for an in-memory node. The report
describes protocol-core and API support only. Android/iOS permissions,
background execution, BLE controller features, and physical interoperability
remain adapter and hardware gates.

React Native receives the same report through `MmpClient.capabilities()` with
camel-case values (`directMessaging`, `groupMessaging`, `fileTransfer`,
`announcements`, `diagnostics`, and `transportInjection`). Native enum mapping
is exhaustive and fails closed on an unknown value, so bridge drift cannot be
silently rendered as a supported feature.

## Versioned native snapshot

Native hosts that need one machine-readable envelope can call
`mmp-ffi::MmpNode::beta_snapshot()`. It contains the same `status`, `metrics`,
and `capabilities` records plus `schema_version: 1`. The version belongs to
this host-facing report and is independent of the MMP/1 wire protocol. The
record remains secret-free and has no peer/session identifiers, raw transport
bytes, provider handles, key material, or persistence paths. Hosts should
branch on `schema_version` and fail closed if they receive a value they do not
understand.

The React Native facade exposes the same envelope as `betaSnapshot()` after
native module registration. It rejects unknown `schemaVersion` values and
normalizes capability spellings explicitly; the existing bounded
`BetaTestSession.report()` remains useful when a local event-history window is
also required.

## Suggested beta UI use

Call `capabilities()` after constructing the native node and before enabling
feature screens. Use `persistentStore` to explain whether restart persistence
is active. Keep a separate platform-health panel for permissions, radio state,
scan/advertising cadence, and observed transport callbacks; those facts cannot
be inferred from this report. Continue to use `status()`, `metrics()`, and the
event stream for runtime evidence. A beta app must never display a capability
as proof that a physical device exchanged a packet.

## Deferred surfaces

Peer/link inventories, hardware energy measurements, deterministic simulator
control, and device-specific background lifecycle remain outside this additive
slice. Implementing them safely requires new host-owned contracts or actual
platform evidence; no unsupported operation is represented as a fake success.
