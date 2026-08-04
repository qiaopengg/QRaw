use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde_json::{Value, json};

use crate::exif_processing::get_primary_sidecar_path;
use crate::image_processing::ImageMetadata;

use super::super::domain::{
    MetadataSnapshot, asset_has_conflicting_results, metadata_has_unknown_source,
};
use super::baseline::capture_sidecar_baseline;
use super::catalog::{read_sidecar_strict, resolve_asset_member_groups};
use super::persistence::{ApplyFailure, ApplyFailureReason, ApplyReport, ensure_baseline_matches};
use super::sidecar_transaction::write_sidecar_transaction_guarded;

const COLOR_TAG_PREFIX: &str = "color:";
const SCHEMA_VERSION: u32 = 1;

pub(crate) fn change_asset_lock_state(
    paths: Vec<PathBuf>,
    locked: bool,
) -> Result<ApplyReport, String> {
    if paths.is_empty() {
        return Err("Select at least one photo before changing its lock".to_string());
    }
    let requested = paths.into_iter().collect::<BTreeSet<_>>();
    let mut report = ApplyReport::default();
    let member_groups =
        match resolve_asset_member_groups(&requested.iter().cloned().collect::<Vec<_>>()) {
            Ok(groups) => groups,
            Err(_) => resolve_groups_individually(&requested, &mut report),
        };
    for member_paths in member_groups {
        let group_report = set_asset_lock_state(member_paths, locked);
        report.succeeded.extend(group_report.succeeded);
        report.failed.extend(group_report.failed);
        report.unchanged.extend(group_report.unchanged);
    }
    Ok(report)
}

pub(crate) fn set_asset_lock_state(paths: Vec<PathBuf>, locked: bool) -> ApplyReport {
    let mut report = ApplyReport::default();
    let unique_paths = paths.into_iter().collect::<BTreeSet<_>>();
    let primary_sidecar = unique_paths
        .first()
        .map(|path| get_primary_sidecar_path(path))
        .unwrap_or_default();
    if let Err((reason, detail)) = ensure_members_consistent(&unique_paths) {
        report.failed.push(ApplyFailure {
            sidecar_path: primary_sidecar,
            reason,
            detail,
        });
        return report;
    }

    let mut baselines = Vec::with_capacity(unique_paths.len());
    let mut updates = Vec::with_capacity(unique_paths.len());
    for image_path in &unique_paths {
        let sidecar_path = get_primary_sidecar_path(&image_path);
        let baseline = match capture_sidecar_baseline(&sidecar_path) {
            Ok(baseline) => baseline,
            Err(detail) => {
                report.failed.push(ApplyFailure {
                    sidecar_path: primary_sidecar,
                    reason: ApplyFailureReason::Io,
                    detail,
                });
                return report;
            }
        };
        let mut metadata = match read_sidecar_strict(&sidecar_path) {
            Ok(metadata) => metadata,
            Err(detail) => {
                report.failed.push(ApplyFailure {
                    sidecar_path: primary_sidecar,
                    reason: ApplyFailureReason::Io,
                    detail,
                });
                return report;
            }
        };
        match update_lock_record(&mut metadata, locked) {
            Ok(true) => updates.push((sidecar_path.clone(), metadata)),
            Ok(false) => {}
            Err((reason, detail)) => {
                report.failed.push(ApplyFailure {
                    sidecar_path: primary_sidecar,
                    reason,
                    detail,
                });
                return report;
            }
        }
        baselines.push((sidecar_path, baseline));
    }

    if updates.is_empty() {
        report.unchanged.push(primary_sidecar);
        return report;
    }
    for (sidecar_path, baseline) in &baselines {
        if let Err((reason, detail)) = ensure_baseline_matches(sidecar_path, baseline) {
            report.failed.push(ApplyFailure {
                sidecar_path: primary_sidecar,
                reason,
                detail,
            });
            return report;
        }
    }
    match write_sidecar_transaction_guarded(&updates, || {
        for (sidecar_path, baseline) in &baselines {
            ensure_baseline_matches(sidecar_path, baseline).map_err(|(_, detail)| detail)?;
        }
        Ok(())
    }) {
        Ok(()) => report.succeeded.push(primary_sidecar),
        Err(detail) => report.failed.push(ApplyFailure {
            sidecar_path: primary_sidecar,
            reason: ApplyFailureReason::Io,
            detail,
        }),
    }
    report
}

fn resolve_groups_individually(
    requested: &BTreeSet<PathBuf>,
    report: &mut ApplyReport,
) -> Vec<Vec<PathBuf>> {
    let mut groups = BTreeMap::<PathBuf, Vec<PathBuf>>::new();
    for path in requested {
        match resolve_asset_member_groups(std::slice::from_ref(path)) {
            Ok(resolved) => {
                for members in resolved {
                    if let Some(key) = members.first() {
                        groups.entry(key.clone()).or_insert(members);
                    }
                }
            }
            Err(detail) => report.failed.push(ApplyFailure {
                sidecar_path: get_primary_sidecar_path(path),
                reason: ApplyFailureReason::InvalidResult,
                detail,
            }),
        }
    }
    groups.into_values().collect()
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
    let had_existing_record = existing_record.is_some();
    let has_visible_result = metadata.rating > 0 || color_label.is_some();
    if existing_record.is_none() && !has_visible_result && !locked {
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
    if had_existing_record && current_locked == locked && record_matches {
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
    use std::path::Path;

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
    fn locking_an_untracked_blank_photo_creates_a_zero_star_manual_lock() {
        let directory = tempdir().unwrap();
        let image_path = directory.path().join("photo.jpg");
        File::create(&image_path).unwrap();
        let sidecar_path = get_primary_sidecar_path(&image_path);

        let report = set_asset_lock_state(vec![image_path], true);

        assert_eq!(report.succeeded, vec![sidecar_path.clone()]);
        let metadata = read_sidecar_strict(&sidecar_path).unwrap();
        assert_eq!(metadata.rating, 0);
        let record = &metadata.feature_data.unwrap()["smartCullingV2"];
        assert_eq!(record["source"], "manual");
        assert_eq!(record["locked"], true);
        assert_eq!(record["rating"], 0);
    }

    #[test]
    fn batch_lock_keeps_valid_assets_and_reports_unresolved_assets() {
        let directory = tempdir().unwrap();
        let valid = directory.path().join("valid.jpg");
        let missing = directory.path().join("missing.jpg");
        File::create(&valid).unwrap();

        let report = change_asset_lock_state(vec![valid.clone(), missing.clone()], true).unwrap();

        assert_eq!(report.succeeded, vec![get_primary_sidecar_path(&valid)]);
        assert_eq!(report.failed.len(), 1);
        assert_eq!(
            report.failed[0].sidecar_path,
            get_primary_sidecar_path(&missing)
        );
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
