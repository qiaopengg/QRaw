// ============================================================================
// PHASE 1: 语义区域识别与局部映射
// ============================================================================
// 本模块实现 V4 文档 Phase 1 要求的真正语义区域识别能力
// 不再依赖全局特征猜测，而是使用真实的语义分割模型
//
// 文档参考：docs/rapid_raw_分析式风格迁移技术架构_v_4.md
// Phase 1 目标：
// - 建立真正的语义区域识别（skin/sky 优先）
// - 基于语义 mask 生成局部参数建议
// - 保持可编辑性和可解释性

use image::{DynamicImage, GrayImage};
use std::sync::Arc;

use crate::ai_processing::AiModels;

/// 语义区域类型
#[derive(Debug, Clone, PartialEq)]
pub enum SemanticRegionType {
    Skin,       // 肤色区域（人像）
    Sky,        // 天空区域
    Subject,    // 主体前景
    Background, // 背景
}

impl SemanticRegionType {
    #[allow(dead_code)]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Skin => "skin",
            Self::Sky => "sky",
            Self::Subject => "subject",
            Self::Background => "background",
        }
    }
}

/// 语义区域分析结果
#[derive(Debug, Clone)]
pub struct SemanticRegionAnalysis {
    pub region_type: SemanticRegionType,
    pub mask: Option<GrayImage>,
    pub coverage: f64,        // 区域覆盖率 [0, 1]
    pub confidence: f64,      // 检测置信度 [0, 1]
    pub mean_luminance: f64,  // 区域平均亮度
    pub mean_saturation: f64, // 区域平均饱和度
}

/// 使用真实语义分割模型分析图像区域
/// 
/// Phase 1 实现：优先支持 skin 和 sky 两类区域
/// Phase 2 扩展：vegetation、subject、background
pub fn analyze_semantic_regions(
    img: &DynamicImage,
    ai_models: Option<&Arc<AiModels>>,
) -> Vec<SemanticRegionAnalysis> {
    let mut regions = Vec::new();

    let Some(models) = ai_models else {
        eprintln!("[Semantic] AI models not available, skipping semantic region analysis");
        return regions;
    };

    eprintln!("[Semantic] Starting semantic region analysis");

    // 1. Sky 区域识别（使用 SkySeg 模型）
    match crate::ai_processing::run_sky_seg_model(img, &models.sky_seg) {
        Ok(sky_mask) => {
            let coverage = calculate_mask_coverage(&sky_mask);
            eprintln!("[Semantic] Sky detected: coverage={:.2}%", coverage * 100.0);
            
            if coverage > 0.02 {
                // 至少 2% 覆盖率才认为有效
                let (mean_lum, mean_sat) = calculate_region_stats(img, &sky_mask);
                regions.push(SemanticRegionAnalysis {
                    region_type: SemanticRegionType::Sky,
                    mask: Some(sky_mask),
                    coverage,
                    confidence: 0.85,
                    mean_luminance: mean_lum,
                    mean_saturation: mean_sat,
                });
                eprintln!("[Semantic] Sky region added: lum={:.3}, sat={:.3}", mean_lum, mean_sat);
            }
        }
        Err(e) => {
            eprintln!("[Semantic] Sky detection failed: {}", e);
        }
    }

    // 2. Subject 前景识别（使用 U2NET 模型）
    match crate::ai_processing::run_u2netp_model(img, &models.u2netp) {
        Ok(subject_mask) => {
            let coverage = calculate_mask_coverage(&subject_mask);
            eprintln!("[Semantic] Subject detected: coverage={:.2}%", coverage * 100.0);
            
            if coverage > 0.05 {
                // 至少 5% 覆盖率
                let (mean_lum, mean_sat) = calculate_region_stats(img, &subject_mask);
                
                // 判断是否包含肤色（简单启发式：检查主体区域的肤色占比）
                let has_skin = detect_skin_in_region(img, &subject_mask);
                eprintln!("[Semantic] Subject has_skin={}", has_skin);
                
                if has_skin {
                    // 如果主体包含肤色，标记为 Skin 区域
                    regions.push(SemanticRegionAnalysis {
                        region_type: SemanticRegionType::Skin,
                        mask: Some(subject_mask.clone()),
                        coverage,
                        confidence: 0.80,
                        mean_luminance: mean_lum,
                        mean_saturation: mean_sat,
                    });
                    eprintln!("[Semantic] Skin region added: lum={:.3}, sat={:.3}", mean_lum, mean_sat);
                }
                
                // 同时保留 Subject 标记
                regions.push(SemanticRegionAnalysis {
                    region_type: SemanticRegionType::Subject,
                    mask: Some(subject_mask),
                    coverage,
                    confidence: 0.85,
                    mean_luminance: mean_lum,
                    mean_saturation: mean_sat,
                });
                eprintln!("[Semantic] Subject region added");
            }
        }
        Err(e) => {
            eprintln!("[Semantic] Subject detection failed: {}", e);
        }
    }

    // 3. Background 区域（通过反向推导）
    // 如果有 subject mask，背景 = 全图 - subject
    if let Some(subject_region) = regions
        .iter()
        .find(|r| r.region_type == SemanticRegionType::Subject)
    {
        if let Some(subject_mask) = &subject_region.mask {
            let bg_mask = invert_mask(subject_mask);
            let coverage = calculate_mask_coverage(&bg_mask);
            eprintln!("[Semantic] Background coverage={:.2}%", coverage * 100.0);
            
            if coverage > 0.1 {
                let (mean_lum, mean_sat) = calculate_region_stats(img, &bg_mask);
                regions.push(SemanticRegionAnalysis {
                    region_type: SemanticRegionType::Background,
                    mask: Some(bg_mask),
                    coverage,
                    confidence: 0.75,
                    mean_luminance: mean_lum,
                    mean_saturation: mean_sat,
                });
                eprintln!("[Semantic] Background region added: lum={:.3}, sat={:.3}", mean_lum, mean_sat);
            }
        }
    }

    eprintln!("[Semantic] Total regions detected: {}", regions.len());
    regions
}

/// 计算 mask 覆盖率
fn calculate_mask_coverage(mask: &GrayImage) -> f64 {
    let (w, h) = mask.dimensions();
    let total_pixels = (w * h) as f64;
    let mut white_pixels = 0u64;
    
    for pixel in mask.pixels() {
        if pixel[0] > 128 {
            white_pixels += 1;
        }
    }
    
    white_pixels as f64 / total_pixels
}

/// 计算区域内的平均亮度和饱和度
fn calculate_region_stats(img: &DynamicImage, mask: &GrayImage) -> (f64, f64) {
    let rgb = img.to_rgb8();
    let (w, h) = rgb.dimensions();
    
    let mut sum_lum = 0.0;
    let mut sum_sat = 0.0;
    let mut count = 0u64;
    
    for y in 0..h {
        for x in 0..w {
            if x < mask.width() && y < mask.height() {
                let mask_val = mask.get_pixel(x, y)[0];
                if mask_val > 128 {
                    let pixel = rgb.get_pixel(x, y);
                    let r = pixel[0] as f64 / 255.0;
                    let g = pixel[1] as f64 / 255.0;
                    let b = pixel[2] as f64 / 255.0;
                    
                    // 计算亮度（简化版）
                    let lum = 0.299 * r + 0.587 * g + 0.114 * b;
                    
                    // 计算饱和度（简化版）
                    let max_c = r.max(g).max(b);
                    let min_c = r.min(g).min(b);
                    let sat = if max_c > 0.0 { (max_c - min_c) / max_c } else { 0.0 };
                    
                    sum_lum += lum;
                    sum_sat += sat;
                    count += 1;
                }
            }
        }
    }
    
    if count > 0 {
        (sum_lum / count as f64, sum_sat / count as f64)
    } else {
        (0.5, 0.3)
    }
}

/// 检测区域内是否包含肤色
fn detect_skin_in_region(img: &DynamicImage, mask: &GrayImage) -> bool {
    let rgb = img.to_rgb8();
    let (w, h) = rgb.dimensions();
    
    let mut skin_pixels = 0u64;
    let mut total_pixels = 0u64;
    
    for y in 0..h.min(mask.height()) {
        for x in 0..w.min(mask.width()) {
            let mask_val = mask.get_pixel(x, y)[0];
            if mask_val > 128 {
                total_pixels += 1;
                let pixel = rgb.get_pixel(x, y);
                let r = pixel[0] as f64;
                let g = pixel[1] as f64;
                let b = pixel[2] as f64;
                
                // 简单肤色检测（YCbCr 色彩空间启发式）
                let is_skin = r > 95.0 && g > 40.0 && b > 20.0
                    && r > g && r > b
                    && (r - g).abs() > 15.0
                    && r < 250.0; // 避免过曝区域
                
                if is_skin {
                    skin_pixels += 1;
                }
            }
        }
    }
    
    if total_pixels > 0 {
        let skin_ratio = skin_pixels as f64 / total_pixels as f64;
        skin_ratio > 0.15 // 至少 15% 肤色像素
    } else {
        false
    }
}

/// 反转 mask
fn invert_mask(mask: &GrayImage) -> GrayImage {
    let (w, h) = mask.dimensions();
    let mut inverted = GrayImage::new(w, h);
    
    for y in 0..h {
        for x in 0..w {
            let val = mask.get_pixel(x, y)[0];
            inverted.put_pixel(x, y, image::Luma([255 - val]));
        }
    }
    
    inverted
}
