use image::{DynamicImage, GenericImageView, Pixel};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

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

#[derive(Serialize, Deserialize, Debug)]
pub struct StyleTransferSuggestion {
    pub key: String,
    pub value: f64,
    pub label: String,
    pub min: f64,
    pub max: f64,
    pub reason: String,
}

#[derive(Serialize, Deserialize, Debug)]
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

    // 降采样加速：如果图像太大，缩小到 max 800px 边
    let analysis_img = if w > 800 || h > 800 {
        img.resize(800, 800, image::imageops::FilterType::Triangle)
    } else {
        img.clone()
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

/// 将两组特征的差异映射为滑块调整参数
fn map_features_to_adjustments(
    ref_feat: &StyleFeatures,
    cur_feat: &StyleFeatures,
    current_adjustments: &Value,
) -> Vec<StyleTransferSuggestion> {
    let mut suggestions = Vec::new();

    // 辅助函数：获取当前调整值
    let get_current = |key: &str, default: f64| -> f64 {
        current_adjustments.get(key).and_then(|v| v.as_f64()).unwrap_or(default)
    };

    // 辅助函数：限制范围
    let clamp = |v: f64, min: f64, max: f64| -> f64 { v.max(min).min(max) };

    // 1. 曝光 (exposure): 基于平均亮度差
    let lum_diff = ref_feat.mean_luminance - cur_feat.mean_luminance;
    if lum_diff.abs() > 0.02 {
        // 亮度差 0~1 映射到 exposure 调整量 0~3
        let exposure_delta = lum_diff * 3.0;
        let cur_exposure = get_current("exposure", 0.0);
        let new_exposure = clamp(cur_exposure + exposure_delta, -5.0, 5.0);
        suggestions.push(StyleTransferSuggestion {
            key: "exposure".to_string(),
            value: (new_exposure * 100.0).round() / 100.0,
            label: "曝光".to_string(),
            min: -5.0, max: 5.0,
            reason: format!("参考图亮度{}", if lum_diff > 0.0 { "更高" } else { "更低" }),
        });
    }

    // 2. 对比度 (contrast): 基于亮度标准差差异
    let contrast_diff = ref_feat.contrast_spread - cur_feat.contrast_spread;
    if contrast_diff.abs() > 0.01 {
        let contrast_delta = contrast_diff * 300.0; // 标准差差 0~0.3 → 调整量 0~90
        let cur_contrast = get_current("contrast", 0.0);
        let new_contrast = clamp(cur_contrast + contrast_delta, -100.0, 100.0);
        suggestions.push(StyleTransferSuggestion {
            key: "contrast".to_string(),
            value: new_contrast.round(),
            label: "对比度".to_string(),
            min: -100.0, max: 100.0,
            reason: format!("参考图对比度{}", if contrast_diff > 0.0 { "更强" } else { "更弱" }),
        });
    }

    // 3. 高光 (highlights): 基于高光像素比例差
    let hl_diff = ref_feat.highlight_ratio - cur_feat.highlight_ratio;
    if hl_diff.abs() > 0.02 {
        let hl_delta = hl_diff * 250.0;
        let cur_hl = get_current("highlights", 0.0);
        let new_hl = clamp(cur_hl + hl_delta, -100.0, 100.0);
        suggestions.push(StyleTransferSuggestion {
            key: "highlights".to_string(),
            value: new_hl.round(),
            label: "高光".to_string(),
            min: -100.0, max: 100.0,
            reason: format!("参考图高光区域{}", if hl_diff > 0.0 { "更多" } else { "更少" }),
        });
    }

    // 4. 阴影 (shadows): 基于阴影像素比例差
    let sh_diff = ref_feat.shadow_ratio - cur_feat.shadow_ratio;
    if sh_diff.abs() > 0.02 {
        // 阴影比例高 → shadows 应该更负（更暗）
        let sh_delta = -sh_diff * 250.0;
        let cur_sh = get_current("shadows", 0.0);
        let new_sh = clamp(cur_sh + sh_delta, -100.0, 100.0);
        suggestions.push(StyleTransferSuggestion {
            key: "shadows".to_string(),
            value: new_sh.round(),
            label: "阴影".to_string(),
            min: -100.0, max: 100.0,
            reason: format!("参考图暗部{}", if sh_diff > 0.0 { "更深" } else { "更浅" }),
        });
    }

    // 5. 色温 (temperature): 基于 R/B 通道比值差
    let temp_diff = ref_feat.rb_ratio - cur_feat.rb_ratio;
    if temp_diff.abs() > 0.02 {
        let temp_delta = temp_diff * 80.0; // R/B 比值差 0~1 → 调整量 0~80
        let cur_temp = get_current("temperature", 0.0);
        let new_temp = clamp(cur_temp + temp_delta, -100.0, 100.0);
        suggestions.push(StyleTransferSuggestion {
            key: "temperature".to_string(),
            value: new_temp.round(),
            label: "色温".to_string(),
            min: -100.0, max: 100.0,
            reason: format!("参考图色调{}", if temp_diff > 0.0 { "偏暖" } else { "偏冷" }),
        });
    }

    // 6. 色调偏移 (tint): 基于 G/B 通道比值差
    let tint_diff = ref_feat.gb_ratio - cur_feat.gb_ratio;
    if tint_diff.abs() > 0.02 {
        let tint_delta = -tint_diff * 60.0; // G 偏高 → tint 偏绿（负值）
        let cur_tint = get_current("tint", 0.0);
        let new_tint = clamp(cur_tint + tint_delta, -100.0, 100.0);
        suggestions.push(StyleTransferSuggestion {
            key: "tint".to_string(),
            value: new_tint.round(),
            label: "色调偏移".to_string(),
            min: -100.0, max: 100.0,
            reason: format!("参考图色调{}", if tint_diff > 0.0 { "偏绿" } else { "偏品红" }),
        });
    }

    // 7. 饱和度 (saturation): 基于 HSL S 通道均值差
    let sat_diff = ref_feat.mean_saturation - cur_feat.mean_saturation;
    if sat_diff.abs() > 0.02 {
        let sat_delta = sat_diff * 200.0;
        let cur_sat = get_current("saturation", 0.0);
        let new_sat = clamp(cur_sat + sat_delta, -100.0, 100.0);
        suggestions.push(StyleTransferSuggestion {
            key: "saturation".to_string(),
            value: new_sat.round(),
            label: "饱和度".to_string(),
            min: -100.0, max: 100.0,
            reason: format!("参考图色彩{}", if sat_diff > 0.0 { "更鲜艳" } else { "更淡雅" }),
        });
    }

    // 8. 自然饱和度 (vibrance): 基于饱和度分布差
    let vib_diff = ref_feat.saturation_spread - cur_feat.saturation_spread;
    if vib_diff.abs() > 0.01 {
        let vib_delta = vib_diff * 250.0;
        let cur_vib = get_current("vibrance", 0.0);
        let new_vib = clamp(cur_vib + vib_delta, -100.0, 100.0);
        suggestions.push(StyleTransferSuggestion {
            key: "vibrance".to_string(),
            value: new_vib.round(),
            label: "自然饱和度".to_string(),
            min: -100.0, max: 100.0,
            reason: format!("参考图色彩层次{}", if vib_diff > 0.0 { "更丰富" } else { "更统一" }),
        });
    }

    // 9. 清晰度/结构 (clarity/structure): 基于拉普拉斯方差差
    let lap_diff = ref_feat.laplacian_variance - cur_feat.laplacian_variance;
    if lap_diff.abs() > 50.0 {
        // 拉普拉斯方差范围很大，归一化
        let clarity_delta = (lap_diff / 100.0).max(-60.0).min(60.0);
        let cur_clarity = get_current("clarity", 0.0);
        let new_clarity = clamp(cur_clarity + clarity_delta, -100.0, 100.0);
        suggestions.push(StyleTransferSuggestion {
            key: "clarity".to_string(),
            value: new_clarity.round(),
            label: "清晰度".to_string(),
            min: -100.0, max: 100.0,
            reason: format!("参考图纹理{}", if lap_diff > 0.0 { "更锐利" } else { "更柔和" }),
        });
    }

    // 10. 暗角 (vignetteAmount): 基于中心-边缘亮度差
    let vig_diff = ref_feat.vignette_diff - cur_feat.vignette_diff;
    if vig_diff.abs() > 0.02 {
        let vig_delta = -vig_diff * 150.0; // 正差值=参考图暗角更重 → vignetteAmount 更负
        let cur_vig = get_current("vignetteAmount", 0.0);
        let new_vig = clamp(cur_vig + vig_delta, -100.0, 100.0);
        suggestions.push(StyleTransferSuggestion {
            key: "vignetteAmount".to_string(),
            value: new_vig.round(),
            label: "暗角".to_string(),
            min: -100.0, max: 100.0,
            reason: format!("参考图暗角{}", if vig_diff > 0.0 { "更明显" } else { "更轻微" }),
        });
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
        let r = image::open(&ref_path).map_err(|e| format!("无法打开参考图: {}", e))?;
        let c = image::open(&cur_path).map_err(|e| format!("无法打开当前图片: {}", e))?;
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
