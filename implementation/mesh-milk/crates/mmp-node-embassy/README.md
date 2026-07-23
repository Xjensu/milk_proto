# mmp-node-embassy

Embedded orchestration on the Embassy async executor — the `no_std` counterpart to
`mmp-node`. Currently a 7-line stub (wiring only; the protocol engines it will drive
are already sans-IO and don't need an embedded-specific rewrite). Post-GA scope
(Epic 7) unless a customer forces it earlier.

**Layering:** depends on `mmp-core`, `mmp-router`, `mmp-store`, `mmp-tal`.

See `docs/IMPLEMENTATION_HANDBOOK.md` §4.9.
