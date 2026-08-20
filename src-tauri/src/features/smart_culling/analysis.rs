use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Result, anyhow};
use image::{DynamicImage, GenericImageView, GrayImage, imageops};

#[cfg(not(all(debug_assertions, target_os = "macos")))]
use super::expression::ExpressionEvidence;
use super::face_geometry::estimate_pose;
use super::face_identity::run_sface_embedding;
use super::face_models::run_yunet_detection;
use super::models::SmartCullingFaceModels;
use super::types::FaceResult;
#[cfg(not(all(debug_assertions, target_os = "macos")))]
use super::types::{EyeDisposition, EyeResult};

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
            #[cfg(all(debug_assertions, target_os = "macos"))]
            let (
                left_eye_result,
                right_eye_result,
                eye_disposition,
                expression,
                expression_descriptor,
            ) = {
                let pose_suppresses_eye_state =
                    estimate_pose(&face.landmarks).suppresses_eye_state();
                let motion = super::face_motion_poc::analyze_calibration_face(
                    img,
                    &face,
                    pose_suppresses_eye_state,
                )?;
                ensure_not_cancelled(cancellation)?;
                motion.into_legacy_parts()
            };
            #[cfg(not(all(debug_assertions, target_os = "macos")))]
            let (
                left_eye_result,
                right_eye_result,
                eye_disposition,
                expression,
                expression_descriptor,
            ) = {
                // YuNet exposes only one point per eye, not an eye bounding box or
                // eyelid contour. OCEC was trained for actual eye crops, so deriving
                // its input size from the two YuNet eye points is not validated
                // evidence. Keep eye state unavailable until an eye detector or a
                // dense-landmark model passes real-photo validation.
                let pose_hides_eyes = estimate_pose(&face.landmarks).suppresses_eye_state();
                let unavailable_reason = if pose_hides_eyes {
                    "eye_pose_unreliable"
                } else {
                    "eye_model_input_unvalidated"
                };
                let right_eye_result = EyeResult::unavailable(unavailable_reason, 0, None);
                let left_eye_result = EyeResult::unavailable(unavailable_reason, 0, None);
                (
                    left_eye_result,
                    right_eye_result,
                    EyeDisposition::Unknown,
                    ExpressionEvidence::unavailable("expression_model_unvalidated"),
                    None,
                )
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
            ensure_not_cancelled(cancellation)?;

            Ok(FaceResult {
                bbox: face.bbox,
                landmarks: face.landmarks,
                detection_score: face.score,
                left_eye: left_eye_result,
                right_eye: right_eye_result,
                eye_disposition,
                expression_state: expression.state.to_string(),
                expression_confidence: expression.confidence,
                expression_reason: expression.reason.to_string(),
                expression_descriptor,
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
