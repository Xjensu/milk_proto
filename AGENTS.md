# AGENTS.md

This file provides guidance to Codex (Codex.ai/code) when working with code in this repository.

## Project

**mesh_milk** — a scientifically grounded peer-to-peer mesh communication protocol for phones, PCs, and IoT devices over BLE (WiFi Direct / UWB as future transports), operating with no Internet infrastructure. The deliverable is a frozen 01–15 core specification suite plus final rev-B extension documents 16–18 (in `specification/`) suitable for Rust implementation, plus a `implementation/mesh-milk` Rust workspace.

## Document Map

The 15-document wording in the project description refers to the frozen core;
the repository also contains final rev-B extension documents 16–18. Do not
edit `specification/` during implementation work unless the explicit
specification-change rule below is satisfied.

The frozen core suite is documents 01–15; documents 16–18 are the final rev-B
extension documents. Together they form the current specification inventory.

- `specification/01`–`specification/15` — the specification suite, numbered by phase:
  - Phase 1: 01 (exec summary draft), 02 (research report)
  - Phase 2: 03 (protocol spec), 04 (architecture), 05 (routing)
  - Phase 3: 06 (sync), 07 (security), 08 (crypto)
  - Phase 4: 09 (BLE transport), 10 (message format), 11 (state machines)
  - Phase 5: 12 (Rust blueprint), 13 (performance model), 14 (failure analysis), 15 (future research)
- `specification/research/bibliography.md` — every cited source: URL, accessed date, confidence level
- `specification/research/systems-comparison.md` — comparison matrix of prior systems
- `specification/research/open-questions.md` — uncertainties carried between phases
- `specification/16`–`specification/18` — final rev-B extension documents for
  file transfer, deployment profiles, and authority announcements
- `implementation/mesh-milk/` — the Rust workspace (reference implementation)
- `docs/` — audit reports, handoffs, decisions, roadmaps, and miscellaneous
  implementation notes; this is the tracked documentation location.
- `other/` — ignored local tooling/staging space; it contains no tracked
  project artifacts and must not be treated as a source of truth.
- Approved plan: `C:\Users\Asus\.Codex\plans\rustling-wishing-peacock.md`

## Fixed Requirements (do not re-litigate; settled in discovery)

- Scale tiers: full function ≤100 nodes, usable at 1k, survivable at 10k+
- Threat model: local active adversary; pseudonymous identities
- Platforms v1: Android, iOS, Linux/Windows, IoT (nRF52/ESP32, `no_std` Rust)
- Latency: minutes acceptable (DTN store-and-forward primary)
- Retention: TTL 24–72 h + per-node storage quota
- Battery: <5%/day background on phones
- Identity: Ed25519 key = identity; petname-model nicknames; content-addressed avatars
- Crypto: X25519/Ed25519 + ChaCha20-Poly1305 + BLAKE3, cipher agility, reserved hybrid ML-KEM slot
- Groups: sender-keys + causal ordering via Merkle-DAG; epoch rotation
- Language: English

## Methodology (applies to all writing in this repo)

- **No invented facts.** Uncertain → state it, search the web, compare ≥2 sources, tag confidence (High/Medium/Low + evidence basis).
- Evidence priority: peer-reviewed papers > RFCs/Bluetooth SIG specs > project docs > blog posts.
- Every major design choice records: rationale, alternatives considered, tradeoffs, supporting evidence.
- Adversarial persona review for major conclusions, written as explicit "Potential flaw / Alternative / Resolution" blocks: Skeptical Professor, Security Auditor, Performance Engineer, Distributed Systems Reviewer, Battery Specialist, Inventive Scientist.
- Every citation added to a doc must also be added to `specification/research/bibliography.md` with a real, fetched URL — no citations from memory.
- Phase gate: user reviews the phase's docs before the next phase begins.
