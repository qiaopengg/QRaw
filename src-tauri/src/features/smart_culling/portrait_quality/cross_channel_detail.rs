//! Offline hypothesis: common RGB response energy, not a calibrated clarity gate.

use image::{DynamicImage, Luma};
use serde::Serialize;

use super::linear_response::{self, FloatGray};

pub(super) const VERSION: &str = "qraw-face-cross-channel-detail-1";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Measurements {
    version: &'static str,
    inner_dimensions: [u32; 2],
    sample_count: usize,
    pub scales: [ScaleEnergy; 2],
    coarse_to_fine_ratio: Option<f64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ScaleEnergy {
    smoothing_order: usize,
    channel_energies: [f64; 3],
    /// RG, RB, GB signed products. Correlated noise can survive this operation.
    pair_energies: [f64; 3],
    pub common_energy: f64,
}

pub(super) fn measure(crop: &DynamicImage) -> Option<Measurements> {
    measure_channels(&rgb_channels(crop))
}

pub(super) fn rgb_channels(crop: &DynamicImage) -> [FloatGray; 3] {
    let rgb = crop.to_rgb8();
    [0, 1, 2].map(|channel| {
        FloatGray::from_fn(rgb.width(), rgb.height(), |x, y| {
            Luma([f64::from(rgb.get_pixel(x, y)[channel])])
        })
    })
}

pub(super) fn response_planes(
    channels: &[FloatGray; 3],
) -> Option<[[linear_response::Response; 2]; 3]> {
    if channels
        .iter()
        .any(|plane| plane.dimensions() != channels[0].dimensions())
    {
        return None;
    }
    Some([
        linear_response::planes(&channels[0])?,
        linear_response::planes(&channels[1])?,
        linear_response::planes(&channels[2])?,
    ])
}

pub(super) fn measure_channels(channels: &[FloatGray; 3]) -> Option<Measurements> {
    let channels = response_planes(channels)?;
    let count = channels[0][0].values.len();
    let scales = [0, 1].map(|scale| {
        let pair_energies = [(0, 1), (0, 2), (1, 2)].map(|(left, right)| {
            channels[left][scale]
                .values
                .iter()
                .zip(&channels[right][scale].values)
                .map(|(a, b)| a * b)
                .sum::<f64>()
                / count as f64
        });
        ScaleEnergy {
            smoothing_order: channels[0][scale].smoothing_order,
            channel_energies: [0, 1, 2]
                .map(|channel| linear_response::energy(&channels[channel][scale].values)),
            common_energy: pair_energies.iter().sum::<f64>() / 3.0,
            pair_energies,
        }
    });
    let fine = scales[0].common_energy;
    let coarse = scales[1].common_energy;
    Some(Measurements {
        version: VERSION,
        inner_dimensions: channels[0][0].dimensions,
        sample_count: count,
        scales,
        coarse_to_fine_ratio: (fine > 0.0 && coarse > 0.0).then(|| coarse / fine),
    })
}

#[path = "cross_channel_detail_tests.rs"]
mod tests;
