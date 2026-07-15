use std::sync::{Arc, Mutex};

use anyhow::{Result, anyhow};
use ort::session::Session;
use tauri::Manager;

const YUNET_MODEL_FILENAME: &str = "face_detection_yunet_2023mar.onnx";
const OCEC_MODEL_FILENAME: &str = "ocec_l.onnx";

pub struct SmartCullingFaceModels {
    pub yunet: Mutex<Session>,
    pub ocec: Mutex<Session>,
}

/// Loads the bundled YuNet + OCEC ONNX models from the app's resource
/// directory. Unlike the other AI models in this project, these are
/// committed to the repository and shipped inside the installer (see
/// `tauri.conf.json` `bundle.resources`), so no runtime download is needed.
pub fn load_face_models(app_handle: &tauri::AppHandle) -> Result<Arc<SmartCullingFaceModels>> {
    let resource_dir = app_handle
        .path()
        .resolve(
            "resources/smart_culling_models",
            tauri::path::BaseDirectory::Resource,
        )
        .map_err(|e| anyhow!("Failed to resolve smart_culling_models resource dir: {}", e))?;

    let yunet_path = resource_dir.join(YUNET_MODEL_FILENAME);
    let ocec_path = resource_dir.join(OCEC_MODEL_FILENAME);

    if !yunet_path.exists() {
        return Err(anyhow!("Bundled model not found: {}", yunet_path.display()));
    }
    if !ocec_path.exists() {
        return Err(anyhow!("Bundled model not found: {}", ocec_path.display()));
    }

    let yunet = Session::builder()?.commit_from_file(&yunet_path)?;
    let ocec = Session::builder()?.commit_from_file(&ocec_path)?;

    Ok(Arc::new(SmartCullingFaceModels {
        yunet: Mutex::new(yunet),
        ocec: Mutex::new(ocec),
    }))
}
