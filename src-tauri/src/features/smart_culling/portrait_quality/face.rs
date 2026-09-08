use image::{DynamicImage, GenericImageView};
use serde::Serialize;

use super::super::{
    analysis::crop_pixel_bbox,
    face_geometry::estimate_pose,
    types::{FaceResult, MIN_RELIABLE_FACE_DETECTION_SCORE},
};
use super::{FaceQualityObservation, QualityUnavailableReason, detail, optical};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FacePlacementMeasurements {
    pub center: [f32; 2],
    pub frame_area_ratio: f32,
    pub top_margin_ratio: f32,
    pub center_distance: f32,
    pub thirds_distance: f32,
    pub golden_distance: f32,
}

pub(super) fn observe(
    image: &DynamicImage,
    face: &FaceResult,
    index: usize,
) -> FaceQualityObservation {
    use QualityUnavailableReason as Reason;
    let mut result = FaceQualityObservation {
        face_index: index,
        crop_dimensions: None,
        detail: None,
        optical: None,
        placement: None,
        yaw_ratio: None,
        roll_degrees: None,
        quality_score: None,
        quality_unavailable_reason: Some(Reason::InvalidFaceBox),
    };
    let [x, y, width, height] = face.bbox;
    let (frame_width, frame_height) = image.dimensions();
    if face.bbox.iter().any(|value| !value.is_finite())
        || width <= 0.0
        || height <= 0.0
        || x + width <= 0.0
        || y + height <= 0.0
        || x >= frame_width as f32
        || y >= frame_height as f32
    {
        return result;
    }
    if x < 0.0 || y < 0.0 || x + width > frame_width as f32 || y + height > frame_height as f32 {
        result.quality_unavailable_reason = Some(Reason::PartialFace);
        return result;
    }
    let Some(crop) = crop_pixel_bbox(image, face.bbox) else {
        return result;
    };
    result.crop_dimensions = Some([crop.width(), crop.height()]);
    result.detail = detail::measure(&crop);
    result.optical = optical::measure(&crop);
    result.placement = Some(placement(face.bbox, frame_width, frame_height));
    let reason = if !face.detection_score.is_finite()
        || face.detection_score < MIN_RELIABLE_FACE_DETECTION_SCORE
    {
        Some(Reason::UnreliableDetection)
    } else if crop.width().min(crop.height()) < 32 {
        // An observation input floor, not an absolute clarity threshold.
        Some(Reason::FaceTooSmall)
    } else if face.landmarks.iter().any(|&(px, py)| {
        !px.is_finite()
            || !py.is_finite()
            || px < x
            || py < y
            || px >= x + width
            || py >= y + height
    }) {
        Some(Reason::LandmarksUnavailable)
    } else {
        let pose = estimate_pose(&face.landmarks);
        if !pose.usable {
            Some(Reason::LandmarksUnavailable)
        } else {
            result.yaw_ratio = Some(pose.yaw_ratio);
            result.roll_degrees = Some(pose.roll_degrees);
            pose.suppresses_eye_state().then_some(Reason::StrongProfile)
        }
    };
    result.quality_unavailable_reason = reason;
    if reason.is_some() {
        return result;
    }
    #[cfg(all(debug_assertions, target_os = "macos"))]
    if super::model::is_available() {
        match super::model::observe(image, &face.landmarks) {
            Ok(score) => result.quality_score = Some(score),
            Err(error) => {
                log::warn!("Portrait face quality observation failed: {error}");
                result.quality_unavailable_reason = Some(Reason::InferenceFailed);
            }
        }
        return result;
    }
    result.quality_unavailable_reason = Some(Reason::ModelUnavailable);
    result
}

fn placement(bbox: [f32; 4], width: u32, height: u32) -> FacePlacementMeasurements {
    let center = [
        (bbox[0] + bbox[2] / 2.0) / width as f32,
        (bbox[1] + bbox[3] / 2.0) / height as f32,
    ];
    let distance = |x: f32, y: f32| (center[0] - x).hypot(center[1] - y);
    let grid_distance = |a: f32, b: f32| {
        [
            distance(a, a),
            distance(a, b),
            distance(b, a),
            distance(b, b),
        ]
        .into_iter()
        .fold(f32::INFINITY, f32::min)
    };
    FacePlacementMeasurements {
        center,
        frame_area_ratio: (bbox[2] / width as f32) * (bbox[3] / height as f32),
        top_margin_ratio: bbox[1] / height as f32,
        center_distance: distance(0.5, 0.5),
        thirds_distance: grid_distance(1.0 / 3.0, 2.0 / 3.0),
        golden_distance: grid_distance(0.381_966_02, 0.618_034),
    }
}
