# Current engineering state

The workspace contains a tested Rust prototype of MMP/1. Codec, cryptographic, session, routing, persistence, and simulator paths are present. The implementation status is summarized in [`../release-evidence/implementation-state.md`](../release-evidence/implementation-state.md).

The next integration sequence is:

1. complete a two-host Linux/BlueZ message and receipt exchange;
2. exercise relay delivery with a destination absent from the first contact;
3. validate mobile native consumers and keystore-backed state;
4. resolve file confidentiality, group epoch control, and claim grammar before extension interop;
5. obtain independent cryptographic review and calibrated energy/scale evidence.

No source-level protocol semantics are changed by this sequence.
