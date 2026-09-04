//! Conservative single-frame five-level expression-quality assessment.
//!
//! The ordinal head was calibrated from the 283 reliable observations in the
//! frozen 001-003 batches. Those batches are known calibration data, so this
//! remains a low-confidence macOS Debug result until real-camera blind testing.

use super::{EXPRESSION_QUALITY_REASON, ExpressionDescriptor, ExpressionEvidence, ordinal};
use crate::features::smart_culling::expression_quality_poc::ExpressionQualityModelOutputs;

const MAX_CALIBRATION_CONFIDENCE: f32 = 0.49;

pub(super) fn assess_single_frame(
    descriptor: &ExpressionDescriptor,
    model_outputs: &ExpressionQualityModelOutputs,
) -> ExpressionEvidence {
    if !descriptor.is_reliable() {
        return ExpressionEvidence::unavailable("expression_frame_evidence_unreliable");
    }
    let Some(classification) = ordinal::classify(descriptor, model_outputs) else {
        return ExpressionEvidence::unavailable("expression_quality_model_output_invalid");
    };

    ExpressionEvidence {
        state: classification.level.state(),
        quality_score: Some(classification.level.normalized_score()),
        confidence: (descriptor.reliability() * classification.probability)
            .clamp(0.0, MAX_CALIBRATION_CONFIDENCE),
        reason: EXPRESSION_QUALITY_REASON,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::features::smart_culling::expression::NON_EYE_BLENDSHAPE_NAMES;

    fn descriptor(yaw: f32, eye_score: f32) -> ExpressionDescriptor {
        let mut scores = NON_EYE_BLENDSHAPE_NAMES
            .into_iter()
            .map(|name| (name, 0.0))
            .collect::<BTreeMap<_, _>>();
        scores.insert("eyeBlinkLeft", eye_score);
        scores.insert("eyeBlinkRight", eye_score);
        ExpressionDescriptor::from_face_motion(&scores, 0.0, Some(0.0), Some(yaw), Some(0.01), 0.99)
            .unwrap()
    }

    fn outputs(value: f32) -> ExpressionQualityModelOutputs {
        ExpressionQualityModelOutputs {
            mtl: [value; 10],
            vgaf: [value; 8],
        }
    }

    #[test]
    fn ordinal_head_emits_one_frozen_level_with_low_confidence() {
        let evidence = assess_single_frame(&descriptor(0.0, 0.0), &outputs(0.0));

        assert!(matches!(
            evidence.state,
            "severe_failure" | "not_recommended" | "natural" | "excellent" | "outstanding"
        ));
        assert!(matches!(
            evidence.quality_score,
            Some(0.0 | 0.25 | 0.5 | 0.75 | 1.0)
        ));
        assert!(evidence.confidence > 0.0);
        assert!(evidence.confidence <= MAX_CALIBRATION_CONFIDENCE);
        assert_eq!(evidence.reason, EXPRESSION_QUALITY_REASON);
    }

    #[test]
    fn eye_blendshape_coefficients_cannot_change_the_expression_level() {
        let baseline = assess_single_frame(&descriptor(0.0, 0.0), &outputs(0.0));
        let changed_eyes = assess_single_frame(&descriptor(0.0, 1.0), &outputs(0.0));

        assert_eq!(baseline, changed_eyes);
    }

    #[test]
    fn unreliable_geometry_stays_unknown_before_model_inference() {
        let evidence = assess_single_frame(&descriptor(40.0, 0.0), &outputs(0.0));

        assert_eq!(evidence.state, "unknown");
        assert_eq!(evidence.quality_score, None);
        assert_eq!(evidence.reason, "expression_frame_evidence_unreliable");
    }

    #[test]
    fn non_finite_model_output_fails_closed_to_unknown() {
        let evidence = assess_single_frame(&descriptor(0.0, 0.0), &outputs(f32::NAN));

        assert_eq!(evidence.state, "unknown");
        assert_eq!(evidence.quality_score, None);
        assert_eq!(evidence.reason, "expression_quality_model_output_invalid");
    }
}
