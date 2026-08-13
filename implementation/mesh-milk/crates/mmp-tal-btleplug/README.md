# mmp-tal-btleplug

PC BLE central role (Linux/Windows/macOS) via the `btleplug` crate: scan, dial,
GATT write/notify shuttle, chunking, `LinkId` minting (`blake3(peripheral id)[0..8]`).
Falls back to the conservative 20-byte ATT chunk floor everywhere — `btleplug` 0.11
exposes no portable MTU query on any backend.

**Layering:** depends on `mmp-tal` and `mmp-types`, plus `btleplug`/`tokio`/`futures`/
`uuid`. Sits above the sans-IO core — `tools/layer-check` only polices crates below
`mmp-node`, so `tokio` is expected and allowed here. GATT UUIDs are re-exported from
`mmp-tal`'s single source of truth, not defined locally.

The single `events()` receiver is a lifecycle boundary: dropping it or
observing adapter-stream termination fences new scans/dials, retires tracked
links, and requests physical disconnects. Hosts should treat either condition
as terminal and recreate the central rather than continuing admission.

Each outbound connection check and Write-Without-Response chunk has a bounded
transport-local deadline. A stalled adapter retires the affected link and
returns an explicit timeout instead of pinning the host drive loop or leaving a
failed session live.

See `docs/IMPLEMENTATION_HANDBOOK.md` §4.1 and `specification/09-transport-ble.md`.
