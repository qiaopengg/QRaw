use image::{DynamicImage, GenericImageView, Pixel};
use image::codecs::jpeg::JpegEncoder;
use rawler::decoders::RawDecodeParams;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::Cursor;
use std::path::Path;
use tauri::Emitter;
use crate::llm_chat::StreamChunkPayload;

const DEFAULT_STYLE_TRANSFER_MODEL: &str = "qwen2.5vl:7b";
const EARLY_EXIT_STYLE_DISTANCE_THRESHOLD: f64 = 0.22;
const LLM_TRIGGER_STYLE_DISTANCE_THRESHOLD: f64 = 0.68;

/// 智能加载图像：先尝试 image crate（支持 JPEG/PNG 等），失败则用 rawler 解码 RAW 文件
/// 为风格分析优化：优先提取预览图（更快、更省内存），避免全尺寸 RAW 解码
fn smart_open_image(path: &str) -> Result<DynamicImage, String> {
    // 先尝试标准格式
    match image::open(path) {
        Ok(img) => Ok(img),
        Err(_std_err) => {
            // 尝试用 rawler 提取预览图（比 raw_to_srgb 快得多且省内存）
            let params = RawDecodeParams::default();
            rawler::analyze::extract_preview_pixels(path, &params)
                .map_err(|e| format!("无法打开图片（标准格式和 RAW 均失败）: {}", e))
        }
    }
}

fn resolve_style_transfer_model(llm_model: Option<String>) -> String {
    match llm_model {
        Some(model) => {
            let trimmed = model.trim();
            if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("auto") {
                DEFAULT_STYLE_TRANSFER_MODEL.to_string()
            } else {
                trimmed.to_string()
            }
        }
        None => DEFAULT_STYLE_TRANSFER_MODEL.to_string(),
    }
}

fn is_vision_model(model_name: &str) -> bool {
    let name = model_name.to_ascii_lowercase();
    name.contains("vl")
        || name.contains("vision")
        || name.contains("llava")
        || name.contains("minicpm-v")
        || name.contains("internvl")
}

fn encode_image_for_vision_model(img: &DynamicImage) -> Result<String, String> {
    let (w, h) = img.dimensions();
    let resized = if w > 1024 || h > 1024 {
        img.resize(1024, 1024, image::imageops::FilterType::Triangle)
    } else {
        img.clone()
    };
    let mut buffer = Vec::new();
    {
        let mut cursor = Cursor::new(&mut buffer);
        let mut encoder = JpegEncoder::new_with_quality(&mut cursor, 88);
        encoder
            .encode_image(&resized)
            .map_err(|e| format!("编码视觉输入图片失败: {}", e))?;
    }
    Ok(base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        &buffer,
    ))
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StyleFeatures {
    pub mean_luminance: f64,
    pub highlight_ratio: f64,
    pub shadow_ratio: f64,
    pub contrast_spread: f64,
    pub p10_luminance: f64,
    pub p50_luminance: f64,
    pub p90_luminance: f64,
    pub p99_luminance: f64,
    pub clipped_highlight_ratio: f64,
    pub waveform_low_band: f64,
    pub waveform_mid_band: f64,
    pub waveform_high_band: f64,
    pub rb_ratio: f64,
    pub gb_ratio: f64,
    pub mean_saturation: f64,
    pub saturation_spread: f64,
    pub shadow_luminance_mean: f64,
    pub mid_luminance_mean: f64,
    pub highlight_luminance_mean: f64,
    pub skin_ratio: f64,
    pub skin_luminance_mean: f64,
    pub skin_rb_ratio: f64,
    pub laplacian_variance: f64,
    pub vignette_diff: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StyleTransferSuggestion {
    pub key: String,
    pub value: f64,
    pub label: String,
    pub min: f64,
    pub max: f64,
    pub reason: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StyleTransferResponse {
    pub understanding: String,
    pub adjustments: Vec<StyleTransferSuggestion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style_debug: Option<StyleTransferDebugInfo>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StyleTransferErrorBreakdown {
    pub tonal: f64,
    pub color: f64,
    pub skin: f64,
    pub highlight_penalty: f64,
    pub total: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StyleTransferDebugInfo {
    pub before: StyleTransferErrorBreakdown,
    pub after: StyleTransferErrorBreakdown,
    pub proximity_before: StyleProximityScore,
    pub proximity_after: StyleProximityScore,
    pub improvement_ratio: f64,
    pub dominant_error: String,
    pub auto_refine_rounds: u32,
    pub suggested_actions: Vec<StyleTransferDebugAction>,
    pub blocked_reasons: Vec<String>,
    pub blocked_items: Vec<StyleConstraintBlockItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constraint_debug: Option<DynamicConstraintDebugInfo>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StyleTransferDebugAction {
    pub key: String,
    pub label: String,
    pub recommended_delta: f64,
    pub priority: u8,
    pub reason: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StyleProximityScore {
    pub tonal: f64,
    pub color: f64,
    pub skin: f64,
    pub highlight: f64,
    pub overall: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StyleConstraintAction {
    pub key: String,
    pub label: String,
    pub delta: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StyleConstraintBlockItem {
    pub category: String,
    pub label: String,
    pub reason: String,
    pub hit_count: u32,
    pub severity: f64,
    pub actions: Vec<StyleConstraintAction>,
}

#[derive(Debug, Clone, Copy)]
struct StyleTransferTuning {
    style_strength: f64,
    highlight_guard_strength: f64,
    skin_protect_strength: f64,
}

impl StyleTransferTuning {
    fn from_options(
        style_strength: Option<f64>,
        highlight_guard_strength: Option<f64>,
        skin_protect_strength: Option<f64>,
    ) -> Self {
        let clamp = |v: f64, min: f64, max: f64| v.max(min).min(max);
        Self {
            style_strength: clamp(style_strength.unwrap_or(1.0), 0.5, 2.0),
            highlight_guard_strength: clamp(highlight_guard_strength.unwrap_or(1.0), 0.5, 2.0),
            skin_protect_strength: clamp(skin_protect_strength.unwrap_or(1.0), 0.5, 2.0),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DynamicConstraintBand {
    pub hard_min: f64,
    pub hard_max: f64,
    pub soft_min: f64,
    pub soft_max: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DynamicConstraintWindow {
    pub source: String,
    pub highlight_risk: f64,
    pub shadow_risk: f64,
    pub saturation_risk: f64,
    pub bands: HashMap<String, DynamicConstraintBand>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DynamicConstraintClampRecord {
    pub key: String,
    pub label: String,
    pub original: f64,
    pub clamped: f64,
    pub reason: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DynamicConstraintDebugInfo {
    pub window: DynamicConstraintWindow,
    pub clamp_count: usize,
    pub clamps: Vec<DynamicConstraintClampRecord>,
}

fn clamp01(v: f64) -> f64 {
    v.max(0.0).min(1.0)
}

fn build_constraint_band(hard_min: f64, hard_max: f64) -> DynamicConstraintBand {
    let span = (hard_max - hard_min).max(0.001);
    let margin = span * 0.12;
    DynamicConstraintBand {
        hard_min,
        hard_max,
        soft_min: (hard_min + margin).min(hard_max),
        soft_max: (hard_max - margin).max(hard_min),
    }
}

fn normalize_hard_band(min_v: f64, max_v: f64, fallback_min: f64, fallback_max: f64) -> (f64, f64) {
    if min_v <= max_v {
        (min_v, max_v)
    } else {
        (fallback_min, fallback_max)
    }
}

fn fallback_features_from_adjustments(current_adjustments: &Value) -> StyleFeatures {
    let exposure = current_adjustments
        .get("exposure")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let brightness = current_adjustments
        .get("brightness")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let saturation = current_adjustments
        .get("saturation")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let vibrance = current_adjustments
        .get("vibrance")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let contrast = current_adjustments
        .get("contrast")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    let mean_luma = (0.5 + exposure * 0.08 + brightness * 0.003).max(0.08).min(0.92);
    let p10 = (mean_luma - 0.20 - contrast * 0.0012).max(0.01).min(0.65);
    let p50 = mean_luma.max(0.06).min(0.94);
    let p90 = (mean_luma + 0.22 + contrast * 0.0011).max(0.22).min(0.98);
    let p99 = (p90 + 0.06).max(0.40).min(0.995);
    let sat = (0.32 + saturation * 0.0024 + vibrance * 0.0018).max(0.04).min(0.88);

    StyleFeatures {
        mean_luminance: mean_luma,
        highlight_ratio: (p90 - 0.75).max(0.0).min(0.35),
        shadow_ratio: (0.26 - p10).max(0.0).min(0.38),
        contrast_spread: (0.18 + contrast.abs() * 0.0014).max(0.08).min(0.45),
        p10_luminance: p10,
        p50_luminance: p50,
        p90_luminance: p90,
        p99_luminance: p99,
        clipped_highlight_ratio: (p99 - 0.975).max(0.0).min(0.05),
        waveform_low_band: (p10 + 0.04).max(0.03).min(0.40),
        waveform_mid_band: p50,
        waveform_high_band: (p90 - 0.03).max(0.30).min(0.97),
        rb_ratio: 1.0,
        gb_ratio: 1.0,
        mean_saturation: sat,
        saturation_spread: (0.11 + vibrance.abs() * 0.0012).max(0.04).min(0.35),
        shadow_luminance_mean: (p10 + 0.03).max(0.02).min(0.40),
        mid_luminance_mean: p50,
        highlight_luminance_mean: (p90 + 0.02).max(0.35).min(0.99),
        skin_ratio: 0.0,
        skin_luminance_mean: p50,
        skin_rb_ratio: 1.0,
        laplacian_variance: 220.0,
        vignette_diff: 0.0,
    }
}

pub fn build_dynamic_constraint_window(
    cur_feat: &StyleFeatures,
    current_adjustments: &Value,
    source: &str,
) -> DynamicConstraintWindow {
    let highlight_risk = clamp01(
        ((cur_feat.p99_luminance - 0.95) / 0.05) * 0.62
            + ((cur_feat.clipped_highlight_ratio - 0.008) / 0.02) * 0.38,
    );
    let shadow_risk = clamp01(
        ((0.08 - cur_feat.p10_luminance) / 0.08) * 0.65
            + ((cur_feat.shadow_ratio - 0.42) / 0.30) * 0.35,
    );
    let saturation_risk = clamp01(
        ((cur_feat.mean_saturation - 0.58) / 0.23) * 0.65
            + ((cur_feat.saturation_spread - 0.25) / 0.15) * 0.35,
    );

    let exposure_min = -2.35 + shadow_risk * 1.20;
    let exposure_max = 2.35 - highlight_risk * 1.45;
    let (exposure_min, exposure_max) = normalize_hard_band(exposure_min, exposure_max, -2.35, 2.35);

    let whites_max = 62.0 - highlight_risk * 58.0;
    let highlights_max = 58.0 - highlight_risk * 52.0;
    let shadows_min = -60.0 + shadow_risk * 48.0;
    let blacks_min = -58.0 + shadow_risk * 46.0;

    let mut sat_cap = 78.0 - saturation_risk * 42.0;
    let mut vib_cap = 80.0 - saturation_risk * 45.0;
    if cur_feat.skin_ratio > 0.02 {
        sat_cap -= 8.0;
        vib_cap -= 10.0;
    }
    let sat_cap = sat_cap.max(22.0).min(82.0);
    let vib_cap = vib_cap.max(20.0).min(82.0);

    let mut temp_cap = 76.0 - highlight_risk * 12.0;
    let mut tint_cap = 72.0 - saturation_risk * 10.0;
    if cur_feat.skin_ratio > 0.02 {
        temp_cap = temp_cap.min(58.0);
        tint_cap = tint_cap.min(52.0);
    }

    let _current_exposure = current_adjustments
        .get("exposure")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    let mut bands = HashMap::new();
    bands.insert("exposure".to_string(), build_constraint_band(exposure_min, exposure_max));
    bands.insert("brightness".to_string(), build_constraint_band(-80.0, 80.0));
    bands.insert("contrast".to_string(), build_constraint_band(-80.0, 80.0));
    bands.insert("highlights".to_string(), build_constraint_band(-80.0, highlights_max.max(12.0).min(80.0)));
    bands.insert("shadows".to_string(), build_constraint_band(shadows_min.max(-80.0).min(0.0), 80.0));
    bands.insert("whites".to_string(), build_constraint_band(-80.0, whites_max.max(8.0).min(80.0)));
    bands.insert("blacks".to_string(), build_constraint_band(blacks_min.max(-80.0).min(0.0), 80.0));
    bands.insert("saturation".to_string(), build_constraint_band(-80.0, sat_cap));
    bands.insert("vibrance".to_string(), build_constraint_band(-80.0, vib_cap));
    bands.insert("temperature".to_string(), build_constraint_band(-temp_cap, temp_cap));
    bands.insert("tint".to_string(), build_constraint_band(-tint_cap, tint_cap));
    bands.insert("clarity".to_string(), build_constraint_band(-80.0, 80.0));
    bands.insert("dehaze".to_string(), build_constraint_band(-70.0, 80.0));
    bands.insert("structure".to_string(), build_constraint_band(-80.0, 80.0));
    bands.insert("sharpness".to_string(), build_constraint_band(0.0, 100.0));
    bands.insert("vignetteAmount".to_string(), build_constraint_band(-80.0, 80.0));

    DynamicConstraintWindow {
        source: source.to_string(),
        highlight_risk,
        shadow_risk,
        saturation_risk,
        bands,
    }
}

pub fn build_dynamic_constraint_window_from_image(
    current_image_path: Option<&str>,
    current_adjustments: &Value,
) -> DynamicConstraintWindow {
    if let Some(path) = current_image_path {
        if Path::new(path).exists() {
            if let Ok(img) = smart_open_image(path) {
                let feat = extract_features(&img);
                return build_dynamic_constraint_window(&feat, current_adjustments, "image");
            }
        }
    }
    let fallback_feat = fallback_features_from_adjustments(current_adjustments);
    build_dynamic_constraint_window(&fallback_feat, current_adjustments, "fallback")
}

fn format_adjustment_value(key: &str, value: f64) -> f64 {
    if key == "exposure" {
        (value * 100.0).round() / 100.0
    } else if key == "sharpness" {
        value.round().max(0.0)
    } else {
        value.round()
    }
}

fn clamp_reason_for_key(key: &str, window: &DynamicConstraintWindow) -> String {
    match key {
        "exposure" | "highlights" | "whites" => format!(
            "动态约束触发：当前图像高光风险 {:.2}，限制提亮幅度",
            window.highlight_risk
        ),
        "shadows" | "blacks" => format!(
            "动态约束触发：当前图像阴影风险 {:.2}，限制压暗幅度",
            window.shadow_risk
        ),
        "saturation" | "vibrance" => format!(
            "动态约束触发：当前图像饱和风险 {:.2}，限制颜色强度",
            window.saturation_risk
        ),
        _ => "动态约束触发：参数超出当前图像可承受范围".to_string(),
    }
}

pub fn clamp_value_with_dynamic_window(
    key: &str,
    value: f64,
    window: &DynamicConstraintWindow,
) -> (f64, Option<String>) {
    let Some(band) = window.bands.get(key) else {
        let hard = if key == "exposure" { (-2.5, 2.5) } else if key == "sharpness" { (0.0, 100.0) } else { (-80.0, 80.0) };
        let clamped = value.max(hard.0).min(hard.1);
        if (clamped - value).abs() > 1e-6 {
            return (
                format_adjustment_value(key, clamped),
                Some("默认安全约束：超出全局参数安全范围".to_string()),
            );
        }
        return (format_adjustment_value(key, clamped), None);
    };
    let clamped = value.max(band.hard_min).min(band.hard_max);
    if (clamped - value).abs() > 1e-6 {
        return (
            format_adjustment_value(key, clamped),
            Some(clamp_reason_for_key(key, window)),
        );
    }
    (format_adjustment_value(key, clamped), None)
}

pub fn apply_dynamic_constraints_to_style_suggestions(
    suggestions: &mut Vec<StyleTransferSuggestion>,
    window: &DynamicConstraintWindow,
) -> DynamicConstraintDebugInfo {
    let mut clamps = Vec::new();
    for suggestion in suggestions.iter_mut() {
        if let Some(band) = window.bands.get(&suggestion.key) {
            let original = suggestion.value;
            let (clamped, reason) = clamp_value_with_dynamic_window(&suggestion.key, suggestion.value, window);
            suggestion.value = clamped;
            suggestion.min = suggestion.min.max(band.hard_min);
            suggestion.max = suggestion.max.min(band.hard_max);
            if suggestion.min > suggestion.max {
                suggestion.min = band.hard_min;
                suggestion.max = band.hard_max;
            }
            if let Some(reason_text) = reason {
                clamps.push(DynamicConstraintClampRecord {
                    key: suggestion.key.clone(),
                    label: suggestion.label.clone(),
                    original,
                    clamped,
                    reason: reason_text,
                });
                if suggestion.reason.is_empty() {
                    suggestion.reason = "动态约束已调整".to_string();
                } else {
                    suggestion.reason = format!("{}；动态约束已调整", suggestion.reason);
                }
            }
        } else {
            let original = suggestion.value;
            let (clamped, reason) = clamp_value_with_dynamic_window(&suggestion.key, suggestion.value, window);
            suggestion.value = clamped;
            if let Some(reason_text) = reason {
                clamps.push(DynamicConstraintClampRecord {
                    key: suggestion.key.clone(),
                    label: suggestion.label.clone(),
                    original,
                    clamped,
                    reason: reason_text,
                });
            }
        }
    }
    clamps.sort_by(|a, b| {
        (b.original - b.clamped)
            .abs()
            .total_cmp(&(a.original - a.clamped).abs())
    });
    DynamicConstraintDebugInfo {
        window: window.clone(),
        clamp_count: clamps.len(),
        clamps,
    }
}

fn empty_constraint_debug(window: &DynamicConstraintWindow) -> DynamicConstraintDebugInfo {
    DynamicConstraintDebugInfo {
        window: window.clone(),
        clamp_count: 0,
        clamps: Vec::new(),
    }
}

fn component_score(error_value: f64, scale: f64) -> f64 {
    (100.0 * (1.0 - error_value / scale).max(0.0)).min(100.0)
}

fn style_proximity_score(
    ref_feat: &StyleFeatures,
    pred_feat: &StyleFeatures,
    breakdown: &StyleTransferErrorBreakdown,
) -> StyleProximityScore {
    let tonal = component_score(breakdown.tonal, 2.0);
    let color = component_score(breakdown.color, 1.25);
    let skin = if ref_feat.skin_ratio > 0.015 && pred_feat.skin_ratio > 0.015 {
        component_score(breakdown.skin, 0.75)
    } else {
        100.0
    };
    let highlight = component_score(breakdown.highlight_penalty, 0.45);
    let skin_weight = if ref_feat.skin_ratio > 0.015 && pred_feat.skin_ratio > 0.015 {
        0.16
    } else {
        0.0
    };
    let overall = tonal * 0.42 + color * 0.28 + highlight * 0.14 + skin * skin_weight;
    let norm = 0.42 + 0.28 + 0.14 + skin_weight;
    StyleProximityScore {
        tonal: (tonal * 10.0).round() / 10.0,
        color: (color * 10.0).round() / 10.0,
        skin: (skin * 10.0).round() / 10.0,
        highlight: (highlight * 10.0).round() / 10.0,
        overall: ((overall / norm) * 10.0).round() / 10.0,
    }
}

fn infer_constraint_category(reason: &str) -> (&'static str, &'static str) {
    if reason.contains("高光") || reason.contains("过曝") {
        ("highlight", "高光受限")
    } else if reason.contains("阴影") || reason.contains("压暗") || reason.contains("黑") {
        ("shadow", "阴影受限")
    } else if reason.contains("饱和") || reason.contains("颜色") {
        ("saturation", "饱和受限")
    } else {
        ("general", "整体受限")
    }
}

fn actions_for_constraint_category(category: &str) -> Vec<StyleConstraintAction> {
    match category {
        "highlight" => vec![
            StyleConstraintAction { key: "highlights".to_string(), label: "高光".to_string(), delta: -6.0 },
            StyleConstraintAction { key: "whites".to_string(), label: "白色色阶".to_string(), delta: -8.0 },
            StyleConstraintAction { key: "exposure".to_string(), label: "曝光".to_string(), delta: -0.08 },
        ],
        "shadow" => vec![
            StyleConstraintAction { key: "shadows".to_string(), label: "阴影".to_string(), delta: 6.0 },
            StyleConstraintAction { key: "blacks".to_string(), label: "黑色色阶".to_string(), delta: 5.0 },
            StyleConstraintAction { key: "exposure".to_string(), label: "曝光".to_string(), delta: 0.06 },
        ],
        "saturation" => vec![
            StyleConstraintAction { key: "saturation".to_string(), label: "饱和度".to_string(), delta: -6.0 },
            StyleConstraintAction { key: "vibrance".to_string(), label: "自然饱和度".to_string(), delta: -6.0 },
        ],
        _ => vec![
            StyleConstraintAction { key: "contrast".to_string(), label: "对比度".to_string(), delta: -4.0 },
            StyleConstraintAction { key: "exposure".to_string(), label: "曝光".to_string(), delta: -0.04 },
        ],
    }
}

fn build_blocked_items(
    total_error_after: f64,
    constraint_debug: &Option<DynamicConstraintDebugInfo>,
) -> Vec<StyleConstraintBlockItem> {
    let Some(debug) = constraint_debug else {
        return Vec::new();
    };
    if debug.clamp_count == 0 || total_error_after < 0.35 {
        return Vec::new();
    }
    let mut grouped: HashMap<String, (String, String, u32, f64)> = HashMap::new();
    for clamp in &debug.clamps {
        let (category, label) = infer_constraint_category(&clamp.reason);
        let key = category.to_string();
        let entry = grouped
            .entry(key)
            .or_insert((label.to_string(), clamp.reason.clone(), 0, 0.0));
        entry.2 += 1;
        entry.3 += (clamp.original - clamp.clamped).abs();
    }
    let mut items: Vec<StyleConstraintBlockItem> = grouped
        .into_iter()
        .map(|(category, (label, reason, hit_count, severity))| StyleConstraintBlockItem {
            actions: actions_for_constraint_category(&category),
            category,
            label,
            reason,
            hit_count,
            severity: (severity * 100.0).round() / 100.0,
        })
        .collect();
    items.sort_by(|a, b| b.severity.total_cmp(&a.severity).then_with(|| b.hit_count.cmp(&a.hit_count)));
    items.truncate(3);
    items
}

fn build_blocked_reasons(
    total_error_after: f64,
    constraint_debug: &Option<DynamicConstraintDebugInfo>,
) -> Vec<String> {
    build_blocked_items(total_error_after, constraint_debug)
        .into_iter()
        .map(|item| format!("{}（命中 {} 次）", item.reason, item.hit_count))
        .collect()
}

/// 从图像中提取风格特征向量
fn extract_features(img: &DynamicImage) -> StyleFeatures {
    let (w, h) = img.dimensions();
    let total_pixels = (w as f64) * (h as f64);
    if total_pixels == 0.0 {
        return StyleFeatures {
            mean_luminance: 0.0, highlight_ratio: 0.0, shadow_ratio: 0.0,
            contrast_spread: 0.0,
            p10_luminance: 0.0, p50_luminance: 0.0, p90_luminance: 0.0, p99_luminance: 0.0,
            clipped_highlight_ratio: 0.0,
            waveform_low_band: 0.0, waveform_mid_band: 0.0, waveform_high_band: 0.0,
            rb_ratio: 1.0, gb_ratio: 1.0,
            mean_saturation: 0.0, saturation_spread: 0.0,
            shadow_luminance_mean: 0.0, mid_luminance_mean: 0.0, highlight_luminance_mean: 0.0,
            skin_ratio: 0.0, skin_luminance_mean: 0.0, skin_rb_ratio: 1.0,
            laplacian_variance: 0.0, vignette_diff: 0.0,
        };
    }

    // 降采样加速：如果图像太大，缩小到 max 600px 边（减少内存占用）
    let analysis_img = if w > 600 || h > 600 {
        img.resize(600, 600, image::imageops::FilterType::Nearest)
    } else {
        img.resize(w, h, image::imageops::FilterType::Nearest) // 避免 clone 大图
    };
    let (aw, ah) = analysis_img.dimensions();
    let a_total = (aw as f64) * (ah as f64);

    let mut sum_lum: f64 = 0.0;
    let mut sum_r: f64 = 0.0;
    let mut sum_g: f64 = 0.0;
    let mut sum_b: f64 = 0.0;
    let mut sum_sat: f64 = 0.0;
    let mut highlight_count: f64 = 0.0;
    let mut shadow_count: f64 = 0.0;
    let mut clipped_highlight_count: f64 = 0.0;
    let mut shadow_lum_sum: f64 = 0.0;
    let mut shadow_lum_count: f64 = 0.0;
    let mut mid_lum_sum: f64 = 0.0;
    let mut mid_lum_count: f64 = 0.0;
    let mut highlight_lum_sum: f64 = 0.0;
    let mut highlight_lum_count: f64 = 0.0;
    let mut skin_count: f64 = 0.0;
    let mut skin_lum_sum: f64 = 0.0;
    let mut skin_rb_sum: f64 = 0.0;
    let mut lum_values: Vec<f64> = Vec::with_capacity(a_total as usize);
    let mut sat_values: Vec<f64> = Vec::with_capacity(a_total as usize);
    let waveform_bins = 32usize;
    let mut waveform_bin_values: Vec<Vec<f64>> = (0..waveform_bins).map(|_| Vec::new()).collect();

    // 暗角检测用
    let cx = aw as f64 / 2.0;
    let cy = ah as f64 / 2.0;
    let max_dist = (cx * cx + cy * cy).sqrt();
    let mut center_lum_sum: f64 = 0.0;
    let mut center_count: f64 = 0.0;
    let mut edge_lum_sum: f64 = 0.0;
    let mut edge_count: f64 = 0.0;

    for y in 0..ah {
        for x in 0..aw {
            let px = analysis_img.get_pixel(x, y).to_rgb();
            let r = px[0] as f64 / 255.0;
            let g = px[1] as f64 / 255.0;
            let b = px[2] as f64 / 255.0;

            let lum = 0.2126 * r + 0.7152 * g + 0.0722 * b;
            sum_lum += lum;
            sum_r += r;
            sum_g += g;
            sum_b += b;
            lum_values.push(lum);

            if lum > 0.8 { highlight_count += 1.0; }
            if lum < 0.2 { shadow_count += 1.0; }
            if lum > 0.98 { clipped_highlight_count += 1.0; }
            if lum < 0.33 {
                shadow_lum_sum += lum;
                shadow_lum_count += 1.0;
            } else if lum < 0.70 {
                mid_lum_sum += lum;
                mid_lum_count += 1.0;
            } else {
                highlight_lum_sum += lum;
                highlight_lum_count += 1.0;
            }

            // HSL saturation
            let max_c = r.max(g).max(b);
            let min_c = r.min(g).min(b);
            let sat = if max_c + min_c == 0.0 || max_c == min_c {
                0.0
            } else {
                let l = (max_c + min_c) / 2.0;
                let delta = max_c - min_c;
                if l <= 0.5 { delta / (max_c + min_c) } else { delta / (2.0 - max_c - min_c) }
            };
            sum_sat += sat;
            sat_values.push(sat);

            // 暗角：中心 vs 边缘
            let dist = ((x as f64 - cx).powi(2) + (y as f64 - cy).powi(2)).sqrt();
            let norm_dist = dist / max_dist;
            if norm_dist < 0.35 {
                center_lum_sum += lum;
                center_count += 1.0;
            } else if norm_dist > 0.7 {
                edge_lum_sum += lum;
                edge_count += 1.0;
            }

            let is_skin = r > 0.35
                && g > 0.20
                && b > 0.12
                && r > g
                && g > b * 0.8
                && (r - g) > 0.02
                && (r - b) > 0.05;
            if is_skin {
                skin_count += 1.0;
                skin_lum_sum += lum;
                skin_rb_sum += if b > 0.001 { r / b } else { 1.0 };
            }

            let mut bin_idx = ((x as f64 / aw as f64) * waveform_bins as f64).floor() as usize;
            if bin_idx >= waveform_bins {
                bin_idx = waveform_bins - 1;
            }
            waveform_bin_values[bin_idx].push(lum);
        }
    }

    let mean_lum = sum_lum / a_total;
    let mean_sat = sum_sat / a_total;

    // 对比度：亮度标准差
    let variance: f64 = lum_values.iter().map(|l| (l - mean_lum).powi(2)).sum::<f64>() / a_total;
    let contrast_spread = variance.sqrt();

    // 饱和度标准差
    let sat_variance: f64 = sat_values.iter().map(|s| (s - mean_sat).powi(2)).sum::<f64>() / a_total;
    let saturation_spread = sat_variance.sqrt();

    let mut lum_sorted = lum_values.clone();
    lum_sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let quantile = |q: f64| -> f64 {
        if lum_sorted.is_empty() {
            return 0.0;
        }
        let idx = ((lum_sorted.len() - 1) as f64 * q).round() as usize;
        lum_sorted[idx.min(lum_sorted.len() - 1)]
    };
    let p10 = quantile(0.10);
    let p50 = quantile(0.50);
    let p90 = quantile(0.90);
    let p99 = quantile(0.99);

    let mut low_sum = 0.0;
    let mut mid_sum = 0.0;
    let mut high_sum = 0.0;
    let mut band_count = 0.0;
    for mut bin in waveform_bin_values {
        if bin.is_empty() {
            continue;
        }
        bin.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let get_q = |q: f64| -> f64 {
            let idx = ((bin.len() - 1) as f64 * q).round() as usize;
            bin[idx.min(bin.len() - 1)]
        };
        low_sum += get_q(0.10);
        mid_sum += get_q(0.50);
        high_sum += get_q(0.90);
        band_count += 1.0;
    }
    let waveform_low_band = if band_count > 0.0 { low_sum / band_count } else { p10 };
    let waveform_mid_band = if band_count > 0.0 { mid_sum / band_count } else { p50 };
    let waveform_high_band = if band_count > 0.0 { high_sum / band_count } else { p90 };

    // R/B 和 G/B 通道比值（色温/色调）
    let mean_r = sum_r / a_total;
    let mean_g = sum_g / a_total;
    let mean_b = sum_b / a_total;
    let rb_ratio = if mean_b > 0.001 { mean_r / mean_b } else { 1.0 };
    let gb_ratio = if mean_b > 0.001 { mean_g / mean_b } else { 1.0 };

    // 拉普拉斯方差（清晰度/结构）
    let gray = analysis_img.to_luma8();
    let laplacian_variance = compute_laplacian_variance(&gray, aw, ah);

    // 暗角差值
    let center_mean = if center_count > 0.0 { center_lum_sum / center_count } else { mean_lum };
    let edge_mean = if edge_count > 0.0 { edge_lum_sum / edge_count } else { mean_lum };
    let vignette_diff = center_mean - edge_mean; // 正值=边缘暗=有暗角
    let shadow_luminance_mean = if shadow_lum_count > 0.0 { shadow_lum_sum / shadow_lum_count } else { p10 };
    let mid_luminance_mean = if mid_lum_count > 0.0 { mid_lum_sum / mid_lum_count } else { p50 };
    let highlight_luminance_mean = if highlight_lum_count > 0.0 { highlight_lum_sum / highlight_lum_count } else { p90 };
    let skin_ratio = skin_count / a_total;
    let skin_luminance_mean = if skin_count > 0.0 { skin_lum_sum / skin_count } else { mean_lum };
    let skin_rb_ratio = if skin_count > 0.0 { skin_rb_sum / skin_count } else { rb_ratio };

    StyleFeatures {
        mean_luminance: mean_lum,
        highlight_ratio: highlight_count / a_total,
        shadow_ratio: shadow_count / a_total,
        contrast_spread,
        p10_luminance: p10,
        p50_luminance: p50,
        p90_luminance: p90,
        p99_luminance: p99,
        clipped_highlight_ratio: clipped_highlight_count / a_total,
        waveform_low_band,
        waveform_mid_band,
        waveform_high_band,
        rb_ratio,
        gb_ratio,
        mean_saturation: mean_sat,
        saturation_spread,
        shadow_luminance_mean,
        mid_luminance_mean,
        highlight_luminance_mean,
        skin_ratio,
        skin_luminance_mean,
        skin_rb_ratio,
        laplacian_variance,
        vignette_diff,
    }
}

/// 计算灰度图的拉普拉斯方差（衡量图像清晰度/纹理丰富度）
fn compute_laplacian_variance(gray: &image::GrayImage, w: u32, h: u32) -> f64 {
    if w < 3 || h < 3 {
        return 0.0;
    }
    let mut sum: f64 = 0.0;
    let mut sum_sq: f64 = 0.0;
    let count = ((w - 2) as f64) * ((h - 2) as f64);

    for y in 1..(h - 1) {
        for x in 1..(w - 1) {
            let center = gray.get_pixel(x, y)[0] as f64;
            let top = gray.get_pixel(x, y - 1)[0] as f64;
            let bottom = gray.get_pixel(x, y + 1)[0] as f64;
            let left = gray.get_pixel(x - 1, y)[0] as f64;
            let right = gray.get_pixel(x + 1, y)[0] as f64;
            let lap = -4.0 * center + top + bottom + left + right;
            sum += lap;
            sum_sq += lap * lap;
        }
    }

    let mean = sum / count;
    (sum_sq / count) - mean * mean
}

fn classify_tonal_style(feat: &StyleFeatures) -> &'static str {
    if feat.p50_luminance > 0.62 && feat.p10_luminance > 0.35 {
        "高调"
    } else if feat.p50_luminance < 0.40 && feat.p90_luminance < 0.72 {
        "低调"
    } else {
        "中间调"
    }
}

fn estimate_features_from_adjustments(
    cur_feat: &StyleFeatures,
    current_values: &HashMap<String, f64>,
    candidate_values: &HashMap<String, f64>,
) -> StyleFeatures {
    let mut f = cur_feat.clone();
    let delta = |k: &str| {
        candidate_values.get(k).copied().unwrap_or_else(|| current_values.get(k).copied().unwrap_or(0.0))
            - current_values.get(k).copied().unwrap_or(0.0)
    };

    let d_exp = delta("exposure");
    let d_con = delta("contrast");
    let d_hl = delta("highlights");
    let d_sh = delta("shadows");
    let d_wh = delta("whites");
    let d_bl = delta("blacks");
    let d_temp = delta("temperature");
    let d_tint = delta("tint");
    let d_sat = delta("saturation");
    let d_vib = delta("vibrance");
    let d_clarity = delta("clarity");
    let d_vig = delta("vignetteAmount");

    f.mean_luminance += d_exp * 0.20 + d_hl * 0.0012 + d_sh * 0.0010 + d_wh * 0.0016 - d_bl * 0.0009;
    f.mid_luminance_mean += d_exp * 0.18 + d_sh * 0.0012 + d_con * 0.0006;
    f.shadow_luminance_mean += d_exp * 0.13 + d_sh * 0.0018 - d_bl * 0.0015 - d_con * 0.0007;
    f.highlight_luminance_mean += d_exp * 0.16 + d_hl * 0.0018 + d_wh * 0.0022 + d_con * 0.0008;
    f.waveform_mid_band += d_exp * 0.18 + d_sh * 0.0010 + d_hl * 0.0008;
    f.waveform_low_band += d_exp * 0.12 + d_sh * 0.0015 - d_bl * 0.0016;
    f.waveform_high_band += d_exp * 0.14 + d_hl * 0.0019 + d_wh * 0.0024;
    f.p10_luminance += d_exp * 0.11 + d_sh * 0.0014 - d_bl * 0.0018 - d_con * 0.0005;
    f.p50_luminance += d_exp * 0.17 + d_con * 0.0004;
    f.p90_luminance += d_exp * 0.13 + d_hl * 0.0016 + d_wh * 0.0019 + d_con * 0.0006;
    f.p99_luminance += d_exp * 0.18 + d_hl * 0.0020 + d_wh * 0.0023 + d_con * 0.0010;
    f.clipped_highlight_ratio += d_exp * 0.05 + d_hl * 0.00045 + d_wh * 0.00055;
    f.contrast_spread += d_con * 0.0021 + d_wh * 0.0008 + d_bl * 0.0008;
    f.rb_ratio += d_temp * 0.0028;
    f.gb_ratio += -d_tint * 0.0025;
    f.mean_saturation += d_sat * 0.0042 + d_vib * 0.0028 - d_exp * 0.0012;
    f.saturation_spread += d_vib * 0.0032 + d_sat * 0.0014;
    f.skin_luminance_mean += d_exp * 0.15 + d_hl * 0.0011 + d_wh * 0.0014;
    f.skin_rb_ratio += d_temp * 0.0022 - d_tint * 0.0008;
    f.laplacian_variance += d_clarity * 15.0;
    f.vignette_diff += -d_vig * 0.0035;

    let clamp01 = |v: f64| v.max(0.0).min(1.0);
    f.mean_luminance = clamp01(f.mean_luminance);
    f.shadow_luminance_mean = clamp01(f.shadow_luminance_mean);
    f.mid_luminance_mean = clamp01(f.mid_luminance_mean);
    f.highlight_luminance_mean = clamp01(f.highlight_luminance_mean);
    f.waveform_low_band = clamp01(f.waveform_low_band);
    f.waveform_mid_band = clamp01(f.waveform_mid_band);
    f.waveform_high_band = clamp01(f.waveform_high_band);
    f.p10_luminance = clamp01(f.p10_luminance);
    f.p50_luminance = clamp01(f.p50_luminance);
    f.p90_luminance = clamp01(f.p90_luminance);
    f.p99_luminance = clamp01(f.p99_luminance);
    f.clipped_highlight_ratio = clamp01(f.clipped_highlight_ratio);
    f.mean_saturation = clamp01(f.mean_saturation);
    f.saturation_spread = clamp01(f.saturation_spread);
    f.skin_luminance_mean = clamp01(f.skin_luminance_mean);
    f
}

fn style_distance_score(
    ref_feat: &StyleFeatures,
    pred_feat: &StyleFeatures,
    tuning: StyleTransferTuning,
) -> f64 {
    let mut s = 0.0;
    s += (ref_feat.waveform_mid_band - pred_feat.waveform_mid_band).abs() * 4.6;
    s += (ref_feat.waveform_low_band - pred_feat.waveform_low_band).abs() * 3.4;
    s += (ref_feat.waveform_high_band - pred_feat.waveform_high_band).abs() * 4.0;
    s += (ref_feat.p10_luminance - pred_feat.p10_luminance).abs() * 3.0;
    s += (ref_feat.p50_luminance - pred_feat.p50_luminance).abs() * 2.6;
    s += (ref_feat.p90_luminance - pred_feat.p90_luminance).abs() * 2.8;
    s += (ref_feat.contrast_spread - pred_feat.contrast_spread).abs() * 7.0;
    s += (ref_feat.mean_saturation - pred_feat.mean_saturation).abs() * 4.0;
    s += (ref_feat.saturation_spread - pred_feat.saturation_spread).abs() * 2.4;
    s += (ref_feat.rb_ratio - pred_feat.rb_ratio).abs() * 2.2;
    s += (ref_feat.gb_ratio - pred_feat.gb_ratio).abs() * 2.0;
    s += (ref_feat.shadow_luminance_mean - pred_feat.shadow_luminance_mean).abs() * 2.2;
    s += (ref_feat.mid_luminance_mean - pred_feat.mid_luminance_mean).abs() * 2.0;
    s += (ref_feat.highlight_luminance_mean - pred_feat.highlight_luminance_mean).abs() * 2.4;
    if ref_feat.skin_ratio > 0.015 && pred_feat.skin_ratio > 0.015 {
        s += (ref_feat.skin_luminance_mean - pred_feat.skin_luminance_mean).abs() * 3.8 * tuning.skin_protect_strength;
        s += (ref_feat.skin_rb_ratio - pred_feat.skin_rb_ratio).abs() * 2.0 * tuning.skin_protect_strength;
    }
    let target_p99_cap = if classify_tonal_style(ref_feat) == "高调" {
        0.985 + (1.0 - tuning.highlight_guard_strength) * 0.012
    } else {
        0.965 + (1.0 - tuning.highlight_guard_strength) * 0.012
    };
    if pred_feat.p99_luminance > target_p99_cap {
        s += (pred_feat.p99_luminance - target_p99_cap) * 30.0 * tuning.highlight_guard_strength;
    }
    s
}

fn style_metric_vector(feat: &StyleFeatures) -> [f64; 13] {
    [
        feat.waveform_mid_band,
        feat.waveform_low_band,
        feat.waveform_high_band,
        feat.p10_luminance,
        feat.p50_luminance,
        feat.p90_luminance,
        feat.contrast_spread,
        feat.mean_saturation,
        feat.saturation_spread,
        feat.rb_ratio,
        feat.gb_ratio,
        feat.skin_luminance_mean,
        feat.skin_rb_ratio,
    ]
}

fn style_metric_weights(ref_feat: &StyleFeatures, pred_feat: &StyleFeatures, tuning: StyleTransferTuning) -> [f64; 13] {
    let skin_w = if ref_feat.skin_ratio > 0.015 && pred_feat.skin_ratio > 0.015 {
        1.0 * tuning.skin_protect_strength
    } else {
        0.0
    };
    [
        4.6, 3.4, 4.0, 3.0, 2.6, 2.8, 7.0, 4.0, 2.4, 2.2, 2.0, 3.8 * skin_w, 2.0 * skin_w,
    ]
}

fn target_p99_cap(ref_feat: &StyleFeatures, tuning: StyleTransferTuning) -> f64 {
    if classify_tonal_style(ref_feat) == "高调" {
        0.985 + (1.0 - tuning.highlight_guard_strength) * 0.012
    } else {
        0.965 + (1.0 - tuning.highlight_guard_strength) * 0.012
    }
}

fn style_error_breakdown(
    ref_feat: &StyleFeatures,
    pred_feat: &StyleFeatures,
    tuning: StyleTransferTuning,
) -> StyleTransferErrorBreakdown {
    let mut tonal = 0.0;
    tonal += (ref_feat.waveform_mid_band - pred_feat.waveform_mid_band).abs() * 4.6;
    tonal += (ref_feat.waveform_low_band - pred_feat.waveform_low_band).abs() * 3.4;
    tonal += (ref_feat.waveform_high_band - pred_feat.waveform_high_band).abs() * 4.0;
    tonal += (ref_feat.p10_luminance - pred_feat.p10_luminance).abs() * 3.0;
    tonal += (ref_feat.p50_luminance - pred_feat.p50_luminance).abs() * 2.6;
    tonal += (ref_feat.p90_luminance - pred_feat.p90_luminance).abs() * 2.8;
    tonal += (ref_feat.contrast_spread - pred_feat.contrast_spread).abs() * 7.0;
    tonal += (ref_feat.shadow_luminance_mean - pred_feat.shadow_luminance_mean).abs() * 2.2;
    tonal += (ref_feat.mid_luminance_mean - pred_feat.mid_luminance_mean).abs() * 2.0;
    tonal += (ref_feat.highlight_luminance_mean - pred_feat.highlight_luminance_mean).abs() * 2.4;

    let mut color = 0.0;
    color += (ref_feat.mean_saturation - pred_feat.mean_saturation).abs() * 4.0;
    color += (ref_feat.saturation_spread - pred_feat.saturation_spread).abs() * 2.4;
    color += (ref_feat.rb_ratio - pred_feat.rb_ratio).abs() * 2.2;
    color += (ref_feat.gb_ratio - pred_feat.gb_ratio).abs() * 2.0;

    let mut skin = 0.0;
    if ref_feat.skin_ratio > 0.015 && pred_feat.skin_ratio > 0.015 {
        skin += (ref_feat.skin_luminance_mean - pred_feat.skin_luminance_mean).abs()
            * 3.8
            * tuning.skin_protect_strength;
        skin +=
            (ref_feat.skin_rb_ratio - pred_feat.skin_rb_ratio).abs() * 2.0 * tuning.skin_protect_strength;
    }

    let p99_cap = target_p99_cap(ref_feat, tuning);
    let highlight_penalty = if pred_feat.p99_luminance > p99_cap {
        (pred_feat.p99_luminance - p99_cap) * 30.0 * tuning.highlight_guard_strength
    } else {
        0.0
    };

    let total = tonal + color + skin + highlight_penalty;
    StyleTransferErrorBreakdown {
        tonal,
        color,
        skin,
        highlight_penalty,
        total,
    }
}

fn estimate_features_for_suggestions(
    cur_feat: &StyleFeatures,
    current_adjustments: &Value,
    suggestions: &[StyleTransferSuggestion],
) -> StyleFeatures {
    let mut current_values: HashMap<String, f64> = HashMap::new();
    let mut candidate_values: HashMap<String, f64> = HashMap::new();
    for s in suggestions {
        let current_value = current_adjustments
            .get(&s.key)
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        current_values.insert(s.key.clone(), current_value);
        candidate_values.insert(s.key.clone(), s.value);
    }
    estimate_features_from_adjustments(cur_feat, &current_values, &candidate_values)
}

fn action_meta(key: &str) -> Option<(&'static str, f64, f64)> {
    match key {
        "exposure" => Some(("曝光", -5.0, 5.0)),
        "contrast" => Some(("对比度", -100.0, 100.0)),
        "highlights" => Some(("高光", -100.0, 100.0)),
        "shadows" => Some(("阴影", -100.0, 100.0)),
        "whites" => Some(("白色色阶", -100.0, 100.0)),
        "blacks" => Some(("黑色色阶", -100.0, 100.0)),
        "temperature" => Some(("色温", -100.0, 100.0)),
        "tint" => Some(("色调偏移", -100.0, 100.0)),
        "saturation" => Some(("饱和度", -100.0, 100.0)),
        "vibrance" => Some(("自然饱和度", -100.0, 100.0)),
        "clarity" => Some(("清晰度", -100.0, 100.0)),
        "vignetteAmount" => Some(("暗角", -100.0, 100.0)),
        _ => None,
    }
}

fn is_key_allowed_for_stage(key: &str, stage: &str) -> bool {
    match stage {
        "tonal" => matches!(
            key,
            "exposure" | "contrast" | "highlights" | "shadows" | "whites" | "blacks"
        ),
        "color" => matches!(key, "temperature" | "tint" | "saturation" | "vibrance"),
        "skin" => matches!(key, "highlights" | "temperature" | "tint"),
        "highlight" => matches!(key, "highlights" | "whites" | "exposure"),
        _ => false,
    }
}

fn upsert_suggestion_delta(
    suggestions: &mut Vec<StyleTransferSuggestion>,
    current_adjustments: &Value,
    key: &str,
    delta: f64,
    reason: &str,
    stage: &str,
) -> bool {
    if delta.abs() < 0.0001 {
        return false;
    }
    if !is_key_allowed_for_stage(key, stage) {
        return false;
    }
    let Some((label, min, max)) = action_meta(key) else {
        return false;
    };
    if let Some(existing) = suggestions.iter_mut().find(|s| s.key == key) {
        let new_value = (existing.value + delta).max(min).min(max);
        existing.value = if key == "exposure" {
            (new_value * 100.0).round() / 100.0
        } else {
            new_value.round()
        };
        existing.reason = format!("{}；{}", existing.reason, reason);
        return true;
    }
    let current_value = current_adjustments
        .get(key)
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let new_value = (current_value + delta).max(min).min(max);
    suggestions.push(StyleTransferSuggestion {
        key: key.to_string(),
        value: if key == "exposure" {
            (new_value * 100.0).round() / 100.0
        } else {
            new_value.round()
        },
        label: label.to_string(),
        min,
        max,
        reason: reason.to_string(),
    });
    true
}

fn auto_refine_suggestions_by_error(
    ref_feat: &StyleFeatures,
    cur_feat: &StyleFeatures,
    current_adjustments: &Value,
    suggestions: &mut Vec<StyleTransferSuggestion>,
    tuning: StyleTransferTuning,
) -> u32 {
    let mut rounds = 0u32;
    for round in 0..2 {
        if suggestions.is_empty() {
            break;
        }
        let pred = estimate_features_for_suggestions(cur_feat, current_adjustments, suggestions);
        let breakdown = style_error_breakdown(ref_feat, &pred, tuning);
        if breakdown.total < 0.45 {
            break;
        }
        let dominant = breakdown
            .tonal
            .max(breakdown.color)
            .max(breakdown.skin)
            .max(breakdown.highlight_penalty);
        let stage = if round == 0 {
            "tonal"
        } else if breakdown.highlight_penalty > 0.04 {
            "highlight"
        } else if breakdown.color >= breakdown.skin {
            "color"
        } else {
            "skin"
        };
        let mut changed = false;
        if (dominant - breakdown.highlight_penalty).abs() < 1e-6 {
            let overflow = (pred.p99_luminance - target_p99_cap(ref_feat, tuning)).max(0.0);
            changed |= upsert_suggestion_delta(
                suggestions,
                current_adjustments,
                "highlights",
                -(2.5 + overflow * 22.0) * tuning.highlight_guard_strength,
                "误差驱动二次微调：抑制过曝顶部堆积",
                stage,
            );
            changed |= upsert_suggestion_delta(
                suggestions,
                current_adjustments,
                "whites",
                -(3.0 + overflow * 26.0) * tuning.highlight_guard_strength,
                "误差驱动二次微调：收紧白场上限",
                stage,
            );
            changed |= upsert_suggestion_delta(
                suggestions,
                current_adjustments,
                "exposure",
                -(0.04 + overflow * 0.25) * tuning.highlight_guard_strength,
                "误差驱动二次微调：回收整体高光余量",
                stage,
            );
        } else if (dominant - breakdown.tonal).abs() < 1e-6 {
            changed |= upsert_suggestion_delta(
                suggestions,
                current_adjustments,
                "exposure",
                ((ref_feat.p50_luminance - pred.p50_luminance) * 0.95).max(-0.12).min(0.12),
                "误差驱动二次微调：修正中间调偏差",
                stage,
            );
            changed |= upsert_suggestion_delta(
                suggestions,
                current_adjustments,
                "contrast",
                ((ref_feat.contrast_spread - pred.contrast_spread) * 28.0).max(-4.0).min(4.0),
                "误差驱动二次微调：匹配对比度扩散",
                stage,
            );
            changed |= upsert_suggestion_delta(
                suggestions,
                current_adjustments,
                "shadows",
                ((ref_feat.waveform_low_band - pred.waveform_low_band) * 18.0).max(-4.0).min(4.0),
                "误差驱动二次微调：补偿暗部波形",
                stage,
            );
        } else if (dominant - breakdown.color).abs() < 1e-6 {
            changed |= upsert_suggestion_delta(
                suggestions,
                current_adjustments,
                "temperature",
                ((ref_feat.rb_ratio - pred.rb_ratio) * 14.0).max(-4.0).min(4.0),
                "误差驱动二次微调：纠正冷暖偏差",
                stage,
            );
            changed |= upsert_suggestion_delta(
                suggestions,
                current_adjustments,
                "tint",
                (-(ref_feat.gb_ratio - pred.gb_ratio) * 12.0).max(-4.0).min(4.0),
                "误差驱动二次微调：纠正色偏",
                stage,
            );
            changed |= upsert_suggestion_delta(
                suggestions,
                current_adjustments,
                "saturation",
                ((ref_feat.mean_saturation - pred.mean_saturation) * 30.0).max(-4.0).min(4.0),
                "误差驱动二次微调：匹配整体饱和度",
                stage,
            );
        } else {
            changed |= upsert_suggestion_delta(
                suggestions,
                current_adjustments,
                "highlights",
                ((ref_feat.skin_luminance_mean - pred.skin_luminance_mean) * 15.0).max(-3.0).min(3.0),
                "误差驱动二次微调：保护肤色亮度层次",
                stage,
            );
            changed |= upsert_suggestion_delta(
                suggestions,
                current_adjustments,
                "temperature",
                ((ref_feat.skin_rb_ratio - pred.skin_rb_ratio) * 8.0).max(-3.0).min(3.0),
                "误差驱动二次微调：校正肤色暖冷",
                stage,
            );
        }
        if !changed {
            break;
        }
        rounds += 1;
    }
    rounds
}

fn build_debug_actions(
    ref_feat: &StyleFeatures,
    pred_feat: &StyleFeatures,
    after: &StyleTransferErrorBreakdown,
    tuning: StyleTransferTuning,
) -> Vec<StyleTransferDebugAction> {
    let mut actions = Vec::new();
    if after.tonal > 0.35 {
        actions.push(StyleTransferDebugAction {
            key: "exposure".to_string(),
            label: "曝光".to_string(),
            recommended_delta: ((ref_feat.p50_luminance - pred_feat.p50_luminance) * 0.9)
                .max(-0.18)
                .min(0.18),
            priority: 1,
            reason: "影调误差主导，优先校正中间调基线".to_string(),
        });
        actions.push(StyleTransferDebugAction {
            key: "contrast".to_string(),
            label: "对比度".to_string(),
            recommended_delta: ((ref_feat.contrast_spread - pred_feat.contrast_spread) * 32.0)
                .max(-6.0)
                .min(6.0),
            priority: 2,
            reason: "影调层次未收敛，补偿明暗跨度".to_string(),
        });
    }
    if after.color > 0.25 {
        actions.push(StyleTransferDebugAction {
            key: "temperature".to_string(),
            label: "色温".to_string(),
            recommended_delta: ((ref_feat.rb_ratio - pred_feat.rb_ratio) * 16.0).max(-6.0).min(6.0),
            priority: 1,
            reason: "色彩误差偏大，优先修正冷暖方向".to_string(),
        });
        actions.push(StyleTransferDebugAction {
            key: "saturation".to_string(),
            label: "饱和度".to_string(),
            recommended_delta: ((ref_feat.mean_saturation - pred_feat.mean_saturation) * 35.0)
                .max(-6.0)
                .min(6.0),
            priority: 2,
            reason: "整体饱和度仍有差距".to_string(),
        });
    }
    if after.skin > 0.18 {
        actions.push(StyleTransferDebugAction {
            key: "highlights".to_string(),
            label: "高光".to_string(),
            recommended_delta: ((ref_feat.skin_luminance_mean - pred_feat.skin_luminance_mean) * 18.0)
                .max(-5.0)
                .min(5.0),
            priority: 1,
            reason: "肤色层次误差偏大，优先压制或提亮皮肤高光".to_string(),
        });
    }
    if after.highlight_penalty > 0.02 {
        actions.push(StyleTransferDebugAction {
            key: "whites".to_string(),
            label: "白色色阶".to_string(),
            recommended_delta: (-(pred_feat.p99_luminance - target_p99_cap(ref_feat, tuning)) * 55.0)
                .max(-10.0)
                .min(0.0),
            priority: 1,
            reason: "过曝惩罚仍在，建议继续收紧白场".to_string(),
        });
    }
    actions.sort_by(|a, b| {
        a.priority
            .cmp(&b.priority)
            .then_with(|| b.recommended_delta.abs().total_cmp(&a.recommended_delta.abs()))
    });
    actions.truncate(4);
    actions
}

fn build_style_debug_info(
    ref_feat: &StyleFeatures,
    cur_feat: &StyleFeatures,
    pred_feat: &StyleFeatures,
    tuning: StyleTransferTuning,
    auto_refine_rounds: u32,
    constraint_debug: Option<DynamicConstraintDebugInfo>,
) -> StyleTransferDebugInfo {
    let before = style_error_breakdown(ref_feat, cur_feat, tuning);
    let after = style_error_breakdown(ref_feat, pred_feat, tuning);
    let proximity_before = style_proximity_score(ref_feat, cur_feat, &before);
    let proximity_after = style_proximity_score(ref_feat, pred_feat, &after);
    let improvement_ratio = if before.total > 1e-6 {
        ((before.total - after.total) / before.total).max(-1.0).min(1.0)
    } else {
        0.0
    };
    let mut dominant_error = ("影调", after.tonal);
    if after.color > dominant_error.1 {
        dominant_error = ("色彩", after.color);
    }
    if after.skin > dominant_error.1 {
        dominant_error = ("肤色", after.skin);
    }
    if after.highlight_penalty > dominant_error.1 {
        dominant_error = ("过曝保护", after.highlight_penalty);
    }
    let suggested_actions = build_debug_actions(ref_feat, pred_feat, &after, tuning);
    let blocked_items = build_blocked_items(after.total, &constraint_debug);
    let blocked_reasons = build_blocked_reasons(after.total, &constraint_debug);
    StyleTransferDebugInfo {
        before,
        after,
        proximity_before,
        proximity_after,
        improvement_ratio,
        dominant_error: dominant_error.0.to_string(),
        auto_refine_rounds,
        suggested_actions,
        blocked_reasons,
        blocked_items,
        constraint_debug,
    }
}

/// 将两组特征的差异映射为滑块调整参数（优化版：更精准的感知映射）
fn map_features_to_adjustments(
    ref_feat: &StyleFeatures,
    cur_feat: &StyleFeatures,
    current_adjustments: &Value,
    tuning: StyleTransferTuning,
) -> Vec<StyleTransferSuggestion> {
    let mut suggestions = Vec::new();

    let get_current = |key: &str, default: f64| -> f64 {
        current_adjustments.get(key).and_then(|v| v.as_f64()).unwrap_or(default)
    };

    let clamp = |v: f64, min: f64, max: f64| -> f64 { v.max(min).min(max) };
    let adain_contrast_scale = if cur_feat.contrast_spread > 0.0001 {
        clamp(
            ref_feat.contrast_spread / cur_feat.contrast_spread * (0.85 + tuning.style_strength * 0.15),
            0.7,
            1.35,
        )
    } else {
        1.0
    };
    let adain_mean_shift = (ref_feat.mean_luminance - cur_feat.mean_luminance) * tuning.style_strength;
    let zone_mid_shift = ref_feat.mid_luminance_mean - cur_feat.mid_luminance_mean;
    let zone_shadow_shift = ref_feat.shadow_luminance_mean - cur_feat.shadow_luminance_mean;
    let zone_high_shift = ref_feat.highlight_luminance_mean - cur_feat.highlight_luminance_mean;

    let waveform_mid_diff = ref_feat.waveform_mid_band - cur_feat.waveform_mid_band;
    if waveform_mid_diff.abs() > 0.015 || adain_mean_shift.abs() > 0.015 {
        let exposure_delta = clamp(
            waveform_mid_diff * 0.95 + adain_mean_shift * 0.75 + zone_mid_shift * 0.55,
            -0.50,
            0.50,
        );
        let cur_exposure = get_current("exposure", 0.0);
        let new_exposure = clamp(cur_exposure + exposure_delta, -2.0, 2.0);
        suggestions.push(StyleTransferSuggestion {
            key: "exposure".to_string(),
            value: (new_exposure * 100.0).round() / 100.0,
            label: "曝光".to_string(),
            min: -5.0, max: 5.0,
            reason: "先对齐波形中间带，建立基础亮度区间".to_string(),
        });
    }

    let spread_ref = ref_feat.waveform_high_band - ref_feat.waveform_low_band;
    let spread_cur = cur_feat.waveform_high_band - cur_feat.waveform_low_band;
    let spread_diff = spread_ref - spread_cur + (adain_contrast_scale - 1.0) * 0.12;
    if spread_diff.abs() > 0.03 {
        let contrast_delta = clamp(spread_diff * 90.0 * tuning.style_strength, -24.0, 24.0);
        let cur_contrast = get_current("contrast", 0.0);
        let new_contrast = clamp(cur_contrast + contrast_delta, -50.0, 50.0);
        suggestions.push(StyleTransferSuggestion {
            key: "contrast".to_string(),
            value: new_contrast.round(),
            label: "对比度".to_string(),
            min: -100.0, max: 100.0,
            reason: "按波形上下带间距匹配影调对比".to_string(),
        });
    }

    let high_band_diff = ref_feat.waveform_high_band - cur_feat.waveform_high_band;
    let clip_diff = ref_feat.clipped_highlight_ratio - cur_feat.clipped_highlight_ratio;
    if high_band_diff.abs() > 0.020 || clip_diff.abs() > 0.01 || zone_high_shift.abs() > 0.02 {
        let hl_delta = clamp(
            high_band_diff * 60.0 + zone_high_shift * 34.0 - clip_diff * 120.0,
            -30.0,
            20.0,
        ) * (0.9 + tuning.style_strength * 0.1);
        let cur_hl = get_current("highlights", 0.0);
        let new_hl = clamp(cur_hl + hl_delta, -50.0, 50.0);
        suggestions.push(StyleTransferSuggestion {
            key: "highlights".to_string(),
            value: new_hl.round(),
            label: "高光".to_string(),
            min: -100.0, max: 100.0,
            reason: "约束高光上沿，避免波形顶部挤压和过曝".to_string(),
        });
    }

    let low_band_diff = ref_feat.waveform_low_band - cur_feat.waveform_low_band;
    if low_band_diff.abs() > 0.020 || zone_shadow_shift.abs() > 0.02 {
        let sh_delta = clamp(low_band_diff * 60.0 + zone_shadow_shift * 45.0, -24.0, 24.0) * (0.9 + tuning.style_strength * 0.1);
        let cur_sh = get_current("shadows", 0.0);
        let new_sh = clamp(cur_sh + sh_delta, -50.0, 50.0);
        suggestions.push(StyleTransferSuggestion {
            key: "shadows".to_string(),
            value: new_sh.round(),
            label: "阴影".to_string(),
            min: -100.0, max: 100.0,
            reason: "对齐波形低部区间，控制暗部密度".to_string(),
        });
    }

    let p99_diff = ref_feat.p99_luminance - cur_feat.p99_luminance;
    if p99_diff.abs() > 0.02 || clip_diff.abs() > 0.01 {
        let whites_delta = clamp(p99_diff * 80.0 - clip_diff * 140.0, -18.0, 14.0);
        let cur_whites = get_current("whites", 0.0);
        let new_whites = clamp(cur_whites + whites_delta, -45.0, 45.0);
        suggestions.push(StyleTransferSuggestion {
            key: "whites".to_string(),
            value: new_whites.round(),
            label: "白色色阶".to_string(),
            min: -100.0, max: 100.0,
            reason: "通过 99 分位亮度匹配白场上限".to_string(),
        });
    }

    let p10_diff = ref_feat.p10_luminance - cur_feat.p10_luminance;
    if p10_diff.abs() > 0.02 {
        let blacks_delta = clamp(p10_diff * 70.0, -15.0, 15.0);
        let cur_blacks = get_current("blacks", 0.0);
        let new_blacks = clamp(cur_blacks + blacks_delta, -45.0, 45.0);
        suggestions.push(StyleTransferSuggestion {
            key: "blacks".to_string(),
            value: new_blacks.round(),
            label: "黑色色阶".to_string(),
            min: -100.0, max: 100.0,
            reason: "通过 10 分位亮度匹配黑场下限".to_string(),
        });
    }

    // ===== 阶段二：影调收敛后再匹配色彩风格 =====
    let temp_diff = ref_feat.rb_ratio - cur_feat.rb_ratio;
    if temp_diff.abs() > 0.03 {
        let skin_temp_bias = if ref_feat.skin_ratio > 0.015 && cur_feat.skin_ratio > 0.015 {
            (ref_feat.skin_rb_ratio - cur_feat.skin_rb_ratio) * 8.0
        } else {
            0.0
        };
        let temp_delta = clamp(temp_diff * 28.0 + skin_temp_bias, -26.0, 26.0);
        let cur_temp = get_current("temperature", 0.0);
        let new_temp = clamp(cur_temp + temp_delta, -50.0, 50.0);
        suggestions.push(StyleTransferSuggestion {
            key: "temperature".to_string(),
            value: new_temp.round(),
            label: "色温".to_string(),
            min: -100.0, max: 100.0,
            reason: format!("参考图色调{}", if temp_diff > 0.0 { "偏暖" } else { "偏冷" }),
        });
    }

    // 6. 色调偏移 (tint): 保守映射
    let tint_diff = ref_feat.gb_ratio - cur_feat.gb_ratio;
    if tint_diff.abs() > 0.03 {
        let tint_delta = clamp(-tint_diff * 25.0, -20.0, 20.0);
        let cur_tint = get_current("tint", 0.0);
        let new_tint = clamp(cur_tint + tint_delta, -40.0, 40.0);
        suggestions.push(StyleTransferSuggestion {
            key: "tint".to_string(),
            value: new_tint.round(),
            label: "色调偏移".to_string(),
            min: -100.0, max: 100.0,
            reason: format!("参考图色调{}", if tint_diff > 0.0 { "偏绿" } else { "偏品红" }),
        });
    }

    // 7. 饱和度 (saturation): 保守映射
    let sat_diff = ref_feat.mean_saturation - cur_feat.mean_saturation;
    if sat_diff.abs() > 0.03 {
        let sat_delta = clamp(sat_diff * 80.0 * tuning.style_strength, -30.0, 30.0);
        let cur_sat = get_current("saturation", 0.0);
        let new_sat = clamp(cur_sat + sat_delta, -50.0, 50.0);
        suggestions.push(StyleTransferSuggestion {
            key: "saturation".to_string(),
            value: new_sat.round(),
            label: "饱和度".to_string(),
            min: -100.0, max: 100.0,
            reason: format!("参考图色彩{}", if sat_diff > 0.0 { "更鲜艳" } else { "更淡雅" }),
        });
    }

    // 8. 自然饱和度 (vibrance): 保守映射
    let vib_diff = ref_feat.saturation_spread - cur_feat.saturation_spread;
    if vib_diff.abs() > 0.02 {
        let vib_delta = clamp(vib_diff * 100.0 * tuning.style_strength, -25.0, 25.0);
        let cur_vib = get_current("vibrance", 0.0);
        let new_vib = clamp(cur_vib + vib_delta, -50.0, 50.0);
        suggestions.push(StyleTransferSuggestion {
            key: "vibrance".to_string(),
            value: new_vib.round(),
            label: "自然饱和度".to_string(),
            min: -100.0, max: 100.0,
            reason: format!("参考图色彩层次{}", if vib_diff > 0.0 { "更丰富" } else { "更统一" }),
        });
    }

    // 9. 清晰度 (clarity): 保守映射
    let lap_diff = ref_feat.laplacian_variance - cur_feat.laplacian_variance;
    if lap_diff.abs() > 100.0 {
        let clarity_delta = clamp(lap_diff / 300.0, -25.0, 25.0);
        let cur_clarity = get_current("clarity", 0.0);
        let new_clarity = clamp(cur_clarity + clarity_delta, -50.0, 50.0);
        suggestions.push(StyleTransferSuggestion {
            key: "clarity".to_string(),
            value: new_clarity.round(),
            label: "清晰度".to_string(),
            min: -100.0, max: 100.0,
            reason: format!("参考图纹理{}", if lap_diff > 0.0 { "更锐利" } else { "更柔和" }),
        });
    }

    // 10. 暗角 (vignetteAmount): 保守映射
    let vig_diff = ref_feat.vignette_diff - cur_feat.vignette_diff;
    if vig_diff.abs() > 0.03 {
        let vig_delta = clamp(-vig_diff * 50.0, -25.0, 25.0);
        let cur_vig = get_current("vignetteAmount", 0.0);
        let new_vig = clamp(cur_vig + vig_delta, -50.0, 50.0);
        suggestions.push(StyleTransferSuggestion {
            key: "vignetteAmount".to_string(),
            value: new_vig.round(),
            label: "暗角".to_string(),
            min: -100.0, max: 100.0,
            reason: format!("参考图暗角{}", if vig_diff > 0.0 { "更明显" } else { "更轻微" }),
        });
    }

    // ===== 亮度安全检查 =====
    let mut brightness_impact: f64 = 0.0;
    for s in &suggestions {
        let cur = get_current(&s.key, 0.0);
        let delta = s.value - cur;
        match s.key.as_str() {
            "exposure" => brightness_impact += delta * 42.0,
            "highlights" => brightness_impact += delta * 0.3,
            "shadows" => brightness_impact += delta * 0.3,
            "contrast" => brightness_impact += delta * 0.1,
            "whites" => brightness_impact += delta * 0.32,
            "blacks" => brightness_impact += delta * 0.22,
            _ => {}
        }
    }
    if brightness_impact.abs() > 60.0 {
        let scale = 60.0 / brightness_impact.abs();
        for s in &mut suggestions {
            match s.key.as_str() {
                "exposure" | "highlights" | "shadows" | "contrast" | "whites" | "blacks" => {
                    let cur = get_current(&s.key, 0.0);
                    let delta = s.value - cur;
                    s.value = cur + delta * scale;
                    if s.key == "exposure" {
                        s.value = (s.value * 100.0).round() / 100.0;
                    } else {
                        s.value = s.value.round();
                    }
                }
                _ => {}
            }
        }
    }

    // ===== 过曝硬保护 =====
    let predicted_p99 = cur_feat.p99_luminance
        + suggestions.iter().fold(0.0, |acc, s| {
            let cur = get_current(&s.key, 0.0);
            let delta = s.value - cur;
            acc + match s.key.as_str() {
                "exposure" => delta * 0.18,
                "highlights" => delta * 0.0020,
                "whites" => delta * 0.0023,
                "contrast" => delta * 0.0010,
                _ => 0.0,
            }
        });
    let target_p99_cap = if classify_tonal_style(ref_feat) == "高调" {
        0.985 + (1.0 - tuning.highlight_guard_strength) * 0.012
    } else {
        0.965 + (1.0 - tuning.highlight_guard_strength) * 0.012
    };
    if predicted_p99 > target_p99_cap {
        for s in &mut suggestions {
            if s.key == "exposure" {
                s.value = (s.value - 0.15 * tuning.highlight_guard_strength).max(-2.5);
            }
            if s.key == "highlights" {
                s.value = (s.value - 8.0 * tuning.highlight_guard_strength).max(-80.0);
            }
            if s.key == "whites" {
                s.value = (s.value - 10.0 * tuning.highlight_guard_strength).max(-80.0);
            }
        }
    }

    if ref_feat.skin_ratio > 0.015 && cur_feat.skin_ratio > 0.015 {
        let predicted_skin_luma = cur_feat.skin_luminance_mean
            + suggestions.iter().fold(0.0, |acc, s| {
                let cur = get_current(&s.key, 0.0);
                let delta = s.value - cur;
                acc + match s.key.as_str() {
                    "exposure" => delta * 0.16,
                    "highlights" => delta * 0.0018,
                    "whites" => delta * 0.0016,
                    "contrast" => delta * 0.0006,
                    _ => 0.0,
                }
            });
        if predicted_skin_luma > 0.78 {
            for s in &mut suggestions {
                if s.key == "exposure" {
                    s.value = (s.value - 0.10 * tuning.skin_protect_strength).max(-2.5);
                }
                if s.key == "highlights" {
                    s.value = (s.value - 6.0 * tuning.skin_protect_strength).max(-80.0);
                }
                if s.key == "whites" {
                    s.value = (s.value - 8.0 * tuning.skin_protect_strength).max(-80.0);
                }
            }
        }
    }

    let mutable_keys: Vec<String> = suggestions.iter().map(|s| s.key.clone()).collect();
    if !mutable_keys.is_empty() {
        let mut current_values: HashMap<String, f64> = HashMap::new();
        for key in &mutable_keys {
            current_values.insert(key.clone(), get_current(key, 0.0));
        }
        let mut candidate_values: HashMap<String, f64> = HashMap::new();
        let mut key_ranges: HashMap<String, (f64, f64)> = HashMap::new();
        for s in &suggestions {
            candidate_values.insert(s.key.clone(), s.value);
            key_ranges.insert(s.key.clone(), (s.min, s.max));
        }
        let mut best_score = style_distance_score(
            ref_feat,
            &estimate_features_from_adjustments(cur_feat, &current_values, &candidate_values),
            tuning,
        );
        let ref_vec = style_metric_vector(ref_feat);
        for _ in 0..3 {
            let pred_feat = estimate_features_from_adjustments(cur_feat, &current_values, &candidate_values);
            let pred_vec = style_metric_vector(&pred_feat);
            let weights = style_metric_weights(ref_feat, &pred_feat, tuning);
            let mut residual = [0.0f64; 13];
            for i in 0..13 {
                residual[i] = (ref_vec[i] - pred_vec[i]) * weights[i];
            }
            for key in &mutable_keys {
                let cur_val = candidate_values.get(key).copied().unwrap_or(0.0);
                let (min_v, max_v) = key_ranges.get(key).copied().unwrap_or((-100.0, 100.0));
                let eps = match key.as_str() {
                    "exposure" => 0.05,
                    "contrast" | "highlights" | "shadows" | "whites" | "blacks" => 2.0,
                    "temperature" | "tint" | "saturation" | "vibrance" => 2.0,
                    "clarity" => 1.5,
                    "vignetteAmount" => 1.5,
                    _ => 1.0,
                };
                candidate_values.insert(key.clone(), (cur_val + eps).max(min_v).min(max_v));
                let plus_feat = estimate_features_from_adjustments(cur_feat, &current_values, &candidate_values);
                let plus_vec = style_metric_vector(&plus_feat);
                candidate_values.insert(key.clone(), cur_val);
                let mut num = 0.0;
                let mut den = 1e-6;
                for i in 0..13 {
                    let j = (plus_vec[i] - pred_vec[i]) / eps;
                    num += j * residual[i];
                    den += j * j;
                }
                let step_limit = if key == "exposure" { 0.16 } else { 6.0 };
                let update = (num / den * 0.35).max(-step_limit).min(step_limit);
                let trial = (cur_val + update).max(min_v).min(max_v);
                candidate_values.insert(key.clone(), trial);
            }
            let score = style_distance_score(
                ref_feat,
                &estimate_features_from_adjustments(cur_feat, &current_values, &candidate_values),
                tuning,
            );
            if score + 1e-6 < best_score {
                best_score = score;
            }
        }
        let base_step = |k: &str| -> f64 {
            match k {
                "exposure" => 0.08,
                "contrast" | "highlights" | "shadows" | "whites" | "blacks" => 4.0,
                "temperature" | "tint" | "saturation" | "vibrance" => 3.0,
                "clarity" => 2.5,
                "vignetteAmount" => 2.0,
                _ => 2.0,
            }
        };
        for round in 0..4 {
            let round_scale = 1.0 - round as f64 * 0.22;
            let mut improved = false;
            for key in &mutable_keys {
                let cur_val = candidate_values.get(key).copied().unwrap_or(0.0);
                let step = (base_step(key) * round_scale).max(0.2);
                let (min_v, max_v) = key_ranges.get(key).copied().unwrap_or((-100.0, 100.0));
                for dir in [-1.0, 1.0] {
                    let trial = (cur_val + dir * step).max(min_v).min(max_v);
                    if (trial - cur_val).abs() < 1e-6 {
                        continue;
                    }
                    candidate_values.insert(key.clone(), trial);
                    let predicted = estimate_features_from_adjustments(cur_feat, &current_values, &candidate_values);
                    let score = style_distance_score(ref_feat, &predicted, tuning);
                    if score + 1e-6 < best_score {
                        best_score = score;
                        improved = true;
                    } else {
                        candidate_values.insert(key.clone(), cur_val);
                    }
                }
            }
            if !improved {
                break;
            }
        }
        for s in &mut suggestions {
            if let Some(v) = candidate_values.get(&s.key) {
                s.value = if s.key == "exposure" {
                    (v * 100.0).round() / 100.0
                } else {
                    v.round()
                };
            }
        }
    }

    suggestions
}

#[tauri::command]
pub async fn analyze_style_transfer(
    reference_path: String,
    current_image_path: String,
    current_adjustments: Value,
    style_strength: Option<f64>,
    highlight_guard_strength: Option<f64>,
    skin_protect_strength: Option<f64>,
) -> Result<StyleTransferResponse, String> {
    // 验证文件存在
    if !Path::new(&reference_path).exists() {
        return Err("参考图文件不存在".to_string());
    }
    if !Path::new(&current_image_path).exists() {
        return Err("当前图片文件不存在".to_string());
    }

    // 检查文件大小（限制 100MB）
    let ref_meta = std::fs::metadata(&reference_path).map_err(|e| format!("无法读取参考图信息: {}", e))?;
    if ref_meta.len() > 100 * 1024 * 1024 {
        return Err("参考图文件过大（超过 100MB）".to_string());
    }

    // 加载图像
    let ref_path = reference_path.clone();
    let cur_path = current_image_path.clone();

    let (ref_img, cur_img) = tokio::task::spawn_blocking(move || {
        let r = smart_open_image(&ref_path).map_err(|e| format!("无法打开参考图: {}", e))?;
        let c = smart_open_image(&cur_path).map_err(|e| format!("无法打开当前图片: {}", e))?;
        Ok::<(DynamicImage, DynamicImage), String>((r, c))
    })
    .await
    .map_err(|e| format!("图像加载任务失败: {}", e))??;

    // 提取特征
    let ref_features = extract_features(&ref_img);
    let cur_features = extract_features(&cur_img);
    let constraint_window = build_dynamic_constraint_window(&cur_features, &current_adjustments, "image");

    let tuning = StyleTransferTuning::from_options(style_strength, highlight_guard_strength, skin_protect_strength);
    let baseline_error = style_error_breakdown(&ref_features, &cur_features, tuning);
    if baseline_error.total < EARLY_EXIT_STYLE_DISTANCE_THRESHOLD {
        let style_debug = build_style_debug_info(
            &ref_features,
            &cur_features,
            &cur_features,
            tuning,
            0,
            Some(empty_constraint_debug(&constraint_window)),
        );
        return Ok(StyleTransferResponse {
            understanding: "两张图片风格差异很小，已走快速路径并跳过重度迁移。".to_string(),
            adjustments: Vec::new(),
            style_debug: Some(style_debug),
        });
    }

    let mut adjustments = map_features_to_adjustments(&ref_features, &cur_features, &current_adjustments, tuning);
    let auto_refine_rounds = auto_refine_suggestions_by_error(
        &ref_features,
        &cur_features,
        &current_adjustments,
        &mut adjustments,
        tuning,
    );
    let constraint_debug = apply_dynamic_constraints_to_style_suggestions(&mut adjustments, &constraint_window);
    let predicted_features = estimate_features_for_suggestions(&cur_features, &current_adjustments, &adjustments);
    let style_debug = build_style_debug_info(
        &ref_features,
        &cur_features,
        &predicted_features,
        tuning,
        auto_refine_rounds,
        Some(constraint_debug),
    );

    let understanding = if adjustments.is_empty() {
        "两张图片的调色风格非常接近，无需调整。".to_string()
    } else {
        format!(
            "已分析参考图风格，建议调整 {} 项参数以匹配参考图的调色风格。",
            adjustments.len()
        )
    };

    Ok(StyleTransferResponse {
        understanding,
        adjustments,
        style_debug: Some(style_debug),
    })
}

/// 将特征向量格式化为 LLM 可读的文本描述
fn describe_features(feat: &StyleFeatures, label: &str) -> String {
    let tonal_style = classify_tonal_style(feat);
    let brightness_desc = if feat.mean_luminance > 0.6 {
        "偏亮"
    } else if feat.mean_luminance < 0.4 {
        "偏暗"
    } else {
        "中等亮度"
    };
    let contrast_desc = if feat.contrast_spread > 0.25 {
        "高对比"
    } else if feat.contrast_spread < 0.15 {
        "低对比"
    } else {
        "中等对比"
    };
    let temp_desc = if feat.rb_ratio > 1.1 {
        "偏暖（橙/黄调）"
    } else if feat.rb_ratio < 0.9 {
        "偏冷（蓝调）"
    } else {
        "中性色温"
    };
    let sat_desc = if feat.mean_saturation > 0.5 {
        "高饱和"
    } else if feat.mean_saturation < 0.25 {
        "低饱和/淡雅"
    } else {
        "中等饱和"
    };
    let vig_desc = if feat.vignette_diff > 0.05 {
        "有明显暗角"
    } else {
        "无明显暗角"
    };

    format!(
        "【{}】风格影调={}；亮度={:.2}（{}），高光占比={:.1}%，阴影占比={:.1}%，对比度={:.3}（{}），\
         P10/P50/P90/P99={:.2}/{:.2}/{:.2}/{:.2}，波形低/中/高带={:.2}/{:.2}/{:.2}，过曝像素={:.2}%，\
         R/B比={:.3}（{}），饱和度={:.3}（{}），饱和度分布={:.3}，分区亮度(暗/中/亮)={:.2}/{:.2}/{:.2}，肤色占比={:.2}%（亮度={:.2}），\
         纹理方差={:.1}，暗角差={:.3}（{}）",
        label,
        tonal_style,
        feat.mean_luminance, brightness_desc,
        feat.highlight_ratio * 100.0,
        feat.shadow_ratio * 100.0,
        feat.contrast_spread, contrast_desc,
        feat.p10_luminance, feat.p50_luminance, feat.p90_luminance, feat.p99_luminance,
        feat.waveform_low_band, feat.waveform_mid_band, feat.waveform_high_band,
        feat.clipped_highlight_ratio * 100.0,
        feat.rb_ratio, temp_desc,
        feat.mean_saturation, sat_desc,
        feat.saturation_spread,
        feat.shadow_luminance_mean, feat.mid_luminance_mean, feat.highlight_luminance_mean,
        feat.skin_ratio * 100.0, feat.skin_luminance_mean,
        feat.laplacian_variance,
        feat.vignette_diff, vig_desc,
    )
}

/// 构建风格迁移的 LLM system prompt
fn build_style_transfer_prompt(
    ref_feat: &StyleFeatures,
    cur_feat: &StyleFeatures,
    algo_suggestions: &[StyleTransferSuggestion],
    current_adjustments: &Value,
    constraint_window: &DynamicConstraintWindow,
) -> String {
    let ref_desc = describe_features(ref_feat, "参考图");
    let cur_desc = describe_features(cur_feat, "当前图");
    let adj_str = serde_json::to_string_pretty(current_adjustments).unwrap_or_default();
    let constraint_str = serde_json::to_string_pretty(constraint_window).unwrap_or_default();

    let algo_desc = if algo_suggestions.is_empty() {
        "算法未检测到显著差异。".to_string()
    } else {
        algo_suggestions
            .iter()
            .map(|s| format!("  - {}({}): {} → 建议值 {}（{}）", s.label, s.key, s.reason, s.value, s.reason))
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!(
        r#"你是一位专业摄影后期调色师。用户想把一张参考图的调色风格复刻到当前图片上。

## 图像统计分析结果
{ref_desc}
{cur_desc}

## 算法初步建议
{algo_desc}

## 当前图片的滑块参数
```json
{adj_str}
```

## 动态约束窗口（必须遵守）
```json
{constraint_str}
```

## 你的任务
1. 先判断参考图与当前图的风格影调类型（高调/低调/中间调），并优先做“影调区间匹配”
2. 影调匹配顺序必须是：exposure/highlights/shadows/whites/blacks/contrast（先波形和亮度分位，再颜色）
3. 只有在影调区间接近后，才调整 temperature/tint/saturation/vibrance 等色彩参数
4. value 是最终绝对值，不是增量
5. 重要：绝不允许为了追色而导致过曝。控制 P99 与过曝像素比例，不要让高光大面积贴顶
6. 参数值必须合理，避免极端值。大多数参数应在 ±50 以内，除非参考图风格确实极端
7. 如果两张图差异不大，只做微调（±5~15），不要过度调整

## 参数范围
- exposure: -5.0 ~ 5.0（步长 0.01）
- brightness: -100 ~ 100
- contrast: -100 ~ 100
- highlights: -100 ~ 100
- shadows: -100 ~ 100
- whites: -100 ~ 100
- blacks: -100 ~ 100
- saturation: -100 ~ 100
- vibrance: -100 ~ 100
- temperature: -100 ~ 100（负=冷/蓝，正=暖/橙）
- tint: -100 ~ 100（负=绿，正=品红）
- clarity: -100 ~ 100
- dehaze: -100 ~ 100
- structure: -100 ~ 100
- sharpness: 0 ~ 100
- vignetteAmount: -100 ~ 100（负=暗角）

## 输出格式（严格 JSON）
{{
  "understanding": "用 1-2 句话描述参考图的风格特征和你的调色思路",
  "adjustments": [
    {{
      "key": "参数键名",
      "value": 数值,
      "label": "中文参数名",
      "min": 最小值,
      "max": 最大值,
      "reason": "调整原因（简短）"
    }}
  ]
}}"#,
        ref_desc = ref_desc,
        cur_desc = cur_desc,
        algo_desc = algo_desc,
        adj_str = adj_str,
        constraint_str = constraint_str,
    )
}

/// 剥离 <think>...</think> 标签（复用 llm_chat 的逻辑）
fn strip_thinking_tags(text: &str) -> String {
    let mut result = text.to_string();
    while let Some(start) = result.find("<think>") {
        if let Some(end) = result.find("</think>") {
            let end_pos = end + "</think>".len();
            result = format!("{}{}", &result[..start], &result[end_pos..]);
        } else {
            result = result[..start].to_string();
            break;
        }
    }
    result.trim().to_string()
}

/// 从文本中提取 JSON
fn extract_json_from_response(text: &str) -> Result<String, String> {
    let cleaned = strip_thinking_tags(text);

    if serde_json::from_str::<Value>(&cleaned).is_ok() {
        return Ok(cleaned);
    }

    // markdown 代码块
    if let Some(start) = cleaned.find("```json") {
        let after = &cleaned[start + 7..];
        if let Some(end) = after.find("```") {
            let candidate = after[..end].trim();
            if serde_json::from_str::<Value>(candidate).is_ok() {
                return Ok(candidate.to_string());
            }
        }
    }
    if let Some(start) = cleaned.find("```\n") {
        let after = &cleaned[start + 4..];
        if let Some(end) = after.find("```") {
            let candidate = after[..end].trim();
            if serde_json::from_str::<Value>(candidate).is_ok() {
                return Ok(candidate.to_string());
            }
        }
    }

    // 花括号匹配
    if let (Some(start), Some(end)) = (cleaned.find('{'), cleaned.rfind('}')) {
        if start < end {
            let candidate = &cleaned[start..=end];
            if serde_json::from_str::<Value>(candidate).is_ok() {
                return Ok(candidate.to_string());
            }
        }
    }

    Err(format!("无法从 LLM 响应中提取 JSON: {}", &cleaned[..cleaned.len().min(200)]))
}

/// 调用 LLM 增强风格迁移结果（流式推送思考过程）
async fn enhance_with_llm(
    ref_img: &DynamicImage,
    cur_img: &DynamicImage,
    ref_feat: &StyleFeatures,
    cur_feat: &StyleFeatures,
    algo_suggestions: &[StyleTransferSuggestion],
    current_adjustments: &Value,
    constraint_window: &DynamicConstraintWindow,
    llm_endpoint: &str,
    llm_api_key: Option<&str>,
    llm_model: &str,
    app_handle: &tauri::AppHandle,
) -> Result<StyleTransferResponse, String> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(180))
        .build()
        .map_err(|e| e.to_string())?;

    let system_prompt = build_style_transfer_prompt(
        ref_feat,
        cur_feat,
        algo_suggestions,
        current_adjustments,
        constraint_window,
    );

    let user_message = if is_vision_model(llm_model) {
        match (
            encode_image_for_vision_model(ref_img),
            encode_image_for_vision_model(cur_img),
        ) {
            (Ok(ref_b64), Ok(cur_b64)) => json!({
                "role": "user",
                "content": [
                    { "type": "text", "text": "请基于两张图片和统计特征完成风格迁移参数建议。第一张是参考图，第二张是待迁移图片。" },
                    { "type": "image_url", "image_url": { "url": format!("data:image/jpeg;base64,{}", ref_b64) } },
                    { "type": "text", "text": "上图是参考图。" },
                    { "type": "image_url", "image_url": { "url": format!("data:image/jpeg;base64,{}", cur_b64) } },
                    { "type": "text", "text": "上图是待迁移图片。请输出严格 JSON。" }
                ]
            }),
            _ => json!({
                "role": "user",
                "content": "请分析参考图的调色风格，给出优化后的调整参数。"
            }),
        }
    } else {
        json!({
            "role": "user",
            "content": "请分析参考图的调色风格，给出优化后的调整参数。"
        })
    };

    let messages = vec![
        json!({ "role": "system", "content": system_prompt }),
        user_message,
    ];

    let endpoint = llm_endpoint.trim_end_matches('/');
    let url = format!("{}/v1/chat/completions", endpoint);

    // 使用流式请求
    let request_body = json!({
        "model": llm_model,
        "messages": messages,
        "temperature": 0.3,
        "stream": true
    });

    let mut req = client.post(&url).json(&request_body);
    if let Some(key) = llm_api_key {
        if !key.is_empty() {
            req = req.bearer_auth(key);
        }
    }

    let response = req.send().await.map_err(|e| format!("LLM 请求失败: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("LLM 返回错误 {}: {}", status, body));
    }

    // 逐行读取 SSE 流，推送思考过程
    let mut full_content = String::new();
    let mut in_thinking = false;

    use futures_util::StreamExt;
    let mut stream = response.bytes_stream();
    let mut line_buffer = String::new();

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.map_err(|e| format!("流读取错误: {}", e))?;
        let chunk_str = String::from_utf8_lossy(&chunk);
        line_buffer.push_str(&chunk_str);

        while let Some(newline_pos) = line_buffer.find('\n') {
            let line = line_buffer[..newline_pos].trim().to_string();
            line_buffer = line_buffer[newline_pos + 1..].to_string();

            if line.is_empty() || line == "data: [DONE]" {
                continue;
            }

            if let Some(json_str) = line.strip_prefix("data: ") {
                if let Ok(sse_json) = serde_json::from_str::<Value>(json_str) {
                    if let Some(delta_content) = sse_json["choices"][0]["delta"]["content"].as_str() {
                        if delta_content.is_empty() {
                            continue;
                        }

                        full_content.push_str(delta_content);

                        if delta_content.contains("<think>") {
                            in_thinking = true;
                        }

                        if in_thinking {
                            let clean = delta_content.replace("<think>", "").replace("</think>", "");
                            if !clean.is_empty() {
                                let _ = app_handle.emit("style-transfer-stream", StreamChunkPayload {
                                    chunk_type: "thinking".to_string(),
                                    text: clean,
                                    result: None,
                                });
                            }
                        }

                        if delta_content.contains("</think>") {
                            in_thinking = false;
                            // 思考结束，推送过渡提示
                            let _ = app_handle.emit("style-transfer-stream", StreamChunkPayload {
                                chunk_type: "thinking".to_string(),
                                text: "\n正在生成调整参数...\n".to_string(),
                                result: None,
                            });
                        }

                        // 非思考内容是 JSON 格式，不推送给前端显示
                        // 前端会在 done 事件中获取解析后的自然语言 understanding
                    }
                }
            }
        }
    }

    // 流结束，解析完整内容
    let json_str = extract_json_from_response(&full_content)?;
    let parsed: StyleTransferResponse = serde_json::from_str(&json_str)
        .map_err(|e| format!("解析风格迁移 JSON 失败: {}，原始: {}", e, &full_content[..full_content.len().min(500)]))?;

    Ok(parsed)
}

/// 带 LLM 增强的风格迁移分析命令（流式推送思考过程）
#[tauri::command]
pub async fn analyze_style_transfer_with_llm(
    reference_path: String,
    current_image_path: String,
    current_adjustments: Value,
    llm_endpoint: String,
    llm_api_key: Option<String>,
    llm_model: Option<String>,
    style_strength: Option<f64>,
    highlight_guard_strength: Option<f64>,
    skin_protect_strength: Option<f64>,
    app_handle: tauri::AppHandle,
) -> Result<StyleTransferResponse, String> {
    // 先执行纯算法分析
    if !Path::new(&reference_path).exists() {
        return Err("参考图文件不存在".to_string());
    }
    if !Path::new(&current_image_path).exists() {
        return Err("当前图片文件不存在".to_string());
    }

    let ref_meta = std::fs::metadata(&reference_path).map_err(|e| format!("无法读取参考图信息: {}", e))?;
    if ref_meta.len() > 100 * 1024 * 1024 {
        return Err("参考图文件过大（超过 100MB）".to_string());
    }

    // 推送：正在加载图像
    let _ = app_handle.emit("style-transfer-stream", StreamChunkPayload {
        chunk_type: "thinking".to_string(),
        text: "正在加载参考图和当前图片...\n".to_string(),
        result: None,
    });

    let ref_path = reference_path.clone();
    let cur_path = current_image_path.clone();

    let (ref_img, cur_img) = tokio::task::spawn_blocking(move || {
        let r = smart_open_image(&ref_path).map_err(|e| format!("无法打开参考图: {}", e))?;
        let c = smart_open_image(&cur_path).map_err(|e| format!("无法打开当前图片: {}", e))?;
        Ok::<(DynamicImage, DynamicImage), String>((r, c))
    })
    .await
    .map_err(|e| format!("图像加载任务失败: {}", e))??;

    // 推送：正在提取特征
    let _ = app_handle.emit("style-transfer-stream", StreamChunkPayload {
        chunk_type: "thinking".to_string(),
        text: "正在提取图像色彩特征（亮度、对比度、色温、饱和度等）...\n".to_string(),
        result: None,
    });

    let ref_features = extract_features(&ref_img);
    let cur_features = extract_features(&cur_img);
    let constraint_window = build_dynamic_constraint_window(&cur_features, &current_adjustments, "image");
    let tuning = StyleTransferTuning::from_options(style_strength, highlight_guard_strength, skin_protect_strength);
    let baseline_error = style_error_breakdown(&ref_features, &cur_features, tuning);
    if baseline_error.total < EARLY_EXIT_STYLE_DISTANCE_THRESHOLD {
        let style_debug = build_style_debug_info(
            &ref_features,
            &cur_features,
            &cur_features,
            tuning,
            0,
            Some(empty_constraint_debug(&constraint_window)),
        );
        let understanding = "两张图片风格差异很小，已走快速路径并跳过重度迁移。".to_string();
        let result = StyleTransferResponse {
            understanding: understanding.clone(),
            adjustments: Vec::new(),
            style_debug: Some(style_debug.clone()),
        };
        let _ = app_handle.emit("style-transfer-stream", StreamChunkPayload {
            chunk_type: "done".to_string(),
            text: String::new(),
            result: Some(crate::llm_chat::ChatAdjustResponse {
                understanding,
                adjustments: Vec::new(),
                style_debug: serde_json::to_value(&style_debug).ok(),
                constraint_debug: style_debug
                    .constraint_debug
                    .as_ref()
                    .and_then(|v| serde_json::to_value(v).ok()),
            }),
        });
        return Ok(result);
    }
    let mut algo_suggestions =
        map_features_to_adjustments(&ref_features, &cur_features, &current_adjustments, tuning);
    let algo_auto_refine_rounds = auto_refine_suggestions_by_error(
        &ref_features,
        &cur_features,
        &current_adjustments,
        &mut algo_suggestions,
        tuning,
    );
    let algo_constraint_debug =
        apply_dynamic_constraints_to_style_suggestions(&mut algo_suggestions, &constraint_window);
    let algo_predicted_features =
        estimate_features_for_suggestions(&cur_features, &current_adjustments, &algo_suggestions);
    let algo_style_debug = build_style_debug_info(
        &ref_features,
        &cur_features,
        &algo_predicted_features,
        tuning,
        algo_auto_refine_rounds,
        Some(algo_constraint_debug.clone()),
    );
    if algo_style_debug.after.total < LLM_TRIGGER_STYLE_DISTANCE_THRESHOLD {
        let understanding = if algo_suggestions.is_empty() {
            "两张图片的调色风格非常接近，无需调整。（残差较低，跳过 LLM 增强）".to_string()
        } else {
            format!(
                "已分析参考图风格，建议调整 {} 项参数。（残差较低，跳过 LLM 增强）",
                algo_suggestions.len()
            )
        };
        let result = StyleTransferResponse {
            understanding: understanding.clone(),
            adjustments: algo_suggestions.clone(),
            style_debug: Some(algo_style_debug.clone()),
        };
        let _ = app_handle.emit("style-transfer-stream", StreamChunkPayload {
            chunk_type: "done".to_string(),
            text: String::new(),
            result: Some(crate::llm_chat::ChatAdjustResponse {
                understanding,
                adjustments: result
                    .adjustments
                    .iter()
                    .map(|s| crate::llm_chat::AdjustmentSuggestion {
                        key: s.key.clone(),
                        value: s.value,
                        label: s.label.clone(),
                        min: s.min,
                        max: s.max,
                        reason: s.reason.clone(),
                    })
                    .collect(),
                style_debug: serde_json::to_value(&algo_style_debug).ok(),
                constraint_debug: algo_style_debug
                    .constraint_debug
                    .as_ref()
                    .and_then(|v| serde_json::to_value(v).ok()),
            }),
        });
        return Ok(result);
    }

    let _ = app_handle.emit("style-transfer-stream", StreamChunkPayload {
        chunk_type: "thinking".to_string(),
        text: format!("算法分析完成，初步建议 {} 项调整。正在请求 AI 优化...\n", algo_suggestions.len()),
        result: None,
    });

    // 尝试 LLM 增强
    let model = resolve_style_transfer_model(llm_model);
    match enhance_with_llm(
        &ref_img,
        &cur_img,
        &ref_features,
        &cur_features,
        &algo_suggestions,
        &current_adjustments,
        &constraint_window,
        &llm_endpoint,
        llm_api_key.as_deref(),
        &model,
        &app_handle,
    )
    .await
    {
        Ok(mut llm_result) => {
            let llm_constraint_debug =
                apply_dynamic_constraints_to_style_suggestions(&mut llm_result.adjustments, &constraint_window);
            let llm_predicted_features =
                estimate_features_for_suggestions(&cur_features, &current_adjustments, &llm_result.adjustments);
            let llm_style_debug = build_style_debug_info(
                &ref_features,
                &cur_features,
                &llm_predicted_features,
                tuning,
                0,
                Some(llm_constraint_debug),
            );
            llm_result.style_debug = Some(llm_style_debug.clone());
            // 推送完成事件
            let _ = app_handle.emit("style-transfer-stream", StreamChunkPayload {
                chunk_type: "done".to_string(),
                text: String::new(),
                result: Some(crate::llm_chat::ChatAdjustResponse {
                    understanding: llm_result.understanding.clone(),
                    adjustments: llm_result.adjustments.iter().map(|s| crate::llm_chat::AdjustmentSuggestion {
                        key: s.key.clone(),
                        value: s.value,
                        label: s.label.clone(),
                        min: s.min,
                        max: s.max,
                        reason: s.reason.clone(),
                    }).collect(),
                    style_debug: serde_json::to_value(&llm_style_debug).ok(),
                    constraint_debug: llm_style_debug
                        .constraint_debug
                        .as_ref()
                        .and_then(|v| serde_json::to_value(v).ok()),
                }),
            });
            Ok(llm_result)
        }
        Err(llm_err) => {
            // LLM 失败时回退到纯算法结果
            log::warn!("LLM 风格增强失败，回退到算法结果: {}", llm_err);
            let understanding = if algo_suggestions.is_empty() {
                "两张图片的调色风格非常接近，无需调整。（LLM 不可用，使用纯算法分析）".to_string()
            } else {
                format!(
                    "已分析参考图风格，建议调整 {} 项参数。（LLM 不可用，使用纯算法分析）",
                    algo_suggestions.len()
                )
            };
            let result = StyleTransferResponse {
                understanding: understanding.clone(),
                adjustments: algo_suggestions.clone(),
                style_debug: Some(algo_style_debug.clone()),
            };
            // 推送完成事件（算法回退）
            let _ = app_handle.emit("style-transfer-stream", StreamChunkPayload {
                chunk_type: "done".to_string(),
                text: String::new(),
                result: Some(crate::llm_chat::ChatAdjustResponse {
                    understanding,
                    adjustments: result.adjustments.iter().map(|s| crate::llm_chat::AdjustmentSuggestion {
                        key: s.key.clone(),
                        value: s.value,
                        label: s.label.clone(),
                        min: s.min,
                        max: s.max,
                        reason: s.reason.clone(),
                    }).collect(),
                    style_debug: serde_json::to_value(&algo_style_debug).ok(),
                    constraint_debug: algo_style_debug
                        .constraint_debug
                        .as_ref()
                        .and_then(|v| serde_json::to_value(v).ok()),
                }),
            });
            Ok(result)
        }
    }
}
