# mmp-cli

PC node binary and debugging/inspection tool: `keygen`, `demo` (two in-memory nodes,
no radio needed), `inspect` (reads the local diagnostics journey log), plus real BLE
subcommands — `connect`/`send` (central role, over `mmp-tal-btleplug`) and `listen`
(peripheral role, over `mmp-tal-peripheral`, Linux/BlueZ only today).

`mmp-cli demo` keeps its human-readable transcript by default. For beta harnesses,
`mmp-cli demo --json` runs the same two-node exchange and emits exactly one
schema-versioned JSON report on stdout; diagnostics are bounded and contain no
seeds, private keys, raw frames, or ciphertext.

**Layering:** a top-level consumer — depends on `mmp-node`, `mmp-core`, `mmp-crypto`,
`mmp-store`, `mmp-tal`, `mmp-tal-btleplug`, `mmp-tal-peripheral`, `mmp-types`, plus
`tokio` for the radio drive loop. Hand-rolled `std::env` argument parsing, no `clap`.

See `docs/IMPLEMENTATION_HANDBOOK.md` §4.2.
