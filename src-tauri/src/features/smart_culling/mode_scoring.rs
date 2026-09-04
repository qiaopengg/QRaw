use super::expression::EXPRESSION_QUALITY_GATE_ENABLED;
pub(crate) use super::mode_evidence::normalize_focus;
use super::mode_evidence::{
    ExpressionAssessment, LEGACY_CLARITY_CONFIDENCE_CAP, adapt_mode_evidence, clarity_evidence,
    expression_assessment, face_area, has_weak_person_evidence, important_faces,
    weakest_eye_disposition,
};
use super::mode_policy::{
    ClarityGateTarget, ModePolicy, ModeStrategy, policy_for, resolve_strategy,
};
use super::portrait_rating::PortraitRatingDecision;
use super::portrait_rating_adapter::{PortraitEvidenceDecision, adapt_portrait_rating};
use super::quality_evidence::{FixedScoreInterval, WeightedEvidence, combine_fixed_weights};
use super::scoring::AnalysisCandidate;
use super::types::{EyeDisposition, FaceResult};

#[derive(Clone, Debug)]
pub(crate) struct ModeEvaluation {
    pub resolved_mode: String,
    pub score: f64,
    pub rating: u8,
    pub confidence: f32,
    pub requires_human_review: bool,
    pub reason_code: &'static str,
}

pub(crate) fn evaluate_mode(requested: &str, item: &AnalysisCandidate) -> ModeEvaluation {
    let important = important_faces(item);
    let strategy = resolve_strategy(
        requested,
        !important.is_empty(),
        !item.key_person_evidence.is_empty(),
    );
    let policy = policy_for(strategy);

    let subjects = match strategy {
        ModeStrategy::Portrait if item.key_person_evidence.is_empty() => important
            .iter()
            .copied()
            .max_by(|left, right| face_area(left).total_cmp(&face_area(right)))
            .into_iter()
            .collect(),
        ModeStrategy::Portrait | ModeStrategy::GroupWithKeyPeople => {
            let Some(subjects) = confirmed_key_subjects(item) else {
                return manual_evaluation(
                    policy.resolved_mode(),
                    key_subject_unresolved_reason(strategy),
                );
            };
            subjects
        }
        ModeStrategy::Environment | ModeStrategy::AutoPeople => important,
        ModeStrategy::GroupScene | ModeStrategy::Landscape | ModeStrategy::AutoScene => Vec::new(),
    };

    if matches!(strategy, ModeStrategy::Portrait) && subjects.is_empty() {
        return manual_evaluation("portrait", "portrait_subject_unreliable");
    }
    if matches!(strategy, ModeStrategy::Environment) && subjects.is_empty() {
        return manual_evaluation("environment", "environment_subject_unreliable");
    }

    let clarity = clarity_evidence(policy.clarity_gate, item, &subjects);
    if strategy == ModeStrategy::Portrait {
        return match adapt_portrait_rating(&clarity, &subjects) {
            PortraitEvidenceDecision::NeedsClarityReview => {
                manual_evaluation("portrait", "portrait_person_clarity_unresolved")
            }
            PortraitEvidenceDecision::Rated {
                decision,
                confidence,
            } => {
                let rating = decision.final_rating();
                ModeEvaluation {
                    resolved_mode: "portrait".to_string(),
                    score: f64::from(rating) / 5.0,
                    rating,
                    confidence,
                    requires_human_review: false,
                    reason_code: match decision {
                        PortraitRatingDecision::SubjectUnclear => "portrait_person_unclear",
                        PortraitRatingDecision::Scored(_) => "portrait_discrete_partial_evidence",
                    },
                }
            }
        };
    }
    if clarity.is_one_star_gate() {
        return one_star_evaluation(
            policy.resolved_mode(),
            clarity_gate_reason(strategy, policy.clarity_gate),
        );
    }

    // Preserve the independently frozen eye contract before the expression
    // calibration gate. This is a hard defect decision, not combined scoring.
    if policy.closed_eye_hard_cap
        && weakest_eye_disposition(&subjects) == Some(EyeDisposition::Unusable)
    {
        return one_star_evaluation(policy.resolved_mode(), closed_eye_reason(strategy));
    }

    // Expression must be assessed before weighted combination, but a low
    // expression score is valid evidence rather than a reason to skip rating.
    if EXPRESSION_QUALITY_GATE_ENABLED
        && policy.weights.expression > f64::EPSILON
        && expression_assessment(&subjects) == ExpressionAssessment::Unknown
    {
        return manual_evaluation(
            policy.resolved_mode(),
            expression_unresolved_reason(strategy),
        );
    }

    let mut evaluation = evaluate_strategy(policy, item, &subjects);
    if matches!(strategy, ModeStrategy::AutoScene) && has_weak_person_evidence(item) {
        evaluation.requires_human_review = true;
        evaluation.reason_code = "auto_people_uncertain";
    }
    evaluation
}

pub(crate) fn rating_for_mode(mode: &str, score: f64) -> u8 {
    let [two, three, four, five] = rating_thresholds(mode);
    if score >= five {
        5
    } else if score >= four {
        4
    } else if score >= three {
        3
    } else if score >= two {
        2
    } else {
        1
    }
}

fn evaluate_strategy(
    policy: ModePolicy,
    item: &AnalysisCandidate,
    subjects: &[&FaceResult],
) -> ModeEvaluation {
    let evidence = adapt_mode_evidence(item, subjects);
    let weights = policy.weights;
    let mut interval = combine_fixed_weights(&[
        WeightedEvidence {
            evidence: &evidence.person_clarity,
            weight: weights.person_clarity,
        },
        WeightedEvidence {
            evidence: &evidence.eyes,
            weight: weights.eyes,
        },
        WeightedEvidence {
            evidence: &evidence.expression,
            weight: weights.expression,
        },
        WeightedEvidence {
            evidence: &evidence.optical,
            weight: weights.optical,
        },
        WeightedEvidence {
            evidence: &evidence.composition,
            weight: weights.composition,
        },
    ])
    .expect("mode policy weights and validated evidence must combine");

    let eye_disposition = weakest_eye_disposition(subjects);
    if policy.closed_eye_hard_cap {
        interval = match eye_disposition {
            Some(EyeDisposition::Unusable) => cap_interval(
                interval,
                maximum_score_for_rating(policy.resolved_mode(), 1),
            ),
            Some(EyeDisposition::DeliberatePoseCandidate) => cap_interval(
                interval,
                maximum_score_for_rating(policy.resolved_mode(), 3),
            ),
            _ => interval,
        };
    }

    let lower_rating = rating_for_mode(policy.resolved_mode(), interval.lower_bound);
    let upper_rating = rating_for_mode(policy.resolved_mode(), interval.upper_bound);
    let requires_human_review = lower_rating != upper_rating;
    let reason_code = strategy_reason(
        policy.strategy,
        eye_disposition,
        subjects,
        requires_human_review,
        interval.missing_weight > f64::EPSILON,
    );

    ModeEvaluation {
        resolved_mode: policy.resolved_mode().to_string(),
        score: interval.lower_bound,
        rating: lower_rating,
        confidence: interval.confidence.clamp(0.0, 1.0) as f32,
        requires_human_review,
        reason_code,
    }
}

fn confirmed_key_subjects(item: &AnalysisCandidate) -> Option<Vec<&FaceResult>> {
    if item.key_person_evidence.is_empty()
        || item
            .key_person_evidence
            .iter()
            .any(|evidence| evidence.status != "confirmed" || !evidence.auto_score_eligible)
    {
        return None;
    }
    item.key_person_evidence
        .iter()
        .map(|evidence| item.faces.get(evidence.face_index?))
        .collect()
}

fn cap_interval(mut interval: FixedScoreInterval, cap: f64) -> FixedScoreInterval {
    interval.lower_bound = interval.lower_bound.min(cap);
    interval.upper_bound = interval.upper_bound.min(cap);
    interval
}

fn rating_thresholds(mode: &str) -> [f64; 4] {
    match mode {
        "portrait" => [0.32, 0.48, 0.63, 0.79],
        "environment" => [0.34, 0.50, 0.65, 0.80],
        "group" => [0.30, 0.46, 0.62, 0.78],
        _ => [0.34, 0.50, 0.64, 0.78],
    }
}

fn maximum_score_for_rating(mode: &str, rating: u8) -> f64 {
    match rating {
        1 => rating_thresholds(mode)[0] - 1e-6,
        2 => rating_thresholds(mode)[1] - 1e-6,
        3 => rating_thresholds(mode)[2] - 1e-6,
        4 => rating_thresholds(mode)[3] - 1e-6,
        _ => 1.0,
    }
}

fn manual_evaluation(mode: &str, reason_code: &'static str) -> ModeEvaluation {
    ModeEvaluation {
        resolved_mode: mode.to_string(),
        score: 0.0,
        rating: 0,
        confidence: 0.0,
        requires_human_review: true,
        reason_code,
    }
}

fn one_star_evaluation(mode: &str, reason_code: &'static str) -> ModeEvaluation {
    ModeEvaluation {
        resolved_mode: mode.to_string(),
        score: 0.0,
        rating: 1,
        confidence: LEGACY_CLARITY_CONFIDENCE_CAP as f32,
        requires_human_review: false,
        reason_code,
    }
}

fn key_subject_unresolved_reason(strategy: ModeStrategy) -> &'static str {
    match strategy {
        ModeStrategy::Portrait => "portrait_key_person_unresolved",
        ModeStrategy::GroupWithKeyPeople => "group_key_person_unresolved",
        _ => "key_person_unresolved",
    }
}

fn clarity_gate_reason(strategy: ModeStrategy, target: ClarityGateTarget) -> &'static str {
    match (strategy, target) {
        (ModeStrategy::Portrait, ClarityGateTarget::Person) => "portrait_person_unclear",
        (ModeStrategy::GroupWithKeyPeople, ClarityGateTarget::Person) => "group_key_person_unclear",
        (ModeStrategy::GroupScene, ClarityGateTarget::Image) => "group_image_unclear",
        (ModeStrategy::Environment, ClarityGateTarget::Image) => "environment_image_unclear",
        (ModeStrategy::Landscape, ClarityGateTarget::Image) => "landscape_image_unclear",
        (ModeStrategy::AutoPeople | ModeStrategy::AutoScene, ClarityGateTarget::Image) => {
            "auto_image_unclear"
        }
        _ => "clarity_gate_unclear",
    }
}

fn closed_eye_reason(strategy: ModeStrategy) -> &'static str {
    if strategy == ModeStrategy::Portrait {
        "portrait_closed_eyes"
    } else {
        "group_closed_eyes"
    }
}

fn expression_unresolved_reason(strategy: ModeStrategy) -> &'static str {
    match strategy {
        ModeStrategy::Portrait => "portrait_expression_unresolved",
        ModeStrategy::GroupWithKeyPeople => "group_expression_unresolved",
        ModeStrategy::Environment => "environment_expression_unresolved",
        ModeStrategy::AutoPeople => "auto_expression_unresolved",
        _ => "expression_unresolved",
    }
}

fn strategy_reason(
    strategy: ModeStrategy,
    eye_disposition: Option<EyeDisposition>,
    subjects: &[&FaceResult],
    requires_human_review: bool,
    has_missing_evidence: bool,
) -> &'static str {
    if matches!(
        strategy,
        ModeStrategy::Portrait | ModeStrategy::GroupWithKeyPeople
    ) {
        if eye_disposition == Some(EyeDisposition::Unusable) {
            return closed_eye_reason(strategy);
        }
        if eye_disposition == Some(EyeDisposition::DeliberatePoseCandidate) {
            return if strategy == ModeStrategy::Portrait {
                "portrait_deliberate_eye_pose"
            } else {
                "group_deliberate_eye_pose"
            };
        }
    }
    if subjects
        .iter()
        .any(|face| face.expression_state == "transitional")
    {
        return match strategy {
            ModeStrategy::Portrait => "portrait_expression_transition",
            ModeStrategy::GroupWithKeyPeople => "group_expression_transition",
            ModeStrategy::Environment => "environment_expression_transition",
            ModeStrategy::AutoPeople => "auto_expression_transition",
            _ => "expression_transition",
        };
    }
    match (strategy, requires_human_review, has_missing_evidence) {
        (ModeStrategy::Portrait, true, _) => "portrait_evidence_interval_review",
        (ModeStrategy::GroupWithKeyPeople, true, _) => "group_key_evidence_interval_review",
        (ModeStrategy::GroupScene, true, _) => "group_scene_evidence_interval_review",
        (ModeStrategy::Environment, true, _) => "environment_evidence_interval_review",
        (ModeStrategy::Landscape, true, _) => "landscape_evidence_interval_review",
        (ModeStrategy::AutoPeople | ModeStrategy::AutoScene, true, _) => {
            "auto_evidence_interval_review"
        }
        (ModeStrategy::Portrait, false, true) => "portrait_partial_evidence_same_tier",
        (ModeStrategy::GroupWithKeyPeople, false, true) => "group_key_partial_evidence_same_tier",
        (ModeStrategy::GroupScene, false, true) => "group_scene_partial_evidence_same_tier",
        (ModeStrategy::Environment, false, true) => "environment_partial_evidence_same_tier",
        (ModeStrategy::Landscape, false, true) => "landscape_partial_evidence_same_tier",
        (ModeStrategy::AutoPeople | ModeStrategy::AutoScene, false, true) => {
            "auto_partial_evidence_same_tier"
        }
        (ModeStrategy::Portrait, false, false) => "portrait_weighted_score",
        (ModeStrategy::GroupWithKeyPeople, false, false) => "group_key_weighted_score",
        (ModeStrategy::GroupScene, false, false) => "group_scene_weighted_score",
        (ModeStrategy::Environment, false, false) => "environment_weighted_score",
        (ModeStrategy::Landscape, false, false) => "landscape_weighted_score",
        (ModeStrategy::AutoPeople | ModeStrategy::AutoScene, false, false) => "auto_weighted_score",
    }
}

#[cfg(test)]
#[path = "mode_scoring_tests.rs"]
mod tests;
