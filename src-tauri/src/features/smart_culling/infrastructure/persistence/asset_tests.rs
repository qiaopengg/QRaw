use std::collections::HashMap;
use std::fs::File;
use std::path::Path;

use serde_json::json;
use tempfile::tempdir;

use super::*;
use crate::exif_processing::{get_primary_sidecar_path, load_sidecar, update_sidecar};
use crate::features::smart_culling::domain::{ColorLabel, ConfirmedResult, ResultSource};
use crate::features::smart_culling::infrastructure::locking::set_asset_lock_state;
use crate::features::smart_culling::infrastructure::manual_reconciliation::reconcile_manual_ownership;
use crate::image_processing::ImageFlag;

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
fn same_size_timestamp_photo_replacement_rejects_the_stale_rating() {
    let directory = tempdir().unwrap();
    let image = directory.path().join("photo.jpg");
    let sidecar = get_primary_sidecar_path(&image);
    std::fs::write(&image, b"original").unwrap();
    write_metadata(&sidecar, &ImageMetadata::default());
    let file_baseline = capture_file_baseline(&image).unwrap();
    let sidecar_baseline = capture_sidecar_baseline(&sidecar).unwrap();
    let original_sidecar = std::fs::read(&sidecar).unwrap();
    let modified = std::fs::metadata(&image).unwrap().modified().unwrap();
    std::fs::write(&image, b"replaced").unwrap();
    File::options()
        .write(true)
        .open(&image)
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_modified(modified))
        .unwrap();

    let report = apply_confirmed_results(vec![ConfirmedWrite {
        sidecar_path: sidecar.clone(),
        member_sidecar_baselines: vec![(sidecar.clone(), sidecar_baseline)],
        file_baselines: vec![(image.clone(), file_baseline)],
        result: ai_result(),
    }]);

    assert!(report.succeeded.is_empty());
    assert_eq!(report.failed[0].reason, ApplyFailureReason::AssetChanged);
    assert_eq!(std::fs::read(image).unwrap(), b"replaced");
    assert_eq!(std::fs::read(sidecar).unwrap(), original_sidecar);
}

#[test]
fn synchronizes_the_result_to_raw_and_jpeg_sidecars() {
    let directory = tempdir().unwrap();
    let raw_sidecar = directory.path().join("IMG_0001.dng.rrdata");
    let jpeg_sidecar = directory.path().join("IMG_0001.jpg.rrdata");
    write_metadata(
        &raw_sidecar,
        &ImageMetadata {
            flag: Some(ImageFlag::Pick),
            adjustments: json!({ "exposure": 0.5 }),
            ..ImageMetadata::default()
        },
    );
    write_metadata(
        &jpeg_sidecar,
        &ImageMetadata {
            flag: Some(ImageFlag::Reject),
            ..ImageMetadata::default()
        },
    );
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
    for (sidecar, flag) in [
        (&raw_sidecar, ImageFlag::Pick),
        (&jpeg_sidecar, ImageFlag::Reject),
    ] {
        let metadata = load_sidecar(sidecar);
        assert_eq!(metadata.rating, 4);
        assert_eq!(metadata.flag, Some(flag));
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
fn preserves_upstream_flags_during_manual_sync_and_lock_changes() {
    let directory = tempdir().unwrap();
    let raw = directory.path().join("IMG_0003.dng");
    let jpeg = directory.path().join("IMG_0003.jpg");
    for (image, flag, rating) in [(&raw, ImageFlag::Pick, 4), (&jpeg, ImageFlag::Reject, 5)] {
        File::create(image).unwrap();
        write_metadata(
            &get_primary_sidecar_path(image),
            &ImageMetadata {
                rating,
                flag: Some(flag),
                exif: Some(HashMap::from([(
                    "DateTimeOriginal".to_string(),
                    "2026-08-04 12:00:00".to_string(),
                )])),
                feature_data: Some(json!({
                    "smartCullingV2": {
                        "source": "ai",
                        "rating": 4,
                        "colorLabel": null,
                        "locked": false
                    }
                })),
                ..ImageMetadata::default()
            },
        );
    }

    let report = reconcile_manual_ownership(vec![jpeg.clone()]);
    assert!(report.failed.is_empty(), "{report:?}");
    assert_eq!(report.succeeded.len(), 1);
    for (image, flag) in [(&raw, ImageFlag::Pick), (&jpeg, ImageFlag::Reject)] {
        let metadata = load_sidecar(&get_primary_sidecar_path(image));
        assert_eq!(metadata.rating, 5);
        assert_eq!(metadata.flag, Some(flag));
        assert_eq!(
            metadata.feature_data.unwrap()["smartCullingV2"]["source"],
            "manual"
        );
    }

    for locked in [false, true] {
        let report = set_asset_lock_state(vec![raw.clone(), jpeg.clone()], locked);
        assert!(report.failed.is_empty(), "{report:?}");
        assert_eq!(report.succeeded.len(), 1);
        for (image, flag) in [(&raw, ImageFlag::Pick), (&jpeg, ImageFlag::Reject)] {
            let metadata = load_sidecar(&get_primary_sidecar_path(image));
            assert_eq!(metadata.rating, 5);
            assert_eq!(metadata.flag, Some(flag));
            assert_eq!(
                metadata.feature_data.unwrap()["smartCullingV2"]["locked"],
                locked
            );
        }
    }
}

#[test]
fn upstream_sidecar_updates_preserve_feature_data_after_clearing_the_last_flag() {
    let directory = tempdir().unwrap();
    let image = directory.path().join("IMG_0004.jpg");
    File::create(&image).unwrap();
    let sidecar = get_primary_sidecar_path(&image);
    let feature_data = json!({
        "smartCullingV2": {
            "source": "ai",
            "rating": 0,
            "colorLabel": null,
            "locked": false
        },
        "otherFeature": { "enabled": true }
    });
    write_metadata(
        &sidecar,
        &ImageMetadata {
            flag: Some(ImageFlag::Pick),
            feature_data: Some(feature_data.clone()),
            ..ImageMetadata::default()
        },
    );

    update_sidecar(&sidecar, &image, |metadata| {
        metadata.flag = Some(ImageFlag::Reject);
    })
    .unwrap();
    let metadata = load_sidecar(&sidecar);
    assert_eq!(metadata.flag, Some(ImageFlag::Reject));
    assert_eq!(metadata.feature_data, Some(feature_data.clone()));

    update_sidecar(&sidecar, &image, |metadata| metadata.flag = None).unwrap();
    assert!(
        sidecar.exists(),
        "extension data must prevent default-sidecar deletion"
    );
    let metadata = load_sidecar(&sidecar);
    assert_eq!(metadata.flag, None);
    assert_eq!(metadata.feature_data, Some(feature_data));
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
