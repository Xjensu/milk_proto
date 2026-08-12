//! Research-only contact-survival and transfer-ranking helpers for MMP/Phi.
//!
//! This module evaluates a caller-supplied mathematical hypothesis. It does
//! not read RSSI, perform ranging, alter [`crate::Sim`], or change any MMP/1
//! wire, router, or node behavior. Coefficients must be fitted and validated
//! against physical traces before these helpers can support a production
//! decision.

use std::cmp::Ordering;
use std::collections::HashSet;

/// Maximum prediction horizon accepted by the research model.
pub const MAX_HORIZON_S: f64 = 86_400.0;
/// Maximum distance-like observation accepted by the bounded model.
pub const MAX_DISTANCE_M: f64 = 1_000_000.0;
/// Maximum absolute coefficient accepted by the bounded model.
pub const MAX_ABS_COEFFICIENT: f64 = 1_000_000.0;
/// Maximum number of candidates in one local ranking pass.
pub const MAX_CANDIDATES: usize = 256;
/// Maximum modeled payload size, matching the current file-transfer bound.
pub const MAX_TRANSFER_BYTES: u64 = 8 * 1024 * 1024;
/// Maximum modeled goodput in bytes per second.
pub const MAX_GOODPUT_BPS: f64 = 1_000_000_000_000.0;
/// Maximum utility value accepted by one local ranking pass.
pub const MAX_UTILITY: f64 = 1_000_000_000_000.0;

/// One bounded observation supplied by a research caller.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContactObservation {
    /// Estimated peer distance in metres.
    pub distance_m: f64,
    /// Positive values conventionally mean the peer is receding.
    pub radial_velocity_mps: f64,
    /// Standard deviation of the distance estimate in metres.
    pub distance_stddev_m: f64,
    /// Packet-error estimate in the closed interval `[0, 1]`.
    pub packet_error_rate: f64,
}

/// Coefficients for the constant-state hazard hypothesis.
///
/// The predictor is `intercept + distance * distance_m + radial_velocity *
/// radial_velocity_mps + distance_stddev * distance_stddev_m + packet_error *
/// packet_error_rate`.
/// The resulting hazard is a stable softplus of that predictor, in inverse
/// seconds. These values are deliberately caller supplied: no physical fit is
/// implied by this crate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HazardCoefficients {
    pub intercept: f64,
    pub distance: f64,
    pub radial_velocity: f64,
    pub distance_stddev: f64,
    pub packet_error: f64,
}

/// A local transfer candidate evaluated against one contact survival model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransferCandidate {
    /// Caller-owned deterministic tie-break identifier.
    pub carrier_id: u64,
    /// Application value in an arbitrary, documented common unit.
    pub value: f64,
    /// Payload size in bytes.
    pub bytes: u64,
    /// Estimated useful goodput in bytes per second.
    pub goodput_bps: f64,
    /// Fixed modeled setup cost.
    pub setup_cost: f64,
    /// Modeled incremental cost per payload byte.
    pub per_byte_cost: f64,
}

/// One candidate after deterministic local ranking.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransferDecision {
    pub carrier_id: u64,
    pub required_seconds: f64,
    pub finish_probability: f64,
    pub modeled_cost: f64,
    pub score: f64,
}

/// Validation or derived-value failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurvivalError {
    /// An observation contained a non-finite or out-of-range value.
    InvalidObservation,
    /// A coefficient was non-finite or exceeded the model bound.
    InvalidCoefficients,
    /// The horizon was non-finite or outside `[0, MAX_HORIZON_S]`.
    InvalidHorizon,
    /// More than [`MAX_CANDIDATES`] candidates were supplied.
    TooManyCandidates,
    /// A candidate contained an invalid size, rate, value, or cost.
    InvalidCandidate,
    /// Two candidates used the same caller-owned identifier.
    DuplicateCarrier,
    /// A derived value overflowed or became non-finite.
    NonFiniteDerived,
}

fn finite_bounded(value: f64, absolute_max: f64) -> bool {
    value.is_finite() && value.abs() <= absolute_max
}

fn validate_observation(observation: ContactObservation) -> Result<(), SurvivalError> {
    if !observation.distance_m.is_finite()
        || !(0.0..=MAX_DISTANCE_M).contains(&observation.distance_m)
        || !finite_bounded(observation.radial_velocity_mps, MAX_DISTANCE_M)
        || !observation.distance_stddev_m.is_finite()
        || !(0.0..=MAX_DISTANCE_M).contains(&observation.distance_stddev_m)
        || !observation.packet_error_rate.is_finite()
        || !(0.0..=1.0).contains(&observation.packet_error_rate)
    {
        return Err(SurvivalError::InvalidObservation);
    }
    Ok(())
}

fn validate_coefficients(coefficients: HazardCoefficients) -> Result<(), SurvivalError> {
    if [
        coefficients.intercept,
        coefficients.distance,
        coefficients.radial_velocity,
        coefficients.distance_stddev,
        coefficients.packet_error,
    ]
    .into_iter()
    .all(|value| finite_bounded(value, MAX_ABS_COEFFICIENT))
    {
        Ok(())
    } else {
        Err(SurvivalError::InvalidCoefficients)
    }
}

fn validate_horizon(horizon_s: f64) -> Result<(), SurvivalError> {
    if horizon_s.is_finite() && (0.0..=MAX_HORIZON_S).contains(&horizon_s) {
        Ok(())
    } else {
        Err(SurvivalError::InvalidHorizon)
    }
}

/// Numerically stable softplus used as a positive hazard transform.
fn stable_softplus(value: f64) -> f64 {
    if value > 40.0 {
        value
    } else if value < -40.0 {
        value.exp()
    } else {
        (1.0 + value.exp()).ln()
    }
}

/// Compute the constant-state hazard in inverse seconds.
pub fn stable_hazard_score(
    observation: ContactObservation,
    coefficients: HazardCoefficients,
) -> Result<f64, SurvivalError> {
    validate_observation(observation)?;
    validate_coefficients(coefficients)?;
    let predictor = coefficients.intercept
        + coefficients.distance * observation.distance_m
        + coefficients.radial_velocity * observation.radial_velocity_mps
        + coefficients.distance_stddev * observation.distance_stddev_m
        + coefficients.packet_error * observation.packet_error_rate;
    if !predictor.is_finite() {
        return Err(SurvivalError::NonFiniteDerived);
    }
    let hazard = stable_softplus(predictor);
    if hazard.is_finite() {
        Ok(hazard)
    } else {
        Err(SurvivalError::NonFiniteDerived)
    }
}

/// Estimate the probability that a contact survives the supplied horizon.
///
/// This is the constant-state survival function `S(t) = exp(-h * t)`, where
/// `h` is the positive softplus hazard returned by [`stable_hazard_score`].
pub fn survival_probability(
    observation: ContactObservation,
    coefficients: HazardCoefficients,
    horizon_s: f64,
) -> Result<f64, SurvivalError> {
    validate_horizon(horizon_s)?;
    let hazard = stable_hazard_score(observation, coefficients)?;
    let exponent = -(hazard * horizon_s);
    if !exponent.is_finite() {
        return Err(SurvivalError::NonFiniteDerived);
    }
    let survival = exponent.exp();
    if survival.is_finite() && (0.0..=1.0).contains(&survival) {
        Ok(survival)
    } else {
        Err(SurvivalError::NonFiniteDerived)
    }
}

/// Rank candidates by expected useful value per modeled cost.
///
/// A candidate requiring `bytes / goodput_bps` seconds receives the survival
/// probability for exactly that duration. The result is local planning data;
/// it does not allocate copies, mutate budgets, or alter a session.
pub fn rank_transfer_candidates(
    observation: ContactObservation,
    coefficients: HazardCoefficients,
    candidates: &[TransferCandidate],
) -> Result<Vec<TransferDecision>, SurvivalError> {
    if candidates.len() > MAX_CANDIDATES {
        return Err(SurvivalError::TooManyCandidates);
    }
    // Validate the model once before processing candidates.
    let _ = stable_hazard_score(observation, coefficients)?;
    let mut seen = HashSet::with_capacity(candidates.len());
    let mut decisions = Vec::with_capacity(candidates.len());

    for candidate in candidates {
        if !seen.insert(candidate.carrier_id) {
            return Err(SurvivalError::DuplicateCarrier);
        }
        if !candidate.value.is_finite()
            || !(0.0..=MAX_UTILITY).contains(&candidate.value)
            || candidate.value == 0.0
            || candidate.bytes > MAX_TRANSFER_BYTES
            || !candidate.goodput_bps.is_finite()
            || !(0.0..=MAX_GOODPUT_BPS).contains(&candidate.goodput_bps)
            || candidate.goodput_bps == 0.0
            || !candidate.setup_cost.is_finite()
            || candidate.setup_cost < 0.0
            || !candidate.per_byte_cost.is_finite()
            || candidate.per_byte_cost < 0.0
        {
            return Err(SurvivalError::InvalidCandidate);
        }

        let required_seconds = candidate.bytes as f64 / candidate.goodput_bps;
        let finish_probability = survival_probability(observation, coefficients, required_seconds)?;
        let modeled_cost = candidate.setup_cost + candidate.per_byte_cost * candidate.bytes as f64;
        if !required_seconds.is_finite() || !modeled_cost.is_finite() || modeled_cost <= 0.0 {
            return Err(SurvivalError::NonFiniteDerived);
        }
        let score = candidate.value * finish_probability / modeled_cost;
        if !score.is_finite() {
            return Err(SurvivalError::NonFiniteDerived);
        }
        decisions.push(TransferDecision {
            carrier_id: candidate.carrier_id,
            required_seconds,
            finish_probability,
            modeled_cost,
            score,
        });
    }

    decisions.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| {
                right
                    .finish_probability
                    .partial_cmp(&left.finish_probability)
                    .unwrap_or(Ordering::Equal)
            })
            .then_with(|| {
                left.modeled_cost
                    .partial_cmp(&right.modeled_cost)
                    .unwrap_or(Ordering::Equal)
            })
            .then_with(|| left.carrier_id.cmp(&right.carrier_id))
    });
    Ok(decisions)
}

#[cfg(test)]
mod tests {
    use super::*;

    const OBSERVATION: ContactObservation = ContactObservation {
        distance_m: 3.0,
        radial_velocity_mps: 0.2,
        distance_stddev_m: 0.1,
        packet_error_rate: 0.05,
    };
    const COEFFICIENTS: HazardCoefficients = HazardCoefficients {
        intercept: -2.0,
        distance: 0.1,
        radial_velocity: 0.2,
        distance_stddev: 0.3,
        packet_error: 0.4,
    };

    #[test]
    fn hazard_and_survival_are_deterministic_and_bounded() {
        let first = stable_hazard_score(OBSERVATION, COEFFICIENTS).unwrap();
        let second = stable_hazard_score(OBSERVATION, COEFFICIENTS).unwrap();
        assert_eq!(first, second);
        let survival = survival_probability(OBSERVATION, COEFFICIENTS, 10.0).unwrap();
        assert!((0.0..=1.0).contains(&survival));
        assert_eq!(
            survival_probability(OBSERVATION, COEFFICIENTS, 0.0),
            Ok(1.0)
        );
    }

    #[test]
    fn survival_and_finish_probability_are_non_increasing_with_duration() {
        let short = survival_probability(OBSERVATION, COEFFICIENTS, 1.0).unwrap();
        let long = survival_probability(OBSERVATION, COEFFICIENTS, 2.0).unwrap();
        assert!(long <= short);

        let candidates = [
            TransferCandidate {
                carrier_id: 1,
                value: 1.0,
                bytes: 100,
                goodput_bps: 100.0,
                setup_cost: 1.0,
                per_byte_cost: 0.01,
            },
            TransferCandidate {
                carrier_id: 2,
                value: 1.0,
                bytes: 200,
                goodput_bps: 100.0,
                setup_cost: 1.0,
                per_byte_cost: 0.01,
            },
        ];
        let ranked = rank_transfer_candidates(OBSERVATION, COEFFICIENTS, &candidates).unwrap();
        let first = ranked
            .iter()
            .find(|candidate| candidate.carrier_id == 1)
            .unwrap();
        let second = ranked
            .iter()
            .find(|candidate| candidate.carrier_id == 2)
            .unwrap();
        assert!(second.finish_probability <= first.finish_probability);
    }

    #[test]
    fn ranking_is_deterministic_and_uses_id_as_final_tie_break() {
        let candidates = [
            TransferCandidate {
                carrier_id: 9,
                value: 1.0,
                bytes: 0,
                goodput_bps: 1.0,
                setup_cost: 1.0,
                per_byte_cost: 0.0,
            },
            TransferCandidate {
                carrier_id: 3,
                value: 1.0,
                bytes: 0,
                goodput_bps: 1.0,
                setup_cost: 1.0,
                per_byte_cost: 0.0,
            },
        ];
        let ranked = rank_transfer_candidates(OBSERVATION, COEFFICIENTS, &candidates).unwrap();
        assert_eq!(
            ranked
                .iter()
                .map(|item| item.carrier_id)
                .collect::<Vec<_>>(),
            vec![3, 9]
        );
    }

    #[test]
    fn malformed_inputs_and_bounds_fail_closed() {
        assert_eq!(
            stable_hazard_score(
                ContactObservation {
                    distance_m: f64::NAN,
                    ..OBSERVATION
                },
                COEFFICIENTS,
            ),
            Err(SurvivalError::InvalidObservation)
        );
        assert_eq!(
            survival_probability(OBSERVATION, COEFFICIENTS, MAX_HORIZON_S + 1.0),
            Err(SurvivalError::InvalidHorizon)
        );
        assert_eq!(
            rank_transfer_candidates(
                OBSERVATION,
                COEFFICIENTS,
                &[TransferCandidate {
                    carrier_id: 1,
                    value: 1.0,
                    bytes: 1,
                    goodput_bps: 0.0,
                    setup_cost: 1.0,
                    per_byte_cost: 0.0,
                }],
            ),
            Err(SurvivalError::InvalidCandidate)
        );
        let duplicate = [
            TransferCandidate {
                carrier_id: 1,
                value: 1.0,
                bytes: 1,
                goodput_bps: 1.0,
                setup_cost: 1.0,
                per_byte_cost: 0.0,
            },
            TransferCandidate {
                carrier_id: 1,
                value: 1.0,
                bytes: 1,
                goodput_bps: 1.0,
                setup_cost: 1.0,
                per_byte_cost: 0.0,
            },
        ];
        assert_eq!(
            rank_transfer_candidates(OBSERVATION, COEFFICIENTS, &duplicate),
            Err(SurvivalError::DuplicateCarrier)
        );
        let many = (0..=MAX_CANDIDATES)
            .map(|carrier_id| TransferCandidate {
                carrier_id: carrier_id as u64,
                value: 1.0,
                bytes: 1,
                goodput_bps: 1.0,
                setup_cost: 1.0,
                per_byte_cost: 0.0,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            rank_transfer_candidates(OBSERVATION, COEFFICIENTS, &many),
            Err(SurvivalError::TooManyCandidates)
        );
    }

    #[test]
    fn accepted_numeric_boundaries_remain_finite() {
        let observation = ContactObservation {
            distance_m: MAX_DISTANCE_M,
            radial_velocity_mps: MAX_DISTANCE_M,
            distance_stddev_m: MAX_DISTANCE_M,
            packet_error_rate: 1.0,
        };
        let coefficients = HazardCoefficients {
            intercept: MAX_ABS_COEFFICIENT,
            distance: MAX_ABS_COEFFICIENT,
            radial_velocity: MAX_ABS_COEFFICIENT,
            distance_stddev: MAX_ABS_COEFFICIENT,
            packet_error: MAX_ABS_COEFFICIENT,
        };

        let hazard = stable_hazard_score(observation, coefficients).unwrap();
        assert!(hazard.is_finite());

        let survival = survival_probability(observation, coefficients, MAX_HORIZON_S).unwrap();
        assert!(survival.is_finite());
        assert!((0.0..=1.0).contains(&survival));

        let decision = rank_transfer_candidates(
            observation,
            coefficients,
            &[TransferCandidate {
                carrier_id: 1,
                value: MAX_UTILITY,
                bytes: MAX_TRANSFER_BYTES,
                goodput_bps: MAX_GOODPUT_BPS,
                setup_cost: 1.0,
                per_byte_cost: 0.0,
            }],
        )
        .unwrap();
        assert!(decision[0].required_seconds.is_finite());
        assert!(decision[0].modeled_cost.is_finite());
        assert!(decision[0].score.is_finite());
    }
}
