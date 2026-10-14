# mmp-node-embassy

Bounded embedded orchestration on the Embassy async executor — the `no_std`
counterpart to `mmp-node`. The current shell provides an
`EmbeddedEventPump<INGRESS_CAPACITY, EGRESS_CAPACITY>`: a fixed-capacity ingress
FIFO over `TalEvent` and a fixed-capacity egress FIFO over already-created
`Vec<u8>` bursts. It adds no executor, hardware backend, or protocol logic.

```rust
let mut pump = EmbeddedEventPump::<8, 4>::default();
pump.try_push_ingress(TalEvent::LinkUp {
    link: 1,
    initiator: false,
    quality: LinkQuality::default(),
})?;
pump.try_push_egress(OutboundData { link: 1, bytes })?;
```

Ingress uses `mmp-tal-embedded::EmbeddedEventQueue`, including its existing
31-byte beacon and bounded link-data validation. Egress applies the same
`MAX_LINK_DATA_BYTES` bound and reports `OutboundError::Full` or
`OutboundError::Oversize` before admission. Both queues are FIFO and permit a
zero-capacity configuration for compile-time budget probes.

**Layering:** depends on the no-std profiles of `mmp-core`, `mmp-router`, and
`mmp-tal`. The current `mmp-store` implementation is filesystem-backed and
`std`-only, so it is deliberately not a dependency of this embedded shell;
an eventual flash-log store must implement the `mmp_core::BundleStore` seam
without widening the target feature closure.

The dependency feature fence is intentional: enabling `mmp-core`'s default
`std` feature would pull the `snow` Noise backend and host entropy support into
the Cortex-M graph. This crate therefore remains target-checkable while the
hardware TAL, executor wiring, and flash persistence remain separate E7 gates.

See `docs/implementation/README.md` §4.9.
