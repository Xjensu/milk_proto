# Release gates

Core gates:

- two physical BLE nodes exchange an encrypted direct message and receipt;
- a relay stores and forwards a message between non-overlapping contacts;
- independent readers consume the committed vectors;
- strict clippy, workspace tests, fuzz campaign evidence, and persistence recovery pass;
- external cryptographic review covers channel binding, prekeys, ratchets, and session code.

Extension gates:

- file encryption is applied before Reed-Solomon and Merkle processing;
- group control has authenticated epoch authorization and bounded retention;
- Friendship and avatar claim grammars have a reviewed vector set;
- authority root compromise and revocation behavior are specified.

Deployment gates:

- Android, iOS, Linux, and embedded capability claims have platform evidence;
- mobile keystore/keychain persistence and background behavior are tested;
- production BLE UUID allocation is recorded.

Research gates:

- energy, contact-window, mobility, 1,000-node, and 10,000-node claims use calibrated traces or device measurements;
- results report seeds, workloads, confidence intervals, and environmental limits.
