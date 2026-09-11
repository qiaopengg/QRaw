//! Observed-pixel-only residual proxy. Structure leakage is measured, never assumed absent.

use anyhow::{Result, anyhow, ensure};
use image::{DynamicImage, RgbImage};
use imageproc::filter::median_filter;
use serde_json::{Value, json};

use super::{cross_channel_detail, noise_covariance, noise_decomposition, noise_sampling};

pub(super) const VERSION: &str = "qraw-observed-rgb-median-residual-1";

// No reference, labels, filenames, case metadata, or injected noise parameters enter this API.
pub(super) fn measure(pixels: &RgbImage) -> Result<Value> {
    ensure!(
        pixels.width().min(pixels.height()) >= 14,
        "observed crop too small"
    );
    let observed = DynamicImage::ImageRgb8(pixels.clone());
    // Reuse the existing robust_detail 5x5 support, preserving RGB covariance here.
    let proxy = DynamicImage::ImageRgb8(median_filter(pixels, 2, 2));
    let paired = noise_decomposition::measure(&proxy, &observed)?;
    let responses = |image| {
        cross_channel_detail::response_planes(&cross_channel_detail::rgb_channels(image))
            .ok_or_else(|| anyhow!("invalid observed noise response"))
    };
    let original = responses(&observed)?;
    let structure = responses(&proxy)?;
    let mut scales = Vec::new();
    for index in 0..2 {
        let scale = &paired["scales"][index];
        let response = &original[0][index];
        let products = (0..response.values.len())
            .map(|p| {
                let residual =
                    [0, 1, 2].map(|c| original[c][index].values[p] - structure[c][index].values[p]);
                noise_covariance::PAIRS
                    .iter()
                    .map(|&(a, b)| residual[a] * residual[b])
                    .sum::<f64>()
                    / 3.0
            })
            .collect::<Vec<_>>();
        let sampling = noise_sampling::measure(
            &products,
            (response.dimensions[0] - 6) as usize,
            (response.dimensions[1] - 6) as usize,
        )?;
        let estimated = scale["residual_common"].as_f64().unwrap();
        ensure!(
            (sampling.mean - estimated).abs() <= 1e-8 * estimated.abs().max(1.0),
            "proxy block mean mismatch"
        );
        scales.push(json!({"proxy_decomposition": scale, "sampling": sampling,
            "corrected_energy": scale["observed"].as_f64().unwrap() - estimated}));
    }
    Ok(
        json!({"version": VERSION, "input": "observedRgb8Only", "median_radius": 2,
        "block_grid": [4, 4], "inner_dimensions": original[0][0].dimensions,
        "clean_reference_used": false, "confidence_interval_validated": false, "scales": scales}),
    )
}

#[path = "observed_noise_tests.rs"]
mod tests;
