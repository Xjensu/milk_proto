# Open technical questions

1. Resolve the Friendship body and avatar-root grammar so independent implementations can exchange those claims.
2. Define an authenticated group administrator anchor, future-epoch authorization, and late-join behavior.
3. Specify file-key, nonce, manifest, and symbol binding for encrypt-before-erasure-coding.
4. Decide whether RBSR priority partitions and authority weighting belong in a later wire version.
5. Measure 64-bit RBSR fingerprint collision and recovery behavior under expected comparison counts.
6. Run physical two-PC BLE, Android, iOS, and embedded durability tests.
7. Complete independent cryptographic review of channel binding, serverless prekeys, ratchet persistence, and group composition.
8. Calibrate clustered mobility, short-contact scheduling, battery, and 1,000/10,000-node simulations.
9. Select production BLE UUIDs and document platform availability.
10. Establish the scope and detection path for a compromised authority root.

An unresolved question with security consequences stays outside the normative core until its behavior can be specified and tested.
