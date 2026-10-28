use super::*;
use crate::image_loader::load_and_composite;
use image::{Rgb, RgbImage};
use serde_json::json;
use std::fs;

fn tagged_jpeg() -> Vec<u8> {
    let image = DynamicImage::ImageRgb8(RgbImage::from_pixel(32, 24, Rgb([80, 120, 180])));
    super::super::test_images::jpeg_with_orientation(&image, 6)
}

#[test]
#[ignore = "requires QRAW_PORTRAIT_DECODE_FIXTURE pointing to a disposable RAW copy"]
fn portrait_snapshot_matches_host_raw_pixels() {
    let fixture = std::env::var("QRAW_PORTRAIT_DECODE_FIXTURE").unwrap();
    assert!(crate::formats::is_raw_file(&fixture));
    let source = Path::new(&fixture);
    let bytes = fs::read(source).unwrap();
    let sidecar = crate::exif_processing::get_primary_sidecar_path(source);
    let before = fs::read(&sidecar).ok();
    let settings = AppSettings::default();
    let start = std::time::Instant::now();
    let observed = decode_portrait_snapshot(&bytes, &fixture, &json!(null), &settings).unwrap();
    let elapsed = start.elapsed();
    assert_eq!(fs::read(&sidecar).ok(), before);
    assert_eq!(fs::read(source).unwrap(), bytes);
    let reference = tempfile::tempdir().unwrap();
    let reference_path = reference.path().join(source.file_name().unwrap());
    fs::write(&reference_path, &bytes).unwrap();
    let expected = load_and_composite(
        &bytes,
        reference_path.to_str().unwrap(),
        &json!(null),
        false,
        &settings,
        None,
    )
    .unwrap();
    assert_eq!(observed, expected);
    println!(
        "QRAW_RAW_SNAPSHOT_PASS {}x{} decode_seconds={:.3}",
        observed.width(),
        observed.height(),
        elapsed.as_secs_f64()
    );
}

#[test]
fn portrait_decode_does_not_create_or_rewrite_source_metadata() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("portrait.jpg");
    let sidecar = crate::exif_processing::get_primary_sidecar_path(&path);
    let bytes = tagged_jpeg();
    fs::write(&path, &bytes).unwrap();
    for existing in [
        None,
        Some(br#"{"rating":5,"adjustments":null,"tags":["manual"]}"#.to_vec()),
    ] {
        if let Some(value) = &existing {
            fs::write(&sidecar, value).unwrap();
        }
        let decoded = decode_portrait_snapshot(
            &bytes,
            path.to_str().unwrap(),
            &json!(null),
            &AppSettings::default(),
        )
        .unwrap();
        assert_eq!(decoded.width(), 24);
        assert_eq!(decoded.height(), 32);
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert_eq!(fs::read(&sidecar).ok(), existing);
    }
}

#[test]
fn snapshot_decode_matches_the_host_pixels_and_reports_invalid_bytes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("host.jpg");
    let bytes = tagged_jpeg();
    fs::write(&path, &bytes).unwrap();
    let settings = AppSettings::default();
    let expected = load_and_composite(
        &bytes,
        path.to_str().unwrap(),
        &json!({}),
        false,
        &settings,
        None,
    )
    .unwrap();
    let observed =
        decode_portrait_snapshot(&bytes, path.to_str().unwrap(), &json!({}), &settings).unwrap();
    assert_eq!(observed, expected);
    assert!(
        decode_portrait_snapshot(b"invalid", path.to_str().unwrap(), &json!({}), &settings)
            .is_err()
    );
}

#[test]
fn snapshot_decode_retains_host_preparation_for_edited_pixels() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("negative.jpg");
    let bytes = tagged_jpeg();
    fs::write(&path, &bytes).unwrap();
    let settings = AppSettings::default();
    let edits = json!({"negativeConversionEnabled": true, "negativeExposure": 0.5});
    let expected = load_and_composite(
        &bytes,
        path.to_str().unwrap(),
        &edits,
        false,
        &settings,
        None,
    )
    .unwrap();
    let observed =
        decode_portrait_snapshot(&bytes, path.to_str().unwrap(), &edits, &settings).unwrap();
    let unedited =
        decode_portrait_snapshot(&bytes, path.to_str().unwrap(), &json!({}), &settings).unwrap();
    assert_eq!(observed, expected);
    assert_ne!(observed, unedited);
}
