use std::env;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::AppState;
use crate::ai_processing::{
    ensure_model, get_model_env, get_or_init_ai_models, get_or_init_clip_models,
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
const STYLE_BACKBONE_ENV: &str = "STYLE_TRANSFER_DINOV2_VITB";
const STYLE_PREPROCESS_ENV: &str = "STYLE_TRANSFER_DINOV2_VITB_PREPROCESS";

fn get_env_or_none(key: &str) -> Option<String> {
    env::var(key).ok().filter(|value| !value.trim().is_empty())
}

async fn ensure_style_transfer_backbone_artifacts(
    app_handle: &tauri::AppHandle,
) -> Result<(), String> {
    let model_dir = get_qraw_models_dir().map_err(|e| e.to_string())?;

    let backbone_url = get_env_or_none("QRAW_STYLE_TRANSFER_BACKBONE_URL")
        .or_else(|| get_model_env(STYLE_BACKBONE_ENV, "URL"));
    let backbone_sha = get_env_or_none("QRAW_STYLE_TRANSFER_BACKBONE_SHA256")
        .or_else(|| get_model_env(STYLE_BACKBONE_ENV, "SHA256"));
    let preprocess_url = get_env_or_none("QRAW_STYLE_TRANSFER_PREPROCESS_URL")
        .or_else(|| get_model_env(STYLE_PREPROCESS_ENV, "URL"));
    let preprocess_sha = get_env_or_none("QRAW_STYLE_TRANSFER_PREPROCESS_SHA256")
        .or_else(|| get_model_env(STYLE_PREPROCESS_ENV, "SHA256"));

    ensure_model(
        app_handle,
        &model_dir,
        STYLE_TRANSFER_BACKBONE_FILENAME,
        STYLE_BACKBONE_ENV,
        backbone_url.as_deref(),
        backbone_sha.as_deref(),
    )
    .await
    .map_err(|e| e.to_string())?;

    ensure_model(
        app_handle,
        &model_dir,
        STYLE_TRANSFER_PREPROCESS_FILENAME,
        STYLE_PREPROCESS_ENV,
        preprocess_url.as_deref(),
        preprocess_sha.as_deref(),
    )
    .await
    .map_err(|e| e.to_string())?;

    Ok(())
}

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
            "Style Transfer Backbone",
            STYLE_TRANSFER_BACKBONE_FILENAME,
            "style-understanding",
            true,
        ),
        make_status(
            &model_dir,
            "style-backbone-preprocess",
            "Style Transfer Backbone Preprocess",
            STYLE_TRANSFER_PREPROCESS_FILENAME,
            "style-understanding-config",
            true,
        ),
        make_status(
            &model_dir,
            "sam-encoder",
            "SAM Encoder",
            SAM_ENCODER_FILENAME,
            "subject-mask",
            true,
        ),
        make_status(
            &model_dir,
            "sam-decoder",
            "SAM Decoder",
            SAM_DECODER_FILENAME,
            "subject-mask",
            true,
        ),
        make_status(
            &model_dir,
            "u2net-foreground",
            "Foreground Model",
            U2NET_FILENAME,
            "semantic-region",
            true,
        ),
        make_status(
            &model_dir,
            "skyseg",
            "Sky Segmentation Model",
            SKYSEG_FILENAME,
            "semantic-region",
            true,
        ),
        make_status(
            &model_dir,
            "depth-anything",
            "Depth Model",
            DEPTH_FILENAME,
            "semantic-region",
            true,
        ),
        make_status(
            &model_dir,
            "clip-model",
            "CLIP Model",
            CLIP_MODEL_FILENAME,
            "optional-vlm-assist",
            false,
        ),
        make_status(
            &model_dir,
            "clip-tokenizer",
            "CLIP Tokenizer",
            CLIP_TOKENIZER_FILENAME,
            "optional-vlm-assist",
            false,
        ),
    ];

    let required_count = models.iter().filter(|m| m.required).count();
    let ready_count = models.iter().filter(|m| m.ready).count();
    let required_ready = models.iter().filter(|m| m.required).all(|m| m.ready);

    Ok(StyleTransferModelStatusResponse {
        model_dir: model_dir.display().to_string(),
        required_ready,
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
    ensure_style_transfer_backbone_artifacts(&app_handle).await?;

    get_or_init_ai_models(&app_handle, &state.ai_state, &state.ai_init_lock)
        .await
        .map_err(|e| e.to_string())?;

    let _ = get_or_init_clip_models(&app_handle, &state.ai_state, &state.ai_init_lock)
        .await
        .map_err(|e| e.to_string());

    get_style_transfer_model_status_response()
}
