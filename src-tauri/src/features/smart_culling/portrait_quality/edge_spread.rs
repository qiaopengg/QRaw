//! Offline edge-spread hypothesis. This is neither CPBD nor a production gate.

use image::{DynamicImage, Luma, imageops};
use imageproc::filter::separable_filter_equal;
use serde::Serialize;

use super::edge_profile::{self, Edge};
use super::linear_response::{self, FloatGray};

pub(super) const VERSION: &str = "qraw-face-edge-spread-1";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Measurements {
    version: &'static str,
    inner_dimensions: [u32; 2],
    candidate_count: usize,
    accepted_profiles: usize,
    /// Outside, low contrast, unstable plateau, non-monotonic, missing crossing.
    rejection_counts: [usize; 5],
    width_p25: Option<f64>,
    width_median: Option<f64>,
    width_p75: Option<f64>,
    contrast_median: Option<f64>,
}

pub(super) fn measure(crop: &DynamicImage) -> Option<Measurements> {
    let (ix, iy) = (crop.width() * 15 / 100, crop.height() * 15 / 100);
    let (width, height) = (crop.width() - 2 * ix, crop.height() - 2 * iy);
    if width.min(height) < 41 {
        return None;
    }
    let gray = linear_response::from_rgb8(&crop.crop_imm(ix, iy, width, height));
    // Fixed binomial order 8 locates edges only. Width uses native image profiles.
    let smooth = separable_filter_equal(
        &gray,
        &[1., 8., 28., 56., 70., 56., 28., 8., 1.].map(|v| v / 256.0),
    );
    let gradient = |x: u32, y: u32| {
        let p = |x, y| smooth.get_pixel(x, y)[0];
        [
            (p(x + 1, y) - p(x - 1, y)) / 2.0,
            (p(x, y + 1) - p(x, y - 1)) / 2.0,
        ]
    };
    let magnitude = FloatGray::from_fn(width, height, |x, y| {
        Luma([if x == 0 || y == 0 || x + 1 == width || y + 1 == height {
            0.0
        } else {
            let [gx, gy] = gradient(x, y);
            gx.hypot(gy)
        }])
    });
    let columns = width.div_ceil(4);
    let mut cells: Vec<Option<Edge>> = (0..columns * height.div_ceil(4)).map(|_| None).collect();
    for y in 20..height - 20 {
        for x in 20..width - 20 {
            let strength = magnitude.get_pixel(x, y)[0];
            if strength < 2.0 {
                continue;
            }
            let normal = gradient(x, y).map(|v| (v / strength) as f32);
            let at = |sign: f32| {
                imageops::interpolate_bilinear(
                    &magnitude,
                    x as f32 + sign * normal[0],
                    y as f32 + sign * normal[1],
                )
                .expect("validated inner support")[0]
            };
            if strength < at(-1.0) || strength <= at(1.0) {
                continue;
            }
            let cell = &mut cells[(y / 4 * columns + x / 4) as usize];
            if cell
                .as_ref()
                .is_none_or(|previous| strength > previous.strength)
            {
                *cell = Some(Edge {
                    x,
                    y,
                    normal,
                    strength,
                });
            }
        }
    }
    let mut candidates: Vec<_> = cells.into_iter().flatten().collect();
    candidates.sort_by(|a, b| b.strength.total_cmp(&a.strength));
    candidates.truncate(2048);
    let mut widths = Vec::new();
    let mut contrasts = Vec::new();
    let mut rejected = [0; 5];
    for edge in &candidates {
        match edge_profile::measure(&gray, edge) {
            Ok(profile) => {
                widths.push(profile.width);
                contrasts.push(profile.contrast);
            }
            Err(reason) => rejected[reason as usize] += 1,
        }
    }
    widths.sort_by(f64::total_cmp);
    contrasts.sort_by(f64::total_cmp);
    let quantile = |values: &[f64], fraction: f64| {
        (values.len() >= 6).then(|| values[((values.len() - 1) as f64 * fraction).floor() as usize])
    };
    Some(Measurements {
        version: VERSION,
        inner_dimensions: [width, height],
        candidate_count: candidates.len(),
        accepted_profiles: widths.len(),
        rejection_counts: rejected,
        width_p25: quantile(&widths, 0.25),
        width_median: quantile(&widths, 0.5),
        width_p75: quantile(&widths, 0.75),
        contrast_median: quantile(&contrasts, 0.5),
    })
}

#[path = "edge_spread_tests.rs"]
mod tests;
