//! Expression evidence is deliberately unavailable until a face-motion model
//! is validated on real burst-photo data.
//!
//! FER+ class confidence is not a valid proxy for whether an expression is
//! settled or transitional, so no emotion-class output is converted into a
//! technical usability decision here.

#[derive(Clone, Debug, PartialEq)]
pub struct ExpressionEvidence {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unvalidated_expression_evidence_never_claims_a_decided_state() {
        let evidence = ExpressionEvidence::unavailable("expression_model_unvalidated");

        assert_eq!(evidence.state, "unknown");
        assert_eq!(evidence.confidence, 0.0);
        assert_eq!(evidence.reason, "expression_model_unvalidated");
    }
}
