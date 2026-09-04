//! Maps current portrait evidence into the frozen discrete rating contract.

use super::mode_evidence::weakest_eye_disposition;
use super::portrait_rating::{
    ExpressionRatingState, EyeRatingState, OpticalAestheticChecks, PortraitRatingDecision,
    PortraitRatingInput, SubjectClarity, ValidationState, calculate_portrait_rating,
};
use super::quality_evidence::{ClarityEvidence, ClarityState};
use super::types::{EyeDisposition, FaceResult};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum PortraitEvidenceDecision {
    NeedsClarityReview,
    Rated {
        decision: PortraitRatingDecision,
        confidence: f32,
    },
}

pub(crate) fn adapt_portrait_rating(
    clarity: &ClarityEvidence,
    subjects: &[&FaceResult],
) -> PortraitEvidenceDecision {
    let subject_clarity = match clarity.state {
        ClarityState::Clear => SubjectClarity::Clear,
        ClarityState::Unclear => SubjectClarity::Unclear,
        ClarityState::Uncertain => return PortraitEvidenceDecision::NeedsClarityReview,
    };
    let eyes = eye_rating_state(subjects);
    let expression = expression_rating_state(subjects);
    // QRaw has a conservative optical proxy and an observation-only aesthetics
    // signal, but no jointly validated optical+composition decision yet.
    let optical_aesthetic = OpticalAestheticChecks {
        optical: ValidationState::UnableToDetermine,
        aesthetic_composition: ValidationState::UnableToDetermine,
    };
    let decision = calculate_portrait_rating(PortraitRatingInput {
        subject_clarity,
        eyes,
        expression,
        optical_aesthetic,
    });
    PortraitEvidenceDecision::Rated {
        decision,
        confidence: decided_confidence(clarity, subjects),
    }
}

fn eye_rating_state(subjects: &[&FaceResult]) -> EyeRatingState {
    match weakest_eye_disposition(subjects) {
        Some(EyeDisposition::Open) => EyeRatingState::Passed,
        Some(EyeDisposition::Unusable) => EyeRatingState::Failed,
        Some(EyeDisposition::Unknown | EyeDisposition::DeliberatePoseCandidate) | None => {
            EyeRatingState::UnableToDetermine
        }
    }
}

fn expression_rating_state(subjects: &[&FaceResult]) -> ExpressionRatingState {
    if subjects.is_empty() {
        return ExpressionRatingState::UnableToDetermine;
    }
    let mut selected = ExpressionRatingState::Outstanding;
    for face in subjects {
        let state = match face.expression_state.as_str() {
            "severe_failure" => ExpressionRatingState::SevereFailure,
            "not_recommended" => ExpressionRatingState::NotRecommended,
            "natural" => ExpressionRatingState::Natural,
            "excellent" => ExpressionRatingState::Excellent,
            "outstanding" => ExpressionRatingState::Outstanding,
            _ => ExpressionRatingState::UnableToDetermine,
        };
        if state.increment() < selected.increment()
            || (state == ExpressionRatingState::UnableToDetermine
                && selected.increment() >= state.increment())
        {
            selected = state;
        }
    }
    selected
}

fn decided_confidence(clarity: &ClarityEvidence, subjects: &[&FaceResult]) -> f32 {
    let mut confidence = clarity.evidence.confidence as f32;
    for face in subjects {
        if face.eye_disposition != EyeDisposition::Unknown {
            confidence = confidence.min(face.left_eye.confidence.min(face.right_eye.confidence));
        }
        if matches!(
            face.expression_state.as_str(),
            "severe_failure" | "not_recommended" | "natural" | "excellent" | "outstanding"
        ) {
            confidence = confidence.min(face.expression_confidence);
        }
    }
    confidence.clamp(0.0, 1.0)
}

#[cfg(test)]
#[path = "portrait_rating_adapter_tests.rs"]
mod tests;
