# Open protocol decisions

## ID_PROOF placement

The message-format documents described `ID_PROOF` as a pre-channel parser record while the crypto rule and Rust session engine send it as the first encrypted Noise transport record. The canonical behavior is the tested encrypted record. HELLO and Noise handshake records are the only pre-transport records. An independent vector and erratum are still desirable.

## File symbol confidentiality

The owner requirement is file-level authenticated encryption before Reed-Solomon and Merkle processing. Current code processes plaintext symbols and exposes them to relays. The implementation remains unchanged in this documentation reboot; file confidentiality is an extension blocker and requires an explicit file-key, nonce, manifest-binding, and symbol-associated-data design.

## Copy-budget scope

Binary splitting conserves a budget for an honest local tree. Receiver-local clamps and buckets limit admission. They do not enforce a Byzantine global spend-once capability. The specification uses the narrower invariant.

## Friendship and avatar claim grammar

The current Rust Friendship body is `friend key32 || since u64 || countersignature`, while older text used different names. Avatar claims have an underspecified blob-root body. Cross-implementation vectors remain blocked until a reviewed grammar is selected.

## Group epoch control

Future sender-key epochs can currently be accepted without an authenticated administrator anchor. A strict equality check would break legitimate convergence because the current wire path carries no anchor. The control schema remains quarantined until its authorization and bounded-retention rules are specified.
