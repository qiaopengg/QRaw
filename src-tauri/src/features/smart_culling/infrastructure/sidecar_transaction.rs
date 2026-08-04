use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

use crate::image_processing::ImageMetadata;

const JOURNAL_PREFIX: &str = ".qraw-smart-culling-transaction-";
const JOURNAL_SUFFIX: &str = ".json";
const JOURNAL_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SidecarTransactionJournal {
    schema_version: u32,
    entries: Vec<SidecarTransactionEntry>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SidecarTransactionEntry {
    path: PathBuf,
    original: Option<Vec<u8>>,
    replacement: Vec<u8>,
}

pub(crate) fn write_sidecar_transaction_guarded<G>(
    updates: &[(PathBuf, ImageMetadata)],
    guard: G,
) -> Result<(), String>
where
    G: FnMut() -> Result<(), String>,
{
    write_sidecar_transaction_with(updates, guard, |_, path, bytes| {
        atomic_write_bytes(path, bytes)
    })
}

pub(crate) fn recover_pending_sidecar_transactions(root: &Path) -> Result<(), String> {
    let entries = fs::read_dir(root)
        .map_err(|error| format!("Cannot inspect sidecar recovery journals: {error}"))?;
    let mut failures = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                failures.push(error.to_string());
                continue;
            }
        };
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !name.starts_with(JOURNAL_PREFIX) || !name.ends_with(JOURNAL_SUFFIX) {
            continue;
        }
        let result = fs::read(&path)
            .map_err(|error| error.to_string())
            .and_then(|bytes| {
                serde_json::from_slice::<SidecarTransactionJournal>(&bytes)
                    .map_err(|error| format!("Invalid sidecar transaction journal: {error}"))
            })
            .and_then(|journal| {
                validate_journal(root, &journal)?;
                rollback_journal(&path, &journal).map(|_| ())
            });
        if let Err(error) = result {
            failures.push(format!("{}: {error}", path.display()));
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("; "))
    }
}

fn write_sidecar_transaction_with<G, F>(
    updates: &[(PathBuf, ImageMetadata)],
    mut guard: G,
    mut writer: F,
) -> Result<(), String>
where
    G: FnMut() -> Result<(), String>,
    F: FnMut(usize, &Path, &[u8]) -> Result<(), String>,
{
    if updates.is_empty() {
        return Ok(());
    }
    let (journal_path, journal) = create_journal(updates)?;
    if let Err(error) = guard() {
        let preserved = rollback_journal(&journal_path, &journal)?;
        return Err(transaction_error(error, &preserved));
    }
    for (index, entry) in journal.entries.iter().enumerate() {
        let current = read_optional(&entry.path)?;
        if current != entry.original {
            let preserved = rollback_journal(&journal_path, &journal)?;
            return Err(transaction_error(
                format!(
                    "sidecar changed during the asset transaction: {}",
                    entry.path.display()
                ),
                &preserved,
            ));
        }
        if let Err(error) = writer(index, &entry.path, &entry.replacement) {
            let preserved = rollback_journal(&journal_path, &journal)?;
            return Err(transaction_error(error, &preserved));
        }
    }
    if let Err(error) = remove_journal(&journal_path) {
        let preserved = rollback_journal(&journal_path, &journal)?;
        return Err(transaction_error(error, &preserved));
    }
    Ok(())
}

fn create_journal(
    updates: &[(PathBuf, ImageMetadata)],
) -> Result<(PathBuf, SidecarTransactionJournal), String> {
    let parent = updates[0].0.parent().ok_or_else(|| {
        format!(
            "Sidecar has no parent directory: {}",
            updates[0].0.display()
        )
    })?;
    let mut entries = Vec::with_capacity(updates.len());
    for (path, metadata) in updates {
        if path.parent() != Some(parent) {
            return Err("All sidecars in an asset transaction must share one folder".to_string());
        }
        entries.push(SidecarTransactionEntry {
            path: path.clone(),
            original: read_optional(path)?,
            replacement: serde_json::to_vec_pretty(metadata).map_err(|error| error.to_string())?,
        });
    }
    let journal = SidecarTransactionJournal {
        schema_version: JOURNAL_SCHEMA_VERSION,
        entries,
    };
    let journal_path = parent.join(format!(
        "{JOURNAL_PREFIX}{}{JOURNAL_SUFFIX}",
        uuid::Uuid::new_v4()
    ));
    let bytes = serde_json::to_vec(&journal).map_err(|error| error.to_string())?;
    atomic_write_bytes(&journal_path, &bytes)?;
    Ok((journal_path, journal))
}

fn rollback_journal(
    journal_path: &Path,
    journal: &SidecarTransactionJournal,
) -> Result<Vec<PathBuf>, String> {
    let mut preserved = Vec::new();
    let mut failures = Vec::new();
    for entry in &journal.entries {
        match read_optional(&entry.path) {
            Ok(current) if current.as_deref() == Some(entry.replacement.as_slice()) => {
                if let Err(error) = restore_original(&entry.path, entry.original.as_deref()) {
                    failures.push(format!("{}: {error}", entry.path.display()));
                }
            }
            Ok(current) if current == entry.original => {}
            Ok(_) => preserved.push(entry.path.clone()),
            Err(error) => failures.push(format!("{}: {error}", entry.path.display())),
        }
    }
    if failures.is_empty() {
        remove_journal(journal_path)?;
        Ok(preserved)
    } else {
        Err(format!(
            "could not fully restore the sidecar transaction; recovery journal retained: {}",
            failures.join("; ")
        ))
    }
}

fn validate_journal(root: &Path, journal: &SidecarTransactionJournal) -> Result<(), String> {
    if journal.schema_version != JOURNAL_SCHEMA_VERSION || journal.entries.is_empty() {
        return Err("Unsupported or empty sidecar transaction journal".to_string());
    }
    if journal
        .entries
        .iter()
        .any(|entry| entry.path.parent() != Some(root))
    {
        return Err("Sidecar transaction journal references a different folder".to_string());
    }
    Ok(())
}

fn transaction_error(error: String, preserved: &[PathBuf]) -> String {
    if preserved.is_empty() {
        format!("{error}; previously written asset members were restored")
    } else {
        format!(
            "{error}; transaction writes were restored and concurrent user changes were preserved at {}",
            preserved
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

fn restore_original(path: &Path, original: Option<&[u8]>) -> Result<(), String> {
    match original {
        Some(bytes) => atomic_write_bytes(path, bytes),
        None => match fs::remove_file(path) {
            Ok(()) => sync_parent(path),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.to_string()),
        },
    }
}

fn read_optional(path: &Path) -> Result<Option<Vec<u8>>, String> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

fn atomic_write_bytes(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("Sidecar has no parent directory: {}", path.display()))?;
    let mut temporary = NamedTempFile::new_in(parent).map_err(|error| error.to_string())?;
    temporary
        .write_all(bytes)
        .map_err(|error| error.to_string())?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| error.to_string())?;
    temporary.persist(path).map_err(|error| error.to_string())?;
    sync_parent(path)
}

fn remove_journal(path: &Path) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => sync_parent(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

fn sync_parent(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        let parent = path
            .parent()
            .ok_or_else(|| format!("Path has no parent directory: {}", path.display()))?;
        fs::File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs::File;

    use tempfile::tempdir;

    use super::*;

    fn write_metadata(path: &Path, rating: u8) {
        let file = File::create(path).unwrap();
        serde_json::to_writer_pretty(
            file,
            &ImageMetadata {
                rating,
                ..ImageMetadata::default()
            },
        )
        .unwrap();
    }

    fn updates(first: &Path, second: &Path) -> Vec<(PathBuf, ImageMetadata)> {
        vec![
            (
                first.to_path_buf(),
                ImageMetadata {
                    rating: 4,
                    ..ImageMetadata::default()
                },
            ),
            (
                second.to_path_buf(),
                ImageMetadata {
                    rating: 4,
                    ..ImageMetadata::default()
                },
            ),
        ]
    }

    #[test]
    fn second_member_failure_restores_the_first_member() {
        let directory = tempdir().unwrap();
        let first = directory.path().join("photo.dng.rrdata");
        let second = directory.path().join("photo.jpg.rrdata");
        write_metadata(&first, 1);
        write_metadata(&second, 1);
        let original_first = fs::read(&first).unwrap();
        let original_second = fs::read(&second).unwrap();

        let result = write_sidecar_transaction_with(
            &updates(&first, &second),
            || Ok(()),
            |index, path, bytes| {
                if index == 1 {
                    Err("injected second-member failure".to_string())
                } else {
                    atomic_write_bytes(path, bytes)
                }
            },
        );

        assert!(result.is_err());
        assert_eq!(fs::read(first).unwrap(), original_first);
        assert_eq!(fs::read(second).unwrap(), original_second);
    }

    #[test]
    fn interrupted_transaction_is_restored_during_folder_recovery() {
        let directory = tempdir().unwrap();
        let first = directory.path().join("photo.dng.rrdata");
        let second = directory.path().join("photo.jpg.rrdata");
        write_metadata(&first, 1);
        write_metadata(&second, 1);
        let original_first = fs::read(&first).unwrap();
        let original_second = fs::read(&second).unwrap();
        let (journal_path, journal) = create_journal(&updates(&first, &second)).unwrap();
        atomic_write_bytes(&first, &journal.entries[0].replacement).unwrap();
        assert!(journal_path.exists());

        recover_pending_sidecar_transactions(directory.path()).unwrap();

        assert_eq!(fs::read(first).unwrap(), original_first);
        assert_eq!(fs::read(second).unwrap(), original_second);
        assert!(!journal_path.exists());
    }

    #[test]
    fn rollback_preserves_a_concurrent_user_change() {
        let directory = tempdir().unwrap();
        let first = directory.path().join("photo.dng.rrdata");
        let second = directory.path().join("photo.jpg.rrdata");
        write_metadata(&first, 1);
        write_metadata(&second, 1);
        let original_first = fs::read(&first).unwrap();
        let external = serde_json::to_vec_pretty(&ImageMetadata {
            rating: 5,
            ..ImageMetadata::default()
        })
        .unwrap();

        let result = write_sidecar_transaction_with(
            &updates(&first, &second),
            || Ok(()),
            |index, path, bytes| {
                if index == 1 {
                    atomic_write_bytes(path, &external)?;
                    Err("injected concurrent change".to_string())
                } else {
                    atomic_write_bytes(path, bytes)
                }
            },
        );

        assert!(result.is_err());
        assert_eq!(fs::read(first).unwrap(), original_first);
        assert_eq!(fs::read(second).unwrap(), external);
    }

    #[test]
    fn guard_window_detects_and_preserves_a_manual_change() {
        let directory = tempdir().unwrap();
        let first = directory.path().join("photo.dng.rrdata");
        let second = directory.path().join("photo.jpg.rrdata");
        write_metadata(&first, 1);
        write_metadata(&second, 1);
        let original_second = fs::read(&second).unwrap();
        let external = serde_json::to_vec_pretty(&ImageMetadata {
            rating: 5,
            ..ImageMetadata::default()
        })
        .unwrap();

        let result = write_sidecar_transaction_with(
            &updates(&first, &second),
            || atomic_write_bytes(&first, &external),
            |_, path, bytes| atomic_write_bytes(path, bytes),
        );

        assert!(result.is_err());
        assert_eq!(fs::read(first).unwrap(), external);
        assert_eq!(fs::read(second).unwrap(), original_second);
    }
}
