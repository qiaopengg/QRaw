use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::exif_processing::{get_creation_date_from_path, try_get_exif_creation_date};
use crate::formats::{is_raw_file, is_supported_image_file};
use crate::image_processing::ImageMetadata;

use super::super::domain::{
    AssetCandidate, AssetDecision, AssetMemberKind, MetadataSnapshot, SkipReason,
    asset_has_conflicting_results, asset_is_protected, group_assets, metadata_has_unknown_source,
};
use super::baseline::{
    FileBaseline, SidecarBaseline, capture_file_baseline, capture_sidecar_baseline,
};
use super::sidecar_transaction::recover_pending_sidecar_transactions;
use crate::exif_processing::get_primary_sidecar_path;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Catalog {
    pub assets: Vec<CatalogAsset>,
    pub skipped: Vec<CatalogSkip>,
    pub failures: Vec<CatalogFailure>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CatalogAssetStatus {
    Eligible,
    Protected,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CatalogAsset {
    pub primary_path: PathBuf,
    pub display_path: PathBuf,
    pub member_paths: Vec<PathBuf>,
    pub capture_time_millis: i64,
    pub capture_time_from_exif: bool,
    pub sequence_number: Option<u64>,
    pub file_baselines: Vec<(PathBuf, FileBaseline)>,
    pub member_sidecar_baselines: Vec<(PathBuf, SidecarBaseline)>,
    pub sidecar_path: PathBuf,
    pub status: CatalogAssetStatus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CatalogSkipReason {
    ExcludedFormat,
    AmbiguousPair,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CatalogSkip {
    pub paths: Vec<PathBuf>,
    pub reason: CatalogSkipReason,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CatalogFailure {
    pub path: PathBuf,
    pub reason: String,
}

pub(crate) fn scan_catalog(root: &Path) -> Result<Catalog, String> {
    if !root.is_dir() {
        return Err(format!(
            "Smart-culling root is not a directory: {}",
            root.display()
        ));
    }

    recover_pending_sidecar_transactions(root)?;

    let (decisions, mut skipped, mut failures) = scan_direct_asset_decisions(root)?;
    let mut assets = Vec::new();
    for decision in decisions {
        match decision {
            AssetDecision::Skipped {
                paths,
                reason: SkipReason::AmbiguousPair,
            } => skipped.push(CatalogSkip {
                paths,
                reason: CatalogSkipReason::AmbiguousPair,
            }),
            AssetDecision::Eligible {
                primary_path,
                display_path,
                member_paths,
            } => match inspect_asset(primary_path, display_path, member_paths) {
                Ok(asset) => assets.push(asset),
                Err(failure) => failures.push(failure),
            },
        }
    }

    assets.sort_by(|left, right| left.primary_path.cmp(&right.primary_path));
    skipped.sort_by(|left, right| left.paths[0].cmp(&right.paths[0]));
    failures.sort_by(|left, right| left.path.cmp(&right.path));

    Ok(Catalog {
        assets,
        skipped,
        failures,
    })
}

fn scan_direct_asset_decisions(
    root: &Path,
) -> Result<(Vec<AssetDecision>, Vec<CatalogSkip>, Vec<CatalogFailure>), String> {
    let mut candidates = Vec::new();
    let mut skipped = Vec::new();
    let mut failures = Vec::new();
    let entries = fs::read_dir(root)
        .map_err(|error| format!("Cannot read smart-culling root {}: {error}", root.display()))?;

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                failures.push(CatalogFailure {
                    path: root.to_path_buf(),
                    reason: error.to_string(),
                });
                continue;
            }
        };
        let path = entry.path();
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(error) => {
                failures.push(CatalogFailure {
                    path,
                    reason: error.to_string(),
                });
                continue;
            }
        };

        if file_type.is_symlink() || file_type.is_dir() {
            continue;
        }
        if !file_type.is_file() || !is_supported_image_file(&path) {
            continue;
        }
        if is_explicitly_excluded(&path) {
            skipped.push(CatalogSkip {
                paths: vec![path],
                reason: CatalogSkipReason::ExcludedFormat,
            });
            continue;
        }

        let kind = if is_raw_file(&path) {
            AssetMemberKind::Raw
        } else if is_jpeg(&path) {
            AssetMemberKind::Jpeg
        } else {
            AssetMemberKind::Other
        };
        let capture_identity = matches!(kind, AssetMemberKind::Raw | AssetMemberKind::Jpeg)
            .then(|| try_get_exif_creation_date(&path).map(|date| date.timestamp()))
            .flatten();
        candidates.push(AssetCandidate {
            path,
            kind,
            capture_identity,
        });
    }

    Ok((group_assets(candidates), skipped, failures))
}

pub(crate) fn resolve_asset_member_groups(paths: &[PathBuf]) -> Result<Vec<Vec<PathBuf>>, String> {
    let mut by_folder = BTreeMap::<PathBuf, Vec<PathBuf>>::new();
    for path in paths {
        let parent = path
            .parent()
            .ok_or_else(|| format!("Image has no parent folder: {}", path.display()))?;
        by_folder
            .entry(parent.to_path_buf())
            .or_default()
            .push(path.clone());
    }

    let mut resolved = BTreeMap::<PathBuf, Vec<PathBuf>>::new();
    for (folder, requested_paths) in by_folder {
        recover_pending_sidecar_transactions(&folder)?;
        let (decisions, skipped, _) = scan_direct_asset_decisions(&folder)?;
        for requested in requested_paths {
            let asset = decisions.iter().find_map(|decision| match decision {
                AssetDecision::Eligible {
                    primary_path,
                    member_paths,
                    ..
                } if member_paths.iter().any(|member| member == &requested) => {
                    Some((primary_path, member_paths))
                }
                _ => None,
            });
            if let Some((primary_path, member_paths)) = asset {
                resolved
                    .entry(primary_path.clone())
                    .or_insert_with(|| member_paths.clone());
                continue;
            }
            if skipped
                .iter()
                .any(|skipped| skipped.paths.iter().any(|path| path == &requested))
                || decisions.iter().any(|decision| {
                    matches!(
                        decision,
                        AssetDecision::Skipped { paths, .. }
                            if paths.iter().any(|path| path == &requested)
                    )
                })
            {
                return Err(format!(
                    "Cannot resolve an ambiguous or unsupported asset: {}",
                    requested.display()
                ));
            }
            return Err(format!(
                "Image is not a direct supported member of its folder: {}",
                requested.display()
            ));
        }
    }
    Ok(resolved.into_values().collect())
}

fn is_explicitly_excluded(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("gif" | "tif" | "tiff")
    )
}

fn is_jpeg(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("jpg" | "jpeg")
    )
}

fn inspect_asset(
    primary_path: PathBuf,
    display_path: PathBuf,
    member_paths: Vec<PathBuf>,
) -> Result<CatalogAsset, CatalogFailure> {
    let mut snapshots = Vec::with_capacity(member_paths.len());
    let mut file_baselines = Vec::with_capacity(member_paths.len());
    let mut member_sidecar_baselines = Vec::with_capacity(member_paths.len());
    for member_path in &member_paths {
        let baseline = capture_file_baseline(member_path).map_err(|reason| CatalogFailure {
            path: member_path.clone(),
            reason,
        })?;
        file_baselines.push((member_path.clone(), baseline));
        let sidecar_path = get_primary_sidecar_path(member_path);
        let metadata = read_sidecar_strict(&sidecar_path).map_err(|reason| CatalogFailure {
            path: sidecar_path.clone(),
            reason,
        })?;
        let sidecar_baseline =
            capture_sidecar_baseline(&sidecar_path).map_err(|reason| CatalogFailure {
                path: sidecar_path.clone(),
                reason,
            })?;
        member_sidecar_baselines.push((sidecar_path, sidecar_baseline));
        snapshots.push(MetadataSnapshot {
            rating: metadata.rating,
            tags: metadata.tags.unwrap_or_default(),
            feature_data: metadata.feature_data,
        });
    }

    let sidecar_path = get_primary_sidecar_path(&primary_path);
    if snapshots.iter().any(metadata_has_unknown_source) {
        return Err(CatalogFailure {
            path: display_path,
            reason: "RAW/JPEG metadata contains an unknown or malformed smart-culling source"
                .to_string(),
        });
    }
    if asset_has_conflicting_results(&snapshots) {
        return Err(CatalogFailure {
            path: display_path,
            reason: "RAW/JPEG members contain conflicting rating or color results".to_string(),
        });
    }
    let capture_time_from_exif = try_get_exif_creation_date(&display_path).is_some();
    let capture_time_millis = get_creation_date_from_path(&display_path).timestamp_millis();
    let sequence_number = trailing_sequence_number(&display_path);
    let status = if asset_is_protected(&snapshots) {
        CatalogAssetStatus::Protected
    } else {
        CatalogAssetStatus::Eligible
    };

    Ok(CatalogAsset {
        primary_path,
        display_path,
        member_paths,
        capture_time_millis,
        capture_time_from_exif,
        sequence_number,
        file_baselines,
        member_sidecar_baselines,
        sidecar_path,
        status,
    })
}

fn trailing_sequence_number(path: &Path) -> Option<u64> {
    let stem = path.file_stem()?.to_str()?;
    let digits = stem
        .chars()
        .rev()
        .take_while(|character| character.is_ascii_digit())
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    (!digits.is_empty()).then(|| digits.parse().ok()).flatten()
}

pub(crate) fn read_sidecar_strict(path: &Path) -> Result<ImageMetadata, String> {
    match fs::read(path) {
        Ok(bytes) => {
            serde_json::from_slice(&bytes).map_err(|error| format!("Invalid sidecar JSON: {error}"))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(ImageMetadata::default()),
        Err(error) => Err(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::fs::File;

    use tempfile::tempdir;

    use super::*;

    fn touch(path: &Path) {
        File::create(path).unwrap();
    }

    fn with_capture_identity(mut metadata: ImageMetadata) -> ImageMetadata {
        metadata.exif = Some(HashMap::from([(
            "DateTimeOriginal".to_string(),
            "2026-08-04 12:00:00".to_string(),
        )]));
        metadata
    }

    fn write_capture_metadata(path: &Path, metadata: ImageMetadata) {
        let file = File::create(get_primary_sidecar_path(path)).unwrap();
        serde_json::to_writer_pretty(file, &with_capture_identity(metadata)).unwrap();
    }

    #[test]
    fn scans_only_direct_images_and_ignores_subfolders() {
        let root = tempdir().unwrap();
        let nested = root.path().join("nested");
        fs::create_dir(&nested).unwrap();
        touch(&root.path().join("cover.jpg"));
        touch(&root.path().join("animation.gif"));
        touch(&root.path().join("scan.tiff"));
        touch(&nested.join("frame.png"));
        touch(&nested.join("nested.gif"));
        touch(&nested.join("notes.txt"));

        let catalog = scan_catalog(root.path()).unwrap();

        assert_eq!(catalog.assets.len(), 1);
        assert_eq!(catalog.skipped.len(), 2);
        assert!(catalog.failures.is_empty());
        assert_eq!(
            catalog.assets[0].display_path,
            root.path().join("cover.jpg")
        );
        assert!(
            catalog
                .skipped
                .iter()
                .all(|item| item.reason == CatalogSkipReason::ExcludedFormat)
        );
    }

    #[test]
    fn does_not_pair_with_or_process_a_nested_file() {
        let root = tempdir().unwrap();
        let nested = root.path().join("nested");
        fs::create_dir(&nested).unwrap();
        touch(&root.path().join("IMG_0001.dng"));
        touch(&nested.join("IMG_0001.jpg"));

        let catalog = scan_catalog(root.path()).unwrap();

        assert_eq!(catalog.assets.len(), 1);
        assert_eq!(catalog.assets[0].member_paths.len(), 1);
        assert_eq!(
            catalog.assets[0].display_path,
            root.path().join("IMG_0001.dng")
        );
        assert!(catalog.skipped.is_empty());
    }

    #[test]
    fn protects_a_raw_jpeg_asset_when_the_jpeg_has_manual_metadata() {
        let root = tempdir().unwrap();
        let raw = root.path().join("IMG_0001.dng");
        let jpeg = root.path().join("IMG_0001.jpg");
        touch(&raw);
        touch(&jpeg);
        write_capture_metadata(&raw, ImageMetadata::default());
        let metadata = ImageMetadata {
            rating: 5,
            ..ImageMetadata::default()
        };
        write_capture_metadata(&jpeg, metadata);

        let catalog = scan_catalog(root.path()).unwrap();

        assert_eq!(catalog.assets.len(), 1);
        assert_eq!(catalog.assets[0].primary_path, raw);
        assert_eq!(catalog.assets[0].display_path, jpeg);
        assert_eq!(catalog.assets[0].status, CatalogAssetStatus::Protected);
    }

    #[test]
    fn reports_an_invalid_sidecar_instead_of_treating_it_as_unprotected() {
        let root = tempdir().unwrap();
        let image = root.path().join("photo.jpg");
        touch(&image);
        fs::write(get_primary_sidecar_path(&image), b"not-json").unwrap();

        let catalog = scan_catalog(root.path()).unwrap();

        assert!(catalog.assets.is_empty());
        assert_eq!(catalog.failures.len(), 1);
        assert!(
            catalog.failures[0]
                .reason
                .starts_with("Invalid sidecar JSON")
        );
    }

    #[test]
    fn reports_conflicting_raw_jpeg_results_instead_of_guessing() {
        let root = tempdir().unwrap();
        let raw = root.path().join("IMG_0002.dng");
        let jpeg = root.path().join("IMG_0002.jpg");
        touch(&raw);
        touch(&jpeg);
        for (path, rating) in [(&raw, 4), (&jpeg, 5)] {
            let metadata = ImageMetadata {
                rating,
                feature_data: Some(serde_json::json!({
                    "smartCullingV2": {
                        "source": "ai",
                        "rating": rating,
                        "colorLabel": null,
                        "locked": false
                    }
                })),
                ..ImageMetadata::default()
            };
            write_capture_metadata(path, metadata);
        }

        let catalog = scan_catalog(root.path()).unwrap();

        assert!(catalog.assets.is_empty());
        assert_eq!(catalog.failures.len(), 1);
        assert!(catalog.failures[0].reason.contains("conflicting"));
    }

    #[test]
    fn reports_an_unknown_smart_culling_source_without_rewriting_it() {
        let root = tempdir().unwrap();
        let image = root.path().join("photo.jpg");
        touch(&image);
        let sidecar = get_primary_sidecar_path(&image);
        let metadata = ImageMetadata {
            feature_data: Some(serde_json::json!({
                "smartCullingV2": {
                    "source": "unknown",
                    "rating": 0
                }
            })),
            ..ImageMetadata::default()
        };
        let file = File::create(&sidecar).unwrap();
        serde_json::to_writer_pretty(file, &metadata).unwrap();

        let catalog = scan_catalog(root.path()).unwrap();

        assert!(catalog.assets.is_empty());
        assert_eq!(catalog.failures.len(), 1);
        assert!(catalog.failures[0].reason.contains("unknown"));
        let persisted: ImageMetadata = serde_json::from_slice(&fs::read(sidecar).unwrap()).unwrap();
        assert_eq!(
            persisted.feature_data.unwrap()["smartCullingV2"]["source"],
            "unknown"
        );
    }

    #[test]
    fn extracts_a_trailing_camera_sequence_number() {
        assert_eq!(
            trailing_sequence_number(Path::new("/photos/DSC_0042.ARW")),
            Some(42)
        );
        assert_eq!(
            trailing_sequence_number(Path::new("/photos/portrait-final.jpg")),
            None
        );
    }

    #[test]
    fn resolves_a_selected_jpeg_to_the_whole_raw_jpeg_asset() {
        let root = tempdir().unwrap();
        let raw = root.path().join("IMG_0008.dng");
        let jpeg = root.path().join("IMG_0008.jpg");
        touch(&raw);
        touch(&jpeg);
        write_capture_metadata(&raw, ImageMetadata::default());
        write_capture_metadata(&jpeg, ImageMetadata::default());

        let resolved = resolve_asset_member_groups(std::slice::from_ref(&jpeg)).unwrap();

        assert_eq!(resolved, vec![vec![raw, jpeg]]);
    }

    #[test]
    fn skips_a_same_stem_pair_when_capture_identity_is_unavailable() {
        let root = tempdir().unwrap();
        touch(&root.path().join("IMG_0009.dng"));
        touch(&root.path().join("IMG_0009.jpg"));

        let catalog = scan_catalog(root.path()).unwrap();

        assert!(catalog.assets.is_empty());
        assert_eq!(catalog.skipped.len(), 1);
        assert_eq!(catalog.skipped[0].reason, CatalogSkipReason::AmbiguousPair);
    }
}
