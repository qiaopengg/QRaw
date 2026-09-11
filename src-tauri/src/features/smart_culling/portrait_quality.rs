//! Measured portrait-quality evidence. No uncalibrated score changes a rating.

use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Result, anyhow};
use image::{DynamicImage, GenericImageView};
use serde::Serialize;

use super::types::FaceResult;

mod clarity_observation;
#[cfg(test)]
mod cross_channel_detail;
#[cfg(test)]
mod decomposition_trace;
#[cfg(test)]
mod degradation_cases;
#[cfg(test)]
mod degradation_intermediates;
mod detail;
#[cfg(test)]
mod edge_profile;
#[cfg(test)]
mod edge_spread;
mod face;
#[cfg(test)]
mod linear_detail;
#[cfg(test)]
mod linear_response;
#[cfg(test)]
mod mechanism_energy;
#[cfg(test)]
mod mechanism_samples;
#[cfg(any(test, all(debug_assertions, target_os = "macos")))]
pub(crate) mod model;
#[cfg(test)]
mod noise_covariance;
#[cfg(test)]
mod noise_decomposition;
#[cfg(test)]
mod noise_mechanism;
#[cfg(test)]
mod noise_sampling;
#[cfg(test)]
mod observed_noise;
mod optical;
mod robust_detail;
#[cfg(test)]
mod stress_variants;

pub(crate) const OBSERVATION_VERSION: &str = "qraw-portrait-quality-observation-1";

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortraitQualityObservation {
    pub version: &'static str,
    pub model_version: &'static str,
    /// Explicitly not original RAW pixels; edits/resampling can change these measurements.
    pub input_source: &'static str,
    pub input_dimensions: [u32; 2],
    pub optical: Option<optical::OpticalMeasurements>,
    pub faces: Vec<FaceQualityObservation>,
    pub subject_face_indices: Vec<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clarity: Option<clarity_observation::ClarityObservation>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FaceQualityObservation {
    pub face_index: usize,
    pub crop_dimensions: Option<[u32; 2]>,
    pub detail: Option<detail::FaceDetailMeasurements>,
    pub optical: Option<optical::OpticalMeasurements>,
    pub placement: Option<face::FacePlacementMeasurements>,
    pub yaw_ratio: Option<f32>,
    pub roll_degrees: Option<f32>,
    /// Raw FIQA utility regressor, not a probability or a focus verdict.
    pub quality_score: Option<f32>,
    pub quality_unavailable_reason: Option<QualityUnavailableReason>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum QualityUnavailableReason {
    InvalidFaceBox,
    PartialFace,
    FaceTooSmall,
    UnreliableDetection,
    LandmarksUnavailable,
    StrongProfile,
    ModelUnavailable,
    InferenceFailed,
}

pub(crate) fn observe(
    image: &DynamicImage,
    faces: &[FaceResult],
    cancellation: &AtomicBool,
) -> Result<PortraitQualityObservation> {
    let mut observations = Vec::with_capacity(faces.len());
    for (index, detected) in faces.iter().enumerate() {
        if cancellation.load(Ordering::Acquire) {
            return Err(anyhow!("portrait quality observation cancelled"));
        }
        observations.push(face::observe(image, detected, index));
    }
    if cancellation.load(Ordering::Acquire) {
        return Err(anyhow!("portrait quality observation cancelled"));
    }
    let (width, height) = image.dimensions();
    Ok(PortraitQualityObservation {
        version: OBSERVATION_VERSION,
        model_version: "ediffiqa-tiny-jun2024-qraw-observation-1",
        input_source: "renderedRgb8",
        input_dimensions: [width, height],
        optical: optical::measure(image),
        faces: observations,
        // Filled by the existing subject selector when constructing the review DTO.
        subject_face_indices: Vec::new(),
        clarity: None,
    })
}

pub(crate) fn observe_with_clarity(
    image: &DynamicImage,
    clarity: &super::infrastructure::ClarityInput,
    faces: &[FaceResult],
    cancellation: &AtomicBool,
) -> Result<PortraitQualityObservation> {
    let mut observation = observe(image, faces, cancellation)?;
    observation.version = "qraw-portrait-quality-observation-2";
    observation.clarity = Some(clarity_observation::observe(clarity, faces, cancellation)?);
    Ok(observation)
}

pub(super) fn for_review(
    item: &super::scoring::AnalysisCandidate,
) -> Option<PortraitQualityObservation> {
    let mut observation = item.portrait_quality.clone()?;
    // Reuse the actual portrait subject selection (largest face or confirmed
    // key people), not a second "important face" approximation in the UI.
    observation.subject_face_indices = super::mode_scoring::portrait_subjects(item)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|subject| {
            item.faces
                .iter()
                .position(|face| std::ptr::eq(face, subject))
        })
        .collect();
    Some(observation)
}

#[cfg(all(test, debug_assertions, target_os = "macos"))]
mod calibration_export;
#[cfg(test)]
mod tests;
