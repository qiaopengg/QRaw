use std::ffi::OsString;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use tauri::Manager;

const MODEL_DIR_ENV: &str = "QRAW_SMART_CULLING_CALIBRATION_MODEL_DIR";
const MODEL_DIR_NAME: &str = "smart_culling_calibration_models";

pub(super) fn resolve(app_handle: &tauri::AppHandle) -> Result<PathBuf> {
    if let Some(path) = validated_override(std::env::var_os(MODEL_DIR_ENV))? {
        return Ok(path);
    }

    let resource_dir = app_handle
        .path()
        .resolve(
            format!("resources/{MODEL_DIR_NAME}"),
            tauri::path::BaseDirectory::Resource,
        )
        .ok();
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .ok()
        .map(|path| path.join(MODEL_DIR_NAME));
    let source_dir = source_model_dir(Path::new(env!("CARGO_MANIFEST_DIR")));

    resource_dir
        .into_iter()
        .chain(app_data_dir)
        .chain([source_dir])
        .find(|path| path.is_dir())
        .ok_or_else(|| {
            anyhow!(
                "Calibration model directory is unavailable; install audited models in the application data directory or set {MODEL_DIR_ENV} to an absolute directory"
            )
        })
}

fn validated_override(value: Option<OsString>) -> Result<Option<PathBuf>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err(anyhow!("{MODEL_DIR_ENV} must be an absolute directory"));
    }
    if !path.is_dir() {
        return Err(anyhow!(
            "{MODEL_DIR_ENV} does not point to a readable directory: {}",
            path.display()
        ));
    }
    Ok(Some(path))
}

fn source_model_dir(manifest_dir: &Path) -> PathBuf {
    manifest_dir.join("src/features/smart_culling/model_assets")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_fallback_stays_inside_the_independent_feature() {
        assert_eq!(
            source_model_dir(Path::new("/project/src-tauri")),
            Path::new("/project/src-tauri/src/features/smart_culling/model_assets")
        );
    }

    #[test]
    fn relative_override_is_rejected_instead_of_using_the_process_directory() {
        let error = validated_override(Some(OsString::from("relative/models"))).unwrap_err();
        assert!(error.to_string().contains("absolute"));
    }
}
