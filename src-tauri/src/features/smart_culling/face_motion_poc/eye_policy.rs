//! Frozen eye-usability policy for the isolated real-photo inference path.
//!
//! A decided state requires the dense eyelid geometry and the independently
//! inferred blink coefficient to agree. Disagreement remains unknown instead
//! of being forced into open or unusable. Any threshold or semantic change
//! requires a policy-version bump and a fresh blind regression run.

pub(super) const EYE_POLICY_VERSION: &str = "qraw-eye-policy-1.0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum EyeUsability {
    Open,
    Unusable,
    Unknown,
}

impl EyeUsability {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Unusable => "unusable",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct EyeMotionEvidence {
    pub aspect_ratio: Option<f32>,
    pub blink_score: Option<f32>,
}

// Frozen by EYE_POLICY_VERSION. Do not tune these for expression, optical, or
// composition work; those features consume the decision without changing it.
const UNUSABLE_MAX_ASPECT_RATIO: f32 = 0.18;
const UNUSABLE_MIN_BLINK_SCORE: f32 = 0.30;
const OPEN_MIN_ASPECT_RATIO: f32 = 0.20;
const OPEN_MAX_BLINK_SCORE: f32 = 0.35;

pub(super) fn classify_eye(evidence: EyeMotionEvidence) -> EyeUsability {
    let (Some(aspect_ratio), Some(blink_score)) = (evidence.aspect_ratio, evidence.blink_score)
    else {
        return EyeUsability::Unknown;
    };
    if !aspect_ratio.is_finite() || !blink_score.is_finite() {
        return EyeUsability::Unknown;
    }

    if aspect_ratio <= UNUSABLE_MAX_ASPECT_RATIO && blink_score >= UNUSABLE_MIN_BLINK_SCORE {
        EyeUsability::Unusable
    } else if aspect_ratio >= OPEN_MIN_ASPECT_RATIO && blink_score <= OPEN_MAX_BLINK_SCORE {
        EyeUsability::Open
    } else {
        EyeUsability::Unknown
    }
}

pub(super) fn combine_eyes(left: EyeUsability, right: EyeUsability) -> EyeUsability {
    if left == EyeUsability::Unusable || right == EyeUsability::Unusable {
        EyeUsability::Unusable
    } else if left == EyeUsability::Open && right == EyeUsability::Open {
        EyeUsability::Open
    } else {
        EyeUsability::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn evidence(aspect_ratio: f32, blink_score: f32) -> EyeMotionEvidence {
        EyeMotionEvidence {
            aspect_ratio: Some(aspect_ratio),
            blink_score: Some(blink_score),
        }
    }

    #[test]
    fn unusable_requires_geometry_and_blink_agreement() {
        assert_eq!(classify_eye(evidence(0.10, 0.80)), EyeUsability::Unusable);
        assert_eq!(classify_eye(evidence(0.10, 0.20)), EyeUsability::Unknown);
        assert_eq!(classify_eye(evidence(0.25, 0.80)), EyeUsability::Unknown);
    }

    #[test]
    fn open_requires_geometry_and_blink_agreement() {
        assert_eq!(classify_eye(evidence(0.25, 0.10)), EyeUsability::Open);
        assert_eq!(classify_eye(evidence(0.19, 0.10)), EyeUsability::Unknown);
        assert_eq!(classify_eye(evidence(0.25, 0.40)), EyeUsability::Unknown);
    }

    #[test]
    fn one_unusable_eye_makes_the_portrait_unusable() {
        assert_eq!(
            combine_eyes(EyeUsability::Open, EyeUsability::Unusable),
            EyeUsability::Unusable
        );
    }

    #[test]
    fn incomplete_eye_evidence_stays_unknown() {
        assert_eq!(
            classify_eye(EyeMotionEvidence {
                aspect_ratio: None,
                blink_score: Some(0.9),
            }),
            EyeUsability::Unknown
        );
        assert_eq!(
            combine_eyes(EyeUsability::Open, EyeUsability::Unknown),
            EyeUsability::Unknown
        );
    }

    #[test]
    fn policy_version_and_threshold_boundaries_are_frozen() {
        assert_eq!(EYE_POLICY_VERSION, "qraw-eye-policy-1.0");
        assert_eq!(UNUSABLE_MAX_ASPECT_RATIO, 0.18);
        assert_eq!(UNUSABLE_MIN_BLINK_SCORE, 0.30);
        assert_eq!(OPEN_MIN_ASPECT_RATIO, 0.20);
        assert_eq!(OPEN_MAX_BLINK_SCORE, 0.35);

        assert_eq!(classify_eye(evidence(0.18, 0.30)), EyeUsability::Unusable);
        assert_eq!(classify_eye(evidence(0.20, 0.35)), EyeUsability::Open);
        assert_eq!(classify_eye(evidence(0.19, 0.34)), EyeUsability::Unknown);
    }
}
