use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Result, anyhow};
use image::{DynamicImage, GenericImageView, GrayImage, imageops};

use super::expression::{ExpressionEvidence, evaluate_expression};
use super::face_geometry::estimate_pose;
use super::face_identity::run_sface_embedding;
use super::face_models::{
    crop_eye_region, run_ferplus_expression, run_ocec_classification, run_yunet_detection,
};
use super::models::SmartCullingFaceModels;
use super::types::{EyeResult, FaceResult};

/// Dimension used for sharpness/exposure analysis. Kept close to the
/// original full-resolution image (instead of the old 720px downscale) so
/// mild focus/blur defects that a heavy downscale would hide are still
/// detected. Fixes the FIXME in the original `culling.rs` implementation.
const ANALYSIS_DIM: u32 = 1600;
const MIN_INTER_OCULAR_DISTANCE: f32 = 16.0;

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
    pub width: u32,
    pub height: u32,
    pub faces: Vec<FaceResult>,
}

pub fn analyze_image_quality(
    img: &DynamicImage,
    detect_faces: bool,
    include_identity: bool,
    face_models: Option<&SmartCullingFaceModels>,
    cancellation: Option<&AtomicBool>,
) -> Result<AnalyzedImage> {
    ensure_not_cancelled(cancellation)?;
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

    ensure_not_cancelled(cancellation)?;
    let faces = if detect_faces {
        detect_faces_in_image(
            img,
            include_identity,
            face_models.ok_or_else(|| anyhow!("face models are required for this analysis"))?,
            cancellation,
        )?
    } else {
        Vec::new()
    };

    Ok(AnalyzedImage {
        sharpness_metric,
        center_focus_metric,
        exposure_metric,
        width,
        height,
        faces,
    })
}

fn detect_faces_in_image(
    img: &DynamicImage,
    include_identity: bool,
    models: &SmartCullingFaceModels,
    cancellation: Option<&AtomicBool>,
) -> Result<Vec<FaceResult>> {
    ensure_not_cancelled(cancellation)?;
    let detections = run_yunet_detection(img, &models.yunet)?;
    ensure_not_cancelled(cancellation)?;

    detections
        .into_iter()
        .map(|face| -> Result<FaceResult> {
            ensure_not_cancelled(cancellation)?;
            let right_eye = face.landmarks[0];
            let left_eye = face.landmarks[1];
            let inter_ocular_distance =
                ((left_eye.0 - right_eye.0).powi(2) + (left_eye.1 - right_eye.1).powi(2)).sqrt();

            // On a strongly turned head the per-eye crops handed to OCEC are
            // foreshortened and partly self-occluded, so a "closed" reading there
            // is not evidence of a blink. Report the eye state as unknown instead
            // of letting a profile shot take a closed-eye penalty.
            let pose_hides_eyes = estimate_pose(&face.landmarks).suppresses_eye_state();
            let (right_eye_result, left_eye_result) = if pose_hides_eyes {
                (unknown_eye(0), unknown_eye(0))
            } else {
                let right = analyze_eye(img, right_eye, inter_ocular_distance, models)?;
                ensure_not_cancelled(cancellation)?;
                let left = analyze_eye(img, left_eye, inter_ocular_distance, models)?;
                (right, left)
            };
            let face_crop = crop_pixel_bbox(img, face.bbox);
            let (sharpness_metric, exposure_metric, local_confidence) = face_crop
                .as_ref()
                .map(|crop| {
                    let gray = crop.to_luma8();
                    let effective = gray.width().min(gray.height()) as f32;
                    (
                        calculate_laplacian_variance(&gray),
                        calculate_exposure_metric(&gray),
                        (effective / 96.0).clamp(0.0, 1.0) * face.score,
                    )
                })
                .unwrap_or((0.0, 0.0, 0.0));
            ensure_not_cancelled(cancellation)?;
            let identity_embedding = if include_identity {
                let embedding = run_sface_embedding(img, &face.landmarks, &models.sface).ok();
                ensure_not_cancelled(cancellation)?;
                embedding
            } else {
                None
            };

            // Expression usability, not emotion: only the certainty of the FER+
            // distribution is consumed. A turned head also invalidates this, for
            // the same reason it invalidates the eye state.
            let expression = match (&models.expression, pose_hides_eyes) {
                (_, true) => ExpressionEvidence::unavailable("expression_pose_unreliable"),
                (None, _) => ExpressionEvidence::unavailable("expression_model_unavailable"),
                (Some(session), false) => face_crop
                    .as_ref()
                    .map(|crop| match run_ferplus_expression(crop, session) {
                        Ok(logits) => evaluate_expression(&logits),
                        Err(_) => ExpressionEvidence::unavailable("expression_inference_failed"),
                    })
                    .unwrap_or_else(|| {
                        ExpressionEvidence::unavailable("expression_face_crop_unavailable")
                    }),
            };
            ensure_not_cancelled(cancellation)?;

            Ok(FaceResult {
                bbox: face.bbox,
                landmarks: face.landmarks,
                detection_score: face.score,
                left_eye: left_eye_result,
                right_eye: right_eye_result,
                expression_state: expression.state.to_string(),
                expression_confidence: expression.confidence,
                expression_reason: expression.reason.to_string(),
                sharpness_metric,
                sharpness_confidence: local_confidence,
                exposure_metric,
                exposure_confidence: local_confidence,
                identity_embedding,
            })
        })
        .collect()
}

fn ensure_not_cancelled(cancellation: Option<&AtomicBool>) -> Result<()> {
    if cancellation.is_some_and(|signal| signal.load(Ordering::Acquire)) {
        Err(anyhow!("smart-culling analysis was cancelled"))
    } else {
        Ok(())
    }
}

fn analyze_eye(
    image: &DynamicImage,
    position: (f32, f32),
    inter_ocular_distance: f32,
    models: &SmartCullingFaceModels,
) -> Result<EyeResult> {
    if inter_ocular_distance < MIN_INTER_OCULAR_DISTANCE {
        return Ok(unknown_eye(0));
    }
    let Some(crop) = crop_eye_region(image, position, inter_ocular_distance) else {
        return Ok(unknown_eye(0));
    };
    let effective_pixels = crop.width().saturating_mul(crop.height());
    let sharpness_metric = Some(calculate_laplacian_variance(&crop.to_luma8()));
    let probability = run_ocec_classification(&crop, &models.ocec)?;
    let (state, confidence) = if probability <= 0.30 {
        ("closed", 1.0 - probability)
    } else if probability >= 0.70 {
        ("open", probability)
    } else {
        ("unknown", ((probability - 0.5).abs() * 2.0).min(0.39))
    };
    Ok(EyeResult {
        open_probability: Some(probability),
        state: state.to_string(),
        confidence,
        effective_pixels,
        sharpness_metric,
    })
}

fn unknown_eye(effective_pixels: u32) -> EyeResult {
    EyeResult {
        open_probability: None,
        state: "unknown".to_string(),
        confidence: 0.0,
        effective_pixels,
        sharpness_metric: None,
    }
}

fn crop_pixel_bbox(image: &DynamicImage, bbox: [f32; 4]) -> Option<DynamicImage> {
    let (width, height) = image.dimensions();
    let x = bbox[0].max(0.0).floor() as u32;
    let y = bbox[1].max(0.0).floor() as u32;
    let crop_width = bbox[2].max(0.0).ceil() as u32;
    let crop_height = bbox[3].max(0.0).ceil() as u32;
    let crop_width = crop_width.min(width.saturating_sub(x));
    let crop_height = crop_height.min(height.saturating_sub(y));
    (crop_width > 0 && crop_height > 0).then(|| image.crop_imm(x, y, crop_width, crop_height))
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

    #[test]
    fn analysis_stops_before_work_when_cancelled() {
        let image = DynamicImage::new_rgb8(8, 8);
        let cancelled = AtomicBool::new(true);

        let result = analyze_image_quality(&image, false, false, None, Some(&cancelled));

        assert!(result.is_err());
    }
}
