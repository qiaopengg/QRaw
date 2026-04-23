use crate::expert_presets::{derive_style_tags, get_expert_preset_by_id, select_expert_preset};
use base64::{Engine as _, engine::general_purpose};
use image::{DynamicImage, GenericImageView};
use rawler::decoders::RawDecodeParams;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::Cursor;
use std::path::Path;
use tauri::Emitter;

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
    pub hue_mean: f64,
    pub hue_spread: f64,
    pub laplacian_variance: f64,
    pub vignette_diff: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StyleTransferSuggestion {
    pub key: String,
    pub value: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub complex_value: Option<serde_json::Value>,
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
    pub scene_profile: Option<StyleSceneProfileDebug>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constraint_debug: Option<DynamicConstraintDebugInfo>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StyleSceneProfileDebug {
    pub reference_tonal_style: String,
    pub current_tonal_style: String,
    pub tonal_gain: f64,
    pub highlight_gain: f64,
    pub shadow_gain: f64,
    pub chroma_limit: f64,
    pub chroma_guard_floor: f64,
    pub color_residual_gain: f64,
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

struct StyleTransferPreparedContext {
    ref_img: DynamicImage,
    cur_img: DynamicImage,
    ref_features: StyleFeatures,
    cur_features: StyleFeatures,
    constraint_window: DynamicConstraintWindow,
    tuning: StyleTransferTuning,
    baseline_error: StyleTransferErrorBreakdown,
    expert_tags: Vec<&'static str>,
    expert_preset_id: Option<&'static str>,
}

#[derive(Clone, Copy)]
struct StyleTransferAlgoOptions {
    pure_algorithm: bool,
    enable_feature_mapping: bool,
    enable_auto_refine: bool,
    enable_expert_preset: bool,
    enable_lut: bool,
}

struct AlgorithmPipelineResult {
    adjustments: Vec<StyleTransferSuggestion>,
    style_debug: StyleTransferDebugInfo,
}

fn validate_style_transfer_paths(
    reference_path: &str,
    current_image_path: &str,
) -> Result<(), String> {
    if !Path::new(reference_path).exists() {
        return Err("参考图文件不存在".to_string());
    }
    if !Path::new(current_image_path).exists() {
        return Err("当前图片文件不存在".to_string());
    }
    let ref_meta =
        std::fs::metadata(reference_path).map_err(|e| format!("无法读取参考图信息: {}", e))?;
    if ref_meta.len() > 100 * 1024 * 1024 {
        return Err("参考图文件过大（超过 100MB）".to_string());
    }
    Ok(())
}

async fn load_style_transfer_images(
    reference_path: &str,
    current_image_path: &str,
) -> Result<(DynamicImage, DynamicImage), String> {
    let ref_path = reference_path.to_string();
    let cur_path = current_image_path.to_string();
    tokio::task::spawn_blocking(move || {
        let r = smart_open_image(&ref_path).map_err(|e| format!("无法打开参考图: {}", e))?;
        let c = smart_open_image(&cur_path).map_err(|e| format!("无法打开当前图片: {}", e))?;
        Ok::<(DynamicImage, DynamicImage), String>((r, c))
    })
    .await
    .map_err(|e| format!("图像加载任务失败: {}", e))?
}

fn build_style_transfer_context(
    ref_img: DynamicImage,
    cur_img: DynamicImage,
    current_adjustments: &Value,
    tuning: StyleTransferTuning,
    enable_expert_preset: bool,
) -> StyleTransferPreparedContext {
    let ref_features = extract_features(&ref_img);
    let cur_features = extract_features(&cur_img);
    let (expert_tags, expert_preset_id) = if enable_expert_preset {
        let tags = derive_style_tags(
            ref_features.mean_luminance,
            ref_features.p10_luminance,
            ref_features.p90_luminance,
            ref_features.contrast_spread,
            ref_features.mean_saturation,
            ref_features.rb_ratio,
        );
        let preset_id = select_expert_preset(&tags).map(|p| p.id);
        (tags, preset_id)
    } else {
        (Vec::new(), None)
    };
    let constraint_window = build_dynamic_constraint_window_for_style_transfer(
        &cur_features,
        &ref_features,
        current_adjustments,
        "image+reference",
    );
    let baseline_error = style_error_breakdown(&ref_features, &cur_features, tuning);
    StyleTransferPreparedContext {
        ref_img,
        cur_img,
        ref_features,
        cur_features,
        constraint_window,
        tuning,
        baseline_error,
        expert_tags,
        expert_preset_id,
    }
}

fn merge_shallow_objects(base: &Value, overlay: &Value) -> Value {
    match (base.as_object(), overlay.as_object()) {
        (Some(b), Some(o)) => {
            let mut merged = b.clone();
            for (k, v) in o.iter() {
                merged.insert(k.clone(), v.clone());
            }
            Value::Object(merged)
        }
        _ => base.clone(),
    }
}

fn apply_numeric_suggestions(base: &Value, suggestions: &[StyleTransferSuggestion]) -> Value {
    let Some(obj) = base.as_object() else {
        return base.clone();
    };
    let mut merged = obj.clone();
    for s in suggestions {
        if s.complex_value.is_some() {
            continue;
        }
        merged.insert(s.key.clone(), json!(s.value));
    }
    Value::Object(merged)
}

fn run_algorithm_pipeline(
    ctx: &StyleTransferPreparedContext,
    current_adjustments: &Value,
    options: StyleTransferAlgoOptions,
) -> AlgorithmPipelineResult {
    let (seeded_adjustments, expert_preset) = if options.enable_expert_preset {
        ctx.expert_preset_id
            .and_then(|id| get_expert_preset_by_id(id))
            .map(|preset| {
                (
                    merge_shallow_objects(current_adjustments, &preset.adjustments),
                    Some(preset),
                )
            })
            .unwrap_or_else(|| (current_adjustments.clone(), None))
    } else {
        (current_adjustments.clone(), None)
    };

    let mut adjustments = if options.enable_feature_mapping {
        map_features_to_adjustments(
            &ctx.ref_features,
            &ctx.cur_features,
            &seeded_adjustments,
            ctx.tuning,
        )
    } else {
        Vec::new()
    };

    let matched_curves = generate_matched_curves(
        &ctx.ref_img,
        &ctx.cur_img,
        current_adjustments,
        ctx.tuning.style_strength,
    );
    let matched_hsl = generate_matched_hsl(
        &ctx.ref_img,
        &ctx.cur_img,
        ctx.tuning.style_strength,
        &ctx.constraint_window,
    );

    adjustments.push(StyleTransferSuggestion {
        key: "curves".to_string(),
        value: 0.0,
        complex_value: Some(matched_curves),
        label: "色彩曲线 (Curves)".to_string(),
        min: 0.0,
        max: 1.0,
        reason: "低权重亮度映射 + 曲线微调（避免强直方图匹配造成偏灰/断层）".to_string(),
    });
    if matched_hsl
        .as_object()
        .map(|o| !o.is_empty())
        .unwrap_or(false)
    {
        adjustments.push(StyleTransferSuggestion {
            key: "hsl".to_string(),
            value: 0.0,
            complex_value: Some(matched_hsl),
            label: "颜色混合器 (HSL)".to_string(),
            min: -100.0,
            max: 100.0,
            reason: "对齐参考图的8色相区间色相/饱和/明度画像，实现颜色风格迁移".to_string(),
        });
    }

    if options.pure_algorithm {
        let predicted_features =
            estimate_features_for_suggestions(&ctx.cur_features, current_adjustments, &adjustments);
        let style_debug = build_style_debug_info(
            &ctx.ref_features,
            &ctx.cur_features,
            &predicted_features,
            ctx.tuning,
            0,
            None,
        );
        return AlgorithmPipelineResult {
            adjustments,
            style_debug,
        };
    }
    let auto_refine_rounds = if options.enable_auto_refine {
        auto_refine_suggestions_by_error(
            &ctx.ref_features,
            &ctx.cur_features,
            &seeded_adjustments,
            &mut adjustments,
            ctx.tuning,
        )
    } else {
        0
    };
    let mut constraint_debug = if options.pure_algorithm {
        None
    } else {
        apply_soft_dynamic_constraints_to_style_suggestions(
            &mut adjustments,
            &ctx.constraint_window,
        );
        Some(apply_dynamic_constraints_to_style_suggestions(
            &mut adjustments,
            &ctx.constraint_window,
        ))
    };
    let mut predicted_features =
        estimate_features_for_suggestions(&ctx.cur_features, current_adjustments, &adjustments);

    let mut extra_refine_rounds: u32 = 0;
    let residual = style_error_breakdown(&ctx.ref_features, &predicted_features, ctx.tuning).total;
    if options.enable_auto_refine && options.enable_feature_mapping && residual > 0.40 {
        let next_adjustments_value = apply_numeric_suggestions(&seeded_adjustments, &adjustments);
        let second_pass = map_features_to_adjustments(
            &ctx.ref_features,
            &predicted_features,
            &next_adjustments_value,
            ctx.tuning,
        );
        let mut keyed: HashMap<String, usize> = HashMap::new();
        for (idx, s) in adjustments.iter().enumerate() {
            keyed.insert(s.key.clone(), idx);
        }
        for s2 in second_pass {
            if s2.complex_value.is_some() {
                continue;
            }
            if let Some(idx) = keyed.get(&s2.key).copied() {
                let base_v = adjustments[idx].value;
                adjustments[idx].value = base_v * 0.6 + s2.value * 0.4;
            } else {
                keyed.insert(s2.key.clone(), adjustments.len());
                adjustments.push(s2);
            }
        }
        if !options.pure_algorithm {
            apply_soft_dynamic_constraints_to_style_suggestions(
                &mut adjustments,
                &ctx.constraint_window,
            );
            constraint_debug = Some(apply_dynamic_constraints_to_style_suggestions(
                &mut adjustments,
                &ctx.constraint_window,
            ));
        }
        predicted_features =
            estimate_features_for_suggestions(&ctx.cur_features, current_adjustments, &adjustments);
        extra_refine_rounds = 1;
    }
    let style_debug = build_style_debug_info(
        &ctx.ref_features,
        &ctx.cur_features,
        &predicted_features,
        ctx.tuning,
        auto_refine_rounds + extra_refine_rounds,
        constraint_debug,
    );
    if options.enable_expert_preset {
        if let Some(preset) = expert_preset {
            let mut existing_keys: HashMap<String, bool> = HashMap::new();
            for s in adjustments.iter() {
                existing_keys.insert(s.key.clone(), true);
            }
            if let Some(map) = preset.adjustments.as_object() {
                for (k, v) in map.iter() {
                    if existing_keys.contains_key(k) {
                        continue;
                    }
                    if let Some(num) = v.as_f64() {
                        if let Some((label, min, max)) = action_meta(k) {
                            adjustments.push(StyleTransferSuggestion {
                                key: k.clone(),
                                value: num,
                                complex_value: None,
                                label: label.to_string(),
                                min,
                                max,
                                reason: format!("专家预设：{}", preset.name),
                            });
                        }
                    } else {
                        let label = action_meta(k)
                            .map(|(l, _, _)| l.to_string())
                            .unwrap_or_else(|| format!("专家预设：{}", preset.name));
                        adjustments.push(StyleTransferSuggestion {
                            key: k.clone(),
                            value: 0.0,
                            complex_value: Some(v.clone()),
                            label,
                            min: 0.0,
                            max: 1.0,
                            reason: format!("专家预设：{}", preset.name),
                        });
                    }
                }
            }
        }
    }

    if options.enable_lut {
        if let Some(lut_data) = generate_3d_lut_with_ot_tps(&ctx.cur_img, &ctx.ref_img, 17) {
            adjustments.push(StyleTransferSuggestion {
                key: "lutSize".to_string(),
                value: 17.0,
                complex_value: None,
                label: "LUT Size".to_string(),
                min: 0.0,
                max: 64.0,
                reason: "".to_string(),
            });
            adjustments.push(StyleTransferSuggestion {
            key: "lutData".to_string(),
            value: 1.0,
            complex_value: Some(serde_json::json!(lut_data)),
            label: "AI 色彩映射 (3D LUT)".to_string(),
            min: 0.0,
            max: 1.0,
            reason: "基于最优传输算法和薄板样条插值(TPS)生成的 3D 色彩查找表，用于精准匹配参考图的非线性色彩风格".to_string(),
        });
            adjustments.push(StyleTransferSuggestion {
                key: "lutIntensity".to_string(),
                value: 100.0,
                complex_value: None,
                label: "LUT Intensity".to_string(),
                min: 0.0,
                max: 100.0,
                reason: "".to_string(),
            });
        }
    }
    AlgorithmPipelineResult {
        adjustments,
        style_debug,
    }
}

fn build_style_transfer_early_exit_response(
    ctx: &StyleTransferPreparedContext,
) -> StyleTransferResponse {
    let style_debug = build_style_debug_info(
        &ctx.ref_features,
        &ctx.cur_features,
        &ctx.cur_features,
        ctx.tuning,
        0,
        Some(empty_constraint_debug(&ctx.constraint_window)),
    );
    StyleTransferResponse {
        understanding: "两张图片风格差异很小，已走快速路径并跳过重度迁移。".to_string(),
        adjustments: Vec::new(),
        style_debug: Some(style_debug),
    }
}

fn build_expert_preset_suffix(ctx: &StyleTransferPreparedContext) -> String {
    if let Some(id) = ctx.expert_preset_id {
        if let Some(preset) = get_expert_preset_by_id(id) {
            let tags = if ctx.expert_tags.is_empty() {
                String::new()
            } else {
                format!(" · {}", ctx.expert_tags.join("、"))
            };
            return format!("（专家预设：{}{}）", preset.name, tags);
        }
    }
    String::new()
}

fn build_algorithm_understanding(adjustment_count: usize, suffix: &str) -> String {
    if adjustment_count == 0 {
        format!("两张图片的调色风格非常接近，无需调整。{}", suffix)
    } else {
        format!(
            "已分析参考图风格，建议调整 {} 项参数。{}",
            adjustment_count, suffix
        )
    }
}

fn build_algorithm_response(
    adjustments: Vec<StyleTransferSuggestion>,
    style_debug: StyleTransferDebugInfo,
    suffix: &str,
    detailed: bool,
) -> StyleTransferResponse {
    let understanding = if adjustments.is_empty() {
        format!("两张图片的调色风格非常接近，无需调整。{}", suffix)
    } else if detailed {
        format!(
            "已分析参考图风格，建议调整 {} 项参数以匹配参考图的调色风格。{}",
            adjustments.len(),
            suffix
        )
    } else {
        build_algorithm_understanding(adjustments.len(), suffix)
    };
    StyleTransferResponse {
        understanding,
        adjustments,
        style_debug: Some(style_debug),
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

fn style_intensity_score(ref_feat: &StyleFeatures, cur_feat: &StyleFeatures) -> f64 {
    let tonal = (ref_feat.p50_luminance - cur_feat.p50_luminance).abs() * 2.0
        + (ref_feat.waveform_high_band - cur_feat.waveform_high_band).abs() * 1.6;
    let color = (ref_feat.mean_saturation - cur_feat.mean_saturation).abs() * 1.7
        + (ref_feat.rb_ratio - cur_feat.rb_ratio).abs() * 0.9
        + (ref_feat.gb_ratio - cur_feat.gb_ratio).abs() * 0.9
        + (ref_feat.hue_mean - cur_feat.hue_mean).abs() * 1.2;
    clamp01((tonal + color) * 0.62)
}

fn soft_pull_ratio_for_key(key: &str) -> f64 {
    match key {
        "exposure" => 0.30,
        "whites" | "highlights" => 0.34,
        "shadows" | "blacks" => 0.38,
        "contrast" => 0.40,
        "temperature" | "tint" => 0.48,
        "saturation" | "vibrance" => 0.52,
        _ => 0.45,
    }
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

fn adjustment_value(current_adjustments: &Value, key: &str) -> f64 {
    current_adjustments
        .get(key)
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0)
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

    let mean_luma = (0.5 + exposure * 0.08 + brightness * 0.003)
        .max(0.08)
        .min(0.92);
    let p10 = (mean_luma - 0.20 - contrast * 0.0012).max(0.01).min(0.65);
    let p50 = mean_luma.max(0.06).min(0.94);
    let p90 = (mean_luma + 0.22 + contrast * 0.0011).max(0.22).min(0.98);
    let p99 = (p90 + 0.06).max(0.40).min(0.995);
    let sat = (0.32 + saturation * 0.0024 + vibrance * 0.0018)
        .max(0.04)
        .min(0.88);

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
        hue_mean: 0.0,
        hue_spread: 0.0,
        laplacian_variance: 220.0,
        vignette_diff: 0.0,
    }
}

pub fn build_dynamic_constraint_window(
    cur_feat: &StyleFeatures,
    current_adjustments: &Value,
    source: &str,
) -> DynamicConstraintWindow {
    build_dynamic_constraint_window_with_reference(cur_feat, current_adjustments, source, None)
}

pub fn build_dynamic_constraint_window_for_style_transfer(
    cur_feat: &StyleFeatures,
    reference_feat: &StyleFeatures,
    current_adjustments: &Value,
    source: &str,
) -> DynamicConstraintWindow {
    build_dynamic_constraint_window_with_reference(
        cur_feat,
        current_adjustments,
        source,
        Some(reference_feat),
    )
}

fn build_dynamic_constraint_window_with_reference(
    cur_feat: &StyleFeatures,
    current_adjustments: &Value,
    source: &str,
    reference_feat: Option<&StyleFeatures>,
) -> DynamicConstraintWindow {
    let exposure_now = adjustment_value(current_adjustments, "exposure");
    let highlights_now = adjustment_value(current_adjustments, "highlights");
    let whites_now = adjustment_value(current_adjustments, "whites");
    let shadows_now = adjustment_value(current_adjustments, "shadows");
    let blacks_now = adjustment_value(current_adjustments, "blacks");
    let saturation_now = adjustment_value(current_adjustments, "saturation");
    let vibrance_now = adjustment_value(current_adjustments, "vibrance");

    let base_highlight_risk = ((cur_feat.p99_luminance - 0.95) / 0.05) * 0.62
        + ((cur_feat.clipped_highlight_ratio - 0.008) / 0.02) * 0.38;
    let base_shadow_risk = ((0.08 - cur_feat.p10_luminance) / 0.08) * 0.65
        + ((cur_feat.shadow_ratio - 0.42) / 0.30) * 0.35;
    let base_saturation_risk = ((cur_feat.mean_saturation - 0.58) / 0.23) * 0.65
        + ((cur_feat.saturation_spread - 0.25) / 0.15) * 0.35;

    let mut highlight_risk = clamp01(
        base_highlight_risk
            + exposure_now.max(0.0) * 0.08
            + highlights_now.max(0.0) * 0.0009
            + whites_now.max(0.0) * 0.0011,
    );
    let shadow_risk = clamp01(
        base_shadow_risk
            + (-exposure_now).max(0.0) * 0.14
            + (-shadows_now).max(0.0) * 0.0018
            + (-blacks_now).max(0.0) * 0.0020,
    );
    let saturation_risk = clamp01(
        base_saturation_risk + saturation_now.max(0.0) * 0.0024 + vibrance_now.max(0.0) * 0.0022,
    );

    let (tonal_lift_intent, tonal_drop_intent, saturation_lift_intent, saturation_drop_intent) =
        if let Some(ref_feat) = reference_feat {
            let tonal_gap = ref_feat.p50_luminance - cur_feat.p50_luminance;
            let sat_gap = ref_feat.mean_saturation - cur_feat.mean_saturation;
            (
                clamp01((tonal_gap - 0.02) / 0.30),
                clamp01(((-tonal_gap) - 0.02) / 0.30),
                clamp01((sat_gap - 0.015) / 0.25),
                clamp01(((-sat_gap) - 0.015) / 0.25),
            )
        } else {
            (0.0, 0.0, 0.0, 0.0)
        };
    let (temp_shift_intent, tint_shift_intent) = if let Some(ref_feat) = reference_feat {
        (
            clamp01(((ref_feat.rb_ratio - cur_feat.rb_ratio).abs() - 0.02) / 0.26),
            clamp01(((ref_feat.gb_ratio - cur_feat.gb_ratio).abs() - 0.02) / 0.26),
        )
    } else {
        (0.0, 0.0)
    };
    if let Some(ref_feat) = reference_feat {
        let highlight_lift_room = (ref_feat.p99_luminance - cur_feat.p99_luminance).max(0.0);
        let clip_pressure =
            (cur_feat.clipped_highlight_ratio - ref_feat.clipped_highlight_ratio).max(0.0);
        let relax = tonal_lift_intent * 0.20 + highlight_lift_room * 0.18 - clip_pressure * 3.5;
        highlight_risk = clamp01((highlight_risk - relax).max(0.0));
    }
    let style_force = reference_feat
        .map(|ref_feat| style_intensity_score(ref_feat, cur_feat))
        .unwrap_or(0.0);

    let exposure_min =
        -2.35 + shadow_risk * 1.20 + tonal_lift_intent * 0.16 - tonal_drop_intent * 0.45;
    let exposure_headroom_relax = tonal_lift_intent
        * (0.26
            + (reference_feat
                .map(|ref_feat| (ref_feat.p90_luminance - cur_feat.p90_luminance).max(0.0))
                .unwrap_or(0.0)
                * 0.75));
    let exposure_max = 2.35 - highlight_risk * (1.45 - tonal_lift_intent * 0.42)
        + tonal_lift_intent * 0.82
        + exposure_headroom_relax
        - tonal_drop_intent * 0.24;
    let (mut exposure_min, mut exposure_max) =
        normalize_hard_band(exposure_min, exposure_max, -2.35, 2.35);
    exposure_min = exposure_min.max(-2.4).min(2.4);
    exposure_max = exposure_max.max(-2.4).min(2.4);

    let whites_max =
        62.0 - highlight_risk * 58.0 + tonal_lift_intent * 24.0 - tonal_drop_intent * 8.0;
    let highlights_max =
        58.0 - highlight_risk * 52.0 + tonal_lift_intent * 20.0 - tonal_drop_intent * 8.0;
    let shadows_min =
        -60.0 + shadow_risk * 48.0 + tonal_lift_intent * 12.0 - tonal_drop_intent * 16.0;
    let blacks_min =
        -58.0 + shadow_risk * 46.0 + tonal_lift_intent * 10.0 - tonal_drop_intent * 14.0;

    let mut sat_cap = 78.0 - saturation_risk * 42.0 + saturation_lift_intent * 18.0
        - saturation_drop_intent * 20.0;
    let mut vib_cap = 80.0 - saturation_risk * 45.0 + saturation_lift_intent * 20.0
        - saturation_drop_intent * 22.0;
    if cur_feat.skin_ratio > 0.02 {
        sat_cap -= 8.0;
        vib_cap -= 10.0;
    }
    let sat_cap = (sat_cap + style_force * 10.0).max(22.0).min(88.0);
    let vib_cap = (vib_cap + style_force * 11.0).max(20.0).min(90.0);

    let mut temp_cap =
        76.0 - highlight_risk * 12.0 + tonal_lift_intent * 4.0 + temp_shift_intent * 10.0;
    let mut tint_cap =
        72.0 - saturation_risk * 10.0 + saturation_lift_intent * 4.0 + tint_shift_intent * 10.0;
    if cur_feat.skin_ratio > 0.02 {
        temp_cap = temp_cap.min(58.0);
        tint_cap = tint_cap.min(52.0);
    }
    temp_cap = (temp_cap + style_force * 8.0).min(68.0);
    tint_cap = (tint_cap + style_force * 7.0).min(62.0);

    let contrast_hard_max = ((80.0_f64 - highlight_risk * 10.0 - shadow_risk * 8.0
        + tonal_drop_intent * 8.0)
        + style_force * 8.0)
        .max(38.0)
        .min(80.0);
    let contrast_hard_min = (-80.0 + saturation_risk * 8.0 - tonal_drop_intent * 6.0)
        .max(-80.0)
        .min(-20.0);

    let mut bands = HashMap::new();
    bands.insert(
        "exposure".to_string(),
        build_constraint_band(exposure_min, exposure_max),
    );
    bands.insert("brightness".to_string(), build_constraint_band(-80.0, 80.0));
    bands.insert(
        "contrast".to_string(),
        build_constraint_band(contrast_hard_min, contrast_hard_max),
    );
    bands.insert(
        "highlights".to_string(),
        build_constraint_band(-80.0, highlights_max.max(10.0).min(80.0)),
    );
    bands.insert(
        "shadows".to_string(),
        build_constraint_band(shadows_min.max(-80.0).min(0.0), 80.0),
    );
    bands.insert(
        "whites".to_string(),
        build_constraint_band(-80.0, whites_max.max(6.0).min(80.0)),
    );
    bands.insert(
        "blacks".to_string(),
        build_constraint_band(blacks_min.max(-80.0).min(0.0), 80.0),
    );
    bands.insert(
        "saturation".to_string(),
        build_constraint_band(-80.0, sat_cap),
    );
    bands.insert(
        "vibrance".to_string(),
        build_constraint_band(-80.0, vib_cap),
    );
    bands.insert(
        "temperature".to_string(),
        build_constraint_band(-temp_cap, temp_cap),
    );
    bands.insert(
        "tint".to_string(),
        build_constraint_band(-tint_cap, tint_cap),
    );
    bands.insert("clarity".to_string(), build_constraint_band(-80.0, 80.0));
    bands.insert("dehaze".to_string(), build_constraint_band(-70.0, 80.0));
    bands.insert("structure".to_string(), build_constraint_band(-80.0, 80.0));
    bands.insert("sharpness".to_string(), build_constraint_band(0.0, 100.0));
    bands.insert(
        "vignetteAmount".to_string(),
        build_constraint_band(-80.0, 80.0),
    );

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

                // 检查是否为纯白色图片（高亮度，低对比度，几乎没有色彩）
                if feat.mean_luminance > 0.95
                    && feat.contrast_spread < 0.05
                    && feat.mean_saturation < 0.05
                {
                    let mut window = build_dynamic_constraint_window(
                        &feat,
                        current_adjustments,
                        "image_white_override",
                    );
                    // 强制要求 AI 必须降低曝光和高光，拉回细节
                    if let Some(band) = window.bands.get_mut("exposure") {
                        band.hard_max = -1.0;
                        band.soft_max = -1.5;
                        band.hard_min = -3.0;
                    }
                    if let Some(band) = window.bands.get_mut("highlights") {
                        band.hard_max = -30.0;
                        band.soft_max = -50.0;
                        band.hard_min = -100.0;
                    }
                    if let Some(band) = window.bands.get_mut("whites") {
                        band.hard_max = -20.0;
                        band.soft_max = -40.0;
                        band.hard_min = -100.0;
                    }
                    return window;
                }

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

fn apply_soft_dynamic_constraints_to_style_suggestions(
    suggestions: &mut Vec<StyleTransferSuggestion>,
    window: &DynamicConstraintWindow,
) {
    for suggestion in suggestions.iter_mut() {
        if suggestion.complex_value.is_some() {
            continue;
        }
        let Some(band) = window.bands.get(&suggestion.key) else {
            continue;
        };
        let mut soft_adjusted = false;
        let pull = soft_pull_ratio_for_key(&suggestion.key);
        if suggestion.value < band.soft_min && suggestion.value > band.hard_min {
            let dist = band.soft_min - suggestion.value;
            suggestion.value =
                format_adjustment_value(&suggestion.key, suggestion.value + dist * pull);
            soft_adjusted = true;
        } else if suggestion.value > band.soft_max && suggestion.value < band.hard_max {
            let dist = suggestion.value - band.soft_max;
            suggestion.value =
                format_adjustment_value(&suggestion.key, suggestion.value - dist * pull);
            soft_adjusted = true;
        }
        if soft_adjusted {
            if suggestion.reason.is_empty() {
                suggestion.reason = "动态软约束已平滑".to_string();
            } else {
                suggestion.reason = format!("{}；动态软约束已平滑", suggestion.reason);
            }
        }
    }
}

pub fn clamp_value_with_dynamic_window(
    key: &str,
    value: f64,
    window: &DynamicConstraintWindow,
) -> (f64, Option<String>) {
    let Some(band) = window.bands.get(key) else {
        let hard = if key == "exposure" {
            (-2.5, 2.5)
        } else if key == "sharpness" {
            (0.0, 100.0)
        } else {
            (-80.0, 80.0)
        };
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
        if suggestion.complex_value.is_some() {
            continue;
        }
        if let Some(band) = window.bands.get(&suggestion.key) {
            let original = suggestion.value;
            let (clamped, reason) =
                clamp_value_with_dynamic_window(&suggestion.key, suggestion.value, window);
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
            let (clamped, reason) =
                clamp_value_with_dynamic_window(&suggestion.key, suggestion.value, window);
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
            StyleConstraintAction {
                key: "highlights".to_string(),
                label: "高光".to_string(),
                delta: -6.0,
            },
            StyleConstraintAction {
                key: "whites".to_string(),
                label: "白色色阶".to_string(),
                delta: -8.0,
            },
            StyleConstraintAction {
                key: "exposure".to_string(),
                label: "曝光".to_string(),
                delta: -0.08,
            },
        ],
        "shadow" => vec![
            StyleConstraintAction {
                key: "shadows".to_string(),
                label: "阴影".to_string(),
                delta: 6.0,
            },
            StyleConstraintAction {
                key: "blacks".to_string(),
                label: "黑色色阶".to_string(),
                delta: 5.0,
            },
            StyleConstraintAction {
                key: "exposure".to_string(),
                label: "曝光".to_string(),
                delta: 0.06,
            },
        ],
        "saturation" => vec![
            StyleConstraintAction {
                key: "saturation".to_string(),
                label: "饱和度".to_string(),
                delta: -6.0,
            },
            StyleConstraintAction {
                key: "vibrance".to_string(),
                label: "自然饱和度".to_string(),
                delta: -6.0,
            },
        ],
        _ => vec![
            StyleConstraintAction {
                key: "contrast".to_string(),
                label: "对比度".to_string(),
                delta: -4.0,
            },
            StyleConstraintAction {
                key: "exposure".to_string(),
                label: "曝光".to_string(),
                delta: -0.04,
            },
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
        .map(
            |(category, (label, reason, hit_count, severity))| StyleConstraintBlockItem {
                actions: actions_for_constraint_category(&category),
                category,
                label,
                reason,
                hit_count,
                severity: (severity * 100.0).round() / 100.0,
            },
        )
        .collect();
    items.sort_by(|a, b| {
        b.severity
            .total_cmp(&a.severity)
            .then_with(|| b.hit_count.cmp(&a.hit_count))
    });
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
pub fn extract_features(img: &DynamicImage) -> StyleFeatures {
    let (w, h) = img.dimensions();
    let total_pixels = (w as f64) * (h as f64);
    if total_pixels == 0.0 {
        return StyleFeatures {
            mean_luminance: 0.0,
            highlight_ratio: 0.0,
            shadow_ratio: 0.0,
            contrast_spread: 0.0,
            p10_luminance: 0.0,
            p50_luminance: 0.0,
            p90_luminance: 0.0,
            p99_luminance: 0.0,
            clipped_highlight_ratio: 0.0,
            waveform_low_band: 0.0,
            waveform_mid_band: 0.0,
            waveform_high_band: 0.0,
            rb_ratio: 1.0,
            gb_ratio: 1.0,
            mean_saturation: 0.0,
            saturation_spread: 0.0,
            shadow_luminance_mean: 0.0,
            mid_luminance_mean: 0.0,
            highlight_luminance_mean: 0.0,
            skin_ratio: 0.0,
            skin_luminance_mean: 0.0,
            skin_rb_ratio: 1.0,
            hue_mean: 0.0,
            hue_spread: 0.0,
            laplacian_variance: 0.0,
            vignette_diff: 0.0,
        };
    }

    let analysis_img = if w > 600 || h > 600 {
        img.resize(600, 600, image::imageops::FilterType::Triangle)
    } else {
        img.clone()
    };
    let analysis_rgb = analysis_img.to_rgb8();
    let (aw, ah) = analysis_rgb.dimensions();
    let a_total = (aw as f64) * (ah as f64);

    let mut linear_lut = [0.0f64; 256];
    for i in 0..=255 {
        let srgb = i as f64 / 255.0;
        linear_lut[i] = if srgb <= 0.04045 {
            srgb / 12.92
        } else {
            ((srgb + 0.055) / 1.055).powf(2.4)
        };
    }

    let mut sum_lum: f64 = 0.0;
    let mut sum_r: f64 = 0.0;
    let mut sum_g: f64 = 0.0;
    let mut sum_b: f64 = 0.0;
    let mut robust_sum_r: f64 = 0.0;
    let mut robust_sum_g: f64 = 0.0;
    let mut robust_sum_b: f64 = 0.0;
    let mut robust_color_weight_sum: f64 = 0.0;
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
    let mut hue_values: Vec<f64> = Vec::with_capacity(a_total as usize);
    let mut hue_weights: Vec<f64> = Vec::with_capacity(a_total as usize);
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

    for (x, y, px) in analysis_rgb.enumerate_pixels() {
        let r = linear_lut[px[0] as usize];
        let g = linear_lut[px[1] as usize];
        let b = linear_lut[px[2] as usize];

        let lum = 0.2126 * r + 0.7152 * g + 0.0722 * b;
        sum_lum += lum;
        sum_r += r;
        sum_g += g;
        sum_b += b;
        lum_values.push(lum);

        if lum > 0.8 {
            highlight_count += 1.0;
        }
        if lum < 0.2 {
            shadow_count += 1.0;
        }
        if lum > 0.98 {
            clipped_highlight_count += 1.0;
        }
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
            if l <= 0.5 {
                delta / (max_c + min_c)
            } else {
                delta / (2.0 - max_c - min_c)
            }
        };
        sum_sat += sat;
        sat_values.push(sat);
        let nx = (x as f64 + 0.5) / aw as f64;
        let ny = (y as f64 + 0.5) / ah as f64;
        let center_bias =
            (1.0 - ((nx - 0.5).abs() * 2.0).powi(2)) * (1.0 - ((ny - 0.5).abs() * 2.0).powi(2));
        let robust_luma_gate = if lum < 0.04 || lum > 0.95 { 0.0 } else { 1.0 };
        let highlight_gate = (1.0 - (lum - 0.75).max(0.0) / 0.25).max(0.0).min(1.0);
        let color_weight = (0.25 + 0.75 * center_bias.max(0.0)) * robust_luma_gate * highlight_gate;
        if color_weight > 0.0 {
            robust_sum_r += r * color_weight;
            robust_sum_g += g * color_weight;
            robust_sum_b += b * color_weight;
            robust_color_weight_sum += color_weight;
        }
        if sat > 0.06 && lum > 0.04 && lum < 0.96 {
            let max_c = r.max(g).max(b);
            let min_c = r.min(g).min(b);
            let delta = max_c - min_c;
            if delta > 1e-6 {
                let mut hue = if (max_c - r).abs() < 1e-6 {
                    ((g - b) / delta) % 6.0
                } else if (max_c - g).abs() < 1e-6 {
                    (b - r) / delta + 2.0
                } else {
                    (r - g) / delta + 4.0
                };
                hue /= 6.0;
                if hue < 0.0 {
                    hue += 1.0;
                }
                hue_values.push(hue);
                hue_weights.push(
                    (sat * (0.35 + 0.65 * center_bias.max(0.0)))
                        .max(0.02)
                        .min(1.0),
                );
            }
        }

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

        let y_val = 0.299 * r + 0.587 * g + 0.114 * b;
        let cb = (b - y_val) * 0.565 + 0.5;
        let cr = (r - y_val) * 0.713 + 0.5;
        let chroma_conf = (1.0 - ((cb - 0.42).abs() / 0.14 + (cr - 0.58).abs() / 0.16) * 0.5)
            .max(0.0)
            .min(1.0);
        let is_skin = y_val > 0.08 && y_val < 0.90 && sat > 0.05 && chroma_conf > 0.35;
        if is_skin {
            let weight = 0.55 + chroma_conf * 0.45;
            skin_count += weight;
            skin_lum_sum += lum * weight;
            skin_rb_sum += if b > 0.001 { (r / b) * weight } else { weight };
        }

        let mut bin_idx = ((x as f64 / aw as f64) * waveform_bins as f64).floor() as usize;
        if bin_idx >= waveform_bins {
            bin_idx = waveform_bins - 1;
        }
        waveform_bin_values[bin_idx].push(lum);
    }

    let mean_lum = sum_lum / a_total;
    let mean_sat = sum_sat / a_total;

    // 对比度：亮度标准差
    let variance: f64 = lum_values
        .iter()
        .map(|l| (l - mean_lum).powi(2))
        .sum::<f64>()
        / a_total;
    let contrast_spread = variance.sqrt();

    // 饱和度标准差
    let sat_variance: f64 = sat_values
        .iter()
        .map(|s| (s - mean_sat).powi(2))
        .sum::<f64>()
        / a_total;
    let saturation_spread = sat_variance.sqrt();
    let (hue_mean, hue_spread) = if hue_values.is_empty() {
        (0.0, 0.0)
    } else {
        let mut sum_sin = 0.0;
        let mut sum_cos = 0.0;
        let mut weight_sum = 0.0;
        for (idx, h) in hue_values.iter().enumerate() {
            let rad = h * std::f64::consts::TAU;
            let w_h = hue_weights.get(idx).copied().unwrap_or(1.0);
            sum_sin += rad.sin() * w_h;
            sum_cos += rad.cos() * w_h;
            weight_sum += w_h;
        }
        let mut mean_angle = sum_sin.atan2(sum_cos);
        if mean_angle < 0.0 {
            mean_angle += std::f64::consts::TAU;
        }
        let r_len = (sum_sin.powi(2) + sum_cos.powi(2)).sqrt() / weight_sum.max(1e-6);
        (
            mean_angle / std::f64::consts::TAU,
            (1.0 - r_len).max(0.0).min(1.0),
        )
    };

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
    let mut ordered_q = [p10, p50, p90, p99];
    ordered_q.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let p10 = ordered_q[0];
    let p50 = ordered_q[1];
    let p90 = ordered_q[2];
    let p99 = ordered_q[3];

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
    let waveform_low_band = if band_count > 0.0 {
        low_sum / band_count
    } else {
        p10
    };
    let waveform_mid_band = if band_count > 0.0 {
        mid_sum / band_count
    } else {
        p50
    };
    let waveform_high_band = if band_count > 0.0 {
        high_sum / band_count
    } else {
        p90
    };

    // R/B 和 G/B 通道比值（色温/色调）
    let mean_r = sum_r / a_total;
    let mean_g = sum_g / a_total;
    let mean_b = sum_b / a_total;
    let robust_mean_r = if robust_color_weight_sum > 0.0 {
        robust_sum_r / robust_color_weight_sum
    } else {
        mean_r
    };
    let robust_mean_g = if robust_color_weight_sum > 0.0 {
        robust_sum_g / robust_color_weight_sum
    } else {
        mean_g
    };
    let robust_mean_b = if robust_color_weight_sum > 0.0 {
        robust_sum_b / robust_color_weight_sum
    } else {
        mean_b
    };
    let rb_ratio = if robust_mean_b > 0.001 {
        robust_mean_r / robust_mean_b
    } else {
        1.0
    };
    let gb_ratio = if robust_mean_b > 0.001 {
        robust_mean_g / robust_mean_b
    } else {
        1.0
    };
    let rb_ratio = if rb_ratio.is_finite() { rb_ratio } else { 1.0 };
    let gb_ratio = if gb_ratio.is_finite() { gb_ratio } else { 1.0 };

    // 拉普拉斯方差（清晰度/结构）
    let gray = analysis_img.to_luma8();
    let laplacian_variance = compute_laplacian_variance(&gray, aw, ah);

    // 暗角差值
    let center_mean = if center_count > 0.0 {
        center_lum_sum / center_count
    } else {
        mean_lum
    };
    let edge_mean = if edge_count > 0.0 {
        edge_lum_sum / edge_count
    } else {
        mean_lum
    };
    let vignette_diff = center_mean - edge_mean; // 正值=边缘暗=有暗角
    let shadow_luminance_mean = if shadow_lum_count > 0.0 {
        shadow_lum_sum / shadow_lum_count
    } else {
        p10
    };
    let mid_luminance_mean = if mid_lum_count > 0.0 {
        mid_lum_sum / mid_lum_count
    } else {
        p50
    };
    let highlight_luminance_mean = if highlight_lum_count > 0.0 {
        highlight_lum_sum / highlight_lum_count
    } else {
        p90
    };
    let skin_luminance_mean = if skin_count > 0.0 {
        skin_lum_sum / skin_count
    } else {
        mean_lum
    };
    let skin_rb_ratio = if skin_count > 0.0 {
        skin_rb_sum / skin_count
    } else {
        rb_ratio
    };

    let safe = |v: f64, fallback: f64| if v.is_finite() { v } else { fallback };
    let highlight_ratio = safe((highlight_count / a_total).max(0.0).min(1.0), 0.0);
    let shadow_ratio = safe((shadow_count / a_total).max(0.0).min(1.0), 0.0);
    let clipped_ratio = safe((clipped_highlight_count / a_total).max(0.0).min(1.0), 0.0);
    let skin_ratio = safe((skin_count / a_total).max(0.0).min(1.0), 0.0);
    let hue_mean = safe(hue_mean.max(0.0).min(1.0), 0.0);
    let hue_spread = safe(hue_spread.max(0.0).min(1.0), 0.0);

    let mut ordered_wave = [
        safe(waveform_low_band.max(0.0).min(1.0), p10),
        safe(waveform_mid_band.max(0.0).min(1.0), p50),
        safe(waveform_high_band.max(0.0).min(1.0), p90),
    ];
    ordered_wave.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    StyleFeatures {
        mean_luminance: safe(mean_lum.max(0.0).min(1.0), 0.0),
        highlight_ratio,
        shadow_ratio,
        contrast_spread: safe(contrast_spread.max(0.0), 0.0),
        p10_luminance: safe(p10.max(0.0).min(1.0), 0.0),
        p50_luminance: safe(p50.max(0.0).min(1.0), 0.0),
        p90_luminance: safe(p90.max(0.0).min(1.0), 0.0),
        p99_luminance: safe(p99.max(0.0).min(1.0), 0.0),
        clipped_highlight_ratio: clipped_ratio,
        waveform_low_band: ordered_wave[0],
        waveform_mid_band: ordered_wave[1],
        waveform_high_band: ordered_wave[2],
        rb_ratio,
        gb_ratio,
        mean_saturation: safe(mean_sat.max(0.0).min(1.0), 0.0),
        saturation_spread: safe(saturation_spread.max(0.0), 0.0),
        shadow_luminance_mean: safe(shadow_luminance_mean.max(0.0).min(1.0), 0.0),
        mid_luminance_mean: safe(mid_luminance_mean.max(0.0).min(1.0), 0.0),
        highlight_luminance_mean: safe(highlight_luminance_mean.max(0.0).min(1.0), 0.0),
        skin_ratio,
        skin_luminance_mean: safe(skin_luminance_mean.max(0.0).min(1.0), 0.0),
        skin_rb_ratio: safe(skin_rb_ratio.max(0.0), rb_ratio),
        hue_mean,
        hue_spread,
        laplacian_variance: safe(laplacian_variance.max(0.0), 0.0),
        vignette_diff: safe(vignette_diff, 0.0),
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

#[derive(Debug, Clone, Copy)]
struct StyleSceneProfile {
    tonal_gain: f64,
    highlight_gain: f64,
    shadow_gain: f64,
    chroma_limit: f64,
    chroma_guard_floor: f64,
    color_residual_gain: f64,
}

fn style_scene_profile(ref_feat: &StyleFeatures, cur_feat: &StyleFeatures) -> StyleSceneProfile {
    let mut profile = if ref_feat.p50_luminance < 0.42 && ref_feat.p90_luminance < 0.74 {
        StyleSceneProfile {
            tonal_gain: 1.12,
            highlight_gain: 0.92,
            shadow_gain: 1.15,
            chroma_limit: 0.88,
            chroma_guard_floor: 0.54,
            color_residual_gain: 1.06,
        }
    } else if ref_feat.p50_luminance > 0.60 && ref_feat.p10_luminance > 0.33 {
        StyleSceneProfile {
            tonal_gain: 1.06,
            highlight_gain: 1.08,
            shadow_gain: 0.95,
            chroma_limit: 0.93,
            chroma_guard_floor: 0.58,
            color_residual_gain: 0.96,
        }
    } else {
        StyleSceneProfile {
            tonal_gain: 1.0,
            highlight_gain: 1.0,
            shadow_gain: 1.0,
            chroma_limit: 1.0,
            chroma_guard_floor: 0.58,
            color_residual_gain: 1.0,
        }
    };
    if ref_feat.skin_ratio > 0.020 && cur_feat.skin_ratio > 0.020 {
        profile.chroma_limit *= 0.92;
        profile.color_residual_gain *= 0.95;
    }
    let tonal_gap = (ref_feat.p50_luminance - cur_feat.p50_luminance).abs();
    if tonal_gap > 0.09 {
        profile.tonal_gain *= 1.08;
        profile.color_residual_gain *= 0.90;
    }
    profile.tonal_gain = profile.tonal_gain.max(0.90).min(1.24);
    profile.highlight_gain = profile.highlight_gain.max(0.86).min(1.16);
    profile.shadow_gain = profile.shadow_gain.max(0.86).min(1.18);
    profile.chroma_limit = profile.chroma_limit.max(0.74).min(1.06);
    profile.chroma_guard_floor = profile.chroma_guard_floor.max(0.48).min(0.70);
    profile.color_residual_gain = profile.color_residual_gain.max(0.82).min(1.12);
    profile
}

fn adaptive_style_thresholds(
    ref_feat: &StyleFeatures,
    cur_feat: &StyleFeatures,
    baseline_error: &StyleTransferErrorBreakdown,
) -> (f64, f64) {
    let style_force = style_intensity_score(ref_feat, cur_feat);
    let color_ratio = baseline_error.color / baseline_error.total.max(1e-6);
    let tonal_ratio = baseline_error.tonal / baseline_error.total.max(1e-6);
    let early_exit = (EARLY_EXIT_STYLE_DISTANCE_THRESHOLD - style_force * 0.055
        + tonal_ratio * 0.018
        - color_ratio * 0.012)
        .max(0.14)
        .min(0.30);
    let llm_trigger =
        (LLM_TRIGGER_STYLE_DISTANCE_THRESHOLD - style_force * 0.14 - color_ratio * 0.08
            + tonal_ratio * 0.03)
            .max(0.42)
            .min(0.80);
    (early_exit, llm_trigger.max(early_exit + 0.12))
}

fn tonal_alignment_score(ref_feat: &StyleFeatures, pred_feat: &StyleFeatures) -> f64 {
    let mut score = 0.0;
    score += (ref_feat.waveform_mid_band - pred_feat.waveform_mid_band).abs() * 4.4;
    score += (ref_feat.waveform_low_band - pred_feat.waveform_low_band).abs() * 3.2;
    score += (ref_feat.waveform_high_band - pred_feat.waveform_high_band).abs() * 3.8;
    score += (ref_feat.p50_luminance - pred_feat.p50_luminance).abs() * 3.4;
    score += (ref_feat.p90_luminance - pred_feat.p90_luminance).abs() * 2.6;
    score += (ref_feat.contrast_spread - pred_feat.contrast_spread).abs() * 6.2;
    score
}

fn estimate_features_from_adjustments(
    cur_feat: &StyleFeatures,
    current_values: &HashMap<String, f64>,
    candidate_values: &HashMap<String, f64>,
) -> StyleFeatures {
    let mut f = cur_feat.clone();
    let delta = |k: &str| {
        candidate_values
            .get(k)
            .copied()
            .unwrap_or_else(|| current_values.get(k).copied().unwrap_or(0.0))
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
    let d_exp_con = d_exp * d_con.signum() * d_con.abs().min(35.0) * 0.001;
    let d_wh_hl = d_wh * d_hl * 0.00002;
    let d_sat_vib = d_sat * d_vib * 0.00003;

    f.mean_luminance +=
        d_exp * 0.20 + d_hl * 0.0012 + d_sh * 0.0010 + d_wh * 0.0016 - d_bl * 0.0009;
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
    f.contrast_spread += d_con * 0.0021 + d_wh * 0.0008 + d_bl * 0.0008 + d_exp_con * 0.32;
    f.rb_ratio += d_temp * 0.0028;
    f.gb_ratio += -d_tint * 0.0025;
    f.mean_saturation += d_sat * 0.0042 + d_vib * 0.0028 - d_exp * 0.0012 + d_sat_vib * 0.28;
    f.saturation_spread += d_vib * 0.0032 + d_sat * 0.0014 + d_sat_vib * 0.22;
    f.p90_luminance += d_wh_hl * 0.25;
    f.hue_mean += (d_tint * 0.0008 - d_temp * 0.0006 + d_sat_vib * 0.16)
        .max(-0.08)
        .min(0.08);
    f.hue_spread += (d_vib * 0.0018 + d_sat * 0.0011 + d_sat_vib * 0.20)
        .max(-0.06)
        .min(0.08);
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
    f.hue_mean = clamp01(f.hue_mean);
    f.hue_spread = clamp01(f.hue_spread);
    f
}

fn style_distance_score(
    ref_feat: &StyleFeatures,
    pred_feat: &StyleFeatures,
    tuning: StyleTransferTuning,
) -> f64 {
    let mut s = 0.0;
    s += (ref_feat.waveform_mid_band - pred_feat.waveform_mid_band).abs() * 6.2;
    s += (ref_feat.waveform_low_band - pred_feat.waveform_low_band).abs() * 4.3;
    s += (ref_feat.waveform_high_band - pred_feat.waveform_high_band).abs() * 4.8;
    s += (ref_feat.p10_luminance - pred_feat.p10_luminance).abs() * 3.6;
    s += (ref_feat.p50_luminance - pred_feat.p50_luminance).abs() * 4.4;
    s += (ref_feat.p90_luminance - pred_feat.p90_luminance).abs() * 3.5;
    s += (ref_feat.contrast_spread - pred_feat.contrast_spread).abs() * 7.2;
    s += (ref_feat.mean_saturation - pred_feat.mean_saturation).abs() * 3.3;
    s += (ref_feat.saturation_spread - pred_feat.saturation_spread).abs() * 2.0;
    s += (ref_feat.hue_mean - pred_feat.hue_mean).abs() * 2.4;
    s += (ref_feat.hue_spread - pred_feat.hue_spread).abs() * 1.8;
    s += (ref_feat.rb_ratio - pred_feat.rb_ratio).abs() * 2.2;
    s += (ref_feat.gb_ratio - pred_feat.gb_ratio).abs() * 2.1;
    s += (ref_feat.shadow_luminance_mean - pred_feat.shadow_luminance_mean).abs() * 2.8;
    s += (ref_feat.mid_luminance_mean - pred_feat.mid_luminance_mean).abs() * 3.2;
    s += (ref_feat.highlight_luminance_mean - pred_feat.highlight_luminance_mean).abs() * 2.9;
    if ref_feat.skin_ratio > 0.015 && pred_feat.skin_ratio > 0.015 {
        s += (ref_feat.skin_luminance_mean - pred_feat.skin_luminance_mean).abs()
            * 3.8
            * tuning.skin_protect_strength;
        s += (ref_feat.skin_rb_ratio - pred_feat.skin_rb_ratio).abs()
            * 2.0
            * tuning.skin_protect_strength;
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

fn style_metric_vector(feat: &StyleFeatures) -> [f64; 15] {
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
        feat.hue_mean,
        feat.hue_spread,
        feat.skin_luminance_mean,
        feat.skin_rb_ratio,
    ]
}

fn style_metric_weights(
    ref_feat: &StyleFeatures,
    pred_feat: &StyleFeatures,
    tuning: StyleTransferTuning,
) -> [f64; 15] {
    let skin_w = if ref_feat.skin_ratio > 0.015 && pred_feat.skin_ratio > 0.015 {
        1.0 * tuning.skin_protect_strength
    } else {
        0.0
    };
    [
        6.2,
        4.3,
        4.8,
        3.6,
        4.4,
        3.5,
        7.2,
        3.3,
        2.0,
        2.2,
        2.1,
        2.4,
        1.8,
        3.8 * skin_w,
        2.0 * skin_w,
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
    tonal += (ref_feat.waveform_mid_band - pred_feat.waveform_mid_band).abs() * 6.2;
    tonal += (ref_feat.waveform_low_band - pred_feat.waveform_low_band).abs() * 4.3;
    tonal += (ref_feat.waveform_high_band - pred_feat.waveform_high_band).abs() * 4.8;
    tonal += (ref_feat.p10_luminance - pred_feat.p10_luminance).abs() * 3.6;
    tonal += (ref_feat.p50_luminance - pred_feat.p50_luminance).abs() * 4.4;
    tonal += (ref_feat.p90_luminance - pred_feat.p90_luminance).abs() * 3.5;
    tonal += (ref_feat.contrast_spread - pred_feat.contrast_spread).abs() * 7.2;
    tonal += (ref_feat.shadow_luminance_mean - pred_feat.shadow_luminance_mean).abs() * 2.8;
    tonal += (ref_feat.mid_luminance_mean - pred_feat.mid_luminance_mean).abs() * 3.2;
    tonal += (ref_feat.highlight_luminance_mean - pred_feat.highlight_luminance_mean).abs() * 2.9;

    let mut color = 0.0;
    color += (ref_feat.mean_saturation - pred_feat.mean_saturation).abs() * 3.3;
    color += (ref_feat.saturation_spread - pred_feat.saturation_spread).abs() * 2.0;
    color += (ref_feat.rb_ratio - pred_feat.rb_ratio).abs() * 2.2;
    color += (ref_feat.gb_ratio - pred_feat.gb_ratio).abs() * 2.1;
    color += (ref_feat.hue_mean - pred_feat.hue_mean).abs() * 2.4;
    color += (ref_feat.hue_spread - pred_feat.hue_spread).abs() * 1.8;

    let mut skin = 0.0;
    if ref_feat.skin_ratio > 0.015 && pred_feat.skin_ratio > 0.015 {
        skin += (ref_feat.skin_luminance_mean - pred_feat.skin_luminance_mean).abs()
            * 3.8
            * tuning.skin_protect_strength;
        skin += (ref_feat.skin_rb_ratio - pred_feat.skin_rb_ratio).abs()
            * 2.0
            * tuning.skin_protect_strength;
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
        "curves" => Some(("色彩曲线", 0.0, 1.0)),
        "hsl" => Some(("HSL", -100.0, 100.0)),
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
        complex_value: None,
        reason: reason.to_string(),
    });
    true
}

fn upsert_suggestion_absolute(
    suggestions: &mut Vec<StyleTransferSuggestion>,
    key: &str,
    value: f64,
    reason: &str,
) -> bool {
    let Some((label, min, max)) = action_meta(key) else {
        return false;
    };
    let target = value.max(min).min(max);
    let normalized = if key == "exposure" {
        (target * 100.0).round() / 100.0
    } else {
        target.round()
    };
    if let Some(existing) = suggestions.iter_mut().find(|s| s.key == key) {
        existing.value = normalized;
        existing.reason = if existing.reason.is_empty() {
            reason.to_string()
        } else {
            format!("{}；{}", existing.reason, reason)
        };
        return true;
    }
    suggestions.push(StyleTransferSuggestion {
        key: key.to_string(),
        value: normalized,
        label: label.to_string(),
        min,
        max,
        complex_value: None,
        reason: reason.to_string(),
    });
    true
}

fn apply_reference_normalize_pass(
    ref_feat: &StyleFeatures,
    cur_feat: &StyleFeatures,
    current_adjustments: &Value,
    suggestions: &mut Vec<StyleTransferSuggestion>,
    tuning: StyleTransferTuning,
) {
    let mut candidate = suggestions.clone();
    let clamp = |v: f64, min: f64, max: f64| -> f64 { v.max(min).min(max) };
    let cur_exposure = adjustment_value(current_adjustments, "exposure");
    let cur_temp = adjustment_value(current_adjustments, "temperature");
    let cur_tint = adjustment_value(current_adjustments, "tint");
    let cur_sat = adjustment_value(current_adjustments, "saturation");
    let cur_vib = adjustment_value(current_adjustments, "vibrance");
    let cur_shadows = adjustment_value(current_adjustments, "shadows");
    let cur_blacks = adjustment_value(current_adjustments, "blacks");
    let tonal_conf = clamp(
        ((ref_feat.p50_luminance - cur_feat.p50_luminance).abs() * 2.6
            + (ref_feat.mean_luminance - cur_feat.mean_luminance).abs() * 2.0)
            * (0.85 + tuning.style_strength * 0.15),
        0.0,
        1.0,
    );
    let color_conf = clamp(
        (ref_feat.rb_ratio - cur_feat.rb_ratio).abs() * 1.8
            + (ref_feat.gb_ratio - cur_feat.gb_ratio).abs() * 1.8
            + (ref_feat.mean_saturation - cur_feat.mean_saturation).abs() * 1.3,
        0.0,
        1.0,
    );
    let exposure_target = clamp(
        cur_exposure
            + (ref_feat.p50_luminance - cur_feat.p50_luminance) * (0.95 + tonal_conf * 0.30)
            + (ref_feat.mean_luminance - cur_feat.mean_luminance) * (0.38 + tonal_conf * 0.18),
        -2.3,
        2.3,
    );
    let bright_gap = (ref_feat.p50_luminance - cur_feat.p50_luminance).max(0.0);
    let mid_gap = (ref_feat.waveform_mid_band - cur_feat.waveform_mid_band).max(0.0);
    let highlight_room = (ref_feat.p90_luminance - cur_feat.p90_luminance).max(0.0);
    let exposure_target = if bright_gap > 0.012 {
        clamp(
            exposure_target
                + bright_gap * (0.22 + tonal_conf * 0.10)
                + mid_gap * 0.16
                + highlight_room * 0.08,
            -2.3,
            2.3,
        )
    } else {
        exposure_target
    };
    let shadows_target = clamp(
        cur_shadows
            + (ref_feat.shadow_luminance_mean - cur_feat.shadow_luminance_mean)
                * (92.0 + tonal_conf * 24.0)
            + bright_gap * 26.0,
        -55.0,
        70.0,
    );
    let blacks_target = clamp(
        cur_blacks
            + (ref_feat.p10_luminance - cur_feat.p10_luminance) * (58.0 + tonal_conf * 16.0)
            + bright_gap * 12.0,
        -55.0,
        45.0,
    );
    let temp_target = clamp(
        cur_temp
            + (ref_feat.rb_ratio - cur_feat.rb_ratio) * (26.0 + color_conf * 8.0)
            + (cur_feat.gb_ratio - ref_feat.gb_ratio) * 6.0,
        -60.0,
        60.0,
    );
    let tint_target = clamp(
        cur_tint
            + (cur_feat.gb_ratio - ref_feat.gb_ratio) * (24.0 + color_conf * 7.0)
            + (ref_feat.rb_ratio - cur_feat.rb_ratio) * 5.0,
        -55.0,
        55.0,
    );
    let sat_target = clamp(
        cur_sat
            + (ref_feat.mean_saturation - cur_feat.mean_saturation) * (62.0 + color_conf * 18.0),
        -55.0,
        55.0,
    );
    let vib_target = clamp(
        cur_vib
            + (ref_feat.saturation_spread - cur_feat.saturation_spread)
                * (70.0 + color_conf * 20.0),
        -55.0,
        55.0,
    );
    let exposure_blend = 0.28 + tonal_conf * 0.36;
    let color_blend = 0.22 + color_conf * 0.32;
    if (exposure_target - cur_exposure).abs() > 0.035 {
        let existing = candidate
            .iter()
            .find(|s| s.key == "exposure")
            .map(|s| s.value)
            .unwrap_or(cur_exposure);
        let blended = existing * (1.0 - exposure_blend) + exposure_target * exposure_blend;
        let _ = upsert_suggestion_absolute(
            &mut candidate,
            "exposure",
            blended,
            "参考归一化：先对齐中间调与整体亮度",
        );
    }
    if (temp_target - cur_temp).abs() > 1.5 {
        let existing = candidate
            .iter()
            .find(|s| s.key == "temperature")
            .map(|s| s.value)
            .unwrap_or(cur_temp);
        let blended = existing * (1.0 - color_blend) + temp_target * color_blend;
        let _ = upsert_suggestion_absolute(
            &mut candidate,
            "temperature",
            blended,
            "参考归一化：先对齐冷暖倾向",
        );
    }
    if (tint_target - cur_tint).abs() > 1.5 {
        let existing = candidate
            .iter()
            .find(|s| s.key == "tint")
            .map(|s| s.value)
            .unwrap_or(cur_tint);
        let blended = existing * (1.0 - color_blend) + tint_target * color_blend;
        let _ = upsert_suggestion_absolute(
            &mut candidate,
            "tint",
            blended,
            "参考归一化：先对齐色偏基线",
        );
    }
    if bright_gap > 0.015 || (shadows_target - cur_shadows).abs() > 1.6 {
        let existing = candidate
            .iter()
            .find(|s| s.key == "shadows")
            .map(|s| s.value)
            .unwrap_or(cur_shadows);
        let shadow_blend = 0.34 + tonal_conf * 0.30;
        let blended = existing * (1.0 - shadow_blend) + shadows_target * shadow_blend;
        let _ = upsert_suggestion_absolute(
            &mut candidate,
            "shadows",
            blended,
            "提亮纠偏：补偿暗部亮度",
        );
    }
    if bright_gap > 0.02 || (blacks_target - cur_blacks).abs() > 1.6 {
        let existing = candidate
            .iter()
            .find(|s| s.key == "blacks")
            .map(|s| s.value)
            .unwrap_or(cur_blacks);
        let black_blend = 0.30 + tonal_conf * 0.26;
        let blended = existing * (1.0 - black_blend) + blacks_target * black_blend;
        let _ = upsert_suggestion_absolute(
            &mut candidate,
            "blacks",
            blended,
            "提亮纠偏：抬升黑场避免整体发闷",
        );
    }
    if ref_feat.mean_saturation < 0.28 && cur_feat.mean_saturation > ref_feat.mean_saturation + 0.03
    {
        let sat_cap = cur_sat + (ref_feat.mean_saturation - cur_feat.mean_saturation) * 48.0;
        let vib_cap = cur_vib + (ref_feat.saturation_spread - cur_feat.saturation_spread) * 42.0;
        let _ = upsert_suggestion_absolute(
            &mut candidate,
            "saturation",
            sat_target.min(sat_cap),
            "低饱和参考保护：限制过度上色",
        );
        let _ = upsert_suggestion_absolute(
            &mut candidate,
            "vibrance",
            vib_target.min(vib_cap),
            "低饱和参考保护：限制同系色扩散",
        );
    }
    let before_pred = estimate_features_for_suggestions(cur_feat, current_adjustments, suggestions);
    let before_error = style_error_breakdown(ref_feat, &before_pred, tuning).total;
    let after_pred = estimate_features_for_suggestions(cur_feat, current_adjustments, &candidate);
    let after_error = style_error_breakdown(ref_feat, &after_pred, tuning).total;
    if after_error + 0.01 < before_error {
        *suggestions = candidate;
    }
}

fn apply_low_confidence_damping(
    ref_feat: &StyleFeatures,
    cur_feat: &StyleFeatures,
    current_adjustments: &Value,
    suggestions: &mut Vec<StyleTransferSuggestion>,
    tuning: StyleTransferTuning,
) {
    if suggestions.is_empty() {
        return;
    }
    let mut current_values: HashMap<String, f64> = HashMap::new();
    let mut candidate_values: HashMap<String, f64> = HashMap::new();
    for s in suggestions.iter() {
        let current_value = adjustment_value(current_adjustments, &s.key);
        current_values.insert(s.key.clone(), current_value);
        candidate_values.insert(s.key.clone(), s.value);
    }
    let base_score = style_distance_score(ref_feat, cur_feat, tuning);
    let predicted =
        estimate_features_from_adjustments(cur_feat, &current_values, &candidate_values);
    let after_score = style_distance_score(ref_feat, &predicted, tuning);
    if base_score < 1e-6 {
        return;
    }
    let improvement = (base_score - after_score) / base_score;
    let tonal_probe = ((ref_feat.p50_luminance - predicted.p50_luminance).abs()
        + (ref_feat.waveform_mid_band - predicted.waveform_mid_band).abs())
    .max(1e-6);
    let tonal_base = ((ref_feat.p50_luminance - cur_feat.p50_luminance).abs()
        + (ref_feat.waveform_mid_band - cur_feat.waveform_mid_band).abs())
    .max(1e-6);
    let color_probe = ((ref_feat.rb_ratio - predicted.rb_ratio).abs()
        + (ref_feat.gb_ratio - predicted.gb_ratio).abs()
        + (ref_feat.hue_mean - predicted.hue_mean).abs())
    .max(1e-6);
    let color_base = ((ref_feat.rb_ratio - cur_feat.rb_ratio).abs()
        + (ref_feat.gb_ratio - cur_feat.gb_ratio).abs()
        + (ref_feat.hue_mean - cur_feat.hue_mean).abs())
    .max(1e-6);
    let probe_improvement = ((tonal_base - tonal_probe) / tonal_base) * 0.55
        + ((color_base - color_probe) / color_base) * 0.45;
    let effective_improvement = improvement.min(probe_improvement);
    let damping: f64 = if effective_improvement < 0.0 {
        0.45
    } else if effective_improvement < 0.05 {
        0.62
    } else if effective_improvement < 0.10 {
        0.78
    } else {
        1.0
    };
    if damping >= 0.999 {
        return;
    }
    for s in suggestions.iter_mut() {
        let cur = adjustment_value(current_adjustments, &s.key);
        let mut damping_for_key = damping;
        let brighten_need = (ref_feat.p50_luminance - cur_feat.p50_luminance).max(0.0)
            + (ref_feat.waveform_mid_band - cur_feat.waveform_mid_band).max(0.0) * 0.7;
        if s.key == "exposure" {
            if brighten_need > 0.02 && s.value > cur {
                if damping_for_key < 0.88 {
                    damping_for_key = 0.88;
                }
            }
        } else if (s.key == "shadows" || s.key == "blacks")
            && brighten_need > 0.018
            && s.value > cur
        {
            if damping_for_key < 0.84 {
                damping_for_key = 0.84;
            }
        }
        let damped = cur + (s.value - cur) * damping_for_key;
        s.value = format_adjustment_value(&s.key, damped);
        if s.reason.is_empty() {
            s.reason = "低置信结果自动降幅".to_string();
        } else {
            s.reason = format!("{}；低置信结果自动降幅", s.reason);
        }
    }
}

fn auto_refine_suggestions_by_error(
    ref_feat: &StyleFeatures,
    cur_feat: &StyleFeatures,
    current_adjustments: &Value,
    suggestions: &mut Vec<StyleTransferSuggestion>,
    tuning: StyleTransferTuning,
) -> u32 {
    let scene_profile = style_scene_profile(ref_feat, cur_feat);
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
        let stage = if round == 0 || breakdown.tonal > 0.52 {
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
                ((ref_feat.p50_luminance - pred.p50_luminance) * 0.95)
                    .max(-0.12)
                    .min(0.12),
                "误差驱动二次微调：修正中间调偏差",
                stage,
            );
            changed |= upsert_suggestion_delta(
                suggestions,
                current_adjustments,
                "contrast",
                ((ref_feat.contrast_spread - pred.contrast_spread) * 28.0)
                    .max(-4.0)
                    .min(4.0),
                "误差驱动二次微调：匹配对比度扩散",
                stage,
            );
            changed |= upsert_suggestion_delta(
                suggestions,
                current_adjustments,
                "shadows",
                ((ref_feat.waveform_low_band - pred.waveform_low_band) * 18.0)
                    .max(-4.0)
                    .min(4.0),
                "误差驱动二次微调：补偿暗部波形",
                stage,
            );
            changed |= upsert_suggestion_delta(
                suggestions,
                current_adjustments,
                "highlights",
                ((ref_feat.waveform_high_band - pred.waveform_high_band) * 16.0)
                    .max(-3.0)
                    .min(3.0),
                "误差驱动二次微调：修正亮部波形",
                stage,
            );
        } else if (dominant - breakdown.color).abs() < 1e-6 {
            let tonal_coupling = if breakdown.tonal > 0.60 {
                0.52
            } else if breakdown.tonal > 0.48 {
                0.68
            } else if breakdown.tonal > 0.38 {
                0.82
            } else {
                1.0
            };
            let sat_overshoot = (pred.mean_saturation - ref_feat.mean_saturation).max(0.0);
            let spread_overshoot = (pred.saturation_spread - ref_feat.saturation_spread).max(0.0);
            let chroma_guard = (1.0 - sat_overshoot * 1.7 - spread_overshoot * 1.5)
                .max(scene_profile.chroma_guard_floor)
                .min(1.0);
            changed |= upsert_suggestion_delta(
                suggestions,
                current_adjustments,
                "temperature",
                (((ref_feat.rb_ratio - pred.rb_ratio) * 18.0
                    + (pred.gb_ratio - ref_feat.gb_ratio) * 5.0)
                    * tonal_coupling
                    * chroma_guard
                    * scene_profile.color_residual_gain)
                    .max(-5.5)
                    .min(5.5),
                "误差驱动二次微调：纠正冷暖偏差",
                stage,
            );
            changed |= upsert_suggestion_delta(
                suggestions,
                current_adjustments,
                "tint",
                ((-(ref_feat.gb_ratio - pred.gb_ratio) * 16.0
                    + (ref_feat.rb_ratio - pred.rb_ratio) * 4.5)
                    * tonal_coupling
                    * chroma_guard
                    * scene_profile.color_residual_gain)
                    .max(-5.5)
                    .min(5.5),
                "误差驱动二次微调：纠正色偏",
                stage,
            );
            changed |= upsert_suggestion_delta(
                suggestions,
                current_adjustments,
                "saturation",
                (((ref_feat.mean_saturation - pred.mean_saturation) * 30.0) * tonal_coupling)
                    .max(-3.2 * scene_profile.chroma_limit)
                    .min(3.2 * scene_profile.chroma_limit),
                "误差驱动二次微调：匹配整体饱和度",
                stage,
            );
            changed |= upsert_suggestion_delta(
                suggestions,
                current_adjustments,
                "vibrance",
                (((ref_feat.saturation_spread - pred.saturation_spread) * 26.0) * tonal_coupling)
                    .max(-2.6 * scene_profile.chroma_limit)
                    .min(2.6 * scene_profile.chroma_limit),
                "误差驱动二次微调：匹配同系色层次",
                stage,
            );
        } else {
            changed |= upsert_suggestion_delta(
                suggestions,
                current_adjustments,
                "highlights",
                ((ref_feat.skin_luminance_mean - pred.skin_luminance_mean) * 15.0)
                    .max(-3.0)
                    .min(3.0),
                "误差驱动二次微调：保护肤色亮度层次",
                stage,
            );
            changed |= upsert_suggestion_delta(
                suggestions,
                current_adjustments,
                "temperature",
                ((ref_feat.skin_rb_ratio - pred.skin_rb_ratio) * 8.0)
                    .max(-3.0)
                    .min(3.0),
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
            recommended_delta: ((ref_feat.rb_ratio - pred_feat.rb_ratio) * 16.0)
                .max(-6.0)
                .min(6.0),
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
            recommended_delta: ((ref_feat.skin_luminance_mean - pred_feat.skin_luminance_mean)
                * 18.0)
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
            recommended_delta: (-(pred_feat.p99_luminance - target_p99_cap(ref_feat, tuning))
                * 55.0)
                .max(-10.0)
                .min(0.0),
            priority: 1,
            reason: "过曝惩罚仍在，建议继续收紧白场".to_string(),
        });
    }
    actions.sort_by(|a, b| {
        a.priority.cmp(&b.priority).then_with(|| {
            b.recommended_delta
                .abs()
                .total_cmp(&a.recommended_delta.abs())
        })
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
    let scene = style_scene_profile(ref_feat, cur_feat);
    let scene_profile = StyleSceneProfileDebug {
        reference_tonal_style: classify_tonal_style(ref_feat).to_string(),
        current_tonal_style: classify_tonal_style(cur_feat).to_string(),
        tonal_gain: scene.tonal_gain,
        highlight_gain: scene.highlight_gain,
        shadow_gain: scene.shadow_gain,
        chroma_limit: scene.chroma_limit,
        chroma_guard_floor: scene.chroma_guard_floor,
        color_residual_gain: scene.color_residual_gain,
    };
    let before = style_error_breakdown(ref_feat, cur_feat, tuning);
    let after = style_error_breakdown(ref_feat, pred_feat, tuning);
    let proximity_before = style_proximity_score(ref_feat, cur_feat, &before);
    let proximity_after = style_proximity_score(ref_feat, pred_feat, &after);
    let improvement_ratio = if before.total > 1e-6 {
        ((before.total - after.total) / before.total)
            .max(-1.0)
            .min(1.0)
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
        scene_profile: Some(scene_profile),
        constraint_debug,
    }
}

fn identity_curve_lut() -> [f64; 256] {
    std::array::from_fn(|idx| idx as f64)
}

fn lut_to_curve_points(lut: &[f64; 256]) -> Vec<serde_json::Value> {
    let mut points = Vec::with_capacity(16);
    let mut prev_y = 0.0;
    for step in 0..16 {
        let x = if step == 15 { 255 } else { step * 17 };
        let y = lut[x].max(prev_y).min(255.0);
        prev_y = y;
        points.push(json!({ "x": x as f64, "y": y.round() }));
    }
    points
}

fn clip_histogram(hist: &mut [u32; 256], ratio: f64) {
    let total: u32 = hist.iter().sum();
    let clip_limit = (total as f64 * ratio).max(1.0) as u32;
    for bucket in hist.iter_mut() {
        *bucket = (*bucket).min(clip_limit);
    }
}

fn build_cdf(hist: &[u32; 256]) -> [f64; 256] {
    let total: u32 = hist.iter().sum();
    let mut cdf = [0.0; 256];
    let mut running = 0u32;
    for idx in 0..256 {
        running += hist[idx];
        cdf[idx] = running as f64 / total.max(1) as f64;
    }
    cdf
}

fn histogram_match_lut(ref_hist: &[u32; 256], cur_hist: &[u32; 256], strength: f64) -> [f64; 256] {
    let mut ref_clipped = *ref_hist;
    let mut cur_clipped = *cur_hist;
    clip_histogram(&mut ref_clipped, 0.015);
    clip_histogram(&mut cur_clipped, 0.015);
    let ref_cdf = build_cdf(&ref_clipped);
    let cur_cdf = build_cdf(&cur_clipped);
    let mix = strength.max(0.0).min(1.0);
    let mut lut = [0.0; 256];
    for x in 0..256 {
        let target_prob = cur_cdf[x];
        let mut y = 255usize;
        for idx in 0..256 {
            if ref_cdf[idx] >= target_prob {
                y = idx;
                break;
            }
        }
        lut[x] = (x as f64) * (1.0 - mix) + (y as f64) * mix;
    }
    lut
}

fn blend_lut(base: &[f64; 256], overlay: &[f64; 256], strength: f64) -> [f64; 256] {
    let mix = strength.max(0.0).min(1.0);
    let mut out = [0.0; 256];
    for idx in 0..256 {
        out[idx] = base[idx] * (1.0 - mix) + overlay[idx] * mix;
    }
    out
}

fn build_range_preserve_lut(
    ref_hist: &[u32; 256],
    cur_hist: &[u32; 256],
    preserve_ratio: f64,
) -> [f64; 256] {
    let preserve = preserve_ratio.max(0.0).min(1.0);
    let find_first = |hist: &[u32; 256]| -> usize {
        for idx in 0..256 {
            if hist[idx] > 0 {
                return idx;
            }
        }
        0
    };
    let find_last = |hist: &[u32; 256]| -> usize {
        for idx in (0..256).rev() {
            if hist[idx] > 0 {
                return idx;
            }
        }
        255
    };
    let ref_min = find_first(ref_hist) as f64;
    let ref_max = find_last(ref_hist) as f64;
    let cur_min = find_first(cur_hist) as f64;
    let cur_max = find_last(cur_hist) as f64;
    let target_min = ref_min * (1.0 - preserve) + cur_min * preserve;
    let target_max = ref_max * (1.0 - preserve) + cur_max * preserve;
    let cur_range = (cur_max - cur_min).max(1.0);
    let mut lut = [0.0; 256];
    for idx in 0..256 {
        let normalized = ((idx as f64) - cur_min) / cur_range;
        lut[idx] = (normalized * (target_max - target_min) + target_min)
            .max(0.0)
            .min(255.0);
    }
    lut
}

fn build_contrast_micro_curve_lut(
    ref_hist: &[u32; 256],
    cur_hist: &[u32; 256],
    strength: f64,
) -> [f64; 256] {
    let ref_cdf = build_cdf(ref_hist);
    let cur_cdf = build_cdf(cur_hist);
    let find_quantile = |cdf: &[f64; 256], q: f64| -> usize {
        for idx in 0..256 {
            if cdf[idx] >= q {
                return idx;
            }
        }
        255
    };
    let ref_spread = find_quantile(&ref_cdf, 0.90) as f64 - find_quantile(&ref_cdf, 0.10) as f64;
    let cur_spread = find_quantile(&cur_cdf, 0.90) as f64 - find_quantile(&cur_cdf, 0.10) as f64;
    let contrast_delta = ((ref_spread - cur_spread) / 255.0).max(-1.0).min(1.0);
    let pivot_strength = strength.max(0.0).min(1.0) * contrast_delta * 24.0;
    let points = [
        (0.0, 0.0),
        (64.0, (64.0 - pivot_strength).max(0.0).min(255.0)),
        (128.0, 128.0),
        (192.0, (192.0 + pivot_strength).max(0.0).min(255.0)),
        (255.0, 255.0),
    ];
    let mut lut = [0.0; 256];
    for idx in 0..256 {
        let x = idx as f64;
        let mut y = x;
        for window in points.windows(2) {
            let (x0, y0) = window[0];
            let (x1, y1) = window[1];
            if x >= x0 && x <= x1 {
                let t = if (x1 - x0).abs() < f64::EPSILON {
                    0.0
                } else {
                    (x - x0) / (x1 - x0)
                };
                y = y0 * (1.0 - t) + y1 * t;
                break;
            }
        }
        lut[idx] = y.max(0.0).min(255.0);
    }
    lut
}

fn extract_tone_curve_lut_from_adjustments(current_adjustments: &Value) -> Option<[f64; 256]> {
    let get_num = |key: &str| {
        current_adjustments
            .get(key)
            .and_then(|value| value.as_f64())
    };
    let has_curve_signal = [
        "exposure",
        "contrast",
        "highlights",
        "shadows",
        "whites",
        "blacks",
    ]
    .iter()
    .any(|key| current_adjustments.get(*key).is_some());
    if !has_curve_signal {
        return None;
    }

    let mut curve = identity_curve_lut();
    if let Some(exposure) = get_num("exposure") {
        let multiplier = 2.0f64.powf(exposure / 100.0);
        for value in &mut curve {
            *value *= multiplier;
        }
    }
    if let Some(contrast) = get_num("contrast") {
        let factor = 1.0 + contrast / 100.0;
        for value in &mut curve {
            *value = (*value - 128.0) * factor + 128.0;
        }
    }
    if let Some(highlights) = get_num("highlights") {
        for value in &mut curve {
            let mask = ((*value - 192.0) / 63.0).max(0.0);
            *value += mask * (highlights / 100.0 * 63.0);
        }
    }
    if let Some(shadows) = get_num("shadows") {
        for value in &mut curve {
            let mask = ((64.0 - *value) / 64.0).max(0.0);
            *value += mask * (shadows / 100.0 * 64.0);
        }
    }
    if let Some(whites) = get_num("whites") {
        for value in &mut curve {
            let mask = ((*value - 128.0) / 127.0).max(0.0);
            *value += mask * (whites / 100.0 * 50.0);
        }
    }
    if let Some(blacks) = get_num("blacks") {
        for value in &mut curve {
            let mask = ((128.0 - *value) / 128.0).max(0.0);
            *value += mask * (blacks / 100.0 * 50.0);
        }
    }
    for value in &mut curve {
        *value = value.max(0.0).min(255.0);
    }
    Some(curve)
}

fn generate_matched_curves(
    ref_img: &DynamicImage,
    cur_img: &DynamicImage,
    current_adjustments: &Value,
    strength: f64,
) -> serde_json::Value {
    let mut curves = serde_json::Map::new();

    let identity_lut = identity_curve_lut();
    let identity = lut_to_curve_points(&identity_lut);
    curves.insert("red".to_string(), serde_json::json!(identity));
    curves.insert("green".to_string(), serde_json::json!(identity));
    curves.insert("blue".to_string(), serde_json::json!(identity));

    let s = strength.max(0.0).min(1.0);
    if s <= 1e-6 {
        curves.insert(
            "luma".to_string(),
            serde_json::json!(lut_to_curve_points(&identity_lut)),
        );
        return serde_json::Value::Object(curves);
    }

    let mut ref_hist = [0u32; 256];
    let mut cur_hist = [0u32; 256];
    let mut ref_r = [0u32; 256];
    let mut ref_g = [0u32; 256];
    let mut ref_b = [0u32; 256];
    let mut cur_r = [0u32; 256];
    let mut cur_g = [0u32; 256];
    let mut cur_b = [0u32; 256];

    for p in ref_img.to_rgb8().pixels() {
        ref_r[p[0] as usize] += 1;
        ref_g[p[1] as usize] += 1;
        ref_b[p[2] as usize] += 1;
        let luma = (p[0] as f64 * 0.2126 + p[1] as f64 * 0.7152 + p[2] as f64 * 0.0722)
            .round()
            .max(0.0)
            .min(255.0) as usize;
        ref_hist[luma] += 1;
    }
    for p in cur_img.to_rgb8().pixels() {
        cur_r[p[0] as usize] += 1;
        cur_g[p[1] as usize] += 1;
        cur_b[p[2] as usize] += 1;
        let luma = (p[0] as f64 * 0.2126 + p[1] as f64 * 0.7152 + p[2] as f64 * 0.0722)
            .round()
            .max(0.0)
            .min(255.0) as usize;
        cur_hist[luma] += 1;
    }

    let weak_match_luma = histogram_match_lut(&ref_hist, &cur_hist, s * 0.18);
    let range_preserve_luma = build_range_preserve_lut(&ref_hist, &cur_hist, 0.35);
    let contrast_micro = build_contrast_micro_curve_lut(&ref_hist, &cur_hist, s * 0.45);
    let mut luma_lut = blend_lut(&identity_lut, &weak_match_luma, 0.75);
    luma_lut = blend_lut(&luma_lut, &range_preserve_luma, 0.35);
    luma_lut = blend_lut(&luma_lut, &contrast_micro, 0.25);
    if let Some(raw_tone_curve) = extract_tone_curve_lut_from_adjustments(current_adjustments) {
        luma_lut = blend_lut(&luma_lut, &raw_tone_curve, 0.22);
    }

    let red_lut = blend_lut(
        &blend_lut(
            &identity_lut,
            &histogram_match_lut(&ref_r, &cur_r, s * 0.10),
            0.70,
        ),
        &build_range_preserve_lut(&ref_r, &cur_r, 0.40),
        0.25,
    );
    let green_lut = blend_lut(
        &blend_lut(
            &identity_lut,
            &histogram_match_lut(&ref_g, &cur_g, s * 0.10),
            0.70,
        ),
        &build_range_preserve_lut(&ref_g, &cur_g, 0.40),
        0.25,
    );
    let blue_lut = blend_lut(
        &blend_lut(
            &identity_lut,
            &histogram_match_lut(&ref_b, &cur_b, s * 0.10),
            0.70,
        ),
        &build_range_preserve_lut(&ref_b, &cur_b, 0.40),
        0.25,
    );

    curves.insert("red".to_string(), json!(lut_to_curve_points(&red_lut)));
    curves.insert("green".to_string(), json!(lut_to_curve_points(&green_lut)));
    curves.insert("blue".to_string(), json!(lut_to_curve_points(&blue_lut)));
    curves.insert("luma".to_string(), json!(lut_to_curve_points(&luma_lut)));

    serde_json::Value::Object(curves)
}

fn rgb_to_hsv(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let c_max = r.max(g).max(b);
    let c_min = r.min(g).min(b);
    let delta = c_max - c_min;
    let mut h = 0.0;
    if delta > 1e-9 {
        if (c_max - r).abs() < 1e-9 {
            h = 60.0 * (((g - b) / delta) % 6.0);
        } else if (c_max - g).abs() < 1e-9 {
            h = 60.0 * (((b - r) / delta) + 2.0);
        } else {
            h = 60.0 * (((r - g) / delta) + 4.0);
        }
    }
    if h < 0.0 {
        h += 360.0;
    }
    let s = if c_max > 1e-9 { delta / c_max } else { 0.0 };
    (h, s, c_max)
}

fn hsl_influence(hue: f64, center: f64, width: f64) -> f64 {
    let dist = (hue - center).abs().min(360.0 - (hue - center).abs());
    let falloff = dist / (width * 0.5);
    (-1.5 * falloff * falloff).exp()
}

fn generate_matched_hsl(
    ref_img: &DynamicImage,
    cur_img: &DynamicImage,
    strength: f64,
    constraint_window: &DynamicConstraintWindow,
) -> serde_json::Value {
    let s = strength.max(0.0).min(1.0);
    let ranges: [(f64, f64); 8] = [
        (358.0, 35.0),
        (25.0, 45.0),
        (60.0, 40.0),
        (115.0, 90.0),
        (180.0, 60.0),
        (225.0, 60.0),
        (280.0, 55.0),
        (330.0, 50.0),
    ];
    let keys: [&str; 8] = [
        "reds", "oranges", "yellows", "greens", "aquas", "blues", "purples", "magentas",
    ];

    let mut ref_w = [0.0f64; 8];
    let mut ref_sum_sin = [0.0f64; 8];
    let mut ref_sum_cos = [0.0f64; 8];
    let mut ref_sum_sat = [0.0f64; 8];
    let mut ref_sum_lum = [0.0f64; 8];

    let mut cur_w = [0.0f64; 8];
    let mut cur_sum_sin = [0.0f64; 8];
    let mut cur_sum_cos = [0.0f64; 8];
    let mut cur_sum_sat = [0.0f64; 8];
    let mut cur_sum_lum = [0.0f64; 8];

    let accumulate = |img: &DynamicImage,
                      w: &mut [f64; 8],
                      sum_sin: &mut [f64; 8],
                      sum_cos: &mut [f64; 8],
                      sum_sat: &mut [f64; 8],
                      sum_lum: &mut [f64; 8]| {
        for p in img.to_rgb8().pixels() {
            let r = p[0] as f64 / 255.0;
            let g = p[1] as f64 / 255.0;
            let b = p[2] as f64 / 255.0;

            // 严格对齐 Shader 里的亮度与 HSL 转换
            let luma = 0.2126 * r + 0.7152 * g + 0.0722 * b;
            if luma < 0.02 || luma > 0.98 {
                continue;
            }

            let (hue, sat, _) = rgb_to_hsv(r, g, b);
            if sat < 0.05 {
                continue;
            } // 忽略无色彩像素

            let mut raw = [0.0f64; 8];
            let mut sum_raw = 0.0;
            for i in 0..8 {
                let infl = hsl_influence(hue, ranges[i].0, ranges[i].1);
                raw[i] = infl;
                sum_raw += infl;
            }
            if sum_raw < 1e-9 {
                continue;
            }

            let hue_rad = hue.to_radians();
            let h_sin = hue_rad.sin();
            let h_cos = hue_rad.cos();

            for i in 0..8 {
                let norm = raw[i] / sum_raw;
                if norm < 0.01 {
                    continue;
                }

                w[i] += norm;
                sum_sin[i] += norm * h_sin;
                sum_cos[i] += norm * h_cos;
                sum_sat[i] += norm * sat;
                sum_lum[i] += norm * luma;
            }
        }
    };

    accumulate(
        ref_img,
        &mut ref_w,
        &mut ref_sum_sin,
        &mut ref_sum_cos,
        &mut ref_sum_sat,
        &mut ref_sum_lum,
    );
    accumulate(
        cur_img,
        &mut cur_w,
        &mut cur_sum_sin,
        &mut cur_sum_cos,
        &mut cur_sum_sat,
        &mut cur_sum_lum,
    );

    // 引入色彩中介关联逻辑：色相画像邻域平滑
    // 目的：实现“中介转换”。例如目标图只有黄色，但参考图是绿色调。
    // 通过让黄色区间“借用”邻近绿色区间的画像特征，使黄色能感知到绿色的趋势，从而实现 黄->绿->莫兰迪绿 的引导。
    let smooth_profile = |w: &mut [f64; 8],
                          s: &mut [f64; 8],
                          c: &mut [f64; 8],
                          sat: &mut [f64; 8],
                          lum: &mut [f64; 8]| {
        let mut nw = [0.0; 8];
        let mut ns = [0.0; 8];
        let mut nc = [0.0; 8];
        let mut nsat = [0.0; 8];
        let mut nlum = [0.0; 8];
        for i in 0..8 {
            let prev = if i == 0 { 7 } else { i - 1 };
            let next = if i == 7 { 0 } else { i + 1 };
            // 权重分配：自身 60%，左右邻居各 20%
            let kernel = [0.2, 0.6, 0.2];
            nw[i] = w[prev] * kernel[0] + w[i] * kernel[1] + w[next] * kernel[2];
            ns[i] = s[prev] * kernel[0] + s[i] * kernel[1] + s[next] * kernel[2];
            nc[i] = c[prev] * kernel[0] + c[i] * kernel[1] + c[next] * kernel[2];
            nsat[i] = sat[prev] * kernel[0] + sat[i] * kernel[1] + sat[next] * kernel[2];
            nlum[i] = lum[prev] * kernel[0] + lum[i] * kernel[1] + lum[next] * kernel[2];
        }
        *w = nw;
        *s = ns;
        *c = nc;
        *sat = nsat;
        *lum = nlum;
    };

    smooth_profile(
        &mut ref_w,
        &mut ref_sum_sin,
        &mut ref_sum_cos,
        &mut ref_sum_sat,
        &mut ref_sum_lum,
    );
    smooth_profile(
        &mut cur_w,
        &mut cur_sum_sin,
        &mut cur_sum_cos,
        &mut cur_sum_sat,
        &mut cur_sum_lum,
    );

    let sat_guard = (1.0 - constraint_window.saturation_risk * 0.65)
        .max(0.25)
        .min(1.0);

    let mut out = serde_json::Map::new();
    for i in 0..8 {
        // 降低门限，因为平滑后每个区间都会有一定权重，从而激活“中介色”
        if ref_w[i] < 20.0 || cur_w[i] < 20.0 {
            continue;
        }

        // 1. 计算圆周平均 Hue
        let ref_mean_hue = ref_sum_sin[i].atan2(ref_sum_cos[i]).to_degrees();
        let cur_mean_hue = cur_sum_sin[i].atan2(cur_sum_cos[i]).to_degrees();

        let mut delta_hue = ref_mean_hue - cur_mean_hue;
        if delta_hue > 180.0 {
            delta_hue -= 360.0;
        }
        if delta_hue < -180.0 {
            delta_hue += 360.0;
        }

        // 引入“不相干颜色门限”：如果色相差距超过 50 度（意味着跨越了几乎两个大色相区），则认为是不相干颜色。
        // 使用平滑的余弦衰减，防止强行将红色扭曲成蓝色等不自然行为。
        let hue_affinity = (delta_hue.abs() / 50.0).min(1.0);
        let hue_gate = (1.0 - hue_affinity * hue_affinity).max(0.0);

        // Shader: hue += slider * 0.3 * 2.0; 故 slider = delta / 0.6
        let hue_slider = (delta_hue / 0.6 * s * hue_gate)
            .round()
            .max(-100.0)
            .min(100.0);

        // 2. 计算平均 Saturation
        let ref_mean_sat = ref_sum_sat[i] / ref_w[i];
        let cur_mean_sat = (cur_sum_sat[i] / cur_w[i]).max(0.01);

        // 解决偏灰元凶 3：消除双重饱和度/明度叠加 (Double Dipping)
        // RGB曲线本身已经极大改变了全图的饱和度，如果 HSL 再拉满，画面会失控。此处削弱系数为 0.4。
        let sat_slider = (((ref_mean_sat / cur_mean_sat) - 1.0) * 100.0 * s * sat_guard * 0.4)
            .round()
            .max(-100.0)
            .min(100.0);

        // 3. 计算平均 Luminance
        let ref_mean_lum = ref_sum_lum[i] / ref_w[i];
        let cur_mean_lum = (cur_sum_lum[i] / cur_w[i]).max(0.01);

        // 明度曲线 (Luma Curve) 已经完美对齐了全图的亮度。如果 HSL 再次应用亮度差值，会导致“暗的变更暗，亮的变更亮”，引发严重死灰。
        // 此处将 Luminance 的系数极大削弱 (0.1)，让它只做极其微弱的局部修饰，不干扰全局曲线。
        let lum_slider = (((ref_mean_lum / cur_mean_lum) - 1.0) * 100.0 * s * 0.1)
            .round()
            .max(-100.0)
            .min(100.0);

        if hue_slider.abs() < 1.0 && sat_slider.abs() < 1.0 && lum_slider.abs() < 1.0 {
            continue;
        }

        out.insert(
            keys[i].to_string(),
            json!({
                "hue": hue_slider,
                "saturation": sat_slider,
                "luminance": lum_slider
            }),
        );
    }

    serde_json::Value::Object(out)
}

/// 将两组特征的差异映射为滑块调整参数（优化版：更精准的感知映射）
fn map_features_to_adjustments(
    ref_feat: &StyleFeatures,
    cur_feat: &StyleFeatures,
    current_adjustments: &Value,
    tuning: StyleTransferTuning,
) -> Vec<StyleTransferSuggestion> {
    let mut suggestions = Vec::new();
    let scene_profile = style_scene_profile(ref_feat, cur_feat);

    let get_current = |key: &str, default: f64| -> f64 {
        current_adjustments
            .get(key)
            .and_then(|v| v.as_f64())
            .unwrap_or(default)
    };

    let clamp = |v: f64, min: f64, max: f64| -> f64 { v.max(min).min(max) };
    let gate = |mag: f64, threshold: f64, width: f64| -> f64 {
        ((mag - threshold) / width).max(0.0).min(1.0)
    };
    let adain_contrast_scale = if cur_feat.contrast_spread > 0.0001 {
        clamp(
            ref_feat.contrast_spread / cur_feat.contrast_spread
                * (0.85 + tuning.style_strength * 0.15),
            0.7,
            1.35,
        )
    } else {
        1.0
    };
    let adain_mean_shift =
        (ref_feat.mean_luminance - cur_feat.mean_luminance) * tuning.style_strength;
    let zone_mid_shift = ref_feat.mid_luminance_mean - cur_feat.mid_luminance_mean;
    let zone_shadow_shift = ref_feat.shadow_luminance_mean - cur_feat.shadow_luminance_mean;
    let zone_high_shift = ref_feat.highlight_luminance_mean - cur_feat.highlight_luminance_mean;
    let tonal_gap = ref_feat.p50_luminance - cur_feat.p50_luminance;
    let highlight_headroom = (target_p99_cap(ref_feat, tuning) - cur_feat.p99_luminance).max(0.0);
    let tonal_lift_bias = if tonal_gap > 0.015 {
        (tonal_gap
            * scene_profile.tonal_gain
            * (0.26 + highlight_headroom * 0.72 * scene_profile.highlight_gain))
            .max(0.0)
            .min(0.20)
    } else {
        0.0
    };

    let waveform_mid_diff = ref_feat.waveform_mid_band - cur_feat.waveform_mid_band;
    let tonal_score = tonal_alignment_score(ref_feat, cur_feat);
    let color_scale = if tonal_score > 1.25 {
        0.42
    } else if tonal_score > 0.95 {
        0.58
    } else if tonal_score > 0.70 {
        0.75
    } else {
        1.0
    };
    let chroma_limiter_base = if ref_feat.mean_saturation < 0.42 {
        0.88
    } else if ref_feat.mean_saturation < 0.34 {
        0.80
    } else {
        1.0
    };
    let chroma_limiter = (chroma_limiter_base * scene_profile.chroma_limit)
        .max(0.70)
        .min(1.05);

    let tonal_gate = gate(
        waveform_mid_diff
            .abs()
            .max(adain_mean_shift.abs())
            .max(tonal_gap.abs()),
        0.010,
        0.018,
    );
    if tonal_gate > 0.0 {
        let positive_exposure_cap = if tonal_gap > 0.0 {
            0.60 + (tonal_gap * 1.08 + highlight_headroom * 0.64).min(0.44)
        } else {
            0.56
        };
        let exposure_delta = clamp(
            (waveform_mid_diff * 1.16
                + adain_mean_shift * 0.98
                + zone_mid_shift * 0.78
                + tonal_lift_bias)
                * scene_profile.tonal_gain,
            -0.56,
            positive_exposure_cap,
        ) * tonal_gate
            * 0.3;
        let cur_exposure = get_current("exposure", 0.0);
        let new_exposure = clamp(cur_exposure + exposure_delta, -1.0, 1.0);
        suggestions.push(StyleTransferSuggestion {
            key: "exposure".to_string(),
            value: (new_exposure * 100.0).round() / 100.0,
            label: "曝光".to_string(),
            min: -5.0,
            max: 5.0,
            complex_value: None,
            reason: "先对齐波形中间带，建立基础亮度区间".to_string(),
        });
    }

    let spread_ref = ref_feat.waveform_high_band - ref_feat.waveform_low_band;
    let spread_cur = cur_feat.waveform_high_band - cur_feat.waveform_low_band;
    let spread_diff = spread_ref - spread_cur + (adain_contrast_scale - 1.0) * 0.12;
    let contrast_gate = gate(spread_diff.abs(), 0.02, 0.05);
    if contrast_gate > 0.0 {
        let contrast_delta = clamp(spread_diff * 90.0 * tuning.style_strength, -12.0, 12.0) * 0.3;
        let cur_contrast = get_current("contrast", 0.0);
        let new_contrast = clamp(cur_contrast + contrast_delta * contrast_gate, -25.0, 25.0);
        suggestions.push(StyleTransferSuggestion {
            key: "contrast".to_string(),
            value: new_contrast.round(),
            label: "对比度".to_string(),
            min: -100.0,
            max: 100.0,
            complex_value: None,
            reason: "按波形上下带间距匹配影调对比".to_string(),
        });
    }

    let high_band_diff = ref_feat.waveform_high_band - cur_feat.waveform_high_band;
    let clip_diff = ref_feat.clipped_highlight_ratio - cur_feat.clipped_highlight_ratio;
    let highlight_gate = gate(
        high_band_diff
            .abs()
            .max(clip_diff.abs() * 0.8)
            .max(zone_high_shift.abs()),
        0.012,
        0.025,
    );
    if highlight_gate > 0.0 {
        let hl_delta = clamp(
            high_band_diff * 60.0 + zone_high_shift * 34.0 - clip_diff * 120.0,
            -30.0,
            20.0,
        ) * (0.9 + tuning.style_strength * 0.1)
            * scene_profile.highlight_gain
            * highlight_gate
            * 0.3;
        let cur_hl = get_current("highlights", 0.0);
        let new_hl = clamp(cur_hl + hl_delta, -25.0, 25.0);
        suggestions.push(StyleTransferSuggestion {
            key: "highlights".to_string(),
            value: new_hl.round(),
            label: "高光".to_string(),
            min: -100.0,
            max: 100.0,
            complex_value: None,
            reason: "约束高光上沿，避免波形顶部挤压和过曝".to_string(),
        });
    }

    let low_band_diff = ref_feat.waveform_low_band - cur_feat.waveform_low_band;
    let shadow_gate = gate(
        low_band_diff.abs().max(zone_shadow_shift.abs()),
        0.012,
        0.025,
    );
    if shadow_gate > 0.0 {
        let lift_bias = if tonal_gap > 0.015 {
            (tonal_gap * 18.0).min(8.0)
        } else {
            0.0
        };
        let sh_delta = clamp(
            low_band_diff * 68.0 + zone_shadow_shift * 55.0 + lift_bias,
            -20.0,
            32.0,
        ) * (0.9 + tuning.style_strength * 0.1)
            * scene_profile.shadow_gain
            * shadow_gate
            * 0.3;
        let cur_sh = get_current("shadows", 0.0);
        let new_sh = clamp(cur_sh + sh_delta, -25.0, 25.0);
        suggestions.push(StyleTransferSuggestion {
            key: "shadows".to_string(),
            value: new_sh.round(),
            label: "阴影".to_string(),
            min: -100.0,
            max: 100.0,
            complex_value: None,
            reason: "对齐波形低部区间，控制暗部密度".to_string(),
        });
    }

    let p99_diff = ref_feat.p99_luminance - cur_feat.p99_luminance;
    let white_gate = gate(p99_diff.abs().max(clip_diff.abs() * 0.9), 0.012, 0.026);
    if white_gate > 0.0 {
        let whites_delta = clamp(
            (p99_diff * 80.0 - clip_diff * 140.0) * scene_profile.highlight_gain,
            -18.0,
            14.0,
        ) * white_gate
            * 0.3;
        let cur_whites = get_current("whites", 0.0);
        let new_whites = clamp(cur_whites + whites_delta, -25.0, 25.0);
        suggestions.push(StyleTransferSuggestion {
            key: "whites".to_string(),
            value: new_whites.round(),
            label: "白色色阶".to_string(),
            min: -100.0,
            max: 100.0,
            complex_value: None,
            reason: "通过 99 分位亮度匹配白场上限".to_string(),
        });
    }

    let p10_diff = ref_feat.p10_luminance - cur_feat.p10_luminance;
    let black_gate = gate(p10_diff.abs(), 0.012, 0.022);
    if black_gate > 0.0 {
        let blacks_delta =
            clamp(p10_diff * 70.0 * scene_profile.shadow_gain, -15.0, 15.0) * black_gate;
        let cur_blacks = get_current("blacks", 0.0);
        let new_blacks = clamp(cur_blacks + blacks_delta, -45.0, 45.0);
        suggestions.push(StyleTransferSuggestion {
            key: "blacks".to_string(),
            value: new_blacks.round(),
            label: "黑色色阶".to_string(),
            min: -100.0,
            max: 100.0,
            complex_value: None,
            reason: "通过 10 分位亮度匹配黑场下限".to_string(),
        });
    }

    // ===== 阶段二：影调收敛后再匹配色彩风格 =====
    let temp_diff = ref_feat.rb_ratio - cur_feat.rb_ratio;
    let hue_diff = ref_feat.hue_mean - cur_feat.hue_mean;
    let temp_gate = gate(temp_diff.abs().max(hue_diff.abs() * 0.7), 0.018, 0.042);
    if temp_gate > 0.0 {
        let skin_temp_bias = if ref_feat.skin_ratio > 0.015 && cur_feat.skin_ratio > 0.015 {
            (ref_feat.skin_rb_ratio - cur_feat.skin_rb_ratio) * 8.0
        } else {
            0.0
        };
        let tint_diff = ref_feat.gb_ratio - cur_feat.gb_ratio;
        let temp_delta = clamp(
            (temp_diff * 32.0 + skin_temp_bias + (-tint_diff) * 5.0 + hue_diff * 18.0)
                * color_scale,
            -30.0,
            30.0,
        ) * temp_gate
            * 0.3;
        let cur_temp = get_current("temperature", 0.0);
        let new_temp = clamp(cur_temp + temp_delta, -25.0, 25.0);
        suggestions.push(StyleTransferSuggestion {
            key: "temperature".to_string(),
            value: new_temp.round(),
            label: "色温".to_string(),
            min: -100.0,
            max: 100.0,
            complex_value: None,
            reason: format!(
                "参考图色调{}",
                if temp_diff > 0.0 { "偏暖" } else { "偏冷" }
            ),
        });
    }

    // 6. 色调偏移 (tint): 保守映射
    let tint_diff = ref_feat.gb_ratio - cur_feat.gb_ratio;
    let tint_gate = gate(tint_diff.abs().max(hue_diff.abs() * 0.8), 0.018, 0.042);
    if tint_gate > 0.0 {
        let tint_delta = clamp(
            (-tint_diff * 27.0 + temp_diff * 6.5 - hue_diff * 14.0) * color_scale,
            -24.0,
            24.0,
        ) * tint_gate
            * 0.3;
        let cur_tint = get_current("tint", 0.0);
        let new_tint = clamp(cur_tint + tint_delta, -20.0, 20.0);
        suggestions.push(StyleTransferSuggestion {
            key: "tint".to_string(),
            value: new_tint.round(),
            label: "色调偏移".to_string(),
            min: -100.0,
            max: 100.0,
            complex_value: None,
            reason: format!(
                "参考图色调{}",
                if tint_diff > 0.0 {
                    "偏绿"
                } else {
                    "偏品红"
                }
            ),
        });
    }

    // 7. 饱和度 (saturation): 保守映射
    let sat_diff = ref_feat.mean_saturation - cur_feat.mean_saturation;
    let sat_gate = gate(sat_diff.abs(), 0.015, 0.045);
    if sat_gate > 0.0 {
        let sat_delta = clamp(
            sat_diff * 70.0 * tuning.style_strength * color_scale * chroma_limiter,
            -22.0,
            22.0,
        ) * sat_gate;
        let cur_sat = get_current("saturation", 0.0);
        let new_sat = clamp(cur_sat + sat_delta, -50.0, 50.0);
        suggestions.push(StyleTransferSuggestion {
            key: "saturation".to_string(),
            value: new_sat.round(),
            label: "饱和度".to_string(),
            min: -100.0,
            max: 100.0,
            complex_value: None,
            reason: format!(
                "参考图色彩{}",
                if sat_diff > 0.0 {
                    "更鲜艳"
                } else {
                    "更淡雅"
                }
            ),
        });
    }

    // 8. 自然饱和度 (vibrance): 保守映射
    let vib_diff = ref_feat.saturation_spread - cur_feat.saturation_spread;
    let vib_gate = gate(vib_diff.abs(), 0.010, 0.035);
    if vib_gate > 0.0 {
        let vib_delta = clamp(
            vib_diff * 82.0 * tuning.style_strength * color_scale * chroma_limiter,
            -18.0,
            18.0,
        ) * vib_gate;
        let cur_vib = get_current("vibrance", 0.0);
        let new_vib = clamp(cur_vib + vib_delta, -50.0, 50.0);
        suggestions.push(StyleTransferSuggestion {
            key: "vibrance".to_string(),
            value: new_vib.round(),
            label: "自然饱和度".to_string(),
            min: -100.0,
            max: 100.0,
            complex_value: None,
            reason: format!(
                "参考图色彩层次{}",
                if vib_diff > 0.0 {
                    "更丰富"
                } else {
                    "更统一"
                }
            ),
        });
    }

    // 9. 清晰度 (clarity): 保守映射
    let lap_diff = ref_feat.laplacian_variance - cur_feat.laplacian_variance;
    let clarity_gate = gate(lap_diff.abs(), 70.0, 120.0);
    if clarity_gate > 0.0 {
        let clarity_delta = clamp(lap_diff / 300.0, -25.0, 25.0) * clarity_gate;
        let cur_clarity = get_current("clarity", 0.0);
        let new_clarity = clamp(cur_clarity + clarity_delta, -50.0, 50.0);
        suggestions.push(StyleTransferSuggestion {
            key: "clarity".to_string(),
            value: new_clarity.round(),
            label: "清晰度".to_string(),
            min: -100.0,
            max: 100.0,
            complex_value: None,
            reason: format!(
                "参考图纹理{}",
                if lap_diff > 0.0 {
                    "更锐利"
                } else {
                    "更柔和"
                }
            ),
        });
    }

    // 10. 暗角 (vignetteAmount): 保守映射
    let vig_diff = ref_feat.vignette_diff - cur_feat.vignette_diff;
    let vignette_evidence = ref_feat.vignette_diff > 0.075
        && (ref_feat.vignette_diff - cur_feat.vignette_diff) > 0.022
        && ref_feat.p50_luminance < 0.88
        && ref_feat.p90_luminance < 0.985;
    if vig_diff.abs() > 0.03 {
        let vig_delta = clamp(-vig_diff * 50.0, -25.0, 25.0);
        let cur_vig = get_current("vignetteAmount", 0.0);
        if vig_delta >= 0.0 || vignette_evidence {
            let new_vig = clamp(cur_vig + vig_delta, -50.0, 50.0);
            suggestions.push(StyleTransferSuggestion {
                key: "vignetteAmount".to_string(),
                value: new_vig.round(),
                label: "暗角".to_string(),
                min: -100.0,
                max: 100.0,
                complex_value: None,
                reason: format!(
                    "参考图暗角{}",
                    if vig_diff > 0.0 {
                        "更明显"
                    } else {
                        "更轻微"
                    }
                ),
            });
        }
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
    let impact_limit = if brightness_impact > 0.0 { 82.0 } else { 58.0 };
    if brightness_impact.abs() > impact_limit {
        let scale = impact_limit / brightness_impact.abs();
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
    let p99_cap_limit = if classify_tonal_style(ref_feat) == "高调" {
        0.985 + (1.0 - tuning.highlight_guard_strength) * 0.012
    } else {
        0.965 + (1.0 - tuning.highlight_guard_strength) * 0.012
    };
    if predicted_p99 > p99_cap_limit {
        let overflow = (predicted_p99 - p99_cap_limit).max(0.0);
        let exposure_pull = (0.02 + overflow * 0.42) * tuning.highlight_guard_strength;
        let highlights_pull = (1.5 + overflow * 110.0) * tuning.highlight_guard_strength;
        let whites_pull = (2.2 + overflow * 130.0) * tuning.highlight_guard_strength;
        for s in &mut suggestions {
            if s.key == "exposure" {
                s.value = (s.value - exposure_pull).max(-2.5);
            }
            if s.key == "highlights" {
                s.value = (s.value - highlights_pull).max(-80.0);
            }
            if s.key == "whites" {
                s.value = (s.value - whites_pull).max(-80.0);
            }
        }
    }
    let predicted_p99_after_protect = cur_feat.p99_luminance
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

    let predicted_mid = cur_feat.p50_luminance
        + suggestions.iter().fold(0.0, |acc, s| {
            let cur = get_current(&s.key, 0.0);
            let delta = s.value - cur;
            acc + match s.key.as_str() {
                "exposure" => delta * 0.17,
                "highlights" => delta * 0.0008,
                "shadows" => delta * 0.0009,
                "whites" => delta * 0.0007,
                "contrast" => delta * 0.0004,
                _ => 0.0,
            }
        });
    let mid_gap = ref_feat.p50_luminance - predicted_mid;
    if mid_gap > 0.03 && predicted_p99_after_protect < p99_cap_limit - 0.015 {
        let safe_lift = (mid_gap * 0.32).max(0.02).min(0.10);
        for s in &mut suggestions {
            if s.key == "exposure" {
                s.value = (s.value + safe_lift).min(2.5);
            }
            if s.key == "highlights" {
                s.value = (s.value + 2.0).min(80.0);
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
            let pred_feat =
                estimate_features_from_adjustments(cur_feat, &current_values, &candidate_values);
            let pred_vec = style_metric_vector(&pred_feat);
            let weights = style_metric_weights(ref_feat, &pred_feat, tuning);
            let mut residual = [0.0f64; 15];
            for i in 0..15 {
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
                let plus_feat = estimate_features_from_adjustments(
                    cur_feat,
                    &current_values,
                    &candidate_values,
                );
                let plus_vec = style_metric_vector(&plus_feat);
                candidate_values.insert(key.clone(), cur_val);
                let mut num = 0.0;
                let mut den = 1e-6;
                for i in 0..15 {
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
                    let predicted = estimate_features_from_adjustments(
                        cur_feat,
                        &current_values,
                        &candidate_values,
                    );
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
    for _ in 0..2 {
        let pred = estimate_features_for_suggestions(cur_feat, current_adjustments, &suggestions);
        let target_cap = target_p99_cap(ref_feat, tuning);
        let mid_gap = ref_feat.p50_luminance - pred.p50_luminance;
        let low_gap = ref_feat.waveform_low_band - pred.waveform_low_band;
        let high_gap = ref_feat.waveform_high_band - pred.waveform_high_band;
        let tonal_gap_total = mid_gap.abs() + low_gap.abs() * 0.6 + high_gap.abs() * 0.6;
        if tonal_gap_total < 0.020 {
            break;
        }
        let mut exp_delta = (mid_gap * 0.92
            + (ref_feat.mid_luminance_mean - pred.mid_luminance_mean) * 0.45)
            .max(-0.14)
            .min(0.16);
        if mid_gap > 0.025 && pred.p99_luminance < target_cap - 0.020 {
            exp_delta = (exp_delta + 0.025).min(0.18);
        }
        if pred.p99_luminance > target_cap - 0.004 {
            exp_delta = exp_delta.min(0.03);
        }
        let mut high_delta = (high_gap * 22.0
            + (ref_feat.highlight_luminance_mean - pred.highlight_luminance_mean) * 16.0)
            .max(-4.5)
            .min(4.5);
        if pred.p99_luminance > target_cap {
            high_delta = high_delta.min(-1.0);
        }
        let shadow_delta = (low_gap * 22.0
            + (ref_feat.shadow_luminance_mean - pred.shadow_luminance_mean) * 18.0)
            .max(-4.5)
            .min(4.5);
        let mut changed = false;
        changed |= upsert_suggestion_delta(
            &mut suggestions,
            current_adjustments,
            "exposure",
            exp_delta,
            "明暗锚点回正：对齐参考图中间调亮度",
            "tonal",
        );
        changed |= upsert_suggestion_delta(
            &mut suggestions,
            current_adjustments,
            "highlights",
            high_delta,
            "明暗锚点回正：对齐参考图亮部波形",
            "tonal",
        );
        changed |= upsert_suggestion_delta(
            &mut suggestions,
            current_adjustments,
            "shadows",
            shadow_delta,
            "明暗锚点回正：对齐参考图暗部波形",
            "tonal",
        );
        if !changed {
            break;
        }
    }
    for _ in 0..2 {
        let pred = estimate_features_for_suggestions(cur_feat, current_adjustments, &suggestions);
        let target_cap = target_p99_cap(ref_feat, tuning);
        let mid_gap = ref_feat.p50_luminance - pred.p50_luminance;
        let low_gap = ref_feat.waveform_low_band - pred.waveform_low_band;
        let high_gap = ref_feat.waveform_high_band - pred.waveform_high_band;
        let p10_gap = ref_feat.p10_luminance - pred.p10_luminance;
        let p90_gap = ref_feat.p90_luminance - pred.p90_luminance;
        let mut changed = false;
        changed |= upsert_suggestion_delta(
            &mut suggestions,
            current_adjustments,
            "exposure",
            (mid_gap * 0.55 * scene_profile.tonal_gain)
                .max(-0.07)
                .min(0.12),
            "组合明暗回正：以曝光对齐中间调",
            "tonal",
        );
        let white_delta = if high_gap > 0.0 && pred.p99_luminance < target_cap - 0.010 {
            ((high_gap * 18.0 + p90_gap * 16.0) * scene_profile.highlight_gain)
                .max(-2.5)
                .min(4.5)
        } else {
            ((high_gap * 14.0 + p90_gap * 11.0) * scene_profile.highlight_gain)
                .max(-4.5)
                .min(2.5)
        };
        changed |= upsert_suggestion_delta(
            &mut suggestions,
            current_adjustments,
            "whites",
            white_delta,
            "组合明暗回正：对白场与高亮上沿做联合校正",
            "tonal",
        );
        changed |= upsert_suggestion_delta(
            &mut suggestions,
            current_adjustments,
            "blacks",
            ((low_gap * 16.0 + p10_gap * 16.0) * scene_profile.shadow_gain)
                .max(-4.0)
                .min(4.0),
            "组合明暗回正：对黑场与暗部密度做联合校正",
            "tonal",
        );
        if !changed {
            break;
        }
    }
    {
        let pred = estimate_features_for_suggestions(cur_feat, current_adjustments, &suggestions);
        let rb_gap = ref_feat.rb_ratio - pred.rb_ratio;
        let gb_gap = ref_feat.gb_ratio - pred.gb_ratio;
        let sat_overshoot = pred.mean_saturation - ref_feat.mean_saturation;
        let spread_overshoot = pred.saturation_spread - ref_feat.saturation_spread;
        let mut changed = false;
        if rb_gap.abs() > 0.006 || gb_gap.abs() > 0.006 {
            changed |= upsert_suggestion_delta(
                &mut suggestions,
                current_adjustments,
                "temperature",
                ((rb_gap * 8.5 + (-gb_gap) * 2.5) * scene_profile.color_residual_gain)
                    .max(-2.0)
                    .min(2.0),
                "色准回正：抑制冷暖偏移残差",
                "color",
            );
            changed |= upsert_suggestion_delta(
                &mut suggestions,
                current_adjustments,
                "tint",
                ((-gb_gap * 8.0 + rb_gap * 2.2) * scene_profile.color_residual_gain)
                    .max(-2.0)
                    .min(2.0),
                "色准回正：抑制绿品偏移残差",
                "color",
            );
        }
        if sat_overshoot > 0.010 || spread_overshoot > 0.012 {
            changed |= upsert_suggestion_delta(
                &mut suggestions,
                current_adjustments,
                "saturation",
                (-(sat_overshoot * 24.0 + spread_overshoot * 9.0) * scene_profile.chroma_limit)
                    .max(-3.0)
                    .min(-0.8),
                "色差保护：回收过量饱和度",
                "color",
            );
            changed |= upsert_suggestion_delta(
                &mut suggestions,
                current_adjustments,
                "vibrance",
                (-(spread_overshoot * 20.0 + sat_overshoot * 7.0) * scene_profile.chroma_limit)
                    .max(-2.5)
                    .min(-0.6),
                "色差保护：回收同系色扩散",
                "color",
            );
        }
        if changed {
            let _ = estimate_features_for_suggestions(cur_feat, current_adjustments, &suggestions);
        }
    }
    apply_reference_normalize_pass(
        ref_feat,
        cur_feat,
        current_adjustments,
        &mut suggestions,
        tuning,
    );
    apply_low_confidence_damping(
        ref_feat,
        cur_feat,
        current_adjustments,
        &mut suggestions,
        tuning,
    );

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
    pure_algorithm: Option<bool>,
    enable_expert_preset: Option<bool>,
    enable_feature_mapping: Option<bool>,
    enable_auto_refine: Option<bool>,
    enable_lut: Option<bool>,
    enable_vlm: Option<bool>,
    llm_endpoint: Option<String>,
    llm_api_key: Option<String>,
    llm_model: Option<String>,
    app_handle: tauri::AppHandle,
) -> Result<StyleTransferResponse, String> {
    let tuning = StyleTransferTuning::from_options(
        style_strength,
        highlight_guard_strength,
        skin_protect_strength,
    );
    let pure_algorithm = pure_algorithm.unwrap_or(false);
    let enable_expert_preset = if pure_algorithm {
        false
    } else {
        enable_expert_preset.unwrap_or(true)
    };
    let enable_feature_mapping = if pure_algorithm {
        false
    } else {
        enable_feature_mapping.unwrap_or(true)
    };
    let enable_auto_refine = if pure_algorithm {
        false
    } else {
        enable_auto_refine.unwrap_or(true)
    };
    let enable_lut = if pure_algorithm {
        false
    } else {
        enable_lut.unwrap_or(true)
    };
    let enable_vlm = if pure_algorithm {
        false
    } else {
        enable_vlm.unwrap_or(true)
    };
    validate_style_transfer_paths(&reference_path, &current_image_path)?;
    let (ref_img, cur_img) =
        load_style_transfer_images(&reference_path, &current_image_path).await?;

    let endpoint_for_check = llm_endpoint.clone();
    let current_adjustments_for_vlm = current_adjustments.clone();

    let (algorithm_result, suffix, early_exit_res, ctx_images) =
        tokio::task::spawn_blocking(move || {
            let ctx = build_style_transfer_context(
                ref_img,
                cur_img,
                &current_adjustments,
                tuning,
                enable_expert_preset,
            );
            let (early_exit_threshold, llm_trigger) = adaptive_style_thresholds(
                &ctx.ref_features,
                &ctx.cur_features,
                &ctx.baseline_error,
            );
            if !pure_algorithm && ctx.baseline_error.total < early_exit_threshold {
                let suffix = build_expert_preset_suffix(&ctx);
                let mut res = build_style_transfer_early_exit_response(&ctx);
                if !suffix.is_empty() {
                    res.understanding = format!("{}{}", res.understanding, suffix);
                }
                return Ok::<
                    (
                        Option<AlgorithmPipelineResult>,
                        String,
                        Option<StyleTransferResponse>,
                        Option<(DynamicImage, DynamicImage)>,
                    ),
                    String,
                >((None, String::new(), Some(res), None));
            }
            let needs_vlm = enable_vlm
                && endpoint_for_check.is_some()
                && ctx.baseline_error.total >= llm_trigger;
            let images = if needs_vlm {
                Some((ctx.ref_img.clone(), ctx.cur_img.clone()))
            } else {
                None
            };

            let algorithm_result = run_algorithm_pipeline(
                &ctx,
                &current_adjustments,
                StyleTransferAlgoOptions {
                    pure_algorithm,
                    enable_feature_mapping,
                    enable_auto_refine,
                    enable_expert_preset,
                    enable_lut,
                },
            );
            let suffix = build_expert_preset_suffix(&ctx);
            Ok::<
                (
                    Option<AlgorithmPipelineResult>,
                    String,
                    Option<StyleTransferResponse>,
                    Option<(DynamicImage, DynamicImage)>,
                ),
                String,
            >((Some(algorithm_result), suffix, None, images))
        })
        .await
        .map_err(|e| format!("分析任务失败: {}", e))??;

    if let Some(res) = early_exit_res {
        return Ok(res);
    }

    let algorithm_result = algorithm_result.unwrap();
    let mut final_res = build_algorithm_response(
        algorithm_result.adjustments.clone(),
        algorithm_result.style_debug.clone(),
        &suffix,
        true,
    );

    if let Some((ref_img, cur_img)) = ctx_images {
        if let Some(endpoint) = llm_endpoint {
            let _ = app_handle.emit(
                "style-transfer-stream",
                serde_json::json!({
                    "chunk_type": "thinking",
                    "text": "\n\n正在启动视觉大模型进行深度风格匹配...\n",
                    "result": Option::<crate::llm_chat::ChatAdjustResponse>::None
                }),
            );

            match run_vlm_refinement(
                &endpoint,
                llm_api_key.as_deref(),
                llm_model.as_deref(),
                &ref_img,
                &cur_img,
                &current_adjustments_for_vlm,
                &algorithm_result.adjustments,
                &app_handle,
            )
            .await
            {
                Ok(vlm_res) => {
                    if !vlm_res.understanding.is_empty() {
                        final_res.understanding = format!(
                            "{}\n\n[视觉模型微调]\n{}",
                            final_res.understanding, vlm_res.understanding
                        );
                    }
                    if !vlm_res.adjustments.is_empty() {
                        let mut keyed: HashMap<String, usize> = HashMap::new();
                        for (idx, s) in final_res.adjustments.iter().enumerate() {
                            keyed.insert(s.key.clone(), idx);
                        }
                        for s in vlm_res.adjustments.clone() {
                            let candidate = StyleTransferSuggestion {
                                key: s.key.clone(),
                                value: s.get_f64_value(),
                                complex_value: s.complex_value,
                                label: s.label,
                                min: s.min,
                                max: s.max,
                                reason: s.reason,
                            };
                            if let Some(idx) = keyed.get(&candidate.key).copied() {
                                final_res.adjustments[idx] = candidate;
                            } else {
                                keyed.insert(candidate.key.clone(), final_res.adjustments.len());
                                final_res.adjustments.push(candidate);
                            }
                        }
                    }
                    let _ = app_handle.emit("style-transfer-stream", serde_json::json!({
                        "chunk_type": "done",
                        "text": "",
                        "result": crate::llm_chat::ChatAdjustResponse {
                            understanding: final_res.understanding.clone(),
                            adjustments: final_res.adjustments.iter().map(|s| crate::llm_chat::AdjustmentSuggestion {
                                key: s.key.clone(),
                                value: serde_json::json!(s.value),
                                complex_value: s.complex_value.clone(),
                                label: s.label.clone(),
                                min: s.min,
                                max: s.max,
                                reason: s.reason.clone()
                            }).collect(),
                            style_debug: final_res.style_debug.clone().map(|d| serde_json::to_value(d).unwrap()),
                            constraint_debug: None
                        }
                    }));
                }
                Err(e) => {
                    let _ = app_handle.emit(
                        "style-transfer-stream",
                        serde_json::json!({
                            "chunk_type": "error",
                            "text": format!("\n视觉模型微调失败: {}\n", e),
                            "result": Option::<crate::llm_chat::ChatAdjustResponse>::None
                        }),
                    );
                }
            }
        }
    } else {
        let _ = app_handle.emit("style-transfer-stream", serde_json::json!({
            "chunk_type": "done",
            "text": "",
            "result": crate::llm_chat::ChatAdjustResponse {
                understanding: final_res.understanding.clone(),
                adjustments: final_res.adjustments.iter().map(|s| crate::llm_chat::AdjustmentSuggestion {
                    key: s.key.clone(),
                    value: serde_json::json!(s.value),
                    complex_value: s.complex_value.clone(),
                    label: s.label.clone(),
                    min: s.min,
                    max: s.max,
                    reason: s.reason.clone()
                }).collect(),
                style_debug: final_res.style_debug.clone().map(|d| serde_json::to_value(d).unwrap()),
                constraint_debug: None
            }
        }));
    }

    Ok(final_res)
}

fn image_to_base64_jpeg(img: &DynamicImage) -> String {
    let mut buf = Cursor::new(Vec::new());
    let (w, h) = img.dimensions();
    let resized = if w > 1024 || h > 1024 {
        img.resize(1024, 1024, image::imageops::FilterType::Triangle)
    } else {
        img.clone()
    };
    resized
        .write_to(&mut buf, image::ImageFormat::Jpeg)
        .unwrap();
    general_purpose::STANDARD.encode(buf.get_ref())
}

async fn run_vlm_refinement(
    endpoint: &str,
    api_key: Option<&str>,
    model: Option<&str>,
    ref_img: &DynamicImage,
    cur_img: &DynamicImage,
    current_adjustments: &Value,
    suggested_adjustments: &[StyleTransferSuggestion],
    app_handle: &tauri::AppHandle,
) -> Result<crate::llm_chat::ChatAdjustResponse, String> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(180))
        .build()
        .map_err(|e| e.to_string())?;

    let model_name = model.unwrap_or("qwen2.5vl:7b").trim();
    let model_name = if model_name.is_empty() || model_name.eq_ignore_ascii_case("auto") {
        "qwen2.5vl:7b"
    } else {
        model_name
    };

    let ref_b64 = image_to_base64_jpeg(ref_img);
    let cur_b64 = image_to_base64_jpeg(cur_img);

    let system_prompt = "你是一位专业摄影师和调色专家。请观察参考图(第一张)和当前图(第二张)，以及初步的参数调整建议。请根据参考图的风格，对当前图的参数进行微调，以达到更好的风格匹配效果。请以JSON格式返回结果，包含understanding和adjustments。";
    let user_text = format!(
        "当前参数: {}\n初步建议: {}",
        current_adjustments,
        serde_json::to_string(suggested_adjustments).unwrap_or_default()
    );

    let messages = json!([
        {
            "role": "system",
            "content": system_prompt
        },
        {
            "role": "user",
            "content": [
                {
                    "type": "text",
                    "text": user_text
                },
                {
                    "type": "image_url",
                    "image_url": {
                        "url": format!("data:image/jpeg;base64,{}", ref_b64)
                    }
                },
                {
                    "type": "image_url",
                    "image_url": {
                        "url": format!("data:image/jpeg;base64,{}", cur_b64)
                    }
                }
            ]
        }
    ]);

    let url = format!("{}/v1/chat/completions", endpoint.trim_end_matches('/'));
    let request_body = json!({
        "model": model_name,
        "messages": messages,
        "temperature": 0.3,
        "stream": true
    });

    let mut req = client.post(&url).json(&request_body);
    if let Some(key) = api_key {
        if !key.is_empty() {
            req = req.bearer_auth(key);
        }
    }

    let response = req.send().await.map_err(|e| format!("请求失败: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("LLM 返回错误 {}: {}", status, body));
    }

    let mut full_content = String::new();
    let mut in_thinking = false;
    let mut thinking_buffer = String::new();

    use futures_util::StreamExt;
    let mut stream = response.bytes_stream();
    let mut line_buffer = String::new();

    use tauri::Emitter;

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
                    if let Some(delta_content) = sse_json["choices"][0]["delta"]["content"].as_str()
                    {
                        if delta_content.is_empty() {
                            continue;
                        }

                        full_content.push_str(delta_content);

                        if delta_content.contains("<think>") {
                            in_thinking = true;
                        }

                        if in_thinking {
                            thinking_buffer.push_str(delta_content);
                            let clean =
                                delta_content.replace("<think>", "").replace("</think>", "");
                            if !clean.is_empty() {
                                let _ = app_handle.emit(
                                    "style-transfer-stream",
                                    json!({
                                        "chunk_type": "thinking",
                                        "text": clean,
                                        "result": Option::<crate::llm_chat::ChatAdjustResponse>::None
                                    }),
                                );
                            }
                        }

                        if delta_content.contains("</think>") {
                            in_thinking = false;
                            thinking_buffer.clear();
                            let _ = app_handle.emit(
                                "style-transfer-stream",
                                json!({
                                    "chunk_type": "thinking",
                                    "text": "\n正在生成微调参数...\n",
                                    "result": Option::<crate::llm_chat::ChatAdjustResponse>::None
                                }),
                            );
                        }
                    }
                }
            }
        }
    }

    let json_str = crate::llm_chat::extract_json(&full_content)?;
    let parsed: crate::llm_chat::ChatAdjustResponse =
        serde_json::from_str(&json_str).map_err(|e| {
            format!(
                "解析 JSON 失败: {}，原始内容: {}",
                e,
                &full_content[..full_content.len().min(500)]
            )
        })?;

    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};

    fn assert_finite_features(feat: &StyleFeatures) {
        let values = [
            feat.mean_luminance,
            feat.highlight_ratio,
            feat.shadow_ratio,
            feat.contrast_spread,
            feat.p10_luminance,
            feat.p50_luminance,
            feat.p90_luminance,
            feat.p99_luminance,
            feat.clipped_highlight_ratio,
            feat.waveform_low_band,
            feat.waveform_mid_band,
            feat.waveform_high_band,
            feat.rb_ratio,
            feat.gb_ratio,
            feat.mean_saturation,
            feat.saturation_spread,
            feat.shadow_luminance_mean,
            feat.mid_luminance_mean,
            feat.highlight_luminance_mean,
            feat.skin_ratio,
            feat.skin_luminance_mean,
            feat.skin_rb_ratio,
            feat.hue_mean,
            feat.hue_spread,
            feat.laplacian_variance,
            feat.vignette_diff,
        ];
        for v in values {
            assert!(v.is_finite());
        }
        assert!((0.0..=1.0).contains(&feat.highlight_ratio));
        assert!((0.0..=1.0).contains(&feat.shadow_ratio));
        assert!((0.0..=1.0).contains(&feat.clipped_highlight_ratio));
        assert!((0.0..=1.0).contains(&feat.hue_mean));
        assert!((0.0..=1.0).contains(&feat.hue_spread));
    }

    #[test]
    fn extract_features_on_uniform_image_is_stable() {
        let img = RgbImage::from_pixel(128, 128, Rgb([180, 140, 120]));
        let feat = extract_features(&DynamicImage::ImageRgb8(img));
        assert_finite_features(&feat);
        assert!(feat.mean_luminance > 0.0);
        assert!(feat.rb_ratio > 0.0);
        assert!(feat.gb_ratio > 0.0);
    }

    #[test]
    fn extract_features_on_extreme_checker_is_stable() {
        let mut img = RgbImage::new(192, 192);
        for y in 0..192 {
            for x in 0..192 {
                let px = if (x + y) % 2 == 0 {
                    Rgb([255, 255, 255])
                } else {
                    Rgb([0, 0, 0])
                };
                img.put_pixel(x, y, px);
            }
        }
        let feat = extract_features(&DynamicImage::ImageRgb8(img));
        assert_finite_features(&feat);
        assert!(feat.p99_luminance >= feat.p90_luminance);
        assert!(feat.p90_luminance >= feat.p50_luminance);
        assert!(feat.p50_luminance >= feat.p10_luminance);
    }

    #[test]
    fn extract_features_on_black_and_white_images_are_stable() {
        let black = RgbImage::from_pixel(96, 96, Rgb([0, 0, 0]));
        let white = RgbImage::from_pixel(96, 96, Rgb([255, 255, 255]));
        let feat_black = extract_features(&DynamicImage::ImageRgb8(black));
        let feat_white = extract_features(&DynamicImage::ImageRgb8(white));
        assert_finite_features(&feat_black);
        assert_finite_features(&feat_white);
        assert!(feat_black.p10_luminance <= feat_black.p50_luminance);
        assert!(feat_black.p50_luminance <= feat_black.p90_luminance);
        assert!(feat_white.p10_luminance <= feat_white.p50_luminance);
        assert!(feat_white.p50_luminance <= feat_white.p90_luminance);
        assert!(feat_black.waveform_low_band <= feat_black.waveform_mid_band);
        assert!(feat_black.waveform_mid_band <= feat_black.waveform_high_band);
        assert!(feat_white.waveform_low_band <= feat_white.waveform_mid_band);
        assert!(feat_white.waveform_mid_band <= feat_white.waveform_high_band);
    }
}

use crate::color_matching::{TPS, kmeans_plus_plus, lab_to_rgb, rgb_to_lab, sinkhorn_ot};
use nalgebra::{DMatrix, DVector, Vector3};

fn extract_lab_pixels_downsampled(img: &image::DynamicImage) -> Vec<Vector3<f64>> {
    use image::GenericImageView;
    let (width, height) = img.dimensions();
    let step = (width * height / 5000).max(1);
    let mut pixels = Vec::new();

    for (i, (_, _, pixel)) in img.pixels().enumerate() {
        if i as u32 % step == 0 {
            let rgb = Vector3::new(
                pixel[0] as f64 / 255.0,
                pixel[1] as f64 / 255.0,
                pixel[2] as f64 / 255.0,
            );
            pixels.push(rgb_to_lab(&rgb));
        }
    }
    pixels
}

fn generate_3d_lut_with_ot_tps(
    cur_img: &image::DynamicImage,
    ref_img: &image::DynamicImage,
    lut_size: usize,
) -> Option<Vec<f32>> {
    let cur_pixels = extract_lab_pixels_downsampled(cur_img);
    let ref_pixels = extract_lab_pixels_downsampled(ref_img);

    let k = 32;
    let (cur_centroids, _) = kmeans_plus_plus(&cur_pixels, k, 50);
    let (ref_centroids, _) = kmeans_plus_plus(&ref_pixels, k, 50);

    let mu = DVector::from_element(k, 1.0 / k as f64);
    let nu = DVector::from_element(k, 1.0 / k as f64);
    let mut cost_matrix = DMatrix::zeros(k, k);
    for i in 0..k {
        for j in 0..k {
            cost_matrix[(i, j)] = (cur_centroids[i] - ref_centroids[j]).norm_squared();
        }
    }
    let p_mat = sinkhorn_ot(&mu, &nu, &cost_matrix, 0.1, 1000, 1e-5);

    let mut src_points = Vec::with_capacity(k);
    let mut dst_points = Vec::with_capacity(k);
    for i in 0..k {
        let mut target = Vector3::zeros();
        let mut weight_sum = 0.0;
        for j in 0..k {
            target += ref_centroids[j] * p_mat[(i, j)];
            weight_sum += p_mat[(i, j)];
        }
        if weight_sum > 0.0 {
            target /= weight_sum;
        }
        src_points.push(cur_centroids[i]);
        dst_points.push(target);
    }

    let tps = TPS::fit(&src_points, &dst_points)?;

    let mut lut_data = Vec::with_capacity(lut_size * lut_size * lut_size * 3);
    for b in 0..lut_size {
        for g in 0..lut_size {
            for r in 0..lut_size {
                let rgb = Vector3::new(
                    r as f64 / (lut_size - 1) as f64,
                    g as f64 / (lut_size - 1) as f64,
                    b as f64 / (lut_size - 1) as f64,
                );
                let lab = rgb_to_lab(&rgb);
                let mapped_lab = tps.transform(&lab);
                let mapped_rgb = lab_to_rgb(&mapped_lab);

                lut_data.push(mapped_rgb[0].clamp(0.0, 1.0) as f32);
                lut_data.push(mapped_rgb[1].clamp(0.0, 1.0) as f32);
                lut_data.push(mapped_rgb[2].clamp(0.0, 1.0) as f32);
            }
        }
    }
    Some(lut_data)
}
