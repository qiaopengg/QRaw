use super::*;

const EYE_STATES: [EyeRatingState; 3] = [
    EyeRatingState::UnableToDetermine,
    EyeRatingState::Passed,
    EyeRatingState::Failed,
];
const EXPRESSION_STATES: [ExpressionRatingState; 6] = [
    ExpressionRatingState::UnableToDetermine,
    ExpressionRatingState::SevereFailure,
    ExpressionRatingState::NotRecommended,
    ExpressionRatingState::Natural,
    ExpressionRatingState::Excellent,
    ExpressionRatingState::Outstanding,
];
const VALIDATION_STATES: [ValidationState; 3] = [
    ValidationState::Passed,
    ValidationState::NotPassed,
    ValidationState::UnableToDetermine,
];

fn input(
    subject_clarity: SubjectClarity,
    eyes: EyeRatingState,
    expression: ExpressionRatingState,
    optical: ValidationState,
    aesthetic_composition: ValidationState,
) -> PortraitRatingInput {
    PortraitRatingInput {
        subject_clarity,
        eyes,
        expression,
        optical_aesthetic: OpticalAestheticChecks {
            optical,
            aesthetic_composition,
        },
    }
}

fn assert_scored(input: PortraitRatingInput, raw_rating: i8, final_rating: u8) {
    let decision = calculate_portrait_rating(input);
    let PortraitRatingDecision::Scored(breakdown) = decision else {
        panic!("clear subject must produce a scored decision");
    };
    assert_eq!(breakdown.raw_rating, raw_rating);
    assert_eq!(breakdown.final_rating, final_rating);
    assert_eq!(decision.final_rating(), final_rating);
}

#[test]
fn matches_all_frozen_boundary_examples() {
    assert_eq!(
        calculate_portrait_rating(input(
            SubjectClarity::Unclear,
            EyeRatingState::Passed,
            ExpressionRatingState::Outstanding,
            ValidationState::Passed,
            ValidationState::Passed,
        )),
        PortraitRatingDecision::SubjectUnclear
    );
    assert_scored(
        input(
            SubjectClarity::Clear,
            EyeRatingState::Passed,
            ExpressionRatingState::Outstanding,
            ValidationState::Passed,
            ValidationState::Passed,
        ),
        5,
        5,
    );
    assert_scored(
        input(
            SubjectClarity::Clear,
            EyeRatingState::Passed,
            ExpressionRatingState::Natural,
            ValidationState::Passed,
            ValidationState::Passed,
        ),
        3,
        3,
    );
    assert_scored(
        input(
            SubjectClarity::Clear,
            EyeRatingState::Failed,
            ExpressionRatingState::Outstanding,
            ValidationState::Passed,
            ValidationState::Passed,
        ),
        1,
        1,
    );
    assert_scored(
        input(
            SubjectClarity::Clear,
            EyeRatingState::UnableToDetermine,
            ExpressionRatingState::Natural,
            ValidationState::NotPassed,
            ValidationState::Passed,
        ),
        0,
        0,
    );
    assert_scored(
        input(
            SubjectClarity::Clear,
            EyeRatingState::Failed,
            ExpressionRatingState::SevereFailure,
            ValidationState::NotPassed,
            ValidationState::NotPassed,
        ),
        -4,
        0,
    );
}

#[test]
fn unclear_subject_short_circuits_every_other_state_to_zero() {
    for eyes in EYE_STATES {
        for expression in EXPRESSION_STATES {
            for optical in VALIDATION_STATES {
                for aesthetic_composition in VALIDATION_STATES {
                    let decision = calculate_portrait_rating(input(
                        SubjectClarity::Unclear,
                        eyes,
                        expression,
                        optical,
                        aesthetic_composition,
                    ));
                    assert_eq!(decision, PortraitRatingDecision::SubjectUnclear);
                    assert_eq!(decision.final_rating(), 0);
                }
            }
        }
    }
}

#[test]
fn every_clear_combination_uses_exact_increments_and_clamps_to_zero_through_five() {
    for eyes in EYE_STATES {
        for expression in EXPRESSION_STATES {
            for optical in VALIDATION_STATES {
                for aesthetic_composition in VALIDATION_STATES {
                    let decision = calculate_portrait_rating(input(
                        SubjectClarity::Clear,
                        eyes,
                        expression,
                        optical,
                        aesthetic_composition,
                    ));
                    let PortraitRatingDecision::Scored(breakdown) = decision else {
                        panic!("clear subject must produce a scored decision");
                    };
                    let expected_optical_aesthetic = i8::from(
                        optical == ValidationState::Passed
                            && aesthetic_composition == ValidationState::Passed,
                    );
                    let expected_raw =
                        eyes.increment() + expression.increment() + expected_optical_aesthetic;

                    assert_eq!(breakdown.eye_increment, eyes.increment());
                    assert_eq!(breakdown.expression_increment, expression.increment());
                    assert_eq!(
                        breakdown.optical_aesthetic_increment,
                        expected_optical_aesthetic
                    );
                    assert_eq!(breakdown.raw_rating, expected_raw);
                    assert_eq!(breakdown.final_rating, expected_raw.clamp(0, 5) as u8);
                    assert!(breakdown.final_rating <= 5);
                }
            }
        }
    }
}

#[test]
fn unable_states_are_zero_increments_without_becoming_failures() {
    let PortraitRatingDecision::Scored(breakdown) = calculate_portrait_rating(input(
        SubjectClarity::Clear,
        EyeRatingState::UnableToDetermine,
        ExpressionRatingState::UnableToDetermine,
        ValidationState::Passed,
        ValidationState::Passed,
    )) else {
        panic!("clear subject must produce a scored decision");
    };

    assert_eq!(breakdown.eye_increment, 0);
    assert_eq!(breakdown.expression_increment, 0);
    assert_eq!(breakdown.optical_aesthetic_increment, 1);
    assert_eq!(breakdown.final_rating, 1);
}

#[test]
fn optical_and_aesthetic_are_one_joint_increment() {
    for optical in VALIDATION_STATES {
        for aesthetic_composition in VALIDATION_STATES {
            let PortraitRatingDecision::Scored(breakdown) = calculate_portrait_rating(input(
                SubjectClarity::Clear,
                EyeRatingState::UnableToDetermine,
                ExpressionRatingState::Natural,
                optical,
                aesthetic_composition,
            )) else {
                panic!("clear subject must produce a scored decision");
            };

            let both_passed = optical == ValidationState::Passed
                && aesthetic_composition == ValidationState::Passed;
            assert_eq!(breakdown.optical_aesthetic_increment, i8::from(both_passed));
        }
    }
}

#[test]
fn policy_version_is_frozen() {
    assert_eq!(
        PORTRAIT_RATING_POLICY_VERSION,
        "qraw-portrait-rating-discrete-1.0"
    );
}
