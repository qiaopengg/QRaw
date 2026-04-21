use std::time::Duration;

use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::Emitter;
use uuid::Uuid;

use crate::file_management::load_settings;
use crate::style_transfer::{
    self, StyleTransferDebugInfo, StyleTransferResponse, StyleTransferSuggestion,
};

const DEFAULT_STYLE_TRANSFER_SERVICE_URL: &str = "http://127.0.0.1:7860";
const STYLE_TRANSFER_HEALTH_PATH: &str = "/health";
const STYLE_TRANSFER_RUN_PATH: &str = "/v1/style-transfer";

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum StyleTransferMode {
    Analysis,
    Generative,
}

impl StyleTransferMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Analysis => "analysis",
            Self::Generative => "generative",
        }
    }

    fn from_settings(value: Option<&str>) -> Self {
        match value.unwrap_or("analysis") {
            "generative" => Self::Generative,
            _ => Self::Analysis,
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
    pub execution_meta: StyleTransferExecutionMeta,
    pub service_status: StyleTransferServiceStatus,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PythonStyleTransferRequest {
    reference_image_path: String,
    content_image_path: String,
    current_adjustments: Value,
    preset: String,
    enable_refiner: bool,
    tile_size: u32,
    tile_overlap: u32,
    controlnet_strength: f32,
    controlnet_guidance_end: f32,
    denoise_strength: f32,
    steps: u32,
    cfg_scale: f32,
    output_format: String,
    preserve_raw_tone_curve: bool,
    task_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PythonStyleTransferResponse {
    status: Option<String>,
    message: Option<String>,
    output_image_path: Option<String>,
    preview_image_path: Option<String>,
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
    settings.style_transfer_allow_fallback.unwrap_or(true)
}

fn preset_config(preset: StyleTransferPreset) -> StyleTransferPresetConfig {
    match preset {
        StyleTransferPreset::Realistic => StyleTransferPresetConfig {
            controlnet_strength: 0.8,
            denoise_strength: 0.35,
            steps: 36,
            cfg_scale: 5.5,
        },
        StyleTransferPreset::Artistic => StyleTransferPresetConfig {
            controlnet_strength: 0.55,
            denoise_strength: 0.55,
            steps: 40,
            cfg_scale: 6.0,
        },
        StyleTransferPreset::Creative => StyleTransferPresetConfig {
            controlnet_strength: 0.3,
            denoise_strength: 0.72,
            steps: 48,
            cfg_scale: 6.5,
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
            reachable: false,
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
        execution_meta: StyleTransferExecutionMeta {
            requested_mode: requested_mode.as_str().to_string(),
            resolved_mode: StyleTransferMode::Analysis.as_str().to_string(),
            engine: "legacy-analysis".to_string(),
            preset: preset.as_str().to_string(),
            refine_enabled: enable_refiner,
            used_fallback,
        },
        service_status,
    }
}

async fn invoke_python_style_transfer(
    request: &StyleTransferRunRequest,
    service_url: &str,
    preset: StyleTransferPreset,
    enable_refiner: bool,
    app_handle: &tauri::AppHandle,
) -> Result<StyleTransferExecutionResponse, String> {
    let config = preset_config(preset);
    
    // 🆕 生成唯一任务ID
    let task_id = Uuid::new_v4().to_string();
    
    let payload = PythonStyleTransferRequest {
        reference_image_path: request.reference_path.clone(),
        content_image_path: request.current_image_path.clone(),
        current_adjustments: request.current_adjustments.clone(),
        preset: preset.as_str().to_string(),
        enable_refiner,
        tile_size: 1024,
        tile_overlap: 96,
        controlnet_strength: config.controlnet_strength,
        controlnet_guidance_end: 0.8,
        denoise_strength: config.denoise_strength,
        steps: config.steps,
        cfg_scale: config.cfg_scale,
        output_format: "rgb16".to_string(),
        preserve_raw_tone_curve: true,
        task_id: Some(task_id.clone()),
    };

    // 🆕 启动 SSE 监听任务
    let sse_url = format!("{}/v1/style-transfer/progress/{}", service_url, task_id);
    let app_handle_clone = app_handle.clone();
    
    tokio::spawn(async move {
        if let Err(e) = listen_sse_progress(&sse_url, &app_handle_clone).await {
            eprintln!("[SSE] Progress stream error: {}", e);
        }
    });

    let client = Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(3600))
        .build()
        .map_err(|err| format!("无法创建风格迁移服务客户端: {}", err))?;

    let response = client
        .post(format!("{}{}", service_url, STYLE_TRANSFER_RUN_PATH))
        .json(&payload)
        .send()
        .await
        .map_err(|err| format!("调用 Python 风格迁移服务失败: {}", err))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Python 风格迁移服务返回错误 {}: {}", status, body));
    }

    let payload = response
        .json::<PythonStyleTransferResponse>()
        .await
        .map_err(|err| format!("解析风格迁移服务响应失败: {}", err))?;

    Ok(StyleTransferExecutionResponse {
        understanding: payload.message.unwrap_or_else(|| {
            "生成式风格迁移已完成。结果为 16-bit RGB 输出，后续可继续并入 RAW 调色链路。"
                .to_string()
        }),
        adjustments: Vec::new(),
        style_debug: None,
        output_image_path: payload.output_image_path,
        preview_image_path: payload.preview_image_path,
        execution_meta: StyleTransferExecutionMeta {
            requested_mode: StyleTransferMode::Generative.as_str().to_string(),
            resolved_mode: StyleTransferMode::Generative.as_str().to_string(),
            engine: "python-service".to_string(),
            preset: preset.as_str().to_string(),
            refine_enabled: enable_refiner,
            used_fallback: false,
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
    })
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
                    // 检查是否是完成信号
                    if progress_data.get("type").and_then(|v| v.as_str()) == Some("done") {
                        // 任务完成，退出循环
                        return Ok(());
                    }
                    
                    // 提取进度消息
                    if let Some(message) = progress_data.get("message").and_then(|v| v.as_str()) {
                        emit_style_transfer_status(app_handle, message);
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
    let allow_fallback = resolve_allow_fallback(request.allow_fallback_to_analysis, &app_handle);
    let service_url = resolve_service_url(request.service_url.as_deref(), &app_handle);

    if requested_mode == StyleTransferMode::Generative {
        emit_style_transfer_status(
            &app_handle,
            format!(
                "\n连接 Python 风格迁移服务...\nPreset: {} · Refiner: {}\n",
                preset.label(),
                if enable_refiner { "On" } else { "Off" }
            ),
        );

        let mut service_status = fetch_service_status(&service_url).await;
        if service_status.reachable && !service_status.ready {
            for attempt in 1..=120 {
                emit_style_transfer_status(
                    &app_handle,
                    format!(
                        "\nPython 服务正在加载模型，请稍候... ({}/120)\n状态: {}{}\n",
                        attempt,
                        service_status.status,
                        service_status
                            .detail
                            .as_ref()
                            .map(|detail| format!(" · {}", detail))
                            .unwrap_or_default()
                    ),
                );
                tokio::time::sleep(Duration::from_secs(5)).await;
                service_status = fetch_service_status(&service_url).await;
                if service_status.reachable && service_status.ready {
                    break;
                }
            }
        }
        if service_status.reachable && service_status.ready {
            emit_style_transfer_status(
                &app_handle,
                "\n服务已就绪，开始执行 SDXL / IP-Adapter / ControlNet 风格迁移...\n",
            );

            match invoke_python_style_transfer(&request, &service_url, preset, enable_refiner, &app_handle).await
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
