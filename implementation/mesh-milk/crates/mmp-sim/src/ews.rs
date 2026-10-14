//! Simulator-only Evidence-Weighted Spray (EWS) evaluator.
//!
//! EWS ranks caller-supplied carrier evidence by marginal delivery gain per
//! caller-supplied cost. It is a deterministic local planning heuristic: it
//! does not change `mmp-router`, wire frames, copy budgets, or admission
//! semantics, and it cannot enforce a network-wide replication limit against
//! Byzantine relays.

use std::collections::HashSet;

/// Maximum number of candidates accepted by one bounded EWS evaluation.
pub const MAX_EWS_CANDIDATES: usize = 256;

/// One caller-supplied carrier estimate for a research comparison.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EwsCandidate {
    /// Stable caller label used only for deterministic tie-breaking.
    pub carrier_id: u64,
    /// Estimated probability that this carrier reaches the destination before
    /// expiry, in the closed interval `[0, 1]`.
    pub delivery_probability: f64,
    /// Caller-defined cost in a common positive unit (airtime, energy, or a
    /// documented composite); it is not inferred by this crate.
    pub cost: f64,
}

/// Local bounds for one EWS planning pass.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EwsLimits {
    /// Maximum total cost accepted by this local plan.
    pub cost_budget: f64,
    /// Maximum number of candidates selected by this local plan.
    pub max_copies: usize,
    /// Stop when the next marginal gain/cost falls below this threshold.
    pub min_marginal_gain_per_cost: f64,
}

/// Why a candidate considered by EWS was not selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EwsRejectionReason {
    /// The candidate's marginal gain was zero.
    ZeroGain,
    /// The candidate fell below the configured marginal-gain/cost threshold.
    BelowThreshold,
    /// The candidate did not fit in the remaining local cost budget.
    Budget,
}

/// One deterministic decision emitted by an EWS evaluation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EwsDecision {
    pub carrier_id: u64,
    /// Delivery-probability gain if this candidate were selected now.
    pub marginal_delivery_gain: f64,
    /// Marginal delivery gain divided by the caller-supplied cost.
    pub marginal_gain_per_cost: f64,
    /// Whether this candidate was included in the local plan.
    pub accepted: bool,
    /// `None` for accepted candidates.
    pub rejection: Option<EwsRejectionReason>,
    /// Cumulative probability after this decision. For a rejected candidate,
    /// this remains the probability before the decision.
    pub cumulative_delivery_probability: f64,
    /// Cumulative cost after this decision. For a rejected candidate, this
    /// remains the cost before the decision.
    pub cumulative_cost: f64,
}

/// Result of one bounded local EWS evaluation.
#[derive(Debug, Clone, PartialEq)]
pub struct EwsPlan {
    /// Candidates considered in deterministic ranking order.
    pub decisions: Vec<EwsDecision>,
    pub selected_candidates: usize,
    /// `1 - product(1 - p_i)` for accepted candidates only.
    pub delivery_probability: f64,
    pub total_cost: f64,
}

/// Validation or derived-value failures for EWS inputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EwsError {
    /// More candidates were supplied than [`MAX_EWS_CANDIDATES`].
    TooManyCandidates,
    /// `max_copies` exceeds the evaluator bound.
    InvalidCopyLimit,
    /// Two candidates reused one caller label.
    DuplicateCarrier,
    /// A probability was NaN, infinite, or otherwise not representable.
    NonFiniteProbability,
    /// A probability was outside `[0, 1]`.
    ProbabilityOutOfRange,
    /// A cost was NaN, infinite, or not strictly positive.
    InvalidCost,
    /// The local budget was NaN, infinite, or negative.
    InvalidBudget,
    /// The threshold was NaN, infinite, or negative.
    InvalidThreshold,
    /// A derived ranking or cumulative value was not finite.
    SummaryOverflow,
    /// Evidence was produced for a different baseline replay.
    ProvenanceMismatch,
    /// The replay's reported metrics do not match its provenance.
    ReplayMetricsMismatch,
}

#[derive(Debug, Clone, Copy)]
struct RankedCandidate {
    candidate: EwsCandidate,
    base_gain_per_cost: f64,
}

/// Evaluate a bounded local EWS plan.
///
/// Candidates are sorted by descending `delivery_probability / cost`, then by
/// descending probability, ascending cost, and ascending `carrier_id`. The
/// evaluator greedily considers that deterministic order, recording budget
/// rejections so a caller can audit the local decision. This is not an
/// optimal knapsack solver and is not a security primitive.
pub fn evaluate(candidates: &[EwsCandidate], limits: EwsLimits) -> Result<EwsPlan, EwsError> {
    if candidates.len() > MAX_EWS_CANDIDATES {
        return Err(EwsError::TooManyCandidates);
    }
    if limits.max_copies > MAX_EWS_CANDIDATES {
        return Err(EwsError::InvalidCopyLimit);
    }
    if !limits.cost_budget.is_finite() || limits.cost_budget < 0.0 {
        return Err(EwsError::InvalidBudget);
    }
    if !limits.min_marginal_gain_per_cost.is_finite() || limits.min_marginal_gain_per_cost < 0.0 {
        return Err(EwsError::InvalidThreshold);
    }

    let mut seen = HashSet::with_capacity(candidates.len());
    let mut ranked = Vec::with_capacity(candidates.len());
    for &candidate in candidates {
        if !seen.insert(candidate.carrier_id) {
            return Err(EwsError::DuplicateCarrier);
        }
        if !candidate.delivery_probability.is_finite() {
            return Err(EwsError::NonFiniteProbability);
        }
        if !(0.0..=1.0).contains(&candidate.delivery_probability) {
            return Err(EwsError::ProbabilityOutOfRange);
        }
        if !candidate.cost.is_finite() || candidate.cost <= 0.0 {
            return Err(EwsError::InvalidCost);
        }
        let base_gain_per_cost = candidate.delivery_probability / candidate.cost;
        if !base_gain_per_cost.is_finite() {
            return Err(EwsError::SummaryOverflow);
        }
        ranked.push(RankedCandidate {
            candidate,
            base_gain_per_cost,
        });
    }

    ranked.sort_by(|left, right| {
        right
            .base_gain_per_cost
            .partial_cmp(&left.base_gain_per_cost)
            .expect("validated finite ranking")
            .then_with(|| {
                right
                    .candidate
                    .delivery_probability
                    .partial_cmp(&left.candidate.delivery_probability)
                    .expect("validated finite probability")
            })
            .then_with(|| {
                left.candidate
                    .cost
                    .partial_cmp(&right.candidate.cost)
                    .expect("validated finite cost")
            })
            .then_with(|| left.candidate.carrier_id.cmp(&right.candidate.carrier_id))
    });

    let mut residual_failure = 1.0;
    let mut total_cost = 0.0;
    let mut selected_candidates = 0;
    let mut decisions = Vec::with_capacity(ranked.len().min(limits.max_copies));

    for ranked_candidate in ranked {
        if selected_candidates >= limits.max_copies {
            break;
        }

        let candidate = ranked_candidate.candidate;
        let marginal_delivery_gain = candidate.delivery_probability * residual_failure;
        let marginal_gain_per_cost = marginal_delivery_gain / candidate.cost;
        if !marginal_gain_per_cost.is_finite() {
            return Err(EwsError::SummaryOverflow);
        }
        if marginal_delivery_gain == 0.0 {
            decisions.push(EwsDecision {
                carrier_id: candidate.carrier_id,
                marginal_delivery_gain,
                marginal_gain_per_cost,
                accepted: false,
                rejection: Some(EwsRejectionReason::ZeroGain),
                cumulative_delivery_probability: 1.0 - residual_failure,
                cumulative_cost: total_cost,
            });
            break;
        }
        if marginal_gain_per_cost < limits.min_marginal_gain_per_cost {
            decisions.push(EwsDecision {
                carrier_id: candidate.carrier_id,
                marginal_delivery_gain,
                marginal_gain_per_cost,
                accepted: false,
                rejection: Some(EwsRejectionReason::BelowThreshold),
                cumulative_delivery_probability: 1.0 - residual_failure,
                cumulative_cost: total_cost,
            });
            break;
        }

        let remaining_budget = limits.cost_budget - total_cost;
        if candidate.cost > remaining_budget {
            decisions.push(EwsDecision {
                carrier_id: candidate.carrier_id,
                marginal_delivery_gain,
                marginal_gain_per_cost,
                accepted: false,
                rejection: Some(EwsRejectionReason::Budget),
                cumulative_delivery_probability: 1.0 - residual_failure,
                cumulative_cost: total_cost,
            });
            continue;
        }

        let next_residual_failure = residual_failure * (1.0 - candidate.delivery_probability);
        let next_total_cost = total_cost + candidate.cost;
        let next_delivery_probability = 1.0 - next_residual_failure;
        if !next_residual_failure.is_finite()
            || !next_total_cost.is_finite()
            || !next_delivery_probability.is_finite()
        {
            return Err(EwsError::SummaryOverflow);
        }
        residual_failure = next_residual_failure;
        total_cost = next_total_cost;
        selected_candidates += 1;
        decisions.push(EwsDecision {
            carrier_id: candidate.carrier_id,
            marginal_delivery_gain,
            marginal_gain_per_cost,
            accepted: true,
            rejection: None,
            cumulative_delivery_probability: next_delivery_probability,
            cumulative_cost: total_cost,
        });
    }

    Ok(EwsPlan {
        decisions,
        selected_candidates,
        delivery_probability: 1.0 - residual_failure,
        total_cost,
    })
}

/// Caller-supplied EWS evidence paired with the exact baseline replay it
/// describes. The probabilities and costs remain assumptions from the caller;
/// this adapter does not infer them from baseline delivery metrics.
#[derive(Debug, Clone, Copy)]
pub struct EwsReplayEvidence<'a> {
    pub provenance: crate::ResearchProvenance,
    pub candidates: &'a [EwsCandidate],
}

/// A deterministic EWS sidecar result bound to one baseline replay.
///
/// This is research evidence only. It does not represent candidate transport
/// metrics and does not mutate a simulator, router, store, or wire frame.
#[derive(Debug, Clone, PartialEq)]
pub struct EwsReplayPlan {
    pub provenance: crate::ResearchProvenance,
    pub plan: EwsPlan,
    pub evidence_count: usize,
    /// BLAKE3-derived digest of the validated evidence, independent of input
    /// order. The digest is an audit label, not a security credential.
    pub evidence_fingerprint: [u8; 16],
}

/// Evaluate EWS evidence against one unchanged MMP/1 baseline replay.
///
/// Exact provenance equality prevents accidentally pairing a candidate model
/// with a different seed, contact schedule, workload, or generator. The
/// baseline's trace and generator are checked independently as a consistency
/// guard. No baseline metric is used to derive `delivery_probability` or
/// `cost`.
pub fn evaluate_replay(
    replay: &crate::Mmp1Replay,
    evidence: EwsReplayEvidence<'_>,
    limits: EwsLimits,
) -> Result<EwsReplayPlan, EwsError> {
    if evidence.provenance != replay.provenance {
        return Err(EwsError::ProvenanceMismatch);
    }
    if replay.metrics.trace_id != replay.provenance.trace_id
        || replay.metrics.generator != replay.provenance.generator
    {
        return Err(EwsError::ReplayMetricsMismatch);
    }

    let plan = evaluate(evidence.candidates, limits)?;
    let evidence_fingerprint = fingerprint(evidence.candidates);
    Ok(EwsReplayPlan {
        provenance: replay.provenance,
        plan,
        evidence_count: evidence.candidates.len(),
        evidence_fingerprint,
    })
}

fn fingerprint(candidates: &[EwsCandidate]) -> [u8; 16] {
    let mut canonical = candidates.to_vec();
    canonical.sort_by_key(|candidate| candidate.carrier_id);
    let mut hasher = blake3::Hasher::new_derive_key("mmp1 ews evidence-v1");
    hasher.update(&(canonical.len() as u64).to_le_bytes());
    for candidate in canonical {
        hasher.update(&candidate.carrier_id.to_le_bytes());
        hasher.update(&candidate.delivery_probability.to_bits().to_le_bytes());
        hasher.update(&candidate.cost.to_bits().to_le_bytes());
    }
    let digest = hasher.finalize();
    let mut result = [0_u8; 16];
    result.copy_from_slice(&digest.as_bytes()[..16]);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ews_orders_candidates_deterministically_and_reports_marginal_gain() {
        let limits = EwsLimits {
            cost_budget: 10.0,
            max_copies: 3,
            min_marginal_gain_per_cost: 0.0,
        };
        let first = evaluate(
            &[
                EwsCandidate {
                    carrier_id: 3,
                    delivery_probability: 0.5,
                    cost: 2.0,
                },
                EwsCandidate {
                    carrier_id: 2,
                    delivery_probability: 0.4,
                    cost: 1.0,
                },
                EwsCandidate {
                    carrier_id: 1,
                    delivery_probability: 0.4,
                    cost: 1.0,
                },
            ],
            limits,
        )
        .unwrap();
        let second = evaluate(
            &[
                EwsCandidate {
                    carrier_id: 1,
                    delivery_probability: 0.4,
                    cost: 1.0,
                },
                EwsCandidate {
                    carrier_id: 3,
                    delivery_probability: 0.5,
                    cost: 2.0,
                },
                EwsCandidate {
                    carrier_id: 2,
                    delivery_probability: 0.4,
                    cost: 1.0,
                },
            ],
            limits,
        )
        .unwrap();

        assert_eq!(first, second);
        assert_eq!(
            first
                .decisions
                .iter()
                .map(|decision| decision.carrier_id)
                .collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert_eq!(first.selected_candidates, 3);
        assert!((first.decisions[0].marginal_delivery_gain - 0.4).abs() < 1e-12);
        assert!((first.decisions[1].marginal_delivery_gain - 0.24).abs() < 1e-12);
        assert!((first.decisions[2].marginal_delivery_gain - 0.18).abs() < 1e-12);
        assert!((first.delivery_probability - 0.82).abs() < 1e-12);
    }

    #[test]
    fn ews_records_budget_rejection_and_keeps_local_copy_bound() {
        let plan = evaluate(
            &[
                EwsCandidate {
                    carrier_id: 10,
                    delivery_probability: 0.8,
                    cost: 1.5,
                },
                EwsCandidate {
                    carrier_id: 11,
                    delivery_probability: 0.7,
                    cost: 0.6,
                },
                EwsCandidate {
                    carrier_id: 12,
                    delivery_probability: 0.9,
                    cost: 1.0,
                },
            ],
            EwsLimits {
                cost_budget: 2.0,
                max_copies: 3,
                min_marginal_gain_per_cost: 0.0,
            },
        )
        .unwrap();

        assert_eq!(plan.selected_candidates, 2);
        assert_eq!(plan.total_cost, 1.6);
        assert!((plan.delivery_probability - 0.97).abs() < 1e-12);
        assert_eq!(plan.decisions[2].carrier_id, 10);
        assert!(!plan.decisions[2].accepted);
        assert_eq!(
            plan.decisions[2].rejection,
            Some(EwsRejectionReason::Budget)
        );
        // This bound applies to this local plan only; it is not a global
        // security guarantee about what remote relays may replicate.
        assert!(plan.selected_candidates <= 3);
    }

    #[test]
    fn ews_budget_rejection_does_not_stop_later_affordable_candidates() {
        // The highest-ranked carrier is individually unaffordable. A budget
        // rejection must not consume the budget or terminate the scan: a
        // later, lower-ranked carrier may still fit and improve delivery.
        let plan = evaluate(
            &[
                EwsCandidate {
                    carrier_id: 7,
                    delivery_probability: 0.9,
                    cost: 2.0,
                },
                EwsCandidate {
                    carrier_id: 8,
                    delivery_probability: 0.4,
                    cost: 1.0,
                },
            ],
            EwsLimits {
                cost_budget: 1.0,
                max_copies: 2,
                min_marginal_gain_per_cost: 0.0,
            },
        )
        .unwrap();

        assert_eq!(plan.selected_candidates, 1);
        assert_eq!(plan.total_cost, 1.0);
        assert!((plan.delivery_probability - 0.4).abs() < 1e-12);

        let rejected = plan.decisions[0];
        assert_eq!(rejected.carrier_id, 7);
        assert!(!rejected.accepted);
        assert_eq!(rejected.rejection, Some(EwsRejectionReason::Budget));
        assert_eq!(rejected.cumulative_delivery_probability, 0.0);
        assert_eq!(rejected.cumulative_cost, 0.0);

        let accepted = plan.decisions[1];
        assert_eq!(accepted.carrier_id, 8);
        assert!(accepted.accepted);
        assert_eq!(accepted.rejection, None);
        assert!((accepted.marginal_delivery_gain - 0.4).abs() < 1e-12);
        assert!((accepted.cumulative_delivery_probability - 0.4).abs() < 1e-12);
        assert_eq!(accepted.cumulative_cost, 1.0);
    }

    #[test]
    fn ews_rejects_invalid_inputs_and_derived_overflow() {
        let limits = EwsLimits {
            cost_budget: 1.0,
            max_copies: 1,
            min_marginal_gain_per_cost: 0.0,
        };
        assert_eq!(
            evaluate(
                &[EwsCandidate {
                    carrier_id: 1,
                    delivery_probability: f64::NAN,
                    cost: 1.0
                }],
                limits
            ),
            Err(EwsError::NonFiniteProbability)
        );
        assert_eq!(
            evaluate(
                &[EwsCandidate {
                    carrier_id: 1,
                    delivery_probability: 1.1,
                    cost: 1.0
                }],
                limits
            ),
            Err(EwsError::ProbabilityOutOfRange)
        );
        assert_eq!(
            evaluate(
                &[
                    EwsCandidate {
                        carrier_id: 1,
                        delivery_probability: 0.5,
                        cost: 1.0
                    },
                    EwsCandidate {
                        carrier_id: 1,
                        delivery_probability: 0.4,
                        cost: 1.0
                    },
                ],
                limits,
            ),
            Err(EwsError::DuplicateCarrier)
        );
        assert_eq!(
            evaluate(
                &[EwsCandidate {
                    carrier_id: 1,
                    delivery_probability: 1.0,
                    cost: f64::from_bits(1)
                }],
                limits
            ),
            Err(EwsError::SummaryOverflow)
        );
        assert_eq!(
            evaluate(
                &[],
                EwsLimits {
                    cost_budget: -1.0,
                    ..limits
                }
            ),
            Err(EwsError::InvalidBudget)
        );
        assert_eq!(
            evaluate(
                &[],
                EwsLimits {
                    min_marginal_gain_per_cost: f64::NAN,
                    ..limits
                }
            ),
            Err(EwsError::InvalidThreshold)
        );
    }

    fn baseline_replay_fixture() -> crate::Mmp1Replay {
        let schedule = crate::ContactSchedule::new(
            3,
            0xE550_0001,
            vec![crate::ContactEvent {
                left: 0,
                right: 1,
                gap_ms: 1_000,
            }],
        )
        .expect("valid schedule");
        crate::ResearchExperiment::new(
            &schedule,
            crate::FixedWorkload {
                sender: 0,
                recipient: 1,
                text: "ews replay",
            },
            crate::ContactGeneratorKind::UniformRandom,
        )
        .run_mmp1_baseline()
        .expect("baseline replay")
    }

    #[test]
    fn ews_replay_adapter_binds_plan_to_exact_provenance_and_order_independent_evidence() {
        let replay = baseline_replay_fixture();
        let first_candidates = [
            EwsCandidate {
                carrier_id: 7,
                delivery_probability: 0.4,
                cost: 1.0,
            },
            EwsCandidate {
                carrier_id: 3,
                delivery_probability: 0.6,
                cost: 2.0,
            },
        ];
        let second_candidates = [first_candidates[1], first_candidates[0]];
        let limits = EwsLimits {
            cost_budget: 3.0,
            max_copies: 2,
            min_marginal_gain_per_cost: 0.0,
        };
        let first = evaluate_replay(
            &replay,
            EwsReplayEvidence {
                provenance: replay.provenance,
                candidates: &first_candidates,
            },
            limits,
        )
        .expect("first sidecar plan");
        let second = evaluate_replay(
            &replay,
            EwsReplayEvidence {
                provenance: replay.provenance,
                candidates: &second_candidates,
            },
            limits,
        )
        .expect("reordered sidecar plan");

        assert_eq!(first.plan, second.plan);
        assert_eq!(first.evidence_count, 2);
        assert_eq!(first.evidence_fingerprint, second.evidence_fingerprint);
        assert_eq!(first.provenance, replay.provenance);
        assert_eq!(first.plan.selected_candidates, 2);
    }

    #[test]
    fn ews_replay_adapter_rejects_mismatched_evidence_provenance() {
        let replay = baseline_replay_fixture();
        let mut mismatched = replay.provenance;
        mismatched.workload_id[0] ^= 1;
        let result = evaluate_replay(
            &replay,
            EwsReplayEvidence {
                provenance: mismatched,
                candidates: &[],
            },
            EwsLimits {
                cost_budget: 0.0,
                max_copies: 0,
                min_marginal_gain_per_cost: 0.0,
            },
        );

        assert_eq!(result, Err(EwsError::ProvenanceMismatch));
    }

    #[test]
    fn ews_replay_adapter_rejects_inconsistent_baseline_metrics() {
        let replay = baseline_replay_fixture();
        let mut inconsistent = replay;
        inconsistent.metrics.trace_id[0] ^= 1;
        let result = evaluate_replay(
            &inconsistent,
            EwsReplayEvidence {
                provenance: inconsistent.provenance,
                candidates: &[],
            },
            EwsLimits {
                cost_budget: 0.0,
                max_copies: 0,
                min_marginal_gain_per_cost: 0.0,
            },
        );

        assert_eq!(result, Err(EwsError::ReplayMetricsMismatch));
    }
}
