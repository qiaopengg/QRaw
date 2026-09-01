//! Expression evidence for burst-photo culling.
//!
//! This module deliberately does not infer an emotion and does not assign a
//! rating. A single frame produces a read-only descriptor and may enter the
//! conservative quality calibration. A stable or transitional technical state
//! still requires chronological descriptors for the same tracked subject in
//! one similar-shot group.

mod sequence;
#[cfg(any(test, all(debug_assertions, target_os = "macos")))]
mod usability;

use std::collections::BTreeMap;

use super::types::FaceResult;
pub(in crate::features::smart_culling) use sequence::{
    ExpressionSequenceAssessment, ExpressionTechnicalState, assess_sequence, assess_sequence_slots,
};

pub(in crate::features::smart_culling) const EXPRESSION_DESCRIPTOR_VERSION: &str =
    "qraw-expression-descriptor-1.0";
pub(in crate::features::smart_culling) const EXPRESSION_SEQUENCE_POLICY_VERSION: &str =
    "qraw-expression-sequence-policy-1.0";
#[cfg(any(test, all(debug_assertions, target_os = "macos")))]
pub(in crate::features::smart_culling) const EXPRESSION_QUALITY_REASON: &str =
    "expression_single_frame_quality_hsemotion_fusion_calibration";
pub(in crate::features::smart_culling) const EXPRESSION_QUALITY_GATE_ENABLED: bool =
    cfg!(any(test, all(debug_assertions, target_os = "macos")));

const MIN_RELIABLE_FACE_PRESENCE: f32 = 0.75;
const MAX_RELIABLE_LANDMARK_ERROR: f32 = 0.18;
const MAX_RELIABLE_ABS_HEAD_ANGLE_DEGREES: f32 = 35.0;
const MAX_COMPARABLE_HEAD_ANGLE_DELTA_DEGREES: f32 = 12.0;

// Exact MediaPipe BlendshapeV2 output order after removing all `eye*`
// coefficients. Eye coefficients remain exclusively owned by the frozen eye
// contract and cannot influence this descriptor or its sequence policy.
const NON_EYE_BLENDSHAPE_NAMES: [&str; 38] = [
    "_neutral",
    "browDownLeft",
    "browDownRight",
    "browInnerUp",
    "browOuterUpLeft",
    "browOuterUpRight",
    "cheekPuff",
    "cheekSquintLeft",
    "cheekSquintRight",
    "jawForward",
    "jawLeft",
    "jawOpen",
    "jawRight",
    "mouthClose",
    "mouthDimpleLeft",
    "mouthDimpleRight",
    "mouthFrownLeft",
    "mouthFrownRight",
    "mouthFunnel",
    "mouthLeft",
    "mouthLowerDownLeft",
    "mouthLowerDownRight",
    "mouthPressLeft",
    "mouthPressRight",
    "mouthPucker",
    "mouthRight",
    "mouthRollLower",
    "mouthRollUpper",
    "mouthShrugLower",
    "mouthShrugUpper",
    "mouthSmileLeft",
    "mouthSmileRight",
    "mouthStretchLeft",
    "mouthStretchRight",
    "mouthUpperUpLeft",
    "mouthUpperUpRight",
    "noseSneerLeft",
    "noseSneerRight",
];

/// Immutable, emotion-agnostic evidence extracted from one face in one frame.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::features::smart_culling) struct ExpressionDescriptor {
    non_eye_blendshapes: [f32; NON_EYE_BLENDSHAPE_NAMES.len()],
    tongue_out: f32,
    head_pitch_degrees: Option<f32>,
    head_yaw_degrees: Option<f32>,
    landmark_consistency_error: Option<f32>,
    face_presence: f32,
}

impl ExpressionDescriptor {
    pub(in crate::features::smart_culling) fn from_face_motion(
        blendshapes: &BTreeMap<&'static str, f32>,
        tongue_out: f32,
        head_pitch_degrees: Option<f32>,
        head_yaw_degrees: Option<f32>,
        landmark_consistency_error: Option<f32>,
        face_presence: f32,
    ) -> Result<Self, ExpressionDescriptorError> {
        let mut non_eye_blendshapes = [0.0; NON_EYE_BLENDSHAPE_NAMES.len()];
        for (index, name) in NON_EYE_BLENDSHAPE_NAMES.iter().enumerate() {
            let value = blendshapes
                .get(name)
                .copied()
                .ok_or(ExpressionDescriptorError::MissingBlendshape(name))?;
            validate_probability(value, name)?;
            non_eye_blendshapes[index] = value;
        }
        validate_probability(tongue_out, "tongueOut")?;
        validate_probability(face_presence, "facePresence")?;
        validate_optional_finite(head_pitch_degrees, "headPitch")?;
        validate_optional_finite(head_yaw_degrees, "headYaw")?;
        if landmark_consistency_error.is_some_and(|value| !value.is_finite() || value < 0.0) {
            return Err(ExpressionDescriptorError::InvalidValue(
                "landmarkConsistencyError",
            ));
        }

        Ok(Self {
            non_eye_blendshapes,
            tongue_out,
            head_pitch_degrees,
            head_yaw_degrees,
            landmark_consistency_error,
            face_presence,
        })
    }

    pub(in crate::features::smart_culling) const fn descriptor_version(&self) -> &'static str {
        EXPRESSION_DESCRIPTOR_VERSION
    }

    pub(in crate::features::smart_culling) fn non_eye_blendshapes(&self) -> &[f32] {
        &self.non_eye_blendshapes
    }

    pub(in crate::features::smart_culling) const fn tongue_out(&self) -> f32 {
        self.tongue_out
    }

    pub(in crate::features::smart_culling) const fn head_pitch_degrees(&self) -> Option<f32> {
        self.head_pitch_degrees
    }

    pub(in crate::features::smart_culling) const fn head_yaw_degrees(&self) -> Option<f32> {
        self.head_yaw_degrees
    }

    pub(in crate::features::smart_culling) fn is_reliable(&self) -> bool {
        self.face_presence >= MIN_RELIABLE_FACE_PRESENCE
            && self
                .landmark_consistency_error
                .is_some_and(|error| error <= MAX_RELIABLE_LANDMARK_ERROR)
            && self
                .head_pitch_degrees
                .is_some_and(|angle| angle.abs() <= MAX_RELIABLE_ABS_HEAD_ANGLE_DEGREES)
            && self
                .head_yaw_degrees
                .is_some_and(|angle| angle.abs() <= MAX_RELIABLE_ABS_HEAD_ANGLE_DEGREES)
    }

    fn reliability(&self) -> f32 {
        if !self.is_reliable() {
            return 0.0;
        }
        let presence = ((self.face_presence - MIN_RELIABLE_FACE_PRESENCE)
            / (1.0 - MIN_RELIABLE_FACE_PRESENCE))
            .clamp(0.35, 1.0);
        let geometry = self
            .landmark_consistency_error
            .map(|error| 1.0 - error / MAX_RELIABLE_LANDMARK_ERROR)
            .unwrap_or(0.0)
            .clamp(0.35, 1.0);
        let pose = self
            .head_pitch_degrees
            .zip(self.head_yaw_degrees)
            .map(|(pitch, yaw)| {
                1.0 - pitch.abs().max(yaw.abs()) / MAX_RELIABLE_ABS_HEAD_ANGLE_DEGREES
            })
            .unwrap_or(0.0)
            .clamp(0.35, 1.0);
        presence.min(geometry).min(pose)
    }

    fn is_comparable_with(&self, other: &Self) -> bool {
        if !self.is_reliable() || !other.is_reliable() {
            return false;
        }
        self.head_pitch_degrees
            .zip(other.head_pitch_degrees)
            .is_some_and(|(left, right)| {
                (left - right).abs() <= MAX_COMPARABLE_HEAD_ANGLE_DELTA_DEGREES
            })
            && self
                .head_yaw_degrees
                .zip(other.head_yaw_degrees)
                .is_some_and(|(left, right)| {
                    (left - right).abs() <= MAX_COMPARABLE_HEAD_ANGLE_DELTA_DEGREES
                })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::features::smart_culling) enum ExpressionDescriptorError {
    MissingBlendshape(&'static str),
    InvalidValue(&'static str),
}

impl std::fmt::Display for ExpressionDescriptorError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingBlendshape(name) => {
                write!(formatter, "missing expression blendshape: {name}")
            }
            Self::InvalidValue(name) => write!(formatter, "invalid expression evidence: {name}"),
        }
    }
}

impl std::error::Error for ExpressionDescriptorError {}

#[derive(Clone, Debug, PartialEq)]
pub struct ExpressionEvidence {
    pub state: &'static str,
    pub quality_score: Option<f32>,
    pub confidence: f32,
    pub reason: &'static str,
}

impl ExpressionEvidence {
    pub fn unavailable(reason: &'static str) -> Self {
        Self {
            state: "unknown",
            quality_score: None,
            confidence: 0.0,
            reason,
        }
    }

    #[cfg(any(test, all(debug_assertions, target_os = "macos")))]
    pub(in crate::features::smart_culling) fn from_single_frame(
        descriptor: &ExpressionDescriptor,
        model_outputs: &super::expression_quality_poc::ExpressionQualityModelOutputs,
    ) -> Self {
        usability::assess_single_frame(descriptor, model_outputs)
    }

    pub(in crate::features::smart_culling) fn from_sequence(
        assessment: &ExpressionSequenceAssessment,
    ) -> Self {
        Self {
            state: assessment.state().as_str(),
            quality_score: None,
            confidence: assessment.confidence(),
            reason: assessment.reason(),
        }
    }
}

/// Updates one chronological, same-subject sequence in place.
///
/// Each slice position represents one frame. A missing or unresolved subject
/// must remain `None`; it is not removed from the sequence. This function only
/// writes expression fields and never reads or mutates eye assessment data.
pub(in crate::features::smart_culling) fn apply_same_subject_sequence(
    frames: &mut [Option<&mut FaceResult>],
) {
    let assessments = {
        let descriptors = frames
            .iter()
            .map(|frame| frame.as_deref().and_then(FaceResult::expression_descriptor))
            .collect::<Vec<_>>();
        assess_sequence_slots(&descriptors)
    };

    for (frame, assessment) in frames.iter_mut().zip(&assessments) {
        if let Some(face) = frame.as_deref_mut() {
            if face.expression_score.is_none() {
                face.apply_expression_sequence_assessment(assessment);
            }
        }
    }
}

fn validate_probability(value: f32, name: &'static str) -> Result<(), ExpressionDescriptorError> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(ExpressionDescriptorError::InvalidValue(name))
    }
}

fn validate_optional_finite(
    value: Option<f32>,
    name: &'static str,
) -> Result<(), ExpressionDescriptorError> {
    if value.is_some_and(|value| !value.is_finite()) {
        Err(ExpressionDescriptorError::InvalidValue(name))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::smart_culling::types::{EyeDisposition, EyeResult};

    fn scores() -> BTreeMap<&'static str, f32> {
        NON_EYE_BLENDSHAPE_NAMES
            .into_iter()
            .map(|name| (name, 0.0))
            .chain([("eyeBlinkLeft", 0.0), ("eyeBlinkRight", 0.0)])
            .collect()
    }

    fn descriptor(scores: &BTreeMap<&'static str, f32>) -> ExpressionDescriptor {
        ExpressionDescriptor::from_face_motion(scores, 0.0, Some(0.0), Some(0.0), Some(0.01), 0.99)
            .unwrap()
    }

    fn face(expression_descriptor: Option<ExpressionDescriptor>) -> FaceResult {
        FaceResult {
            bbox: [0.0, 0.0, 100.0, 100.0],
            landmarks: [(0.0, 0.0); 5],
            detection_score: 1.0,
            left_eye: EyeResult::unavailable("test", 0, None),
            right_eye: EyeResult::unavailable("test", 0, None),
            eye_disposition: EyeDisposition::Unknown,
            expression_state: "unknown".to_string(),
            expression_score: None,
            expression_confidence: 0.0,
            expression_reason: "expression_requires_sequence_context".to_string(),
            expression_descriptor,
            sharpness_metric: 1.0,
            sharpness_confidence: 1.0,
            exposure_metric: 1.0,
            exposure_confidence: 1.0,
            identity_embedding: None,
        }
    }

    #[test]
    fn unvalidated_expression_evidence_never_claims_a_decided_state() {
        let evidence = ExpressionEvidence::unavailable("expression_model_unvalidated");

        assert_eq!(evidence.state, "unknown");
        assert_eq!(evidence.quality_score, None);
        assert_eq!(evidence.confidence, 0.0);
        assert_eq!(evidence.reason, "expression_model_unvalidated");
    }

    #[test]
    fn descriptor_has_an_independent_version_and_no_mutable_accessors() {
        let descriptor = descriptor(&scores());

        assert_eq!(
            descriptor.descriptor_version(),
            "qraw-expression-descriptor-1.0"
        );
        assert_eq!(descriptor.non_eye_blendshapes().len(), 38);
        assert_eq!(descriptor.tongue_out(), 0.0);
    }

    #[test]
    fn eye_coefficients_cannot_change_the_expression_descriptor() {
        let baseline = scores();
        let mut changed_eyes = baseline.clone();
        changed_eyes.insert("eyeBlinkLeft", 1.0);
        changed_eyes.insert("eyeBlinkRight", 1.0);

        assert_eq!(descriptor(&baseline), descriptor(&changed_eyes));
    }

    #[test]
    fn missing_or_invalid_model_evidence_fails_loudly() {
        let mut missing = scores();
        missing.remove("mouthSmileLeft");
        assert!(matches!(
            ExpressionDescriptor::from_face_motion(
                &missing,
                0.0,
                Some(0.0),
                Some(0.0),
                Some(0.0),
                1.0,
            ),
            Err(ExpressionDescriptorError::MissingBlendshape(
                "mouthSmileLeft"
            ))
        ));

        let mut invalid = scores();
        invalid.insert("jawOpen", f32::NAN);
        assert!(
            ExpressionDescriptor::from_face_motion(
                &invalid,
                0.0,
                Some(0.0),
                Some(0.0),
                Some(0.0),
                1.0,
            )
            .is_err()
        );
    }

    #[test]
    fn one_reliable_frame_emits_a_calibrated_quality_score() {
        let model_outputs = super::super::expression_quality_poc::ExpressionQualityModelOutputs {
            mtl: [0.0; 10],
            vgaf: [0.0; 8],
        };
        let evidence =
            ExpressionEvidence::from_single_frame(&descriptor(&scores()), &model_outputs);

        assert_eq!(evidence.state, "scored");
        assert!(evidence.quality_score.is_some());
        assert_eq!(evidence.reason, EXPRESSION_QUALITY_REASON);
    }

    #[test]
    fn root_api_exposes_sequence_state_without_rating_inputs() {
        let results = assess_sequence(&[
            descriptor(&scores()),
            descriptor(&scores()),
            descriptor(&scores()),
        ]);

        assert_eq!(results[1].state(), ExpressionTechnicalState::Stable);
    }

    #[test]
    fn same_subject_api_writes_only_aligned_sequence_results() {
        let baseline = descriptor(&scores());
        let mut transitioned_scores = scores();
        for name in ["jawOpen", "mouthFunnel", "mouthLeft", "mouthRight"] {
            transitioned_scores.insert(name, 0.9);
        }
        let transition = descriptor(&transitioned_scores);
        let mut first = face(Some(baseline.clone()));
        let mut middle = face(Some(transition));
        let mut last = face(Some(baseline));

        apply_same_subject_sequence(&mut [Some(&mut first), Some(&mut middle), Some(&mut last)]);

        assert_eq!(first.expression_state, "unknown");
        assert_eq!(middle.expression_state, "transitional");
        assert_eq!(
            middle.expression_reason,
            "expression_sequence_isolated_transition"
        );
        assert!(middle.expression_confidence > 0.0);
        assert_eq!(last.expression_state, "unknown");
        assert!(middle.expression_descriptor().is_some());
    }

    #[test]
    fn one_frame_api_cannot_promote_expression_out_of_unknown() {
        let mut only = face(Some(descriptor(&scores())));

        apply_same_subject_sequence(&mut [Some(&mut only)]);

        assert_eq!(only.expression_state, "unknown");
        assert_eq!(only.expression_confidence, 0.0);
        assert_eq!(only.expression_reason, "expression_sequence_too_short");
    }

    #[test]
    fn sequence_motion_state_cannot_overwrite_single_frame_quality_scores() {
        let baseline = descriptor(&scores());
        let mut first = face(Some(baseline.clone()));
        let mut middle = face(Some(baseline.clone()));
        let mut last = face(Some(baseline));
        first.expression_state = "scored".to_string();
        first.expression_score = Some(0.9);
        first.expression_confidence = 0.4;
        first.expression_reason = EXPRESSION_QUALITY_REASON.to_string();
        for face in [&mut middle, &mut last] {
            face.expression_state = "scored".to_string();
            face.expression_score = Some(0.1);
            face.expression_confidence = 0.4;
            face.expression_reason = EXPRESSION_QUALITY_REASON.to_string();
        }

        apply_same_subject_sequence(&mut [Some(&mut first), Some(&mut middle), Some(&mut last)]);

        assert_eq!(first.expression_state, "scored");
        assert_eq!(first.expression_score, Some(0.9));
        assert_eq!(first.expression_reason, EXPRESSION_QUALITY_REASON);
        assert!(
            [&middle, &last]
                .into_iter()
                .all(|face| face.expression_state == "scored" && face.expression_score == Some(0.1))
        );
        assert_eq!(middle.expression_reason, EXPRESSION_QUALITY_REASON);
    }
}
