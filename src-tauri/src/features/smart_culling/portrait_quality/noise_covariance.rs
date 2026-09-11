//! Finite-window same-position terms; off-position terms also include sampling variation.

use anyhow::{Result, anyhow, ensure};
use serde::Serialize;

use super::linear_response::{self, FloatGray, Response};

pub(super) const PAIRS: [(usize, usize); 3] = [(0, 1), (0, 2), (1, 2)];

#[derive(Serialize)]
pub(super) struct SpatialMoments {
    pub roi_means: [f64; 3],
    pub zero_lag_pairs: [f64; 3],
    pub same_position: [[f64; 3]; 2],
}

pub(super) fn measure(
    residual: &[FloatGray; 3],
    responses: &[Response; 2],
) -> Result<SpatialMoments> {
    ensure!(
        residual
            .iter()
            .all(|plane| plane.dimensions() == residual[0].dimensions()),
        "mismatched residual input dimensions"
    );
    let mut centered = [
        linear_response::inner(&residual[0]),
        linear_response::inner(&residual[1]),
        linear_response::inner(&residual[2]),
    ]
    .into_iter()
    .collect::<Option<Vec<_>>>()
    .ok_or_else(|| anyhow!("invalid residual ROI"))?;
    ensure!(
        centered
            .iter()
            .all(|plane| plane.dimensions() == centered[0].dimensions()),
        "mismatched residual channels"
    );
    let (width, height) = centered[0].dimensions();
    ensure!(
        responses.iter().all(|r| r.dimensions == [width, height]
            && r.values.len() == ((width - 6) * (height - 6)) as usize),
        "mismatched response window"
    );
    let roi_means = [0, 1, 2].map(|channel| {
        let values = centered[channel].as_raw();
        values.iter().sum::<f64>() / values.len() as f64
    });
    for (plane, mean) in centered.iter_mut().zip(roi_means) {
        for pixel in plane.pixels_mut() {
            pixel[0] -= mean;
        }
    }
    let zero_lag_pairs = PAIRS.map(|(a, b)| {
        centered[a]
            .pixels()
            .zip(centered[b].pixels())
            .map(|(a, b)| a[0] * b[0])
            .sum::<f64>()
            / centered[a].as_raw().len() as f64
    });
    Ok(SpatialMoments {
        roi_means,
        zero_lag_pairs,
        same_position: [
            diagonal(&centered, &responses[0]),
            diagonal(&centered, &responses[1]),
        ],
    })
}

fn diagonal(centered: &[FloatGray], response: &Response) -> [f64; 3] {
    let (width, height) = centered[0].dimensions();
    let stride = (width + 1) as usize;
    // Each squared kernel tap contributes over a rectangle of valid response centers.
    // A difference array avoids rescanning every pixel for every tap and channel pair.
    let mut weights = vec![0.0; stride * (height + 1) as usize];
    let side = response.smoothing_order + 3;
    let radius = side / 2;
    for (index, value) in response.kernel.iter().enumerate() {
        let x0 = 3 + index % side - radius;
        let y0 = 3 + index / side - radius;
        let x1 = x0 + width as usize - 6;
        let y1 = y0 + height as usize - 6;
        for (x, y, sign) in [(x0, y0, 1.0), (x1, y0, -1.0), (x0, y1, -1.0), (x1, y1, 1.0)] {
            weights[y * stride + x] += sign * value * value;
        }
    }
    let mut result = [0.0; 3];
    for y in 0..height as usize {
        for x in 0..width as usize {
            let index = y * stride + x;
            let left = if x > 0 { weights[index - 1] } else { 0.0 };
            let above = if y > 0 { weights[index - stride] } else { 0.0 };
            let corner = if x > 0 && y > 0 {
                weights[index - stride - 1]
            } else {
                0.0
            };
            weights[index] += left + above - corner;
            for (pair, (a, b)) in PAIRS.iter().enumerate() {
                result[pair] += weights[index]
                    * centered[*a].get_pixel(x as u32, y as u32)[0]
                    * centered[*b].get_pixel(x as u32, y as u32)[0];
            }
        }
    }
    result.map(|value| value / response.values.len() as f64)
}

#[cfg(test)]
#[path = "noise_covariance_tests.rs"]
mod tests;
