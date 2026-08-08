# E5-S4 Conformance Handoff

## Checkpoint

This checkpoint is commit `31a1106` (`test(mmp-node): validate group snapshot
trailing bytes`). The frozen `specification/` directory was not modified.

Recent focused commits, oldest first:

- `7d3c39e` — independent envelope and outer-bundle reader;
- `01a4014` — HELLO and all 17 registered frame vectors;
- `c34d7eb` — outer-bundle vectors for classes 0 through 5;
- `89a2185` — strict Ed25519 non-canonical `S + L` rejection vector;
- `2bb6d81` — complete 9-domain signature matrix (9 accepts, 72 rejects);
- `66786fb` — envelope-cap and malformed-TLV negative vectors.
- `c34069b` — independent Beacon and all-registered-frame payload grammars;
- `b07a0b7` — machine-readable coverage manifest;
- `45ccdf1` — group-control security boundary and remediation order.
- `a73f747` — CLI snapshot restoration/staging and a restart regression test;
  the manual `connect` → `Ctrl-C` → `send --store` workflow now preserves
  claims, group blobs, replay state, and receipt suppression state.
- `7f29d7d` — runtime outbound frame/envelope cap checks in `SessionEngine`,
  preventing release builds from emitting locally generated oversized frames;
  The receive framing, post-decrypt validation, and `Frame::decode_payload`
  paths now reject payloads above the frozen 4096-byte cap. The 16-byte Noise
  AEAD tag therefore leaves a 4080-byte maximum encrypted application payload.
- follow-up group hardening binds each direct sender-key `member` to the
  authenticated origin, rejects stale/non-member distributions, and suppresses
  duplicate `(member, epoch)` receiver-chain resets. Admin forwarding was
  removed because the frozen sender-key record has no provenance proof for
  another member.
- `8f40af4` — strict cap enforcement, sender-key provenance/stale/duplicate
  guards, simulator boundary correction, generated-vector refresh, and mobile
  packaging preflight checks.
- `c929c70` — parser memory-amplification and file-transfer admission
  hardening: `RECON_IDS`/`WANT` counts are checked against remaining bytes before
  allocation, and `FileManifest` rejects every entry above the normative
  8 MiB `FILE_MAX` before any receiver-side offer or blob assembly can use it.
- `e24637d` — local snapshot hardening: Bloom restore rejects zero/oversized
  moduli with checked slice arithmetic; SeenIndex and ReceiptFilter reject
  impossible bucket/exact counts before allocation, while preserving the
  fixed 12-bucket and 1024-entry limits.
- `d420934` — group snapshot hardening: OrSet member/tag/tombstone counts are
  checked against remaining bytes before loops, and GroupState length arithmetic
  is checked before slicing, including the 32-bit overflow path.
- `31a1106` — corrected the GroupState trailing-byte regression fixture to use
  the valid eight-byte empty OR-Set encoding, so the test exercises the exact
  record-length check rather than an earlier malformed-inner-record path.

The manual desktop/mobile installation and interoperability procedure is in
`docs/MANUAL_INTEROP_TESTING.md` (committed earlier). It accurately states
that Linux BlueZ PC↔PC BLE is executable today, while Android/iOS currently
have artifact/API bridge checks rather than a turnkey consuming application.

## What is verified

`implementation/mesh-milk/vectors/` now contains deterministic artifacts for:

- HELLO, the 17 registered frame tags, and envelope framing;
- outer Bundle classes 0–5 with production-generated signatures;
- strict Ed25519 canonical-S rejection (`S + L`);
- all nine registered signing domains and their full cross-verification matrix;
- lifetime-reset and budget-clamp invariants;
- unknown-critical, out-of-order, and duplicate TLVs;
- the normative 4096-byte logical frame and encrypted-envelope cap, with
  4080-byte maximum application plaintext before the Noise tag.
- strict negative coverage for hostile `u16::MAX` reconciliation/request
  counts, plus the exact `FILE_MAX` boundary and an oversized-manifest reject.
- persistence decoder regression coverage for zero/oversized Bloom filters,
  more-than-12 bucket snapshots, and exact-count claims with no remaining
  records.
- group persistence regression coverage for hostile OR-Set counts and
  overflowed/trailing GroupState records.
- independent Beacon and frame-payload grammar checks;
- `coverage.json`, mapping all committed artifacts to requirement IDs, source
  sections, and verification evidence.

The independent reader intentionally does not call production frame, bundle,
TLV, or payload decoders. Production tests separately perform semantic
round-trips and cryptographic checks.

## Verification evidence

The latest serial workspace matrix passed, including 26 `mmp-cli`, 33
`mmp-core`, 26 `mmp-node`, 40 `mmp-types` unit tests, and 3 new parser/manifest
integration tests; `mmp-sim` passed 9 active tests with 4 intentionally ignored
scale/deployment cases. Workspace clippy with `-D warnings`, `cargo run -p
layer-check`, and `cargo run -p mmp-cli -- demo` also passed. Rustfmt could not
be run because the rustfmt component is not installed in the environment.
The persistence slice additionally passed 33 `mmp-core` unit tests, 4 focused
integration tests, and `mmp-store`'s 7 tests after the build cache was cleaned
to recover 7.1 GiB of disk space.
The group-decoder follow-up passed 34 `mmp-core` unit tests, 27 `mmp-node`
unit tests, and focused clippy for both crates.
The final post-follow-up workspace matrix passed with 34 `mmp-core`, 4
`mmp-core` persistence integration, 27 `mmp-node`, 40 `mmp-types`, and 3
`mmp-types` parser/manifest integration tests; `mmp-sim` again passed 9 active
tests with 4 intentionally ignored cases. Workspace clippy and `layer-check`
also passed.

Regenerate vectors with:

```text
cargo run -p mmp-cli -- vectors
```

Then run the focused suite before committing:

```text
cargo test -p mmp-cli --all-targets -- --nocapture
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p layer-check
```

## Deliberate specification boundaries

These are implementation/specification discrepancies, not silent protocol
changes:

1. `SigDomain::GroupMsg` is used by the current class-2 implementation but is
   not one of the nine registered document-10 domains. It is excluded from the
   9×9 matrix and remains provisional.
2. Class-4 Merkle-path framing is an implementation extension beyond the
   frozen class-4 description.
3. Class-5 `AnnouncementBody` carries a `suite` byte required by the current
   document-18 signing preimage but omitted from the frozen document-10
    listing. The vector is explicitly marked provisional.
4. Group bootstrap still uses the authenticated peer as a trust-on-first-use
   inviter because the frozen sender-key record has no admin-signed membership
   envelope. The candidate control schema remains quarantined; this is a
   documented residual risk, not a specification edit.

Do not “fix” these by editing `specification/` during routine implementation.
Record evidence and obtain a protocol decision first.

## Highest-priority remaining work

1. Run the manual BLE procedure on two Linux PCs. The current Windows host has
   a working Bluetooth adapter but no usable BlueZ peripheral path, so this
   hardware gate cannot be executed here. Mobile work currently stops
   at building and exercising the FFI artifacts until a real Android/iOS host
   application is supplied.
2. Keep the candidate group-control implementation quarantined until a
   normative control schema/domain decision resolves the findings in
   `docs/GROUP_CONTROL_SECURITY_AUDIT.md`.
3. Add independent cryptographic/payload vectors for the remaining opaque HS,
   class-1, and class-2 interiors only after their normative grammar and key
   context are available.
4. Reconcile stale status text in the dirty handbook/API/mobile integration
   files (strict 4096-byte cap and restart persistence claims) with the shipped
   implementation. Keep that cleanup selective; do not stage the unrelated
   revocation and integration edits already present in those files.

## Worktree hygiene

At handoff, these unrelated files are intentionally dirty or untracked and
must not be swept into a conformance commit:

- `docs/API_REFERENCE.md`, `docs/IMPLEMENTATION_HANDBOOK.md`;
- `implementation/mesh-milk/MOBILE_INTEGRATION.md` and
  `REACT_NATIVE_INTEGRATION.md`;
- `implementation/mesh-milk/crates/mmp-types/src/domain.rs`, `src/lib.rs`,
  `src/tests.rs`, and `src/group_control.rs`;
- `.tmp-*-audit-20260801/` directories;
- `docs/E5-S4_CONFORMANCE_DESIGN.md` and
  `docs/E5-S4_IMPLEMENTATION_PLAN.md` (planning/audit material).

Stage files explicitly by path. Do not use a blanket `git add .`.
