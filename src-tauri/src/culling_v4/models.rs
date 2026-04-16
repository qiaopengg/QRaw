use std::sync::{Arc, Mutex};

use anyhow::Result;
use ort::session::Session;
use tauri::AppHandle;

use crate::ai_processing::{ensure_model, get_qraw_models_dir};
use super::types::CullingModelsV4;

const FACE_DETECTOR_FILENAME: &str = "yolov8n-face.onnx";
const YUNET_FILENAME: &str = "face_detection_yunet_2023mar.onnx";
const YUNET_URL: &str = "https://github.com/opencv/opencv_zoo/raw/main/models/face_detection_yunet/face_detection_yunet_2023mar.onnx";
const YUNET_SHA256: &str = "8f2383e4dd3cfbb4553ea8718107fc0423210dc964f9f4280604804ed2552fa4";
const FERPLUS_FILENAME: &str = "emotion-ferplus-8.onnx";
const FERPLUS_URL: &str = "https://github.com/onnx/models/raw/main/validated/vision/body_analysis/emotion_ferplus/model/emotion-ferplus-8.onnx";
const FERPLUS_SHA256: &str = "a2a2ba6a335a3b29c21acb6272f962bd3d47f84952aaffa03b60986e04efa61c";
const NIMA_AESTHETIC_FILENAME: &str = "nima.onnx";
const LANDMARK_106_FILENAME: &str = "2d106det.onnx";
const LANDMARK_106_URL: &str = "https://huggingface.co/fofr/comfyui/resolve/main/insightface/models/buffalo_l/2d106det.onnx";
const HSEMOTION_FILENAME: &str = "hsemotion.onnx";
const NIMA_TECHNICAL_FILENAME: &str = "nima_technical.onnx";

fn try_load(path: &std::path::Path, name: &str) -> Option<Session> {
    if !path.exists() { return None; }
    match Session::builder().and_then(|b| b.commit_from_file(path)) {
        Ok(s) => { log::info!("Loaded {}", name); Some(s) }
        Err(e) => { log::warn!("Failed to load {}: {}", name, e); None }
    }
}

pub async fn get_or_init_culling_models_v4(app_handle: &AppHandle) -> Result<Arc<CullingModelsV4>> {
    let dir = get_qraw_models_dir()?;
    let _ = ort::init().with_name("AI-CullingV4").commit();

    // ── YuNet (primary face detector, auto-download) ──
    let yunet_detector = match ensure_model(app_handle, &dir, YUNET_FILENAME, "YUNET", Some(YUNET_URL), Some(YUNET_SHA256)).await {
        Ok(p) => try_load(&p, "YuNet").map(Mutex::new),
        Err(_) => None,
    };

    // ── YOLOv8-Face (fallback, loaded if present, no auto-download) ──
    let yolo_session = try_load(&dir.join(FACE_DETECTOR_FILENAME), "YOLOv8-Face");

    // Need at least one face detector
    let face_detector = if let Some(s) = yolo_session {
        Mutex::new(s)
    } else if let Some(ref _y) = yunet_detector {
        // Use YuNet as both primary and fallback
        let p = dir.join(YUNET_FILENAME);
        Mutex::new(Session::builder()?.commit_from_file(&p)?)
    } else {
        return Err(anyhow::anyhow!("No face detection model available"));
    };

    // ── FerPlus (auto-download) ──
    let ferplus = match ensure_model(app_handle, &dir, FERPLUS_FILENAME, "EXPRESSION", Some(FERPLUS_URL), Some(FERPLUS_SHA256)).await {
        Ok(p) => try_load(&p, "FerPlus"),
        Err(_) => None,
    };

    // ── HSEmotion (loaded if present) ──
    let hsemotion = try_load(&dir.join(HSEMOTION_FILENAME), "HSEmotion");
    let expression_model = hsemotion.or(ferplus).map(Mutex::new);

    // ── 106-point landmark (auto-download) ──
    let landmark_106 = match ensure_model(app_handle, &dir, LANDMARK_106_FILENAME, "LANDMARK_106", Some(LANDMARK_106_URL), None).await {
        Ok(p) => try_load(&p, "2d106det").map(Mutex::new),
        Err(_) => None,
    };

    // ── NIMA Aesthetic (loaded if present) ──
    let nima_aesthetic = try_load(&dir.join(NIMA_AESTHETIC_FILENAME), "NIMA-Aesthetic").map(Mutex::new);

    // ── NIMA Technical (loaded if present) ──
    let nima_technical = try_load(&dir.join(NIMA_TECHNICAL_FILENAME), "NIMA-Technical").map(Mutex::new);

    crate::register_exit_handler();

    Ok(Arc::new(CullingModelsV4 {
        face_detector,
        yunet_detector,
        landmark_106,
        expression_model,
        nima_aesthetic,
        nima_technical,
    }))
}
