use std::sync::mpsc;
use std::time::Duration;

use tempfile::tempdir;

use super::*;

fn bytes(rating: u8) -> Vec<u8> {
    serde_json::to_vec_pretty(&ImageMetadata {
        rating,
        ..ImageMetadata::default()
    })
    .unwrap()
}

fn entry(
    path: PathBuf,
    original: Option<Vec<u8>>,
    replacement: Vec<u8>,
) -> SidecarTransactionEntry {
    SidecarTransactionEntry {
        path,
        original,
        replacement,
    }
}

fn journal(root: &Path, entries: Vec<SidecarTransactionEntry>) -> PathBuf {
    let path = root.join(format!("{JOURNAL_PREFIX}test{JOURNAL_SUFFIX}"));
    fs::write(
        &path,
        serde_json::to_vec(&SidecarTransactionJournal {
            schema_version: JOURNAL_SCHEMA_VERSION,
            entries,
        })
        .unwrap(),
    )
    .unwrap();
    path
}

#[test]
fn rejects_photo_targets_before_restoring_any_other_member() {
    let directory = tempdir().unwrap();
    let sidecar = directory.path().join("photo.jpg.rrdata");
    let photo = directory.path().join("photo.jpg");
    let photo_bytes = b"original image bytes".to_vec();
    fs::write(&sidecar, bytes(4)).unwrap();
    fs::write(&photo, &photo_bytes).unwrap();
    let log = journal(
        directory.path(),
        vec![
            entry(sidecar.clone(), Some(bytes(1)), bytes(4)),
            entry(photo.clone(), None, photo_bytes.clone()),
        ],
    );

    assert!(recover_pending_sidecar_transactions(directory.path()).is_err());
    assert_eq!(fs::read(photo).unwrap(), photo_bytes);
    assert_eq!(fs::read(sidecar).unwrap(), bytes(4));
    assert!(log.exists());
}

#[test]
fn rejects_duplicate_targets_and_preserves_the_journal() {
    let directory = tempdir().unwrap();
    let sidecar = directory.path().join("photo.jpg.rrdata");
    fs::write(&sidecar, bytes(4)).unwrap();
    let log = journal(
        directory.path(),
        vec![
            entry(sidecar.clone(), Some(bytes(1)), bytes(4)),
            entry(sidecar.clone(), None, bytes(1)),
        ],
    );
    assert!(recover_pending_sidecar_transactions(directory.path()).is_err());
    assert_eq!(fs::read(sidecar).unwrap(), bytes(4));
    assert!(log.exists());
}

#[test]
fn rejects_metadata_shaped_photo_payloads_and_directory_or_outside_targets() {
    for kind in ["photo", "directory", "outside", "invalid_payload"] {
        let directory = tempdir().unwrap();
        let outside = tempdir().unwrap();
        let good = directory.path().join("good.jpg.rrdata");
        fs::write(&good, bytes(4)).unwrap();
        let bad = match kind {
            "photo" => directory.path().join("photo.jpg"),
            "outside" => outside.path().join("photo.jpg.rrdata"),
            _ => directory.path().join("bad.jpg.rrdata"),
        };
        if kind == "directory" {
            fs::create_dir(&bad).unwrap();
        } else {
            fs::write(&bad, bytes(4)).unwrap();
        }
        let original = if kind == "invalid_payload" {
            b"not metadata".to_vec()
        } else {
            bytes(1)
        };
        let log = journal(
            directory.path(),
            vec![
                entry(good.clone(), Some(bytes(1)), bytes(4)),
                entry(bad.clone(), Some(original), bytes(4)),
            ],
        );
        assert!(
            recover_pending_sidecar_transactions(directory.path()).is_err(),
            "{kind}"
        );
        assert_eq!(fs::read(good).unwrap(), bytes(4));
        if kind == "directory" {
            assert!(bad.is_dir());
        } else {
            assert_eq!(fs::read(bad).unwrap(), bytes(4));
        }
        assert!(log.exists());
    }
}

#[cfg(unix)]
#[test]
fn rejects_symbolic_links_for_journal_files_and_targets() {
    use std::os::unix::fs::symlink;
    let directory = tempdir().unwrap();
    let target = directory.path().join("real.jpg.rrdata");
    let link = directory.path().join("linked.jpg.rrdata");
    fs::write(&target, bytes(4)).unwrap();
    symlink(&target, &link).unwrap();
    let log = journal(directory.path(), vec![entry(link.clone(), None, bytes(4))]);
    assert!(recover_pending_sidecar_transactions(directory.path()).is_err());
    assert_eq!(fs::read(&target).unwrap(), bytes(4));
    assert!(link.is_symlink());
    assert!(log.exists());
    fs::remove_file(&log).unwrap();
    let backing_log = directory.path().join("untrusted.json");
    fs::write(
        &backing_log,
        serde_json::to_vec(&SidecarTransactionJournal {
            schema_version: JOURNAL_SCHEMA_VERSION,
            entries: vec![entry(target.clone(), None, bytes(4))],
        })
        .unwrap(),
    )
    .unwrap();
    symlink(backing_log, &log).unwrap();
    assert!(recover_pending_sidecar_transactions(directory.path()).is_err());
    assert_eq!(fs::read(target).unwrap(), bytes(4));
    assert!(log.is_symlink());
}

#[test]
fn recovery_waits_for_the_complete_active_asset_transaction() {
    let directory = tempdir().unwrap();
    let first = directory.path().join("photo.dng.rrdata");
    let second = directory.path().join("photo.jpg.rrdata");
    fs::write(&first, bytes(1)).unwrap();
    fs::write(&second, bytes(1)).unwrap();
    let updates = [&first, &second].map(|path| {
        (
            path.clone(),
            ImageMetadata {
                rating: 4,
                ..ImageMetadata::default()
            },
        )
    });
    let (first_written_tx, first_written_rx) = mpsc::channel();
    let (continue_tx, continue_rx) = mpsc::channel();
    let writer = std::thread::spawn(move || {
        write_sidecar_transaction_with(
            &updates,
            || Ok(()),
            |index, path, replacement| {
                atomic_write_bytes(path, replacement)?;
                if index == 0 {
                    first_written_tx.send(()).unwrap();
                    continue_rx.recv().unwrap();
                }
                Ok(())
            },
        )
    });
    first_written_rx
        .recv_timeout(Duration::from_secs(10))
        .unwrap();
    let root = directory.path().to_path_buf();
    let (started_tx, started_rx) = mpsc::channel();
    let (finished_tx, finished_rx) = mpsc::channel();
    let recovery = std::thread::spawn(move || {
        started_tx.send(()).unwrap();
        let result = recover_pending_sidecar_transactions(&root);
        finished_tx.send(()).unwrap();
        result
    });
    started_rx.recv().unwrap();
    let recovered_during_write = finished_rx.recv_timeout(Duration::from_millis(250)).is_ok();
    continue_tx.send(()).unwrap();
    assert!(writer.join().unwrap().is_ok());
    assert!(recovery.join().unwrap().is_ok());
    assert!(
        !recovered_during_write,
        "recovery consumed an active transaction journal"
    );
    assert_eq!(fs::read(first).unwrap(), bytes(4));
    assert_eq!(fs::read(second).unwrap(), bytes(4));
}

#[test]
fn a_later_member_read_error_still_restores_already_written_members() {
    let directory = tempdir().unwrap();
    let first = directory.path().join("photo.dng.rrdata");
    let second = directory.path().join("photo.jpg.rrdata");
    fs::write(&first, bytes(1)).unwrap();
    fs::write(&second, bytes(1)).unwrap();
    let updates = [&first, &second].map(|path| {
        (
            path.clone(),
            ImageMetadata {
                rating: 4,
                ..ImageMetadata::default()
            },
        )
    });
    let result = write_sidecar_transaction_with(
        &updates,
        || Ok(()),
        |index, path, replacement| {
            atomic_write_bytes(path, replacement)?;
            if index == 0 {
                fs::remove_file(&second).unwrap();
                fs::create_dir(&second).unwrap();
            }
            Ok(())
        },
    );
    assert!(result.is_err());
    assert_eq!(fs::read(first).unwrap(), bytes(1));
    assert!(second.is_dir());
    assert!(fs::read_dir(directory.path()).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(JOURNAL_PREFIX)
    }));
}
