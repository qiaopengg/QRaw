use super::*;
use crate::features::smart_culling::quality_evidence::ClarityEvidence;
use crate::features::smart_culling::types::EyeResult;

fn eye(state: &str) -> EyeResult {
    EyeResult {
        open_probability: None,
        state: state.to_string(),
        confidence: if state == "unknown" { 0.0 } else { 0.9 },
        reason: "test".to_string(),
        effective_pixels: 100,
        sharpness_metric: Some(100.0),
    }
}

fn face(eyes: EyeDisposition, expression: &str) -> FaceResult {
    let eye_state = if eyes == EyeDisposition::Unknown {
        "unknown"
    } else {
        "known"
    };
    FaceResult {
        bbox: [0.0, 0.0, 100.0, 100.0],
        landmarks: [(0.0, 0.0); 5],
        detection_score: 0.95,
        left_eye: eye(eye_state),
        right_eye: eye(eye_state),
        eye_disposition: eyes,
        expression_state: expression.to_string(),
        expression_score: Some(0.5),
        expression_confidence: 0.4,
        expression_reason: "test".to_string(),
        expression_descriptor: None,
        sharpness_metric: 300.0,
        sharpness_confidence: 0.9,
        exposure_metric: 0.8,
        exposure_confidence: 0.9,
        identity_embedding: None,
    }
}

fn clarity(state: ClarityState) -> ClarityEvidence {
    ClarityEvidence::try_new(state, Some(0.8), 0.25, "test", "test-v1").unwrap()
}

fn rating(state: ClarityState, eyes: EyeDisposition, expression: &str) -> u8 {
    let face = face(eyes, expression);
    let PortraitEvidenceDecision::Rated { decision, .. } =
        adapt_portrait_rating(&clarity(state), &[&face])
    else {
        panic!("expected a discrete portrait rating")
    };
    decision.final_rating()
}

#[test]
fn unclear_subject_is_zero_before_eye_or_expression_can_help() {
    assert_eq!(
        rating(ClarityState::Unclear, EyeDisposition::Open, "outstanding"),
        0
    );
}

#[test]
fn uncertain_subject_requires_review_instead_of_guessing_clear_or_unclear() {
    let face = face(EyeDisposition::Open, "natural");
    assert_eq!(
        adapt_portrait_rating(&clarity(ClarityState::Uncertain), &[&face]),
        PortraitEvidenceDecision::NeedsClarityReview
    );
}

#[test]
fn five_expression_levels_map_to_the_frozen_increments() {
    let levels = [
        ("severe_failure", 0),
        ("not_recommended", 1),
        ("natural", 2),
        ("excellent", 3),
        ("outstanding", 4),
    ];
    for (expression, expected) in levels {
        assert_eq!(
            rating(ClarityState::Clear, EyeDisposition::Open, expression),
            expected
        );
    }
}

#[test]
fn unknown_signals_add_zero_without_forcing_manual_review() {
    let face = face(EyeDisposition::Unknown, "unknown");
    let PortraitEvidenceDecision::Rated { decision, .. } =
        adapt_portrait_rating(&clarity(ClarityState::Clear), &[&face])
    else {
        panic!("clear subject must remain scoreable")
    };

    assert_eq!(decision.final_rating(), 0);
}

#[test]
fn missing_subject_cannot_be_promoted_to_an_outstanding_expression() {
    let PortraitEvidenceDecision::Rated { decision, .. } =
        adapt_portrait_rating(&clarity(ClarityState::Clear), &[])
    else {
        panic!("the adapter should preserve the unable-to-determine state")
    };

    assert_eq!(decision.final_rating(), 0);
}

#[test]
fn closed_eyes_are_negative_two_instead_of_a_one_star_hard_cap() {
    assert_eq!(
        rating(ClarityState::Clear, EyeDisposition::Unusable, "outstanding"),
        0
    );
}
