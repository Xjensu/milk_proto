# MMP/1 protocol specification

Status: normative MMP/1 source

This directory is the normative source for the MMP/1 wire protocol and node behavior. The key words MUST, MUST NOT, REQUIRED, SHALL, SHALL NOT, SHOULD, SHOULD NOT, RECOMMENDED, MAY, and OPTIONAL are to be interpreted as described in RFC 2119 and RFC 8174 when they appear in uppercase.

MMP/1 is a store-carry-forward protocol. A bundle can survive multiple contacts, but delivery is eventual and bounded by lifetime, storage, contact opportunities, and admission policy. A node MUST verify an origin signature before storing or forwarding a bundle. A beacon is an unauthenticated discovery hint; it never authorizes storage, forwarding, or identity attribution.

Normative topics:

1. [`terminology.md`](terminology.md) defines identifiers, clocks, contacts, and node tiers.
2. [`protocol-model.md`](protocol-model.md) defines node roles and the contact lifecycle.
3. [`identity-claims.md`](identity-claims.md) defines identities, claims, and local names.
4. [`bundles.md`](bundles.md) defines canonical bundle encoding, identifiers, signatures, lifetimes, and payload classes.
5. [`sessions.md`](sessions.md) defines Noise XX, channel binding, frames, and direct-message protection.
6. [`synchronization.md`](synchronization.md) defines inventory reconciliation, receipts, and durable indexes.
7. [`routing.md`](routing.md) defines quotas, copy budgets, Spray-and-Wait, Focus forwarding, and utility.
8. [`cryptography.md`](cryptography.md) defines the cryptographic suite and key handling.
9. [`ble-transport.md`](ble-transport.md) defines beacon and GATT transport behavior.
10. [`state-machines.md`](state-machines.md) defines observable node and session transitions.
11. [`security.md`](security.md) states the threat model and residual risks.
12. [`conformance.md`](conformance.md) defines vectors, required tests, and implementation evidence.
13. [`registries.md`](registries.md) centralizes wire constants, limits, and status.

Files, groups, friendship, and authority announcements are documented as extensions. They cannot be required for the first direct-message milestone.
