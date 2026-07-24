# mmp-tal

Transport Adaptation Layer vocabulary shared by every radio backend (doc 03 §7,
doc 12 §2): `TalEvent`, `LinkId`, `LinkQuality`, the frame length-prefix helpers, and
`MemLink` (an in-memory link used by tests and `mmp-sim`). A TAL is a byte shuttle —
it never parses protocol bytes; that stays in `mmp-core`'s session engine so every
radio backend (and the simulator) exercises identical protocol logic.

**Layering:** depends only on `mmp-types`.
`#![cfg_attr(not(feature = "std"), no_std)]`. Every concrete transport
(`mmp-tal-btleplug`, `mmp-tal-peripheral`, `mmp-tal-embedded`, `mmp-tal-mobile`) and
`mmp-node` itself depend on this crate.

See `docs/IMPLEMENTATION_HANDBOOK.md` §1.1 and `specification/09-transport-ble.md`.
