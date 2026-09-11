//! Offline 0.3 hypothesis. Linear filter noise correction is not a focus verdict.

use image::DynamicImage;
use serde::Serialize;

use super::linear_response::{self, Response};

pub(super) const VERSION: &str = "qraw-face-linear-noise-detail-1";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LinearDetailMeasurements {
    version: &'static str,
    noise_sigma: f64,
    inner_dimensions: [u32; 2],
    sample_count: u64,
    scales: [ScaleEnergy; 2],
    coarse_to_fine_ratio: Option<f64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ScaleEnergy {
    smoothing_order: usize,
    kernel_noise_gain: f64,
    observed_energy: f64,
    expected_noise_energy: f64,
    /// Signed estimate: do not clamp over-subtraction into a blur observation.
    corrected_energy: f64,
}

pub(super) fn measure(crop: &DynamicImage, noise_sigma: f64) -> Option<LinearDetailMeasurements> {
    from_responses(
        &linear_response::planes(&linear_response::from_rgb8(crop))?,
        noise_sigma,
    )
}

pub(super) fn from_responses(
    planes: &[Response; 2],
    noise_sigma: f64,
) -> Option<LinearDetailMeasurements> {
    if !(noise_sigma * noise_sigma).is_finite() || noise_sigma < 0.0 {
        return None;
    }
    let scales = [0, 1].map(|index| {
        let response = &planes[index];
        let kernel_noise_gain = response.noise_gain;
        let observed_energy = linear_response::energy(&response.values);
        let expected_noise_energy = noise_sigma * noise_sigma * kernel_noise_gain;
        ScaleEnergy {
            smoothing_order: response.smoothing_order,
            kernel_noise_gain,
            observed_energy,
            expected_noise_energy,
            corrected_energy: observed_energy - expected_noise_energy,
        }
    });
    let fine = scales[0].corrected_energy;
    let coarse = scales[1].corrected_energy;
    Some(LinearDetailMeasurements {
        version: VERSION,
        noise_sigma,
        inner_dimensions: planes[0].dimensions,
        sample_count: planes[0].values.len() as u64,
        scales,
        coarse_to_fine_ratio: (fine > 0.0 && coarse > 0.0).then(|| coarse / fine),
    })
}

#[path = "linear_detail_tests.rs"]
mod tests;
