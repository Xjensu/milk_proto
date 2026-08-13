# mmp-tal-peripheral

PC BLE peripheral role (GATT server + advertiser): the M5b milestone. Linux backend
on `bluer` (real BlueZ D-Bus GATT server, verified against actual BlueZ); Windows and
macOS are `PeriphError::Unsupported` stubs with a documented WinRT design sketch —
not yet implemented, not silently pretended to work.

**Layering:** depends on `mmp-tal` and `mmp-types`, plus `tokio`. Sits above the
sans-IO core, same as `mmp-tal-btleplug` — `layer-check` only polices crates below
`mmp-node`. GATT UUIDs are re-exported from `mmp-tal`'s single source of truth.

Linux notifications use a 250 ms per-chunk writability deadline. A timed-out
notification retires the link and returns `PeriphError::NotifyTimeout`, keeping
a stalled BlueZ peer from blocking the host drive loop or shutdown path.

Dropping the public event receiver is also terminal: the Linux backend fences
new admission, signals shutdown, and retires active BlueZ links through the
same bounded lifecycle path. This prevents a host that stopped consuming
events from retaining GATT tasks and radio resources indefinitely.

See `docs/IMPLEMENTATION_HANDBOOK.md` §4.1 and `specification/09-transport-ble.md`.
