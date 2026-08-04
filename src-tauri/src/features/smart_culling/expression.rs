//! Expression *usability* evidence derived from the FER+ classifier.
//!
//! Deliberate scope limit: this module never reports which emotion the model
//! predicted, and no emotion class is allowed to influence a rating. The feature
//! rules state that expression may only judge whether the captured instant is
//! technically usable, not whether the subject looks happy, attractive, or
//! "should be smiling".
//!
//! What the classifier is used for instead is its own certainty. A face caught
//! mid-transition (talking, a expression just starting or just collapsing) does
//! not resemble any single trained class, so the distribution comes out flat.
//! A settled expression produces a peaked distribution. Peakedness is therefore
//! a legitimate, emotion-agnostic signal for "is this a usable instant".

/// Number of FER+ output classes. Only the shape of the distribution is used.
pub const EXPRESSION_CLASS_COUNT: usize = 8;

/// Top-1 probability at or above this, combined with a low enough spread,
/// means the face has settled into one expression.
const STABLE_TOP_PROBABILITY: f32 = 0.55;
const STABLE_MAX_ENTROPY: f32 = 0.55;
/// A very flat distribution means the instant matches no settled expression.
const TRANSITIONAL_MAX_TOP_PROBABILITY: f32 = 0.35;
const TRANSITIONAL_MIN_ENTROPY: f32 = 0.75;

#[derive(Clone, Debug, PartialEq)]
pub struct ExpressionEvidence {
    /// `stable`, `transitional`, or `unknown`. Never an emotion name.
    pub state: &'static str,
    pub confidence: f32,
    pub reason: &'static str,
}

impl ExpressionEvidence {
    pub fn unavailable(reason: &'static str) -> Self {
        Self {
            state: "unknown",
            confidence: 0.0,
            reason,
        }
    }
}

/// Converts raw FER+ logits into usability evidence.
///
/// Returns `unknown` rather than guessing whenever the output is malformed or
/// the distribution sits between the two decided bands.
pub fn evaluate_expression(logits: &[f32]) -> ExpressionEvidence {
    let Some(probabilities) = softmax(logits) else {
        return ExpressionEvidence::unavailable("expression_output_invalid");
    };
    let top_probability = probabilities
        .iter()
        .copied()
        .fold(f32::MIN, f32::max)
        .clamp(0.0, 1.0);
    let entropy = normalized_entropy(&probabilities);

    if top_probability >= STABLE_TOP_PROBABILITY && entropy <= STABLE_MAX_ENTROPY {
        return ExpressionEvidence {
            state: "stable",
            // Peaked and decisive: confidence tracks how far past the bar it is.
            confidence: (top_probability * (1.0 - entropy)).clamp(0.0, 1.0),
            reason: "expression_settled_instant",
        };
    }
    if top_probability <= TRANSITIONAL_MAX_TOP_PROBABILITY || entropy >= TRANSITIONAL_MIN_ENTROPY {
        return ExpressionEvidence {
            state: "transitional",
            confidence: (entropy * (1.0 - top_probability)).clamp(0.0, 1.0),
            reason: "expression_mid_transition",
        };
    }
    ExpressionEvidence::unavailable("expression_evidence_inconclusive")
}

fn softmax(logits: &[f32]) -> Option<Vec<f32>> {
    if logits.len() != EXPRESSION_CLASS_COUNT || logits.iter().any(|value| !value.is_finite()) {
        return None;
    }
    let maximum = logits.iter().copied().fold(f32::MIN, f32::max);
    let exponentials = logits
        .iter()
        .map(|value| (value - maximum).exp())
        .collect::<Vec<_>>();
    let total = exponentials.iter().sum::<f32>();
    if !total.is_finite() || total <= f32::EPSILON {
        return None;
    }
    Some(exponentials.iter().map(|value| value / total).collect())
}

/// Shannon entropy scaled to 0..=1 so the thresholds stay independent of the
/// class count.
fn normalized_entropy(probabilities: &[f32]) -> f32 {
    let entropy = -probabilities
        .iter()
        .filter(|probability| **probability > 0.0)
        .map(|probability| probability * probability.ln())
        .sum::<f32>();
    (entropy / (EXPRESSION_CLASS_COUNT as f32).ln()).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_peaked_distribution_is_a_settled_instant() {
        let mut logits = vec![0.0f32; EXPRESSION_CLASS_COUNT];
        logits[1] = 8.0;

        let evidence = evaluate_expression(&logits);

        assert_eq!(evidence.state, "stable");
        assert_eq!(evidence.reason, "expression_settled_instant");
        assert!(evidence.confidence > 0.5);
    }

    #[test]
    fn a_flat_distribution_is_a_transitional_instant() {
        let logits = vec![0.0f32; EXPRESSION_CLASS_COUNT];

        let evidence = evaluate_expression(&logits);

        assert_eq!(evidence.state, "transitional");
        assert_eq!(evidence.reason, "expression_mid_transition");
    }

    #[test]
    fn the_middle_band_stays_unknown_instead_of_guessing() {
        // Two classes competing: neither settled nor clearly flat.
        let mut logits = vec![0.0f32; EXPRESSION_CLASS_COUNT];
        logits[0] = 2.4;
        logits[1] = 2.0;

        let evidence = evaluate_expression(&logits);

        assert_eq!(evidence.state, "unknown");
        assert_eq!(evidence.reason, "expression_evidence_inconclusive");
        assert_eq!(evidence.confidence, 0.0);
    }

    #[test]
    fn malformed_output_reports_unavailable_rather_than_a_state() {
        for broken in [
            vec![0.0f32; EXPRESSION_CLASS_COUNT - 1],
            vec![f32::NAN; EXPRESSION_CLASS_COUNT],
            vec![f32::INFINITY; EXPRESSION_CLASS_COUNT],
        ] {
            let evidence = evaluate_expression(&broken);
            assert_eq!(evidence.state, "unknown");
            assert_eq!(evidence.reason, "expression_output_invalid");
        }
    }

    #[test]
    fn evidence_never_leaks_an_emotion_label() {
        // Whichever class wins, the reported state must stay emotion-agnostic.
        for winner in 0..EXPRESSION_CLASS_COUNT {
            let mut logits = vec![0.0f32; EXPRESSION_CLASS_COUNT];
            logits[winner] = 9.0;
            let evidence = evaluate_expression(&logits);
            assert!(
                ["stable", "transitional", "unknown"].contains(&evidence.state),
                "unexpected state {}",
                evidence.state
            );
        }
    }
}
