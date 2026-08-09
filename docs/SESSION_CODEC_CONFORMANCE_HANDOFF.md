# Session and Codec Conformance Handoff

Status: bounded implementation audit, 2026-08-09. This note is an
implementation handoff only. No file under `specification/` was changed, and
the frozen frame or bundle wire format was not extended.

## Audit scope

The audit covered `mmp-core::SessionEngine`, `mmp-core::Reconciler`,
`mmp-types::frame`, `mmp-types::wire`, and the already-recorded ID proof and
AEAD-associated-data contracts. It was intentionally narrower than a
repository-wide security certification or an external cryptographic review.

## Confirmed implementation alignment

1. **ID proof ordering.** `SessionEngine` accepts only `FT_HS` envelopes while
   in `Handshake`. Noise XX completion moves the engine to `Binding`, queues an
   encrypted `ID_PROOF` transport message, and does not enable `SUMMARY`, RBSR,
   or application frames until the peer proof verifies. A plaintext proof and
   a pre-proof application frame both close the session. This matches
   `docs/ID_PROOF_ERRATUM.md`; the contradictory normative wording remains in
   the frozen specification and still needs a separately reviewed erratum.

2. **Canonical bundle AEAD AD.** The executable boundary is
   `Bundle::canonical_aead_ad()`: fixed header, canonical extensions, and
   `payload_len`, with the five lifetime bytes zeroed. Existing direct, async,
   and group callers use it. The details and vectors are recorded in
   `docs/AEAD_AD_CONFORMANCE.md`; no second AD construction was found in this
   audit.

3. **Envelope and Noise-tag boundary.** The receive parser rejects a declared
   payload above 4096 bytes before appending it to the reassembly buffer. Since
   Noise transport adds a 16-byte tag, a 4080-byte plaintext is the largest
   application payload that can produce one 4096-byte encrypted envelope. The
   new regression `noise_tag_is_counted_at_envelope_boundary` proves both
   directions: 4080 bytes are emitted at the exact boundary, while 4081 bytes
   fail closed rather than being truncated or emitted oversized.

4. **RBSR domain grammar.** The session accepts only domains D0--D3, reserves
   the high bit for replies, allows the initiator to open domains in order, and
   requires a responder to finish the current domain before accepting the next.
   Range, terminal-list, pending-work, and missing-ID ledgers have explicit
   bounds; resource exhaustion closes the session instead of silently claiming
   convergence. The wire remains one base domain byte plus the existing reply
   flag; no unreviewed priority partition was added.

5. **Malformed input.** `read_envelope` checks the length prefix before
   reassembly growth. `Frame::decode_payload` rejects unknown types, trailing
   bytes, oversized variable fields, and impossible fixed-width counts before
   reserving vectors for the reconciliation/request paths. The focused
   `mmp-core` suite passed with 64 tests after the boundary regression.

6. **Pre-auth Noise receive storage.** `NoiseHandshake::advance` now rejects
   an HS input above `MAX_HANDSHAKE_MESSAGE = 256` before taking the Noise
   state or calling `snow`. Each handshake allocates one fixed 256-byte
   receive buffer and reuses it for every `read_message` call; the buffer is
   never resized from attacker input. The bound is wire-neutral for the
   current v1 profile: `Hello::decode` caps suites at 16 bytes, so the largest
   identity-plus-HELLO payload is 57 bytes, and the largest standard Noise XX
   message carrying that payload is 153 bytes. The remaining margin is for
   framing/implementation headroom, not an extension to the wire format.
   `noise_xx_rejects_oversized_pre_auth_input_without_poisoning_state` covers
   early rejection and confirms that a subsequent valid first message still
   progresses the handshake.

7. **Noise XX first-message payload handling.** `snow` permits a payload on
   XX message 1. MMP/1 does not use that cleartext payload: identity+HELLO is
   carried in encrypted messages 2 and 3. The responder therefore consumes
   any bounded message-1 payload for transcript correctness but deliberately
   ignores it rather than parsing a provisional `remote_intro`. The regression
   `noise_xx_ignores_nonempty_msg1_payload_until_encrypted_intro` injects an
   invalid cleartext payload and still completes the handshake from the
   encrypted msg3 identity. This preserves interoperability with Noise peers
   that use a bounded msg1 payload while preventing unauthenticated HELLO or
   identity state from influencing the session. No specification file changed.

## Residual gaps requiring a reviewed follow-up

### Clear envelope type is not authenticated by Noise

The two-byte length and one-byte `typ` are outside the Noise ciphertext, and
`NoiseChannel::encrypt`/`decrypt` expose no associated-data argument. An active
on-path peer can therefore retag a valid ciphertext before the receiver calls
`Frame::decode_payload`. Most retags fail strict decoding; however, opaque
`BUNDLE`, `HS`, and friendship payload variants can reach a different host
callback, and a valid bundle retagged into another opaque class is not
cryptographically distinguishable at the transport layer. Bundle signatures
and downstream semantic decoders limit durable impact, but they do not make
the frame type authenticated.

This is a real conformance/security residual, not something to “fix” by adding
an unreviewed byte to v1. A future protocol revision must either authenticate
the type as Noise associated data or redundantly carry it inside the encrypted
plaintext, then publish vectors and update independent implementations. Until
that decision is made, host callbacks must continue to treat every decoded
frame as untrusted input and perform their own semantic validation.

### Public frame encoders are infallible

`Frame::encode_payload()` and `Frame::encode()` use debug assertions for the
aggregate 4096-byte cap and cast collection lengths to wire-width integers.
`SessionEngine` validates the encoded plaintext and post-Noise ciphertext
before transmission, so the network path fails closed. Other local callers
that invoke the public infallible encoders directly can still construct a
non-canonical or truncated value in a release build. A future API hardening
should add a fallible `try_encode_payload`/`try_encode` seam (or enforce
constructor invariants) without changing any accepted v1 bytes.

### Pre-auth Noise parser residual caveats

The reference `mmp-crypto`/`snow` path no longer allocates a receive buffer
proportional to unauthenticated input: the fixed 256-byte buffer is allocated
once when the handshake object is created. This closes that specific
allocation-shape gap without changing any accepted current v1 handshake bytes.
It does **not** claim zero total allocation: `Hello::decode` still creates a
bounded suite vector (maximum 16 bytes), and outbound reply buffers are
allocated from the locally supplied HELLO. `Hello::encode` remains an
infallible local API, so callers should validate their own HELLO before
construction; non-conforming oversized suite lists are rejected by the peer.
The responder's cleartext XX message-1 payload is consumed but ignored;
identity+HELLO is accepted only from the encrypted message-2/message-3
payloads prescribed by the current handshake construction.

The cap is enforced in the standard-library `snow` implementation only. The
planned embedded/no-std Noise fallback has a separate allocation and parser
review gate; this handoff must not be read as evidence that that future path is
closed. A future `snow` upgrade or cipher-suite revision must re-derive the
bound and rerun the handshake vectors before changing it.

## Verification run

```text
cargo test -p mmp-core --lib -- --test-threads=1
  64 passed
cargo test -p mmp-core --lib noise_tag_is_counted_at_envelope_boundary -- --test-threads=1
  1 passed
cargo test -p mmp-crypto --lib noise_xx -- --test-threads=1
  5 passed
```

The full workspace matrix and release gates remain the orchestrator's
responsibility. Native Android/iOS radio and background behavior, Linux/BlueZ
two-device exchange, MCU linked-image/HIL behavior, and independent RBSR
priority-domain conformance remain external gates.
