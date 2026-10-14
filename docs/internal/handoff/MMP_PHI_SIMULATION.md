# Energy-Dual Rendezvous simulation record

Status: research-only; no wire or scheduler change.

The simulator exposes `mmp_sim::energy::RendezvousEnergyModel`. It evaluates a
fixed-product model for normalized transmit and receive activity rates:

```text
h <= q
E = c_tx * h + c_rx * q
h* = sqrt(K * c_rx / c_tx)
q* = sqrt(K * c_tx / c_rx)
```

The result is an algebraic model check. It excludes collisions, channel overlap,
controller filtering, operating-system wakeups, radio firmware, background
execution policy, and whole-device energy. The model therefore cannot support a
battery claim or a production scheduler without device measurements.

Reproduction requires the simulator revision, seed, node count, schedule family,
workload, and all model parameters. Paired baseline and candidate runs must use
the same schedule and report delivery, latency, store high-water, wire-byte
counts, contact sessions, and quota status.
