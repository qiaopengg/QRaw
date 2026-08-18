//! Shared raw evidence extraction for the isolated face-motion POC.

use std::collections::BTreeMap;

use anyhow::Result;
use image::DynamicImage;

use super::super::face_models::DetectedFace;
use super::eye_policy::{EyeMotionEvidence, EyeUsability, classify_eye, combine_eyes};
use super::roi::FaceRoi;
use super::{FaceMotionPocModels, run_blendshapes, run_face_mesh};

pub(super) struct FaceMotionEvidenceDump {
    pub roi: FaceRoi,
    pub face_presence: f32,
    pub tongue_out: f32,
    pub left_eye_aspect_ratio: Option<f32>,
    pub right_eye_aspect_ratio: Option<f32>,
    pub head_pitch_degrees: Option<f32>,
    pub head_yaw_degrees: Option<f32>,
    pub landmark_consistency_error: Option<f32>,
    pub left_eye: EyeUsability,
    pub right_eye: EyeUsability,
    pub overall_eye: EyeUsability,
    pub blendshapes: BTreeMap<&'static str, f32>,
}

pub(super) fn extract_face_motion_evidence(
    image: &DynamicImage,
    detection: &DetectedFace,
    models: &FaceMotionPocModels,
) -> Result<FaceMotionEvidenceDump> {
    let roi = FaceRoi::from_detection(detection)?;
    let canonical_roi = roi.extract(image)?;
    let mesh = run_face_mesh(&canonical_roi, &models.face_mesh)?;
    let image_landmarks = roi.project_landmarks(&mesh.landmarks)?;
    let (head_pitch_degrees, head_yaw_degrees) = dense_head_angles(&mesh.landmarks);
    let landmark_consistency_error = landmark_consistency_error(detection, &image_landmarks);
    let blendshapes = run_blendshapes(&image_landmarks, &models.face_blendshapes)?
        .into_iter()
        .map(|score| (score.name, score.score))
        .collect::<BTreeMap<_, _>>();
    let left_eye_aspect_ratio = eye_aspect_ratio(&image_landmarks, [362, 385, 387, 263, 373, 380]);
    let right_eye_aspect_ratio = eye_aspect_ratio(&image_landmarks, [33, 160, 158, 133, 153, 144]);
    let left_eye = classify_eye(EyeMotionEvidence {
        aspect_ratio: left_eye_aspect_ratio,
        blink_score: blendshapes.get("eyeBlinkLeft").copied(),
    });
    let right_eye = classify_eye(EyeMotionEvidence {
        aspect_ratio: right_eye_aspect_ratio,
        blink_score: blendshapes.get("eyeBlinkRight").copied(),
    });

    Ok(FaceMotionEvidenceDump {
        roi,
        face_presence: mesh.face_presence,
        tongue_out: mesh.tongue_out,
        left_eye_aspect_ratio,
        right_eye_aspect_ratio,
        head_pitch_degrees,
        head_yaw_degrees,
        landmark_consistency_error,
        left_eye,
        right_eye,
        overall_eye: combine_eyes(left_eye, right_eye),
        blendshapes,
    })
}

fn landmark_consistency_error(detection: &DetectedFace, landmarks: &[[f32; 2]]) -> Option<f32> {
    let first_eye = centroid(landmarks, &[468, 469, 470, 471, 472])?;
    let second_eye = centroid(landmarks, &[473, 474, 475, 476, 477])?;
    let nose = *landmarks.get(1)?;
    let first_mouth = *landmarks.get(61)?;
    let second_mouth = *landmarks.get(291)?;
    let scale = distance(
        [detection.landmarks[0].0, detection.landmarks[0].1],
        [detection.landmarks[1].0, detection.landmarks[1].1],
    );
    if !scale.is_finite() || scale <= f32::EPSILON {
        return None;
    }

    let direct = correspondence_error(
        detection,
        [first_eye, second_eye, nose, first_mouth, second_mouth],
    );
    let swapped = correspondence_error(
        detection,
        [second_eye, first_eye, nose, second_mouth, first_mouth],
    );
    Some(direct.min(swapped) / scale)
}

fn correspondence_error(detection: &DetectedFace, mesh: [[f32; 2]; 5]) -> f32 {
    let squared_error = detection
        .landmarks
        .iter()
        .zip(mesh)
        .map(|(&(x, y), point)| distance([x, y], point).powi(2))
        .sum::<f32>();
    (squared_error / 5.0).sqrt()
}

fn centroid(landmarks: &[[f32; 2]], indices: &[usize]) -> Option<[f32; 2]> {
    let mut sum = [0.0, 0.0];
    for &index in indices {
        let point = *landmarks.get(index)?;
        sum[0] += point[0];
        sum[1] += point[1];
    }
    Some([sum[0] / indices.len() as f32, sum[1] / indices.len() as f32])
}

fn dense_head_angles(landmarks: &[[f32; 3]]) -> (Option<f32>, Option<f32>) {
    const FOREHEAD: usize = 10;
    const CHIN: usize = 152;
    const IMAGE_LEFT_CHEEK: usize = 234;
    const IMAGE_RIGHT_CHEEK: usize = 454;

    let (Some(&forehead), Some(&chin), Some(&left_cheek), Some(&right_cheek)) = (
        landmarks.get(FOREHEAD),
        landmarks.get(CHIN),
        landmarks.get(IMAGE_LEFT_CHEEK),
        landmarks.get(IMAGE_RIGHT_CHEEK),
    ) else {
        return (None, None);
    };
    if [forehead, chin, left_cheek, right_cheek]
        .iter()
        .flatten()
        .any(|value| !value.is_finite())
    {
        return (None, None);
    }

    let horizontal = subtract(right_cheek, left_cheek);
    let vertical = subtract(chin, forehead);
    let normal = cross(horizontal, vertical);
    let length = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
    if !length.is_finite() || length <= f32::EPSILON {
        return (None, None);
    }
    let normal = normal.map(|value| value / length);
    let forward = normal[2].abs().max(f32::EPSILON);
    (
        Some(normal[1].atan2(forward).to_degrees()),
        Some(normal[0].atan2(forward).to_degrees()),
    )
}

fn subtract(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

fn cross(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}

fn eye_aspect_ratio(landmarks: &[[f32; 2]], indices: [usize; 6]) -> Option<f32> {
    if indices.iter().any(|&index| index >= landmarks.len()) {
        return None;
    }
    let [
        outer,
        upper_outer,
        upper_inner,
        inner,
        lower_inner,
        lower_outer,
    ] = indices.map(|index| landmarks[index]);
    let width = distance(outer, inner);
    (width > f32::EPSILON).then(|| {
        (distance(upper_outer, lower_outer) + distance(upper_inner, lower_inner)) / (2.0 * width)
    })
}

fn distance(left: [f32; 2], right: [f32; 2]) -> f32 {
    (right[0] - left[0]).hypot(right[1] - left[1])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eye_aspect_ratio_is_scale_invariant() {
        let mut landmarks = vec![
            [0.0, 0.0],
            [1.0, -1.0],
            [3.0, -1.0],
            [4.0, 0.0],
            [3.0, 1.0],
            [1.0, 1.0],
        ];
        let base = eye_aspect_ratio(&landmarks, [0, 1, 2, 3, 4, 5]).unwrap();
        for point in &mut landmarks {
            point[0] *= 5.0;
            point[1] *= 5.0;
        }

        assert!((base - 0.5).abs() < 1e-6);
        assert!((eye_aspect_ratio(&landmarks, [0, 1, 2, 3, 4, 5]).unwrap() - base).abs() < 1e-6);
    }

    #[test]
    fn dense_head_angles_use_the_face_plane_normal() {
        let mut landmarks = vec![[0.0, 0.0, 0.0]; 455];
        landmarks[10] = [0.0, -1.0, 0.0];
        landmarks[152] = [0.0, 1.0, 0.0];
        landmarks[234] = [-1.0, 0.0, 0.0];
        landmarks[454] = [1.0, 0.0, 0.0];

        let (pitch, yaw) = dense_head_angles(&landmarks);

        assert!(pitch.unwrap().abs() < 1e-6);
        assert!(yaw.unwrap().abs() < 1e-6);
    }

    #[test]
    fn dense_head_angles_reject_degenerate_geometry() {
        assert_eq!(dense_head_angles(&[[0.0, 0.0, 0.0]; 10]), (None, None));
        assert_eq!(dense_head_angles(&[[0.0, 0.0, 0.0]; 455]), (None, None));
    }

    #[test]
    fn landmark_consistency_is_normalized_by_eye_distance() {
        let detection = DetectedFace {
            bbox: [0.0, 0.0, 40.0, 40.0],
            score: 1.0,
            landmarks: [
                (10.0, 10.0),
                (30.0, 10.0),
                (20.0, 20.0),
                (15.0, 30.0),
                (25.0, 30.0),
            ],
        };
        let mut landmarks = vec![[0.0, 0.0]; 478];
        landmarks[468..=472].fill([10.0, 10.0]);
        landmarks[473..=477].fill([30.0, 10.0]);
        landmarks[1] = [20.0, 20.0];
        landmarks[61] = [15.0, 30.0];
        landmarks[291] = [25.0, 30.0];

        assert!(landmark_consistency_error(&detection, &landmarks).unwrap() < 1e-6);
        landmarks[1] = [30.0, 20.0];
        assert!(landmark_consistency_error(&detection, &landmarks).unwrap() > 0.20);
    }
}
