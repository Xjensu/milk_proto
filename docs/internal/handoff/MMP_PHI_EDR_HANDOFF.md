# Energy-Dual Rendezvous implementation record

Status: research-only follow-up; no normative MMP/1 or transport change.

The Rust simulator implements the controlled Energy-Dual Rendezvous equations
described in [`MMP_PHI_SIMULATION.md`](MMP_PHI_SIMULATION.md). The implementation
keeps the baseline activity product fixed and computes the square-root
stationary point. Unit tests cover the fixed-product invariant, equal energy
contribution at the stationary point, a representative model value, and
rejection of non-finite or non-representable inputs.

The result is conditional on a linear cost model and fixed contact hazard. It
does not measure BLE energy and does not select a mobile scheduler. The model
omits collisions, controller filtering, operating-system wakeups, radio
firmware, background policy, and whole-device energy.

The simulator also provides a baseline-only `ResearchExperiment` sidecar. It
records schedule provenance, seed, workload, and candidate adapter decisions
without changing wire bytes or routing behavior. A replay adapter may skip a
scheduled contact while advancing the same virtual time and trace identity.

Required evidence for a future comparison:

- identical schedule, seed, node count, workload, and simulator revision;
- delivery, latency, quota, store high-water, wire-byte, and contact-session
  metrics for both runs;
- device model, OS policy, controller energy, host wakeups, and repeated runs
  for any battery statement.

Current state: the host model and replay seam are tested; no calibrated BLE or
whole-device measurement is recorded.
