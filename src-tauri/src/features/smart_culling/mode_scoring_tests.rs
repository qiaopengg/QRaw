use image_hasher::ImageHash;

use super::*;
use crate::features::smart_culling::expression::EXPRESSION_QUALITY_REASON;
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
        expression_score: match expression_state {
            "usable" => Some(1.0),
            "unusable" => Some(0.0),
            _ => None,
        },
        expression_confidence: if matches!(expression_state, "usable" | "unusable") {
            0.4
        } else {
            0.0
        },
        expression_reason: if matches!(expression_state, "usable" | "unusable") {
            EXPRESSION_QUALITY_REASON.to_string()
        } else {
            "expression_test".to_string()
        },
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
        #[cfg(all(debug_assertions, target_os = "macos"))]
        vision_quality: Default::default(),
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
    let single = evaluate_mode("auto", &candidate(vec![face(5.0, 40.0, "open", "usable")]));
    let multiple = evaluate_mode(
        "auto",
        &candidate(vec![
            face(5.0, 40.0, "open", "usable"),
            face(55.0, 40.0, "open", "usable"),
        ]),
    );

    assert_eq!(single.resolved_mode, "auto");
    assert_eq!(multiple.resolved_mode, "auto");
    assert!((single.score - multiple.score).abs() < f64::EPSILON);
}

#[test]
fn weak_person_evidence_in_auto_scene_is_sent_to_manual_review() {
    let mut weak = face(5.0, 40.0, "open", "usable");
    weak.detection_score = 0.50;

    let evaluation = evaluate_mode("auto", &candidate(vec![weak]));

    assert_eq!(evaluation.resolved_mode, "auto");
    assert!(evaluation.requires_human_review);
    assert_eq!(evaluation.reason_code, "auto_people_uncertain");
}

#[cfg(all(debug_assertions, target_os = "macos"))]
#[test]
fn vision_human_observation_at_threshold_changes_only_the_review_outcome() {
    let baseline = evaluate_mode("auto", &candidate(Vec::new()));
    let mut item = candidate(Vec::new());
    item.vision_quality.human_count = 1;
    item.vision_quality.max_human_confidence = Some(0.50);

    let evaluation = evaluate_mode("auto", &item);

    assert!(evaluation.requires_human_review);
    assert_eq!(evaluation.reason_code, "auto_people_uncertain");
    assert_eq!(evaluation.resolved_mode, baseline.resolved_mode);
    assert_eq!(evaluation.score, baseline.score);
    assert_eq!(evaluation.confidence, baseline.confidence);
    assert_eq!(
        rating_for_mode(&evaluation.resolved_mode, evaluation.score),
        rating_for_mode(&baseline.resolved_mode, baseline.score)
    );
}

#[cfg(all(debug_assertions, target_os = "macos"))]
#[test]
fn low_confidence_vision_human_observation_does_not_change_auto_scene() {
    let baseline = evaluate_mode("auto", &candidate(Vec::new()));
    let mut item = candidate(Vec::new());
    item.vision_quality.human_count = 1;
    item.vision_quality.max_human_confidence = Some(0.49);

    let evaluation = evaluate_mode("auto", &item);

    assert_eq!(evaluation.resolved_mode, baseline.resolved_mode);
    assert_eq!(evaluation.score, baseline.score);
    assert_eq!(evaluation.confidence, baseline.confidence);
    assert_eq!(
        evaluation.requires_human_review,
        baseline.requires_human_review
    );
    assert_eq!(evaluation.reason_code, baseline.reason_code);
}

#[cfg(all(debug_assertions, target_os = "macos"))]
#[test]
fn vision_face_capture_quality_changes_only_weighted_person_clarity() {
    let mut low = candidate(vec![face(5.0, 40.0, "open", "usable")]);
    low.vision_quality.face_capture_qualities = vec![Some(0.20)];
    let mut high = candidate(vec![face(5.0, 40.0, "open", "usable")]);
    high.vision_quality.face_capture_qualities = vec![Some(0.80)];

    let low_evaluation = evaluate_mode("auto", &low);
    let high_evaluation = evaluate_mode("auto", &high);

    assert!(high_evaluation.score > low_evaluation.score);
    assert!((high_evaluation.score - low_evaluation.score - 0.30).abs() < 1e-6);
    assert_eq!(high_evaluation.confidence, low_evaluation.confidence);
}

#[cfg(all(debug_assertions, target_os = "macos"))]
#[test]
fn vision_face_capture_quality_cannot_change_the_frozen_closed_eye_gate() {
    let mut low = candidate(vec![face(5.0, 40.0, "closed", "usable")]);
    low.vision_quality.face_capture_qualities = vec![Some(0.05)];
    let mut high = candidate(vec![face(5.0, 40.0, "closed", "usable")]);
    high.vision_quality.face_capture_qualities = vec![Some(0.95)];

    let low_evaluation = evaluate_mode("portrait", &low);
    let high_evaluation = evaluate_mode("portrait", &high);

    assert_eq!(low_evaluation.score, 0.0);
    assert_eq!(high_evaluation.score, 0.0);
    assert_eq!(low_evaluation.reason_code, "portrait_closed_eyes");
    assert_eq!(high_evaluation.reason_code, "portrait_closed_eyes");
    assert_eq!(low_evaluation.confidence, high_evaluation.confidence);
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
        face(65.0, 25.0, "open", "usable"),
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
fn every_selected_person_must_complete_expression_before_group_combination() {
    let mut unresolved = candidate(vec![
        face(5.0, 40.0, "open", "usable"),
        face(55.0, 40.0, "open", "unknown"),
    ]);
    unresolved.key_person_evidence = vec![confirmed_key(1, 0), confirmed_key(2, 1)];

    let unresolved_evaluation = evaluate_mode("group", &unresolved);
    assert_eq!(unresolved_evaluation.score, 0.0);
    assert!(unresolved_evaluation.requires_human_review);
    assert_eq!(
        unresolved_evaluation.reason_code,
        "group_expression_unresolved"
    );

    unresolved.faces[1].expression_state = "unusable".to_string();
    unresolved.faces[1].expression_score = Some(0.0);
    unresolved.faces[1].expression_confidence = 0.4;
    let low_quality_evaluation = evaluate_mode("group", &unresolved);
    assert!(low_quality_evaluation.score > 0.0);
    assert_ne!(
        low_quality_evaluation.reason_code,
        "group_expression_unresolved"
    );
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

    let mut portrait = candidate(vec![face(5.0, 40.0, "open", "usable")]);
    portrait.sharpness_metric = 2.0;
    portrait.faces[0].sharpness_metric = 2.0;
    let portrait_evaluation = evaluate_mode("portrait", &portrait);
    assert_eq!(rating_for_mode("portrait", portrait_evaluation.score), 1);
    assert!(!portrait_evaluation.requires_human_review);
    assert_eq!(portrait_evaluation.reason_code, "portrait_person_unclear");
}

#[test]
fn uncertain_person_clarity_continues_to_later_evidence() {
    let mut item = candidate(vec![face(5.0, 40.0, "open", "usable")]);
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
        &candidate(vec![face(5.0, 40.0, "closed", "usable")]),
    );
    assert_eq!(rating_for_mode("portrait", portrait.score), 1);
    assert!(!portrait.requires_human_review);

    let mut key_group_item = candidate(vec![face(5.0, 40.0, "closed", "usable")]);
    key_group_item.key_person_evidence = vec![confirmed_key(1, 0)];
    let key_group = evaluate_mode("group", &key_group_item);
    assert_eq!(rating_for_mode("group", key_group.score), 1);
    assert!(!key_group.requires_human_review);

    for mode in ["environment", "auto"] {
        let weighted = evaluate_mode(mode, &candidate(vec![face(5.0, 40.0, "closed", "usable")]));
        assert!(rating_for_mode(mode, weighted.score) > 1);
    }
}

#[test]
fn deliberate_eye_pose_never_exceeds_three_stars_in_portrait() {
    let evaluation = evaluate_mode(
        "portrait",
        &candidate(vec![face(5.0, 40.0, "deliberate", "usable")]),
    );

    assert_eq!(rating_for_mode("portrait", evaluation.score), 3);
    assert_eq!(evaluation.reason_code, "portrait_deliberate_eye_pose");
}

#[test]
fn deliberate_pose_cannot_hide_another_selected_subjects_unknown_eyes() {
    let mut item = candidate(vec![
        face(5.0, 40.0, "deliberate", "usable"),
        face(55.0, 40.0, "unknown", "usable"),
    ]);
    item.key_person_evidence = vec![confirmed_key(1, 0), confirmed_key(2, 1)];

    let evaluation = evaluate_mode("group", &item);

    assert!(evaluation.requires_human_review);
    assert_eq!(evaluation.reason_code, "group_key_evidence_interval_review");
}

#[test]
fn technical_sequence_state_cannot_bypass_expression_quality_gate() {
    let stable = evaluate_mode(
        "portrait",
        &candidate(vec![face(5.0, 40.0, "open", "stable")]),
    );
    let transitional = evaluate_mode(
        "portrait",
        &candidate(vec![face(5.0, 40.0, "open", "transitional")]),
    );

    assert_eq!(stable.score, 0.0);
    assert_eq!(transitional.score, 0.0);
    assert!(stable.requires_human_review);
    assert!(transitional.requires_human_review);
    assert_eq!(stable.reason_code, "portrait_expression_unresolved");
    assert_eq!(transitional.reason_code, "portrait_expression_unresolved");
}

#[test]
fn synthetic_single_frame_expression_is_evaluated_before_combination() {
    let usable = evaluate_mode(
        "portrait",
        &candidate(vec![face(5.0, 40.0, "open", "usable")]),
    );
    let low_quality = evaluate_mode(
        "portrait",
        &candidate(vec![face(5.0, 40.0, "open", "unusable")]),
    );

    assert!(usable.score > low_quality.score);
    assert!(low_quality.score > 0.0);
    assert_ne!(low_quality.reason_code, "portrait_expression_unresolved");
}

#[test]
fn synthetic_expression_cannot_replace_frozen_closed_eye_one_star() {
    let portrait = evaluate_mode(
        "portrait",
        &candidate(vec![face(5.0, 40.0, "closed", "unusable")]),
    );
    assert_eq!(rating_for_mode("portrait", portrait.score), 1);
    assert!(!portrait.requires_human_review);
    assert_eq!(portrait.reason_code, "portrait_closed_eyes");

    let mut group_item = candidate(vec![face(5.0, 40.0, "closed", "unusable")]);
    group_item.key_person_evidence = vec![confirmed_key(1, 0)];
    let group = evaluate_mode("group", &group_item);
    assert_eq!(rating_for_mode("group", group.score), 1);
    assert!(!group.requires_human_review);
    assert_eq!(group.reason_code, "group_closed_eyes");
}

#[test]
fn unknown_expression_stops_before_weighted_combination() {
    let usable = evaluate_mode(
        "portrait",
        &candidate(vec![face(5.0, 40.0, "open", "usable")]),
    );
    let unknown = evaluate_mode(
        "portrait",
        &candidate(vec![face(5.0, 40.0, "open", "unknown")]),
    );

    assert!(usable.score > 0.0);
    assert_eq!(unknown.score, 0.0);
    assert!(unknown.requires_human_review);
    assert_eq!(unknown.reason_code, "portrait_expression_unresolved");
}

#[test]
fn environmental_portrait_without_a_subject_does_not_fall_back_to_landscape() {
    let evaluation = evaluate_mode("environment", &candidate(Vec::new()));

    assert_eq!(evaluation.resolved_mode, "environment");
    assert!(evaluation.requires_human_review);
    assert_eq!(evaluation.reason_code, "environment_subject_unreliable");
}
