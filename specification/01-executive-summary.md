# 01 — Executive Summary

> **Status: FINAL for v1 specification suite** (supersedes the Phase 1 draft). All 15 documents complete; this summary reflects the whole. Date: 2026-06-10.

## What mesh_milk is

**mesh_milk (MMP/1)** is a peer-to-peer mesh communication protocol for smartphones, PCs, and IoT sensors that operates entirely without Internet infrastructure. It provides decentralized cryptographic identity, nicknames and avatars, end-to-end encrypted direct and group messaging, and delivery across kilometers and hours — messages travel by radio hop *and* by physically riding in moving devices' pockets (store-carry-forward). It functions fully at neighborhood scale (≤100 nodes), usably at district scale (~1,000), and degrades gracefully — slower, never collapsing — at city scale (10,000+).

The suite comprises a literature-grounded research report (02), normative specifications (03–11), an implementation blueprint with simulator (12), a quantitative performance model (13), failure analysis (14), and a research agenda (15). Every design choice records rationale, alternatives, tradeoffs, and supporting evidence; 59 sources are catalogued with confidence ratings; 13 open questions are tracked with resolution paths.

## The three facts that shaped everything

1. **Flooding meshes have a delivery cliff** (peer-reviewed Bluetooth Mesh simulations; LoRa field experience). MMP's data plane never floods: replication is bounded Binary Spray-and-Wait (≈ log N copies), steered by locally-observed utility — never by third-party routing claims, which are unfixable poison.
2. **BLE + mobile OS policy is a narrow envelope**: 31-byte universal beacons, ~50–150 kbps realistic connections, crippled background radio (especially iOS). Hence a two-plane design — duty-cycled beacons for presence/summaries, short authenticated sessions for bulk sync — and DTN store-carry-forward as the long-haul mechanism. "100+ hops" is achieved over space *and time*, not as 100 synchronous radio hops (which physics rules out).
3. **Retrofitted security fails** (Bridgefy broken twice, even after adopting Signal's library; bitchat repeating the classes in 2025). MMP authenticates every behavior-affecting byte from the first design day: identity *is* an Ed25519 key; the origin's public key rides **inline** in every bundle, which is signed (domain-separated) over a canonical form and verified *unconditionally* before storage or forwarding — there is no store-before-verify window at all; beacons are unauthenticated by physics and therefore are *hints only*, never triggers of durable state. Every published Bridgefy/bitchat failure has a named closure (07 §4). *(Suite hardened 2026-06-11 per `AUDIT.md` / `CHANGELOG.md`: inline origin keys, global signing-domain separation, resurrection-resistant dedup, exact-match purge, budget clamps.)*

## Architecture at a glance

| Layer | Mechanism | Spec |
|---|---|---|
| Identity | Ed25519 key = identity; petname nicknames as signed claims; no registry | 03, 08 |
| Discovery | 31-byte beacons, rotating ephemeral IDs synchronized with BLE address rotation | 03, 09 |
| Transport | GATT sessions under Noise XX with identity channel-binding; L2CAP CoC optional | 08, 09 |
| Long-haul | TTL-bounded signed bundles, age-based (clockless) lifetimes, quota stores | 03, 04 |
| Routing | Spray-and-Wait/Focus + local utility; signed receipts purge via rotating Blooms | 05 |
| Sync | Range-Based Set Reconciliation; LWW claims; causal parent-hash DAG; OR-Set groups | 06 |
| Crypto | X25519/Ed25519/ChaCha20-Poly1305/BLAKE3; serverless prekeys; sender-key groups with epoch rotation; reserved hybrid ML-KEM suite | 08 |
| Content | Reed-Solomon-coded blobs, per-symbol verification against signed Merkle roots | 08 |
| Devices | T0 sensors (`no_std` core, friend-buffered) → T1 leaves (iOS bg) → T2 phone relays → T3 PC anchors | 04 |

## What the numbers say (doc 13)

- **Cost per message grows with log N, not N**: ~22–36 KiB network-wide per 1 KiB message across 100→10k nodes (epidemic would be ~280× worse at 10k). Per-node relay load: ~1 MiB/day, ~1–2 minutes of airtime.
- **Radio energy: ~0.35%/day** of a phone battery by arithmetic; the honest unknown is the whole-phone multiplier (k_sys ∈ [3,10] → 1–3.5%/day), making the <5% target *plausible with a measurement plan* (OQ-13), not yet proven. Sensors: ~3 years on a coin cell.
- **Latency**: seconds-to-minutes when paths exist; mobility-dependent for long hauls. The BLE-parameterized simulation (mmp-sim, embedded real routing crates) is the named instrument for the remaining [S]-tagged claims.

## What it honestly does not do (07 §7, 08 §11)

No anonymity beyond pseudonymity; no deniability (relays must verify signatures); no post-compromise security between physical meetings; no protection from an adversary present at first contact (TOFU + verification ceremony); no key revocation without infrastructure; relays see bundle origin/destination IDs (the named metadata concession, with v2 escalation paths). Jamming and full physical eclipse are detected or routed-around-by-mobility, not prevented.

## Status and the path to running code

The specification is implementation-ready: byte-exact wire formats with canonical-encoding rules (10), normative state machines with testable invariants (11), and a Rust workspace blueprint with a strict `no_std` core, typestate sessions, fuzzing/golden-vector/property-test mandates, and a build order whose gates are the simulator de-risking pass and an external cryptographic review (12 §10). Production blockers before a v1.0 tag: SIG UUID assignment (OQ-12), k_sys energy measurement (OQ-13), simulation validation of routing constants (OQ-4), and external review of the three non-standard crypto compositions (OQ-10).

The internal adversarial-review process caught real flaws during writing — beacon/address rotation linkage, an MLS mischaracterization fixed against the RFC text, a crash-loop replication amplifier, a spec ambiguity in lifetime canonicalization — which is evidence the method works, and equally evidence that external eyes and field pilots are the necessary next step (15 §5).

## Rev. B extension (2026-07-03)

Three extension documents build on the frozen v1 suite without changing its wire format: **16** (photo/document file transfer over the existing blob plane — signed two-tier manifests, eager thumbnails, pull-only payloads, zero new wire allocations), **17** (deployment profiles as versioned constant-vectors with drill pass-bands — two train profiles and a campus profile, built on the finding that 2.4 GHz does not reliably cross a carriage gangway), and **18** (scoped authority certificates, public announcement bundles, and Sybil-resistant priority admission with a proved per-receiver abuse bound). The extension work was reviewed by an external multi-model council (`COUNCIL-REVIEW.md`, Rounds 1–2) — the external-eyes step rev. A's audit could not run — which re-graded the suite's novelty claims against prior art and identified errata-candidates for a future maintenance revision (recorded in `CHANGELOG.md`, docs 01–15 remain frozen). New research items R16–R21 and open questions OQ-14–18 track what the extension deliberately left open.
