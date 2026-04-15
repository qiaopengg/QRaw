use std::sync::{Arc, Mutex};

use anyhow::Result;
use ort::session::Session;
use tauri::AppHandle;
use tokio::sync::Mutex as TokioMutex;

use crate::ai_processing::{ensure_model, get_qraw_models_dir};

use super::types::CullingModelsV4;

// ═══════════════════════════════════════════════════════════════
// Model definitions: filename, download URL, SHA256
// ═══════════════════════════════════════════════════════════════

// Face detector (required)
const FACE_DETECTOR_FILENAME: &str = "yolov8n-face.onnx";
// No public direct download URL for yolov8n-face.onnx — user can set QRAW_MODEL_FACE_DETECTOR_URL
// or it will be available if they've used the old culling feature before

// YuNet face detector (optional, preferred over YOLOv8)
const YUNET_FILENAME: &str = "face_detection_yunet_2023mar.onnx";
const YUNET_URL: &str = "https://github.com/opencv/opencv_zoo/raw/main/models/face_detection_yunet/face_detection_yunet_2023mar.onnx";
const YUNET_SHA256: &str = "8f2383e4dd3cfbb4553ea8718107fc0423210dc964f9f4280604804ed2552fa4";

// FerPlus expression model (fallback for HSEmotion)
const FERPLUS_FILENAME: &str = "emotion-ferplus-8.onnx";
const FERPLUS_URL: &str = "https://github.com/onnx/models/raw/main/validated/vision/body_analysis/emotion_ferplus/model/emotion-ferplus-8.onnx";
const FERPLUS_SHA256: &str = "a2a2ba6a335a3b29c21acb6272f962bd3d47f84952aaffa03b60986e04efa61c";

// NIMA Aesthetic model
const NIMA_AESTHETIC_FILENAME: &str = "nima.onnx";

// InsightFace 2d106det landmark model (optional, enables EAR blink detection)
const LANDMARK_106_FILENAME: &str = "2d106det.onnx";
const LANDMARK_106_URL: &str = "https://huggingface.co/fofr/comfyui/resolve/main/insightface/models/buffalo_l/2d106det.onnx";

// HSEmotion (optional, replaces FerPlus if available)
const HSEMOTION_FILENAME: &str = "hsemotion.onnx";

// NIMA Technical (optional)
const NIMA_TECHNICAL_FILENAME: &str = "nima_technical.onnx";

/// Initialize all culling V4 models
/// Required models are auto-downloaded, optional models are loaded if present
pub async fn get_or_init_culling_models_v4(
    app_handle: &AppHandle,
) -> Result<Arc<CullingModelsV4>> {
    let models_dir = get_qraw_models_dir()?;
    let _ = ort::init().with_name("AI-CullingV4").commit();

    // ── Required: Face detector ──
    // Use env var override or existing file (no default URL — must be pre-provided or from existing culling setup)
    let face_path = ensure_model(
        app_handle, &models_dir, FACE_DETECTOR_FILENAME, "FACE_DETECTOR",
        std::env::var("QRAW_MODEL_FACE_DETECTOR_URL").ok().as_deref(),
        std::env::var("QRAW_MODEL_FACE_DETECTOR_SHA256").ok().as_deref(),
    ).await?;
    let face_detector = Session::builder()?.commit_from_file(&face_path)?;

    // ── Optional: YuNet (auto-download) ──
    let yunet_detector = match ensure_model(
        app_handle, &models_dir, YUNET_FILENAME, "YUNET",
        Some(YUNET_URL), Some(YUNET_SHA256),
    ).await {
        Ok(path) => Session::builder()?.commit_from_file(&path).ok().map(Mutex::new),
        Err(e) => {
            log::warn!("YuNet model not available: {}", e);
            None
        }
    };

    // ── Optional: FerPlus expression (auto-download) ──
    let ferplus_session = match ensure_model(
        app_handle, &models_dir, FERPLUS_FILENAME, "EXPRESSION",
        Some(FERPLUS_URL), Some(FERPLUS_SHA256),
    ).await {
        Ok(path) => Session::builder()?.commit_from_file(&path).ok(),
        Err(e) => {
            log::warn!("FerPlus model not available: {}", e);
            None
        }
    };

    // ── Optional: HSEmotion (GitHub 下载受网络限制，需手动准备) ──
    // TODO: hsemotion.onnx 需要从 GitHub 下载或 Python 导出
    //   方式1: pip3 install hsemotion-onnx && python3 -c "from hsemotion_onnx.facial_emotions import HSEmotionRecognizer; HSEmotionRecognizer(model_name='enet_b0_8_best_afew')"
    //          然后从 ~/.cache 中找到 enet_b0_8_best_afew.onnx 复制到 ~/.qraw/models/hsemotion.onnx
    //   方式2: 直接下载 https://github.com/HSE-asavchenko/face-emotion-recognition/raw/main/models/affectnet_emotions/onnx/enet_b0_8_best_afew.onnx
    //   不影响主流程：缺失时自动降级为 FerPlus
    let hsemotion_session = {
        let path = models_dir.join(HSEMOTION_FILENAME);
        if path.exists() {
            match Session::builder()?.commit_from_file(&path) {
                Ok(sess) => {
                    log::info!("Loaded HSEmotion model");
                    Some(sess)
                }
                Err(e) => {
                    log::warn!("Failed to load HSEmotion: {}", e);
                    None
                }
            }
        } else {
            log::info!("HSEmotion not found, using FerPlus fallback");
            None
        }
    };

    // Expression: prefer HSEmotion, fallback to FerPlus
    let expression_model = hsemotion_session
        .or(ferplus_session)
        .map(Mutex::new);

    // ── Optional: 106-point landmark (auto-download from HuggingFace) ──
    let landmark_106 = match ensure_model(
        app_handle, &models_dir, LANDMARK_106_FILENAME, "LANDMARK_106",
        Some(LANDMARK_106_URL), None,
    ).await {
        Ok(path) => {
            match Session::builder()?.commit_from_file(&path) {
                Ok(sess) => {
                    log::info!("Loaded 2d106det landmark model — EAR blink detection enabled");
                    Some(Mutex::new(sess))
                }
                Err(e) => {
                    log::warn!("Failed to load 2d106det: {}", e);
                    None
                }
            }
        }
        Err(e) => {
            log::warn!("2d106det download failed: {} — EAR blink detection disabled", e);
            None
        }
    };

    // ── Optional: NIMA Aesthetic (需手动准备) ──
    // TODO: nima.onnx 需要从 idealo Keras 权重导出
    //   方式: pip3 install tensorflow tf2onnx && 运行 scripts/export_models.py
    //   不影响主流程：缺失时跳过美学评分
    let nima_aesthetic = {
        let path = models_dir.join(NIMA_AESTHETIC_FILENAME);
        if path.exists() {
            Session::builder()?.commit_from_file(&path).ok().map(Mutex::new)
        } else {
            None
        }
    };

    // ── Optional: NIMA Technical (需手动准备) ──
    // TODO: nima_technical.onnx 需要从 idealo Keras 权重导出
    //   方式: 同 nima.onnx，使用 weights_mobilenet_technical_0.11.hdf5
    //   不影响主流程：缺失时跳过技术质量交叉验证
    let nima_technical = {
        let path = models_dir.join(NIMA_TECHNICAL_FILENAME);
        if path.exists() {
            Session::builder()?.commit_from_file(&path).ok().map(Mutex::new)
        } else {
            None
        }
    };

    crate::register_exit_handler();

    let models = Arc::new(CullingModelsV4 {
        face_detector: Mutex::new(face_detector),
        yunet_detector,
        landmark_106,
        expression_model,
        nima_aesthetic,
        nima_technical,
    });

    Ok(models)
}
