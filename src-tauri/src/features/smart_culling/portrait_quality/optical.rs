//! Rendered RGB8 observations, not judgements about creative exposure or white balance.

use image::{DynamicImage, GenericImageView, Pixel};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpticalMeasurements {
    pub sample_count: u64,
    pub sample_stride: u32,
    pub shadow_clip_ratio: f64,
    pub highlight_clip_ratio: f64,
    pub channel_shadow_clip_ratio: [f64; 3],
    pub channel_highlight_clip_ratio: [f64; 3],
    pub mean_rgb: [f64; 3],
    pub luma_p05: u8,
    pub luma_p50: u8,
    pub luma_p95: u8,
    pub luma_std_dev: f64,
}

pub(super) fn measure(image: &DynamicImage) -> Option<OpticalMeasurements> {
    let (width, height) = image.dimensions();
    if width == 0 || height == 0 {
        return None;
    }
    // Deterministic sampling preserves clipped pixel values (unlike averaging
    // thumbnails). Ratios estimate the rendered frame, not a RAW histogram.
    let stride = width.max(height).div_ceil(512).max(1);
    let mut histogram = [0_u64; 256];
    let mut channel_dark = [0_u64; 3];
    let mut channel_bright = [0_u64; 3];
    let mut channel_sum = [0_u64; 3];
    let mut count = 0_u64;
    let mut luma_sum = 0.0;
    let mut luma_squared_sum = 0.0;
    for y in (0..height).step_by(stride as usize) {
        for x in (0..width).step_by(stride as usize) {
            let pixel = image.get_pixel(x, y);
            let luma = pixel.to_luma()[0];
            histogram[luma as usize] += 1;
            luma_sum += f64::from(luma);
            luma_squared_sum += f64::from(luma).powi(2);
            for channel in 0..3 {
                let value = pixel[channel];
                channel_sum[channel] += u64::from(value);
                channel_dark[channel] += u64::from(value < 5);
                channel_bright[channel] += u64::from(value >= 250);
            }
            count += 1;
        }
    }
    let denominator = count as f64;
    let percentile = |percent: u64| {
        let target = (count * percent).div_ceil(100).max(1);
        let mut cumulative = 0;
        for (value, frequency) in histogram.iter().enumerate() {
            cumulative += frequency;
            if cumulative >= target {
                return value as u8;
            }
        }
        255
    };
    Some(OpticalMeasurements {
        sample_count: count,
        sample_stride: stride,
        shadow_clip_ratio: histogram[..5].iter().sum::<u64>() as f64 / denominator,
        highlight_clip_ratio: histogram[250..].iter().sum::<u64>() as f64 / denominator,
        channel_shadow_clip_ratio: channel_dark.map(|value| value as f64 / denominator),
        channel_highlight_clip_ratio: channel_bright.map(|value| value as f64 / denominator),
        mean_rgb: channel_sum.map(|value| value as f64 / denominator),
        luma_p05: percentile(5),
        luma_p50: percentile(50),
        luma_p95: percentile(95),
        luma_std_dev: (luma_squared_sum / denominator - (luma_sum / denominator).powi(2))
            .max(0.0)
            .sqrt(),
    })
}
