# MMP/Φ simulator research track

Status: controlled mathematical experiment; not a wire-protocol change and
not a production battery claim. The frozen `specification/` directory is
unchanged.

## Energy-Dual Rendezvous (EDR)

The simulator exposes `mmp_sim::energy::RendezvousEnergyModel`. It evaluates
the deliberately narrow hypothesis

```text
h ≈ q · λ · ρ
E = c_tx · λ + c_rx · ρ
```

where `λ` and `ρ` are normalized transmit and receive activity rates, `q` is
held constant, and the baseline product `λρ` is preserved. The constrained
minimum is therefore

```text
λ* = sqrt(K · c_rx / c_tx)
ρ* = sqrt(K · c_tx / c_rx)
```

The implementation uses normalized rates rather than BLE timings. It does not
model collisions, channel overlap, controller filtering, OS wakeups, radio
firmware, background execution policy, or whole-phone energy. Those effects
must be measured before an EDR point can influence a platform scheduler.

For the illustrative baseline `(λ, ρ) = (1, 1)` with modeled daily costs
`c_tx = 0.12` and `c_rx = 4.5`, the model reports approximately:

```text
λ*       = 6.1237
ρ*       = 0.1633
E_base   = 4.6200
E_opt    = 1.4697
idealized gain ≈ 3.14×
```

This is a sanity-check of the square-root algebra only. It is not evidence that
MMP/1 or any phone achieves a 3.14× or 2× battery improvement. The relevant
acceptance experiment must compare discovery probability, missed contacts,
radio/controller energy, host wakeups, whole-device energy, airtime, and
delivery latency against the existing duty-cycle policy.

## Verification

The two simulator tests pin the fixed-hazard invariant and the idealized gain
calculation:

```text
cargo test -p mmp-sim edr_ -- --nocapture
```

The next research step is to feed measured activity/energy samples from Linux,
Android, and iOS harnesses into this model and compare EDR candidates against
the current policy under the same contact traces. Until that evidence exists,
EDR remains opt-in research instrumentation and must not change v1 behavior.
