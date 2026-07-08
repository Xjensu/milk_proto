# vectors/ — golden test vectors (doc 10 §6.4, doc 12 §7.1)

Populated at ROADMAP **Stage 1** alongside the `mmp-types` codec. Every frame and
bundle type gets fixed-key vectors, cross-checked by an independent decoder written
from doc 10 alone.

Required negative vectors (post-audit + rev. B):
- cross-signing-domain: a signature minted under one doc 10 §3.1 tag presented under
  another → reject (AUDIT F4); rev. B extends the set with `mmp1/manifest`,
  `mmp1/authcert`, `mmp1/announce` (CHANGELOG rev. B)
- non-canonical / overlarge-S Ed25519 → reject via `verify_strict` (AUDIT F9)
- `lifetime_ms` reset → identical BundleID (AUDIT F1)
- budget grant > L_MAX → clamped (AUDIT F3)

This directory intentionally contains no vectors at Stage 0.
