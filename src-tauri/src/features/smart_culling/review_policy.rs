use super::api::ReviewResult;

pub(crate) fn result_is_writable(result: &ReviewResult) -> bool {
    if result.source == "manual" {
        return result.rating <= 5;
    }
    !result.requires_human_review && (1..=5).contains(&result.rating)
}

#[cfg(test)]
mod tests {
    use super::result_is_writable;
    use crate::features::smart_culling::api::ReviewResult;

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

    #[test]
    fn writes_all_valid_ai_ratings_without_a_pool_gate() {
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
}
