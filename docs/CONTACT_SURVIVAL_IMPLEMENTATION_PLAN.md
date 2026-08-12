# Contact-Survival Simulator Model Implementation Plan

**Goal:** Add a bounded, deterministic simulator-only evaluator for the MMP/Φ contact-survival hypothesis without changing MMP/1 wire behavior, routing, or production APIs.

**Architecture:** `mmp-sim` will expose a pure mathematical module. A caller supplies a validated contact observation and coefficients; the module computes a stable hazard, survival probability, and deterministic transfer ranking. The model is explicitly experimental: it does not alter `Sim`, `Node`, `mmp-core`, or `mmp-router`, and it makes no physical-performance claim.

**Tech Stack:** Rust, existing `mmp-sim` crate, `std` math only, no new dependencies.

---

### Task 1: Define the bounded research API

**Files:**
- Create: `implementation/mesh-milk/crates/mmp-sim/src/contact_survival.rs`
- Modify: `implementation/mesh-milk/crates/mmp-sim/src/lib.rs`

- [ ] Add `ContactObservation` with `distance_m`, `radial_velocity_mps`, `distance_stddev_m`, and `packet_error_rate`.
- [ ] Add `HazardCoefficients` with finite coefficients for the four observation terms plus an intercept.
- [ ] Add `TransferCandidate` with a stable `carrier_id`, positive `value`, bounded `bytes`, positive `goodput_bps`, `setup_cost`, and `per_byte_cost`.
- [ ] Add `SurvivalError` variants for non-finite values, range violations, excessive horizon, excessive candidates, and non-finite derived values.
- [ ] Export the module from `lib.rs` without connecting it to `Sim`.

### Task 2: Implement stable survival evaluation

**Files:**
- Modify: `implementation/mesh-milk/crates/mmp-sim/src/contact_survival.rs`

- [ ] Validate observations: distance and standard deviation in `[0, 1_000_000]`, radial velocity in `[-1_000_000, 1_000_000]`, packet error rate in `[0, 1]`, and every coefficient finite with absolute value at most `1_000_000`.
- [ ] Validate the horizon in `[0, 86_400]` seconds.
- [ ] Compute the linear predictor and apply a numerically stable softplus hazard. Return `exp(-hazard * horizon)` and reject any non-finite intermediate.
- [ ] Preserve monotonicity: a longer horizon must never increase survival for the same observation and coefficients.

### Task 3: Implement deterministic transfer ranking

**Files:**
- Modify: `implementation/mesh-milk/crates/mmp-sim/src/contact_survival.rs`

- [ ] Cap the candidate list at 256 and payload size at 8 MiB.
- [ ] Compute `required_seconds = bytes / goodput_bps`, `finish_probability = survival_probability(required_seconds)`, and `score = value * finish_probability / (setup_cost + per_byte_cost * bytes)`.
- [ ] Require finite, non-negative values and strictly positive total modeled cost.
- [ ] Reject duplicate carrier IDs.
- [ ] Sort descending by score, then descending finish probability, ascending cost, and ascending carrier ID.
- [ ] Return only local decisions; do not mutate copy budgets or call production routing code.

### Task 4: Add focused tests

**Files:**
- Modify: `implementation/mesh-milk/crates/mmp-sim/src/contact_survival.rs`

- [ ] Test identical inputs produce byte-for-byte/equality-identical results.
- [ ] Test survival is non-increasing with horizon and finish probability is non-increasing with payload size.
- [ ] Test invalid NaN, infinity, negative/range-invalid values, duplicate IDs, excessive candidates, and zero goodput are rejected.
- [ ] Test deterministic tie-breaking and the 256-candidate/8 MiB bounds.
- [ ] Test no public API refers to `mmp-core`, `mmp-node`, wire frames, or production router state.

### Task 5: Verify and commit

**Files:**
- Only the two implementation files above and this plan document.

- [ ] Run `cargo test -p mmp-sim --all-targets --locked` from `implementation/mesh-milk`; expect all tests to pass.
- [ ] Run `cargo clippy -p mmp-sim --all-targets --all-features --locked -- -D warnings`; expect no findings.
- [ ] Run `git diff --check` and verify `git diff --name-only -- specification` is empty.
- [ ] Inspect `git diff --name-only`; stage only the plan and simulator files.
- [ ] Commit with a normal message such as `feat(sim): add contact survival research model`.

**Evidence boundary:** Passing tests validate arithmetic, bounds, and deterministic behavior only. They do not validate RSSI, Channel Sounding, human mobility, battery use, delivery improvement, or the claim that MMP/Φ is 2× better.
