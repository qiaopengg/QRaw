//! Head-pose geometry derived from the five YuNet landmarks.
//!
//! Scope note: this module deliberately does **not** classify expression.
//! YuNet only reports the two eye centres, the nose tip and the two mouth
//! corners, with no upper/lower lip points, so smile-versus-neutral cannot be
//! recovered from it. Claiming otherwise would put an unvalidated signal into
//! the rating pipeline, which the release rules forbid.
//!
//! The five points provide only coarse head-orientation evidence. Eye state is
//! currently always unknown; this estimate distinguishes a visibly turned face
//! from a frontal face whose eye-model input contract is still unvalidated. It
//! does not create an eye-state score.

/// Below this inter-ocular distance the landmarks are too coarse for the
/// orientation estimate to mean anything.
const MIN_USABLE_INTEROCULAR: f32 = 10.0;

/// Normalised nose offset used only to select the unavailable-evidence reason.
/// It never creates a rating signal or penalty. The exact boundary still needs
/// the frozen real-photo set (`DATA-01`) before it can drive scoring.
const STRONG_PROFILE_YAW_RATIO: f32 = 0.55;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FacePose {
    /// 0.0 for a frontal head, approaching 1.0 as it turns towards profile.
    pub yaw_ratio: f32,
    /// In-plane rotation of the eye line, in degrees.
    pub roll_degrees: f32,
    /// False when the landmarks are degenerate or the face is too small.
    pub usable: bool,
}

impl FacePose {
    /// Unusable geometry must not imply "frontal"; callers treat this as
    /// "no orientation evidence" rather than as a good frontal face.
    fn unusable() -> Self {
        Self {
            yaw_ratio: 0.0,
            roll_degrees: 0.0,
            usable: false,
        }
    }

    /// True only when the geometry is usable *and* clearly off-axis.
    pub fn suppresses_eye_state(&self) -> bool {
        self.usable && self.yaw_ratio >= STRONG_PROFILE_YAW_RATIO
    }
}

/// Estimates head orientation from the landmark set.
///
/// Landmark order follows YuNet: right eye, left eye, nose tip, right mouth
/// corner, left mouth corner. The nose offset is projected onto the eye axis so
/// in-plane roll does not leak into the yaw measurement.
pub fn estimate_pose(landmarks: &[(f32, f32); 5]) -> FacePose {
    if landmarks
        .iter()
        .flat_map(|point| [point.0, point.1])
        .any(|value| !value.is_finite())
    {
        return FacePose::unusable();
    }

    let right_eye = landmarks[0];
    let left_eye = landmarks[1];
    let nose = landmarks[2];

    let eye_dx = left_eye.0 - right_eye.0;
    let eye_dy = left_eye.1 - right_eye.1;
    let interocular = (eye_dx * eye_dx + eye_dy * eye_dy).sqrt();
    if !interocular.is_finite() || interocular < MIN_USABLE_INTEROCULAR {
        return FacePose::unusable();
    }

    let axis_x = eye_dx / interocular;
    let axis_y = eye_dy / interocular;
    let eye_mid = (
        (right_eye.0 + left_eye.0) / 2.0,
        (right_eye.1 + left_eye.1) / 2.0,
    );
    let nose_offset_along_axis = (nose.0 - eye_mid.0) * axis_x + (nose.1 - eye_mid.1) * axis_y;

    FacePose {
        // A frontal nose sits on the eye-line midpoint. Scaling by two maps a
        // half-interocular sideways shift onto the top of the range.
        yaw_ratio: ((nose_offset_along_axis.abs() / interocular) * 2.0).clamp(0.0, 1.0),
        roll_degrees: eye_dy.atan2(eye_dx).to_degrees(),
        usable: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Frontal face: eyes level, nose centred between them.
    fn frontal() -> [(f32, f32); 5] {
        [
            (100.0, 100.0),
            (160.0, 100.0),
            (130.0, 130.0),
            (110.0, 160.0),
            (150.0, 160.0),
        ]
    }

    #[test]
    fn a_frontal_face_reports_no_yaw() {
        let pose = estimate_pose(&frontal());

        assert!(pose.usable);
        assert!(pose.yaw_ratio < 0.05, "yaw was {}", pose.yaw_ratio);
        assert!(pose.roll_degrees.abs() < 0.5);
        assert!(!pose.suppresses_eye_state());
    }

    #[test]
    fn a_turned_head_is_detected() {
        let mut landmarks = frontal();
        // Nose shifted well towards one eye: the head is turned.
        landmarks[2].0 = 152.0;

        let pose = estimate_pose(&landmarks);

        assert!(pose.usable);
        assert!(pose.yaw_ratio > STRONG_PROFILE_YAW_RATIO);
        assert!(pose.suppresses_eye_state());
    }

    #[test]
    fn roll_does_not_leak_into_the_yaw_estimate() {
        // Same face rotated 90 degrees in-plane: eyes vertical, nose still on
        // the eye-line midpoint, so yaw must stay near zero.
        let landmarks = [
            (100.0, 100.0),
            (100.0, 160.0),
            (70.0, 130.0),
            (40.0, 110.0),
            (40.0, 150.0),
        ];

        let pose = estimate_pose(&landmarks);

        assert!(pose.usable);
        assert!(pose.yaw_ratio < 0.05, "yaw was {}", pose.yaw_ratio);
        assert!((pose.roll_degrees.abs() - 90.0).abs() < 0.5);
        assert!(!pose.suppresses_eye_state());
    }

    #[test]
    fn tiny_or_degenerate_faces_report_unusable_instead_of_frontal() {
        let mut small = frontal();
        small[1].0 = small[0].0 + 4.0;
        let small_pose = estimate_pose(&small);
        assert!(!small_pose.usable);
        assert!(
            !small_pose.suppresses_eye_state(),
            "missing geometry must not be reported as a strong profile"
        );

        let mut broken = frontal();
        broken[2].1 = f32::NAN;
        assert!(!estimate_pose(&broken).usable);
    }
}
