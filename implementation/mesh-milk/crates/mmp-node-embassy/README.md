# mmp-node-embassy

Embedded orchestration on the Embassy async executor — the `no_std` counterpart to
`mmp-node`. Currently a 7-line stub (wiring only; the protocol engines it will drive
are already sans-IO and don't need an embedded-specific rewrite). Post-GA scope
(Epic 7) unless a customer forces it earlier.

**Layering:** depends on the no-std profiles of `mmp-core`, `mmp-router`, and
`mmp-tal`. The current `mmp-store` implementation is filesystem-backed and
`std`-only, so it is deliberately not a dependency of this embedded shell;
an eventual flash-log store must implement the `mmp_core::BundleStore` seam
without widening the target feature closure.

The dependency feature fence is intentional: enabling `mmp-core`'s default
`std` feature would pull the `snow` Noise backend and host entropy support into
the Cortex-M graph. This crate therefore remains target-checkable while the
hardware TAL, executor wiring, and flash persistence remain separate E7 gates.

See `docs/IMPLEMENTATION_HANDBOOK.md` §4.9.
