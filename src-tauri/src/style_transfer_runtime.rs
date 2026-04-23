use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::file_management::load_settings;
use crate::style_transfer::{
    self, StyleTransferDebugInfo, StyleTransferResponse, StyleTransferSuggestion,
};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum StyleTransferMode {
    Analysis,
}

impl StyleTransferMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Analysis => "analysis",
        }
    }

    fn stage_label(self) -> &'static str {
        match self {
            Self::Analysis => "analysis",
        }
    }

    fn expected_wait_range(self) -> &'static str {
        match self {
            Self::Analysis => "1-5s",
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum StyleTransferPreset {
    Realistic,
    Artistic,
    Creative,
}

impl StyleTransferPreset {
    fn as_str(self) -> &'static str {
        match self {
            Self::Realistic => "realistic",
            Self::Artistic => "artistic",
            Self::Creative => "creative",
        }
    }

    fn from_settings(value: Option<&str>) -> Self {
        match value.unwrap_or("artistic") {
            "realistic" => Self::Realistic,
            "creative" => Self::Creative,
            _ => Self::Artistic,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StyleTransferRunRequest {
    pub reference_path: String,
    pub current_image_path: String,
    pub current_adjustments: Value,
    pub mode: Option<StyleTransferMode>,
    pub preset: Option<StyleTransferPreset>,
    pub style_strength: Option<f64>,
    pub highlight_guard_strength: Option<f64>,
    pub skin_protect_strength: Option<f64>,
    pub pure_algorithm: Option<bool>,
    pub enable_expert_preset: Option<bool>,
    pub enable_feature_mapping: Option<bool>,
    pub enable_auto_refine: Option<bool>,
    pub enable_lut: Option<bool>,
    pub enable_vlm: Option<bool>,
    pub llm_endpoint: Option<String>,
    pub llm_api_key: Option<String>,
    pub llm_model: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StyleTransferExecutionMeta {
    pub requested_mode: String,
    pub resolved_mode: String,
    pub engine: String,
    pub preset: String,
    pub stage: String,
    pub expected_wait_range: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StyleTransferExecutionResponse {
    pub understanding: String,
    #[serde(default)]
    pub adjustments: Vec<StyleTransferSuggestion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style_debug: Option<StyleTransferDebugInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_image_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview_image_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pure_generation_image_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_processed_image_path: Option<String>,
    pub execution_meta: StyleTransferExecutionMeta,
}

fn resolve_mode(
    explicit: Option<StyleTransferMode>,
    _app_handle: &tauri::AppHandle,
) -> StyleTransferMode {
    if let Some(mode) = explicit {
        return mode;
    }
    StyleTransferMode::Analysis
}

fn resolve_preset(
    explicit: Option<StyleTransferPreset>,
    app_handle: &tauri::AppHandle,
) -> StyleTransferPreset {
    if let Some(preset) = explicit {
        return preset;
    }

    let settings = load_settings(app_handle.clone()).unwrap_or_default();
    StyleTransferPreset::from_settings(settings.style_transfer_preset.as_deref())
}

fn wrap_analysis_response(
    analysis_response: StyleTransferResponse,
    requested_mode: StyleTransferMode,
    preset: StyleTransferPreset,
) -> StyleTransferExecutionResponse {
    StyleTransferExecutionResponse {
        understanding: analysis_response.understanding,
        adjustments: analysis_response.adjustments,
        style_debug: analysis_response.style_debug,
        output_image_path: None,
        preview_image_path: None,
        pure_generation_image_path: None,
        post_processed_image_path: None,
        execution_meta: StyleTransferExecutionMeta {
            requested_mode: requested_mode.as_str().to_string(),
            resolved_mode: StyleTransferMode::Analysis.as_str().to_string(),
            engine: "legacy-analysis".to_string(),
            preset: preset.as_str().to_string(),
            stage: StyleTransferMode::Analysis.stage_label().to_string(),
            expected_wait_range: StyleTransferMode::Analysis.expected_wait_range().to_string(),
        },
    }
}

#[tauri::command]
pub async fn run_style_transfer(
    request: StyleTransferRunRequest,
    app_handle: tauri::AppHandle,
) -> Result<StyleTransferExecutionResponse, String> {
    let requested_mode = resolve_mode(request.mode, &app_handle);
    let preset = resolve_preset(request.preset, &app_handle);
    let analysis_response = style_transfer::analyze_style_transfer(
        request.reference_path,
        request.current_image_path,
        request.current_adjustments,
        request.style_strength,
        request.highlight_guard_strength,
        request.skin_protect_strength,
        request.pure_algorithm,
        request.enable_expert_preset,
        request.enable_feature_mapping,
        request.enable_auto_refine,
        request.enable_lut,
        request.enable_vlm,
        request.llm_endpoint,
        request.llm_api_key,
        request.llm_model,
        app_handle.clone(),
    )
    .await?;

    Ok(wrap_analysis_response(
        analysis_response,
        requested_mode,
        preset,
    ))
}
