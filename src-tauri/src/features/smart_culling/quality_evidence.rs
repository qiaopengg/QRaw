//! Model-independent quality evidence contracts.
//!
//! Model inference belongs in dedicated adapters. This module only validates
//! their scalar output, represents clarity gates, and combines the fixed mode
//! weights without promoting the remaining signals when one is unavailable.

const WEIGHT_SUM_TOLERANCE: f64 = 1e-9;
const LEGACY_OPTICAL_CONFIDENCE: f64 = 0.25;

pub(crate) const LEGACY_OPTICAL_SOURCE_VERSION: &str = "legacy_laplacian_exposure_65_35_v1";
pub(crate) const COMPOSITION_UNAVAILABLE_SOURCE_VERSION: &str = "composition_model_unavailable_v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EvidenceValidationError {
    ScoreNotFinite,
    ScoreOutOfRange,
    ConfidenceNotFinite,
    ConfidenceOutOfRange,
    ConfidenceWithoutScore,
    EmptyReason,
    EmptySourceVersion,
    DecidedClarityRequiresScore,
}

/// A calibrated scalar in the inclusive `[0, 1]` range.
///
/// `None` means unavailable, not zero. Unavailable evidence must have zero
/// confidence so a missing model can never look like a negative prediction.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ScoreEvidence {
    pub score: Option<f64>,
    pub confidence: f64,
    pub reason: &'static str,
    pub source_version: &'static str,
}

impl ScoreEvidence {
    pub(crate) fn try_available(
        score: f64,
        confidence: f64,
        reason: &'static str,
        source_version: &'static str,
    ) -> Result<Self, EvidenceValidationError> {
        let evidence = Self {
            score: Some(score),
            confidence,
            reason,
            source_version,
        };
        evidence.validate()?;
        Ok(evidence)
    }

    pub(crate) const fn unavailable(reason: &'static str, source_version: &'static str) -> Self {
        Self {
            score: None,
            confidence: 0.0,
            reason,
            source_version,
        }
    }

    pub(crate) fn validate(&self) -> Result<(), EvidenceValidationError> {
        if self.reason.trim().is_empty() {
            return Err(EvidenceValidationError::EmptyReason);
        }
        if self.source_version.trim().is_empty() {
            return Err(EvidenceValidationError::EmptySourceVersion);
        }
        if !self.confidence.is_finite() {
            return Err(EvidenceValidationError::ConfidenceNotFinite);
        }
        if !(0.0..=1.0).contains(&self.confidence) {
            return Err(EvidenceValidationError::ConfidenceOutOfRange);
        }
        match self.score {
            Some(score) if !score.is_finite() => Err(EvidenceValidationError::ScoreNotFinite),
            Some(score) if !(0.0..=1.0).contains(&score) => {
                Err(EvidenceValidationError::ScoreOutOfRange)
            }
            Some(_) => Ok(()),
            None if self.confidence > 0.0 => Err(EvidenceValidationError::ConfidenceWithoutScore),
            None => Ok(()),
        }
    }
}

/// `Unclear` is a source-calibrated, definite failure and therefore a one-star
/// gate. Borderline or unavailable results must be emitted as `Uncertain` so
/// later evidence can still be evaluated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ClarityState {
    Clear,
    Uncertain,
    Unclear,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ClarityEvidence {
    pub state: ClarityState,
    pub evidence: ScoreEvidence,
}

impl ClarityEvidence {
    pub(crate) fn try_new(
        state: ClarityState,
        score: Option<f64>,
        confidence: f64,
        reason: &'static str,
        source_version: &'static str,
    ) -> Result<Self, EvidenceValidationError> {
        let evidence = ScoreEvidence {
            score,
            confidence,
            reason,
            source_version,
        };
        evidence.validate()?;
        if state != ClarityState::Uncertain && evidence.score.is_none() {
            return Err(EvidenceValidationError::DecidedClarityRequiresScore);
        }
        Ok(Self { state, evidence })
    }

    pub(crate) fn unknown(reason: &'static str, source_version: &'static str) -> Self {
        Self {
            state: ClarityState::Uncertain,
            evidence: ScoreEvidence::unavailable(reason, source_version),
        }
    }

    pub(crate) fn is_one_star_gate(&self) -> bool {
        self.state == ClarityState::Unclear
    }

    pub(crate) fn continues_scoring(&self) -> bool {
        !self.is_one_star_gate()
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct WeightedEvidence<'a> {
    pub evidence: &'a ScoreEvidence,
    pub weight: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FixedScoreInterval {
    /// Score with every missing signal held at zero.
    pub lower_bound: f64,
    /// Score with every missing signal held at one.
    pub upper_bound: f64,
    pub missing_weight: f64,
    /// Fixed-weight confidence; missing evidence contributes zero.
    pub confidence: f64,
}

impl FixedScoreInterval {
    pub(crate) fn complete_score(self) -> Option<f64> {
        (self.missing_weight <= WEIGHT_SUM_TOLERANCE).then_some(self.lower_bound)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FixedCombineError {
    EmptySignals,
    InvalidWeight {
        index: usize,
    },
    InvalidEvidence {
        index: usize,
        error: EvidenceValidationError,
    },
    WeightsMustSumToOne,
}

/// Combines fixed product weights without re-normalizing around missing data.
///
/// Missing evidence produces an honest interval. For example, a score of 0.8
/// at 50% weight plus a missing 50% signal yields `[0.4, 0.9]`, not `0.8`.
pub(crate) fn combine_fixed_weights(
    signals: &[WeightedEvidence<'_>],
) -> Result<FixedScoreInterval, FixedCombineError> {
    if signals.is_empty() {
        return Err(FixedCombineError::EmptySignals);
    }

    let mut total_weight = 0.0;
    let mut lower_bound = 0.0;
    let mut missing_weight = 0.0;
    let mut confidence = 0.0;

    for (index, signal) in signals.iter().enumerate() {
        if !signal.weight.is_finite() || !(0.0..=1.0).contains(&signal.weight) {
            return Err(FixedCombineError::InvalidWeight { index });
        }
        signal
            .evidence
            .validate()
            .map_err(|error| FixedCombineError::InvalidEvidence { index, error })?;

        total_weight += signal.weight;
        confidence += signal.evidence.confidence * signal.weight;
        if let Some(score) = signal.evidence.score {
            lower_bound += score * signal.weight;
        } else {
            missing_weight += signal.weight;
        }
    }

    if (total_weight - 1.0).abs() > WEIGHT_SUM_TOLERANCE {
        return Err(FixedCombineError::WeightsMustSumToOne);
    }

    Ok(FixedScoreInterval {
        lower_bound,
        upper_bound: lower_bound + missing_weight,
        missing_weight,
        confidence,
    })
}

/// Transitional proxy derived only from the project's existing handcrafted
/// Laplacian and clipping metrics. It is deliberately low-confidence and its
/// source version cannot be confused with a learned IQA model.
pub(crate) fn legacy_optical_evidence(
    laplacian_variance: f64,
    exposure_metric: f64,
) -> ScoreEvidence {
    if !laplacian_variance.is_finite()
        || laplacian_variance < 0.0
        || !exposure_metric.is_finite()
        || !(0.0..=1.0).contains(&exposure_metric)
    {
        return ScoreEvidence::unavailable(
            "legacy_optical_metric_invalid",
            LEGACY_OPTICAL_SOURCE_VERSION,
        );
    }

    let normalized_focus = ((laplacian_variance + 1.0).log10() / 3.5).clamp(0.0, 1.0);
    let score = normalized_focus * 0.65 + exposure_metric * 0.35;
    ScoreEvidence::try_available(
        score,
        LEGACY_OPTICAL_CONFIDENCE,
        "legacy_handcrafted_optical_proxy_low_confidence",
        LEGACY_OPTICAL_SOURCE_VERSION,
    )
    .expect("validated legacy optical inputs must produce valid evidence")
}

/// Composition remains unavailable until a separately validated composition
/// model is integrated. Handwritten aesthetic rules are intentionally absent.
pub(crate) const fn composition_evidence_unavailable() -> ScoreEvidence {
    ScoreEvidence::unavailable(
        "composition_model_unavailable",
        COMPOSITION_UNAVAILABLE_SOURCE_VERSION,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn available(score: f64, confidence: f64) -> ScoreEvidence {
        ScoreEvidence::try_available(score, confidence, "test_available", "test_v1").unwrap()
    }

    #[test]
    fn available_evidence_rejects_non_finite_and_out_of_range_values() {
        assert_eq!(
            ScoreEvidence::try_available(f64::NAN, 0.5, "test", "test_v1"),
            Err(EvidenceValidationError::ScoreNotFinite)
        );
        assert_eq!(
            ScoreEvidence::try_available(1.1, 0.5, "test", "test_v1"),
            Err(EvidenceValidationError::ScoreOutOfRange)
        );
        assert_eq!(
            ScoreEvidence::try_available(0.5, f64::INFINITY, "test", "test_v1"),
            Err(EvidenceValidationError::ConfidenceNotFinite)
        );
        assert_eq!(
            ScoreEvidence::try_available(0.5, -0.1, "test", "test_v1"),
            Err(EvidenceValidationError::ConfidenceOutOfRange)
        );
    }

    #[test]
    fn clarity_only_definite_unclear_short_circuits_to_one_star() {
        let unclear = ClarityEvidence::try_new(
            ClarityState::Unclear,
            Some(0.1),
            0.95,
            "person_definitely_unclear",
            "clarity_test_v1",
        )
        .unwrap();
        let uncertain = ClarityEvidence::try_new(
            ClarityState::Uncertain,
            Some(0.45),
            0.4,
            "person_clarity_borderline",
            "clarity_test_v1",
        )
        .unwrap();
        let unavailable = ClarityEvidence::unknown("clarity_model_unavailable", "none_v1");

        assert!(unclear.is_one_star_gate());
        assert!(!unclear.continues_scoring());
        assert!(!uncertain.is_one_star_gate());
        assert!(uncertain.continues_scoring());
        assert!(unavailable.continues_scoring());
    }

    #[test]
    fn decided_clarity_without_a_score_fails_validation() {
        assert_eq!(
            ClarityEvidence::try_new(
                ClarityState::Clear,
                None,
                0.0,
                "invalid_decision",
                "clarity_test_v1",
            ),
            Err(EvidenceValidationError::DecidedClarityRequiresScore)
        );
    }

    #[test]
    fn fixed_weights_preserve_the_missing_signal_interval() {
        let present = available(0.8, 0.9);
        let missing = ScoreEvidence::unavailable("model_unavailable", "missing_v1");
        let combined = combine_fixed_weights(&[
            WeightedEvidence {
                evidence: &present,
                weight: 0.5,
            },
            WeightedEvidence {
                evidence: &missing,
                weight: 0.5,
            },
        ])
        .unwrap();

        assert!((combined.lower_bound - 0.4).abs() < f64::EPSILON);
        assert!((combined.upper_bound - 0.9).abs() < f64::EPSILON);
        assert!((combined.missing_weight - 0.5).abs() < f64::EPSILON);
        assert!((combined.confidence - 0.45).abs() < f64::EPSILON);
        assert_eq!(combined.complete_score(), None);
    }

    #[test]
    fn complete_fixed_weights_return_an_exact_score() {
        let first = available(0.2, 0.8);
        let second = available(0.8, 0.6);
        let combined = combine_fixed_weights(&[
            WeightedEvidence {
                evidence: &first,
                weight: 0.25,
            },
            WeightedEvidence {
                evidence: &second,
                weight: 0.75,
            },
        ])
        .unwrap();

        assert!((combined.lower_bound - 0.65).abs() < f64::EPSILON);
        assert_eq!(combined.lower_bound, combined.upper_bound);
        assert_eq!(combined.complete_score(), Some(combined.lower_bound));
    }

    #[test]
    fn fixed_combination_rejects_invalid_weights_and_evidence() {
        let valid = available(0.5, 0.5);
        assert_eq!(
            combine_fixed_weights(&[WeightedEvidence {
                evidence: &valid,
                weight: f64::NAN,
            }]),
            Err(FixedCombineError::InvalidWeight { index: 0 })
        );
        assert_eq!(
            combine_fixed_weights(&[WeightedEvidence {
                evidence: &valid,
                weight: 0.5,
            }]),
            Err(FixedCombineError::WeightsMustSumToOne)
        );

        let invalid = ScoreEvidence {
            score: None,
            confidence: 0.5,
            reason: "invalid_missing_signal",
            source_version: "test_v1",
        };
        assert_eq!(
            combine_fixed_weights(&[WeightedEvidence {
                evidence: &invalid,
                weight: 1.0,
            }]),
            Err(FixedCombineError::InvalidEvidence {
                index: 0,
                error: EvidenceValidationError::ConfidenceWithoutScore,
            })
        );
    }

    #[test]
    fn invalid_legacy_metrics_become_explicitly_unavailable() {
        for evidence in [
            legacy_optical_evidence(f64::NAN, 0.5),
            legacy_optical_evidence(-1.0, 0.5),
            legacy_optical_evidence(10.0, 1.1),
        ] {
            assert_eq!(evidence.score, None);
            assert_eq!(evidence.confidence, 0.0);
            assert_eq!(evidence.reason, "legacy_optical_metric_invalid");
            assert_eq!(evidence.source_version, LEGACY_OPTICAL_SOURCE_VERSION);
        }
    }

    #[test]
    fn legacy_proxy_is_low_confidence_and_never_claims_to_be_a_model() {
        let evidence = legacy_optical_evidence(0.0, 1.0);

        assert!((evidence.score.unwrap() - 0.35).abs() < f64::EPSILON);
        assert_eq!(evidence.confidence, LEGACY_OPTICAL_CONFIDENCE);
        assert_eq!(
            evidence.reason,
            "legacy_handcrafted_optical_proxy_low_confidence"
        );
        assert!(evidence.source_version.starts_with("legacy_"));
    }

    #[test]
    fn composition_has_no_handwritten_fallback() {
        let evidence = composition_evidence_unavailable();

        assert_eq!(evidence.score, None);
        assert_eq!(evidence.confidence, 0.0);
        assert_eq!(evidence.reason, "composition_model_unavailable");
        assert_eq!(
            evidence.source_version,
            COMPOSITION_UNAVAILABLE_SOURCE_VERSION
        );
    }
}
