//! Frozen eye-state assessment built from the audited face-motion evidence.

use super::super::types::{EyeDisposition, EyeResult};
use super::evidence::FaceMotionEvidenceDump;
use super::eye_policy::EyeUsability;
use super::{EYE_MODEL_CONTRACT_VERSION, EYE_POLICY_VERSION};

// Three observed residual-aperture samples landed immediately above the old
// -5 degree boundary. The 1.1 policy keeps the frozen eye classifier untouched
// and only admits this narrow -4.5 degree band into the existing deliberate
// pose candidate state instead of forcing an automatic one-star result.
const DELIBERATE_POSE_MAX_PITCH_DEGREES: f32 = -4.5;
const DELIBERATE_POSE_MIN_EYE_ASPECT_RATIO: f32 = 0.10;
const DELIBERATE_POSE_MAX_BLINK_SCORE: f32 = 0.50;

pub(in crate::features::smart_culling) struct EyeAssessment {
    left_eye: EyeResult,
    right_eye: EyeResult,
    disposition: EyeDisposition,
}

impl EyeAssessment {
    pub(in crate::features::smart_culling) fn left_eye(&self) -> &EyeResult {
        &self.left_eye
    }

    pub(in crate::features::smart_culling) fn right_eye(&self) -> &EyeResult {
        &self.right_eye
    }

    pub(in crate::features::smart_culling) const fn disposition(&self) -> EyeDisposition {
        self.disposition
    }

    pub(in crate::features::smart_culling) const fn policy_version(&self) -> &'static str {
        EYE_POLICY_VERSION
    }

    pub(in crate::features::smart_culling) const fn model_contract_version(&self) -> &'static str {
        EYE_MODEL_CONTRACT_VERSION
    }

    pub(in crate::features::smart_culling) fn into_legacy_parts(
        self,
    ) -> (EyeResult, EyeResult, EyeDisposition) {
        (self.left_eye, self.right_eye, self.disposition)
    }
}

pub(super) fn assess(
    evidence: &FaceMotionEvidenceDump,
    pose_suppresses_eye_state: bool,
) -> EyeAssessment {
    if pose_suppresses_eye_state {
        return EyeAssessment {
            left_eye: EyeResult::unavailable("eye_pose_unreliable", 0, None),
            right_eye: EyeResult::unavailable("eye_pose_unreliable", 0, None),
            disposition: EyeDisposition::Unknown,
        };
    }

    EyeAssessment {
        left_eye: eye_result(
            evidence.left_eye,
            evidence.left_eye_aspect_ratio,
            evidence.blendshapes.get("eyeBlinkLeft").copied(),
            evidence,
        ),
        right_eye: eye_result(
            evidence.right_eye,
            evidence.right_eye_aspect_ratio,
            evidence.blendshapes.get("eyeBlinkRight").copied(),
            evidence,
        ),
        disposition: disposition(evidence),
    }
}

fn disposition(evidence: &FaceMotionEvidenceDump) -> EyeDisposition {
    use EyeUsability::{Open, Unknown, Unusable};

    match (evidence.left_eye, evidence.right_eye) {
        (Open, Open) => EyeDisposition::Open,
        (Unusable, Unusable) | (Unusable, Unknown) | (Unknown, Unusable)
            if has_deliberate_pose_evidence(evidence) =>
        {
            EyeDisposition::DeliberatePoseCandidate
        }
        (Unusable, _) | (_, Unusable) => EyeDisposition::Unusable,
        (Unknown, _) | (_, Unknown) => EyeDisposition::Unknown,
    }
}

fn has_deliberate_pose_evidence(evidence: &FaceMotionEvidenceDump) -> bool {
    let (Some(left_aspect), Some(right_aspect)) = (
        evidence.left_eye_aspect_ratio,
        evidence.right_eye_aspect_ratio,
    ) else {
        return false;
    };
    let (Some(left_blink), Some(right_blink)) = (
        evidence.blendshapes.get("eyeBlinkLeft").copied(),
        evidence.blendshapes.get("eyeBlinkRight").copied(),
    ) else {
        return false;
    };
    let min_aspect_ratio = left_aspect.min(right_aspect);
    let min_blink = left_blink.min(right_blink);
    evidence
        .head_pitch_degrees
        .is_some_and(|pitch| pitch <= DELIBERATE_POSE_MAX_PITCH_DEGREES)
        && min_aspect_ratio >= DELIBERATE_POSE_MIN_EYE_ASPECT_RATIO
        && min_blink <= DELIBERATE_POSE_MAX_BLINK_SCORE
}

fn eye_result(
    usability: EyeUsability,
    aspect_ratio: Option<f32>,
    blink_score: Option<f32>,
    evidence: &FaceMotionEvidenceDump,
) -> EyeResult {
    let open_probability = aspect_ratio.zip(blink_score).map(|(aspect, blink)| {
        let geometric_openness = (aspect / 0.25).clamp(0.0, 1.0);
        ((geometric_openness + (1.0 - blink).clamp(0.0, 1.0)) * 0.5).clamp(0.0, 1.0)
    });
    let (state, reason) = match usability {
        EyeUsability::Open => ("open", "face_motion_eye_open"),
        EyeUsability::Unusable => ("closed", "face_motion_eye_closed"),
        EyeUsability::Unknown => ("unknown", "face_motion_eye_uncertain"),
    };
    let confidence = if usability == EyeUsability::Unknown {
        0.0
    } else {
        open_probability
            .map(|probability| ((probability - 0.5).abs() * 2.0).clamp(0.35, 1.0))
            .unwrap_or(0.0)
    };
    let eye_edge = evidence.roi.width.min(evidence.roi.height) * 0.15;

    EyeResult {
        open_probability,
        state: state.to_string(),
        confidence,
        reason: reason.to_string(),
        effective_pixels: eye_edge.max(0.0).powi(2).round() as u32,
        sharpness_metric: None,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::features::smart_culling::face_motion_poc::roi::FaceRoi;

    fn evidence(left: EyeUsability, right: EyeUsability) -> FaceMotionEvidenceDump {
        FaceMotionEvidenceDump {
            roi: FaceRoi {
                center_x: 50.0,
                center_y: 50.0,
                width: 100.0,
                height: 100.0,
                rotation: 0.0,
            },
            face_presence: 1.0,
            tongue_out: 0.0,
            left_eye_aspect_ratio: Some(0.10),
            right_eye_aspect_ratio: Some(0.10),
            head_pitch_degrees: Some(0.0),
            head_yaw_degrees: Some(0.0),
            landmark_consistency_error: Some(0.0),
            left_eye: left,
            right_eye: right,
            overall_eye: left,
            blendshapes: BTreeMap::from([("eyeBlinkLeft", 0.8), ("eyeBlinkRight", 0.8)]),
        }
    }

    #[test]
    fn frontal_bilateral_closure_remains_unusable() {
        let result = assess(
            &evidence(EyeUsability::Unusable, EyeUsability::Unusable),
            false,
        );
        assert_eq!(result.disposition(), EyeDisposition::Unusable);
        assert_eq!(result.left_eye().state, "closed");
        assert_eq!(result.right_eye().state, "closed");
    }

    #[test]
    fn head_pose_alone_does_not_excuse_bilateral_closure() {
        let mut sample = evidence(EyeUsability::Unusable, EyeUsability::Unusable);
        sample.head_pitch_degrees = Some(-20.0);

        let result = assess(&sample, false);

        assert_eq!(result.disposition(), EyeDisposition::Unusable);
    }

    #[test]
    fn downward_pose_with_residual_eye_aperture_is_a_pose_candidate() {
        let mut sample = evidence(EyeUsability::Unknown, EyeUsability::Unusable);
        sample.left_eye_aspect_ratio = Some(0.18);
        sample.right_eye_aspect_ratio = Some(0.17);
        sample.head_pitch_degrees = Some(-8.0);
        sample.blendshapes.insert("eyeBlinkLeft", 0.45);

        let result = assess(&sample, false);

        assert_eq!(
            result.disposition(),
            EyeDisposition::DeliberatePoseCandidate
        );
    }

    #[test]
    fn mild_downward_residual_aperture_regression_is_a_pose_candidate() {
        let mut sample = evidence(EyeUsability::Unusable, EyeUsability::Unusable);
        sample.left_eye_aspect_ratio = Some(0.1289);
        sample.right_eye_aspect_ratio = Some(0.1783);
        sample.head_pitch_degrees = Some(-4.87);
        sample.blendshapes.insert("eyeBlinkLeft", 0.3890);
        sample.blendshapes.insert("eyeBlinkRight", 0.3452);

        let result = assess(&sample, false);

        assert_eq!(
            result.disposition(),
            EyeDisposition::DeliberatePoseCandidate
        );
    }

    #[test]
    fn pose_boundary_does_not_excuse_a_nearly_frontal_closure() {
        let mut sample = evidence(EyeUsability::Unusable, EyeUsability::Unusable);
        sample.left_eye_aspect_ratio = Some(0.13);
        sample.right_eye_aspect_ratio = Some(0.18);
        sample.head_pitch_degrees = Some(-4.49);
        sample.blendshapes.insert("eyeBlinkLeft", 0.39);
        sample.blendshapes.insert("eyeBlinkRight", 0.35);

        let result = assess(&sample, false);

        assert_eq!(result.disposition(), EyeDisposition::Unusable);
    }

    #[test]
    fn one_closed_eye_is_unusable_without_boundary_evidence() {
        let result = assess(&evidence(EyeUsability::Open, EyeUsability::Unusable), false);
        assert_eq!(result.disposition(), EyeDisposition::Unusable);
        assert_eq!(result.left_eye().state, "open");
        assert_eq!(result.right_eye().state, "closed");
    }

    #[test]
    fn strong_profile_keeps_eye_state_unknown() {
        let result = assess(&evidence(EyeUsability::Open, EyeUsability::Open), true);

        assert_eq!(result.disposition(), EyeDisposition::Unknown);
        assert_eq!(result.left_eye().state, "unknown");
        assert_eq!(result.right_eye().state, "unknown");
        assert_eq!(result.left_eye().reason, "eye_pose_unreliable");
    }

    #[test]
    fn assessment_exposes_the_frozen_contract_without_mutators() {
        let result = assess(&evidence(EyeUsability::Open, EyeUsability::Open), false);

        assert_eq!(result.policy_version(), "qraw-eye-policy-1.1");
        assert_eq!(
            result.model_contract_version(),
            "qraw-eye-model-contract-1.0"
        );
        assert_eq!(result.disposition(), EyeDisposition::Open);
    }
}
