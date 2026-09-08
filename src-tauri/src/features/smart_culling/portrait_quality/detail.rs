//! Local face detail at native and bounded scales; deliberately no pass threshold.

use image::{DynamicImage, GenericImageView, imageops};
use imageproc::gradients::{horizontal_sobel, vertical_sobel};
use serde::Serialize;

use super::super::analysis::calculate_laplacian_variance;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FaceDetailMeasurements {
    pub native_dimensions: [u32; 2],
    pub scaled_dimensions: [u32; 2],
    pub native_laplacian: f64,
    pub scaled_laplacian: f64,
    pub scaled_sobel_energy: f64,
    pub blur_retention_ratio: Option<f64>,
}

pub(super) fn measure(crop: &DynamicImage) -> Option<FaceDetailMeasurements> {
    let (width, height) = crop.dimensions();
    // Exclude outer hair/background and rectangle edges; this is a fixed inner
    // ROI, not semantic skin/occlusion segmentation.
    let inset_x = width * 15 / 100;
    let inset_y = height * 15 / 100;
    let inner = crop.crop_imm(inset_x, inset_y, width - 2 * inset_x, height - 2 * inset_y);
    if inner.width() < 3 || inner.height() < 3 {
        return None;
    }
    let native_laplacian = calculate_laplacian_variance(&inner.to_luma8());
    // Never upscale a small face and misrepresent invented pixels as evidence.
    let scaled = inner
        .thumbnail(inner.width().min(128), inner.height().min(128))
        .to_luma8();
    if scaled.width() < 3 || scaled.height() < 3 {
        return None;
    }
    let scaled_laplacian = calculate_laplacian_variance(&scaled);
    let horizontal = horizontal_sobel(&scaled);
    let vertical = vertical_sobel(&scaled);
    let mut energy = 0.0;
    let mut count = 0_u64;
    for y in 1..scaled.height() - 1 {
        for x in 1..scaled.width() - 1 {
            energy += f64::from(horizontal.get_pixel(x, y)[0]).powi(2)
                + f64::from(vertical.get_pixel(x, y)[0]).powi(2);
            count += 1;
        }
    }
    let blurred_laplacian = calculate_laplacian_variance(&imageops::blur(&scaled, 1.0));
    Some(FaceDetailMeasurements {
        native_dimensions: [inner.width(), inner.height()],
        scaled_dimensions: [scaled.width(), scaled.height()],
        native_laplacian,
        scaled_laplacian,
        scaled_sobel_energy: energy / count as f64,
        blur_retention_ratio: (scaled_laplacian > 0.0)
            .then(|| blurred_laplacian / scaled_laplacian),
    })
}
