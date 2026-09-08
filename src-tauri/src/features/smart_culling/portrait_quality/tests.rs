use super::*;
use crate::features::smart_culling::types::{EyeDisposition, EyeResult};
use image::{Rgb, RgbImage};

fn sample_face() -> FaceResult {
    FaceResult {
        bbox: [40.0, 40.0, 120.0, 120.0],
        landmarks: [
            (70.0, 80.0),
            (130.0, 80.0),
            (100.0, 110.0),
            (78.0, 140.0),
            (122.0, 140.0),
        ],
        detection_score: 0.95,
        left_eye: EyeResult::unavailable("test", 0, None),
        right_eye: EyeResult::unavailable("test", 0, None),
        eye_disposition: EyeDisposition::Unknown,
        expression_state: "unknown".into(),
        expression_score: None,
        expression_confidence: 0.0,
        expression_reason: "test".into(),
        expression_descriptor: None,
        sharpness_metric: 0.0,
        sharpness_confidence: 0.0,
        exposure_metric: 0.0,
        exposure_confidence: 0.0,
        identity_embedding: None,
    }
}

#[test]
fn optical_neutral_image_has_no_clipping_or_color_judgement() {
    let image = DynamicImage::ImageRgb8(RgbImage::from_pixel(20, 20, Rgb([128; 3])));
    let measured = optical::measure(&image).unwrap();
    assert_eq!(measured.sample_count, 400);
    assert_eq!(measured.mean_rgb, [128.0; 3]);
    assert_eq!(measured.shadow_clip_ratio, 0.0);
    assert_eq!(measured.highlight_clip_ratio, 0.0);
    assert_eq!(measured.luma_p50, 128);
    assert_eq!(measured.luma_std_dev, 0.0);
    let json = serde_json::to_value(measured).unwrap();
    assert!(json.get("passed").is_none());
}

#[test]
fn optical_detects_channel_clipping_hidden_by_grayscale() {
    let image = DynamicImage::ImageRgb8(RgbImage::from_pixel(20, 20, Rgb([255, 100, 100])));
    let measured = optical::measure(&image).unwrap();
    assert_eq!(measured.highlight_clip_ratio, 0.0);
    assert_eq!(measured.channel_highlight_clip_ratio, [1.0, 0.0, 0.0]);
}

#[test]
fn optical_sampling_is_bounded_and_retains_black_white_clipping() {
    let image = DynamicImage::ImageRgb8(RgbImage::from_fn(2048, 1024, |x, _| {
        Rgb([if x < 1024 { 0 } else { 255 }; 3])
    }));
    let measured = optical::measure(&image).unwrap();
    assert!(measured.sample_count <= 512 * 512);
    assert_eq!(measured.sample_stride, 4);
    assert_eq!(measured.shadow_clip_ratio, 0.5);
    assert_eq!(measured.highlight_clip_ratio, 0.5);
    assert_eq!(measured.luma_p05, 0);
    assert_eq!(measured.luma_p95, 255);
    assert!(optical::measure(&DynamicImage::new_rgb8(0, 0)).is_none());
}

#[test]
fn added_blur_lowers_native_and_scaled_local_detail() {
    let image = DynamicImage::ImageRgb8(RgbImage::from_fn(160, 160, |x, y| {
        Rgb([if (x / 5 + y / 7) % 2 == 0 { 30 } else { 220 }; 3])
    }));
    let original = detail::measure(&image).unwrap();
    let blurred = detail::measure(&image.blur(3.0)).unwrap();
    assert!(original.native_laplacian > blurred.native_laplacian);
    assert!(original.scaled_laplacian > blurred.scaled_laplacian);
    assert!(original.scaled_sobel_energy > blurred.scaled_sobel_energy);
    assert!(original.scaled_dimensions.iter().all(|&side| side <= 128));
}

#[test]
fn flat_or_insufficient_detail_does_not_produce_nan_or_upsample() {
    let image = DynamicImage::new_rgb8(32, 32);
    let measured = detail::measure(&image).unwrap();
    assert_eq!(measured.scaled_dimensions, measured.native_dimensions);
    assert_eq!(measured.scaled_sobel_energy, 0.0);
    assert_eq!(measured.blur_retention_ratio, None);
    assert!(detail::measure(&DynamicImage::new_rgb8(1, 1)).is_none());
    assert!(detail::measure(&DynamicImage::new_rgb8(1000, 3)).is_none());
}

#[test]
fn sharp_background_cannot_change_face_detail_measurement() {
    let plain = DynamicImage::new_rgb8(200, 200);
    let noisy_background = DynamicImage::ImageRgb8(RgbImage::from_fn(200, 200, |x, y| {
        if (40..160).contains(&x) && (40..160).contains(&y) {
            Rgb([0; 3])
        } else {
            Rgb([if (x + y) % 2 == 0 { 0 } else { 255 }; 3])
        }
    }));
    let left = face::observe(&plain, &sample_face(), 0).detail.unwrap();
    let right = face::observe(&noisy_background, &sample_face(), 0)
        .detail
        .unwrap();
    assert_eq!(left.native_laplacian, right.native_laplacian);
    assert_eq!(left.scaled_sobel_energy, right.scaled_sobel_energy);
}

#[test]
fn unusable_face_inputs_have_reasons_not_zero_quality_scores() {
    let image = DynamicImage::new_rgb8(200, 200);
    let cases = [
        (
            [f32::NAN, 0.0, 120.0, 120.0],
            QualityUnavailableReason::InvalidFaceBox,
        ),
        (
            [-10.0, 40.0, 120.0, 120.0],
            QualityUnavailableReason::PartialFace,
        ),
        (
            [40.0, 40.0, 12.0, 12.0],
            QualityUnavailableReason::FaceTooSmall,
        ),
        (
            [210.0, 40.0, 120.0, 120.0],
            QualityUnavailableReason::InvalidFaceBox,
        ),
    ];
    for (bbox, reason) in cases {
        let mut detected = sample_face();
        detected.bbox = bbox;
        let result = face::observe(&image, &detected, 0);
        assert_eq!(result.quality_unavailable_reason, Some(reason));
        assert_eq!(result.quality_score, None);
        serde_json::to_string(&result).unwrap();
    }
    let mut detected = sample_face();
    detected.landmarks[0].0 = f32::NAN;
    assert_eq!(
        face::observe(&image, &detected, 0).quality_unavailable_reason,
        Some(QualityUnavailableReason::LandmarksUnavailable)
    );
}

#[test]
fn face_position_reports_distances_not_composition_passes() {
    let result = face::observe(&DynamicImage::new_rgb8(200, 200), &sample_face(), 0);
    let placement = result.placement.unwrap();
    assert_eq!(placement.center, [0.5, 0.5]);
    assert_eq!(placement.center_distance, 0.0);
    assert!(placement.thirds_distance > 0.0);
    assert!(
        serde_json::to_value(placement)
            .unwrap()
            .get("passed")
            .is_none()
    );
}

#[test]
fn no_faces_and_cancellation_are_explicit() {
    let image = DynamicImage::new_rgb8(200, 200);
    let observation = observe(&image, &[], &AtomicBool::new(false)).unwrap();
    assert!(observation.faces.is_empty());
    assert!(observation.optical.is_some());
    assert_eq!(observation.input_source, "renderedRgb8");
    assert!(observe(&image, &[sample_face()], &AtomicBool::new(true)).is_err());
}
