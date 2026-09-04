use std::collections::HashSet;

use super::api::{ReviewChange, ReviewResult, SmartCullingCapabilities};
use super::coordinator_support::valid_color;

pub(crate) fn apply_review_changes(
    results: &mut [ReviewResult],
    changes: Vec<ReviewChange>,
) -> Result<(), String> {
    if changes.is_empty() {
        return Err("At least one review change is required".to_string());
    }

    let mut result_ids = HashSet::with_capacity(changes.len());
    let mut result_indices = Vec::with_capacity(changes.len());
    for change in &changes {
        if change.rating > 5 || !valid_color(change.color_label.as_deref()) {
            return Err("Review rating or color label is invalid".to_string());
        }
        if !result_ids.insert(change.result_id.as_str()) {
            return Err(format!(
                "Review change contains duplicate result id: {}",
                change.result_id
            ));
        }
        let index = results
            .iter()
            .position(|result| result.result_id == change.result_id)
            .ok_or_else(|| format!("Review result does not exist: {}", change.result_id))?;
        result_indices.push(index);
    }

    for (change, index) in changes.into_iter().zip(result_indices) {
        let result = &mut results[index];
        result.rating = change.rating;
        result.color_label = change.color_label;
        result.source = "manual".to_string();
        result.reason_codes.clear();
        result.confidence = 0.0;
        result.protected = true;
        result.requires_human_review = false;
    }
    Ok(())
}

pub(crate) fn result_is_writable(result: &ReviewResult) -> bool {
    if result.source == "manual" {
        return result.rating <= 5;
    }
    !result.requires_human_review && result.rating <= 5
}

pub(crate) fn requires_calibration_acknowledgement(
    capabilities: &SmartCullingCapabilities,
    writable_results: &[ReviewResult],
) -> bool {
    !capabilities.release_ready && writable_results.iter().any(|result| result.source == "ai")
}

#[cfg(test)]
mod tests {
    use super::{apply_review_changes, requires_calibration_acknowledgement, result_is_writable};
    use crate::features::smart_culling::api::{
        ReviewChange, ReviewResult, SmartCullingCapabilities,
    };

    fn result(source: &str, rating: u8, requires_human_review: bool) -> ReviewResult {
        ReviewResult {
            result_id: "result".to_string(),
            path: "/photo.jpg".to_string(),
            member_paths: vec!["/photo.jpg".to_string()],
            folder: "/".to_string(),
            group_id: "group".to_string(),
            group_kind: "single".to_string(),
            group_index: 1,
            group_rank: 1,
            group_size: 1,
            recommended_count: 1,
            rating,
            color_label: None,
            source: source.to_string(),
            mode: "portrait".to_string(),
            reason_codes: Vec::new(),
            confidence: 0.0,
            protected: source == "manual",
            requires_human_review,
            width: 1,
            height: 1,
            faces: Vec::new(),
            key_person_evidence: Vec::new(),
        }
    }

    fn change(result_id: &str, rating: u8) -> ReviewChange {
        ReviewChange {
            result_id: result_id.to_string(),
            rating,
            color_label: None,
        }
    }

    #[test]
    fn writes_all_valid_ai_ratings_without_a_pool_gate() {
        assert!(result_is_writable(&result("ai", 0, false)));
        assert!(result_is_writable(&result("ai", 1, false)));
        assert!(result_is_writable(&result("ai", 5, false)));
    }

    #[test]
    fn keeps_unresolved_ai_results_out_of_the_write_queue() {
        assert!(!result_is_writable(&result("ai", 0, true)));
        assert!(!result_is_writable(&result("ai", 4, true)));
    }

    #[test]
    fn preserves_a_user_cleared_zero_star_manual_result() {
        assert!(result_is_writable(&result("manual", 0, false)));
    }

    #[test]
    fn calibration_acknowledgement_is_required_only_for_ai_writes() {
        let limited = SmartCullingCapabilities::default();
        let mut released = SmartCullingCapabilities::default();
        released.release_ready = true;

        assert!(requires_calibration_acknowledgement(
            &limited,
            &[result("ai", 4, false)]
        ));
        assert!(!requires_calibration_acknowledgement(
            &limited,
            &[result("manual", 4, false)]
        ));
        assert!(!requires_calibration_acknowledgement(
            &released,
            &[result("ai", 4, false)]
        ));
    }

    #[test]
    fn applies_a_valid_batch_after_every_change_is_validated() {
        let mut first = result("ai", 2, true);
        first.result_id = "first".to_string();
        let mut second = result("ai", 3, true);
        second.result_id = "second".to_string();
        let mut results = vec![first, second];

        apply_review_changes(&mut results, vec![change("first", 4), change("second", 5)]).unwrap();

        assert_eq!(results[0].rating, 4);
        assert_eq!(results[1].rating, 5);
        assert!(results.iter().all(|result| {
            result.source == "manual" && result.protected && !result.requires_human_review
        }));
    }

    #[test]
    fn rejects_an_invalid_later_change_without_partially_mutating_results() {
        let mut first = result("ai", 2, true);
        first.result_id = "first".to_string();
        let mut second = result("ai", 3, true);
        second.result_id = "second".to_string();
        let mut results = vec![first, second];
        let before = results.clone();

        let error =
            apply_review_changes(&mut results, vec![change("first", 4), change("second", 6)])
                .unwrap_err();

        assert_eq!(error, "Review rating or color label is invalid");
        assert_eq!(results[0].rating, before[0].rating);
        assert_eq!(results[0].source, before[0].source);
        assert_eq!(results[1].rating, before[1].rating);
        assert_eq!(results[1].source, before[1].source);
    }

    #[test]
    fn rejects_unknown_and_duplicate_result_ids_without_mutation() {
        let mut item = result("ai", 2, true);
        item.result_id = "known".to_string();
        let mut results = vec![item];

        let unknown = apply_review_changes(&mut results, vec![change("missing", 4)])
            .expect_err("unknown result must fail loudly");
        assert!(unknown.contains("missing"));
        assert_eq!(results[0].rating, 2);
        assert_eq!(results[0].source, "ai");

        let duplicate =
            apply_review_changes(&mut results, vec![change("known", 4), change("known", 5)])
                .expect_err("duplicate result must fail loudly");
        assert!(duplicate.contains("duplicate"));
        assert_eq!(results[0].rating, 2);
        assert_eq!(results[0].source, "ai");
    }
}
