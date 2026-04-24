use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::AppState;
use crate::ai_processing::{
    get_or_init_ai_models, get_or_init_clip_models, get_or_init_style_transfer_backbone,
    get_qraw_models_dir,
};

const STYLE_TRANSFER_BACKBONE_FILENAME: &str = "style_transfer_dinov2_vitb.onnx";
const STYLE_TRANSFER_PREPROCESS_FILENAME: &str = "style_transfer_dinov2_vitb.preprocess.json";
const SAM_ENCODER_FILENAME: &str = "sam_vit_b_01ec64_encoder.onnx";
const SAM_DECODER_FILENAME: &str = "sam_vit_b_01ec64_decoder.onnx";
const U2NET_FILENAME: &str = "u2net.onnx";
const SKYSEG_FILENAME: &str = "skyseg_u2net.onnx";
const DEPTH_FILENAME: &str = "depth_anything_v2_vits.onnx";
const CLIP_MODEL_FILENAME: &str = "clip_model.onnx";
const CLIP_TOKENIZER_FILENAME: &str = "clip_tokenizer.json";
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StyleTransferModelArtifactStatus {
    pub id: String,
    pub name: String,
    pub filename: String,
    pub role: String,
    pub required: bool,
    pub ready: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StyleTransferModelStatusResponse {
    pub model_dir: String,
    pub required_ready: bool,
    pub full_ready: bool,
    pub degraded_mode: bool,
    pub ready_count: usize,
    pub required_count: usize,
    pub models: Vec<StyleTransferModelArtifactStatus>,
}

fn make_status(
    model_dir: &Path,
    id: &str,
    name: &str,
    filename: &str,
    role: &str,
    required: bool,
) -> StyleTransferModelArtifactStatus {
    let path = model_dir.join(filename);
    let metadata = fs::metadata(&path).ok();
    StyleTransferModelArtifactStatus {
        id: id.to_string(),
        name: name.to_string(),
        filename: filename.to_string(),
        role: role.to_string(),
        required,
        ready: metadata.is_some(),
        path: path.exists().then(|| path.display().to_string()),
        bytes: metadata.map(|m| m.len()),
    }
}

pub fn get_style_transfer_model_status_response() -> Result<StyleTransferModelStatusResponse, String>
{
    let model_dir = get_qraw_models_dir().map_err(|e| e.to_string())?;
    let models = vec![
        make_status(
            &model_dir,
            "style-backbone",
            "智能风格理解",
            STYLE_TRANSFER_BACKBONE_FILENAME,
            "style-understanding",
            true,
        ),
        make_status(
            &model_dir,
            "style-backbone-preprocess",
            "风格理解配置",
            STYLE_TRANSFER_PREPROCESS_FILENAME,
            "style-understanding-config",
            true,
        ),
        make_status(
            &model_dir,
            "sam-encoder",
            "智能主体识别",
            SAM_ENCODER_FILENAME,
            "subject-mask",
            true,
        ),
        make_status(
            &model_dir,
            "sam-decoder",
            "主体识别解码",
            SAM_DECODER_FILENAME,
            "subject-mask",
            true,
        ),
        make_status(
            &model_dir,
            "u2net-foreground",
            "前景分离",
            U2NET_FILENAME,
            "semantic-region",
            true,
        ),
        make_status(
            &model_dir,
            "skyseg",
            "天空识别",
            SKYSEG_FILENAME,
            "semantic-region",
            true,
        ),
        make_status(
            &model_dir,
            "depth-anything",
            "景深分析",
            DEPTH_FILENAME,
            "semantic-region",
            true,
        ),
        make_status(
            &model_dir,
            "clip-model",
            "图像理解（可选）",
            CLIP_MODEL_FILENAME,
            "optional-vlm-assist",
            false,
        ),
        make_status(
            &model_dir,
            "clip-tokenizer",
            "文本理解（可选）",
            CLIP_TOKENIZER_FILENAME,
            "optional-vlm-assist",
            false,
        ),
    ];

    let required_count = models.iter().filter(|m| m.required).count();
    let ready_count = models.iter().filter(|m| m.required && m.ready).count();
    let required_ready = models.iter().filter(|m| m.required).all(|m| m.ready);
    let backbone_ready = models
        .iter()
        .filter(|m| {
            matches!(
                m.id.as_str(),
                "style-backbone" | "style-backbone-preprocess"
            )
        })
        .all(|m| m.ready);
    let full_ready = required_ready;
    let degraded_mode = required_ready && !backbone_ready;

    Ok(StyleTransferModelStatusResponse {
        model_dir: model_dir.display().to_string(),
        required_ready,
        full_ready,
        degraded_mode,
        ready_count,
        required_count,
        models,
    })
}

#[tauri::command]
pub fn get_style_transfer_model_status() -> Result<StyleTransferModelStatusResponse, String> {
    get_style_transfer_model_status_response()
}

#[tauri::command]
pub async fn prepare_style_transfer_models(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<StyleTransferModelStatusResponse, String> {
    get_or_init_style_transfer_backbone(&app_handle, &state.ai_init_lock)
        .await
        .map_err(|e| e.to_string())?;

    get_or_init_ai_models(&app_handle, &state.ai_state, &state.ai_init_lock)
        .await
        .map_err(|e| e.to_string())?;

    let _ = get_or_init_clip_models(&app_handle, &state.ai_state, &state.ai_init_lock)
        .await
        .map_err(|e| e.to_string());

    get_style_transfer_model_status_response()
}
