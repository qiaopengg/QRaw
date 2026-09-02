use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{Result, anyhow};
use image::DynamicImage;
use once_cell::sync::OnceCell;
#[cfg(target_os = "macos")]
use ort::execution_providers::coreml::{
    CoreMLComputeUnits, CoreMLExecutionProvider, CoreMLModelFormat,
};
#[cfg(target_os = "windows")]
use ort::execution_providers::directml::DirectMLExecutionProvider;
use ort::session::{Session, builder::GraphOptimizationLevel};
use ort::tensor::TensorElementType;
use ort::value::ValueType;
use sha2::{Digest, Sha256};
use tauri::Manager;

use super::face_identity::run_sface_embedding;
use super::face_models::{run_ocec_classification, run_yunet_detection};

const YUNET_MODEL_FILENAME: &str = "face_detection_yunet_2023mar.onnx";
const OCEC_MODEL_FILENAME: &str = "ocec_l.onnx";
const SFACE_MODEL_FILENAME: &str = "face_recognition_sface_2021dec_coreml.onnx";
const OCEC_BATCH_DIMENSION: (&str, i64) = ("batch", 1);
const OCEC_INPUT_SHAPE: [i64; 4] = [1, 3, 24, 40];
const YUNET_SHA256: &str = "8f2383e4dd3cfbb4553ea8718107fc0423210dc964f9f4280604804ed2552fa4";
const OCEC_SHA256: &str = "de9b8031f8b521a862d8cff55ba88c2fccab6ac96484ba53154dd12c53c7c7f9";
const SFACE_SHA256: &str = "3e4a66d8a95745ce8b972e78d1330918db04bdb8ef4a81d02088c50aa8d55a15";

pub struct SmartCullingFaceModels {
    pub yunet: Mutex<Session>,
    pub ocec: Mutex<Session>,
    pub sface: Mutex<Session>,
    // FER+ is intentionally not loaded: class-distribution confidence is not
    // validated expression-quality evidence. Its bundled resource remains
    // only for rollback until a replacement face-motion model is accepted.
}

// Core ML session teardown is asynchronous. Keeping the compiled models
// for the application lifetime also avoids recompiling them for every folder
// inspection and prevents native cleanup from racing a later task.
static FACE_MODELS: OnceCell<Arc<SmartCullingFaceModels>> = OnceCell::new();

/// Loads the bundled YuNet + OCEC ONNX models from the app's resource
/// directory. Unlike the other AI models in this project, these are
/// committed to the repository and shipped inside the installer (see
/// `tauri.conf.json` `bundle.resources`), so no runtime download is needed.
pub fn load_face_models(app_handle: &tauri::AppHandle) -> Result<Arc<SmartCullingFaceModels>> {
    FACE_MODELS
        .get_or_try_init(|| load_face_models_uncached(app_handle))
        .cloned()
}

fn load_face_models_uncached(app_handle: &tauri::AppHandle) -> Result<Arc<SmartCullingFaceModels>> {
    let resource_dir = app_handle
        .path()
        .resolve(
            "resources/smart_culling_models",
            tauri::path::BaseDirectory::Resource,
        )
        .map_err(|e| anyhow!("Failed to resolve smart_culling_models resource dir: {}", e))?;

    load_face_models_from_resource_dir(&resource_dir)
}

#[cfg(test)]
pub(super) fn load_face_models_for_test(
    resource_dir: &Path,
) -> Result<Arc<SmartCullingFaceModels>> {
    load_face_models_from_resource_dir(resource_dir)
}

fn load_face_models_from_resource_dir(resource_dir: &Path) -> Result<Arc<SmartCullingFaceModels>> {
    if !cfg!(any(target_os = "macos", target_os = "windows")) {
        return Err(anyhow!(
            "Smart culling is available only on validated macOS and Windows GPU paths"
        ));
    }

    let yunet_path = resource_dir.join(YUNET_MODEL_FILENAME);
    let ocec_path = resource_dir.join(OCEC_MODEL_FILENAME);
    let sface_path = resource_dir.join(SFACE_MODEL_FILENAME);

    verify_model(&yunet_path, YUNET_SHA256)?;
    verify_model(&ocec_path, OCEC_SHA256)?;
    verify_model(&sface_path, SFACE_SHA256)?;

    let yunet = gpu_session(&yunet_path, None)?;
    validate_session_contract(
        &yunet,
        "input",
        &[1, 3, 640, 640],
        &[
            "cls_8", "cls_16", "cls_32", "obj_8", "obj_16", "obj_32", "bbox_8", "bbox_16",
            "bbox_32", "kps_8", "kps_16", "kps_32",
        ],
    )?;
    // OCEC declares an unbounded `batch` dimension, while this feature always
    // classifies one face crop at a time. Pinning it to one lets Core ML compile
    // the complete graph as a static MLProgram instead of rejecting the graph
    // or failing when an unbounded program is torn down.
    let ocec = gpu_session(&ocec_path, Some(OCEC_BATCH_DIMENSION))?;
    validate_session_contract(&ocec, "images", &OCEC_INPUT_SHAPE, &["prob_open"])?;
    let sface = gpu_session(&sface_path, None)?;
    validate_session_contract(&sface, "data", &[1, 3, 112, 112], &["fc1"])?;

    let models = Arc::new(SmartCullingFaceModels {
        yunet: Mutex::new(yunet),
        ocec: Mutex::new(ocec),
        sface: Mutex::new(sface),
    });
    smoke_test(&models)?;
    Ok(models)
}

fn smoke_test(models: &SmartCullingFaceModels) -> Result<()> {
    let image = DynamicImage::new_rgb8(64, 64);
    run_yunet_detection(&image, &models.yunet)
        .map_err(|error| anyhow!("YuNet GPU smoke test failed: {error}"))?;
    run_ocec_classification(&image, &models.ocec)
        .map_err(|error| anyhow!("OCEC GPU smoke test failed: {error}"))?;
    let landmarks = [
        (38.2946, 51.6963),
        (73.5318, 51.5014),
        (56.0252, 71.7366),
        (41.5493, 92.3655),
        (70.7299, 92.2041),
    ];
    let sface_image = DynamicImage::new_rgb8(112, 112);
    run_sface_embedding(&sface_image, &landmarks, &models.sface)
        .map_err(|error| anyhow!("SFace Core ML/DirectML smoke test failed: {error}"))?;
    Ok(())
}

pub(super) fn verify_model(path: &Path, expected_sha256: &str) -> Result<()> {
    let bytes = fs::read(path)
        .map_err(|error| anyhow!("Bundled model cannot be read ({}): {error}", path.display()))?;
    let actual = hex::encode(Sha256::digest(&bytes));
    if actual != expected_sha256 {
        return Err(anyhow!(
            "Bundled model integrity check failed ({}): expected {expected_sha256}, got {actual}",
            path.display()
        ));
    }
    Ok(())
}

fn gpu_session(model_path: &PathBuf, dimension_override: Option<(&str, i64)>) -> Result<Session> {
    gpu_session_with_optimization(model_path, dimension_override, None)
}

pub(super) fn gpu_session_with_optimization(
    model_path: &PathBuf,
    dimension_override: Option<(&str, i64)>,
    optimization_level: Option<GraphOptimizationLevel>,
) -> Result<Session> {
    // The currently bundled YuNet/OCEC models pass the strict provider gate.
    // Future models may use an audited low-cost CPU-node allowlist, but must
    // not weaken this validated path globally.
    let builder = Session::builder()?.with_config_entry("session.disable_cpu_ep_fallback", "1")?;
    let builder = if let Some((name, size)) = dimension_override {
        builder.with_dimension_override(name, size)?
    } else {
        builder
    };
    let builder = if let Some(level) = optimization_level {
        builder.with_optimization_level(level)?
    } else {
        builder
    };

    #[cfg(target_os = "macos")]
    let builder = builder.with_execution_providers([CoreMLExecutionProvider::default()
        // MLProgram supports the complete OCEC graph while the legacy
        // NeuralNetwork format leaves nodes on the default CPU EP.
        .with_model_format(CoreMLModelFormat::MLProgram)
        .with_compute_units(CoreMLComputeUnits::CPUAndGPU)
        .with_static_input_shapes(true)
        .build()
        .error_on_failure()])?;

    #[cfg(target_os = "windows")]
    let builder = builder.with_execution_providers([DirectMLExecutionProvider::default()
        .build()
        .error_on_failure()])?;

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let builder = builder;

    builder
        .commit_from_file(model_path)
        .map_err(|error| anyhow!("Validated GPU inference path is unavailable: {error}"))
}

pub(super) fn validate_session_contract(
    session: &Session,
    input_name: &str,
    expected_shape: &[i64],
    output_names: &[&str],
) -> Result<()> {
    let input = session
        .inputs
        .iter()
        .find(|input| input.name == input_name)
        .ok_or_else(|| anyhow!("Bundled model input '{input_name}' is missing"))?;
    let ValueType::Tensor { ty, shape, .. } = &input.input_type else {
        return Err(anyhow!(
            "Bundled model input '{input_name}' is not a tensor"
        ));
    };
    if *ty != TensorElementType::Float32 || !shape_matches(shape, expected_shape) {
        return Err(anyhow!(
            "Bundled model input '{input_name}' contract mismatch: expected float32 {expected_shape:?}, got {ty:?} {shape}"
        ));
    }
    for output_name in output_names {
        if !session
            .outputs
            .iter()
            .any(|output| output.name == *output_name)
        {
            return Err(anyhow!("Bundled model output '{output_name}' is missing"));
        }
    }
    Ok(())
}

fn shape_matches(actual: &[i64], expected: &[i64]) -> bool {
    actual.len() == expected.len()
        && actual
            .iter()
            .zip(expected)
            .all(|(actual, expected)| *expected < 0 || actual == expected)
}

#[cfg(test)]
mod tests {
    use super::{OCEC_BATCH_DIMENSION, OCEC_INPUT_SHAPE, shape_matches};

    #[test]
    fn contract_shape_allows_only_declared_dynamic_dimensions() {
        assert!(shape_matches(&[8, 3, 24, 40], &[-1, 3, 24, 40]));
        assert!(!shape_matches(&[8, 1, 24, 40], &[-1, 3, 24, 40]));
    }

    #[test]
    fn ocec_session_contract_pins_the_runtime_batch_to_one() {
        assert_eq!(OCEC_BATCH_DIMENSION, ("batch", 1));
        assert_eq!(OCEC_INPUT_SHAPE, [1, 3, 24, 40]);
        assert!(!shape_matches(&[2, 3, 24, 40], &OCEC_INPUT_SHAPE));
    }
}
