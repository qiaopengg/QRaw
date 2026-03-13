use image::{DynamicImage, GenericImageView, Pixel};
use rawler::decoders::RawDecodeParams;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;
use tauri::Emitter;
use crate::llm_chat::StreamChunkPayload;

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
    pub rb_ratio: f64,
    pub gb_ratio: f64,
    pub mean_saturation: f64,
    pub saturation_spread: f64,
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
}

/// 从图像中提取风格特征向量
fn extract_features(img: &DynamicImage) -> StyleFeatures {
    let (w, h) = img.dimensions();
    let total_pixels = (w as f64) * (h as f64);
    if total_pixels == 0.0 {
        return StyleFeatures {
            mean_luminance: 0.0, highlight_ratio: 0.0, shadow_ratio: 0.0,
            contrast_spread: 0.0, rb_ratio: 1.0, gb_ratio: 1.0,
            mean_saturation: 0.0, saturation_spread: 0.0,
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
    let mut lum_values: Vec<f64> = Vec::with_capacity(a_total as usize);
    let mut sat_values: Vec<f64> = Vec::with_capacity(a_total as usize);

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

    StyleFeatures {
        mean_luminance: mean_lum,
        highlight_ratio: highlight_count / a_total,
        shadow_ratio: shadow_count / a_total,
        contrast_spread,
        rb_ratio,
        gb_ratio,
        mean_saturation: mean_sat,
        saturation_spread,
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

/// 将两组特征的差异映射为滑块调整参数（优化版：更精准的感知映射）
fn map_features_to_adjustments(
    ref_feat: &StyleFeatures,
    cur_feat: &StyleFeatures,
    current_adjustments: &Value,
) -> Vec<StyleTransferSuggestion> {
    let mut suggestions = Vec::new();

    let get_current = |key: &str, default: f64| -> f64 {
        current_adjustments.get(key).and_then(|v| v.as_f64()).unwrap_or(default)
    };

    let clamp = |v: f64, min: f64, max: f64| -> f64 { v.max(min).min(max) };

    // ===== 保守系数设计原则 =====
    // RAW 预览 vs JPEG 参考图天然存在巨大差异（RAW 未处理，JPEG 已调色）
    // 因此所有系数必须非常保守，宁可调不够也不能调过头
    // 每个参数都有两级钳位：delta 钳位 + 最终值钳位

    // 1. 曝光 (exposure): 线性映射，保守系数
    let lum_diff = ref_feat.mean_luminance - cur_feat.mean_luminance;
    if lum_diff.abs() > 0.05 {
        let exposure_delta = clamp(lum_diff * 1.0, -0.8, 0.8);
        let cur_exposure = get_current("exposure", 0.0);
        let new_exposure = clamp(cur_exposure + exposure_delta, -2.0, 2.0);
        suggestions.push(StyleTransferSuggestion {
            key: "exposure".to_string(),
            value: (new_exposure * 100.0).round() / 100.0,
            label: "曝光".to_string(),
            min: -5.0, max: 5.0,
            reason: format!("参考图亮度{}", if lum_diff > 0.0 { "更高" } else { "更低" }),
        });
    }

    // 2. 对比度 (contrast): 保守映射
    let contrast_diff = ref_feat.contrast_spread - cur_feat.contrast_spread;
    if contrast_diff.abs() > 0.02 {
        let contrast_delta = clamp(contrast_diff * 100.0, -30.0, 30.0);
        let cur_contrast = get_current("contrast", 0.0);
        let new_contrast = clamp(cur_contrast + contrast_delta, -50.0, 50.0);
        suggestions.push(StyleTransferSuggestion {
            key: "contrast".to_string(),
            value: new_contrast.round(),
            label: "对比度".to_string(),
            min: -100.0, max: 100.0,
            reason: format!("参考图对比度{}", if contrast_diff > 0.0 { "更强" } else { "更弱" }),
        });
    }

    // 3. 高光 (highlights): 保守映射
    let hl_diff = ref_feat.highlight_ratio - cur_feat.highlight_ratio;
    if hl_diff.abs() > 0.05 {
        let hl_delta = clamp(hl_diff * 80.0, -30.0, 30.0);
        let cur_hl = get_current("highlights", 0.0);
        let new_hl = clamp(cur_hl + hl_delta, -50.0, 50.0);
        suggestions.push(StyleTransferSuggestion {
            key: "highlights".to_string(),
            value: new_hl.round(),
            label: "高光".to_string(),
            min: -100.0, max: 100.0,
            reason: format!("参考图高光区域{}", if hl_diff > 0.0 { "更多" } else { "更少" }),
        });
    }

    // 4. 阴影 (shadows): 保守映射
    let sh_diff = ref_feat.shadow_ratio - cur_feat.shadow_ratio;
    if sh_diff.abs() > 0.05 {
        let sh_delta = clamp(-sh_diff * 80.0, -30.0, 30.0);
        let cur_sh = get_current("shadows", 0.0);
        let new_sh = clamp(cur_sh + sh_delta, -50.0, 50.0);
        suggestions.push(StyleTransferSuggestion {
            key: "shadows".to_string(),
            value: new_sh.round(),
            label: "阴影".to_string(),
            min: -100.0, max: 100.0,
            reason: format!("参考图暗部{}", if sh_diff > 0.0 { "更深" } else { "更浅" }),
        });
    }

    // 5. 色温 (temperature): 保守映射
    let temp_diff = ref_feat.rb_ratio - cur_feat.rb_ratio;
    if temp_diff.abs() > 0.03 {
        let temp_delta = clamp(temp_diff * 30.0, -30.0, 30.0);
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
        let sat_delta = clamp(sat_diff * 80.0, -30.0, 30.0);
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
        let vib_delta = clamp(vib_diff * 100.0, -25.0, 25.0);
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

    // ===== 总体亮度安全检查 =====
    // 防止 exposure + highlights + shadows 同方向叠加导致白图/黑图
    // 基于 delta（变化量）计算综合亮度影响，而非最终绝对值
    let mut brightness_impact: f64 = 0.0;
    for s in &suggestions {
        let cur = get_current(&s.key, 0.0);
        let delta = s.value - cur;
        match s.key.as_str() {
            "exposure" => brightness_impact += delta * 40.0, // exposure 1.0 EV ≈ 40 亮度单位
            "highlights" => brightness_impact += delta * 0.3,
            "shadows" => brightness_impact += delta * 0.3,
            "contrast" => brightness_impact += delta * 0.1,
            _ => {}
        }
    }
    // 如果综合亮度变化影响超过 60，按比例缩减所有亮度相关参数的 delta
    if brightness_impact.abs() > 60.0 {
        let scale = 60.0 / brightness_impact.abs();
        for s in &mut suggestions {
            match s.key.as_str() {
                "exposure" | "highlights" | "shadows" | "contrast" => {
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

    suggestions
}

#[tauri::command]
pub async fn analyze_style_transfer(
    reference_path: String,
    current_image_path: String,
    current_adjustments: Value,
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

    // 映射为调整参数
    let adjustments = map_features_to_adjustments(&ref_features, &cur_features, &current_adjustments);

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
    })
}

/// 将特征向量格式化为 LLM 可读的文本描述
fn describe_features(feat: &StyleFeatures, label: &str) -> String {
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
        "【{}】亮度={:.2}（{}），高光占比={:.1}%，阴影占比={:.1}%，对比度={:.3}（{}），\
         R/B比={:.3}（{}），饱和度={:.3}（{}），饱和度分布={:.3}，\
         纹理方差={:.1}，暗角差={:.3}（{}）",
        label,
        feat.mean_luminance, brightness_desc,
        feat.highlight_ratio * 100.0,
        feat.shadow_ratio * 100.0,
        feat.contrast_spread, contrast_desc,
        feat.rb_ratio, temp_desc,
        feat.mean_saturation, sat_desc,
        feat.saturation_spread,
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
) -> String {
    let ref_desc = describe_features(ref_feat, "参考图");
    let cur_desc = describe_features(cur_feat, "当前图");
    let adj_str = serde_json::to_string_pretty(current_adjustments).unwrap_or_default();

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

## 你的任务
1. 理解参考图的整体风格（如：复古胶片、清新日系、电影感、高对比黑白等）
2. 基于统计数据和算法建议，给出优化后的调整参数
3. 你可以修正算法建议的值（算法是纯数学映射，可能不够精准），也可以补充算法未覆盖的参数
4. value 是最终绝对值，不是增量
5. 重要：参数值必须合理，避免极端值。大多数参数应在 ±50 以内，除非参考图风格确实极端
6. 优先调整色温、饱和度、对比度等对风格影响最大的参数
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
    ref_feat: &StyleFeatures,
    cur_feat: &StyleFeatures,
    algo_suggestions: &[StyleTransferSuggestion],
    current_adjustments: &Value,
    llm_endpoint: &str,
    llm_api_key: Option<&str>,
    llm_model: &str,
    app_handle: &tauri::AppHandle,
) -> Result<StyleTransferResponse, String> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(180))
        .build()
        .map_err(|e| e.to_string())?;

    let system_prompt = build_style_transfer_prompt(ref_feat, cur_feat, algo_suggestions, current_adjustments);

    let messages = vec![
        json!({ "role": "system", "content": system_prompt }),
        json!({ "role": "user", "content": "请分析参考图的调色风格，给出优化后的调整参数。" }),
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
    let mut parsed: StyleTransferResponse = serde_json::from_str(&json_str)
        .map_err(|e| format!("解析风格迁移 JSON 失败: {}，原始: {}", e, &full_content[..full_content.len().min(500)]))?;

    // 安全钳位：防止 LLM 返回极端值
    for adj in &mut parsed.adjustments {
        match adj.key.as_str() {
            "exposure" => adj.value = adj.value.max(-2.5).min(2.5),
            _ => adj.value = adj.value.max(-80.0).min(80.0),
        }
    }

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
    let algo_suggestions = map_features_to_adjustments(&ref_features, &cur_features, &current_adjustments);

    // 推送：算法分析完成
    let _ = app_handle.emit("style-transfer-stream", StreamChunkPayload {
        chunk_type: "thinking".to_string(),
        text: format!("算法分析完成，初步建议 {} 项调整。正在请求 AI 优化...\n", algo_suggestions.len()),
        result: None,
    });

    // 尝试 LLM 增强
    let model = llm_model.unwrap_or_else(|| "qwen3.5:9b".to_string());
    match enhance_with_llm(
        &ref_features,
        &cur_features,
        &algo_suggestions,
        &current_adjustments,
        &llm_endpoint,
        llm_api_key.as_deref(),
        &model,
        &app_handle,
    )
    .await
    {
        Ok(llm_result) => {
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
                }),
            });
            Ok(result)
        }
    }
}
