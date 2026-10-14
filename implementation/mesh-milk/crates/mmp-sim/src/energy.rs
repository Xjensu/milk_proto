//! Simulator-only rendezvous-energy model for the MMP/Φ research track.
//!
//! This is deliberately not a protocol parameter or a radio driver. It models
//! only the controlled hypothesis `hazard ≈ q · λ · ρ`, where `λ` and `ρ` are
//! normalized transmitter and receiver activity rates. The caller must replace
//! the idealized result with measured controller, OS, and whole-device energy.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Versioned schema name for one measured rendezvous trace.
pub const RENDEZVOUS_MEASUREMENT_JSON_SCHEMA: &str = "mmp-sim.rendezvous-measurement";
/// Versioned schema name for a baseline/candidate pair.
pub const PAIRED_RENDEZVOUS_MEASUREMENT_JSON_SCHEMA: &str = "mmp-sim.paired-rendezvous-measurement";
/// Current JSON schema version for both research-only records.
pub const RENDEZVOUS_MEASUREMENT_JSON_VERSION: u16 = 1;
/// Maximum accepted UTF-8 byte length for one measurement record.
pub const MAX_RENDEZVOUS_MEASUREMENT_JSON_BYTES: usize = 4 * 1024;
/// Maximum accepted UTF-8 byte length for a paired measurement record.
pub const MAX_PAIRED_RENDEZVOUS_MEASUREMENT_JSON_BYTES: usize = 8 * 1024;

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
    /// Full 16-byte identifier shared by the baseline and candidate runs.
    ///
    /// This is the replay trace identity, not a protocol credential. Keeping
    /// all 128 bits avoids collapsing distinct schedules to the same pairing
    /// key before comparison.
    pub trace_id: [u8; 16],
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
    /// Baseline and candidate observations do not describe one workload.
    ///
    /// A paired comparison may differ in observed outcomes, latency samples,
    /// and energy counters, but the number of contact opportunities and
    /// offered bundles must be identical. Otherwise a rate delta can be
    /// attributed to the candidate strategy even though its denominator or
    /// workload changed.
    MismatchedWorkload,
    /// A derived summary value cannot be represented as a finite number.
    SummaryOverflow,
}

/// Errors raised while decoding or encoding the strict research-record JSON
/// seam. The JSON representation is intentionally separate from the MMP/1
/// wire format and must not be used as a protocol serializer.
#[derive(Debug)]
pub enum MeasurementJsonError {
    /// The input is not valid JSON or does not match the required field types.
    Json(serde_json::Error),
    /// The input exceeds the bounded harness record size.
    InputTooLarge { limit: usize, actual: usize },
    /// The top-level schema marker is not the required record type.
    WrongSchema {
        expected: &'static str,
        actual: String,
    },
    /// The record version is not understood by this implementation.
    UnsupportedVersion(u16),
    /// The trace identifier is not exactly 16 bytes of lowercase hexadecimal.
    InvalidTraceId,
    /// The typed measurement failed its existing semantic validation.
    Validation(MeasurementError),
}

impl fmt::Display for MeasurementJsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => write!(f, "invalid rendezvous JSON: {error}"),
            Self::InputTooLarge { limit, actual } => {
                write!(
                    f,
                    "rendezvous JSON is too large: {actual} bytes (limit {limit})"
                )
            }
            Self::WrongSchema { expected, actual } => {
                write!(
                    f,
                    "wrong rendezvous JSON schema: expected {expected}, got {actual}"
                )
            }
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported rendezvous JSON schema version: {version}")
            }
            Self::InvalidTraceId => write!(
                f,
                "trace_id must be exactly 32 lowercase hexadecimal characters"
            ),
            Self::Validation(error) => write!(f, "invalid rendezvous measurement: {error:?}"),
        }
    }
}

impl std::error::Error for MeasurementJsonError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Json(error) => Some(error),
            _ => None,
        }
    }
}

/// Private serde representation for the stable, versioned measurement input
/// contract. `deny_unknown_fields` makes typoed or future fields fail closed;
/// every field is required so a partial log cannot be mistaken for a complete
/// observation.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct MeasurementJsonRecord {
    schema: String,
    schema_version: u16,
    trace_id: String,
    tx_events: u64,
    rx_windows: u64,
    contact_attempts: u64,
    discoveries: u64,
    offered_bundles: u64,
    delivered_bundles: u64,
    useful_payload_bytes: u64,
    airtime_ms: f64,
    discovery_latency_ms_sum: f64,
    discovery_latency_samples: u64,
    delivery_latency_ms_sum: f64,
    delivery_latency_samples: u64,
    radio_energy_mwh: f64,
    host_energy_mwh: f64,
    whole_device_energy_mwh: f64,
    host_wakeups: u64,
}

/// Private serde representation for a paired baseline/candidate record.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PairedMeasurementJsonRecord {
    schema: String,
    schema_version: u16,
    baseline: MeasurementJsonRecord,
    candidate: MeasurementJsonRecord,
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

    /// Serialize one validated measurement to deterministic, versioned JSON.
    ///
    /// This is a research-harness interchange format, not a protocol or
    /// cryptographic encoding. The serializer emits fields in the stable
    /// declaration order below and writes `trace_id` as lowercase hex.
    pub fn to_json(&self) -> Result<String, MeasurementJsonError> {
        self.validate().map_err(MeasurementJsonError::Validation)?;
        serde_json::to_string(&MeasurementJsonRecord::from_measurement(self))
            .map_err(MeasurementJsonError::Json)
    }

    /// Parse one complete measurement record from strict, versioned JSON.
    /// Unknown or missing fields are rejected by serde and semantic validation
    /// is applied before the record is returned to the caller.
    pub fn from_json(input: &str) -> Result<Self, MeasurementJsonError> {
        ensure_json_size(input, MAX_RENDEZVOUS_MEASUREMENT_JSON_BYTES)?;
        let record: MeasurementJsonRecord =
            serde_json::from_str(input).map_err(MeasurementJsonError::Json)?;
        record.into_measurement()
    }
}

impl MeasurementJsonRecord {
    fn from_measurement(measurement: &RendezvousMeasurement) -> Self {
        Self {
            schema: RENDEZVOUS_MEASUREMENT_JSON_SCHEMA.to_owned(),
            schema_version: RENDEZVOUS_MEASUREMENT_JSON_VERSION,
            trace_id: hex::encode(measurement.trace_id),
            tx_events: measurement.tx_events,
            rx_windows: measurement.rx_windows,
            contact_attempts: measurement.contact_attempts,
            discoveries: measurement.discoveries,
            offered_bundles: measurement.offered_bundles,
            delivered_bundles: measurement.delivered_bundles,
            useful_payload_bytes: measurement.useful_payload_bytes,
            airtime_ms: measurement.airtime_ms,
            discovery_latency_ms_sum: measurement.discovery_latency_ms_sum,
            discovery_latency_samples: measurement.discovery_latency_samples,
            delivery_latency_ms_sum: measurement.delivery_latency_ms_sum,
            delivery_latency_samples: measurement.delivery_latency_samples,
            radio_energy_mwh: measurement.radio_energy_mwh,
            host_energy_mwh: measurement.host_energy_mwh,
            whole_device_energy_mwh: measurement.whole_device_energy_mwh,
            host_wakeups: measurement.host_wakeups,
        }
    }

    fn into_measurement(self) -> Result<RendezvousMeasurement, MeasurementJsonError> {
        if self.schema != RENDEZVOUS_MEASUREMENT_JSON_SCHEMA {
            return Err(MeasurementJsonError::WrongSchema {
                expected: RENDEZVOUS_MEASUREMENT_JSON_SCHEMA,
                actual: self.schema,
            });
        }
        if self.schema_version != RENDEZVOUS_MEASUREMENT_JSON_VERSION {
            return Err(MeasurementJsonError::UnsupportedVersion(
                self.schema_version,
            ));
        }
        let trace_id = decode_trace_id(&self.trace_id)?;
        let measurement = RendezvousMeasurement {
            trace_id,
            tx_events: self.tx_events,
            rx_windows: self.rx_windows,
            contact_attempts: self.contact_attempts,
            discoveries: self.discoveries,
            offered_bundles: self.offered_bundles,
            delivered_bundles: self.delivered_bundles,
            useful_payload_bytes: self.useful_payload_bytes,
            airtime_ms: self.airtime_ms,
            discovery_latency_ms_sum: self.discovery_latency_ms_sum,
            discovery_latency_samples: self.discovery_latency_samples,
            delivery_latency_ms_sum: self.delivery_latency_ms_sum,
            delivery_latency_samples: self.delivery_latency_samples,
            radio_energy_mwh: self.radio_energy_mwh,
            host_energy_mwh: self.host_energy_mwh,
            whole_device_energy_mwh: self.whole_device_energy_mwh,
            host_wakeups: self.host_wakeups,
        };
        measurement
            .validate()
            .map_err(MeasurementJsonError::Validation)?;
        Ok(measurement)
    }
}

fn decode_trace_id(value: &str) -> Result<[u8; 16], MeasurementJsonError> {
    if value.len() != 32
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(MeasurementJsonError::InvalidTraceId);
    }
    let decoded = hex::decode(value).map_err(|_| MeasurementJsonError::InvalidTraceId)?;
    decoded
        .try_into()
        .map_err(|_| MeasurementJsonError::InvalidTraceId)
}

fn ensure_json_size(input: &str, limit: usize) -> Result<(), MeasurementJsonError> {
    if input.len() > limit {
        return Err(MeasurementJsonError::InputTooLarge {
            limit,
            actual: input.len(),
        });
    }
    Ok(())
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
        if baseline.contact_attempts != candidate.contact_attempts
            || baseline.offered_bundles != candidate.offered_bundles
        {
            return Err(MeasurementError::MismatchedWorkload);
        }
        Ok(Self {
            baseline,
            candidate,
        })
    }

    /// Produce a deterministic comparison summary from the validated pair.
    pub fn summary(&self) -> Result<RendezvousComparison, MeasurementError> {
        let baseline_discovery_rate =
            rate(self.baseline.discoveries, self.baseline.contact_attempts);
        let candidate_discovery_rate =
            rate(self.candidate.discoveries, self.candidate.contact_attempts);
        let baseline_delivery_rate = rate(
            self.baseline.delivered_bundles,
            self.baseline.offered_bundles,
        );
        let candidate_delivery_rate = rate(
            self.candidate.delivered_bundles,
            self.candidate.offered_bundles,
        );
        let baseline_discovery_rate_ci95 =
            rate_confidence95(self.baseline.discoveries, self.baseline.contact_attempts)?;
        let candidate_discovery_rate_ci95 =
            rate_confidence95(self.candidate.discoveries, self.candidate.contact_attempts)?;
        let baseline_delivery_rate_ci95 = rate_confidence95(
            self.baseline.delivered_bundles,
            self.baseline.offered_bundles,
        )?;
        let candidate_delivery_rate_ci95 = rate_confidence95(
            self.candidate.delivered_bundles,
            self.candidate.offered_bundles,
        )?;
        let whole_device_energy_ratio = ratio(
            self.candidate.whole_device_energy_mwh,
            self.baseline.whole_device_energy_mwh,
        )?;
        Ok(RendezvousComparison {
            trace_id: self.baseline.trace_id,
            baseline_discovery_rate,
            candidate_discovery_rate,
            discovery_rate_delta: candidate_discovery_rate - baseline_discovery_rate,
            baseline_discovery_rate_ci95,
            candidate_discovery_rate_ci95,
            baseline_delivery_rate,
            candidate_delivery_rate,
            delivery_rate_delta: candidate_delivery_rate - baseline_delivery_rate,
            baseline_delivery_rate_ci95,
            candidate_delivery_rate_ci95,
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

    /// Serialize a validated baseline/candidate pair to deterministic JSON.
    /// The two nested records retain their own schema markers so a standalone
    /// record and a pair cannot be confused by a harness importer.
    pub fn to_json(&self) -> Result<String, MeasurementJsonError> {
        let record = PairedMeasurementJsonRecord {
            schema: PAIRED_RENDEZVOUS_MEASUREMENT_JSON_SCHEMA.to_owned(),
            schema_version: RENDEZVOUS_MEASUREMENT_JSON_VERSION,
            baseline: MeasurementJsonRecord::from_measurement(&self.baseline),
            candidate: MeasurementJsonRecord::from_measurement(&self.candidate),
        };
        serde_json::to_string(&record).map_err(MeasurementJsonError::Json)
    }

    /// Parse and validate a complete baseline/candidate pair from strict JSON.
    pub fn from_json(input: &str) -> Result<Self, MeasurementJsonError> {
        ensure_json_size(input, MAX_PAIRED_RENDEZVOUS_MEASUREMENT_JSON_BYTES)?;
        let record: PairedMeasurementJsonRecord =
            serde_json::from_str(input).map_err(MeasurementJsonError::Json)?;
        record.into_pair()
    }
}

impl PairedMeasurementJsonRecord {
    fn into_pair(self) -> Result<PairedRendezvousMeasurement, MeasurementJsonError> {
        if self.schema != PAIRED_RENDEZVOUS_MEASUREMENT_JSON_SCHEMA {
            return Err(MeasurementJsonError::WrongSchema {
                expected: PAIRED_RENDEZVOUS_MEASUREMENT_JSON_SCHEMA,
                actual: self.schema,
            });
        }
        if self.schema_version != RENDEZVOUS_MEASUREMENT_JSON_VERSION {
            return Err(MeasurementJsonError::UnsupportedVersion(
                self.schema_version,
            ));
        }
        let baseline = self.baseline.into_measurement()?;
        let candidate = self.candidate.into_measurement()?;
        PairedRendezvousMeasurement::new(baseline, candidate)
            .map_err(MeasurementJsonError::Validation)
    }
}

/// Stable fields used to compare two validated rendezvous observations.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RendezvousComparison {
    pub trace_id: [u8; 16],
    pub baseline_discovery_rate: f64,
    pub candidate_discovery_rate: f64,
    pub discovery_rate_delta: f64,
    /// Wilson 95% interval for the baseline discovery proportion. `None`
    /// means the trace contained no contact attempts.
    pub baseline_discovery_rate_ci95: Option<RateConfidenceInterval>,
    /// Wilson 95% interval for the candidate discovery proportion. `None`
    /// means the trace contained no contact attempts.
    pub candidate_discovery_rate_ci95: Option<RateConfidenceInterval>,
    pub baseline_delivery_rate: f64,
    pub candidate_delivery_rate: f64,
    pub delivery_rate_delta: f64,
    /// Wilson 95% interval for the baseline delivery proportion. `None`
    /// means the trace offered no bundles.
    pub baseline_delivery_rate_ci95: Option<RateConfidenceInterval>,
    /// Wilson 95% interval for the candidate delivery proportion. `None`
    /// means the trace offered no bundles.
    pub candidate_delivery_rate_ci95: Option<RateConfidenceInterval>,
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

/// A Wilson-score 95% confidence interval for a binomial proportion.
///
/// This is a measurement annotation, not a protocol claim. It describes
/// sampling uncertainty in one replayed trace; it does not account for
/// dependence between contacts or for variation between independent traces.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RateConfidenceInterval {
    pub successes: u64,
    pub trials: u64,
    pub estimate: f64,
    pub lower: f64,
    pub upper: f64,
}

fn rate(successes: u64, total: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        successes as f64 / total as f64
    }
}

/// Return a Wilson-score 95% interval without relying on a normal
/// approximation at the boundaries. `None` is explicit for a zero-sample
/// trace, where a rate estimate has no evidentiary basis.
fn rate_confidence95(
    successes: u64,
    total: u64,
) -> Result<Option<RateConfidenceInterval>, MeasurementError> {
    if total == 0 {
        return Ok(None);
    }
    let n = total as f64;
    let p = successes as f64 / n;
    let z = 1.959_963_984_540_054_f64;
    let z_squared = z * z;
    let denominator = 1.0 + z_squared / n;
    let center = (p + z_squared / (2.0 * n)) / denominator;
    let margin = z * (p.mul_add(1.0 - p, z_squared / (4.0 * n)) / n).sqrt() / denominator;
    let lower = (center - margin).max(0.0);
    let upper = (center + margin).min(1.0);
    if [p, center, margin, lower, upper]
        .iter()
        .all(|value| value.is_finite())
    {
        Ok(Some(RateConfidenceInterval {
            successes,
            trials: total,
            estimate: p,
            lower,
            upper,
        }))
    } else {
        Err(MeasurementError::SummaryOverflow)
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

/// Maximum contact opportunities accepted by the bounded EDR replay seam.
///
/// This is a harness bound, not a protocol limit. It prevents an accidental
/// fixture from being presented as a small, reproducible experiment while
/// retaining enough headroom for a host-side trace.
pub const MAX_EDR_REPLAY_CONTACTS: u64 = 100_000;
/// Maximum offered bundles accepted by the bounded EDR replay seam.
pub const MAX_EDR_REPLAY_BUNDLES: u64 = 100_000;

/// Workload identity and aggregate counts shared by a paired EDR replay.
///
/// The seed and trace identifier are replay metadata only; they are not
/// protocol credentials. The workload is deliberately separate from
/// [`RendezvousMeasurement`] because this model predicts activity energy and
/// must not fabricate radio, host, or delivery observations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EdrReplayWorkload {
    /// Full trace identity retained by both baseline and candidate results.
    pub trace_id: [u8; 16],
    /// Seed used to generate the contact trace.
    pub seed: u64,
    /// Number of contact opportunities in the replayed trace.
    pub contact_attempts: u64,
    /// Number of bundles offered to the workload.
    pub offered_bundles: u64,
}

impl EdrReplayWorkload {
    /// Validate the bounded host-side replay input.
    pub fn validate(self) -> bool {
        self.contact_attempts <= MAX_EDR_REPLAY_CONTACTS
            && self.offered_bundles <= MAX_EDR_REPLAY_BUNDLES
    }
}

/// One deterministic, paired baseline/candidate EDR model result.
///
/// Both sides retain the same [`EdrReplayWorkload`]. This is an analytic
/// activity comparison: it does not run the reference router and does not
/// claim that discovery or delivery outcomes remain unchanged on real BLE.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PairedEdrEnergyRun {
    /// Shared seed, trace, and workload counts.
    pub workload: EdrReplayWorkload,
    /// MMP/1 activity point used as the baseline.
    pub baseline: RendezvousExperimentPoint,
    /// EDR activity point evaluated against the same workload.
    pub candidate: RendezvousExperimentPoint,
}

impl PairedEdrEnergyRun {
    /// Candidate energy divided by baseline energy. `None` is impossible for
    /// a valid model today, but keeps callers from treating a zero baseline as
    /// a useful multiplier if the representation changes later.
    pub fn energy_ratio(self) -> Option<f64> {
        if self.baseline.activity.energy == 0.0 {
            None
        } else {
            let ratio = self.candidate.activity.energy / self.baseline.activity.energy;
            ratio.is_finite().then_some(ratio)
        }
    }

    /// Candidate minus baseline modeled energy in the model's activity unit.
    pub fn energy_delta(self) -> f64 {
        self.candidate.activity.energy - self.baseline.activity.energy
    }
}

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
        if [
            tx_activity,
            rx_activity,
            tx_cost_per_activity,
            rx_cost_per_activity,
        ]
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

    /// Pair every bounded EDR matrix point with the MMP/1 baseline using one
    /// identical seed, trace, and workload.
    ///
    /// The returned array is a deterministic research report input. It only
    /// compares the fixed-hazard activity model; it does not alter routing,
    /// copy budgets, wire fields, or simulator behavior. A caller must pair
    /// these modeled energies with independently observed BLE metrics before
    /// making any performance claim.
    pub fn paired_experiment_matrix(
        &self,
        workload: EdrReplayWorkload,
    ) -> Option<[PairedEdrEnergyRun; 7]> {
        if !workload.validate() {
            return None;
        }
        let baseline = self.experiment_point(1.0)?;
        let matrix = self.default_experiment_matrix()?;
        Some([
            PairedEdrEnergyRun {
                workload,
                baseline,
                candidate: matrix[0],
            },
            PairedEdrEnergyRun {
                workload,
                baseline,
                candidate: matrix[1],
            },
            PairedEdrEnergyRun {
                workload,
                baseline,
                candidate: matrix[2],
            },
            PairedEdrEnergyRun {
                workload,
                baseline,
                candidate: matrix[3],
            },
            PairedEdrEnergyRun {
                workload,
                baseline,
                candidate: matrix[4],
            },
            PairedEdrEnergyRun {
                workload,
                baseline,
                candidate: matrix[5],
            },
            PairedEdrEnergyRun {
                workload,
                baseline,
                candidate: matrix[6],
            },
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
        let tx_activity =
            (self.hazard_product * self.rx_cost_per_activity / self.tx_cost_per_activity).sqrt();
        let rx_activity =
            (self.hazard_product * self.tx_cost_per_activity / self.rx_cost_per_activity).sqrt();
        // The constructor guarantees positive finite inputs, so this cannot
        // fail unless a future caller changes the representation invariants.
        self.evaluate(tx_activity, rx_activity)
            .expect("valid square-root optimum")
    }

    /// Baseline energy divided by the idealized optimum energy.
    ///
    /// This compatibility method retains the original scalar API. Callers
    /// that ingest untrusted or very wide floating-point ranges should prefer
    /// [`Self::checked_idealized_gain`], because a finite baseline and finite
    /// optimum can still have a quotient that overflows to `+infinity`.
    pub fn idealized_gain(&self) -> f64 {
        self.baseline_energy() / self.optimum().energy
    }

    /// Return the idealized gain only when the quotient is representable.
    ///
    /// The gain is a property of the research model, not a measured battery
    /// result. `None` means that the finite model inputs produce an
    /// unrepresentable ratio; callers must not turn that case into a claimed
    /// performance multiplier.
    pub fn checked_idealized_gain(&self) -> Option<f64> {
        let result = self.idealized_gain();
        result.is_finite().then_some(result)
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
        assert_eq!(model.checked_idealized_gain(), Some(gain));
    }

    #[test]
    fn checked_idealized_gain_rejects_overflowing_ratio() {
        // Every constructor term remains finite and positive, but the
        // baseline/optimum quotient is larger than the f64 range. The
        // compatibility method necessarily returns +infinity; the checked
        // path prevents that value from being treated as an experiment
        // result.
        let model = RendezvousEnergyModel::from_baseline(1e308, 1e-308, 1.0, 1e-12).unwrap();
        assert!(model.baseline_energy().is_finite());
        assert!(model.optimum().energy.is_finite());
        assert!(model.idealized_gain().is_infinite());
        assert_eq!(model.checked_idealized_gain(), None);
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

    fn measurement(
        trace_id: [u8; 16],
        discoveries: u64,
        delivered: u64,
        energy: f64,
    ) -> RendezvousMeasurement {
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
        let baseline = measurement([7; 16], 2, 2, 100.0);
        let candidate = measurement([7; 16], 3, 3, 75.0);
        let summary = PairedRendezvousMeasurement::new(baseline, candidate)
            .unwrap()
            .summary()
            .unwrap();

        assert_eq!(summary.trace_id, [7; 16]);
        assert_eq!(summary.baseline_discovery_rate, 0.5);
        assert_eq!(summary.candidate_discovery_rate, 0.75);
        assert_eq!(summary.discovery_rate_delta, 0.25);
        assert_eq!(summary.baseline_delivery_rate, 0.5);
        assert_eq!(summary.candidate_delivery_rate, 0.75);
        assert_eq!(summary.delivery_rate_delta, 0.25);
        let baseline_discovery_ci = summary.baseline_discovery_rate_ci95.unwrap();
        assert_eq!(
            baseline_discovery_ci.estimate,
            summary.baseline_discovery_rate
        );
        assert!(baseline_discovery_ci.lower < baseline_discovery_ci.estimate);
        assert!(baseline_discovery_ci.estimate < baseline_discovery_ci.upper);
        let candidate_delivery_ci = summary.candidate_delivery_rate_ci95.unwrap();
        assert_eq!(
            candidate_delivery_ci.estimate,
            summary.candidate_delivery_rate
        );
        assert!(candidate_delivery_ci.lower < candidate_delivery_ci.estimate);
        assert!(candidate_delivery_ci.estimate < candidate_delivery_ci.upper);
        assert_eq!(summary.whole_device_energy_ratio, Some(0.75));
        assert_eq!(summary.baseline_discovery_latency_ms, Some(4.0));
        assert_eq!(summary.candidate_delivery_latency_ms, Some(4.0));
    }

    #[test]
    fn paired_edr_measurement_rejects_mismatched_trace_and_bad_counts() {
        let baseline = measurement([7; 16], 2, 2, 100.0);
        let mut mismatched_trace = [7; 16];
        mismatched_trace[15] = 8;
        let mismatched = measurement(mismatched_trace, 3, 3, 75.0);
        assert_eq!(
            PairedRendezvousMeasurement::new(baseline, mismatched),
            Err(MeasurementError::MismatchedTrace)
        );

        let mut invalid = measurement([7; 16], 2, 2, 100.0);
        invalid.discoveries = invalid.contact_attempts + 1;
        assert_eq!(
            PairedRendezvousMeasurement::new(invalid, measurement([7; 16], 3, 3, 75.0)),
            Err(MeasurementError::InconsistentCounts)
        );
    }

    #[test]
    fn paired_edr_measurement_rejects_mismatched_workload_denominators() {
        let baseline = measurement([7; 16], 2, 2, 100.0);
        let mut candidate = measurement([7; 16], 3, 3, 75.0);
        candidate.offered_bundles += 1;
        assert_eq!(
            PairedRendezvousMeasurement::new(baseline, candidate),
            Err(MeasurementError::MismatchedWorkload)
        );

        let mut candidate = measurement([7; 16], 3, 3, 75.0);
        candidate.contact_attempts += 1;
        assert_eq!(
            PairedRendezvousMeasurement::new(baseline, candidate),
            Err(MeasurementError::MismatchedWorkload)
        );
    }

    #[test]
    fn paired_edr_measurement_rejects_nonfinite_energy() {
        let mut invalid = measurement([7; 16], 2, 2, 100.0);
        invalid.whole_device_energy_mwh = f64::NAN;
        assert_eq!(
            PairedRendezvousMeasurement::new(invalid, measurement([7; 16], 3, 3, 75.0)),
            Err(MeasurementError::NonFiniteValue)
        );
    }

    #[test]
    fn paired_edr_measurement_marks_zero_sample_intervals_unknown() {
        let mut baseline = measurement([7; 16], 0, 0, 100.0);
        let mut candidate = measurement([7; 16], 0, 0, 75.0);
        baseline.contact_attempts = 0;
        candidate.contact_attempts = 0;
        baseline.offered_bundles = 0;
        candidate.offered_bundles = 0;
        let summary = PairedRendezvousMeasurement::new(baseline, candidate)
            .unwrap()
            .summary()
            .unwrap();
        assert_eq!(summary.baseline_discovery_rate_ci95, None);
        assert_eq!(summary.candidate_discovery_rate_ci95, None);
        assert_eq!(summary.baseline_delivery_rate_ci95, None);
        assert_eq!(summary.candidate_delivery_rate_ci95, None);
    }

    #[test]
    fn measurement_json_round_trip_is_deterministic_and_lossless() {
        let value = measurement([0xabu8; 16], 2, 2, 100.0);
        let json = value.to_json().unwrap();
        assert_eq!(json, value.to_json().unwrap());
        assert!(json.contains("\"schema\":\"mmp-sim.rendezvous-measurement\""));
        assert!(json.contains("\"trace_id\":\"abababababababababababababababab\""));
        assert_eq!(RendezvousMeasurement::from_json(&json).unwrap(), value);
    }

    #[test]
    fn paired_measurement_json_round_trip_preserves_comparison() {
        let baseline = measurement([7; 16], 2, 2, 100.0);
        let candidate = measurement([7; 16], 3, 3, 75.0);
        let pair = PairedRendezvousMeasurement::new(baseline, candidate).unwrap();
        let json = pair.to_json().unwrap();
        let decoded = PairedRendezvousMeasurement::from_json(&json).unwrap();
        assert_eq!(decoded.summary().unwrap(), pair.summary().unwrap());
    }

    #[test]
    fn measurement_json_rejects_unknown_missing_bad_schema_and_invalid_values() {
        let value = measurement([7; 16], 2, 2, 100.0);
        let json = value.to_json().unwrap();
        let unknown = json.trim_end_matches('}').to_owned() + ",\"typo\":1}";
        assert!(matches!(
            RendezvousMeasurement::from_json(&unknown),
            Err(MeasurementJsonError::Json(_))
        ));
        let missing = json.replacen(",\"host_wakeups\":5", "", 1);
        assert!(matches!(
            RendezvousMeasurement::from_json(&missing),
            Err(MeasurementJsonError::Json(_))
        ));
        let wrong_schema = json.replace(
            "mmp-sim.rendezvous-measurement",
            "mmp-sim.other-measurement",
        );
        assert!(matches!(
            RendezvousMeasurement::from_json(&wrong_schema),
            Err(MeasurementJsonError::WrongSchema { .. })
        ));
        let invalid_count = json.replace("\"discoveries\":2", "\"discoveries\":5");
        assert!(matches!(
            RendezvousMeasurement::from_json(&invalid_count),
            Err(MeasurementJsonError::Validation(
                MeasurementError::InconsistentCounts
            ))
        ));
        let invalid_trace = json.replace(
            "07070707070707070707070707070707",
            "0707070707070707070707070707070G",
        );
        assert!(matches!(
            RendezvousMeasurement::from_json(&invalid_trace),
            Err(MeasurementJsonError::InvalidTraceId)
        ));
    }

    #[test]
    fn measurement_json_rejects_nonfinite_and_oversized_input() {
        let value = measurement([7; 16], 2, 2, 100.0);
        let json = value.to_json().unwrap();
        let nonfinite = json.replace("\"airtime_ms\":80.0", "\"airtime_ms\":1e999");
        assert!(matches!(
            RendezvousMeasurement::from_json(&nonfinite),
            Err(MeasurementJsonError::Json(_))
        ));
        let oversized = format!(
            "{}{}",
            json,
            " ".repeat(MAX_RENDEZVOUS_MEASUREMENT_JSON_BYTES)
        );
        assert!(matches!(
            RendezvousMeasurement::from_json(&oversized),
            Err(MeasurementJsonError::InputTooLarge { .. })
        ));
    }

    #[test]
    fn edr_experiment_matrix_is_deterministic_and_marks_binary_spray_baseline() {
        let model = RendezvousEnergyModel::from_baseline(1.0, 1.0, 0.12, 4.5).unwrap();
        let first = model.default_experiment_matrix().unwrap();
        let second = model.default_experiment_matrix().unwrap();

        assert_eq!(first, second);
        assert_eq!(first.len(), 7);
        assert_eq!(first[3].tx_scale, 1.0);
        assert_eq!(
            first[3].profile,
            RendezvousExperimentProfile::BinarySprayBaseline
        );
        for point in first {
            assert!(
                (point.activity.tx_activity * point.activity.rx_activity - model.hazard_product())
                    .abs()
                    < 1e-9
            );
        }
        assert!(first
            .iter()
            .any(|point| point.profile == RendezvousExperimentProfile::EdrDutyCycle));
    }

    #[test]
    fn edr_matrix_energy_is_never_below_the_analytic_fixed_hazard_optimum() {
        let model = RendezvousEnergyModel::from_baseline(1.0, 1.0, 0.12, 4.5).unwrap();
        let matrix = model.default_experiment_matrix().unwrap();
        let optimum = model.optimum();
        let tolerance = optimum.energy.max(1.0) * 1e-12;

        for point in matrix {
            assert!(
                point.activity.energy + tolerance >= optimum.energy,
                "{}x transmit point undercut the analytic optimum: {} < {}",
                point.tx_scale,
                point.activity.energy,
                optimum.energy
            );
        }

        let baseline = matrix
            .iter()
            .find(|point| point.profile == RendezvousExperimentProfile::BinarySprayBaseline)
            .expect("matrix must contain one baseline point");
        assert!((baseline.activity.energy - model.baseline_energy()).abs() <= tolerance);
    }

    #[test]
    fn paired_edr_matrix_reuses_trace_seed_and_workload_without_claiming_delivery() {
        let model = RendezvousEnergyModel::from_baseline(1.0, 1.0, 0.12, 4.5).unwrap();
        let workload = EdrReplayWorkload {
            trace_id: [0x5a; 16],
            seed: 0xED00_2026,
            contact_attempts: 128,
            offered_bundles: 32,
        };
        let first = model.paired_experiment_matrix(workload).unwrap();
        let second = model.paired_experiment_matrix(workload).unwrap();

        assert_eq!(first, second, "paired replay must be deterministic");
        for run in first {
            assert_eq!(run.workload, workload);
            assert_eq!(
                run.baseline.profile,
                RendezvousExperimentProfile::BinarySprayBaseline
            );
            assert!(run.energy_ratio().is_some());
            // The model result is activity-only; no delivery observation is
            // fabricated by this research seam.
            assert!(run.workload.offered_bundles <= MAX_EDR_REPLAY_BUNDLES);
        }
        assert!(first[4].energy_delta() < 0.0);
        assert_eq!(
            first[3].candidate.profile,
            RendezvousExperimentProfile::BinarySprayBaseline
        );
    }

    #[test]
    fn paired_edr_matrix_rejects_unbounded_workload_before_model_evaluation() {
        let model = RendezvousEnergyModel::from_baseline(1.0, 1.0, 0.12, 4.5).unwrap();
        let workload = EdrReplayWorkload {
            trace_id: [0; 16],
            seed: 1,
            contact_attempts: MAX_EDR_REPLAY_CONTACTS + 1,
            offered_bundles: 0,
        };
        assert!(model.paired_experiment_matrix(workload).is_none());
    }
}
