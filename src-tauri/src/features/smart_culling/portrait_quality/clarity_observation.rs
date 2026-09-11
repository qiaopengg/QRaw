//! Native clarity evidence stays separate from rendered FIQA/optical evidence.

use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Result, anyhow};
use image::DynamicImage;
use serde::Serialize;

use super::super::{
    infrastructure::{ClarityInput, ClarityInputUnavailable},
    types::{FaceResult, MIN_RELIABLE_FACE_DETECTION_SCORE},
};
use super::{QualityUnavailableReason, detail, face, robust_detail};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClarityObservation {
    pub version: &'static str,
    pub input_source: &'static str,
    pub input_dimensions: Option<[u32; 2]>,
    pub source_sha256: Option<String>,
    pub adjustments_sha256: Option<String>,
    pub unavailable_reason: Option<ClarityInputUnavailable>,
    pub faces: Vec<FaceClarityObservation>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FaceClarityObservation {
    pub face_index: usize,
    pub bbox: [f32; 4],
    pub detail: Option<detail::FaceDetailMeasurements>,
    pub robust_detail: Option<robust_detail::RobustDetailMeasurements>,
    pub unavailable_reason: Option<QualityUnavailableReason>,
}

pub(super) fn observe_face(
    image: &DynamicImage,
    detected: &FaceResult,
    index: usize,
) -> FaceClarityObservation {
    let mut result = FaceClarityObservation {
        face_index: index,
        bbox: detected.bbox,
        detail: None,
        robust_detail: None,
        unavailable_reason: None,
    };
    let crop = face::validated_crop(image, detected.bbox).and_then(|crop| {
        if !detected.detection_score.is_finite()
            || detected.detection_score < MIN_RELIABLE_FACE_DETECTION_SCORE
        {
            Err(QualityUnavailableReason::UnreliableDetection)
        } else if crop.width().min(crop.height()) < 32 {
            Err(QualityUnavailableReason::FaceTooSmall)
        } else {
            Ok(crop)
        }
    });
    match crop {
        Ok(crop) => {
            result.detail = detail::measure(&crop);
            result.robust_detail = robust_detail::measure(&crop);
        }
        Err(reason) => result.unavailable_reason = Some(reason),
    }
    result
}

pub(super) fn observe(
    input: &ClarityInput,
    faces: &[FaceResult],
    cancellation: &AtomicBool,
) -> Result<ClarityObservation> {
    let mut observations = Vec::new();
    if let Some(image) = input.image() {
        for (index, detected) in faces.iter().enumerate() {
            if cancellation.load(Ordering::Acquire) {
                return Err(anyhow!("portrait clarity observation cancelled"));
            }
            observations.push(observe_face(image, detected, index));
        }
    }
    if cancellation.load(Ordering::Acquire) {
        return Err(anyhow!("portrait clarity observation cancelled"));
    }
    Ok(ClarityObservation {
        version: "qraw-clarity-input-1",
        input_source: "decodedRgb8LosslessGeometry",
        input_dimensions: input.image().map(|image| [image.width(), image.height()]),
        source_sha256: input.source_sha256.clone(),
        adjustments_sha256: input.adjustments_sha256.clone(),
        unavailable_reason: input.unavailable_reason(),
        faces: observations,
    })
}
