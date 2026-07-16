use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ColorLabel {
    Green,
    Yellow,
    Red,
}

impl ColorLabel {
    pub(crate) fn as_tag(self) -> &'static str {
        match self {
            Self::Green => "green",
            Self::Yellow => "yellow",
            Self::Red => "red",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ResultSource {
    Ai,
    Manual,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ConfirmedResult {
    pub result_id: String,
    pub source: ResultSource,
    pub rating: u8,
    pub color_label: Option<ColorLabel>,
    pub reason_codes: Vec<String>,
    pub confidence: f32,
    pub mode: String,
    pub model_version: String,
    pub policy_version: String,
    pub confirmed_at: String,
}

impl ConfirmedResult {
    pub(crate) fn validate(&self) -> Result<(), &'static str> {
        if self.result_id.is_empty() {
            return Err("result id is required");
        }
        if self.rating > 5 || (self.source == ResultSource::Ai && self.rating == 0) {
            return Err("AI rating must be 1-5 and manual rating must be 0-5");
        }
        if self.source == ResultSource::Manual && !self.reason_codes.is_empty() {
            return Err("manual results cannot retain AI reasons");
        }
        if self.reason_codes.len() > 2 {
            return Err("results cannot contain more than two reasons");
        }
        if self.confirmed_at.is_empty() {
            return Err("confirmation time is required");
        }
        if self.source == ResultSource::Ai {
            if !(0.0..=1.0).contains(&self.confidence) {
                return Err("confidence must be between 0 and 1");
            }
            if self.mode.is_empty()
                || self.model_version.is_empty()
                || self.policy_version.is_empty()
            {
                return Err("AI mode and versions are required");
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(source: ResultSource, rating: u8, reasons: &[&str]) -> ConfirmedResult {
        ConfirmedResult {
            result_id: "result-1".to_string(),
            source,
            rating,
            color_label: None,
            reason_codes: reasons.iter().map(|reason| (*reason).to_string()).collect(),
            confidence: 0.8,
            mode: "auto".to_string(),
            model_version: "test".to_string(),
            policy_version: "test".to_string(),
            confirmed_at: "2026-07-15T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn ai_results_require_a_visible_rating() {
        assert!(result(ResultSource::Ai, 0, &[]).validate().is_err());
        assert!(
            result(ResultSource::Ai, 5, &["sharp_subject"])
                .validate()
                .is_ok()
        );
    }

    #[test]
    fn manual_results_clear_ai_reasons_and_can_cancel_rating() {
        let mut manual = result(ResultSource::Manual, 0, &[]);
        manual.mode.clear();
        manual.model_version.clear();
        manual.policy_version.clear();
        assert!(manual.validate().is_ok());
        assert!(
            result(ResultSource::Manual, 0, &["sharp_subject"])
                .validate()
                .is_err()
        );
    }

    #[test]
    fn results_are_limited_to_two_deterministic_reasons() {
        assert!(
            result(
                ResultSource::Ai,
                4,
                &["sharp_subject", "group_best", "extra_reason"],
            )
            .validate()
            .is_err()
        );
    }
}
