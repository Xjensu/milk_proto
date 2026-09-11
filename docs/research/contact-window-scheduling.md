# Contact-window scheduling research

Short BLE contacts make setup overhead and queue choice part of delivery behavior. The current session caps continuous airtime at 60 seconds and performs reconciliation before transfer. A contact-window scheduler MAY use RSSI history, channel-sounding measurements, or observed dwell time to order data, but it MUST expose uncertainty and MUST degrade to ordinary ordering when measurements are unavailable.

The Energy-Dual Rendezvous model studies a contact split with `h <= q` and `E = c_tx * h + c_rx * q`. Under a fixed product, the algebraic optimum is `h* = sqrt(K * c_rx / c_tx)` and `q* = sqrt(K * c_tx / c_rx)`. The simulator result is an idealized model check. It excludes collisions, controller filtering, OS wakeups, radio firmware, and whole-device energy, so it is not a battery result.

The next experiment must compare identical workloads and seeds across schedules and measure discovery probability, missed contacts, controller and radio energy, host wakeups, whole-device energy, airtime, storage, and latency. Bluetooth Core 6 monitoring and Channel Sounding are optional capabilities and cannot be assumed across Android, iOS, Linux, and embedded targets.
