//! Offline spatial edge profiles; no noise-energy subtraction or focus verdict.

use image::imageops::interpolate_bilinear;

use super::linear_response::FloatGray;

pub(super) struct Edge {
    pub x: u32,
    pub y: u32,
    pub normal: [f32; 2],
    pub strength: f64,
}

pub(super) struct Profile {
    pub width: f64,
    pub contrast: f64,
}

/// Counts are serialized in this order; rejected support is never "blurry".
pub(super) enum Rejection {
    Outside = 0,
    LowContrast,
    UnstablePlateau,
    NonMonotonic,
    MissingCrossing,
}

pub(super) fn measure(gray: &FloatGray, edge: &Edge) -> Result<Profile, Rejection> {
    let [nx, ny] = edge.normal;
    let mut sampled = [0.0; 31];
    // Reuse image's bounded bilinear sampler (f32 interpolation); accumulate f64.
    // Tangential averaging reduces noise without changing the normal pixel scale.
    for (i, value) in sampled.iter_mut().enumerate() {
        let d = i as f32 - 15.0;
        for tangent in -6..=6 {
            let x = edge.x as f32 + d * nx - tangent as f32 * ny;
            let y = edge.y as f32 + d * ny + tangent as f32 * nx;
            *value += interpolate_bilinear(gray, x, y).ok_or(Rejection::Outside)?[0];
        }
        *value /= 13.0;
    }
    let smooth: Vec<f64> = sampled
        .windows(5)
        .map(|p| (p[0] + 4.0 * p[1] + 6.0 * p[2] + 4.0 * p[3] + p[4]) / 16.0)
        .collect();
    let left = smooth[..3].iter().sum::<f64>() / 3.0;
    let right = smooth[24..].iter().sum::<f64>() / 3.0;
    let contrast = right - left;
    if !contrast.is_finite() || contrast < 24.0 {
        return Err(Rejection::LowContrast);
    }
    let tolerance = 0.2 * contrast + 4.0;
    let peak_slope = smooth.windows(2).map(|p| p[1] - p[0]).fold(0.0, f64::max);
    // A noise-displaced gradient maximum can truncate a broad edge. Require
    // both endpoint spans (two pixels) to be flatter than the transition itself.
    if smooth[..3].iter().any(|p| (p - left).abs() > tolerance)
        || smooth[24..].iter().any(|p| (p - right).abs() > tolerance)
        || (smooth[2] - smooth[0]).abs() > 0.5 * peak_slope
        || (smooth[26] - smooth[24]).abs() > 0.5 * peak_slope
    {
        return Err(Rejection::UnstablePlateau);
    }
    let reverse = smooth
        .windows(2)
        .map(|p| (p[0] - p[1]).max(0.0))
        .sum::<f64>();
    if reverse > tolerance {
        return Err(Rejection::NonMonotonic);
    }
    let crossing = |fraction: f64| {
        let threshold = left + fraction * contrast;
        smooth.windows(2).enumerate().find_map(|(i, p)| {
            (p[0] <= threshold && p[1] > threshold)
                .then(|| i as f64 + (threshold - p[0]) / (p[1] - p[0]))
        })
    };
    let width = crossing(0.9).ok_or(Rejection::MissingCrossing)?
        - crossing(0.1).ok_or(Rejection::MissingCrossing)?;
    if !width.is_finite() || width <= 0.0 {
        return Err(Rejection::MissingCrossing);
    }
    Ok(Profile { width, contrast })
}
