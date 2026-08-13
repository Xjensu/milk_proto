# Contact-Survival Simulator Model Implementation Plan

**Goal:** Add a bounded, deterministic simulator-only evaluator for the MMP/Phi contact-survival hypothesis without changing MMP/1 wire behavior, routing, or production APIs.

**Architecture:** `mmp-sim` exposes a pure mathematical module. A caller supplies a validated contact observation and coefficients; the module computes a stable hazard, survival probability, and deterministic transfer ranking. The model is explicitly experimental: it does not alter `Sim`, `Node`, `mmp-core`, or `mmp-router`, and it makes no physical-performance claim.

**Tech Stack:** Rust, existing `mmp-sim` crate, `std` math only, no new dependencies.

### Task 1: Define the bounded research API

**Files:**
- Create: `implementation/mesh-milk/crates/mmp-sim/src/contact_survival.rs`
- Modify: `implementation/mesh-milk/crates/mmp-sim/src/lib.rs`

- [x] Add `ContactObservation` with `distance_m`, `radial_velocity_mps`, `distance_stddev_m`, and `packet_error_rate`.
- [x] Add `HazardCoefficients` with finite coefficients for the four observation terms plus an intercept.
- [x] Add `TransferCandidate` with a stable `carrier_id`, positive `value`, bounded `bytes`, positive `goodput_bps`, `setup_cost`, and `per_byte_cost`.
- [x] Add `SurvivalError` variants for non-finite values, range violations, excessive horizon, excessive candidates, and non-finite derived values.
- [x] Export the module from `lib.rs` without connecting it to `Sim`.

### Task 2: Implement stable survival evaluation

**Files:**
- Modify: `implementation/mesh-milk/crates/mmp-sim/src/contact_survival.rs`

- [x] Validate observations: distance and standard deviation in `[0, 1_000_000]`, radial velocity in `[-1_000_000, 1_000_000]`, packet error rate in `[0, 1]`, and every coefficient finite with absolute value at most `1_000_000`.
- [x] Validate the horizon in `[0, 86_400]` seconds.
- [x] Compute the linear predictor and apply a numerically stable softplus hazard. Return `exp(-hazard * horizon)` and reject any non-finite intermediate.
- [x] Preserve monotonicity: a longer horizon must never increase survival for the same observation and coefficients.

### Task 3: Implement deterministic transfer ranking

**Files:**
- Modify: `implementation/mesh-milk/crates/mmp-sim/src/contact_survival.rs`

- [x] Cap the candidate list at 256 and payload size at 8 MiB.
- [x] Compute `required_seconds = bytes / goodput_bps`, `finish_probability = survival_probability(required_seconds)`, and `score = value * finish_probability / (setup_cost + per_byte_cost * bytes)`.
- [x] Require finite, non-negative values and strictly positive total modeled cost.
- [x] Reject duplicate carrier IDs.
- [x] Sort descending by score, then descending finish probability, ascending cost, and ascending carrier ID.
- [x] Return only local decisions; do not mutate copy budgets or call production routing code.

### Task 4: Add focused tests

**Files:**
- Modify: `implementation/mesh-milk/crates/mmp-sim/src/contact_survival.rs`

- [x] Test identical inputs produce equality-identical results.
- [x] Test survival is non-increasing with horizon and finish probability is non-increasing with payload size.
- [x] Test invalid NaN, infinity, negative/range-invalid values, duplicate IDs, excessive candidates, and zero goodput are rejected.
- [x] Test deterministic tie-breaking and the 256-candidate/8 MiB bounds.
- [x] Keep the public API independent of `mmp-core`, `mmp-node`, wire frames, and production router state.

### Task 5: Verify and record status

**Files:**
- The two implementation files above were committed before this documentation
  closeout (`3be3330` and `b12d377`). This task updates this plan only.

- [x] Run focused module tests with `cargo test -p mmp-sim --lib contact_survival --locked`; 5 tests pass.
- [x] Run `cargo clippy -p mmp-sim --all-targets --locked -- -D warnings`; the local run completed with no findings.
- [x] Run `git diff --check`; no whitespace errors were reported.
- [x] Confirm the implementation commits in history (`3be3330` adds the model and `b12d377` hardens its invariants).
- [ ] Run the full simulator target set in CI/nightly; long-running simulator and platform/hardware checks remain outside this local pass.

**Final status — FIXED:** The bounded simulator model and its focused arithmetic tests are implemented and committed; this plan now records the verified local evidence without claiming unrun CI or nightly checks.

**Evidence boundary:** Passing tests validate arithmetic, bounds, and deterministic behavior only. They do not validate RSSI, Channel Sounding, human mobility, battery use, delivery improvement, or the claim that MMP/Phi is 2x better. Those claims remain untested until reproducible simulator, CI, and physical measurements are available.
