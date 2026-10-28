use super::*;

#[test]
fn same_size_and_timestamp_do_not_hide_changed_photo_content() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("photo.jpg");
    fs::write(&path, b"original").unwrap();
    let modified = fs::metadata(&path).unwrap().modified().unwrap();
    let before = capture_file_baseline(&path).unwrap();
    fs::write(&path, b"replaced").unwrap();
    fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(modified))
        .unwrap();
    let after = capture_file_baseline(&path).unwrap();
    assert_eq!(before.length, after.length);
    assert_eq!(before.modified_nanos, after.modified_nanos);
    assert_ne!(
        before, after,
        "content changes must invalidate an analysed photo"
    );
}

#[test]
fn baseline_is_stable_for_unchanged_content_and_rejects_non_files() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("photo.jpg");
    fs::write(&path, b"unchanged").unwrap();
    let first = capture_file_baseline(&path).unwrap();
    assert_eq!(first, capture_file_baseline(&path).unwrap());
    assert!(capture_file_baseline(directory.path()).is_err());
    assert!(capture_file_baseline(&directory.path().join("missing.jpg")).is_err());
    #[cfg(unix)]
    {
        let link = directory.path().join("linked.jpg");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        assert!(capture_file_baseline(&link).is_err());
    }
}
