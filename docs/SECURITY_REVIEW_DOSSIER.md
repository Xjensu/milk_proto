# Security Review Dossier

**Purpose.** This document assembles what an external cryptographic/security reviewer needs
before starting (backlog `E6-S4`, roadmap Phase D exit gate, doc 12 §10's "external crypto
review window before any v1.0 tag," doc 01's OQ-10 production blocker). **The review itself is
external — this document is the package, not the review.** Everything below is a pointer into
already-shipped, already-tested spec/code, not a new analysis; where a claim needed a number to
back it, that number was verified against the current repository state at the date below, not
recalled from memory.

**Compiled:** 2026-07-22, against commit `ee12373` (`master`) and `Cargo.lock` as of that commit.

**Current-state refresh:** 2026-08-09. The original commit/date above remains the provenance of
the review package; the vector and release-gate status in §§5–6 below was rechecked against the
current workspace so that a reviewer is not handed superseded gap claims.

---

## 1. Scope: what this review is specifically being asked to confirm or refute

Three project-specific, non-standard cryptographic compositions, each already flagged in the
specification itself (doc 08 §9, doc 01, `specification/research/open-questions.md` OQ-10) as
requiring external confirmation before a v1.0 tag:

1. **Channel binding (doc 08 §3.2).** Identity keys sign the Noise XX handshake hash plus both
   parties' identity public keys (`"mmp1/chanbind"‖0x01‖noise_handshake_hash‖initiator_pubkey‖
   responder_pubkey`, doc 10 §3.1's domain-separation scheme), rather than the handshake hash
   alone — closing identity-misbinding/unknown-key-share. Precedent cited: libp2p's Noise spec
   uses the same sign-the-session-hash pattern (doc 08 §9 persona review).
2. **Serverless X3DH-adapted prekeys (doc 08 §4.2).** First-contact DTN messaging needs
   forward-secret key agreement with no server to enforce one-time-prekey consumption. Two
   adversary-controllable degradations are already documented and bounded (never below the
   sealed-box floor): replay-consumption of published one-time prekeys, and **prekey-batch
   suppression** by an active relay-adversary (AUDIT F6b) — an eclipse-class attack that can force
   first-message forward secrecy down to the no-FS baseline at will.
3. **Opportunistic DH ratchet step (doc 08 §5.2).** The Double Ratchet's DH step, adapted for a
   store-and-forward network: post-compromise security is opportunistic (heals only when parties
   actually meet), not continuous. Doc 15 R3 suggests this may be a novel-enough security
   definition ("PCS upon contact") to be worth its own formal treatment, not just a review comment.

**Formal-modeling angle already proposed (doc 15 R3, not yet executed):** Tamarin/ProVerif models
of all three, extending the Noise Explorer methodology [F3] that already covers standard Noise XX.
Whether to build these models, or review by inspection, is the reviewer's call — this dossier
doesn't presume the answer.

Everything else in `mmp-crypto` is either a standard primitive used in its standard mode (Noise
XX itself, Ed25519, X25519, ChaCha20-Poly1305) or a fixed-input hash/derive-key application (BLAKE3
for BundleID/Merkle roots/ratchet epochs — doc 08 §1's "two hash functions, deliberately" note
explains why BLAKE3 is *not* substituted inside Noise). Reviewing those for implementation bugs is
in scope in the ordinary sense; they are not the object of OQ-10's *composition* concern.

## 2. Crypto inventory

**Suite 0x01 (mandatory v1, doc 08 §1) — exact resolved versions, `Cargo.lock` at commit `ee12373`:**

| Role | Primitive | Crate | Resolved version |
|---|---|---|---|
| Signatures | Ed25519 (strict/canonical-S only, `verify_strict`) | `ed25519-dalek` | 2.2.0 |
| Key agreement | X25519 | `x25519-dalek` | 2.0.1 |
| AEAD | ChaCha20-Poly1305 (IETF) | `chacha20poly1305` | 0.10.1 |
| Hash / KDF / MAC (application layer) | BLAKE3 | `blake3` | 1.8.5 |
| Session handshake (Noise XX) | `Noise_XX_25519_ChaChaPoly_BLAKE2s` via `snow` | `snow` | 0.9.6 |
| Secret zeroization | `zeroize` on drop | `zeroize` | 1.9.0 |
| RNG (injected only, never ambient) | `rand_core::CryptoRngCore` trait | `rand_core` | 0.6.4 |

**Suite 0x02 (reserved, unimplemented):** hybrid X25519+ML-KEM-768. No code exists for it —
`mmp-crypto::noise::SUPPORTED_SUITES = &[0x01]` (E6-S2, 2026-07-22) explicitly refuses any
handshake advertising only suite `0x02`, rather than silently accepting an unimplemented
composition. Blocked on **OQ-11** (hybrid Noise pattern maturity — no stable IETF/Noise-community
draft to implement against yet).

**No custom primitives anywhere** (doc 08 §9's own claim, re-verified against the crate list
above): every cryptographic operation in `mmp-crypto` is a call into one of the audited crates in
the table; the compositions in §1 above are the only project-specific *arrangements* of them.

**Key hierarchy** (doc 08 §2): permanent Ed25519 identity key signs an X25519 exchange-key claim
and a prekey-batch claim; the exchange key doubles as the Noise XX static; each session produces
ephemeral transport keys; the first session between two identities derives a Conversation root
`CK₀`, from which conversation ratchet chains and the receipt MAC key are derived. Group sender
keys are separate per-group/per-epoch random keys, distributed over pairwise conversation
channels, and are not derived from `CK₀`. Identity keys never rotate — compromise means identity
replacement, not key rotation (consistent with the petname trust model having no registrar to
attest a rotation).

**Ratchet-state persistence (doc 08 §5.3 amendment, 2026-07-16):** phones/PCs persist conversation
ratchet state across restart, but only keystore-wrapped; Core/`no_std` targets never persist it.
Reviewers auditing persistence code should check this boundary specifically — it's a reversal of
the implementation handbook's earlier "don't persist" default, made after finding `Conv` retains
`ck0` (a backward-secrecy hazard if persisted naively).

## 3. Threat model (doc 07, current state post-`E1-S3` consolidation)

Doc 07 is the threat-catalog hub the rest of the suite cross-references (its own §9: "Failure-mode
interactions → doc 14"; doc 18's A1–A7 catalog folds in via §7 rows #19–23; this session's E1-S6
added doc 14 pointer rows F23/F24 for A6/A7 specifically).

- **§1 Adversary model** / **§2 Assets and goals** — local active adversary, pseudonymous
  identities (this repo's fixed, non-relitigated threat tier, per `CLAUDE.md`).
- **§3 Attack→defense matrix** and **§4 Bridgefy/bitchat failure-class closures** — the comparative
  security baseline against the two most-cited prior BLE-mesh messengers' known failure modes.
- **§7 Residual risk register — 10 entries, reviewed here for completeness, not re-litigated:**
  1. First-contact MITM window (TOFU limit) — social mitigation, not cryptographic.
  2. First-message FS absent without a prekey claim (§4.2 above) — never below sealed-box
     confidentiality; can be *forced* by prekey suppression (the eclipse vector applied to prekeys).
  3. Contact-occurrence/coarse-volume metadata visibility to co-present observers.
  4. Fully-eclipsed node partition — detectable, not preventable.
  5. Web-of-trust quota side channel.
  6. Receipt-filter false-positive suppression, ~1% per pair.
  7. Sybil base-rate consumption — fairness degradation, not a safety breach.
  8. **Stolen operator ROOT key (doc 18 §7 A6) — the one entry that is genuinely open-ended, not
     merely bounded:** unbounded within the compromised scope until re-pin. Contained to one
     operator's scope namespace, never global.
  9. Stolen conductor/AuthCert key (doc 18 §7 A1) — bounded by budget × remaining validity.
  10. Declared-priority queue-jump (doc 18 §7 A7) — scheduling nuisance, no defense at wire-phase
      ordering in v1, filed as a `CHANGELOG.md` errata candidate for a future structural fix.

  Every entry is stated honestly rather than hidden, bounded except #8 (explicitly flagged as the
  exception), with a named escalation path per entry (doc 07 §7's own closing line).

## 4. Fuzz results (backlog `E5-S6`, doc 12 §7 mandate)

Four `cargo-fuzz` targets exist under `implementation/mesh-milk/fuzz/`, in priority order (pre-auth
parsers first — attacker input costs the attacker nothing before any identity is proven):

| Target | Parser under test | Invariant |
|---|---|---|
| `fuzz_beacon` | `Beacon::decode` | unauthenticated broadcast, 24 B fixed input; no panic/OOM |
| `fuzz_envelope` | `read_envelope` + `Frame::decode_payload` | session-frame length-prefix + pre-auth payload decode (HELLO, HS, ID_PROOF) |
| `fuzz_bundle` | `Bundle::decode_canonical` | every bundle any relay sees, before `verify_strict` |
| `fuzz_hs` | `NoiseHandshake::responder().advance()` | first message from an unauthenticated peer |

**Honest status, not overstated:**
- **CI `fuzz-build`** (every push): all four targets build — a link-correctness gate, not a fuzzing
  run.
- **CI `fuzz-run`** (weekly scheduled run or manual `workflow_dispatch`): a 2-minute smoke run per target on
  `ubuntu-latest`, corpus uploaded as a build artifact. **This is a smoke check, not the hardening
  run** doc 12 §7's "N CPU-hours once a baseline throughput is measured" calls for — no baseline has
  been measured yet, so no CPU-hour figure is pre-committed (same no-pre-commit discipline the
  roadmap's Phase C exit gate states generally).
- **Deterministic seed recipe, no persisted discovered corpus:** `seed_corpus.py` derives hand-picked
  seeds (maximal-length beacon, every-TLV-type bundle, truncated Noise message, and the Noise
  256/257-byte boundary) from committed vectors; CI runs `check`, `write`, and `verify` before the
  fuzz build. Generated seed files and libFuzzer's discovered-input directory remain gitignored by
  design, so a measured CPU-hour hardening run is still open. This distinction is tracked under
  E5-S6 rather than silently dropped.
- **Windows toolchain gap, not a code defect:** all four harnesses compile cleanly under plain
  `rustc` on Windows; the sanitizer-instrumented `cargo fuzz build` link step fails on Windows for
  toolchain reasons (`clang_rt.asan_dynamic_runtime_thunk` / unresolved `__sancov_pcs`), documented
  in `fuzz/README.md`. CI's `ubuntu-latest` jobs are the actual verification.

**Reviewer note:** if the review scope includes running the fuzzers for real (rather than reading
the harnesses), this is the point where a genuine CPU-hour budget should get set — the project has
deliberately not invented one.

## 5. Golden test vectors (backlog `E5-S4`, doc 10 §6.4, doc 12 §7.1)

The committed vector inventory under `implementation/mesh-milk/vectors/` is generated by
`mmp-cli vectors` directly from the real `mmp-types`/`mmp-crypto` codecs (never hand-authored).
It includes positive and negative envelope, bundle, beacon, HELLO, frame-payload, size-boundary,
canonical-S, and cross-domain artifacts. The consuming suite is layered: regeneration is
deterministic, committed files byte-match current codec output (the drift guard), independent
readers re-parse the wire grammars without production decoders, and `coverage.json` maps every
artifact to a specification section and evidence string.

The artifact inventory and its stable IDs are authoritative in `vectors/coverage.json`. Secret
MAC keys are intentionally omitted from public vectors; secrets never appear in a vector.

The deterministic public-only X3DH-adapted agreement artifact is now committed as
`prekey_agreement.json`. It uses a byte-exact test RNG, asserts bilateral CK0 agreement during
generation, and publishes only the signed public batch, ephemeral public key, prekey id, and a
one-way CK0 fingerprint. Because private inputs are intentionally omitted, a second
implementation can independently validate the public batch and wire bytes, but cannot recompute
the CK0 fingerprint from the artifact alone; the full cryptographic property remains covered by
`mmp-crypto::prekey_agreement_matches_both_sides`. The overlarge-S/non-canonical Ed25519
negative is no longer deferred: `noncanonical_s_negative.json` is committed and independently
checked, with strict verification covered by `mmp-crypto::sign_verify_and_cross_domain_rejects`.

## 6. Known gaps a reviewer should not be surprised by

Stated here so the review isn't spent rediscovering already-tracked items:

- **Clippy is wired into CI, but remote-run evidence remains external:** the root workflow's
  `host` matrix runs `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
  A local pass does not substitute for a completed GitHub Actions run.
- **The MSRV floor is declared, but the exact compiler release is not pinned:** the workspace
  sets `rust-version = "1.96"` while `rust-toolchain.toml` intentionally follows rolling
  `stable`. Consumers requiring bit-for-bit toolchain reproducibility must pin that toolchain
  outside this repository. Per-crate metadata/README coverage is complete for the current crates;
  `missing_docs` remains a separate, open documentation-hardening effort.
- **`AuthorityStore` non-secret state is durably validated and persisted** by `mmp-store`, FFI, and
  CLI snapshots. Ratchet/prekey secrets and platform-keystore integration remain intentionally
  open; those secrets must not be inferred from the checksum-only authority record.
- **`mmp-sim` has no clustered-mobility (SLAW) contact model** (E6-S1 finding, 2026-07-22) — the
  doc 13 §5 same-district latency band cannot yet be validated at all, only its cost/quota-side
  claims (which do hold, measured, at N=1,000).
- **Suite `0x02` is unimplemented** (§2 above) — not a bug, a stated OQ-11 dependency.

## 7. Where things live (for the reviewer's own navigation)

- Spec: `specification/07-security-spec.md` (threat model), `specification/08-crypto-spec.md`
  (crypto compositions), `specification/10-message-format.md` §3.1 (signing-domain separation),
  `specification/18-authority-announcements.md` §7 (A1–A7 attack catalog).
- Code: `implementation/mesh-milk/crates/mmp-crypto/src/` (`noise.rs` §3.2/handshake,
  `prekey.rs`/`sealed.rs` §4, `ratchet.rs` §5), `implementation/mesh-milk/crates/mmp-core/src/`
  (session/claims/revocation enforcement).
- Fuzz: `implementation/mesh-milk/fuzz/`.
- Vectors: `implementation/mesh-milk/vectors/`.
- Open questions this review is expected to move: OQ-10 (this dossier's primary subject), OQ-11
  (out of scope — nothing to review yet) — `specification/research/open-questions.md`.

## 8. What "done" looks like for this backlog story

Per `docs/AGILE_BACKLOG.md` E6-S4: this dossier existing and being accurate is the story's
deliverable — **the review itself is external** and its findings, once received, get folded back
into doc 07/08 and `CHANGELOG.md` exactly like every other post-audit revision in this project's
history (`AUDIT.md`, the rev-B `CR-*` disposition series in `COUNCIL-REVIEW.md`), not into this file.
