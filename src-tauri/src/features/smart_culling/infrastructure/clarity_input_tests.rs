use super::*;
use image::{ImageFormat, Rgb, RgbImage};
use serde_json::json;
use std::io::Cursor;

fn source() -> DynamicImage {
    DynamicImage::ImageRgb8(RgbImage::from_fn(32, 24, |x, y| {
        Rgb([x as u8, y as u8, (x + y) as u8])
    }))
}

fn bytes(image: &DynamicImage) -> Vec<u8> {
    let mut encoded = Cursor::new(Vec::new());
    image.write_to(&mut encoded, ImageFormat::Png).unwrap();
    encoded.into_inner()
}

#[test]
fn an_unedited_photos_default_metadata_provides_native_identity_pixels() {
    let original = source();
    let metadata = crate::image_processing::ImageMetadata::default();
    assert!(metadata.adjustments.is_null());
    let expected =
        crate::adjustment_utils::apply_all_transformations(&original, &metadata.adjustments)
            .0
            .into_owned();
    let input = ClarityInput::from_bytes(&bytes(&original), false, &metadata.adjustments);
    assert_eq!(input.image(), Some(&expected));
    assert_eq!(input.unavailable_reason(), None);
    assert_eq!(
        input.adjustments_sha256,
        Some(hex::encode(Sha256::digest(b"null")))
    );
}

#[test]
fn native_pixels_ignore_photometric_edits_but_follow_lossless_geometry() {
    let source = source();
    let encoded = bytes(&source);
    let edits = json!({"exposure": 4.0, "sharpening": 100.0});
    assert_eq!(
        ClarityInput::from_bytes(&encoded, false, &edits)
            .image()
            .unwrap(),
        &source
    );
    for steps in 0..4 {
        for horizontal in [false, true] {
            for vertical in [false, true] {
                let edits = json!({"orientationSteps": steps, "flipHorizontal": horizontal,
                    "flipVertical": vertical, "crop": {"x": 3.0, "y": 4.0, "width": 12.0, "height": 10.0}});
                let expected = crate::adjustment_utils::apply_all_transformations(&source, &edits)
                    .0
                    .into_owned();
                let mut input = ClarityInput::from_bytes(&encoded, false, &edits);
                input.check_render_dimensions(expected.dimensions());
                assert_eq!(input.image().unwrap(), &expected);
            }
        }
    }
}

#[test]
fn same_dimensions_are_not_mistaken_for_same_face_coordinates() {
    let original = source();
    let input =
        ClarityInput::from_bytes(&bytes(&original), false, &json!({"flipHorizontal": true}));
    let aligned = input.image().unwrap();
    assert_eq!(aligned.dimensions(), original.dimensions());
    assert_eq!(aligned.get_pixel(2, 5), original.get_pixel(29, 5));
    assert_ne!(aligned.get_pixel(2, 5), original.get_pixel(2, 5));
}

#[test]
fn unsupported_inputs_keep_reasons_instead_of_rendered_fallbacks() {
    let encoded = bytes(&source());
    for (raw, edits, reason) in [
        (
            true,
            json!({}),
            ClarityInputUnavailable::RawSourceUnvalidated,
        ),
        (
            false,
            json!({"rotation": 1.0}),
            ClarityInputUnavailable::GeometryUnvalidated,
        ),
        (
            false,
            json!({"transformHorizontal": 1.0}),
            ClarityInputUnavailable::GeometryUnvalidated,
        ),
        (
            false,
            json!({"transformScale": 90.0}),
            ClarityInputUnavailable::GeometryUnvalidated,
        ),
        (
            false,
            json!({"aiPatches": [{"visible": true}]}),
            ClarityInputUnavailable::ContentEdited,
        ),
    ] {
        let input = ClarityInput::from_bytes(&encoded, raw, &edits);
        assert!(input.image().is_none());
        assert_eq!(input.unavailable_reason(), Some(reason));
    }
    let mut input = ClarityInput::from_bytes(&encoded, false, &json!({}));
    input.check_render_dimensions((720, 360));
    assert_eq!(
        input.unavailable_reason(),
        Some(ClarityInputUnavailable::RenderDimensionsMismatch)
    );
    assert_eq!(
        ClarityInput::from_bytes(b"broken", false, &json!({})).unavailable_reason(),
        Some(ClarityInputUnavailable::DecodeFailed)
    );
}

#[test]
fn exif_all_eight_orientations_match_pixels_and_editor_geometry() {
    let original = image::load_from_memory(&super::super::test_images::jpeg_with_orientation(
        &source(),
        1,
    ))
    .unwrap();
    for orientation in 1_u8..=8 {
        let tagged = super::super::test_images::jpeg_with_orientation(&source(), orientation);
        let decoded = ClarityInput::from_bytes(&tagged, false, &json!({}));
        let image = decoded.image().unwrap();
        let corners = [
            (0, 0),
            (31, 0),
            (31, 23),
            (0, 23),
            (0, 0),
            (0, 23),
            (31, 23),
            (31, 0),
        ];
        let (x, y) = corners[usize::from(orientation - 1)];
        assert_eq!(
            image.get_pixel(0, 0),
            original.get_pixel(x, y),
            "EXIF {orientation}"
        );
        assert_eq!(
            image.dimensions(),
            if orientation >= 5 { (24, 32) } else { (32, 24) }
        );
        for steps in 0..4 {
            for horizontal in [false, true] {
                for vertical in [false, true] {
                    let edits = json!({"orientationSteps": steps, "flipHorizontal": horizontal,
                        "flipVertical": vertical, "crop": {"x": 2.5, "y": 3.6, "width": 15.6, "height": 12.2}});
                    let host =
                        crate::image_loader::load_image_with_orientation(&tagged, None).unwrap();
                    let expected =
                        crate::adjustment_utils::apply_all_transformations(&host, &edits)
                            .0
                            .to_rgb8();
                    let actual = ClarityInput::from_bytes(&tagged, false, &edits);
                    assert_eq!(actual.image().unwrap().to_rgb8(), expected);
                }
            }
        }
    }
}

#[test]
fn crop_rounding_and_clipping_follow_the_host_but_invalid_crops_are_unavailable() {
    let original = source();
    let encoded = bytes(&original);
    for crop in [
        json!(null),
        json!({"x": 2.6, "y": 3.4, "width": 100.0, "height": 100.0}),
        json!({"x": 0.0, "y": 0.0, "width": 32.0, "height": 24.0}),
    ] {
        let edits = json!({"crop": crop});
        let expected = crate::adjustment_utils::apply_all_transformations(&original, &edits)
            .0
            .into_owned();
        assert_eq!(
            ClarityInput::from_bytes(&encoded, false, &edits).image(),
            Some(&expected)
        );
    }
    for crop in [
        json!({}),
        json!("bad"),
        json!({"x": -1, "y": 0, "width": 5, "height": 5}),
        json!({"x": 32, "y": 0, "width": 5, "height": 5}),
        json!({"x": 0, "y": 24, "width": 5, "height": 5}),
        json!({"x": 0, "y": 0, "width": 0.4, "height": 5}),
    ] {
        assert_eq!(
            ClarityInput::from_bytes(&encoded, false, &json!({"crop": crop})).unavailable_reason(),
            Some(ClarityInputUnavailable::InvalidCrop)
        );
    }
}

#[test]
fn unvalidated_edits_and_bad_adjustments_never_become_native_evidence() {
    let encoded = bytes(&source());
    for edits in [
        json!({"orientationSteps": -1}),
        json!({"orientationSteps": 4}),
        json!({"rotation": 90}),
        json!({"lensDistortionParams": {"k1": 0.1}}),
        json!({"lensDistortionParams": {"tca_vr": 1.1}}),
        json!({"lensBlurEnabled": true}),
        json!({"aiPatches": [{}]}),
        json!({"aiPatches": "bad"}),
    ] {
        assert!(
            ClarityInput::from_bytes(&encoded, false, &edits)
                .image()
                .is_none(),
            "{edits}"
        );
    }
    for edits in [
        json!([]),
        json!(true),
        json!(1),
        json!("bad"),
        json!({"rotation": "bad"}),
        json!({"flipHorizontal": 1}),
    ] {
        assert_eq!(
            ClarityInput::from_bytes(&encoded, false, &edits).unavailable_reason(),
            Some(ClarityInputUnavailable::InvalidAdjustments)
        );
    }
    let hidden = json!({"aiPatches": [{"visible": false}], "rotation": 360,
        "lensDistortionEnabled": false, "lensTcaEnabled": false, "lensDistortionParams": {"k1": 0.1, "tca_vr": 1.1}});
    assert_eq!(
        ClarityInput::from_bytes(&encoded, false, &hidden).image(),
        Some(&source())
    );
}

#[test]
fn rotation_validation_uses_the_hosts_actual_numeric_precision() {
    let original = source();
    let encoded = bytes(&original);
    let degrees = 360_000_360.0_f64;
    assert_eq!(degrees % 360.0, 0.0);
    assert_ne!(degrees as f32 % 360.0, 0.0);
    let edits = json!({"rotation": degrees});
    let host = crate::adjustment_utils::apply_all_transformations(&original, &edits)
        .0
        .into_owned();
    assert_eq!(host.dimensions(), original.dimensions());
    assert_ne!(host.to_rgb8(), original.to_rgb8());
    for degrees in [degrees, -degrees, 1e300] {
        assert_eq!(
            ClarityInput::from_bytes(&encoded, false, &json!({"rotation": degrees}))
                .unavailable_reason(),
            Some(ClarityInputUnavailable::GeometryUnvalidated)
        );
    }
    for degrees in [0.0, 360.0, -720.0] {
        assert_eq!(
            ClarityInput::from_bytes(&encoded, false, &json!({"rotation": degrees})).image(),
            Some(&original)
        );
    }
}

#[test]
fn input_provenance_records_the_actual_bytes_and_edit_snapshot() {
    let encoded = bytes(&source());
    let plain = ClarityInput::from_bytes(&encoded, false, &json!({}));
    let flipped = ClarityInput::from_bytes(&encoded, false, &json!({"flipHorizontal": true}));
    assert_eq!(plain.source_sha256, flipped.source_sha256);
    assert_ne!(plain.adjustments_sha256, flipped.adjustments_sha256);
    let mut unavailable = ClarityInput::unavailable(ClarityInputUnavailable::RawSourceUnvalidated);
    unavailable.check_render_dimensions((32, 24));
    assert_eq!(
        unavailable.unavailable_reason(),
        Some(ClarityInputUnavailable::RawSourceUnvalidated)
    );
    assert!(unavailable.image().is_none());
}
