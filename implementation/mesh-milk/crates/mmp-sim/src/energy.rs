//! Simulator-only rendezvous-energy model for the MMP/Φ research track.
//!
//! This is deliberately not a protocol parameter or a radio driver. It models
//! only the controlled hypothesis `hazard ≈ q · λ · ρ`, where `λ` and `ρ` are
//! normalized transmitter and receiver activity rates. The caller must replace
//! the idealized result with measured controller, OS, and whole-device energy.

/// One normalized rendezvous activity point and its modeled energy split.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RendezvousActivity {
    /// Normalized advertisement/transmit activity per day.
    pub tx_activity: f64,
    /// Normalized scan/receive activity per day.
    pub rx_activity: f64,
    /// Modeled transmit energy in the caller's daily energy unit.
    pub tx_energy: f64,
    /// Modeled receive energy in the caller's daily energy unit.
    pub rx_energy: f64,
    /// Sum of [`Self::tx_energy`] and [`Self::rx_energy`].
    pub energy: f64,
}

/// A platform-provided, aggregate observation for one controlled rendezvous
/// trace. This is input to the research harness only; none of these values are
/// protocol fields or scheduler parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RendezvousMeasurement {
    /// Stable identifier shared by the baseline and candidate runs.
    pub trace_id: u64,
    /// Number of advertisement/transmit events emitted by the controller.
    pub tx_events: u64,
    /// Number of scan/receive windows requested or observed.
    pub rx_windows: u64,
    /// Number of contact opportunities in the replayed trace.
    pub contact_attempts: u64,
    /// Contact opportunities for which discovery completed.
    pub discoveries: u64,
    /// Number of bundles offered to the delivery workload.
    pub offered_bundles: u64,
    /// Number of offered bundles delivered to the destination.
    pub delivered_bundles: u64,
    /// Useful application payload bytes delivered.
    pub useful_payload_bytes: u64,
    /// Total measured radio airtime in milliseconds.
    pub airtime_ms: f64,
    /// Sum of discovery latencies for the recorded samples, in milliseconds.
    pub discovery_latency_ms_sum: f64,
    /// Number of discovery latency samples included in the sum.
    pub discovery_latency_samples: u64,
    /// Sum of delivery latencies for the recorded samples, in milliseconds.
    pub delivery_latency_ms_sum: f64,
    /// Number of delivery latency samples included in the sum.
    pub delivery_latency_samples: u64,
    /// Measured radio/controller energy in mWh.
    pub radio_energy_mwh: f64,
    /// Measured host/SoC energy in mWh.
    pub host_energy_mwh: f64,
    /// Measured whole-device energy in mWh. It is reported separately rather
    /// than added to the radio and host channels, which may overlap.
    pub whole_device_energy_mwh: f64,
    /// Number of host wakeups attributed to the experiment.
    pub host_wakeups: u64,
}

/// Validation failures for externally supplied research measurements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeasurementError {
    /// A floating-point measurement was NaN or infinite.
    NonFiniteValue,
    /// A floating-point measurement was negative.
    NegativeValue,
    /// A success/sample count exceeds its corresponding total.
    InconsistentCounts,
    /// Baseline and candidate observations do not describe one trace.
    MismatchedTrace,
    /// A derived summary value cannot be represented as a finite number.
    SummaryOverflow,
}

impl RendezvousMeasurement {
    /// Validate this externally supplied observation before comparison.
    pub fn validate(&self) -> Result<(), MeasurementError> {
        for value in [
            self.airtime_ms,
            self.discovery_latency_ms_sum,
            self.delivery_latency_ms_sum,
            self.radio_energy_mwh,
            self.host_energy_mwh,
            self.whole_device_energy_mwh,
        ] {
            if !value.is_finite() {
                return Err(MeasurementError::NonFiniteValue);
            }
            if value < 0.0 {
                return Err(MeasurementError::NegativeValue);
            }
        }
        if self.discoveries > self.contact_attempts
            || self.delivered_bundles > self.offered_bundles
            || self.discovery_latency_samples > self.discoveries
            || self.delivery_latency_samples > self.delivered_bundles
        {
            return Err(MeasurementError::InconsistentCounts);
        }
        Ok(())
    }
}

/// A baseline/candidate pair collected from the same contact trace.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PairedRendezvousMeasurement {
    baseline: RendezvousMeasurement,
    candidate: RendezvousMeasurement,
}

impl PairedRendezvousMeasurement {
    /// Validate and pair two observations without changing either one.
    pub fn new(
        baseline: RendezvousMeasurement,
        candidate: RendezvousMeasurement,
    ) -> Result<Self, MeasurementError> {
        baseline.validate()?;
        candidate.validate()?;
        if baseline.trace_id != candidate.trace_id {
            return Err(MeasurementError::MismatchedTrace);
        }
        Ok(Self { baseline, candidate })
    }

    /// Produce a deterministic comparison summary from the validated pair.
    pub fn summary(&self) -> Result<RendezvousComparison, MeasurementError> {
        let baseline_discovery_rate = rate(self.baseline.discoveries, self.baseline.contact_attempts);
        let candidate_discovery_rate = rate(self.candidate.discoveries, self.candidate.contact_attempts);
        let baseline_delivery_rate = rate(self.baseline.delivered_bundles, self.baseline.offered_bundles);
        let candidate_delivery_rate = rate(self.candidate.delivered_bundles, self.candidate.offered_bundles);
        let whole_device_energy_ratio = ratio(
            self.candidate.whole_device_energy_mwh,
            self.baseline.whole_device_energy_mwh,
        )?;
        Ok(RendezvousComparison {
            trace_id: self.baseline.trace_id,
            baseline_discovery_rate,
            candidate_discovery_rate,
            discovery_rate_delta: candidate_discovery_rate - baseline_discovery_rate,
            baseline_delivery_rate,
            candidate_delivery_rate,
            delivery_rate_delta: candidate_delivery_rate - baseline_delivery_rate,
            baseline_radio_energy_mwh: self.baseline.radio_energy_mwh,
            candidate_radio_energy_mwh: self.candidate.radio_energy_mwh,
            baseline_host_energy_mwh: self.baseline.host_energy_mwh,
            candidate_host_energy_mwh: self.candidate.host_energy_mwh,
            baseline_whole_device_energy_mwh: self.baseline.whole_device_energy_mwh,
            candidate_whole_device_energy_mwh: self.candidate.whole_device_energy_mwh,
            whole_device_energy_ratio,
            baseline_discovery_latency_ms: mean(
                self.baseline.discovery_latency_ms_sum,
                self.baseline.discovery_latency_samples,
            ),
            candidate_discovery_latency_ms: mean(
                self.candidate.discovery_latency_ms_sum,
                self.candidate.discovery_latency_samples,
            ),
            baseline_delivery_latency_ms: mean(
                self.baseline.delivery_latency_ms_sum,
                self.baseline.delivery_latency_samples,
            ),
            candidate_delivery_latency_ms: mean(
                self.candidate.delivery_latency_ms_sum,
                self.candidate.delivery_latency_samples,
            ),
        })
    }
}

/// Stable fields used to compare two validated rendezvous observations.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RendezvousComparison {
    pub trace_id: u64,
    pub baseline_discovery_rate: f64,
    pub candidate_discovery_rate: f64,
    pub discovery_rate_delta: f64,
    pub baseline_delivery_rate: f64,
    pub candidate_delivery_rate: f64,
    pub delivery_rate_delta: f64,
    pub baseline_radio_energy_mwh: f64,
    pub candidate_radio_energy_mwh: f64,
    pub baseline_host_energy_mwh: f64,
    pub candidate_host_energy_mwh: f64,
    pub baseline_whole_device_energy_mwh: f64,
    pub candidate_whole_device_energy_mwh: f64,
    /// Candidate divided by baseline; `None` means the baseline reading was 0.
    pub whole_device_energy_ratio: Option<f64>,
    pub baseline_discovery_latency_ms: Option<f64>,
    pub candidate_discovery_latency_ms: Option<f64>,
    pub baseline_delivery_latency_ms: Option<f64>,
    pub candidate_delivery_latency_ms: Option<f64>,
}

fn rate(successes: u64, total: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        successes as f64 / total as f64
    }
}

fn mean(sum: f64, samples: u64) -> Option<f64> {
    if samples == 0 {
        None
    } else {
        Some(sum / samples as f64)
    }
}

fn ratio(numerator: f64, denominator: f64) -> Result<Option<f64>, MeasurementError> {
    if denominator == 0.0 {
        return Ok(None);
    }
    let result = numerator / denominator;
    if result.is_finite() {
        Ok(Some(result))
    } else {
        Err(MeasurementError::SummaryOverflow)
    }
}

/// Profiles used by the research-only EDR comparison matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RendezvousExperimentProfile {
    /// Current MMP/1 binary-spray workload used as the comparison baseline.
    BinarySprayBaseline,
    /// An EDR duty-cycle candidate with an altered transmit activity rate.
    EdrDutyCycle,
}

/// One fixed-hazard activity point in the EDR comparison matrix.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RendezvousExperimentPoint {
    /// Matrix profile associated with this point.
    pub profile: RendezvousExperimentProfile,
    /// Transmit activity relative to the measured baseline activity.
    pub tx_scale: f64,
    /// Activity and modeled energy for the point.
    pub activity: RendezvousActivity,
}

/// Default transmit-activity multipliers for the deterministic research sweep.
///
/// The point at `1.0` is the current binary-spray baseline. Other points are
/// EDR candidates and are deliberately bounded around it; these are normalized
/// model values, not BLE timing recommendations.
pub const DEFAULT_EDR_TX_SCALES: [f64; 7] = [0.25, 0.5, 0.75, 1.0, 2.0, 4.0, 8.0];

/// Controlled square-root optimum for the energy-dual rendezvous hypothesis.
///
/// Activity values are normalized rates rather than BLE timings. The model is
/// useful for comparing candidate duty-cycle points, but it does not account
/// for collisions, controller scheduling, OS wakeups, or platform support.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RendezvousEnergyModel {
    baseline_tx_activity: f64,
    baseline_rx_activity: f64,
    tx_cost_per_activity: f64,
    rx_cost_per_activity: f64,
    hazard_product: f64,
}

impl RendezvousEnergyModel {
    /// Construct a model from one measured baseline point.
    ///
    /// All four arguments must be finite and strictly positive. The baseline
    /// rendezvous product is retained as the fixed-hazard target for the
    /// idealized optimization.
    pub fn from_baseline(
        tx_activity: f64,
        rx_activity: f64,
        tx_cost_per_activity: f64,
        rx_cost_per_activity: f64,
    ) -> Option<Self> {
        if [tx_activity, rx_activity, tx_cost_per_activity, rx_cost_per_activity]
            .iter()
            .any(|value| !value.is_finite() || *value <= 0.0)
        {
            return None;
        }
        let hazard_product = tx_activity * rx_activity;
        let tx_energy = tx_activity * tx_cost_per_activity;
        let rx_energy = rx_activity * rx_cost_per_activity;
        let optimum_tx_activity = hazard_product * rx_cost_per_activity / tx_cost_per_activity;
        let optimum_rx_activity = hazard_product * tx_cost_per_activity / rx_cost_per_activity;
        let optimum_tx_energy = optimum_tx_activity.sqrt() * tx_cost_per_activity;
        let optimum_rx_energy = optimum_rx_activity.sqrt() * rx_cost_per_activity;
        if [
            hazard_product,
            tx_energy,
            rx_energy,
            tx_energy + rx_energy,
            optimum_tx_activity,
            optimum_rx_activity,
            optimum_tx_energy,
            optimum_rx_energy,
            optimum_tx_energy + optimum_rx_energy,
        ]
        .iter()
        .any(|value| !value.is_finite() || *value <= 0.0)
        {
            return None;
        }
        Some(Self {
            baseline_tx_activity: tx_activity,
            baseline_rx_activity: rx_activity,
            tx_cost_per_activity,
            rx_cost_per_activity,
            hazard_product,
        })
    }

    /// The baseline modeled energy in the caller's daily energy unit.
    pub fn baseline_energy(&self) -> f64 {
        self.baseline_tx_activity * self.tx_cost_per_activity
            + self.baseline_rx_activity * self.rx_cost_per_activity
    }

    /// The fixed normalized hazard product retained by the model.
    pub fn hazard_product(&self) -> f64 {
        self.hazard_product
    }

    /// Evaluate one candidate activity point under the fixed-hazard model.
    pub fn evaluate(&self, tx_activity: f64, rx_activity: f64) -> Option<RendezvousActivity> {
        if !tx_activity.is_finite()
            || !rx_activity.is_finite()
            || tx_activity <= 0.0
            || rx_activity <= 0.0
            || (tx_activity * rx_activity - self.hazard_product).abs() > self.hazard_product * 1e-9
        {
            return None;
        }
        let tx_energy = tx_activity * self.tx_cost_per_activity;
        let rx_energy = rx_activity * self.rx_cost_per_activity;
        Some(RendezvousActivity {
            tx_activity,
            rx_activity,
            tx_energy,
            rx_energy,
            energy: tx_energy + rx_energy,
        })
    }

    /// Build a deterministic, wire-neutral matrix for the binary-spray
    /// baseline and bounded EDR duty-cycle candidates.
    ///
    /// Every point preserves the model's normalized hazard product. The
    /// matrix is intended for trace replay and measurement planning only; it
    /// does not run the router, change a copy budget, or select a scheduler
    /// policy. `None` is returned only if the model cannot represent one of
    /// the derived points as finite positive values.
    pub fn default_experiment_matrix(&self) -> Option<[RendezvousExperimentPoint; 7]> {
        Some([
            self.experiment_point(DEFAULT_EDR_TX_SCALES[0])?,
            self.experiment_point(DEFAULT_EDR_TX_SCALES[1])?,
            self.experiment_point(DEFAULT_EDR_TX_SCALES[2])?,
            self.experiment_point(DEFAULT_EDR_TX_SCALES[3])?,
            self.experiment_point(DEFAULT_EDR_TX_SCALES[4])?,
            self.experiment_point(DEFAULT_EDR_TX_SCALES[5])?,
            self.experiment_point(DEFAULT_EDR_TX_SCALES[6])?,
        ])
    }

    fn experiment_point(&self, tx_scale: f64) -> Option<RendezvousExperimentPoint> {
        if !tx_scale.is_finite() || tx_scale <= 0.0 {
            return None;
        }
        let tx_activity = self.baseline_tx_activity * tx_scale;
        if !tx_activity.is_finite() || tx_activity <= 0.0 {
            return None;
        }
        let rx_activity = self.hazard_product / tx_activity;
        let activity = self.evaluate(tx_activity, rx_activity)?;
        Some(RendezvousExperimentPoint {
            profile: if tx_scale == 1.0 {
                RendezvousExperimentProfile::BinarySprayBaseline
            } else {
                RendezvousExperimentProfile::EdrDutyCycle
            },
            tx_scale,
            activity,
        })
    }

    /// Compute the square-root minimum of `c_tx·λ + c_rx·ρ` subject to
    /// `λ·ρ = hazard_product`.
    pub fn optimum(&self) -> RendezvousActivity {
        let tx_activity = (self.hazard_product * self.rx_cost_per_activity / self.tx_cost_per_activity).sqrt();
        let rx_activity = (self.hazard_product * self.tx_cost_per_activity / self.rx_cost_per_activity).sqrt();
        // The constructor guarantees positive finite inputs, so this cannot
        // fail unless a future caller changes the representation invariants.
        self.evaluate(tx_activity, rx_activity).expect("valid square-root optimum")
    }

    /// Baseline energy divided by the idealized optimum energy.
    pub fn idealized_gain(&self) -> f64 {
        self.baseline_energy() / self.optimum().energy
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edr_optimum_balances_energy_contributions_and_preserves_hazard() {
        let model = RendezvousEnergyModel::from_baseline(1.0, 1.0, 0.12, 4.5).unwrap();
        let optimum = model.optimum();

        assert!((optimum.tx_activity * optimum.rx_activity - model.hazard_product()).abs() < 1e-12);
        assert!((optimum.tx_energy - optimum.rx_energy).abs() < 1e-12);
        assert!(optimum.energy < model.baseline_energy());
    }

    #[test]
    fn edr_reports_the_idealized_gain_without_claiming_a_radio_result() {
        let model = RendezvousEnergyModel::from_baseline(1.0, 1.0, 0.12, 4.5).unwrap();
        let gain = model.idealized_gain();

        assert!(gain > 3.0 && gain < 3.3);
    }

    #[test]
    fn edr_rejects_nonrepresentable_baseline_terms() {
        assert!(RendezvousEnergyModel::from_baseline(f64::MAX, f64::MAX, 1.0, 1.0).is_none());
        assert!(RendezvousEnergyModel::from_baseline(
            f64::MIN_POSITIVE,
            f64::MIN_POSITIVE,
            1.0,
            1.0,
        )
        .is_none());
        assert!(RendezvousEnergyModel::from_baseline(1.0, 1.0, f64::MAX, f64::MAX).is_none());
    }

    fn measurement(trace_id: u64, discoveries: u64, delivered: u64, energy: f64) -> RendezvousMeasurement {
        RendezvousMeasurement {
            trace_id,
            tx_events: 40,
            rx_windows: 20,
            contact_attempts: 4,
            discoveries,
            offered_bundles: 4,
            delivered_bundles: delivered,
            useful_payload_bytes: 1_024,
            airtime_ms: 80.0,
            discovery_latency_ms_sum: 8.0,
            discovery_latency_samples: discoveries,
            delivery_latency_ms_sum: 12.0,
            delivery_latency_samples: delivered,
            radio_energy_mwh: energy / 2.0,
            host_energy_mwh: energy / 4.0,
            whole_device_energy_mwh: energy,
            host_wakeups: 5,
        }
    }

    #[test]
    fn paired_edr_measurement_reports_reproducible_rates_energy_and_latency() {
        let baseline = measurement(7, 2, 2, 100.0);
        let candidate = measurement(7, 3, 3, 75.0);
        let summary = PairedRendezvousMeasurement::new(baseline, candidate)
            .unwrap()
            .summary()
            .unwrap();

        assert_eq!(summary.trace_id, 7);
        assert_eq!(summary.baseline_discovery_rate, 0.5);
        assert_eq!(summary.candidate_discovery_rate, 0.75);
        assert_eq!(summary.discovery_rate_delta, 0.25);
        assert_eq!(summary.baseline_delivery_rate, 0.5);
        assert_eq!(summary.candidate_delivery_rate, 0.75);
        assert_eq!(summary.delivery_rate_delta, 0.25);
        assert_eq!(summary.whole_device_energy_ratio, Some(0.75));
        assert_eq!(summary.baseline_discovery_latency_ms, Some(4.0));
        assert_eq!(summary.candidate_delivery_latency_ms, Some(4.0));
    }

    #[test]
    fn paired_edr_measurement_rejects_mismatched_trace_and_bad_counts() {
        let baseline = measurement(7, 2, 2, 100.0);
        let mismatched = measurement(8, 3, 3, 75.0);
        assert_eq!(
            PairedRendezvousMeasurement::new(baseline, mismatched),
            Err(MeasurementError::MismatchedTrace)
        );

        let mut invalid = measurement(7, 2, 2, 100.0);
        invalid.discoveries = invalid.contact_attempts + 1;
        assert_eq!(
            PairedRendezvousMeasurement::new(invalid, measurement(7, 3, 3, 75.0)),
            Err(MeasurementError::InconsistentCounts)
        );
    }

    #[test]
    fn paired_edr_measurement_rejects_nonfinite_energy() {
        let mut invalid = measurement(7, 2, 2, 100.0);
        invalid.whole_device_energy_mwh = f64::NAN;
        assert_eq!(
            PairedRendezvousMeasurement::new(invalid, measurement(7, 3, 3, 75.0)),
            Err(MeasurementError::NonFiniteValue)
        );
    }

    #[test]
    fn edr_experiment_matrix_is_deterministic_and_marks_binary_spray_baseline() {
        let model = RendezvousEnergyModel::from_baseline(1.0, 1.0, 0.12, 4.5).unwrap();
        let first = model.default_experiment_matrix().unwrap();
        let second = model.default_experiment_matrix().unwrap();

        assert_eq!(first, second);
        assert_eq!(first.len(), 7);
        assert_eq!(first[3].tx_scale, 1.0);
        assert_eq!(first[3].profile, RendezvousExperimentProfile::BinarySprayBaseline);
        for point in first {
            assert!((point.activity.tx_activity * point.activity.rx_activity
                - model.hazard_product())
                .abs()
                < 1e-9);
        }
        assert!(first
            .iter()
            .any(|point| point.profile == RendezvousExperimentProfile::EdrDutyCycle));
    }
}
