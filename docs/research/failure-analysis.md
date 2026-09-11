# Failure analysis

The principal failure classes are malformed input, signature or AEAD failure, replay, stale lifetime, duplicate delivery, quota exhaustion, token-bucket exhaustion, link loss, session timeout, OS background suspension, corrupted durable state, BLE stack incompatibility, group epoch divergence, file-symbol loss, and authority-root compromise.

The protocol response is fail-closed parsing, verify-before-store, BundleID and symbol deduplication, monotonic expiry, bounded queues, atomic snapshots, retry with backoff, and explicit release gates. These responses limit local resource use; they do not prevent RF jamming, eclipse, metadata observation, or a stolen identity key.

The implementation-state report maps each class to current code and evidence. Unresolved items are open engineering decisions rather than completed mitigations.
