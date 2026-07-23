# mmp-tal-embedded

nRF52 (`nrf-softdevice`) / ESP32 (NimBLE) transport adaptation layer. Currently an
11-line `no_std`-clean placeholder — a deliberate scope decision (post-GA unless a
customer forces it, Epic 7), not an oversight. Acceptance gate already recorded in
the code: ≤256 KiB flash / ≤48 KiB static RAM (doc 12 §7.6) plus the OQ-7 Reed-Solomon
decode benchmark on nRF52-class silicon.

**Layering:** depends only on `mmp-tal`.

See `docs/IMPLEMENTATION_HANDBOOK.md` §4.9.
