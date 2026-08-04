use super::types::KeyPersonEvidence;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct KeyPersonDecision {
    pub rating: u8,
    pub requires_human_review: bool,
    pub reason_code: Option<String>,
}

pub(crate) fn apply_key_person_gate(
    evidence: &[KeyPersonEvidence],
    base_rating: u8,
) -> KeyPersonDecision {
    if evidence.is_empty() {
        return KeyPersonDecision {
            rating: base_rating,
            requires_human_review: false,
            reason_code: None,
        };
    }

    if let Some(unresolved) = evidence
        .iter()
        .find(|item| matches!(item.status.as_str(), "suspected" | "ambiguous" | "unknown"))
    {
        return KeyPersonDecision {
            rating: 0,
            requires_human_review: true,
            reason_code: Some(format!(
                "key_person_{}_{}",
                unresolved.priority, unresolved.status
            )),
        };
    }

    if let Some(missing) = evidence.iter().find(|item| item.status == "missing") {
        return KeyPersonDecision {
            rating: base_rating.min(2),
            requires_human_review: false,
            reason_code: Some(format!("key_person_{}_missing", missing.priority)),
        };
    }

    if evidence.iter().all(|item| item.status == "confirmed") {
        KeyPersonDecision {
            rating: base_rating,
            requires_human_review: false,
            reason_code: evidence
                .first()
                .map(|item| format!("key_person_{}_confirmed", item.priority)),
        }
    } else {
        KeyPersonDecision {
            rating: 0,
            requires_human_review: true,
            reason_code: Some("key_person_identity_unknown".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn evidence(priority: usize, status: &str) -> KeyPersonEvidence {
        KeyPersonEvidence {
            priority,
            face_index: None,
            similarity: None,
            status: status.to_string(),
            auto_score_eligible: status == "confirmed",
            performance_rank: None,
        }
    }

    #[test]
    fn every_selected_identity_must_be_confirmed_before_secondary_scoring() {
        let decision =
            apply_key_person_gate(&[evidence(1, "confirmed"), evidence(2, "suspected")], 5);
        assert_eq!(decision.rating, 0);
        assert!(decision.requires_human_review);
    }

    #[test]
    fn a_clearly_missing_person_caps_the_result_at_two_stars() {
        let decision =
            apply_key_person_gate(&[evidence(1, "confirmed"), evidence(2, "missing")], 5);
        assert_eq!(decision.rating, 2);
        assert!(!decision.requires_human_review);
    }

    #[test]
    fn suspected_ambiguous_and_unknown_identities_use_manual_review() {
        for status in ["suspected", "ambiguous", "unknown"] {
            let decision = apply_key_person_gate(&[evidence(1, status)], 4);
            assert_eq!(decision.rating, 0);
            assert!(decision.requires_human_review);
        }
    }
}
