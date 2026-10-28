//! Recovery journals are untrusted directory contents, never arbitrary write instructions.

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use crate::exif_processing::get_primary_sidecar_path;
use crate::formats::is_supported_image_file;
use crate::image_processing::ImageMetadata;

use super::{JOURNAL_SCHEMA_VERSION, SidecarTransactionJournal};

pub(super) fn validate_targets<'a>(
    root: &Path,
    targets: impl IntoIterator<Item = &'a Path>,
) -> Result<(), String> {
    let mut seen = HashSet::new();
    for path in targets {
        if path.parent() != Some(root) {
            return Err("Sidecar transaction references a different folder".to_string());
        }
        let image_path = path.with_extension("");
        if path
            .extension()
            .is_none_or(|extension| extension != "rrdata")
            || !is_supported_image_file(&image_path)
            || get_primary_sidecar_path(&image_path) != path
        {
            return Err(format!(
                "Transaction target is not an image sidecar: {}",
                path.display()
            ));
        }
        if !seen.insert(path) {
            return Err(format!("Duplicate transaction target: {}", path.display()));
        }
        match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.file_type().is_file() => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Ok(_) => {
                return Err(format!(
                    "Transaction target is not a regular file: {}",
                    path.display()
                ));
            }
            Err(error) => {
                return Err(format!(
                    "Cannot inspect transaction target {}: {error}",
                    path.display()
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn validate_journal(
    root: &Path,
    journal: &SidecarTransactionJournal,
) -> Result<(), String> {
    if journal.schema_version != JOURNAL_SCHEMA_VERSION || journal.entries.is_empty() {
        return Err("Unsupported or empty sidecar transaction journal".to_string());
    }
    validate_targets(
        root,
        journal.entries.iter().map(|entry| entry.path.as_path()),
    )?;
    // Validate every member before rollback can modify even the first sidecar.
    for entry in &journal.entries {
        for bytes in entry
            .original
            .iter()
            .chain(std::iter::once(&entry.replacement))
        {
            serde_json::from_slice::<ImageMetadata>(bytes)
                .map_err(|error| format!("Invalid metadata in recovery journal: {error}"))?;
        }
    }
    Ok(())
}
