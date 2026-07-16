use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FileBaseline {
    pub length: u64,
    pub modified_nanos: Option<u128>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SidecarBaseline {
    pub exists: bool,
    pub length: u64,
    pub modified_nanos: Option<u128>,
    pub content_hash: Option<String>,
}

pub(crate) fn capture_file_baseline(path: &Path) -> Result<FileBaseline, String> {
    let metadata = fs::metadata(path).map_err(|error| error.to_string())?;
    Ok(FileBaseline {
        length: metadata.len(),
        modified_nanos: modified_nanos(&metadata),
    })
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
