//! End-to-end contracts for the portrait review panel, using the real scorer.
use super::{candidate, confirmed_key, face};
use crate::features::smart_culling::{
    api::ReviewChange, review_policy::apply_review_changes, scoring::organize_results,
};
use serde_json::{Value, json};
use std::path::Path;

#[test]
fn review_exposes_the_actual_discrete_breakdown_for_every_eye_expression_pair() {
    for (eyes, eye_increment) in [("open", 2), ("closed", -2), ("unknown", 0)] {
        for (expression, expression_increment) in [
            ("severe_failure", -2),
            ("not_recommended", -1),
            ("natural", 0),
            ("excellent", 1),
            ("outstanding", 2),
            ("unknown", 0),
        ] {
            let result = organize_results(
                Path::new("."),
                "portrait",
                vec![candidate(vec![face(5.0, 40.0, eyes, expression)])],
            )
            .remove(0);
            let raw: i8 = eye_increment + expression_increment;
            assert_eq!(result.rating, raw.clamp(0, 5) as u8);
            let evidence = &serde_json::to_value(&result).unwrap()["portraitRating"];
            assert_eq!(evidence["status"], "scored");
            assert_eq!(evidence["subjectFaceIndices"], json!([0]));
            assert_eq!(evidence["breakdown"]["eyeIncrement"], eye_increment);
            assert_eq!(
                evidence["breakdown"]["expressionIncrement"],
                expression_increment
            );
            assert_eq!(evidence["breakdown"]["opticalAestheticIncrement"], 0);
            assert_eq!(evidence["breakdown"]["rawRating"], raw);
            assert_eq!(evidence["breakdown"]["finalRating"], result.rating);
            assert_eq!(evidence["opticalAesthetic"]["optical"], "unableToDetermine");
        }
    }
}

#[test]
fn unresolved_clarity_keeps_eye_expression_results_without_inventing_a_total() {
    let mut item = candidate(vec![face(5.0, 40.0, "open", "excellent")]);
    item.faces[0].sharpness_metric = 3.0;
    let result = organize_results(Path::new("."), "portrait", vec![item]).remove(0);
    assert!(result.requires_human_review);
    let value = serde_json::to_value(result).unwrap();
    assert_eq!(value["portraitRating"]["status"], "pending");
    assert_eq!(value["portraitRating"]["eyes"], "passed");
    assert_eq!(value["portraitRating"]["expression"], "excellent");
    assert_eq!(value["portraitRating"]["breakdown"], Value::Null);
}

#[test]
fn failed_gate_exposes_no_increments_and_confirmed_subjects_match_the_scorer() {
    let mut item = candidate(vec![face(5.0, 40.0, "open", "outstanding")]);
    item.sharpness_metric = 2.0;
    item.faces[0].sharpness_metric = 2.0;
    let result = organize_results(Path::new("."), "portrait", vec![item]).remove(0);
    let value = serde_json::to_value(result).unwrap();
    assert_eq!(value["portraitRating"]["status"], "subjectUnclear");
    assert_eq!(value["portraitRating"]["breakdown"], Value::Null);

    let mut item = candidate(vec![
        face(5.0, 40.0, "closed", "severe_failure"),
        face(55.0, 25.0, "open", "outstanding"),
    ]);
    item.key_person_evidence = vec![confirmed_key(1, 1)];
    let result = organize_results(Path::new("."), "portrait", vec![item]).remove(0);
    assert_eq!(result.rating, 4);
    let value = serde_json::to_value(result).unwrap();
    assert_eq!(value["portraitRating"]["subjectFaceIndices"], json!([1]));
    assert_eq!(value["portraitRating"]["eyes"], "passed");
}

#[test]
fn unresolved_subjects_and_other_modes_never_borrow_another_persons_rating() {
    let mut item = candidate(vec![face(5.0, 40.0, "open", "outstanding")]);
    item.key_person_evidence = vec![confirmed_key(1, 0)];
    item.key_person_evidence[0].status = "suspected".into();
    let result = organize_results(Path::new("."), "portrait", vec![item]).remove(0);
    let value = serde_json::to_value(result).unwrap();
    assert_eq!(value["portraitRating"]["subjectFaceIndices"], json!([]));
    assert_eq!(value["portraitRating"]["eyes"], "unableToDetermine");
    assert_eq!(value["portraitRating"]["status"], "pending");
    let result = organize_results(Path::new("."), "landscape", vec![candidate(vec![])]).remove(0);
    assert!(
        serde_json::to_value(result)
            .unwrap()
            .get("portraitRating")
            .is_none()
    );
}

#[test]
fn manual_rating_preserves_original_ai_evidence_without_overwriting_the_manual_decision() {
    let mut results = organize_results(
        Path::new("."),
        "portrait",
        vec![candidate(vec![face(5.0, 40.0, "open", "excellent")])],
    );
    let before = serde_json::to_value(&results[0]).unwrap()["portraitRating"].clone();
    assert_eq!(before["breakdown"]["finalRating"], 3);
    apply_review_changes(
        &mut results,
        vec![ReviewChange {
            result_id: "frame".into(),
            rating: 5,
            color_label: None,
        }],
    )
    .unwrap();
    assert_eq!(results[0].rating, 5);
    assert_eq!(results[0].source, "manual");
    assert!(results[0].protected);
    assert!(!results[0].requires_human_review);
    assert_eq!(
        serde_json::to_value(&results[0]).unwrap()["portraitRating"],
        before
    );
}

#[test]
fn review_only_group_never_exposes_a_valid_total_from_its_individual_mode_evaluation() {
    let mut first = candidate(vec![face(5.0, 40.0, "open", "excellent")]);
    let mut second = candidate(vec![face(5.0, 40.0, "open", "excellent")]);
    first.result_id = "first".into();
    first.path = "first.jpg".into();
    first.capture_time_millis = 1_000;
    first.sequence_number = Some(1);
    first.exposure_metric = 0.9;
    second.result_id = "second".into();
    second.path = "second.jpg".into();
    second.capture_time_millis = 1_100;
    second.sequence_number = Some(2);
    second.exposure_metric = 0.5;
    let results = organize_results(Path::new("."), "portrait", vec![first, second]);
    assert_eq!(results.len(), 2);
    for result in results {
        assert_eq!(result.group_kind, "reviewOnly");
        assert!(result.requires_human_review);
        let value = serde_json::to_value(result).unwrap();
        assert_eq!(value["portraitRating"]["status"], "pending");
        assert_eq!(value["portraitRating"]["expression"], "excellent");
        assert_eq!(value["portraitRating"]["breakdown"], Value::Null);
    }
}
