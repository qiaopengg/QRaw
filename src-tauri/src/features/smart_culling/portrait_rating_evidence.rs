//! Review-only projection of the actual rating decision and its subject states.
//! This module never computes a rating or promotes unvalidated measurements.
use serde::Serialize;

use super::mode_scoring::{ModeEvaluation, portrait_subjects};
use super::portrait_rating::{
    ExpressionRatingState, EyeRatingState, OpticalAestheticChecks, PORTRAIT_RATING_POLICY_VERSION,
    PortraitRatingBreakdown, PortraitRatingDecision,
};
use super::portrait_rating_adapter::{
    expression_rating_state, eye_rating_state, optical_aesthetic_checks,
};
use super::scoring::AnalysisCandidate;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortraitRatingEvidence {
    version: &'static str,
    subject_face_indices: Vec<usize>,
    eyes: EyeRatingState,
    expression: ExpressionRatingState,
    optical_aesthetic: OpticalAestheticChecks,
    status: RatingStatus,
    breakdown: Option<PortraitRatingBreakdown>,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
enum RatingStatus {
    Pending,
    SubjectUnclear,
    Scored,
}

pub(super) fn for_review(
    item: &AnalysisCandidate,
    evaluation: &ModeEvaluation,
    requires_human_review: bool,
) -> Option<PortraitRatingEvidence> {
    if evaluation.resolved_mode != "portrait" {
        return None;
    }
    let subjects = portrait_subjects(item).unwrap_or_default();
    // Group/identity review can defer an otherwise valid mode decision.
    let (status, breakdown) = match (requires_human_review, evaluation.portrait_decision) {
        (false, Some(PortraitRatingDecision::Scored(breakdown))) => {
            (RatingStatus::Scored, Some(breakdown))
        }
        (false, Some(PortraitRatingDecision::SubjectUnclear)) => {
            (RatingStatus::SubjectUnclear, None)
        }
        _ => (RatingStatus::Pending, None),
    };
    Some(PortraitRatingEvidence {
        version: PORTRAIT_RATING_POLICY_VERSION,
        subject_face_indices: subjects
            .iter()
            .filter_map(|subject| {
                item.faces
                    .iter()
                    .position(|face| std::ptr::eq(face, *subject))
            })
            .collect(),
        eyes: eye_rating_state(&subjects),
        expression: expression_rating_state(&subjects),
        optical_aesthetic: optical_aesthetic_checks(),
        status,
        breakdown,
    })
}
