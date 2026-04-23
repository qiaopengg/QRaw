use image::{GrayImage, imageops};
use rayon::prelude::*;
use tauri::{AppHandle, Emitter};

use crate::image_processing::perform_auto_analysis;

use super::types::*;

/// Calculate Laplacian variance (reused from existing culling code)
fn calculate_laplacian_variance(image: &GrayImage) -> f64 {
    let (width, height) = image.dimensions();
    if width < 3 || height < 3 {
        return 0.0;
    }
    let mut values = Vec::with_capacity(((width - 2) * (height - 2)) as usize);
    let mut sum = 0.0;
    for y in 1..height - 1 {
        for x in 1..width - 1 {
            let c = image.get_pixel(x, y)[0] as i32;
            let n = image.get_pixel(x, y - 1)[0] as i32;
            let s = image.get_pixel(x, y + 1)[0] as i32;
            let w = image.get_pixel(x - 1, y)[0] as i32;
            let e = image.get_pixel(x + 1, y)[0] as i32;
            let v = (n + s + w + e - 4 * c) as f64;
            values.push(v);
            sum += v;
        }
    }
    if values.is_empty() {
        return 0.0;
    }
    let mean = sum / values.len() as f64;
    values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64
}

/// Calculate Laplacian variance only on masked (white) regions
pub fn calculate_masked_laplacian_variance(gray: &GrayImage, mask: &GrayImage) -> f64 {
    let (w, h) = gray.dimensions();
    if w < 3 || h < 3 {
        return 0.0;
    }
    let mut values = Vec::new();
    let mut sum = 0.0;
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            if mask.get_pixel(x, y)[0] < 128 {
                continue;
            }
            let c = gray.get_pixel(x, y)[0] as i32;
            let n = gray.get_pixel(x, y - 1)[0] as i32;
            let s = gray.get_pixel(x, y + 1)[0] as i32;
            let e = gray.get_pixel(x + 1, y)[0] as i32;
            let w_px = gray.get_pixel(x - 1, y)[0] as i32;
            let v = (n + s + e + w_px - 4 * c) as f64;
            values.push(v);
            sum += v;
        }
    }
    if values.is_empty() {
        return 0.0;
    }
    let mean = sum / values.len() as f64;
    values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64
}

/// Extract foreground mask from depth map (nearest `ratio` fraction of pixels)
pub fn extract_foreground_mask(depth: &GrayImage, ratio: f32) -> GrayImage {
    let mut values: Vec<u8> = depth.pixels().map(|p| p[0]).collect();
    values.sort_unstable();
    let threshold_idx = ((1.0 - ratio) * values.len() as f32) as usize;
    let threshold = values.get(threshold_idx).copied().unwrap_or(128);
    let (w, h) = depth.dimensions();
    let mut mask = GrayImage::new(w, h);
    for (x, y, p) in depth.enumerate_pixels() {
        mask.put_pixel(x, y, image::Luma([if p[0] >= threshold { 255 } else { 0 }]));
    }
    mask
}

/// Compute exposure health from auto analysis results
/// perform_auto_analysis returns "how much correction is needed":
///   exposure: how many stops to adjust (negative = photo is bright, needs darkening)
///   shadows: how much shadow recovery needed (0-100, 0 = no recovery needed)
///   highlights: how much highlight recovery needed (-100 to 0, 0 = no recovery needed)
///
/// Key insight: highlights = -80 means "high contrast photo with bright highlights"
/// This is NORMAL for portraits with studio lighting or outdoor sun.
/// Only penalize when BOTH exposure AND highlights are extreme.
fn compute_exposure_health(auto: &crate::image_processing::AutoAdjustmentResults) -> f64 {
    // Exposure: how far from "perfect" (0 = perfect)
    // Use gentler decay: exp=1.0 → 0.61, exp=2.0 → 0.37
    let exp_score = (-0.5 * auto.exposure.abs()).exp();

    // Shadows: 0 = no recovery needed = perfect
    let shadow_score = 1.0 - (auto.shadows / 100.0).clamp(0.0, 1.0);

    // Highlights: 0 = no recovery needed = perfect
    // BUT: many normal photos have highlights = -60 to -80 (bright areas)
    // Only penalize severely when highlights are extreme AND exposure is also off
    let highlight_raw = 1.0 - (auto.highlights.abs() / 100.0).clamp(0.0, 1.0);
    // Soften highlight penalty: only count 40% of it
    let highlight_score = 1.0 - (1.0 - highlight_raw) * 0.4;

    // Weighted combination: exposure matters most, highlights least
    let score = exp_score * 0.5 + shadow_score * 0.2 + highlight_score * 0.3;
    score.clamp(0.0, 1.0)
}

/// Compute dynamic range from histogram
fn compute_dynamic_range(gray: &GrayImage) -> f64 {
    let mut hist = [0u32; 256];
    for p in gray.pixels() {
        hist[p[0] as usize] += 1;
    }
    let total = gray.pixels().count() as f64;
    let threshold = total * 0.001;

    let mut low = 0usize;
    let mut high = 255usize;
    let mut cumsum = 0u32;
    for (i, &count) in hist.iter().enumerate() {
        cumsum += count;
        if cumsum as f64 > threshold {
            low = i;
            break;
        }
    }
    cumsum = 0;
    for i in (0..256).rev() {
        cumsum += hist[i];
        if cumsum as f64 > threshold {
            high = i;
            break;
        }
    }
    (high.saturating_sub(low)) as f64 / 255.0
}

/// Stage 1: Technical elimination
/// Uses Depth Anything for subject-aware sharpness if available
pub fn stage_1_technical(
    registry: &AssetRegistry,
    depth_map_fn: Option<&(dyn Fn(&image::DynamicImage) -> Option<GrayImage> + Sync)>,
    settings: &CullingSettingsV4,
    app_handle: &AppHandle,
) -> Vec<TechnicalVerdict> {
    let total = registry.assets.len();
    let completed = std::sync::atomic::AtomicUsize::new(0);

    registry.assets
        .par_iter()
        .map(|asset| {
            let done = completed.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            let _ = app_handle.emit(
                "culling-progress",
                CullingProgressV4 { current: done, total, stage: "Analyzing technical quality...".into() },
            );

            let gray = &*asset.gray_thumbnail;
            let bt = settings.blur_threshold;

            // Global sharpness
            let sharpness = calculate_laplacian_variance(gray);

            // Subject sharpness (Depth Anything if available)
            let subject_sharpness = if let Some(depth_fn) = depth_map_fn {
                if let Some(depth_map) = depth_fn(&*asset.thumbnail) {
                    let (tw, th) = gray.dimensions();
                    let resized_depth = imageops::resize(&depth_map, tw, th, imageops::FilterType::Triangle);
                    let mask = extract_foreground_mask(&resized_depth, 0.30);
                    let masked = calculate_masked_laplacian_variance(gray, &mask);
                    if masked > 0.0 { masked } else { sharpness }
                } else {
                    sharpness
                }
            } else {
                sharpness
            };

            // Exposure health
            let auto = perform_auto_analysis(&*asset.thumbnail);
            let exposure_health = compute_exposure_health(&auto);
            let dynamic_range = compute_dynamic_range(gray);

            // ── Decision logic (veto-based) ──

            // Veto: severe blur
            if subject_sharpness < bt * 0.25 {
                return TechnicalVerdict::Fail { reason: TechnicalIssue::SevereBlur };
            }

            // Veto: severe overexposure
            if exposure_health < 0.15 && auto.highlights.abs() > 70.0 {
                return TechnicalVerdict::Fail { reason: TechnicalIssue::SevereOverexposure };
            }

            // Veto: severe underexposure
            if exposure_health < 0.15 && auto.shadows > 70.0 {
                return TechnicalVerdict::Fail { reason: TechnicalIssue::SevereUnderexposure };
            }

            // Marginal: mild blur
            if subject_sharpness < bt * 0.5 {
                return TechnicalVerdict::Marginal {
                    reason: TechnicalIssue::MildBlur,
                    sharpness: subject_sharpness,
                    exposure_health,
                };
            }

            // Marginal: motion blur (EXIF-based)
            if let (Some(shutter), Some(focal)) = (asset.exposure_time, asset.focal_length) {
                if shutter > 1.5 / focal && subject_sharpness < bt * 0.6 {
                    return TechnicalVerdict::Marginal {
                        reason: TechnicalIssue::MotionBlur,
                        sharpness: subject_sharpness,
                        exposure_health,
                    };
                }
            }

            // Debug logging - emit to frontend
            let auto_debug = format!(
                "[Stage1] {} → sharpness={:.1} subject={:.1} exp_health={:.3} dr={:.3} bt={:.1} auto.exp={:.3} auto.shadows={:.1} auto.highlights={:.1} verdict=Pass",
                asset.path.split('/').last().unwrap_or(&asset.path),
                sharpness, subject_sharpness, exposure_health, dynamic_range, bt,
                auto.exposure, auto.shadows, auto.highlights
            );
            let _ = app_handle.emit("culling-debug", &auto_debug);

            TechnicalVerdict::Pass {
                sharpness,
                subject_sharpness,
                exposure_health,
                dynamic_range,
                nima_technical: None, // Filled later if model available
            }
        })
        .collect()
}
