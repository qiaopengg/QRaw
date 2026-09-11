//! Noise-aware observations, not a calibrated pass/fail gate.

use image::{DynamicImage, GrayImage};
use imageproc::filter::median_filter;
use serde::Serialize;

use super::detail::{self, FaceDetailMeasurements};

pub(crate) const VERSION: &str = "qraw-face-robust-detail-1";

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RobustDetailMeasurements {
    pub version: &'static str,
    /// Robust high-pass scale estimate, not a camera ISO/noise probability.
    pub noise_sigma: f64,
    pub median_radius: u32,
    pub denoised_detail: FaceDetailMeasurements,
}

pub(super) fn measure(crop: &DynamicImage) -> Option<RobustDetailMeasurements> {
    let gray = crop.to_luma8();
    if gray.width().min(gray.height()) < 7 {
        return None;
    }
    // Median before downsampling: do not first turn pixel noise into coarse texture.
    // Fixed 5x5 support; this is a new observation version, not the old thresholds.
    let filtered = DynamicImage::ImageLuma8(median_filter(&gray, 2, 2));
    Some(RobustDetailMeasurements {
        version: VERSION,
        noise_sigma: noise_sigma(&gray),
        median_radius: 2,
        denoised_detail: detail::measure(&filtered)?,
    })
}

fn noise_sigma(gray: &GrayImage) -> f64 {
    noise_sigma_from_samples(gray.width(), gray.height(), |x, y| {
        f64::from(gray.get_pixel(x, y)[0])
    })
    .expect("validated finite grayscale crop")
}

pub(super) fn noise_sigma_from_samples(
    width: u32,
    height: u32,
    sample: impl Fn(u32, u32) -> f64,
) -> Option<f64> {
    if width.min(height) < 3 {
        return None;
    }
    let mut residuals = Vec::with_capacity(((width - 2) * (height - 2)) as usize);
    for y in 1..height - 1 {
        for x in 1..width - 1 {
            let p = |dx: i32, dy: i32| sample((x as i32 + dx) as u32, (y as i32 + dy) as u32);
            let response = p(-1, -1) - 2.0 * p(0, -1) + p(1, -1) - 2.0 * p(-1, 0) + 4.0 * p(0, 0)
                - 2.0 * p(1, 0)
                + p(-1, 1)
                - 2.0 * p(0, 1)
                + p(1, 1);
            if !response.is_finite() {
                return None;
            }
            residuals.push(response.abs());
        }
    }
    let middle = residuals.len() / 2;
    let (_, median, _) = residuals.select_nth_unstable_by(middle, f64::total_cmp);
    // Kernel squared weights sum to 36. For independent Gaussian noise,
    // median(abs(response)) = 0.67448975 * 6 * sigma; texture can bias it.
    Some(*median / (0.674_489_75 * 6.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Luma;

    #[test]
    fn flat_and_tiny_inputs_do_not_invent_detail() {
        let flat = DynamicImage::ImageLuma8(GrayImage::from_pixel(40, 40, Luma([128])));
        let measured = measure(&flat).unwrap();
        assert_eq!(measured.noise_sigma, 0.0);
        assert_eq!(measured.denoised_detail.native_laplacian, 0.0);
        assert!(measured.denoised_detail.blur_retention_ratio.is_none());
        assert!(measure(&DynamicImage::new_luma8(6, 80)).is_none());
    }

    #[test]
    fn isolated_impulses_are_not_promoted_to_structure() {
        let image = DynamicImage::ImageLuma8(GrayImage::from_fn(100, 100, |x, y| {
            Luma([if x % 7 == 0 && y % 7 == 0 { 255 } else { 128 }])
        }));
        assert!(detail::measure(&image).unwrap().native_laplacian > 0.0);
        assert_eq!(
            measure(&image).unwrap().denoised_detail.native_laplacian,
            0.0
        );
    }
}
