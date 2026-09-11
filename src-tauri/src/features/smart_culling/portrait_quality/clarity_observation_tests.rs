use super::*;
use crate::features::smart_culling::{
    analysis,
    infrastructure::{ClarityInput, ClarityInputUnavailable},
};
use image::ImageFormat;
use serde_json::json;
use std::io::Cursor;

fn input(edits: serde_json::Value) -> ClarityInput {
    let image = DynamicImage::ImageRgb8(RgbImage::from_fn(240, 200, |x, y| {
        Rgb([if x < 120 && (x / 4 + y / 5) % 2 == 0 {
            20
        } else {
            160
        }; 3])
    }));
    let mut encoded = Cursor::new(Vec::new());
    image.write_to(&mut encoded, ImageFormat::Png).unwrap();
    ClarityInput::from_bytes(encoded.get_ref(), false, &edits)
}

#[test]
fn native_observation_and_legacy_metrics_use_the_same_aligned_face() {
    let mut input = input(json!({"flipHorizontal": true}));
    let rendered = DynamicImage::new_rgb8(240, 200);
    input.check_render_dimensions(rendered.dimensions());
    let mut face = sample_face();
    face.bbox = [120.0, 40.0, 100.0, 120.0];
    // Landmarks/profile do not make native detail unavailable; FIQA remains independent.
    face.landmarks = [(f32::NAN, f32::NAN); 5];
    let mut quality = analysis::analyze_image_quality(&rendered, false, false, None, None).unwrap();
    quality.faces.push(face);
    assert!(analysis::apply_source_clarity_metrics(
        input.image().unwrap(),
        &mut quality
    ));
    let crop = analysis::crop_pixel_bbox(input.image().unwrap(), quality.faces[0].bbox).unwrap();
    assert_eq!(
        quality.faces[0].sharpness_metric,
        analysis::calculate_laplacian_variance(&crop.to_luma8())
    );
    assert!(quality.faces[0].sharpness_metric > 0.0);
    let observed =
        observe_with_clarity(&rendered, &input, &quality.faces, &AtomicBool::new(false)).unwrap();
    assert_eq!(observed.input_source, "renderedRgb8");
    assert_eq!(
        observed.faces[0].detail.as_ref().unwrap().native_laplacian,
        0.0
    );
    let native = observed.clarity.as_ref().unwrap();
    assert_eq!(native.input_source, "decodedRgb8LosslessGeometry");
    assert!(native.faces[0].detail.as_ref().unwrap().native_laplacian > 0.0);
    assert!(native.faces[0].robust_detail.is_some());
    assert!(native.faces[0].unavailable_reason.is_none());
    let serialized = serde_json::to_value(observed).unwrap();
    assert!(serialized["clarity"].get("passed").is_none());
    assert!(serialized["clarity"].get("rating").is_none());
}

#[test]
fn unavailable_source_never_uses_rendered_pixels_for_new_clarity() {
    let rendered = DynamicImage::new_rgb8(200, 200);
    for reason in [
        ClarityInputUnavailable::RawSourceUnvalidated,
        ClarityInputUnavailable::GeometryUnvalidated,
        ClarityInputUnavailable::ContentEdited,
        ClarityInputUnavailable::RenderDimensionsMismatch,
    ] {
        let input = ClarityInput::unavailable(reason);
        let observed =
            observe_with_clarity(&rendered, &input, &[sample_face()], &AtomicBool::new(false))
                .unwrap();
        assert!(observed.optical.is_some());
        let clarity = observed.clarity.unwrap();
        assert_eq!(clarity.unavailable_reason, Some(reason));
        assert!(clarity.input_dimensions.is_none());
        assert!(clarity.faces.is_empty());
    }
    assert!(
        observe_with_clarity(&rendered, &input(json!({})), &[], &AtomicBool::new(true)).is_err()
    );
}

#[test]
fn invalid_partial_small_or_unreliable_faces_have_no_native_measurements() {
    let input = input(json!({}));
    for (bbox, score, expected) in [
        (
            [-1.0, 0.0, 100.0, 100.0],
            0.9,
            QualityUnavailableReason::PartialFace,
        ),
        (
            [f32::NAN, 0.0, 100.0, 100.0],
            0.9,
            QualityUnavailableReason::InvalidFaceBox,
        ),
        (
            [10.0, 10.0, 10.0, 10.0],
            0.9,
            QualityUnavailableReason::FaceTooSmall,
        ),
        (
            [10.0, 10.0, 100.0, 100.0],
            0.1,
            QualityUnavailableReason::UnreliableDetection,
        ),
    ] {
        let mut face = sample_face();
        face.bbox = bbox;
        face.detection_score = score;
        let observed = clarity_observation::observe_face(input.image().unwrap(), &face, 3);
        assert_eq!(observed.face_index, 3);
        assert_eq!(observed.unavailable_reason, Some(expected));
        assert!(observed.detail.is_none());
        assert!(observed.robust_detail.is_none());
    }
}
