use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use tempfile::NamedTempFile;

#[cfg(test)]
use crate::exif_processing::load_sidecar;
use crate::image_processing::ImageMetadata;

use super::super::domain::{
    ConfirmedResult, MetadataSnapshot, ResultSource, asset_has_conflicting_results,
    asset_is_protected, metadata_has_unknown_source,
};
#[cfg(test)]
use super::super::domain::{MetadataOwnership, classify_metadata_ownership};
use super::baseline::{
    FileBaseline, SidecarBaseline, capture_file_baseline, capture_sidecar_baseline,
};
use super::catalog::read_sidecar_strict;
#[cfg(test)]
use super::manual_reconciliation::reconcile_manual_ownership;

const COLOR_TAG_PREFIX: &str = "color:";
const SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ApplyFailureReason {
    AssetChanged,
    BaselineConflict,
    ManualProtection,
    InvalidResult,
    Io,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ApplyFailure {
    pub sidecar_path: PathBuf,
    pub reason: ApplyFailureReason,
    pub detail: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub(crate) struct ApplyReport {
    pub succeeded: Vec<PathBuf>,
    pub failed: Vec<ApplyFailure>,
}

pub(crate) struct ConfirmedWrite {
    pub sidecar_path: PathBuf,
    pub member_sidecar_baselines: Vec<(PathBuf, SidecarBaseline)>,
    pub file_baselines: Vec<(PathBuf, FileBaseline)>,
    pub result: ConfirmedResult,
}

pub(crate) fn apply_confirmed_results(items: Vec<ConfirmedWrite>) -> ApplyReport {
    let mut report = ApplyReport::default();

    for item in items {
        match apply_one(&item) {
            Ok(()) => report.succeeded.push(item.sidecar_path),
            Err((reason, detail)) => report.failed.push(ApplyFailure {
                sidecar_path: item.sidecar_path,
                reason,
                detail,
            }),
        }
    }

    report
}

fn apply_one(item: &ConfirmedWrite) -> Result<(), (ApplyFailureReason, String)> {
    item.result
        .validate()
        .map_err(|reason| (ApplyFailureReason::InvalidResult, reason.to_string()))?;
    ensure_file_baselines_match(&item.file_baselines)?;
    ensure_asset_sidecars_writable(&item.member_sidecar_baselines)?;

    let mut updates = Vec::with_capacity(item.member_sidecar_baselines.len());
    for (sidecar_path, _) in &item.member_sidecar_baselines {
        let mut metadata =
            read_sidecar_strict(sidecar_path).map_err(|error| (ApplyFailureReason::Io, error))?;
        merge_result(&mut metadata, &item.result);
        updates.push((sidecar_path, metadata));
    }
    ensure_file_baselines_match(&item.file_baselines)?;
    ensure_asset_sidecars_writable(&item.member_sidecar_baselines)?;
    for (sidecar_path, metadata) in updates {
        atomic_write_sidecar(sidecar_path, &metadata)
            .map_err(|error| (ApplyFailureReason::Io, error))?;
    }
    Ok(())
}

fn ensure_asset_sidecars_writable(
    baselines: &[(PathBuf, SidecarBaseline)],
) -> Result<(), (ApplyFailureReason, String)> {
    let mut snapshots = Vec::with_capacity(baselines.len());
    for (sidecar_path, baseline) in baselines {
        ensure_baseline_matches(sidecar_path, baseline)?;
        let metadata =
            read_sidecar_strict(sidecar_path).map_err(|error| (ApplyFailureReason::Io, error))?;
        snapshots.push(MetadataSnapshot {
            rating: metadata.rating,
            tags: metadata.tags.unwrap_or_default(),
            feature_data: metadata.feature_data,
        });
    }
    if snapshots.iter().any(metadata_has_unknown_source) {
        Err((
            ApplyFailureReason::InvalidResult,
            "RAW/JPEG metadata contains an unknown or malformed smart-culling source".to_string(),
        ))
    } else if asset_has_conflicting_results(&snapshots) {
        Err((
            ApplyFailureReason::BaselineConflict,
            "RAW/JPEG members contain conflicting rating or color results".to_string(),
        ))
    } else if asset_is_protected(&snapshots) {
        Err((
            ApplyFailureReason::ManualProtection,
            "RAW/JPEG asset contains a user-locked rating or color label".to_string(),
        ))
    } else {
        Ok(())
    }
}

fn ensure_file_baselines_match(
    baselines: &[(PathBuf, FileBaseline)],
) -> Result<(), (ApplyFailureReason, String)> {
    for (path, baseline) in baselines {
        let current = capture_file_baseline(path)
            .map_err(|error| (ApplyFailureReason::AssetChanged, error))?;
        if &current != baseline {
            return Err((
                ApplyFailureReason::AssetChanged,
                format!("photo changed after analysis started: {}", path.display()),
            ));
        }
    }
    Ok(())
}

pub(crate) fn ensure_baseline_matches(
    sidecar_path: &Path,
    baseline: &SidecarBaseline,
) -> Result<(), (ApplyFailureReason, String)> {
    let current =
        capture_sidecar_baseline(sidecar_path).map_err(|error| (ApplyFailureReason::Io, error))?;
    if &current == baseline {
        Ok(())
    } else {
        Err((
            ApplyFailureReason::BaselineConflict,
            "sidecar changed after the smart-culling task started".to_string(),
        ))
    }
}

fn merge_result(metadata: &mut ImageMetadata, result: &ConfirmedResult) {
    metadata.rating = result.rating;

    let mut tags = metadata.tags.take().unwrap_or_default();
    tags.retain(|tag| !tag.starts_with(COLOR_TAG_PREFIX));
    if let Some(color) = result.color_label {
        tags.push(format!("{COLOR_TAG_PREFIX}{}", color.as_tag()));
    }
    metadata.tags = if tags.is_empty() { None } else { Some(tags) };

    let mut feature_data = metadata
        .feature_data
        .take()
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}));
    let record = if result.source == ResultSource::Manual {
        json!({
            "schemaVersion": SCHEMA_VERSION,
            "source": result.source,
            "edited": true,
            "locked": true,
            "assetSynchronized": true,
            "resultId": result.result_id,
            "rating": result.rating,
            "colorLabel": result.color_label,
            "confirmedAt": result.confirmed_at,
        })
    } else {
        json!({
            "schemaVersion": SCHEMA_VERSION,
            "source": result.source,
            "edited": false,
            "locked": false,
            "resultId": result.result_id,
            "rating": result.rating,
            "colorLabel": result.color_label,
            "reasonCodes": result.reason_codes,
            "confidence": result.confidence,
            "mode": result.mode,
            "modelVersion": result.model_version,
            "policyVersion": result.policy_version,
            "confirmedAt": result.confirmed_at,
        })
    };
    feature_data
        .as_object_mut()
        .expect("feature_data was normalized to an object")
        .insert("smartCullingV2".to_string(), record);
    metadata.feature_data = Some(feature_data);
}

pub(crate) fn atomic_write_sidecar(path: &Path, metadata: &ImageMetadata) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("Sidecar has no parent directory: {}", path.display()))?;
    let bytes = serde_json::to_vec_pretty(metadata).map_err(|error| error.to_string())?;
    let mut temporary = NamedTempFile::new_in(parent).map_err(|error| error.to_string())?;
    temporary
        .write_all(&bytes)
        .map_err(|error| error.to_string())?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| error.to_string())?;
    temporary.persist(path).map_err(|error| error.to_string())?;

    #[cfg(unix)]
    fs::File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| error.to_string())?;

    Ok(())
}

#[cfg(test)]
mod asset_tests;

#[cfg(test)]
mod tests {
    use std::fs::File;

    use serde_json::json;
    use tempfile::tempdir;

    use super::*;
    use crate::features::smart_culling::domain::ColorLabel;

    fn write_metadata(path: &Path, metadata: &ImageMetadata) {
        let file = File::create(path).unwrap();
        serde_json::to_writer_pretty(file, metadata).unwrap();
    }

    fn result(source: ResultSource) -> ConfirmedResult {
        ConfirmedResult {
            result_id: "result-1".to_string(),
            source,
            rating: if source == ResultSource::Ai { 4 } else { 0 },
            color_label: if source == ResultSource::Ai {
                Some(ColorLabel::Green)
            } else {
                None
            },
            reason_codes: if source == ResultSource::Ai {
                vec!["sharp_subject".to_string()]
            } else {
                Vec::new()
            },
            confidence: 0.9,
            mode: "auto".to_string(),
            model_version: "test-model".to_string(),
            policy_version: "test-policy".to_string(),
            confirmed_at: "2026-07-15T00:00:00Z".to_string(),
        }
    }

    fn write_item(
        sidecar_path: PathBuf,
        sidecar_baseline: SidecarBaseline,
        result: ConfirmedResult,
    ) -> ConfirmedWrite {
        ConfirmedWrite {
            sidecar_path: sidecar_path.clone(),
            member_sidecar_baselines: vec![(sidecar_path, sidecar_baseline)],
            file_baselines: Vec::new(),
            result,
        }
    }

    #[test]
    fn atomically_merges_ai_result_and_preserves_unrelated_metadata() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("photo.dng.rrdata");
        let metadata = ImageMetadata {
            rating: 0,
            adjustments: json!({ "exposure": 0.75 }),
            tags: Some(vec!["user:portfolio".to_string()]),
            feature_data: Some(json!({ "anotherFeature": { "value": 7 } })),
            ..ImageMetadata::default()
        };
        write_metadata(&path, &metadata);
        let baseline = capture_sidecar_baseline(&path).unwrap();

        let report = apply_confirmed_results(vec![write_item(
            path.clone(),
            baseline,
            result(ResultSource::Ai),
        )]);

        assert_eq!(report.succeeded, vec![path.clone()]);
        assert!(report.failed.is_empty());
        let updated = load_sidecar(&path);
        assert_eq!(updated.rating, 4);
        assert_eq!(
            updated.tags,
            Some(vec![
                "user:portfolio".to_string(),
                "color:green".to_string()
            ])
        );
        assert_eq!(updated.adjustments, json!({ "exposure": 0.75 }));
        assert_eq!(
            updated.feature_data.as_ref().unwrap()["anotherFeature"]["value"],
            7
        );
        assert_eq!(
            updated.feature_data.as_ref().unwrap()["smartCullingV2"]["source"],
            "ai"
        );
        assert_eq!(
            updated.feature_data.as_ref().unwrap()["smartCullingV2"]["locked"],
            false
        );
    }

    #[test]
    fn rejects_a_changed_sidecar_without_overwriting_it() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("photo.jpg.rrdata");
        write_metadata(&path, &ImageMetadata::default());
        let baseline = capture_sidecar_baseline(&path).unwrap();
        let changed = ImageMetadata {
            rating: 5,
            ..ImageMetadata::default()
        };
        write_metadata(&path, &changed);

        let report = apply_confirmed_results(vec![write_item(
            path.clone(),
            baseline,
            result(ResultSource::Ai),
        )]);

        assert!(report.succeeded.is_empty());
        assert_eq!(
            report.failed[0].reason,
            ApplyFailureReason::BaselineConflict
        );
        assert_eq!(load_sidecar(&path).rating, 5);
    }

    #[test]
    fn rejects_manual_metadata_even_when_the_baseline_matches() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("photo.jpg.rrdata");
        let manual = ImageMetadata {
            rating: 5,
            ..ImageMetadata::default()
        };
        write_metadata(&path, &manual);
        let baseline = capture_sidecar_baseline(&path).unwrap();

        let report =
            apply_confirmed_results(vec![write_item(path, baseline, result(ResultSource::Ai))]);

        assert!(report.succeeded.is_empty());
        assert_eq!(
            report.failed[0].reason,
            ApplyFailureReason::ManualProtection
        );
    }

    #[test]
    fn rejects_the_whole_asset_when_a_non_primary_member_is_user_locked() {
        let directory = tempdir().unwrap();
        let raw_sidecar = directory.path().join("IMG_0001.dng.rrdata");
        let jpeg_sidecar = directory.path().join("IMG_0001.jpg.rrdata");
        write_metadata(&raw_sidecar, &ImageMetadata::default());
        write_metadata(
            &jpeg_sidecar,
            &ImageMetadata {
                rating: 5,
                ..ImageMetadata::default()
            },
        );
        let raw_baseline = capture_sidecar_baseline(&raw_sidecar).unwrap();
        let jpeg_baseline = capture_sidecar_baseline(&jpeg_sidecar).unwrap();

        let report = apply_confirmed_results(vec![ConfirmedWrite {
            sidecar_path: raw_sidecar.clone(),
            member_sidecar_baselines: vec![
                (raw_sidecar.clone(), raw_baseline),
                (jpeg_sidecar, jpeg_baseline),
            ],
            file_baselines: Vec::new(),
            result: result(ResultSource::Ai),
        }]);

        assert!(report.succeeded.is_empty());
        assert_eq!(
            report.failed[0].reason,
            ApplyFailureReason::ManualProtection
        );
        assert_eq!(load_sidecar(&raw_sidecar).rating, 0);
    }

    #[test]
    fn an_ai_update_does_not_relock_an_unlocked_manual_result() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("photo.jpg.rrdata");
        write_metadata(
            &path,
            &ImageMetadata {
                rating: 3,
                feature_data: Some(json!({
                    "smartCullingV2": {
                        "source": "manual",
                        "rating": 3,
                        "colorLabel": null,
                        "locked": false
                    }
                })),
                ..ImageMetadata::default()
            },
        );
        let baseline = capture_sidecar_baseline(&path).unwrap();

        let report = apply_confirmed_results(vec![write_item(
            path.clone(),
            baseline,
            result(ResultSource::Ai),
        )]);

        assert_eq!(report.succeeded, vec![path.clone()]);
        let metadata = load_sidecar(&path);
        let record = &metadata.feature_data.unwrap()["smartCullingV2"];
        assert_eq!(record["source"], "ai");
        assert_eq!(record["locked"], false);
    }

    #[test]
    fn persists_a_review_edit_as_manual_when_rating_and_color_are_cancelled() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("photo.jpg.rrdata");
        write_metadata(&path, &ImageMetadata::default());
        let baseline = capture_sidecar_baseline(&path).unwrap();

        let report = apply_confirmed_results(vec![write_item(
            path.clone(),
            baseline,
            result(ResultSource::Manual),
        )]);

        assert_eq!(report.succeeded, vec![path.clone()]);
        let updated = load_sidecar(&path);
        assert_eq!(updated.rating, 0);
        assert_eq!(updated.tags, None);
        assert_eq!(
            updated.feature_data.as_ref().unwrap()["smartCullingV2"]["source"],
            "manual"
        );
        assert_eq!(
            updated.feature_data.as_ref().unwrap()["smartCullingV2"]["locked"],
            true
        );
        assert!(
            updated.feature_data.as_ref().unwrap()["smartCullingV2"]
                .get("reasonCodes")
                .is_none()
        );
        assert!(
            updated.feature_data.as_ref().unwrap()["smartCullingV2"]
                .get("modelVersion")
                .is_none()
        );
        assert_eq!(
            classify_metadata_ownership(&MetadataSnapshot {
                rating: updated.rating,
                tags: updated.tags.unwrap_or_default(),
                feature_data: updated.feature_data,
            }),
            MetadataOwnership::Manual
        );
    }

    #[test]
    fn keeps_successful_items_when_a_later_item_conflicts() {
        let directory = tempdir().unwrap();
        let first = directory.path().join("first.jpg.rrdata");
        let second = directory.path().join("second.jpg.rrdata");
        write_metadata(&first, &ImageMetadata::default());
        write_metadata(&second, &ImageMetadata::default());
        let first_baseline = capture_sidecar_baseline(&first).unwrap();
        let second_baseline = capture_sidecar_baseline(&second).unwrap();
        write_metadata(
            &second,
            &ImageMetadata {
                rating: 5,
                ..ImageMetadata::default()
            },
        );

        let report = apply_confirmed_results(vec![
            write_item(first.clone(), first_baseline, result(ResultSource::Ai)),
            write_item(second.clone(), second_baseline, result(ResultSource::Ai)),
        ]);

        assert_eq!(report.succeeded, vec![first.clone()]);
        assert_eq!(report.failed.len(), 1);
        assert_eq!(load_sidecar(&first).rating, 4);
        assert_eq!(load_sidecar(&second).rating, 5);
    }

    #[test]
    fn rejects_a_photo_that_changed_after_analysis() {
        let directory = tempdir().unwrap();
        let image_path = directory.path().join("photo.jpg");
        let sidecar_path = directory.path().join("photo.jpg.rrdata");
        std::fs::write(&image_path, b"original").unwrap();
        write_metadata(&sidecar_path, &ImageMetadata::default());
        let image_baseline = capture_file_baseline(&image_path).unwrap();
        let sidecar_baseline = capture_sidecar_baseline(&sidecar_path).unwrap();
        std::fs::write(&image_path, b"changed contents").unwrap();

        let report = apply_confirmed_results(vec![ConfirmedWrite {
            sidecar_path: sidecar_path.clone(),
            member_sidecar_baselines: vec![(sidecar_path.clone(), sidecar_baseline)],
            file_baselines: vec![(image_path, image_baseline)],
            result: result(ResultSource::Ai),
        }]);

        assert!(report.succeeded.is_empty());
        assert_eq!(report.failed[0].reason, ApplyFailureReason::AssetChanged);
        assert_eq!(load_sidecar(&sidecar_path).rating, 0);
    }

    #[test]
    fn reconciles_a_user_change_to_an_ai_result_without_restoring_ai_traces() {
        let directory = tempdir().unwrap();
        let image_path = directory.path().join("photo.jpg");
        File::create(&image_path).unwrap();
        let sidecar_path = crate::exif_processing::get_primary_sidecar_path(&image_path);
        write_metadata(
            &sidecar_path,
            &ImageMetadata {
                rating: 0,
                feature_data: Some(json!({
                    "smartCullingV2": {
                        "source": "ai",
                        "rating": 4,
                        "colorLabel": "green",
                        "reasonCodes": ["sharp_subject"]
                    }
                })),
                ..ImageMetadata::default()
            },
        );

        let report = reconcile_manual_ownership(vec![image_path]);

        assert_eq!(report.succeeded, vec![sidecar_path.clone()]);
        let updated = load_sidecar(&sidecar_path);
        let record = &updated.feature_data.unwrap()["smartCullingV2"];
        assert_eq!(record["source"], "manual");
        assert_eq!(record["rating"], 0);
        assert_eq!(record["locked"], true);
        assert!(record.get("reasonCodes").is_none());
    }

    #[test]
    fn records_manual_ownership_for_a_new_user_rating() {
        let directory = tempdir().unwrap();
        let image_path = directory.path().join("manual.jpg");
        File::create(&image_path).unwrap();
        let sidecar_path = crate::exif_processing::get_primary_sidecar_path(&image_path);
        write_metadata(
            &sidecar_path,
            &ImageMetadata {
                rating: 3,
                ..ImageMetadata::default()
            },
        );

        let report = reconcile_manual_ownership(vec![image_path]);

        assert_eq!(report.succeeded, vec![sidecar_path.clone()]);
        let updated = load_sidecar(&sidecar_path);
        let record = &updated.feature_data.unwrap()["smartCullingV2"];
        assert_eq!(record["source"], "manual");
        assert_eq!(record["rating"], 3);
        assert_eq!(record["edited"], true);
        assert_eq!(record["locked"], true);

        let mut cancelled = load_sidecar(&sidecar_path);
        cancelled.rating = 0;
        write_metadata(&sidecar_path, &cancelled);
        let report = reconcile_manual_ownership(vec![directory.path().join("manual.jpg")]);

        assert_eq!(report.succeeded, vec![sidecar_path.clone()]);
        let updated = load_sidecar(&sidecar_path);
        assert_eq!(updated.feature_data.unwrap()["smartCullingV2"]["rating"], 0);
    }

    #[test]
    fn does_not_reconcile_over_an_unknown_source_record() {
        let directory = tempdir().unwrap();
        let image_path = directory.path().join("photo.jpg");
        File::create(&image_path).unwrap();
        let sidecar_path = crate::exif_processing::get_primary_sidecar_path(&image_path);
        write_metadata(
            &sidecar_path,
            &ImageMetadata {
                rating: 3,
                feature_data: Some(json!({
                    "smartCullingV2": {
                        "source": "unknown",
                        "rating": 2,
                        "colorLabel": null
                    }
                })),
                ..ImageMetadata::default()
            },
        );

        let report = reconcile_manual_ownership(vec![image_path]);

        assert!(report.succeeded.is_empty());
        assert_eq!(report.failed[0].reason, ApplyFailureReason::InvalidResult);
        let metadata = load_sidecar(&sidecar_path);
        assert_eq!(
            metadata.feature_data.unwrap()["smartCullingV2"]["source"],
            "unknown"
        );
    }
}
