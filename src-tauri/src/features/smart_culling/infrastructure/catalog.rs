use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::formats::{is_raw_file, is_supported_image_file};
use crate::image_processing::ImageMetadata;

use super::super::domain::{
    AssetCandidate, AssetDecision, AssetMemberKind, MetadataSnapshot, SkipReason,
    asset_is_protected, group_assets,
};
use super::baseline::{
    FileBaseline, SidecarBaseline, capture_file_baseline, capture_sidecar_baseline,
};
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
    pub member_paths: Vec<PathBuf>,
    pub file_baselines: Vec<(PathBuf, FileBaseline)>,
    pub sidecar_path: PathBuf,
    pub sidecar_baseline: SidecarBaseline,
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

    let mut supported_by_folder = BTreeMap::<PathBuf, Vec<AssetCandidate>>::new();
    let mut skipped = Vec::new();
    let mut failures = Vec::new();
    let mut pending = vec![root.to_path_buf()];

    while let Some(directory) = pending.pop() {
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) => {
                failures.push(CatalogFailure {
                    path: directory,
                    reason: error.to_string(),
                });
                continue;
            }
        };

        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    failures.push(CatalogFailure {
                        path: directory.clone(),
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

            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                pending.push(path);
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
            supported_by_folder
                .entry(directory.clone())
                .or_default()
                .push(AssetCandidate { path, kind });
        }
    }

    let mut assets = Vec::new();
    for candidates in supported_by_folder.into_values() {
        for decision in group_assets(candidates) {
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
                    member_paths,
                } => match inspect_asset(primary_path, member_paths) {
                    Ok(asset) => assets.push(asset),
                    Err(failure) => failures.push(failure),
                },
            }
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
    member_paths: Vec<PathBuf>,
) -> Result<CatalogAsset, CatalogFailure> {
    let mut snapshots = Vec::with_capacity(member_paths.len());
    let mut file_baselines = Vec::with_capacity(member_paths.len());
    for member_path in &member_paths {
        let baseline = capture_file_baseline(member_path).map_err(|reason| CatalogFailure {
            path: member_path.clone(),
            reason,
        })?;
        file_baselines.push((member_path.clone(), baseline));
        let sidecar_path = get_primary_sidecar_path(member_path);
        let metadata = read_sidecar_strict(&sidecar_path).map_err(|reason| CatalogFailure {
            path: sidecar_path,
            reason,
        })?;
        snapshots.push(MetadataSnapshot {
            rating: metadata.rating,
            tags: metadata.tags.unwrap_or_default(),
            feature_data: metadata.feature_data,
        });
    }

    let sidecar_path = get_primary_sidecar_path(&primary_path);
    let sidecar_baseline =
        capture_sidecar_baseline(&sidecar_path).map_err(|reason| CatalogFailure {
            path: sidecar_path.clone(),
            reason,
        })?;
    let status = if asset_is_protected(&snapshots) {
        CatalogAssetStatus::Protected
    } else {
        CatalogAssetStatus::Eligible
    };

    Ok(CatalogAsset {
        primary_path,
        member_paths,
        file_baselines,
        sidecar_path,
        sidecar_baseline,
        status,
    })
}

fn read_sidecar_strict(path: &Path) -> Result<ImageMetadata, String> {
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
    use std::fs::File;

    use tempfile::tempdir;

    use super::*;

    fn touch(path: &Path) {
        File::create(path).unwrap();
    }

    #[test]
    fn recursively_scans_supported_images_and_excludes_gif_and_tiff() {
        let root = tempdir().unwrap();
        let nested = root.path().join("nested");
        fs::create_dir(&nested).unwrap();
        touch(&root.path().join("cover.jpg"));
        touch(&nested.join("frame.png"));
        touch(&nested.join("animation.gif"));
        touch(&nested.join("scan.tiff"));
        touch(&nested.join("notes.txt"));

        let catalog = scan_catalog(root.path()).unwrap();

        assert_eq!(catalog.assets.len(), 2);
        assert_eq!(catalog.skipped.len(), 2);
        assert!(catalog.failures.is_empty());
        assert!(
            catalog
                .skipped
                .iter()
                .all(|item| item.reason == CatalogSkipReason::ExcludedFormat)
        );
    }

    #[test]
    fn only_pairs_files_inside_the_same_folder() {
        let root = tempdir().unwrap();
        let nested = root.path().join("nested");
        fs::create_dir(&nested).unwrap();
        touch(&root.path().join("IMG_0001.dng"));
        touch(&nested.join("IMG_0001.jpg"));

        let catalog = scan_catalog(root.path()).unwrap();

        assert_eq!(catalog.assets.len(), 2);
        assert!(catalog.skipped.is_empty());
    }

    #[test]
    fn protects_a_raw_jpeg_asset_when_the_jpeg_has_manual_metadata() {
        let root = tempdir().unwrap();
        let raw = root.path().join("IMG_0001.dng");
        let jpeg = root.path().join("IMG_0001.jpg");
        touch(&raw);
        touch(&jpeg);
        let jpeg_sidecar = get_primary_sidecar_path(&jpeg);
        let metadata = ImageMetadata {
            rating: 5,
            ..ImageMetadata::default()
        };
        let file = File::create(jpeg_sidecar).unwrap();
        serde_json::to_writer_pretty(file, &metadata).unwrap();

        let catalog = scan_catalog(root.path()).unwrap();

        assert_eq!(catalog.assets.len(), 1);
        assert_eq!(catalog.assets[0].primary_path, raw);
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
}
