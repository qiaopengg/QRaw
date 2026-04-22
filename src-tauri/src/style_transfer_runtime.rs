use std::time::Duration;

use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::{Emitter, Manager};
use uuid::Uuid;

use crate::file_management::load_settings;
use crate::style_transfer::{
    self, StyleTransferDebugInfo, StyleTransferResponse, StyleTransferSuggestion,
};
use crate::{ActiveStyleTransferTask, AppState};

const DEFAULT_STYLE_TRANSFER_SERVICE_URL: &str = "http://127.0.0.1:7860";
const STYLE_TRANSFER_HEALTH_PATH: &str = "/health";
const STYLE_TRANSFER_RUN_PATH: &str = "/v1/style-transfer";
const STYLE_TRANSFER_CANCEL_PATH: &str = "/v1/style-transfer/cancel";
const STYLE_TRANSFER_MAX_READY_ATTEMPTS: u32 = 60;
const STYLE_TRANSFER_READY_WAIT_MS: u64 = 2_000;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum StyleTransferMode {
    Analysis,
    GenerativePreview,
    GenerativeExport,
}

impl StyleTransferMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Analysis => "analysis",
            Self::GenerativePreview => "generativePreview",
            Self::GenerativeExport => "generativeExport",
        }
    }

    fn from_settings(value: Option<&str>) -> Self {
        match value.unwrap_or("analysis") {
            "generative" | "generativePreview" => Self::GenerativePreview,
            "generativeExport" => Self::GenerativeExport,
            _ => Self::Analysis,
        }
    }

    fn is_generative(self) -> bool {
        matches!(self, Self::GenerativePreview | Self::GenerativeExport)
    }

    fn stage_label(self) -> &'static str {
        match self {
            Self::Analysis => "analysis",
            Self::GenerativePreview => "preview",
            Self::GenerativeExport => "export",
        }
    }

    fn expected_wait_range(self) -> &'static str {
        match self {
            Self::Analysis => "1-5s",
            Self::GenerativePreview => "10-30s",
            Self::GenerativeExport => "30-120s",
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum StyleTransferOutputFormat {
    Tiff,
    Png,
    Jpg,
}

impl StyleTransferOutputFormat {
    fn as_str(self) -> &'static str {
        match self {
            Self::Tiff => "tiff",
            Self::Png => "png",
            Self::Jpg => "jpg",
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

    fn label(self) -> &'static str {
        match self {
            Self::Realistic => "Realistic",
            Self::Artistic => "Artistic",
            Self::Creative => "Creative",
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
    pub service_url: Option<String>,
    pub enable_refiner: Option<bool>,
    pub allow_fallback_to_analysis: Option<bool>,
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
    pub output_format: Option<StyleTransferOutputFormat>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StyleTransferServiceStatus {
    pub service_url: String,
    pub reachable: bool,
    pub ready: bool,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pipeline: Option<String>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StyleTransferExecutionMeta {
    pub requested_mode: String,
    pub resolved_mode: String,
    pub engine: String,
    pub preset: String,
    pub refine_enabled: bool,
    pub used_fallback: bool,
    pub stage: String,
    pub expected_wait_range: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_format: Option<String>,
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
    pub service_status: StyleTransferServiceStatus,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PythonStyleTransferRequest {
    reference_image_path: String,
    content_image_path: String,
    current_adjustments: Value,
    stage: String,
    preset: String,
    enable_refiner: bool,
    tile_size: u32,
    tile_overlap: u32,
    allow_tiling: bool,
    preview_max_side: Option<u32>,
    controlnet_strength: f32,
    controlnet_guidance_end: f32,
    denoise_strength: f32,
    steps: u32,
    cfg_scale: f32,
    export_format: String,
    preserve_raw_tone_curve: bool,
    enable_color_alignment: bool,
    color_alignment_mode: String,
    luminance_strength: f32,
    tone_curve_strength: f32,
    dynamic_range_preserve: f32,
    enable_raw_fusion: bool,
    raw_blend_strength: f32,
    raw_blend_mode: String,
    preserve_highlights: bool,
    preserve_shadows: bool,
    task_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PythonStyleTransferResponse {
    status: Option<String>,
    message: Option<String>,
    output_image_path: Option<String>,
    preview_image_path: Option<String>,
    pure_generation_image_path: Option<String>,
    post_processed_image_path: Option<String>,
    used_fallback: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PythonStyleTransferHealthResponse {
    status: Option<String>,
    ready: Option<bool>,
    version: Option<String>,
    pipeline: Option<String>,
    capabilities: Option<Vec<String>>,
    detail: Option<String>,
}

#[derive(Clone, Copy)]
struct StyleTransferPresetConfig {
    controlnet_strength: f32,
    denoise_strength: f32,
    steps: u32,
    cfg_scale: f32,
    preview_max_side: Option<u32>,
    allow_tiling: bool,
}

#[derive(Clone, Copy)]
struct StyleTransferQualityConfig {
    preserve_raw_tone_curve: bool,
    enable_color_alignment: bool,
    color_alignment_mode: &'static str,
    luminance_strength: f32,
    tone_curve_strength: f32,
    dynamic_range_preserve: f32,
    enable_raw_fusion: bool,
    raw_blend_strength: f32,
}

fn default_service_url() -> String {
    DEFAULT_STYLE_TRANSFER_SERVICE_URL.to_string()
}

fn normalize_service_url(raw: &str) -> String {
    let trimmed = raw.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        default_service_url()
    } else if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("http://{}", trimmed)
    }
}

fn resolve_service_url(
    explicit: Option<&str>,
    app_handle: &tauri::AppHandle,
) -> String {
    if let Some(url) = explicit {
        return normalize_service_url(url);
    }

    let settings = load_settings(app_handle.clone()).unwrap_or_default();
    normalize_service_url(
        settings
            .style_transfer_service_url
            .as_deref()
            .unwrap_or(DEFAULT_STYLE_TRANSFER_SERVICE_URL),
    )
}

fn resolve_mode(
    explicit: Option<StyleTransferMode>,
    app_handle: &tauri::AppHandle,
) -> StyleTransferMode {
    if let Some(mode) = explicit {
        return mode;
    }

    let settings = load_settings(app_handle.clone()).unwrap_or_default();
    StyleTransferMode::from_settings(settings.style_transfer_mode.as_deref())
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

fn resolve_refiner(
    explicit: Option<bool>,
    app_handle: &tauri::AppHandle,
) -> bool {
    if let Some(enable_refiner) = explicit {
        return enable_refiner;
    }

    let settings = load_settings(app_handle.clone()).unwrap_or_default();
    settings.style_transfer_enable_refiner.unwrap_or(false)
}

fn resolve_allow_fallback(
    explicit: Option<bool>,
    app_handle: &tauri::AppHandle,
) -> bool {
    if let Some(allow_fallback) = explicit {
        return allow_fallback;
    }

    let settings = load_settings(app_handle.clone()).unwrap_or_default();
    settings.style_transfer_allow_fallback.unwrap_or(false)
}

fn resolve_output_format(
    explicit: Option<StyleTransferOutputFormat>,
    mode: StyleTransferMode,
) -> StyleTransferOutputFormat {
    explicit.unwrap_or(match mode {
        StyleTransferMode::GenerativeExport => StyleTransferOutputFormat::Tiff,
        _ => StyleTransferOutputFormat::Png,
    })
}

fn preset_config(preset: StyleTransferPreset, mode: StyleTransferMode) -> StyleTransferPresetConfig {
    match mode {
        StyleTransferMode::GenerativeExport => match preset {
            StyleTransferPreset::Realistic => StyleTransferPresetConfig {
                controlnet_strength: 0.68,
                denoise_strength: 0.28,
                steps: 24,
                cfg_scale: 4.8,
                preview_max_side: None,
                allow_tiling: true,
            },
            StyleTransferPreset::Artistic => StyleTransferPresetConfig {
                controlnet_strength: 0.58,
                denoise_strength: 0.38,
                steps: 28,
                cfg_scale: 5.6,
                preview_max_side: None,
                allow_tiling: true,
            },
            StyleTransferPreset::Creative => StyleTransferPresetConfig {
                controlnet_strength: 0.52,
                denoise_strength: 0.44,
                steps: 30,
                cfg_scale: 5.8,
                preview_max_side: None,
                allow_tiling: true,
            },
        },
        StyleTransferMode::GenerativePreview => match preset {
            StyleTransferPreset::Realistic => StyleTransferPresetConfig {
                controlnet_strength: 0.68,
                denoise_strength: 0.26,
                steps: 18,
                cfg_scale: 4.8,
                preview_max_side: Some(1280),
                allow_tiling: false,
            },
            StyleTransferPreset::Artistic => StyleTransferPresetConfig {
                controlnet_strength: 0.58,
                denoise_strength: 0.36,
                steps: 24,
                cfg_scale: 5.4,
                preview_max_side: Some(1280),
                allow_tiling: false,
            },
            StyleTransferPreset::Creative => StyleTransferPresetConfig {
                controlnet_strength: 0.52,
                denoise_strength: 0.42,
                steps: 26,
                cfg_scale: 5.6,
                preview_max_side: Some(1280),
                allow_tiling: false,
            },
        },
        StyleTransferMode::Analysis => StyleTransferPresetConfig {
            controlnet_strength: 0.0,
            denoise_strength: 0.0,
            steps: 0,
            cfg_scale: 0.0,
            preview_max_side: None,
            allow_tiling: false,
        },
    }
}

fn quality_config(
    preset: StyleTransferPreset,
    mode: StyleTransferMode,
) -> StyleTransferQualityConfig {
    match mode {
        StyleTransferMode::GenerativeExport => match preset {
            StyleTransferPreset::Realistic => StyleTransferQualityConfig {
                preserve_raw_tone_curve: true,
                enable_color_alignment: true,
                color_alignment_mode: "full",
                luminance_strength: 0.42,
                tone_curve_strength: 0.34,
                dynamic_range_preserve: 0.42,
                enable_raw_fusion: true,
                raw_blend_strength: 0.58,
            },
            StyleTransferPreset::Artistic => StyleTransferQualityConfig {
                preserve_raw_tone_curve: true,
                enable_color_alignment: true,
                color_alignment_mode: "full",
                luminance_strength: 0.48,
                tone_curve_strength: 0.40,
                dynamic_range_preserve: 0.34,
                enable_raw_fusion: true,
                raw_blend_strength: 0.50,
            },
            StyleTransferPreset::Creative => StyleTransferQualityConfig {
                preserve_raw_tone_curve: true,
                enable_color_alignment: true,
                color_alignment_mode: "full",
                luminance_strength: 0.44,
                tone_curve_strength: 0.38,
                dynamic_range_preserve: 0.30,
                enable_raw_fusion: true,
                raw_blend_strength: 0.42,
            },
        },
        StyleTransferMode::GenerativePreview => match preset {
            StyleTransferPreset::Realistic => StyleTransferQualityConfig {
                preserve_raw_tone_curve: false,
                enable_color_alignment: true,
                color_alignment_mode: "full",
                luminance_strength: 0.30,
                tone_curve_strength: 0.22,
                dynamic_range_preserve: 0.28,
                enable_raw_fusion: false,
                raw_blend_strength: 0.0,
            },
            StyleTransferPreset::Artistic => StyleTransferQualityConfig {
                preserve_raw_tone_curve: false,
                enable_color_alignment: true,
                color_alignment_mode: "full",
                luminance_strength: 0.36,
                tone_curve_strength: 0.26,
                dynamic_range_preserve: 0.22,
                enable_raw_fusion: false,
                raw_blend_strength: 0.0,
            },
            StyleTransferPreset::Creative => StyleTransferQualityConfig {
                preserve_raw_tone_curve: false,
                enable_color_alignment: true,
                color_alignment_mode: "full",
                luminance_strength: 0.34,
                tone_curve_strength: 0.24,
                dynamic_range_preserve: 0.20,
                enable_raw_fusion: false,
                raw_blend_strength: 0.0,
            },
        },
        StyleTransferMode::Analysis => StyleTransferQualityConfig {
            preserve_raw_tone_curve: false,
            enable_color_alignment: false,
            color_alignment_mode: "none",
            luminance_strength: 0.0,
            tone_curve_strength: 0.0,
            dynamic_range_preserve: 0.0,
            enable_raw_fusion: false,
            raw_blend_strength: 0.0,
        },
    }
}

fn emit_style_transfer_status(app_handle: &tauri::AppHandle, text: impl Into<String>) {
    let _ = app_handle.emit(
        "style-transfer-stream",
        json!({
            "chunk_type": "thinking",
            "text": text.into(),
            "result": Value::Null
        }),
    );
}

fn emit_style_transfer_done(
    app_handle: &tauri::AppHandle,
    response: &StyleTransferExecutionResponse,
) {
    let _ = app_handle.emit(
        "style-transfer-stream",
        json!({
            "chunk_type": "done",
            "text": "",
            "result": response
        }),
    );
}

async fn fetch_service_status(service_url: &str) -> StyleTransferServiceStatus {
    let health_url = format!("{}{}", service_url, STYLE_TRANSFER_HEALTH_PATH);
    let client = match Client::builder().timeout(Duration::from_secs(3)).build() {
        Ok(client) => client,
        Err(err) => {
            return StyleTransferServiceStatus {
                service_url: service_url.to_string(),
                reachable: false,
                ready: false,
                status: "client_error".to_string(),
                version: None,
                pipeline: None,
                capabilities: Vec::new(),
                detail: Some(err.to_string()),
            };
        }
    };

    let response = match client.get(&health_url).send().await {
        Ok(response) => response,
        Err(err) => {
            return StyleTransferServiceStatus {
                service_url: service_url.to_string(),
                reachable: false,
                ready: false,
                status: "offline".to_string(),
                version: None,
                pipeline: None,
                capabilities: Vec::new(),
                detail: Some(err.to_string()),
            };
        }
    };

    if !response.status().is_success() {
        return StyleTransferServiceStatus {
            service_url: service_url.to_string(),
            reachable: true,
            ready: false,
            status: format!("http_{}", response.status().as_u16()),
            version: None,
            pipeline: None,
            capabilities: Vec::new(),
            detail: response.text().await.ok(),
        };
    }

    match response.json::<PythonStyleTransferHealthResponse>().await {
        Ok(payload) => StyleTransferServiceStatus {
            service_url: service_url.to_string(),
            reachable: true,
            ready: payload.ready.unwrap_or(false),
            status: payload.status.unwrap_or_else(|| "ok".to_string()),
            version: payload.version,
            pipeline: payload.pipeline,
            capabilities: payload.capabilities.unwrap_or_default(),
            detail: payload.detail,
        },
        Err(err) => StyleTransferServiceStatus {
            service_url: service_url.to_string(),
            reachable: true,
            ready: false,
            status: "invalid_health_payload".to_string(),
            version: None,
            pipeline: None,
            capabilities: Vec::new(),
            detail: Some(format!("Health payload parse warning: {}", err)),
        },
    }
}

fn skipped_service_status(service_url: String, detail: &str) -> StyleTransferServiceStatus {
    StyleTransferServiceStatus {
        service_url,
        reachable: false,
        ready: false,
        status: "skipped".to_string(),
        version: None,
        pipeline: None,
        capabilities: Vec::new(),
        detail: Some(detail.to_string()),
    }
}

fn wrap_analysis_response(
    analysis_response: StyleTransferResponse,
    requested_mode: StyleTransferMode,
    preset: StyleTransferPreset,
    enable_refiner: bool,
    used_fallback: bool,
    service_status: StyleTransferServiceStatus,
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
            refine_enabled: enable_refiner,
            used_fallback,
            stage: StyleTransferMode::Analysis.stage_label().to_string(),
            expected_wait_range: StyleTransferMode::Analysis.expected_wait_range().to_string(),
            output_format: None,
        },
        service_status,
    }
}

async fn invoke_python_style_transfer(
    request: &StyleTransferRunRequest,
    service_url: &str,
    mode: StyleTransferMode,
    preset: StyleTransferPreset,
    enable_refiner: bool,
    app_handle: &tauri::AppHandle,
) -> Result<StyleTransferExecutionResponse, String> {
    let config = preset_config(preset, mode);
    let quality = quality_config(preset, mode);
    let output_format = resolve_output_format(request.output_format, mode);
    let resolved_refiner = enable_refiner && mode == StyleTransferMode::GenerativeExport;
    
    // 🆕 生成唯一任务ID
    let task_id = Uuid::new_v4().to_string();
    
    let payload = PythonStyleTransferRequest {
        reference_image_path: request.reference_path.clone(),
        content_image_path: request.current_image_path.clone(),
        current_adjustments: request.current_adjustments.clone(),
        stage: mode.stage_label().to_string(),
        preset: preset.as_str().to_string(),
        enable_refiner: resolved_refiner,
        tile_size: 1024,
        tile_overlap: 96,
        allow_tiling: config.allow_tiling,
        preview_max_side: config.preview_max_side,
        controlnet_strength: config.controlnet_strength,
        controlnet_guidance_end: 0.8,
        denoise_strength: config.denoise_strength,
        steps: config.steps,
        cfg_scale: config.cfg_scale,
        export_format: output_format.as_str().to_string(),
        preserve_raw_tone_curve: quality.preserve_raw_tone_curve,
        enable_color_alignment: quality.enable_color_alignment,
        color_alignment_mode: quality.color_alignment_mode.to_string(),
        luminance_strength: quality.luminance_strength,
        tone_curve_strength: quality.tone_curve_strength,
        dynamic_range_preserve: quality.dynamic_range_preserve,
        enable_raw_fusion: quality.enable_raw_fusion,
        raw_blend_strength: quality.raw_blend_strength,
        raw_blend_mode: "luminance".to_string(),
        preserve_highlights: true,
        preserve_shadows: true,
        task_id: Some(task_id.clone()),
    };

    // 🆕 启动 SSE 监听任务
    let sse_url = format!("{}/v1/style-transfer/progress/{}", service_url, task_id);
    let app_handle_clone = app_handle.clone();
    {
        let state = app_handle.state::<AppState>();
        *state.style_transfer_task.lock().unwrap() = Some(ActiveStyleTransferTask {
            task_id: task_id.clone(),
            service_url: service_url.to_string(),
        });
    }
    
    let sse_task = tokio::spawn(async move {
        if let Err(e) = listen_sse_progress(&sse_url, &app_handle_clone).await {
            eprintln!("[SSE] Progress stream error: {}", e);
        }
    });

    let client = Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(3600))
        .build()
        .map_err(|err| format!("无法创建风格迁移服务客户端: {}", err))?;

    let response = match client
        .post(format!("{}{}", service_url, STYLE_TRANSFER_RUN_PATH))
        .json(&payload)
        .send()
        .await
    {
        Ok(response) => response,
        Err(err) => {
            sse_task.abort();
            *app_handle
                .state::<AppState>()
                .style_transfer_task
                .lock()
                .unwrap() = None;
            return Err(format!("调用 Python 风格迁移服务失败: {}", err));
        }
    };

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        sse_task.abort();
        *app_handle
            .state::<AppState>()
            .style_transfer_task
            .lock()
            .unwrap() = None;
        return Err(format!("Python 风格迁移服务返回错误 {}: {}", status, body));
    }

    let payload = match response.json::<PythonStyleTransferResponse>().await {
        Ok(payload) => payload,
        Err(err) => {
            sse_task.abort();
            *app_handle
                .state::<AppState>()
                .style_transfer_task
                .lock()
                .unwrap() = None;
            return Err(format!("解析风格迁移服务响应失败: {}", err));
        }
    };

    let response = StyleTransferExecutionResponse {
        understanding: payload.message.unwrap_or_else(|| {
            match mode {
                StyleTransferMode::GenerativePreview => {
                    "生成式风格预览已完成。当前结果为预览图，可继续确认高质量导出。".to_string()
                }
                StyleTransferMode::GenerativeExport => {
                    "生成式高质量导出已完成。当前结果为 16-bit RGB 衍生图，可继续进入现有导出工作流。"
                        .to_string()
                }
                StyleTransferMode::Analysis => {
                    "风格迁移已完成。".to_string()
                }
            }
        }),
        adjustments: Vec::new(),
        style_debug: None,
        output_image_path: payload.output_image_path,
        preview_image_path: payload.preview_image_path,
        pure_generation_image_path: payload.pure_generation_image_path,
        post_processed_image_path: payload.post_processed_image_path,
        execution_meta: StyleTransferExecutionMeta {
            requested_mode: mode.as_str().to_string(),
            resolved_mode: mode.as_str().to_string(),
            engine: "python-service".to_string(),
            preset: preset.as_str().to_string(),
            refine_enabled: resolved_refiner,
            used_fallback: payload.used_fallback.unwrap_or(false),
            stage: mode.stage_label().to_string(),
            expected_wait_range: mode.expected_wait_range().to_string(),
            output_format: Some(output_format.as_str().to_string()),
        },
        service_status: StyleTransferServiceStatus {
            service_url: service_url.to_string(),
            reachable: true,
            ready: payload.status.as_deref() != Some("warming_up"),
            status: payload.status.unwrap_or_else(|| "completed".to_string()),
            version: None,
            pipeline: Some("sdxl-ip-adapter-controlnet".to_string()),
            capabilities: vec![
                "img2img".to_string(),
                "ip_adapter".to_string(),
                "controlnet".to_string(),
                "tiled_vae".to_string(),
            ],
            detail: None,
        },
    };
    *app_handle
        .state::<AppState>()
        .style_transfer_task
        .lock()
        .unwrap() = None;
    sse_task.abort();
    Ok(response)
}

#[tauri::command]
pub async fn cancel_style_transfer(app_handle: tauri::AppHandle) -> Result<(), String> {
    let active_task = {
        let state = app_handle.state::<AppState>();
        state.style_transfer_task.lock().unwrap().clone()
    };

    let Some(active_task) = active_task else {
        return Err("No style transfer task is currently running.".to_string());
    };

    let client = Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|err| format!("无法创建风格迁移取消客户端: {}", err))?;

    let response = client
        .post(format!(
            "{}{}/{}",
            active_task.service_url, STYLE_TRANSFER_CANCEL_PATH, active_task.task_id
        ))
        .send()
        .await
        .map_err(|err| format!("调用风格迁移取消接口失败: {}", err))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("风格迁移取消失败 {}: {}", status, body));
    }

    *app_handle
        .state::<AppState>()
        .style_transfer_task
        .lock()
        .unwrap() = None;
    let _ = app_handle.emit(
        "style-transfer-stream",
        json!({
            "chunk_type": "error",
            "text": "style_transfer_cancelled",
            "result": Value::Null
        }),
    );
    Ok(())
}

/// 🆕 监听 SSE 进度流
async fn listen_sse_progress(
    sse_url: &str,
    app_handle: &tauri::AppHandle,
) -> Result<(), String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(3600))
        .build()
        .map_err(|e| format!("Failed to create SSE client: {}", e))?;

    let response = client
        .get(sse_url)
        .send()
        .await
        .map_err(|e| format!("Failed to connect to SSE endpoint: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("SSE endpoint returned status: {}", response.status()));
    }

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Stream error: {}", e))?;
        let text = String::from_utf8_lossy(&chunk);
        buffer.push_str(&text);

        // 处理 SSE 消息（按行分割）
        while let Some(line_end) = buffer.find('\n') {
            let line = buffer[..line_end].trim().to_string();
            buffer = buffer[line_end + 1..].to_string();

            // 解析 SSE 数据行
            if line.starts_with("data:") {
                let json_str = line[5..].trim();
                
                // 解析 JSON 数据
                if let Ok(progress_data) = serde_json::from_str::<Value>(json_str) {
                    // 提取进度消息
                    if let Some(message) = progress_data.get("message").and_then(|v| v.as_str()) {
                        emit_style_transfer_status(app_handle, message);
                    }

                    if matches!(
                        progress_data.get("type").and_then(|v| v.as_str()),
                        Some("done" | "cancelled" | "error")
                    ) {
                        return Ok(());
                    }
                }
            } else if line.starts_with(":") {
                // 心跳消息，忽略
                continue;
            }
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn check_style_transfer_service(
    service_url: Option<String>,
    app_handle: tauri::AppHandle,
) -> Result<StyleTransferServiceStatus, String> {
    let resolved_url = resolve_service_url(service_url.as_deref(), &app_handle);
    Ok(fetch_service_status(&resolved_url).await)
}

#[tauri::command]
pub async fn run_style_transfer(
    request: StyleTransferRunRequest,
    app_handle: tauri::AppHandle,
) -> Result<StyleTransferExecutionResponse, String> {
    let requested_mode = resolve_mode(request.mode, &app_handle);
    let preset = resolve_preset(request.preset, &app_handle);
    let enable_refiner = resolve_refiner(request.enable_refiner, &app_handle);
    let effective_refiner = enable_refiner && requested_mode == StyleTransferMode::GenerativeExport;
    let allow_fallback = resolve_allow_fallback(request.allow_fallback_to_analysis, &app_handle);
    let service_url = resolve_service_url(request.service_url.as_deref(), &app_handle);

    if requested_mode.is_generative() {
        emit_style_transfer_status(
            &app_handle,
            format!(
                "\n已选择 {} 路径，预计耗时 {}。\n连接 Python 风格迁移服务...\nPreset: {} · Refiner: {}\n",
                if requested_mode == StyleTransferMode::GenerativePreview {
                    "生成式预览"
                } else {
                    "高质量导出"
                },
                requested_mode.expected_wait_range(),
                preset.label(),
                if effective_refiner { "On" } else { "Off" }
            ),
        );

        let mut service_status = fetch_service_status(&service_url).await;
        if service_status.reachable && !service_status.ready {
            for attempt in 1..=STYLE_TRANSFER_MAX_READY_ATTEMPTS {
                emit_style_transfer_status(
                    &app_handle,
                    format!(
                        "\nPython 服务正在加载模型，请稍候... ({}/{})\n状态: {}{}\n",
                        attempt,
                        STYLE_TRANSFER_MAX_READY_ATTEMPTS,
                        service_status.status,
                        service_status
                            .detail
                            .as_ref()
                            .map(|detail| format!(" · {}", detail))
                            .unwrap_or_default()
                    ),
                );
                tokio::time::sleep(Duration::from_millis(STYLE_TRANSFER_READY_WAIT_MS)).await;
                service_status = fetch_service_status(&service_url).await;
                if service_status.reachable && service_status.ready {
                    break;
                }
            }
        }
        if service_status.reachable && service_status.ready {
            emit_style_transfer_status(
                &app_handle,
                if requested_mode == StyleTransferMode::GenerativePreview {
                    "\n服务已就绪，开始执行生成式预览。\n阶段：输入校验 -> 预览准备 -> 模型推理 -> 结果校验 -> 预览输出\n"
                } else {
                    "\n服务已就绪，开始执行高质量导出。\n阶段：输入校验 -> 全尺寸准备 -> 分块推理 -> 结果校验 -> 正式导出\n"
                },
            );

            match invoke_python_style_transfer(
                &request,
                &service_url,
                requested_mode,
                preset,
                enable_refiner,
                &app_handle,
            )
            .await
            {
                Ok(response) => {
                    emit_style_transfer_done(&app_handle, &response);
                    return Ok(response);
                }
                Err(err) if !allow_fallback => {
                    let _ = app_handle.emit(
                        "style-transfer-stream",
                        json!({
                            "chunk_type": "error",
                            "text": err,
                            "result": Value::Null
                        }),
                    );
                    return Err(err);
                }
                Err(err) => {
                    emit_style_transfer_status(
                        &app_handle,
                        format!(
                            "\n生成式服务暂不可用，已回退到本地参数风格匹配。\n原因: {}\n",
                            err
                        ),
                    );

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

                    return Ok(wrap_analysis_response(
                        analysis_response,
                        requested_mode,
                        preset,
                        enable_refiner,
                        true,
                        service_status,
                    ));
                }
            }
        }

        if !allow_fallback {
            let detail = service_status
                .detail
                .clone()
                .unwrap_or_else(|| "服务未启动或未就绪".to_string());
            return Err(format!(
                "Python 风格迁移服务不可用: {} ({})",
                service_status.status, detail
            ));
        }

        emit_style_transfer_status(
            &app_handle,
            format!(
                "\nPython 风格迁移服务未就绪，回退到本地参数风格匹配。\n状态: {}{}\n",
                service_status.status,
                service_status
                    .detail
                    .as_ref()
                    .map(|detail| format!(" · {}", detail))
                    .unwrap_or_default()
            ),
        );

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

        return Ok(wrap_analysis_response(
            analysis_response,
            requested_mode,
            preset,
            enable_refiner,
            true,
            service_status,
        ));
    }

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
        enable_refiner,
        false,
        skipped_service_status(service_url, "Skipped in analysis mode"),
    ))
}
