use std::fs::File;
use std::path::Path;

use serde_json::json;
use tempfile::tempdir;

use super::*;
use crate::exif_processing::load_sidecar;
use crate::features::smart_culling::domain::{ColorLabel, ConfirmedResult, ResultSource};

fn write_metadata(path: &Path, metadata: &ImageMetadata) {
    let file = File::create(path).unwrap();
    serde_json::to_writer_pretty(file, metadata).unwrap();
}

fn ai_result() -> ConfirmedResult {
    ConfirmedResult {
        result_id: "asset-result".to_string(),
        source: ResultSource::Ai,
        rating: 4,
        color_label: Some(ColorLabel::Green),
        reason_codes: vec!["sharp_subject".to_string()],
        confidence: 0.9,
        mode: "portrait".to_string(),
        model_version: "test-model".to_string(),
        policy_version: "test-policy".to_string(),
        confirmed_at: "2026-08-03T00:00:00Z".to_string(),
    }
}

#[test]
fn synchronizes_the_result_to_raw_and_jpeg_sidecars() {
    let directory = tempdir().unwrap();
    let raw_sidecar = directory.path().join("IMG_0001.dng.rrdata");
    let jpeg_sidecar = directory.path().join("IMG_0001.jpg.rrdata");
    write_metadata(
        &raw_sidecar,
        &ImageMetadata {
            adjustments: json!({ "exposure": 0.5 }),
            ..ImageMetadata::default()
        },
    );
    write_metadata(&jpeg_sidecar, &ImageMetadata::default());
    let raw_baseline = capture_sidecar_baseline(&raw_sidecar).unwrap();
    let jpeg_baseline = capture_sidecar_baseline(&jpeg_sidecar).unwrap();

    let report = apply_confirmed_results(vec![ConfirmedWrite {
        sidecar_path: raw_sidecar.clone(),
        member_sidecar_baselines: vec![
            (raw_sidecar.clone(), raw_baseline),
            (jpeg_sidecar.clone(), jpeg_baseline),
        ],
        file_baselines: Vec::new(),
        result: ai_result(),
    }]);

    assert_eq!(report.succeeded, vec![raw_sidecar.clone()]);
    for sidecar in [&raw_sidecar, &jpeg_sidecar] {
        let metadata = load_sidecar(sidecar);
        assert_eq!(metadata.rating, 4);
        assert_eq!(
            metadata.feature_data.unwrap()["smartCullingV2"]["locked"],
            false
        );
    }
    assert_eq!(
        load_sidecar(&raw_sidecar).adjustments,
        json!({ "exposure": 0.5 })
    );
}

#[test]
fn refuses_to_overwrite_conflicting_unlocked_member_results() {
    let directory = tempdir().unwrap();
    let raw_sidecar = directory.path().join("IMG_0002.dng.rrdata");
    let jpeg_sidecar = directory.path().join("IMG_0002.jpg.rrdata");
    for (sidecar, rating) in [(&raw_sidecar, 3), (&jpeg_sidecar, 4)] {
        write_metadata(
            sidecar,
            &ImageMetadata {
                rating,
                feature_data: Some(json!({
                    "smartCullingV2": {
                        "source": "ai",
                        "rating": rating,
                        "colorLabel": null,
                        "locked": false
                    }
                })),
                ..ImageMetadata::default()
            },
        );
    }
    let raw_baseline = capture_sidecar_baseline(&raw_sidecar).unwrap();
    let jpeg_baseline = capture_sidecar_baseline(&jpeg_sidecar).unwrap();

    let report = apply_confirmed_results(vec![ConfirmedWrite {
        sidecar_path: raw_sidecar.clone(),
        member_sidecar_baselines: vec![
            (raw_sidecar.clone(), raw_baseline),
            (jpeg_sidecar.clone(), jpeg_baseline),
        ],
        file_baselines: Vec::new(),
        result: ai_result(),
    }]);

    assert!(report.succeeded.is_empty());
    assert_eq!(
        report.failed[0].reason,
        ApplyFailureReason::BaselineConflict
    );
    assert_eq!(load_sidecar(&raw_sidecar).rating, 3);
    assert_eq!(load_sidecar(&jpeg_sidecar).rating, 4);
}
