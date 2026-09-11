//! Shared offline linear responses; preserves the fixed 0.3 ROI and kernels.

use image::{DynamicImage, ImageBuffer, Luma, imageops};
use imageproc::filter::separable_filter_equal;

pub(super) type FloatGray = ImageBuffer<Luma<f64>, Vec<f64>>;
const FINE: [f64; 3] = [0.25, 0.5, 0.25];
const COARSE: [f64; 5] = [0.0625, 0.25, 0.375, 0.25, 0.0625];

pub(super) struct Response {
    pub dimensions: [u32; 2],
    pub smoothing_order: usize,
    pub noise_gain: f64,
    pub kernel: Vec<f64>,
    pub values: Vec<f64>,
}

pub(super) fn from_rgb8(image: &DynamicImage) -> FloatGray {
    let gray = image.to_luma8();
    FloatGray::from_fn(gray.width(), gray.height(), |x, y| {
        Luma([f64::from(gray.get_pixel(x, y)[0])])
    })
}

pub(super) fn energy(values: &[f64]) -> f64 {
    values.iter().map(|value| value * value).sum::<f64>() / values.len() as f64
}

pub(super) fn inner(gray: &FloatGray) -> Option<FloatGray> {
    let (ix, iy) = (gray.width() * 15 / 100, gray.height() * 15 / 100);
    let (width, height) = (gray.width() - 2 * ix, gray.height() - 2 * iy);
    if width.min(height) < 9 || gray.as_raw().iter().any(|p| !p.is_finite()) {
        return None;
    }
    Some(imageops::crop_imm(gray, ix, iy, width, height).to_image())
}

pub(super) fn planes(gray: &FloatGray) -> Option<[Response; 2]> {
    let inner = inner(gray)?;
    let (width, height) = inner.dimensions();
    Some([&FINE[..], &COARSE[..]].map(|kernel| {
        let smooth = separable_filter_equal(&inner, kernel);
        let mut values = Vec::with_capacity(((width - 6) * (height - 6)) as usize);
        for y in 3..height - 3 {
            for x in 3..width - 3 {
                let p = |x, y| smooth.get_pixel(x, y)[0];
                values.push(p(x - 1, y) + p(x + 1, y) + p(x, y - 1) + p(x, y + 1) - 4.0 * p(x, y));
            }
        }
        let smoothing_order = kernel.len() - 1;
        let kernel = combined_kernel(kernel);
        Response {
            dimensions: [width, height],
            smoothing_order,
            noise_gain: kernel.iter().map(|value| value * value).sum(),
            kernel,
            values,
        }
    }))
}

fn combined_kernel(smoothing: &[f64]) -> Vec<f64> {
    let side = smoothing.len() + 2;
    let mut kernel = vec![0.0; side * side];
    for (y, &vy) in smoothing.iter().enumerate() {
        for (x, &vx) in smoothing.iter().enumerate() {
            let center = (y + 1) * side + x + 1;
            let value = vx * vy;
            kernel[center] -= 4.0 * value;
            for index in [center - 1, center + 1, center - side, center + side] {
                kernel[index] += value;
            }
        }
    }
    kernel
}
