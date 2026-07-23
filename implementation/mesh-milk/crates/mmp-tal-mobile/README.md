# mmp-tal-mobile

Android/iOS BLE glue behind UniFFI callbacks (doc 12 §6). Deliberately (and
permanently) thin: on mobile **the platform owns the radio** — Kotlin/Swift implement
scanning, advertising, and GATT natively and drive `mmp-ffi`'s node directly with the
same `TalEvent` vocabulary. There is no Rust radio code on phones, so this crate only
re-exports the shared TAL types (`LinkId`, `LinkQuality`, `TalEvent`) and a small
`att_chunk(mtu)` fragmentation helper.

**Layering:** depends only on `mmp-tal`.

See `implementation/mesh-milk/MOBILE_INTEGRATION.md` §4 (Android) and §5 (iOS).
