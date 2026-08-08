# MMP/1 ID_PROOF Erratum and Implementation Contract

Status: implementation clarification; `specification/` is intentionally
unchanged.

## Confirmed contradiction

The supplied review correctly found two incompatible descriptions:

- `specification/08-crypto-spec.md` §3.2 places `ID_PROOF` inside the encrypted
  Noise transport after the XX handshake;
- `specification/10-message-format.md` §2 and §2.2 still call `HS` and
  `ID_PROOF` the only pre-channel parsers.

These descriptions would produce different state machines for independent
implementations. The Rust implementation follows the encrypted interpretation,
which is the only one compatible with the Noise channel boundary and the
security requirement that application frames remain gated until identity proof.

## Implemented state machine

```text
HANDSHAKE
  HS1 / HS2 / HS3 are Noise handshake envelopes only
      ↓ Noise XX completes and transport AEAD is available
BINDING
  each side sends one encrypted ID_PROOF transport frame
  no SUMMARY, RBSR, BUNDLE, or application frame is accepted yet
      ↓ peer signature and claimed key verify
ESTABLISHED
  SUMMARY and all application/control frames are enabled
```

The proof signs the channel-binding object
`handshake_hash ‖ initiator_identity_pubkey ‖ responder_identity_pubkey` under
the `mmp1/chanbind` domain. The claimed identity must match the identity key
carried in the Noise peer introduction. Any plaintext proof, pre-proof
application frame, mismatched key, failed signature, or failed admission closes
the session silently.

## Evidence

`mmp-core` tests prove that a plaintext `ID_PROOF` is rejected after Noise
completion, a pre-proof application frame is rejected, and the queued proof is
accepted only through the encrypted transport. `mmp-crypto` keeps the binding
object ordering and domain separation explicit. This is a transport/state-machine
erratum only; it allocates no new frame type and changes no wire bytes.

## Required specification follow-up

Before claiming the document suite is frozen for independent implementations,
publish a small erratum or revision that states that `HS1/HS2/HS3` are the only
pre-transport records and that encrypted `ID_PROOF` is the first post-handshake
record. Add one deterministic handshake vector with the Noise transcript,
encrypted proof envelope, claimed key, and expected transition. Until then,
`docs/PROPOSAL_AUDIT_MMP_PHI.md` and this file are the implementation authority
for the discrepancy, not a silent modification of `specification/`.
