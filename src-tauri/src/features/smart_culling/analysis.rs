use anyhow::{Result, anyhow};
use image::{DynamicImage, GenericImageView, GrayImage, imageops};

use super::face_models::{
    crop_eye_region, is_eye_closed, run_ocec_classification, run_yunet_detection,
};
use super::models::SmartCullingFaceModels;
use super::types::FaceResult;

const WEIGHT_SHARPNESS: f64 = 0.40;
const WEIGHT_CENTER_FOCUS: f64 = 0.35;
const WEIGHT_EXPOSURE: f64 = 0.25;

/// Dimension used for sharpness/exposure analysis. Kept close to the
/// original full-resolution image (instead of the old 720px downscale) so
/// mild focus/blur defects that a heavy downscale would hide are still
/// detected. Fixes the FIXME in the original `culling.rs` implementation.
const ANALYSIS_DIM: u32 = 1600;

pub fn calculate_laplacian_variance(image: &GrayImage) -> f64 {
    let (width, height) = image.dimensions();
    if width < 3 || height < 3 {
        return 0.0;
    }

    let mut sum = 0.0;
    let mut sum_of_squares = 0.0;
    let mut count = 0_u64;

    for y in 1..height - 1 {
        for x in 1..width - 1 {
            let p_center = image.get_pixel(x, y)[0] as i32;
            let p_north = image.get_pixel(x, y - 1)[0] as i32;
            let p_south = image.get_pixel(x, y + 1)[0] as i32;
            let p_west = image.get_pixel(x - 1, y)[0] as i32;
            let p_east = image.get_pixel(x + 1, y)[0] as i32;
            let conv_val = (p_north + p_south + p_west + p_east - 4 * p_center) as f64;
            sum += conv_val;
            sum_of_squares += conv_val * conv_val;
            count += 1;
        }
    }

    if count == 0 {
        return 0.0;
    }
    let count = count as f64;
    let mean = sum / count;
    (sum_of_squares / count - mean * mean).max(0.0)
}

pub fn calculate_exposure_metric(image: &GrayImage) -> f64 {
    let histogram = imageproc::stats::histogram(image);
    let total_pixels = (image.width() * image.height()) as f64;
    if total_pixels == 0.0 {
        return 0.0;
    }

    let clip_threshold_dark = 5;
    let clip_threshold_bright = 250;

    let dark_pixels = histogram.channels[0][0..clip_threshold_dark]
        .iter()
        .sum::<u32>() as f64;
    let bright_pixels = histogram.channels[0][clip_threshold_bright..256]
        .iter()
        .sum::<u32>() as f64;

    let dark_clip_ratio = dark_pixels / total_pixels;
    let bright_clip_ratio = bright_pixels / total_pixels;

    let penalty = (dark_clip_ratio * 5.0) + (bright_clip_ratio * 5.0);

    (1.0f64 - penalty).max(0.0)
}

#[derive(Clone)]
pub struct AnalyzedImage {
    pub sharpness_metric: f64,
    pub center_focus_metric: f64,
    pub exposure_metric: f64,
    pub quality_score: f64,
    pub width: u32,
    pub height: u32,
    pub faces: Vec<FaceResult>,
}

pub fn analyze_image_quality(
    img: &DynamicImage,
    detect_faces: bool,
    face_models: Option<&SmartCullingFaceModels>,
) -> Result<AnalyzedImage> {
    let (width, height) = img.dimensions();
    let thumbnail = img.thumbnail(ANALYSIS_DIM, ANALYSIS_DIM);
    let gray_thumbnail = thumbnail.to_luma8();

    let sharpness_metric = calculate_laplacian_variance(&gray_thumbnail);
    let exposure_metric = calculate_exposure_metric(&gray_thumbnail);

    let (thumb_w, thumb_h) = gray_thumbnail.dimensions();
    let center_crop = imageops::crop_imm(
        &gray_thumbnail,
        thumb_w / 4,
        thumb_h / 4,
        thumb_w / 2,
        thumb_h / 2,
    )
    .to_image();
    let center_focus_metric = calculate_laplacian_variance(&center_crop);

    let normalized_sharpness = ((sharpness_metric + 1.0).log10() / 3.5).min(1.0);
    let normalized_center_focus = ((center_focus_metric + 1.0).log10() / 3.5).min(1.0);

    let quality_score = (normalized_sharpness * WEIGHT_SHARPNESS)
        + (normalized_center_focus * WEIGHT_CENTER_FOCUS)
        + (exposure_metric * WEIGHT_EXPOSURE);

    let faces = if detect_faces {
        detect_faces_in_image(
            img,
            face_models.ok_or_else(|| anyhow!("face models are required for this analysis"))?,
        )?
    } else {
        Vec::new()
    };

    Ok(AnalyzedImage {
        sharpness_metric,
        center_focus_metric,
        exposure_metric,
        quality_score,
        width,
        height,
        faces,
    })
}

fn detect_faces_in_image(
    img: &DynamicImage,
    models: &SmartCullingFaceModels,
) -> Result<Vec<FaceResult>> {
    let detections = run_yunet_detection(img, &models.yunet)?;

    detections
        .into_iter()
        .map(|face| -> Result<FaceResult> {
            let right_eye = face.landmarks[0];
            let left_eye = face.landmarks[1];
            let inter_ocular_distance =
                ((left_eye.0 - right_eye.0).powi(2) + (left_eye.1 - right_eye.1).powi(2)).sqrt();

            let eye_open_probs: Vec<f32> = [right_eye, left_eye]
                .iter()
                .map(|&eye_pos| {
                    let crop = crop_eye_region(img, eye_pos, inter_ocular_distance)
                        .ok_or_else(|| anyhow!("eye crop is outside the rendered image"))?;
                    run_ocec_classification(&crop, &models.ocec)
                })
                .collect::<Result<_>>()?;

            let eye_open_prob = if eye_open_probs.is_empty() {
                None
            } else {
                Some(eye_open_probs.iter().sum::<f32>() / eye_open_probs.len() as f32)
            };

            let is_closed = eye_open_prob.map(is_eye_closed).unwrap_or(false);

            Ok(FaceResult {
                bbox: face.bbox,
                eye_open_prob,
                is_closed,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use image::Luma;

    use super::*;

    #[test]
    fn laplacian_variance_is_zero_for_a_flat_image() {
        let image = GrayImage::from_pixel(8, 8, Luma([128]));

        assert_eq!(calculate_laplacian_variance(&image), 0.0);
    }

    #[test]
    fn laplacian_variance_detects_high_frequency_detail() {
        let image = GrayImage::from_fn(8, 8, |x, y| {
            if (x + y) % 2 == 0 {
                Luma([0])
            } else {
                Luma([255])
            }
        });

        assert!(calculate_laplacian_variance(&image) > 100_000.0);
    }
}
