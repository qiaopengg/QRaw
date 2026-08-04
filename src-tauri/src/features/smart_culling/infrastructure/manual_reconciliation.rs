use std::collections::BTreeSet;
use std::path::PathBuf;

use serde_json::{Value, json};

use crate::exif_processing::get_primary_sidecar_path;
use crate::image_processing::ImageMetadata;

use super::super::domain::{
    MetadataOwnership, MetadataSnapshot, classify_metadata_ownership, metadata_has_unknown_source,
};
use super::baseline::{SidecarBaseline, capture_sidecar_baseline};
use super::catalog::{read_sidecar_strict, resolve_asset_member_groups};
use super::persistence::{ApplyFailure, ApplyFailureReason, ApplyReport, ensure_baseline_matches};
use super::sidecar_transaction::write_sidecar_transaction_guarded;

const COLOR_TAG_PREFIX: &str = "color:";
const SCHEMA_VERSION: u32 = 1;

struct MemberState {
    sidecar_path: PathBuf,
    baseline: SidecarBaseline,
    metadata: ImageMetadata,
    ownership: MetadataOwnership,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct VisibleResult {
    rating: u8,
    color_label: Option<String>,
}

pub(crate) fn reconcile_manual_ownership(paths: Vec<PathBuf>) -> ApplyReport {
    let requested = paths.into_iter().collect::<BTreeSet<_>>();
    if requested.is_empty() {
        return ApplyReport::default();
    }
    let member_groups =
        match resolve_asset_member_groups(&requested.iter().cloned().collect::<Vec<_>>()) {
            Ok(groups) => groups,
            Err(detail) => {
                return ApplyReport {
                    succeeded: Vec::new(),
                    failed: vec![ApplyFailure {
                        sidecar_path: get_primary_sidecar_path(
                            requested.first().expect("the requested set is not empty"),
                        ),
                        reason: ApplyFailureReason::Io,
                        detail,
                    }],
                    unchanged: Vec::new(),
                };
            }
        };

    let mut report = ApplyReport::default();
    for member_paths in member_groups {
        let sidecar_path = get_primary_sidecar_path(
            member_paths
                .first()
                .expect("resolved assets always contain a member"),
        );
        match reconcile_asset(&member_paths, &requested) {
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

fn reconcile_asset(
    member_paths: &[PathBuf],
    requested: &BTreeSet<PathBuf>,
) -> Result<bool, (ApplyFailureReason, String)> {
    let mut members = Vec::with_capacity(member_paths.len());
    for image_path in member_paths {
        let sidecar_path = get_primary_sidecar_path(image_path);
        let baseline = capture_sidecar_baseline(&sidecar_path)
            .map_err(|error| (ApplyFailureReason::Io, error))?;
        let metadata =
            read_sidecar_strict(&sidecar_path).map_err(|error| (ApplyFailureReason::Io, error))?;
        let snapshot = snapshot(&metadata);
        if metadata_has_unknown_source(&snapshot) {
            return Err((
                ApplyFailureReason::InvalidResult,
                "RAW/JPEG metadata contains an unknown or malformed smart-culling source"
                    .to_string(),
            ));
        }
        let ownership = classify_metadata_ownership(&snapshot);
        members.push(MemberState {
            sidecar_path,
            baseline,
            metadata,
            ownership,
        });
    }

    let mut requested_results = BTreeSet::new();
    for (image_path, member) in member_paths.iter().zip(&members) {
        if requested.contains(image_path) && member.ownership == MetadataOwnership::Manual {
            requested_results.insert(visible_result(&member.metadata)?);
        }
    }
    if requested_results.is_empty() {
        return Ok(false);
    }
    if requested_results.len() > 1 {
        return Err(member_conflict());
    }
    let result = requested_results
        .into_iter()
        .next()
        .expect("one requested manual result was found");

    for member in &members {
        if member.ownership == MetadataOwnership::Manual
            && visible_result(&member.metadata)? != result
            && !is_untouched_synchronized_member(&member.metadata)?
        {
            return Err(member_conflict());
        }
    }
    for member in &members {
        ensure_baseline_matches(&member.sidecar_path, &member.baseline)?;
    }
    for member in &mut members {
        merge_manual_result(&mut member.metadata, &result)?;
    }
    for member in &members {
        ensure_baseline_matches(&member.sidecar_path, &member.baseline)?;
    }
    let baselines = members
        .iter()
        .map(|member| (member.sidecar_path.clone(), member.baseline.clone()))
        .collect::<Vec<_>>();
    let updates = members
        .into_iter()
        .map(|member| (member.sidecar_path, member.metadata))
        .collect::<Vec<_>>();
    write_sidecar_transaction_guarded(&updates, || {
        for (sidecar_path, baseline) in &baselines {
            ensure_baseline_matches(sidecar_path, baseline).map_err(|(_, detail)| detail)?;
        }
        Ok(())
    })
    .map_err(|error| (ApplyFailureReason::Io, error))?;
    Ok(true)
}

fn snapshot(metadata: &ImageMetadata) -> MetadataSnapshot {
    MetadataSnapshot {
        rating: metadata.rating,
        tags: metadata.tags.clone().unwrap_or_default(),
        feature_data: metadata.feature_data.clone(),
    }
}

fn visible_result(metadata: &ImageMetadata) -> Result<VisibleResult, (ApplyFailureReason, String)> {
    let colors = metadata
        .tags
        .as_ref()
        .into_iter()
        .flatten()
        .filter_map(|tag| tag.strip_prefix(COLOR_TAG_PREFIX))
        .collect::<Vec<_>>();
    if colors.len() > 1 {
        return Err((
            ApplyFailureReason::InvalidResult,
            "metadata contains multiple color labels".to_string(),
        ));
    }
    Ok(VisibleResult {
        rating: metadata.rating,
        color_label: colors.first().map(|color| (*color).to_string()),
    })
}

fn merge_manual_result(
    metadata: &mut ImageMetadata,
    result: &VisibleResult,
) -> Result<(), (ApplyFailureReason, String)> {
    metadata.rating = result.rating;
    let mut tags = metadata.tags.take().unwrap_or_default();
    tags.retain(|tag| !tag.starts_with(COLOR_TAG_PREFIX));
    if let Some(color) = &result.color_label {
        tags.push(format!("{COLOR_TAG_PREFIX}{color}"));
    }
    metadata.tags = (!tags.is_empty()).then_some(tags);

    let mut feature_data = match metadata.feature_data.take() {
        Some(Value::Object(object)) => object,
        Some(_) => {
            return Err((
                ApplyFailureReason::InvalidResult,
                "featureData must be an object before manual ownership can be recorded".to_string(),
            ));
        }
        None => serde_json::Map::new(),
    };
    feature_data.insert(
        "smartCullingV2".to_string(),
        json!({
            "schemaVersion": SCHEMA_VERSION,
            "source": "manual",
            "edited": true,
            "locked": true,
            "assetSynchronized": true,
            "rating": result.rating,
            "colorLabel": result.color_label,
            "confirmedAt": chrono::Utc::now().to_rfc3339(),
        }),
    );
    metadata.feature_data = Some(Value::Object(feature_data));
    Ok(())
}

fn is_untouched_synchronized_member(
    metadata: &ImageMetadata,
) -> Result<bool, (ApplyFailureReason, String)> {
    let Some(record) = metadata
        .feature_data
        .as_ref()
        .and_then(|feature_data| feature_data.get("smartCullingV2"))
    else {
        return Ok(false);
    };
    if !matches!(
        record.get("source").and_then(Value::as_str),
        Some("ai" | "manual")
    ) || record.get("assetSynchronized").and_then(Value::as_bool) != Some(true)
    {
        return Ok(false);
    }
    let visible = visible_result(metadata)?;
    Ok(
        record.get("rating").and_then(Value::as_u64) == Some(visible.rating as u64)
            && record.get("colorLabel").and_then(Value::as_str) == visible.color_label.as_deref(),
    )
}

fn member_conflict() -> (ApplyFailureReason, String) {
    (
        ApplyFailureReason::BaselineConflict,
        "RAW/JPEG members contain conflicting user results".to_string(),
    )
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::fs::File;
    use std::path::Path;

    use tempfile::tempdir;

    use super::*;

    fn write_metadata(path: &Path, metadata: &ImageMetadata) {
        let file = File::create(path).unwrap();
        serde_json::to_writer_pretty(file, metadata).unwrap();
    }

    fn ai_metadata(rating: u8) -> ImageMetadata {
        ImageMetadata {
            rating,
            exif: Some(HashMap::from([(
                "DateTimeOriginal".to_string(),
                "2026-08-04 12:00:00".to_string(),
            )])),
            feature_data: Some(json!({
                "smartCullingV2": {
                    "source": "ai",
                    "rating": rating,
                    "colorLabel": null,
                    "locked": false
                }
            })),
            ..ImageMetadata::default()
        }
    }

    #[test]
    fn synchronizes_a_user_edit_to_the_whole_raw_jpeg_asset() {
        let directory = tempdir().unwrap();
        let raw = directory.path().join("IMG_0001.dng");
        let jpeg = directory.path().join("IMG_0001.jpg");
        File::create(&raw).unwrap();
        File::create(&jpeg).unwrap();
        write_metadata(&get_primary_sidecar_path(&raw), &ai_metadata(4));
        let mut edited = ai_metadata(4);
        edited.rating = 5;
        write_metadata(&get_primary_sidecar_path(&jpeg), &edited);

        let report = reconcile_manual_ownership(vec![jpeg.clone()]);

        assert_eq!(report.succeeded.len(), 1);
        for image in [&raw, &jpeg] {
            let metadata = read_sidecar_strict(&get_primary_sidecar_path(image)).unwrap();
            assert_eq!(metadata.rating, 5);
            let record = &metadata.feature_data.unwrap()["smartCullingV2"];
            assert_eq!(record["source"], "manual");
            assert_eq!(record["locked"], true);
        }

        let mut edited_again = read_sidecar_strict(&get_primary_sidecar_path(&jpeg)).unwrap();
        edited_again.rating = 3;
        write_metadata(&get_primary_sidecar_path(&jpeg), &edited_again);

        let report = reconcile_manual_ownership(vec![jpeg.clone()]);

        assert_eq!(report.succeeded.len(), 1);
        assert_eq!(
            read_sidecar_strict(&get_primary_sidecar_path(&raw))
                .unwrap()
                .rating,
            3
        );
        assert_eq!(
            read_sidecar_strict(&get_primary_sidecar_path(&jpeg))
                .unwrap()
                .rating,
            3
        );
    }

    #[test]
    fn preserves_conflicting_existing_user_results() {
        let directory = tempdir().unwrap();
        let raw = directory.path().join("IMG_0002.dng");
        let jpeg = directory.path().join("IMG_0002.jpg");
        File::create(&raw).unwrap();
        File::create(&jpeg).unwrap();
        write_metadata(
            &get_primary_sidecar_path(&raw),
            &ImageMetadata {
                rating: 5,
                exif: ai_metadata(0).exif,
                ..ImageMetadata::default()
            },
        );
        write_metadata(
            &get_primary_sidecar_path(&jpeg),
            &ImageMetadata {
                rating: 4,
                exif: ai_metadata(0).exif,
                ..ImageMetadata::default()
            },
        );

        let report = reconcile_manual_ownership(vec![raw.clone(), jpeg.clone()]);

        assert!(report.succeeded.is_empty());
        assert_eq!(
            report.failed[0].reason,
            ApplyFailureReason::BaselineConflict
        );
        assert_eq!(
            read_sidecar_strict(&get_primary_sidecar_path(&raw))
                .unwrap()
                .rating,
            5
        );
        assert_eq!(
            read_sidecar_strict(&get_primary_sidecar_path(&jpeg))
                .unwrap()
                .rating,
            4
        );
    }
}
