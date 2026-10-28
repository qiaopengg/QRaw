//! Adversarial input-status propagation; no new clarity measurements or thresholds.

use super::{candidate, confirmed_key, evaluate_mode, face};
use crate::features::smart_culling::{
    infrastructure::ClarityInput,
    portrait_quality::{self, QualityUnavailableReason},
    review_policy::result_is_writable,
    scoring::{AnalysisCandidate, organize_results},
    types::FaceResult,
};
use image::{DynamicImage, ImageFormat};
use std::{io::Cursor, path::Path, sync::atomic::AtomicBool};

fn subject(bbox: [f32; 4]) -> FaceResult {
    let mut subject = face(bbox[0], bbox[2], "open", "outstanding");
    subject.bbox = bbox;
    subject
}

fn observed(faces: Vec<FaceResult>) -> AnalysisCandidate {
    let mut item = candidate(faces);
    let image = DynamicImage::new_rgb8(100, 100);
    let mut bytes = Cursor::new(Vec::new());
    image.write_to(&mut bytes, ImageFormat::Png).unwrap();
    let input = ClarityInput::from_bytes(bytes.get_ref(), false, &serde_json::json!(null));
    item.portrait_quality = Some(
        portrait_quality::observe_with_clarity(
            &image,
            &input,
            &item.faces,
            &AtomicBool::new(false),
        )
        .unwrap(),
    );
    // Keep the existing fixed scoring fixture. These tests challenge refusal
    // propagation, not whether this image or its sharpness is actually clear.
    item
}

fn assert_unrated(item: AnalysisCandidate) {
    let result = organize_results(Path::new("."), "portrait", vec![item]).remove(0);
    assert!(
        result.requires_human_review,
        "refused input must remain unrated"
    );
    assert!(!result_is_writable(&result));
    let json = serde_json::to_value(result).unwrap();
    assert_eq!(json["portraitRating"]["status"], "pending");
    assert!(json["portraitRating"]["breakdown"].is_null());
}

#[test]
fn selected_face_input_refusals_cannot_be_overridden_by_legacy_focus() {
    for (bbox, reason) in [
        (
            [85.0, 10.0, 40.0, 40.0],
            QualityUnavailableReason::PartialFace,
        ),
        (
            [20.0, 10.0, 20.0, 20.0],
            QualityUnavailableReason::FaceTooSmall,
        ),
        (
            [110.0, 10.0, 40.0, 40.0],
            QualityUnavailableReason::InvalidFaceBox,
        ),
    ] {
        let item = observed(vec![subject(bbox)]);
        let native = item
            .portrait_quality
            .as_ref()
            .unwrap()
            .clarity
            .as_ref()
            .unwrap();
        assert!(native.unavailable_reason.is_none());
        assert_eq!(native.faces[0].unavailable_reason, Some(reason));
        assert_unrated(item);
    }
}

#[test]
fn selected_face_observation_must_exist_match_and_be_unique() {
    for corruption in [
        "missing",
        "duplicate",
        "wrong_index",
        "wrong_box",
        "wrong_dimensions",
    ] {
        let mut item = observed(vec![subject([10.0, 10.0, 40.0, 40.0])]);
        assert!(!evaluate_mode("portrait", &item).requires_human_review);
        let native = item
            .portrait_quality
            .as_mut()
            .unwrap()
            .clarity
            .as_mut()
            .unwrap();
        match corruption {
            "missing" => native.faces.clear(),
            "duplicate" => native.faces.push(native.faces[0].clone()),
            "wrong_index" => native.faces[0].face_index = 7,
            "wrong_box" => native.faces[0].bbox[0] += 1.0,
            "wrong_dimensions" => native.input_dimensions = Some([99, 100]),
            _ => unreachable!(),
        }
        assert_unrated(item);
    }
}

#[test]
fn native_contract_cannot_disappear_from_a_new_observation() {
    let mut item = observed(vec![subject([10.0, 10.0, 40.0, 40.0])]);
    item.portrait_quality.as_mut().unwrap().clarity = None;
    assert_unrated(item);
}

#[test]
fn unselected_face_refusal_does_not_discard_a_valid_selected_subject() {
    let item = observed(vec![
        subject([10.0, 10.0, 40.0, 40.0]),
        subject([70.0, 10.0, 20.0, 20.0]),
    ]);
    let native = item
        .portrait_quality
        .as_ref()
        .unwrap()
        .clarity
        .as_ref()
        .unwrap();
    assert!(native.faces[0].unavailable_reason.is_none());
    assert_eq!(
        native.faces[1].unavailable_reason,
        Some(QualityUnavailableReason::FaceTooSmall)
    );
    let result = evaluate_mode("portrait", &item);
    assert!(!result.requires_human_review);
    assert_eq!(result.rating, 4);
}

#[test]
fn selected_key_person_is_checked_even_when_another_face_has_valid_input() {
    let mut item = observed(vec![
        subject([10.0, 10.0, 40.0, 40.0]),
        subject([70.0, 10.0, 20.0, 20.0]),
    ]);
    item.key_person_evidence = vec![confirmed_key(1, 1)];
    assert_unrated(item);
}
