# mmp-tal-embedded

nRF52 (`nrf-softdevice`) / ESP32 (NimBLE) transport adaptation layer. Hardware
backends remain out of scope, but the crate now provides a generic
`EmbeddedEventQueue<const CAPACITY>` for bounded, allocation-free-after-
construction TAL ingress. It reuses the shared `TalEvent` vocabulary and rejects
events beyond the existing beacon/session envelope limits. The queue is an
adapter seam, not a radio driver or a new protocol implementation.

The hardware acceptance gate remains: ≤256 KiB flash / ≤48 KiB static RAM (doc 12
§7.6) plus the OQ-7 Reed-Solomon decode benchmark on nRF52-class silicon.

**Layering:** depends on `mmp-tal` and the existing `mmp-types` frame limits only.

See `docs/IMPLEMENTATION_HANDBOOK.md` §4.9.
