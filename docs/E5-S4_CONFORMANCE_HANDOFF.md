# E5-S4 Conformance Handoff

## Checkpoint

This checkpoint is on commit `66786fb` (`test(mesh-milk): add wire canonical
negative vectors`). The frozen `specification/` directory was not modified.
The implementation changes are intentionally limited to deterministic test
vectors, test-only readers, and their documentation.

Recent focused commits, oldest first:

- `7d3c39e` — independent envelope and outer-bundle reader;
- `01a4014` — HELLO and all 17 registered frame vectors;
- `c34d7eb` — outer-bundle vectors for classes 0 through 5;
- `89a2185` — strict Ed25519 non-canonical `S + L` rejection vector;
- `2bb6d81` — complete 9-domain signature matrix (9 accepts, 72 rejects);
- `66786fb` — envelope-cap and malformed-TLV negative vectors.

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
- the normative 4096-byte logical frame cap versus the implementation's
  4128-byte encrypted-envelope headroom.

The independent reader intentionally does not call production frame, bundle,
TLV, or payload decoders. Production tests separately perform semantic
round-trips and cryptographic checks.

## Verification evidence

The latest focused run passed 22 `mmp-cli` tests. Workspace clippy with
`-D warnings`, `cargo run -p layer-check`, `git diff --check`, and the serial
workspace test matrix also passed during this checkpoint. Rustfmt could not be
run because the rustfmt component is not installed in the environment.

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

1. Document 10's logical frame cap is 4096 bytes, while the current runtime
   `MAX_ENVELOPE_PAYLOAD` is 4128 bytes to leave room for an AEAD tag. The
   vector records 4097 as normative-reject/runtime-accept and 4129 as runtime
   reject. A production decision is still required: make runtime admission
   strictly 4096, or obtain an explicit specification revision.
2. `SigDomain::GroupMsg` is used by the current class-2 implementation but is
   not one of the nine registered document-10 domains. It is excluded from the
   9×9 matrix and remains provisional.
3. Class-4 Merkle-path framing is an implementation extension beyond the
   frozen class-4 description.
4. Class-5 `AnnouncementBody` carries a `suite` byte required by the current
   document-18 signing preimage but omitted from the frozen document-10
   listing. The vector is explicitly marked provisional.

Do not “fix” these by editing `specification/` during routine implementation.
Record evidence and obtain a protocol decision first.

## Highest-priority remaining work

1. Add independent payload grammar checks for Beacon and the 17 frame payloads;
   the current independent reader proves envelope boundaries but not every
   payload field.
2. Add a machine-readable coverage manifest mapping each vector to its
   requirement and source section. Keep the existing filenames stable.
3. Decide and implement the 4096/4128 admission policy after a specification
   review; add an interoperability regression test for the chosen behavior.
4. Audit the dirty candidate `implementation/mesh-milk/crates/mmp-types/src/group_control.rs`
   before any integration. The audit found missing authenticated outer
   envelopes, replay protection, signer/slot binding, batch limits, and
   unregistered signing domains. Do not stage it as-is.
5. Run the manual BLE procedure on two Linux PCs. Mobile work currently stops
   at building and exercising the FFI artifacts until a real Android/iOS host
   application is supplied.

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
