# mmp-tal-peripheral

PC BLE peripheral role (GATT server + advertiser): the M5b milestone. Linux backend
on `bluer` (real BlueZ D-Bus GATT server, verified against actual BlueZ); Windows and
macOS are `PeriphError::Unsupported` stubs with a documented WinRT design sketch —
not yet implemented, not silently pretended to work.

**Layering:** depends on `mmp-tal` and `mmp-types`, plus `tokio`. Sits above the
sans-IO core, same as `mmp-tal-btleplug` — `layer-check` only polices crates below
`mmp-node`. GATT UUIDs are re-exported from `mmp-tal`'s single source of truth.

See `docs/IMPLEMENTATION_HANDBOOK.md` §4.1 and `specification/09-transport-ble.md`.
