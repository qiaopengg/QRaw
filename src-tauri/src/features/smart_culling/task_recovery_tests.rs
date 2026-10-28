use std::sync::{Arc, mpsc};

use tempfile::tempdir;

use super::*;

fn snapshot(state: &str) -> SmartCullingSnapshot {
    SmartCullingSnapshot {
        task_id: Some("task".to_string()),
        root_path: Some("/synthetic".to_string()),
        state: state.to_string(),
        ..SmartCullingSnapshot::default()
    }
}

fn read_marker(path: &Path) -> InterruptedTask {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

#[test]
fn late_remember_cannot_replace_a_newer_marker() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("interrupted.json");
    let store = Arc::new(MarkerStore::default());
    let old = snapshot("cancelling");
    let new = snapshot("readyForReview");
    let (release_tx, release_rx) = mpsc::channel();
    let worker_store = Arc::clone(&store);
    let worker_path = path.clone();
    let worker = std::thread::spawn(move || {
        release_rx.recv().unwrap();
        worker_store.remember(&worker_path, &old).unwrap();
    });
    store.remember(&path, &new).unwrap();
    release_tx.send(()).unwrap();
    worker.join().unwrap();

    assert_eq!(read_marker(&path).state, "readyForReview");
}

#[test]
fn late_remember_cannot_resurrect_a_forgotten_task() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("interrupted.json");
    let store = MarkerStore::default();
    let old = snapshot("readyForReview");
    let completed = snapshot("completed");
    store.remember(&path, &old).unwrap();
    store.forget(&path, &completed).unwrap();
    store.remember(&path, &old).unwrap();
    store.remember(&path, &completed).unwrap();

    assert!(!path.exists());
}

#[test]
fn old_forget_cannot_remove_a_newer_task_marker() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("interrupted.json");
    let store = MarkerStore::default();
    let old = snapshot("completed");
    let new = snapshot("indexing");
    store.remember(&path, &new).unwrap();
    store.forget(&path, &old).unwrap();

    assert_eq!(read_marker(&path).state, "indexing");
}

#[test]
fn take_accepts_a_legacy_marker_and_blocks_older_or_replayed_operations() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("interrupted.json");
    let store = MarkerStore::default();
    let old = snapshot("cancelling");
    let inspected = snapshot("configuring");
    fs::write(
        &path,
        br#"{"task_id":"legacy","root_path":"/photos","state":"readyForReview"}"#,
    )
    .unwrap();
    let marker = store.take(&path, &inspected).unwrap().unwrap();
    assert_eq!(marker.task_id, "legacy");
    assert_eq!(marker.root_path, "/photos");
    assert_eq!(marker.state, "readyForReview");
    assert!(!path.exists());
    store.remember(&path, &old).unwrap();
    store.remember(&path, &inspected).unwrap();
    assert!(!path.exists());
}

#[test]
fn old_take_cannot_consume_a_newer_marker() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("interrupted.json");
    let store = MarkerStore::default();
    let old = snapshot("configuring");
    let new = snapshot("indexing");
    store.remember(&path, &new).unwrap();
    assert!(store.take(&path, &old).unwrap().is_none());
    assert_eq!(read_marker(&path).state, "indexing");
}

#[test]
fn io_failure_keeps_the_newer_revision_barrier() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("interrupted.json");
    let store = MarkerStore::default();
    let old = snapshot("cancelling");
    let new = snapshot("completed");
    fs::create_dir(&path).unwrap();
    assert!(store.forget(&path, &new).is_err());
    fs::remove_dir(&path).unwrap();
    store.remember(&path, &old).unwrap();
    assert!(!path.exists());
}

#[test]
fn malformed_marker_is_reported_and_preserved() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("interrupted.json");
    let store = MarkerStore::default();
    fs::write(&path, b"{broken").unwrap();
    assert!(store.take(&path, &snapshot("configuring")).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"{broken");
}

#[test]
fn take_waits_for_the_whole_marker_write() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("interrupted.json");
    let store = Arc::new(MarkerStore::default());
    let old = snapshot("readyForReview");
    let inspected = snapshot("configuring");
    let (written_tx, written_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let writer_store = Arc::clone(&store);
    let writer_path = path.clone();
    let writer = std::thread::spawn(move || {
        writer_store.apply(old.revision, || {
            write_marker(
                &writer_path,
                &InterruptedTask {
                    task_id: "task".to_string(),
                    root_path: "/synthetic".to_string(),
                    state: old.state,
                },
            )?;
            written_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            Ok(())
        })
    });
    written_rx.recv().unwrap();
    let (started_tx, started_rx) = mpsc::channel();
    let (finished_tx, finished_rx) = mpsc::channel();
    let reader_store = Arc::clone(&store);
    let reader_path = path.clone();
    let reader = std::thread::spawn(move || {
        started_tx.send(()).unwrap();
        let result = reader_store.take(&reader_path, &inspected);
        finished_tx.send(()).unwrap();
        result
    });
    started_rx.recv().unwrap();
    let finished_early = finished_rx
        .recv_timeout(std::time::Duration::from_millis(250))
        .is_ok();
    release_tx.send(()).unwrap();
    writer.join().unwrap().unwrap();
    let marker = reader.join().unwrap().unwrap().unwrap();
    assert!(
        !finished_early,
        "take must not cross a pending marker write"
    );
    assert_eq!(marker.state, "readyForReview");
    assert!(!path.exists());
}

#[test]
fn interruption_marker_round_trips_without_task_results() {
    let marker = InterruptedTask {
        task_id: "task".to_string(),
        root_path: "/photos".to_string(),
        state: "readyForReview".to_string(),
    };
    let encoded = serde_json::to_vec(&marker).unwrap();
    let decoded = serde_json::from_slice::<InterruptedTask>(&encoded).unwrap();
    assert_eq!(decoded.task_id, marker.task_id);
    assert_eq!(decoded.root_path, marker.root_path);
    assert_eq!(decoded.state, marker.state);
}
