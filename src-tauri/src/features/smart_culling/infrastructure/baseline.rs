use std::fs;
use std::io::Read;
use std::path::Path;
use std::time::UNIX_EPOCH;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FileBaseline {
    pub length: u64,
    pub modified_nanos: Option<u128>,
    pub content_hash: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SidecarBaseline {
    pub exists: bool,
    pub length: u64,
    pub modified_nanos: Option<u128>,
    pub content_hash: Option<String>,
}

pub(crate) fn capture_file_baseline(path: &Path) -> Result<FileBaseline, String> {
    let initial = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if !initial.is_file() {
        return Err(format!("Photo is not a regular file: {}", path.display()));
    }
    let mut file = fs::File::open(path).map_err(|error| error.to_string())?;
    let metadata = file.metadata().map_err(|error| error.to_string())?;
    if !same_file_snapshot(&initial, &metadata) {
        return Err(format!(
            "Photo changed before capturing its baseline: {}",
            path.display()
        ));
    }
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let after = file.metadata().map_err(|error| error.to_string())?;
    let current = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if !same_file_snapshot(&metadata, &after) || !same_file_snapshot(&after, &current) {
        return Err(format!(
            "Photo changed while capturing its baseline: {}",
            path.display()
        ));
    }
    Ok(FileBaseline {
        length: metadata.len(),
        modified_nanos: modified_nanos(&metadata),
        content_hash: hasher.finalize().to_hex().to_string(),
    })
}

fn same_file_snapshot(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    let same = left.is_file()
        && right.is_file()
        && left.len() == right.len()
        && modified_nanos(left) == modified_nanos(right);
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        same && left.dev() == right.dev()
            && left.ino() == right.ino()
            && left.ctime() == right.ctime()
            && left.ctime_nsec() == right.ctime_nsec()
    }
    #[cfg(not(unix))]
    {
        same
    }
}

pub(crate) fn capture_sidecar_baseline(path: &Path) -> Result<SidecarBaseline, String> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(SidecarBaseline {
                exists: false,
                length: 0,
                modified_nanos: None,
                content_hash: None,
            });
        }
        Err(error) => return Err(error.to_string()),
    };
    let bytes = fs::read(path).map_err(|error| error.to_string())?;

    Ok(SidecarBaseline {
        exists: true,
        length: metadata.len(),
        modified_nanos: modified_nanos(&metadata),
        content_hash: Some(blake3::hash(&bytes).to_hex().to_string()),
    })
}

fn modified_nanos(metadata: &fs::Metadata) -> Option<u128> {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos())
}

#[cfg(test)]
#[path = "baseline_tests.rs"]
mod tests;
