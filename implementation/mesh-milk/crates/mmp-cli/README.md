# mmp-cli

PC node binary and debugging/inspection tool: `keygen`, `demo` (two in-memory nodes,
no radio needed), `inspect` (reads the local diagnostics journey log), plus real BLE
subcommands — `connect`/`send` (central role, over `mmp-tal-btleplug`) and `listen`
(peripheral role, over `mmp-tal-peripheral`, Linux/BlueZ only today).

`mmp-cli demo` keeps its human-readable transcript by default. For beta harnesses,
`mmp-cli demo --json` runs the same two-node exchange and emits exactly one
schema-versioned JSON report on stdout; diagnostics are bounded and contain no
seeds, private keys, raw frames, or ciphertext.

The real BLE commands treat an unexpected end of the transport event stream as
a failed run (exit code `2`), even when the local snapshot can still be written.
Use Ctrl-C for an intentional clean shutdown; this distinction keeps a beta
harness from recording an adapter/backend failure as a successful session.

**Layering:** a top-level consumer — depends on `mmp-node`, `mmp-core`, `mmp-crypto`,
`mmp-store`, `mmp-tal`, `mmp-tal-btleplug`, `mmp-tal-peripheral`, `mmp-types`, plus
`tokio` for the radio drive loop. Hand-rolled `std::env` argument parsing, no `clap`.

See `docs/implementation/README.md` §4.2.
