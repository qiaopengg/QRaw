use image_hasher::ImageHash;

use super::*;
use crate::features::smart_culling::types::{EyeResult, KeyPersonEvidence};

fn eye(state: &str) -> EyeResult {
    EyeResult {
        open_probability: None,
        state: state.to_string(),
        confidence: if state == "unknown" { 0.0 } else { 0.9 },
        reason: format!("eye_{state}_test"),
        effective_pixels: 100,
        sharpness_metric: Some(100.0),
    }
}

fn face(x: f32, width: f32, eye_state: &str, expression_state: &str) -> FaceResult {
    FaceResult {
        bbox: [x, 10.0, width, 30.0],
        landmarks: [(0.0, 0.0); 5],
        detection_score: 0.95,
        left_eye: eye(eye_state),
        right_eye: eye(eye_state),
        eye_disposition: match eye_state {
            "open" => EyeDisposition::Open,
            "closed" => EyeDisposition::Unusable,
            "deliberate" => EyeDisposition::DeliberatePoseCandidate,
            _ => EyeDisposition::Unknown,
        },
        expression_state: expression_state.to_string(),
        expression_confidence: if expression_state == "unknown" {
            0.0
        } else {
            0.9
        },
        expression_reason: "expression_test".to_string(),
        expression_descriptor: None,
        sharpness_metric: 300.0,
        sharpness_confidence: 0.9,
        exposure_metric: 0.8,
        exposure_confidence: 0.9,
        identity_embedding: None,
    }
}

fn candidate(faces: Vec<FaceResult>) -> AnalysisCandidate {
    AnalysisCandidate {
        result_id: "frame".to_string(),
        path: "frame.jpg".into(),
        member_paths: Vec::new(),
        hash: ImageHash::from_bytes(&[0; 32]).unwrap(),
        capture_time_millis: 0,
        capture_time_from_exif: true,
        sequence_number: None,
        sharpness_metric: 300.0,
        center_focus_metric: 250.0,
        exposure_metric: 0.8,
        width: 100,
        height: 100,
        faces,
        key_person_evidence: Vec::new(),
    }
}

fn confirmed_key(priority: usize, face_index: usize) -> KeyPersonEvidence {
    KeyPersonEvidence {
        priority,
        face_index: Some(face_index),
        similarity: Some(0.95),
        status: "confirmed".to_string(),
        auto_score_eligible: true,
        performance_rank: None,
    }
}

#[test]
fn auto_people_strategy_does_not_change_with_people_count() {
    let single = evaluate_mode("auto", &candidate(vec![face(5.0, 40.0, "open", "stable")]));
    let multiple = evaluate_mode(
        "auto",
        &candidate(vec![
            face(5.0, 40.0, "open", "stable"),
            face(55.0, 40.0, "open", "stable"),
        ]),
    );

    assert_eq!(single.resolved_mode, "auto");
    assert_eq!(multiple.resolved_mode, "auto");
    assert!((single.score - multiple.score).abs() < f64::EPSILON);
}

#[test]
fn weak_person_evidence_in_auto_scene_is_sent_to_manual_review() {
    let mut weak = face(5.0, 40.0, "open", "stable");
    weak.detection_score = 0.50;

    let evaluation = evaluate_mode("auto", &candidate(vec![weak]));

    assert_eq!(evaluation.resolved_mode, "auto");
    assert!(evaluation.requires_human_review);
    assert_eq!(evaluation.reason_code, "auto_people_uncertain");
}

#[test]
fn group_without_keys_uses_scene_evidence_and_ignores_every_face_signal() {
    let open = evaluate_mode("group", &candidate(vec![face(5.0, 40.0, "open", "stable")]));
    let closed = evaluate_mode(
        "group",
        &candidate(vec![face(5.0, 40.0, "closed", "transitional")]),
    );

    assert_eq!(open.resolved_mode, "group");
    assert_eq!(open.score, closed.score);
    assert_eq!(open.requires_human_review, closed.requires_human_review);
}

#[test]
fn portrait_and_group_key_modes_score_only_confirmed_selected_faces() {
    let mut item = candidate(vec![
        face(5.0, 50.0, "closed", "transitional"),
        face(65.0, 25.0, "open", "stable"),
    ]);
    item.key_person_evidence = vec![confirmed_key(1, 1)];

    for mode in ["portrait", "group"] {
        let evaluation = evaluate_mode(mode, &item);
        assert_eq!(rating_for_mode(mode, evaluation.score), 5);
        assert!(!evaluation.requires_human_review);
        assert!(!evaluation.reason_code.contains("closed_eyes"));
    }
}

#[test]
fn unresolved_key_identity_stays_zero_star_manual_review() {
    let mut item = candidate(vec![face(5.0, 40.0, "open", "stable")]);
    item.key_person_evidence = vec![KeyPersonEvidence {
        priority: 1,
        face_index: Some(0),
        similarity: Some(0.75),
        status: "suspected".to_string(),
        auto_score_eligible: false,
        performance_rank: None,
    }];

    let evaluation = evaluate_mode("group", &item);

    assert_eq!(evaluation.score, 0.0);
    assert!(evaluation.requires_human_review);
    assert_eq!(evaluation.reason_code, "group_key_person_unresolved");
}

#[test]
fn image_and_person_definite_blur_gates_short_circuit_to_one_star() {
    let mut scene = candidate(Vec::new());
    scene.sharpness_metric = 2.0;
    scene.center_focus_metric = 2.0;
    let scene_evaluation = evaluate_mode("group", &scene);
    assert_eq!(rating_for_mode("group", scene_evaluation.score), 1);
    assert!(!scene_evaluation.requires_human_review);
    assert_eq!(scene_evaluation.reason_code, "group_image_unclear");

    let mut portrait = candidate(vec![face(5.0, 40.0, "open", "stable")]);
    portrait.sharpness_metric = 2.0;
    portrait.faces[0].sharpness_metric = 2.0;
    let portrait_evaluation = evaluate_mode("portrait", &portrait);
    assert_eq!(rating_for_mode("portrait", portrait_evaluation.score), 1);
    assert!(!portrait_evaluation.requires_human_review);
    assert_eq!(portrait_evaluation.reason_code, "portrait_person_unclear");
}

#[test]
fn uncertain_person_clarity_continues_to_later_evidence() {
    let mut item = candidate(vec![face(5.0, 40.0, "open", "stable")]);
    item.faces[0].sharpness_metric = 3.0;

    let evaluation = evaluate_mode("portrait", &item);

    assert!(evaluation.score > 0.0);
    assert_ne!(evaluation.reason_code, "portrait_person_unclear");
}

#[test]
fn score_interval_requires_review_when_missing_composition_crosses_tiers() {
    let evaluation = evaluate_mode("landscape", &candidate(Vec::new()));

    assert!(evaluation.requires_human_review);
    assert_eq!(evaluation.reason_code, "landscape_evidence_interval_review");
}

#[test]
fn ordinary_closed_eyes_are_hard_capped_only_in_portrait_and_key_group() {
    let portrait = evaluate_mode(
        "portrait",
        &candidate(vec![face(5.0, 40.0, "closed", "stable")]),
    );
    assert_eq!(rating_for_mode("portrait", portrait.score), 1);
    assert!(!portrait.requires_human_review);

    let mut key_group_item = candidate(vec![face(5.0, 40.0, "closed", "stable")]);
    key_group_item.key_person_evidence = vec![confirmed_key(1, 0)];
    let key_group = evaluate_mode("group", &key_group_item);
    assert_eq!(rating_for_mode("group", key_group.score), 1);
    assert!(!key_group.requires_human_review);

    for mode in ["environment", "auto"] {
        let weighted = evaluate_mode(mode, &candidate(vec![face(5.0, 40.0, "closed", "stable")]));
        assert!(rating_for_mode(mode, weighted.score) > 1);
    }
}

#[test]
fn deliberate_eye_pose_never_exceeds_three_stars_in_portrait() {
    let evaluation = evaluate_mode(
        "portrait",
        &candidate(vec![face(5.0, 40.0, "deliberate", "stable")]),
    );

    assert_eq!(rating_for_mode("portrait", evaluation.score), 3);
    assert_eq!(evaluation.reason_code, "portrait_deliberate_eye_pose");
}

#[test]
fn deliberate_pose_cannot_hide_another_selected_subjects_unknown_eyes() {
    let mut item = candidate(vec![
        face(5.0, 40.0, "deliberate", "stable"),
        face(55.0, 40.0, "unknown", "stable"),
    ]);
    item.key_person_evidence = vec![confirmed_key(1, 0), confirmed_key(2, 1)];

    let evaluation = evaluate_mode("group", &item);

    assert!(evaluation.requires_human_review);
    assert_eq!(evaluation.reason_code, "group_key_evidence_interval_review");
}

#[test]
fn expression_sequence_state_changes_the_weighted_score() {
    let stable = evaluate_mode(
        "portrait",
        &candidate(vec![face(5.0, 40.0, "open", "stable")]),
    );
    let transitional = evaluate_mode(
        "portrait",
        &candidate(vec![face(5.0, 40.0, "open", "transitional")]),
    );

    assert!(stable.score > transitional.score);
    assert_eq!(transitional.reason_code, "portrait_expression_transition");
}

#[test]
fn unknown_expression_uses_an_interval_instead_of_weight_renormalization() {
    let stable = evaluate_mode(
        "portrait",
        &candidate(vec![face(5.0, 40.0, "open", "stable")]),
    );
    let unknown = evaluate_mode(
        "portrait",
        &candidate(vec![face(5.0, 40.0, "open", "unknown")]),
    );

    assert!(unknown.score < stable.score);
    assert!(unknown.requires_human_review);
    assert_eq!(unknown.reason_code, "portrait_evidence_interval_review");
}

#[test]
fn environmental_portrait_without_a_subject_does_not_fall_back_to_landscape() {
    let evaluation = evaluate_mode("environment", &candidate(Vec::new()));

    assert_eq!(evaluation.resolved_mode, "environment");
    assert!(evaluation.requires_human_review);
    assert_eq!(evaluation.reason_code, "environment_subject_unreliable");
}
