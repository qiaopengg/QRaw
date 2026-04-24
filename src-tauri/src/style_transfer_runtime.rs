use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::file_management::load_settings;
use crate::style_transfer::{
    self, StyleTransferDebugInfo, StyleTransferResponse, StyleTransferSuggestion,
};
use crate::style_transfer_models::{self, StyleTransferModelStatusResponse};

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
pub enum StyleTransferStrategyMode {
    Safe,
    Strong,
}

impl StyleTransferStrategyMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Safe => "safe",
            Self::Strong => "strong",
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
    #[serde(default)]
    pub reference_path: Option<String>,
    #[serde(default)]
    pub main_reference_path: Option<String>,
    #[serde(default)]
    pub aux_reference_paths: Vec<String>,
    pub current_image_path: String,
    pub current_adjustments: Value,
    pub mode: Option<StyleTransferMode>,
    pub strategy_mode: Option<StyleTransferStrategyMode>,
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
    pub strategy_mode: String,
    pub stage: String,
    pub expected_wait_range: String,
    pub reference_count: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StyleTransferProcessingDebug {
    pub canonical_input_used: bool,
    pub style_backbone_used: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style_backbone_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub semantic_similarity: Option<f64>,
    #[serde(default)]
    pub aux_semantic_similarities: Vec<f64>,
    pub reference_count: usize,
    pub pure_algorithm: bool,
    pub feature_mapping_enabled: bool,
    pub auto_refine_enabled: bool,
    pub expert_preset_enabled: bool,
    pub lut_enabled: bool,
    pub vlm_enabled: bool,
    pub local_region_count: usize,
    pub slider_mapping_count: usize,
    pub risk_warning_count: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StyleTransferExecutionResponse {
    pub understanding: String,
    #[serde(default)]
    pub adjustments: Vec<StyleTransferSuggestion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub global_adjustments: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guarded_global_adjustments: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub curves: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hsl: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub global_lut: Option<Value>,
    #[serde(default)]
    pub local_regions: Vec<Value>,
    #[serde(default)]
    pub guarded_local_regions: Vec<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality_report: Option<Value>,
    #[serde(default)]
    pub risk_warnings: Vec<String>,
    #[serde(default)]
    pub slider_mapping: Vec<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub processing_debug: Option<StyleTransferProcessingDebug>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_status: Option<StyleTransferModelStatusResponse>,
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
    strategy_mode: StyleTransferStrategyMode,
    reference_count: usize,
    request: &StyleTransferRunRequest,
    model_status: Option<StyleTransferModelStatusResponse>,
) -> StyleTransferExecutionResponse {
    let engine = if analysis_response.backbone_used {
        if request.pure_algorithm.unwrap_or(false) {
            "analysis-canonical-vitb-pure"
        } else {
            "analysis-canonical-vitb-enhanced"
        }
    } else if model_status
        .as_ref()
        .map(|status| status.required_ready)
        .unwrap_or(false)
    {
        "analysis-canonical-fallback"
    } else {
        "analysis-legacy-fallback"
    };
    let processing_debug = Some(StyleTransferProcessingDebug {
        canonical_input_used: analysis_response.canonical_input_used,
        style_backbone_used: analysis_response.backbone_used,
        style_backbone_model: analysis_response
            .backbone_used
            .then(|| "style_transfer_dinov2_vitb.onnx".to_string()),
        semantic_similarity: analysis_response.semantic_similarity,
        aux_semantic_similarities: analysis_response.aux_semantic_similarities.clone(),
        reference_count,
        pure_algorithm: request.pure_algorithm.unwrap_or(false),
        feature_mapping_enabled: request.enable_feature_mapping.unwrap_or(true)
            && !request.pure_algorithm.unwrap_or(false),
        auto_refine_enabled: request.enable_auto_refine.unwrap_or(true)
            && !request.pure_algorithm.unwrap_or(false),
        expert_preset_enabled: request.enable_expert_preset.unwrap_or(true)
            && !request.pure_algorithm.unwrap_or(false),
        lut_enabled: request.enable_lut.unwrap_or(true) && !request.pure_algorithm.unwrap_or(false),
        vlm_enabled: request.enable_vlm.unwrap_or(true) && !request.pure_algorithm.unwrap_or(false),
        local_region_count: analysis_response.local_regions.len(),
        slider_mapping_count: analysis_response.slider_mapping.len(),
        risk_warning_count: analysis_response.risk_warnings.len(),
    });
    StyleTransferExecutionResponse {
        understanding: analysis_response.understanding,
        adjustments: analysis_response.adjustments,
        global_adjustments: Some(analysis_response.global_adjustments),
        guarded_global_adjustments: Some(analysis_response.guarded_global_adjustments),
        curves: Some(analysis_response.curves),
        hsl: Some(analysis_response.hsl),
        global_lut: analysis_response.global_lut,
        local_regions: analysis_response
            .local_regions
            .into_iter()
            .map(|region| serde_json::to_value(region).unwrap_or(Value::Null))
            .collect(),
        guarded_local_regions: analysis_response
            .guarded_local_regions
            .into_iter()
            .map(|region| serde_json::to_value(region).unwrap_or(Value::Null))
            .collect(),
        quality_report: analysis_response
            .quality_report
            .and_then(|report| serde_json::to_value(report).ok()),
        risk_warnings: analysis_response.risk_warnings,
        slider_mapping: analysis_response
            .slider_mapping
            .into_iter()
            .map(|entry| serde_json::to_value(entry).unwrap_or(Value::Null))
            .collect(),
        processing_debug,
        style_debug: analysis_response.style_debug,
        output_image_path: None,
        preview_image_path: None,
        pure_generation_image_path: None,
        post_processed_image_path: None,
        model_status,
        execution_meta: StyleTransferExecutionMeta {
            requested_mode: requested_mode.as_str().to_string(),
            resolved_mode: StyleTransferMode::Analysis.as_str().to_string(),
            engine: engine.to_string(),
            preset: preset.as_str().to_string(),
            strategy_mode: strategy_mode.as_str().to_string(),
            stage: StyleTransferMode::Analysis.stage_label().to_string(),
            expected_wait_range: StyleTransferMode::Analysis
                .expected_wait_range()
                .to_string(),
            reference_count,
        },
    }
}

fn resolve_reference_path(request: &StyleTransferRunRequest) -> Result<String, String> {
    request
        .main_reference_path
        .clone()
        .or_else(|| request.reference_path.clone())
        .filter(|path| !path.trim().is_empty())
        .ok_or_else(|| "主参考图不能为空".to_string())
}

fn resolve_strategy_mode(explicit: Option<StyleTransferStrategyMode>) -> StyleTransferStrategyMode {
    explicit.unwrap_or(StyleTransferStrategyMode::Safe)
}

fn adjust_tuning_for_strategy(
    strategy_mode: StyleTransferStrategyMode,
    style_strength: Option<f64>,
    highlight_guard_strength: Option<f64>,
    skin_protect_strength: Option<f64>,
) -> (Option<f64>, Option<f64>, Option<f64>) {
    let scale = |value: Option<f64>, factor: f64| value.map(|v| (v * factor).clamp(0.5, 2.0));
    match strategy_mode {
        StyleTransferStrategyMode::Safe => (
            scale(style_strength, 0.9),
            scale(highlight_guard_strength.or(Some(1.0)), 1.15),
            scale(skin_protect_strength.or(Some(1.0)), 1.15),
        ),
        StyleTransferStrategyMode::Strong => (
            scale(style_strength, 1.1),
            scale(highlight_guard_strength.or(Some(1.0)), 0.95),
            scale(skin_protect_strength.or(Some(1.0)), 0.95),
        ),
    }
}

#[tauri::command]
pub async fn run_style_transfer(
    request: StyleTransferRunRequest,
    app_handle: tauri::AppHandle,
) -> Result<StyleTransferExecutionResponse, String> {
    let request_snapshot = request.clone();
    let requested_mode = resolve_mode(request.mode, &app_handle);
    let preset = resolve_preset(request.preset, &app_handle);
    let strategy_mode = resolve_strategy_mode(request.strategy_mode);
    let reference_path = resolve_reference_path(&request)?;
    let reference_count = 1 + request.aux_reference_paths.len();
    let (style_strength, highlight_guard_strength, skin_protect_strength) =
        adjust_tuning_for_strategy(
            strategy_mode,
            request.style_strength,
            request.highlight_guard_strength,
            request.skin_protect_strength,
        );
    let analysis_response = style_transfer::analyze_style_transfer(
        reference_path,
        request.aux_reference_paths.clone(),
        request.current_image_path,
        request.current_adjustments,
        style_strength,
        highlight_guard_strength,
        skin_protect_strength,
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
    let model_status = style_transfer_models::get_style_transfer_model_status_response().ok();

    Ok(wrap_analysis_response(
        analysis_response,
        requested_mode,
        preset,
        strategy_mode,
        reference_count,
        &request_snapshot,
        model_status,
    ))
}
