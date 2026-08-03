use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::exif_processing::get_primary_sidecar_path;
use crate::image_processing::ImageMetadata;

use super::super::domain::{
    MetadataSnapshot, asset_has_conflicting_results, metadata_has_unknown_source,
};
use super::baseline::capture_sidecar_baseline;
use super::catalog::{read_sidecar_strict, resolve_asset_member_groups};
use super::persistence::{
    ApplyFailure, ApplyFailureReason, ApplyReport, atomic_write_sidecar, ensure_baseline_matches,
};

const COLOR_TAG_PREFIX: &str = "color:";
const SCHEMA_VERSION: u32 = 1;

pub(crate) fn change_asset_lock_state(paths: Vec<PathBuf>, locked: bool) -> Result<(), String> {
    if paths.is_empty() {
        return Err("Select at least one photo before changing its lock".to_string());
    }
    let member_groups = resolve_asset_member_groups(&paths)?;
    let mut report = ApplyReport::default();
    for member_paths in member_groups {
        let group_report = set_asset_lock_state(member_paths, locked);
        report.succeeded.extend(group_report.succeeded);
        report.failed.extend(group_report.failed);
    }
    if let Some(failure) = report.failed.first() {
        Err(format!(
            "Failed to change asset lock at {}: {}",
            failure.sidecar_path.display(),
            failure.detail
        ))
    } else {
        Ok(())
    }
}

pub(crate) fn set_asset_lock_state(paths: Vec<PathBuf>, locked: bool) -> ApplyReport {
    let mut report = ApplyReport::default();
    let unique_paths = paths.into_iter().collect::<BTreeSet<_>>();
    if let Err((reason, detail)) = ensure_members_consistent(&unique_paths) {
        let sidecar_path = unique_paths
            .first()
            .map(|path| get_primary_sidecar_path(path))
            .unwrap_or_default();
        report.failed.push(ApplyFailure {
            sidecar_path,
            reason,
            detail,
        });
        return report;
    }

    for image_path in unique_paths {
        let sidecar_path = get_primary_sidecar_path(&image_path);
        match update_one(&sidecar_path, locked) {
            Ok(true) => report.succeeded.push(sidecar_path),
            Ok(false) => {}
            Err((reason, detail)) => report.failed.push(ApplyFailure {
                sidecar_path,
                reason,
                detail,
            }),
        }
    }

    report
}

fn ensure_members_consistent(
    paths: &BTreeSet<PathBuf>,
) -> Result<(), (ApplyFailureReason, String)> {
    let mut snapshots = Vec::with_capacity(paths.len());
    for image_path in paths {
        let sidecar_path = get_primary_sidecar_path(image_path);
        let metadata =
            read_sidecar_strict(&sidecar_path).map_err(|error| (ApplyFailureReason::Io, error))?;
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
    } else {
        Ok(())
    }
}

fn update_one(sidecar_path: &Path, locked: bool) -> Result<bool, (ApplyFailureReason, String)> {
    let baseline =
        capture_sidecar_baseline(sidecar_path).map_err(|error| (ApplyFailureReason::Io, error))?;
    let mut metadata =
        read_sidecar_strict(sidecar_path).map_err(|error| (ApplyFailureReason::Io, error))?;
    if !update_lock_record(&mut metadata, locked)? {
        return Ok(false);
    }

    ensure_baseline_matches(sidecar_path, &baseline)?;
    atomic_write_sidecar(sidecar_path, &metadata)
        .map_err(|error| (ApplyFailureReason::Io, error))?;
    Ok(true)
}

fn update_lock_record(
    metadata: &mut ImageMetadata,
    locked: bool,
) -> Result<bool, (ApplyFailureReason, String)> {
    if metadata
        .feature_data
        .as_ref()
        .is_some_and(|value| !value.is_object())
    {
        return Err((
            ApplyFailureReason::InvalidResult,
            "featureData must be an object before its lock can be changed".to_string(),
        ));
    }
    let color_label = metadata
        .tags
        .as_ref()
        .into_iter()
        .flatten()
        .find_map(|tag| tag.strip_prefix(COLOR_TAG_PREFIX))
        .map(str::to_string);
    let existing_record = metadata
        .feature_data
        .as_ref()
        .and_then(|value| value.get("smartCullingV2"))
        .cloned();
    let has_visible_result = metadata.rating > 0 || color_label.is_some();
    if existing_record.is_none() && !has_visible_result {
        return Ok(false);
    }

    let mut record = match existing_record {
        Some(Value::Object(record)) => {
            let valid_source = matches!(
                record.get("source").and_then(Value::as_str),
                Some("ai" | "manual")
            );
            let valid_rating = record
                .get("rating")
                .and_then(Value::as_u64)
                .is_some_and(|rating| rating <= 5);
            let valid_color = record
                .get("colorLabel")
                .is_none_or(|color| color.is_null() || color.is_string());
            let valid_locked = record.get("locked").is_none_or(Value::is_boolean);
            if !valid_source || !valid_rating || !valid_color || !valid_locked {
                return Err((
                    ApplyFailureReason::InvalidResult,
                    "smartCullingV2 has an unknown or malformed source record".to_string(),
                ));
            }
            Value::Object(record)
        }
        Some(_) => {
            return Err((
                ApplyFailureReason::InvalidResult,
                "smartCullingV2 must be an object before its lock can be changed".to_string(),
            ));
        }
        None => manual_record(metadata.rating, color_label.as_deref()),
    };
    let record_matches = record.get("rating").and_then(Value::as_u64)
        == Some(metadata.rating as u64)
        && record.get("colorLabel").and_then(Value::as_str) == color_label.as_deref();
    if !record_matches {
        record = manual_record(metadata.rating, color_label.as_deref());
    }
    let source = record.get("source").and_then(Value::as_str);
    let current_locked = record
        .get("locked")
        .and_then(Value::as_bool)
        .unwrap_or(source == Some("manual"));
    if current_locked == locked && record_matches {
        return Ok(false);
    }

    let Some(record) = record.as_object_mut() else {
        unreachable!("records are normalized to JSON objects");
    };
    record.insert("locked".to_string(), json!(locked));
    record.insert("assetSynchronized".to_string(), json!(true));
    record.insert(
        "lockUpdatedAt".to_string(),
        json!(chrono::Utc::now().to_rfc3339()),
    );

    let feature_data = match metadata.feature_data.take() {
        Some(Value::Object(object)) => Value::Object(object),
        Some(_) => {
            return Err((
                ApplyFailureReason::InvalidResult,
                "featureData must be an object before its lock can be changed".to_string(),
            ));
        }
        None => json!({}),
    };
    let mut feature_data = feature_data
        .as_object()
        .cloned()
        .expect("feature data was normalized to an object");
    feature_data.insert("smartCullingV2".to_string(), Value::Object(record.clone()));
    metadata.feature_data = Some(Value::Object(feature_data));
    Ok(true)
}

fn manual_record(rating: u8, color_label: Option<&str>) -> Value {
    json!({
        "schemaVersion": SCHEMA_VERSION,
        "source": "manual",
        "edited": true,
        "rating": rating,
        "colorLabel": color_label,
        "confirmedAt": chrono::Utc::now().to_rfc3339(),
    })
}

#[cfg(test)]
mod tests {
    use std::fs::File;

    use serde_json::json;
    use tempfile::tempdir;

    use super::*;
    use crate::features::smart_culling::domain::{
        MetadataOwnership, MetadataSnapshot, classify_metadata_ownership,
    };

    fn write_metadata(path: &Path, metadata: &ImageMetadata) {
        let file = File::create(path).unwrap();
        serde_json::to_writer_pretty(file, metadata).unwrap();
    }

    #[test]
    fn unlocking_a_historical_manual_rating_makes_it_eligible() {
        let directory = tempdir().unwrap();
        let image_path = directory.path().join("photo.jpg");
        File::create(&image_path).unwrap();
        let sidecar_path = get_primary_sidecar_path(&image_path);
        write_metadata(
            &sidecar_path,
            &ImageMetadata {
                rating: 4,
                ..ImageMetadata::default()
            },
        );

        let report = set_asset_lock_state(vec![image_path], false);

        assert_eq!(report.succeeded, vec![sidecar_path.clone()]);
        let metadata = read_sidecar_strict(&sidecar_path).unwrap();
        assert_eq!(
            classify_metadata_ownership(&MetadataSnapshot {
                rating: metadata.rating,
                tags: metadata.tags.unwrap_or_default(),
                feature_data: metadata.feature_data,
            }),
            MetadataOwnership::Unprotected
        );
    }

    #[test]
    fn a_user_can_explicitly_lock_an_unchanged_ai_result() {
        let directory = tempdir().unwrap();
        let image_path = directory.path().join("photo.jpg");
        File::create(&image_path).unwrap();
        let sidecar_path = get_primary_sidecar_path(&image_path);
        write_metadata(
            &sidecar_path,
            &ImageMetadata {
                rating: 4,
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

        let report = set_asset_lock_state(vec![image_path], true);

        assert_eq!(report.succeeded, vec![sidecar_path.clone()]);
        let metadata = read_sidecar_strict(&sidecar_path).unwrap();
        let record = &metadata.feature_data.as_ref().unwrap()["smartCullingV2"];
        assert_eq!(record["source"], "ai");
        assert_eq!(record["locked"], true);
        assert_eq!(record["assetSynchronized"], true);
    }

    #[test]
    fn unlocking_an_untracked_blank_photo_does_not_create_a_sidecar() {
        let directory = tempdir().unwrap();
        let image_path = directory.path().join("photo.jpg");
        File::create(&image_path).unwrap();
        let sidecar_path = get_primary_sidecar_path(&image_path);

        let report = set_asset_lock_state(vec![image_path], false);

        assert!(report.succeeded.is_empty());
        assert!(report.failed.is_empty());
        assert!(!sidecar_path.exists());
    }

    #[test]
    fn conflicting_member_results_are_not_unlocked_or_overwritten() {
        let directory = tempdir().unwrap();
        let raw_path = directory.path().join("photo.dng");
        let jpeg_path = directory.path().join("photo.jpg");
        File::create(&raw_path).unwrap();
        File::create(&jpeg_path).unwrap();
        for (path, rating) in [(&raw_path, 3), (&jpeg_path, 4)] {
            write_metadata(
                &get_primary_sidecar_path(path),
                &ImageMetadata {
                    rating,
                    feature_data: Some(json!({
                        "smartCullingV2": {
                            "source": "manual",
                            "rating": rating,
                            "colorLabel": null,
                            "locked": true
                        }
                    })),
                    ..ImageMetadata::default()
                },
            );
        }

        let report = set_asset_lock_state(vec![raw_path.clone(), jpeg_path.clone()], false);

        assert!(report.succeeded.is_empty());
        assert_eq!(
            report.failed[0].reason,
            ApplyFailureReason::BaselineConflict
        );
        for path in [&raw_path, &jpeg_path] {
            let metadata = read_sidecar_strict(&get_primary_sidecar_path(path)).unwrap();
            assert_eq!(
                metadata.feature_data.unwrap()["smartCullingV2"]["locked"],
                true
            );
        }
    }

    #[test]
    fn an_unknown_source_is_preserved_instead_of_being_unlocked() {
        let directory = tempdir().unwrap();
        let image_path = directory.path().join("photo.jpg");
        File::create(&image_path).unwrap();
        let sidecar_path = get_primary_sidecar_path(&image_path);
        write_metadata(
            &sidecar_path,
            &ImageMetadata {
                feature_data: Some(json!({
                    "smartCullingV2": {
                        "source": "unknown",
                        "rating": 0,
                        "colorLabel": null,
                        "locked": true
                    }
                })),
                ..ImageMetadata::default()
            },
        );

        let report = set_asset_lock_state(vec![image_path], false);

        assert!(report.succeeded.is_empty());
        assert_eq!(report.failed[0].reason, ApplyFailureReason::InvalidResult);
        let metadata = read_sidecar_strict(&sidecar_path).unwrap();
        assert_eq!(
            metadata.feature_data.unwrap()["smartCullingV2"]["source"],
            "unknown"
        );
    }
}
