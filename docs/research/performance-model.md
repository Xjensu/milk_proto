# Performance and scale model

The baseline model separates wire bytes, contact sessions, delivery, latency, store high-water, radio/controller activity, and whole-device energy. Simulator counters for outbound and inbound bytes are in-memory transfer counts; they are not BLE airtime or battery measurements.

Store pressure is bounded by class lifetimes, quotas, per-origin buckets, and in-flight caps. The reference values are 32 KiB Core, 128 MiB Standard, and 2 GiB Anchor. The 1,000-node and 10,000-node targets are design tiers, not demonstrated results. Uniform deterministic simulations cover some quota and persistence behavior; clustered mobility and hardware-calibrated latency gates remain open.

The `<5%/day` phone battery target is a requirement for measurement, not a current result. A valid battery study must report device model, OS policy, scan and advertising duty cycle, contact workload, screen state, controller energy, whole-device energy, and repeated runs. The Energy-Dual Rendezvous parameterization is research material until such data exists.
