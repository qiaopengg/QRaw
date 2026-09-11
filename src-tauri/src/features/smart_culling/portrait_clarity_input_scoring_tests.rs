//! Regressions for unavailable native inputs reaching the real portrait scorer.

use super::{candidate, evaluate_mode, face};
use crate::features::smart_culling::{
    infrastructure::{ClarityInput, ClarityInputUnavailable},
    portrait_quality,
    scoring::{AnalysisCandidate, organize_results},
};
use image::{DynamicImage, ImageFormat};
use serde_json::json;
use std::{io::Cursor, path::Path, sync::atomic::AtomicBool};

fn attach_input(item: &mut AnalysisCandidate, input: &ClarityInput) {
    item.portrait_quality = Some(
        portrait_quality::observe_with_clarity(
            &DynamicImage::new_rgb8(100, 100),
            input,
            &item.faces,
            &AtomicBool::new(false),
        )
        .unwrap(),
    );
}

fn original_bytes() -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    DynamicImage::new_rgb8(100, 100)
        .write_to(&mut bytes, ImageFormat::Png)
        .unwrap();
    bytes.into_inner()
}

#[test]
fn unavailable_native_inputs_never_produce_valid_portrait_stars() {
    use ClarityInputUnavailable::*;
    for reason in [
        NotRequested,
        RawSourceUnvalidated,
        ContentEdited,
        GeometryUnvalidated,
        InvalidAdjustments,
        InvalidCrop,
        DecodeFailed,
        RenderDimensionsMismatch,
    ] {
        for focus in [0.0, 300.0] {
            let mut item = candidate(vec![face(20.0, 40.0, "open", "outstanding")]);
            item.sharpness_metric = focus;
            item.faces[0].sharpness_metric = focus;
            assert!(!evaluate_mode("portrait", &item).requires_human_review);
            attach_input(&mut item, &ClarityInput::unavailable(reason));
            let evaluation = evaluate_mode("portrait", &item);
            assert!(
                evaluation.requires_human_review,
                "{reason:?}, focus={focus}"
            );
            assert_eq!(evaluation.rating, 0);
            assert_eq!(evaluation.confidence, 0.0);
            assert_eq!(evaluation.reason_code, "portrait_person_clarity_unresolved");
        }
    }
}

#[test]
fn same_size_unvalidated_rotation_reaches_review_with_original_reason() {
    let input = ClarityInput::from_bytes(&original_bytes(), false, &json!({"rotation": 15}));
    assert_eq!(
        input.unavailable_reason(),
        Some(ClarityInputUnavailable::GeometryUnvalidated)
    );
    let mut item = candidate(vec![face(20.0, 40.0, "open", "outstanding")]);
    attach_input(&mut item, &input);
    let result = organize_results(Path::new("."), "portrait", vec![item]).remove(0);
    assert!(result.requires_human_review);
    assert_eq!(
        result.rating, 0,
        "zero is an unrated placeholder, not a discard"
    );
    assert!(
        result
            .reason_codes
            .iter()
            .any(|reason| reason == "portrait_person_clarity_unresolved")
    );
    let review = serde_json::to_value(result).unwrap();
    assert_eq!(
        review["portraitQuality"]["clarity"]["unavailableReason"],
        "geometryUnvalidated"
    );
}

#[test]
fn supported_native_geometry_preserves_the_existing_rating() {
    for adjustments in [
        json!(null),
        json!({}),
        json!({"orientationSteps": 1, "flipHorizontal": true}),
    ] {
        let input = ClarityInput::from_bytes(&original_bytes(), false, &adjustments);
        assert_eq!(input.unavailable_reason(), None);
        for focus in [0.0, 300.0] {
            let mut item = candidate(vec![face(20.0, 40.0, "open", "outstanding")]);
            item.sharpness_metric = focus;
            item.faces[0].sharpness_metric = focus;
            let before = evaluate_mode("portrait", &item);
            attach_input(&mut item, &input);
            let after = evaluate_mode("portrait", &item);
            assert_eq!(after.rating, before.rating);
            assert_eq!(after.requires_human_review, before.requires_human_review);
            assert_eq!(after.reason_code, before.reason_code);
        }
    }
}

#[test]
fn original_input_restriction_does_not_change_other_modes() {
    for mode in ["auto", "environment", "group", "landscape"] {
        let mut item = candidate(vec![face(20.0, 40.0, "open", "outstanding")]);
        let before = evaluate_mode(mode, &item);
        attach_input(
            &mut item,
            &ClarityInput::unavailable(ClarityInputUnavailable::RawSourceUnvalidated),
        );
        let after = evaluate_mode(mode, &item);
        assert_eq!(after.rating, before.rating, "{mode}");
        assert_eq!(
            after.requires_human_review, before.requires_human_review,
            "{mode}"
        );
        assert_eq!(after.reason_code, before.reason_code, "{mode}");
        assert_eq!(after.score, before.score, "{mode}");
    }
}
