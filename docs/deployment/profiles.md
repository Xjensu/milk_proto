# Profile format

A profile names a node tier, transport capabilities, local quotas, timer policy, scan/advertising policy, and topology assumptions. It MUST specify:

- supported wire version and suite;
- maximum bundle and frame sizes;
- storage and per-origin limits;
- BLE role and platform evidence;
- whether authority roots are pinned, gossiped, or absent;
- test workload, seed, pass/fail bands, and retained artifacts.

Core is intended for constrained sensors, Standard for phones and PCs, and Anchor for powered infrastructure. The tier does not grant authority and does not alter identity or cryptography.
