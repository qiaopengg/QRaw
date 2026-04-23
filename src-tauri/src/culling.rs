use crate::ai_processing::{
    ClipModels, CullingModels, YunetFace, detect_faces_yolov8, detect_faces_yunet,
    get_or_init_clip_models, get_or_init_culling_models, run_expression_model,
    score_aesthetics_nima,
};
use crate::exif_processing::get_creation_date_from_path;
use crate::file_management::{load_settings, parse_virtual_path};
use crate::formats::is_raw_file;
use image::{DynamicImage, GenericImageView, GrayImage, Rgba, imageops};
use image_hasher::{HashAlg, HasherConfig};
use imageproc::geometric_transformations::{Interpolation, rotate_about_center};
use ort::value::Tensor;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::f32::consts::PI;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use tauri::{AppHandle, Emitter, State};

use crate::candidates::TAG_CANDIDATES;
use crate::image_loader;
use crate::image_processing::ImageMetadata;
use crate::image_processing::perform_auto_analysis;
use std::time::Duration;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum CullingProfile {
    Portrait,
    Landscape,
    Event,
    Default,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CullingSettings {
    pub similarity_threshold: u32,
    pub blur_threshold: f64,
    pub group_similar: bool,
    pub filter_blurry: bool,
    #[serde(default = "default_culling_profile")]
    pub profile: CullingProfile,
    #[serde(default = "default_true")]
    pub check_expression: bool,
    #[serde(default = "default_expression_strictness")]
    pub expression_strictness: u32,
}

fn default_culling_profile() -> CullingProfile {
    CullingProfile::Default
}

fn default_true() -> bool {
    true
}

fn default_expression_strictness() -> u32 {
    50
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ImageAnalysisResult {
    pub path: String,
    pub quality_score: f64,
    pub calibrated_score: f64,
    pub sharpness_metric: f64,
    pub center_focus_metric: f64,
    pub exposure_metric: f64,
    pub face_score: Option<f64>,
    pub aesthetic_score: Option<f64>,
    pub width: u32,
    pub height: u32,
    pub suggested_rating: u8,
    pub reasons: Vec<String>,
    pub score_breakdown: HashMap<String, f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub face_detector_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_id: Option<String>,
    #[serde(default)]
    pub is_cover: bool,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CullGroup {
    pub representative: ImageAnalysisResult,
    pub duplicates: Vec<ImageAnalysisResult>,
}

#[derive(Serialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct CullingSuggestions {
    pub similar_groups: Vec<CullGroup>,
    pub blurry_images: Vec<ImageAnalysisResult>,
    pub bad_expressions: Vec<ImageAnalysisResult>,
    pub failed_paths: Vec<String>,
}

#[derive(Serialize, Clone)]
struct CullingProgress {
    current: usize,
    total: usize,
    stage: String,
}

struct ImageAnalysisData {
    hash: image_hasher::ImageHash,
    clip_vec: Option<Vec<f32>>,
    result: ImageAnalysisResult,
    capture_time: i64,
}

struct InferenceSemaphore {
    permits: Mutex<usize>,
    cvar: Condvar,
}

struct InferencePermit<'a> {
    sem: &'a InferenceSemaphore,
}

impl InferenceSemaphore {
    fn new(permits: usize) -> Self {
        Self {
            permits: Mutex::new(permits.max(1)),
            cvar: Condvar::new(),
        }
    }

    fn acquire(&self) -> InferencePermit<'_> {
        let mut guard = self.permits.lock().unwrap();
        while *guard == 0 {
            guard = self.cvar.wait(guard).unwrap();
        }
        *guard -= 1;
        InferencePermit { sem: self }
    }
}

impl Drop for InferencePermit<'_> {
    fn drop(&mut self) {
        let mut guard = self.sem.permits.lock().unwrap();
        *guard += 1;
        self.sem.cvar.notify_one();
    }
}

fn calculate_laplacian_variance(image: &GrayImage) -> f64 {
    let (width, height) = image.dimensions();
    if width < 3 || height < 3 {
        return 0.0;
    }

    let mut laplacian_values = Vec::with_capacity(((width - 2) * (height - 2)) as usize);
    let mut sum = 0.0;

    for y in 1..height - 1 {
        for x in 1..width - 1 {
            let p_center = image.get_pixel(x, y)[0] as i32;
            let p_north = image.get_pixel(x, y - 1)[0] as i32;
            let p_south = image.get_pixel(x, y + 1)[0] as i32;
            let p_west = image.get_pixel(x - 1, y)[0] as i32;
            let p_east = image.get_pixel(x + 1, y)[0] as i32;
            let conv_val = (p_north + p_south + p_west + p_east - 4 * p_center) as f64;
            laplacian_values.push(conv_val);
            sum += conv_val;
        }
    }

    if laplacian_values.is_empty() {
        return 0.0;
    }
    let mean = sum / laplacian_values.len() as f64;

    laplacian_values
        .iter()
        .map(|v| (v - mean).powi(2))
        .sum::<f64>()
        / laplacian_values.len() as f64
}

fn softmax_1d(xs: &[f32]) -> Vec<f32> {
    if xs.is_empty() {
        return Vec::new();
    }
    let m = xs.iter().cloned().fold(f32::NEG_INFINITY, |a, b| a.max(b));
    let mut exps = Vec::with_capacity(xs.len());
    let mut sum = 0.0f32;
    for &x in xs {
        let e = (x - m).exp();
        exps.push(e);
        sum += e;
    }
    if sum <= 0.0 {
        return vec![0.0; xs.len()];
    }
    exps.into_iter().map(|e| e / sum).collect()
}

fn l2_normalize(v: &mut [f32]) {
    let mut norm2 = 0.0f32;
    for &x in v.iter() {
        norm2 += x * x;
    }
    let norm = norm2.sqrt();
    if norm > 1e-8 {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let mut sum = 0.0f32;
    for (x, y) in a.iter().zip(b.iter()) {
        sum += x * y;
    }
    sum
}

fn preprocess_clip_image(image: &DynamicImage) -> ndarray::Array4<f32> {
    let input_size: u32 = 224;
    let resized = image.resize_to_fill(input_size, input_size, imageops::FilterType::Triangle);
    let rgb = resized.to_rgb8();
    let mean = [0.48145466f32, 0.4578275f32, 0.40821073f32];
    let std = [0.26862954f32, 0.2613026f32, 0.2757771f32];
    let mut arr = ndarray::Array4::<f32>::zeros((1, 3, input_size as usize, input_size as usize));
    for (x, y, p) in rgb.enumerate_pixels() {
        let xf = x as usize;
        let yf = y as usize;
        let r = (p[0] as f32 / 255.0 - mean[0]) / std[0];
        let g = (p[1] as f32 / 255.0 - mean[1]) / std[1];
        let b = (p[2] as f32 / 255.0 - mean[2]) / std[2];
        arr[[0, 0, yf, xf]] = r;
        arr[[0, 1, yf, xf]] = g;
        arr[[0, 2, yf, xf]] = b;
    }
    arr
}

fn mean_and_highlight_clip(
    gray: &GrayImage,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    clip_thresh: u8,
) -> (f64, f64, u32) {
    if w == 0 || h == 0 {
        return (0.0, 0.0, 0);
    }
    let mut sum: u64 = 0;
    let mut clipped: u64 = 0;
    let mut count: u64 = 0;
    let x2 = (x + w).min(gray.width());
    let y2 = (y + h).min(gray.height());
    for yy in y..y2 {
        for xx in x..x2 {
            let v = gray.get_pixel(xx, yy)[0];
            sum += v as u64;
            if v >= clip_thresh {
                clipped += 1;
            }
            count += 1;
        }
    }
    if count == 0 {
        return (0.0, 0.0, 0);
    }
    let mean = (sum as f64) / (count as f64);
    let clip_ratio = (clipped as f64) / (count as f64);
    (mean, clip_ratio, count as u32)
}

fn analyze_image(
    path: &str,
    hasher: &image_hasher::Hasher,
    highlight_compression: f32,
    linear_mode: String,
    profile: &CullingProfile,
    blur_threshold: f64,
    check_expression: bool,
    expression_strictness: u32,
    clip_models: Option<&ClipModels>,
    clip_text_inputs: Option<&(Vec<String>, Vec<i64>, Vec<i64>, usize)>,
    culling_models: Option<&CullingModels>,
    inference_sem: &InferenceSemaphore,
) -> Result<ImageAnalysisData, String> {
    const ANALYSIS_DIM: u32 = 720;
    let file_bytes = std::fs::read(path).map_err(|e| e.to_string())?;

    let img = image_loader::load_base_image_from_bytes(
        &file_bytes,
        path,
        true,
        highlight_compression,
        linear_mode,
        None,
    )
    .map_err(|e| e.to_string())?;

    let (width, height) = img.dimensions();
    let thumbnail = img.thumbnail(ANALYSIS_DIM, ANALYSIS_DIM);
    let gray_thumbnail = thumbnail.to_luma8();

    let sharpness_metric = calculate_laplacian_variance(&gray_thumbnail);
    let auto = perform_auto_analysis(&img);
    let exposure_stops = (auto.exposure / 25.0).clamp(-8.0, 8.0);
    let norm_exposure = (-0.8 * exposure_stops.abs()).exp().clamp(0.0, 1.0);
    let mut exposure_metric = norm_exposure;
    let mut face_exposure_metric: Option<f64> = None;
    let mut face_highlight_clip: Option<f64> = None;
    let mut bg_highlight_clip: Option<f64> = None;
    let mut halo_forgiven: bool = false;

    let (thumb_w, thumb_h) = gray_thumbnail.dimensions();
    let center_crop = imageops::crop_imm(
        &gray_thumbnail,
        thumb_w / 4,
        thumb_h / 4,
        thumb_w / 2,
        thumb_h / 2,
    )
    .to_image();
    let center_focus_metric = calculate_laplacian_variance(&center_crop);

    let mut max_face_area_ratio: f64 = 0.0;
    let mut blink_prob: f64 = 0.0;
    let mut smile_prob: f64 = 0.0;
    let mut aesthetic_score: Option<f64> = None;
    let mut ai_fallback = false;
    let mut clip_vec: Option<Vec<f32>> = None;
    let mut primary_face_bbox: Option<(u32, u32, u32, u32)> = None;

    let mut face_score: Option<f64> = None;
    let mut unnatural_expression_max = 0.0;
    let expression_strictness_factor = (expression_strictness.min(100) as f64) / 100.0;

    // Default 0.6. Strictness 0 -> 0.9, 50 -> 0.6, 100 -> 0.3
    let blink_thresh = 0.9 - 0.6 * expression_strictness_factor;
    // Default 0.5. Strictness 0 -> 0.8, 50 -> 0.5, 100 -> 0.2
    let unnatural_thresh = 0.8 - 0.6 * expression_strictness_factor;

    let is_portrait_profile = matches!(profile, CullingProfile::Portrait);
    let is_landscape_profile = matches!(profile, CullingProfile::Landscape);

    let exp_skip = if is_portrait_profile {
        4.5
    } else if is_landscape_profile {
        5.0
    } else {
        3.0
    };
    let shadow_skip = if is_portrait_profile {
        85.0
    } else if is_landscape_profile {
        95.0
    } else {
        60.0
    };
    let highlight_skip = if is_portrait_profile {
        85.0
    } else if is_landscape_profile {
        95.0
    } else {
        60.0
    };
    let skip_face = sharpness_metric < (blur_threshold * 0.25).max(5.0);
    let skip_heavy = skip_face
        || exposure_stops.abs() > exp_skip
        || auto.shadows.abs() > shadow_skip
        || auto.highlights.abs() > highlight_skip;

    let mut face_detector_type = None;

    if let Some(models) = culling_models {
        if !skip_face {
            let _permit = inference_sem.acquire();

            #[derive(Clone)]
            struct DetectedFace {
                x1: f32,
                y1: f32,
                x2: f32,
                y2: f32,
                score: f32,
                landmarks: Option<[(f32, f32); 5]>,
            }

            let current_face_detector;

            let faces: Result<Vec<DetectedFace>, String> =
                if let Some(yunet) = models.yunet_detector.as_ref() {
                    match detect_faces_yunet(&img, yunet) {
                        Ok(fs) if !fs.is_empty() => {
                            current_face_detector = Some("yunet".to_string());
                            Ok(fs
                                .into_iter()
                                .map(|f: YunetFace| DetectedFace {
                                    x1: f.x1,
                                    y1: f.y1,
                                    x2: f.x2,
                                    y2: f.y2,
                                    score: f.score,
                                    landmarks: Some(f.landmarks),
                                })
                                .collect())
                        }
                        _ => {
                            current_face_detector = Some("yolov8".to_string());
                            detect_faces_yolov8(&img, &models.face_detector)
                                .map(|fs| {
                                    fs.into_iter()
                                        .map(|f| DetectedFace {
                                            x1: f.x1,
                                            y1: f.y1,
                                            x2: f.x2,
                                            y2: f.y2,
                                            score: f.score,
                                            landmarks: None,
                                        })
                                        .collect()
                                })
                                .map_err(|e| e.to_string())
                        }
                    }
                } else {
                    current_face_detector = Some("yolov8".to_string());
                    detect_faces_yolov8(&img, &models.face_detector)
                        .map(|fs| {
                            fs.into_iter()
                                .map(|f| DetectedFace {
                                    x1: f.x1,
                                    y1: f.y1,
                                    x2: f.x2,
                                    y2: f.y2,
                                    score: f.score,
                                    landmarks: None,
                                })
                                .collect()
                        })
                        .map_err(|e| e.to_string())
                };

            face_detector_type = current_face_detector;

            match faces {
                Ok(mut faces) => {
                    let img_area = (width as f64) * (height as f64);
                    faces.sort_by(|a, b| {
                        let area_a = (a.x2 - a.x1).max(0.0) as f64 * (a.y2 - a.y1).max(0.0) as f64;
                        let area_b = (b.x2 - b.x1).max(0.0) as f64 * (b.y2 - b.y1).max(0.0) as f64;
                        area_b
                            .partial_cmp(&area_a)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });

                    let mut primary_faces = Vec::new();
                    for f in faces.into_iter() {
                        let area =
                            ((f.x2 - f.x1).max(0.0) as f64) * ((f.y2 - f.y1).max(0.0) as f64);
                        let ratio = if img_area <= 0.0 {
                            0.0
                        } else {
                            area / img_area
                        };
                        if ratio >= 0.01 {
                            primary_faces.push((f, ratio));
                        }
                        if primary_faces.len() >= 3 {
                            break;
                        }
                    }

                    if let Some((_, r)) = primary_faces.first() {
                        max_face_area_ratio = *r;
                    }
                    if let Some((f, _)) = primary_faces.first() {
                        let x1 = f.x1.floor().max(0.0) as u32;
                        let y1 = f.y1.floor().max(0.0) as u32;
                        let x2 = f.x2.ceil().min(width as f32) as u32;
                        let y2 = f.y2.ceil().min(height as f32) as u32;
                        if x2 > x1 && y2 > y1 {
                            primary_face_bbox = Some((x1, y1, x2, y2));
                        }
                    }

                    if !primary_faces.is_empty() {
                        let mut blink_max = 0.0f64;
                        let mut smile_sum = 0.0f64;
                        let mut unnatural_max = 0.0f64;
                        let mut expr_count = 0usize;

                        for (face, ratio) in primary_faces {
                            max_face_area_ratio = max_face_area_ratio.max(ratio);
                            let bx1 = face.x1.floor().max(0.0) as f32;
                            let by1 = face.y1.floor().max(0.0) as f32;
                            let bx2 = face.x2.ceil().min(width as f32) as f32;
                            let by2 = face.y2.ceil().min(height as f32) as f32;
                            if bx2 <= bx1 || by2 <= by1 {
                                continue;
                            }
                            let bw = (bx2 - bx1).max(1.0);
                            let bh = (by2 - by1).max(1.0);
                            let side = bw.max(bh);
                            let cx = (bx1 + bx2) * 0.5;
                            let cy = (by1 + by2) * 0.5;
                            let pad = (side * 0.15).max(8.0);
                            let half = (side * 0.5) + pad;
                            let mut x1 = (cx - half).floor();
                            let mut y1 = (cy - half).floor();
                            let mut x2 = (cx + half).ceil();
                            let mut y2 = (cy + half).ceil();
                            x1 = x1.max(0.0);
                            y1 = y1.max(0.0);
                            x2 = x2.min(width as f32);
                            y2 = y2.min(height as f32);
                            if x2 <= x1 + 1.0 || y2 <= y1 + 1.0 {
                                continue;
                            }
                            let x1u = x1 as u32;
                            let y1u = y1 as u32;
                            let x2u = x2 as u32;
                            let y2u = y2 as u32;
                            let crop =
                                imageops::crop_imm(&img, x1u, y1u, x2u - x1u, y2u - y1u).to_image();
                            let mut crop_dyn = DynamicImage::ImageRgba8(crop);
                            if let Some(lms) = face.landmarks {
                                let (le, re) = (lms[0], lms[1]);
                                let angle_rad = (re.1 - le.1).atan2(re.0 - le.0);
                                if angle_rad.abs() > (1.0 * PI / 180.0) {
                                    let rotated = rotate_about_center(
                                        &crop_dyn.to_rgba32f(),
                                        -angle_rad,
                                        Interpolation::Bilinear,
                                        Rgba([0.0f32, 0.0, 0.0, 0.0]),
                                    );
                                    crop_dyn = DynamicImage::ImageRgba32F(rotated);
                                }
                            }
                            if let Some(expr_model) = &models.expression_model {
                                if check_expression {
                                    match run_expression_model(&crop_dyn, expr_model) {
                                        Ok((b, s, u)) => {
                                            blink_max = blink_max.max(b as f64);
                                            smile_sum += s as f64;
                                            unnatural_max = unnatural_max.max(u as f64);
                                            expr_count += 1;
                                        }
                                        Err(_) => {
                                            ai_fallback = true;
                                        }
                                    }
                                } else {
                                    // If expression check is disabled, just assume good expressions
                                    blink_max = 0.0;
                                    smile_sum += 0.5;
                                    unnatural_max = 0.0;
                                    expr_count += 1;
                                }
                            } else {
                                ai_fallback = true;
                            }
                        }

                        blink_prob = blink_max.clamp(0.0, 1.0);
                        smile_prob = if expr_count > 0 {
                            (smile_sum / expr_count as f64).clamp(0.0, 1.0)
                        } else {
                            0.0
                        };

                        let base = (1.0 - blink_prob).clamp(0.0, 1.0);
                        let mut final_face_score = base * (0.85 + 0.15 * smile_prob);

                        if is_portrait_profile && check_expression {
                            let max_penalty = 0.2 + 0.4 * expression_strictness_factor;
                            let penalty = (unnatural_max * max_penalty).clamp(0.0, max_penalty);
                            final_face_score -= penalty;
                        }

                        face_score = Some(final_face_score.clamp(0.0, 1.0));
                        unnatural_expression_max = unnatural_max;
                    }
                }
                Err(_) => {
                    ai_fallback = true;
                }
            }
        }

        if !skip_heavy {
            if let Some(aes_model) = &models.aesthetic_model {
                match score_aesthetics_nima(&img, aes_model) {
                    Ok(score) => {
                        aesthetic_score = Some(score as f64);
                    }
                    Err(_) => {
                        ai_fallback = true;
                    }
                }
            }
        }
    } else {
        ai_fallback = true;
    }

    if !skip_heavy {
        if let (Some(cm), Some((texts, ids_data, mask_data, max_len))) =
            (clip_models, clip_text_inputs)
        {
            let _permit = inference_sem.acquire();
            let image_input = preprocess_clip_image(&img);

            let n = texts.len();
            let ids_array = ndarray::Array::from_shape_vec((n, *max_len), ids_data.clone())
                .map_err(|e| e.to_string())?
                .into_dyn();
            let mask_array = ndarray::Array::from_shape_vec((n, *max_len), mask_data.clone())
                .map_err(|e| e.to_string())?
                .into_dyn();
            let image_dyn = image_input.into_dyn();

            let image_val = Tensor::from_array(image_dyn.as_standard_layout().into_owned())
                .map_err(|e| e.to_string())?;
            let ids_val = Tensor::from_array(ids_array.as_standard_layout().into_owned())
                .map_err(|e| e.to_string())?;
            let mask_val = Tensor::from_array(mask_array.as_standard_layout().into_owned())
                .map_err(|e| e.to_string())?;

            let logits = {
                let mut sess = cm.model.lock().unwrap();
                let outputs = sess
                    .run(ort::inputs![ids_val, image_val, mask_val])
                    .map_err(|e| e.to_string())?;
                outputs[0]
                    .try_extract_array::<f32>()
                    .map_err(|e| e.to_string())?
                    .to_owned()
            };

            let flat = logits.into_raw_vec();
            if !flat.is_empty() {
                let mut probs = if flat.iter().all(|v| (0.0..=1.0).contains(v)) {
                    flat
                } else {
                    softmax_1d(&flat)
                };
                l2_normalize(&mut probs);
                clip_vec = Some(probs);
            }
        }
    }

    let bt = blur_threshold.max(1.0);
    let k_blur = (2.0f64.ln()) / bt;
    let norm_blur = (1.0 - (-k_blur * sharpness_metric).exp()).clamp(0.0, 1.0);
    let norm_center_blur = (1.0 - (-k_blur * center_focus_metric).exp()).clamp(0.0, 1.0);

    let blended_blur = if is_landscape_profile {
        norm_center_blur.max(norm_blur)
    } else {
        (norm_blur * 0.6) + (norm_center_blur * 0.4)
    };

    let capture_time = get_creation_date_from_path(Path::new(path)).timestamp_millis();

    let has_large_face = max_face_area_ratio > 0.05;

    if is_portrait_profile && has_large_face {
        if let Some((x1, y1, x2, y2)) = primary_face_bbox {
            let sx = (thumb_w as f64) / (width as f64);
            let sy = (thumb_h as f64) / (height as f64);
            let fx1 = ((x1 as f64) * sx).floor().clamp(0.0, (thumb_w - 1) as f64) as u32;
            let fy1 = ((y1 as f64) * sy).floor().clamp(0.0, (thumb_h - 1) as f64) as u32;
            let fx2 = ((x2 as f64) * sx).ceil().clamp(0.0, thumb_w as f64) as u32;
            let fy2 = ((y2 as f64) * sy).ceil().clamp(0.0, thumb_h as f64) as u32;
            let fw = fx2.saturating_sub(fx1);
            let fh = fy2.saturating_sub(fy1);
            if fw > 8 && fh > 8 {
                let clip_thresh: u8 = 250;
                let (face_mean, face_clip, face_px) =
                    mean_and_highlight_clip(&gray_thumbnail, fx1, fy1, fw, fh, clip_thresh);
                let (global_mean, global_clip, global_px) =
                    mean_and_highlight_clip(&gray_thumbnail, 0, 0, thumb_w, thumb_h, clip_thresh);
                let bg_px = global_px.saturating_sub(face_px).max(1);
                let bg_clip = ((global_clip * (global_px as f64)) - (face_clip * (face_px as f64)))
                    / (bg_px as f64);
                let face_luma_score = (1.0 - ((face_mean - 130.0).abs() / 130.0)).clamp(0.0, 1.0);
                let face_clip_penalty = ((face_clip - 0.02).max(0.0) * 12.0).clamp(0.0, 1.0);
                let face_metric = (face_luma_score * (1.0 - face_clip_penalty)).clamp(0.0, 1.0);
                face_exposure_metric = Some(face_metric);
                face_highlight_clip = Some(face_clip);
                bg_highlight_clip = Some(bg_clip.max(0.0));

                if bg_clip > 0.05 && face_clip < 0.05 && bg_clip > face_clip * 1.5 {
                    halo_forgiven = true;
                }

                if halo_forgiven {
                    exposure_metric = exposure_metric.max(0.6);
                }

                exposure_metric = (0.7 * face_metric) + (0.3 * exposure_metric);
                exposure_metric = exposure_metric.clamp(0.0, 1.0);
                let _ = global_mean;
                let _ = global_clip;
            }
        }
    } else if is_landscape_profile {
        let clip_thresh: u8 = 250;
        let (_, global_clip, _) =
            mean_and_highlight_clip(&gray_thumbnail, 0, 0, thumb_w, thumb_h, clip_thresh);

        if global_clip > 0.05 {
            let max_highlight_penalty = 0.5;
            let current_penalty = 1.0 - exposure_metric;
            let capped_penalty = current_penalty.min(max_highlight_penalty);
            exposure_metric = 1.0 - capped_penalty;
        }

        if auto.shadows.abs() > 40.0 && auto.highlights.abs() > 40.0 {
            exposure_metric = exposure_metric.max(0.65);
        }
    }

    let (mut w_blur, mut w_exp, mut w_aes, mut w_face) = match profile {
        CullingProfile::Portrait => {
            if has_large_face {
                (0.35, 0.20, 0.0, 0.45)
            } else {
                (0.50, 0.30, 0.0, 0.20)
            }
        }
        CullingProfile::Landscape => (0.60, 0.20, 0.20, 0.0),
        CullingProfile::Event => {
            if has_large_face {
                (0.30, 0.25, 0.15, 0.30)
            } else {
                (0.45, 0.30, 0.15, 0.10)
            }
        }
        CullingProfile::Default => {
            if has_large_face {
                (0.35, 0.25, 0.10, 0.30)
            } else {
                (0.50, 0.40, 0.10, 0.0)
            }
        }
    };

    if face_score.is_none() {
        w_face = 0.0;
    }
    if aesthetic_score.is_none() {
        w_aes = 0.0;
    }
    if w_aes > 0.2 {
        w_aes = 0.2;
    }
    let w_sum = w_blur + w_exp + w_aes + w_face;
    if w_sum > 0.0 {
        w_blur /= w_sum;
        w_exp /= w_sum;
        w_aes /= w_sum;
        w_face /= w_sum;
    }

    let mut quality_score = (blended_blur * w_blur)
        + (exposure_metric * w_exp)
        + (aesthetic_score
            .map(|s| (s / 10.0).clamp(0.0, 1.0))
            .unwrap_or(0.0)
            * w_aes)
        + (face_score.unwrap_or(0.0) * w_face);

    if blended_blur < 0.15 {
        quality_score *= 0.5;
    }
    let base_blink_thresh = if is_portrait_profile {
        blink_thresh
    } else {
        blink_thresh + 0.1
    };
    if has_large_face && check_expression && blink_prob > base_blink_thresh {
        quality_score *= 0.6;
    }
    if is_portrait_profile && check_expression && unnatural_expression_max > unnatural_thresh {
        quality_score *= 0.65; // Severe penalty to ensure it drops to 1-2 stars
    }

    quality_score = quality_score.clamp(0.0, 1.0);

    let calibrated_score = quality_score;

    let hash = hasher.hash_image(&thumbnail);

    let mut reasons: Vec<String> = Vec::new();
    if blended_blur < 0.2 {
        reasons.push("blurSevere".to_string());
    } else if blended_blur < 0.4 {
        reasons.push("blurMild".to_string());
    }

    if is_portrait_profile && has_large_face {
        if let Some(fm) = face_exposure_metric {
            if fm < 0.35 {
                reasons.push("exposureSevere".to_string());
            } else if fm < 0.55 && !halo_forgiven {
                reasons.push("exposurePoor".to_string());
            }
        } else {
            if exposure_stops.abs() > 3.0
                || auto.shadows.abs() > 60.0
                || auto.highlights.abs() > 60.0
            {
                reasons.push("exposureSevere".to_string());
            } else if exposure_stops.abs() > 1.5 {
                reasons.push("exposurePoor".to_string());
            }
        }
    } else if is_landscape_profile {
        if exposure_stops.abs() > 4.0 || auto.shadows.abs() > 80.0 {
            reasons.push("exposureSevere".to_string());
        } else if exposure_stops.abs() > 2.0 {
            reasons.push("exposurePoor".to_string());
        }
    } else {
        if exposure_stops.abs() > 3.0 || auto.shadows.abs() > 60.0 || auto.highlights.abs() > 60.0 {
            reasons.push("exposureSevere".to_string());
        } else if exposure_stops.abs() > 1.5 {
            reasons.push("exposurePoor".to_string());
        }
    }

    if has_large_face && check_expression && blink_prob > base_blink_thresh {
        reasons.push("blinkDetected".to_string());
    }
    if is_portrait_profile && check_expression && unnatural_expression_max > unnatural_thresh {
        reasons.push("unnaturalExpression".to_string());
    }
    if smile_prob > 0.8 {
        reasons.push("smileGood".to_string());
    }
    if ai_fallback {
        reasons.push("aiFallback".to_string());
    }

    let suggested_rating = if calibrated_score > 0.85 {
        5
    } else if calibrated_score > 0.70 {
        4
    } else if calibrated_score > 0.50 {
        3
    } else if calibrated_score > 0.35 {
        2
    } else {
        1
    };

    let mut score_breakdown: HashMap<String, f64> = HashMap::new();
    score_breakdown.insert("blur".to_string(), blended_blur);
    score_breakdown.insert("exposure".to_string(), exposure_metric);
    score_breakdown.insert("blinkProb".to_string(), blink_prob);
    score_breakdown.insert("smileProb".to_string(), smile_prob);
    score_breakdown.insert("unnaturalProb".to_string(), unnatural_expression_max);
    if let Some(v) = face_exposure_metric {
        score_breakdown.insert("faceExposure".to_string(), v);
    }
    if let Some(v) = face_highlight_clip {
        score_breakdown.insert("faceHighlightClip".to_string(), v);
    }
    if let Some(v) = bg_highlight_clip {
        score_breakdown.insert("bgHighlightClip".to_string(), v);
    }
    if halo_forgiven {
        score_breakdown.insert("haloForgiven".to_string(), 1.0);
    }
    score_breakdown.insert("face".to_string(), face_score.unwrap_or(0.0));
    score_breakdown.insert(
        "aesthetic".to_string(),
        aesthetic_score
            .map(|s| (s / 10.0).clamp(0.0, 1.0))
            .unwrap_or(0.0),
    );
    score_breakdown.insert("calibrated".to_string(), calibrated_score);

    Ok(ImageAnalysisData {
        hash,
        capture_time,
        clip_vec,
        result: ImageAnalysisResult {
            path: path.to_string(),
            quality_score,
            calibrated_score,
            sharpness_metric,
            center_focus_metric,
            exposure_metric,
            face_score,
            aesthetic_score,
            width,
            height,
            suggested_rating,
            reasons,
            score_breakdown,
            face_detector_type,
            group_id: None,
            is_cover: false,
        },
    })
}

#[tauri::command]
pub async fn cull_images(
    paths: Vec<String>,
    settings: CullingSettings,
    state: State<'_, crate::AppState>,
    app_handle: AppHandle,
) -> Result<CullingSuggestions, String> {
    if paths.is_empty() {
        return Ok(CullingSuggestions::default());
    }

    #[derive(Clone)]
    struct AssetEntry {
        path: String,
        capture_time: i64,
        is_raw: bool,
    }

    let mut by_stem: HashMap<String, Vec<AssetEntry>> = HashMap::new();
    for path in paths {
        let path_obj = Path::new(&path);
        let stem = match path_obj.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };
        let capture_time = get_creation_date_from_path(path_obj).timestamp_millis();
        let is_raw = is_raw_file(path_obj);
        by_stem.entry(stem.clone()).or_default().push(AssetEntry {
            path,
            capture_time,
            is_raw,
        });
    }

    let mut primary_paths: Vec<String> = Vec::new();
    let mut primary_to_secondaries: HashMap<String, Vec<String>> = HashMap::new();

    for (_, mut entries) in by_stem {
        entries.sort_by_key(|e| e.capture_time);
        let mut cluster: Vec<AssetEntry> = Vec::new();
        let mut last_time: Option<i64> = None;

        let flush_cluster =
            |cluster: &mut Vec<AssetEntry>,
             primary_paths: &mut Vec<String>,
             primary_to_secondaries: &mut HashMap<String, Vec<String>>| {
                if cluster.is_empty() {
                    return;
                }
                let mut primary_idx: usize = 0;
                for (i, e) in cluster.iter().enumerate() {
                    if e.is_raw {
                        primary_idx = i;
                        break;
                    }
                }
                let primary_path = cluster[primary_idx].path.clone();
                let mut secondaries = Vec::new();
                for (i, e) in cluster.iter().enumerate() {
                    if i != primary_idx {
                        secondaries.push(e.path.clone());
                    }
                }
                primary_paths.push(primary_path.clone());
                if !secondaries.is_empty() {
                    primary_to_secondaries.insert(primary_path, secondaries);
                }
                cluster.clear();
            };

        for e in entries {
            if let Some(t) = last_time {
                if (e.capture_time - t).abs() >= 1000 {
                    flush_cluster(
                        &mut cluster,
                        &mut primary_paths,
                        &mut primary_to_secondaries,
                    );
                }
            }
            last_time = Some(e.capture_time);
            cluster.push(e);
        }
        flush_cluster(
            &mut cluster,
            &mut primary_paths,
            &mut primary_to_secondaries,
        );
    }

    let total_count = primary_paths.len();
    let completed_count = Arc::new(AtomicUsize::new(0));
    let _ = app_handle.emit("culling-start", total_count);
    let _ = app_handle.emit(
        "culling-progress",
        CullingProgress {
            current: 0,
            total: total_count,
            stage: "Preparing models...".to_string(),
        },
    );

    let app_settings = load_settings(app_handle.clone()).unwrap_or_default();
    let hc = app_settings.raw_highlight_compression.unwrap_or(2.5);
    let lrm = app_settings.linear_raw_mode;

    let culling_models =
        get_or_init_culling_models(&app_handle, &state.ai_state, &state.ai_init_lock)
            .await
            .ok();
    let clip_models = if settings.group_similar {
        match tokio::time::timeout(
            Duration::from_secs(
                std::env::var("QRAW_CULL_CLIP_INIT_TIMEOUT_SECS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(2),
            ),
            get_or_init_clip_models(&app_handle, &state.ai_state, &state.ai_init_lock),
        )
        .await
        {
            Ok(Ok(m)) => Some(m),
            _ => None,
        }
    } else {
        None
    };

    let clip_text_inputs = if let Some(cm) = clip_models.as_ref() {
        let max_texts: usize = std::env::var("QRAW_CULL_CLIP_TEXT_COUNT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(256);
        let texts: Vec<String> = TAG_CANDIDATES
            .iter()
            .take(max_texts.max(1))
            .map(|&s| s.to_string())
            .collect();
        let encodings = match cm.tokenizer.encode_batch(texts.clone(), true) {
            Ok(v) => v,
            Err(_) => Vec::new(),
        };
        if encodings.is_empty() {
            None
        } else {
            let max_len = encodings
                .iter()
                .map(|e| e.get_ids().len())
                .max()
                .unwrap_or(0)
                .max(1);
            let mut ids_data = Vec::with_capacity(texts.len() * max_len);
            let mut mask_data = Vec::with_capacity(texts.len() * max_len);
            for enc in encodings {
                let mut ids = enc.get_ids().iter().map(|&i| i as i64).collect::<Vec<_>>();
                let mut mask = enc
                    .get_attention_mask()
                    .iter()
                    .map(|&m| m as i64)
                    .collect::<Vec<_>>();
                ids.resize(max_len, 0);
                mask.resize(max_len, 0);
                ids_data.extend_from_slice(&ids);
                mask_data.extend_from_slice(&mask);
            }
            Some(Arc::new((texts, ids_data, mask_data, max_len)))
        }
    } else {
        None
    };
    let infer_concurrency: usize = std::env::var("QRAW_CULL_INFER_CONCURRENCY")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2);
    let inference_sem = Arc::new(InferenceSemaphore::new(infer_concurrency));

    let hasher = HasherConfig::new()
        .hash_alg(HashAlg::DoubleGradient)
        .hash_size(16, 16)
        .to_hasher();

    let sem_for_workers = inference_sem.clone();
    let profile_for_workers = settings.profile.clone();
    let culling_models_for_workers = culling_models.clone();
    let clip_models_for_workers = clip_models.clone();
    let clip_text_inputs_for_workers = clip_text_inputs.clone();

    let analysis_results: Vec<Result<ImageAnalysisData, (String, String)>> = primary_paths
        .par_iter()
        .map(|path| {
            let completed = completed_count.fetch_add(1, Ordering::Relaxed) + 1;
            let _ = app_handle.emit(
                "culling-progress",
                CullingProgress {
                    current: completed,
                    total: total_count,
                    stage: "Analyzing images...".to_string(),
                },
            );

            analyze_image(
                path,
                &hasher,
                hc,
                lrm.clone(),
                &profile_for_workers,
                settings.blur_threshold,
                settings.check_expression,
                settings.expression_strictness,
                clip_models_for_workers.as_deref(),
                clip_text_inputs_for_workers.as_deref(),
                culling_models_for_workers.as_deref(),
                sem_for_workers.as_ref(),
            )
            .map_err(|e| (path.to_string(), e))
        })
        .collect();

    let mut successful_analyses = Vec::new();
    let mut failed_paths = Vec::new();
    for res in analysis_results {
        match res {
            Ok(data) => successful_analyses.push(data),
            Err((path, error)) => {
                eprintln!("Failed to analyze image {}: {}", path, error);
                failed_paths.push(path);
            }
        }
    }

    let _ = app_handle.emit(
        "culling-progress",
        CullingProgress {
            current: total_count,
            total: total_count,
            stage: "Grouping similar images...".to_string(),
        },
    );

    let mut suggestions = CullingSuggestions {
        failed_paths,
        ..Default::default()
    };
    let mut processed_indices = vec![false; successful_analyses.len()];

    successful_analyses.sort_by_key(|a| a.capture_time);

    let cos_thresh: f32 = std::env::var("QRAW_CULL_COS_THRESH")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.92);
    let max_window_size: usize = std::env::var("QRAW_CULL_WINDOW_SIZE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(50);

    if settings.group_similar {
        let is_similar = |a: usize, b: usize, data: &[ImageAnalysisData]| -> bool {
            if let (Some(va), Some(vb)) = (data[a].clip_vec.as_ref(), data[b].clip_vec.as_ref()) {
                if va.len() == vb.len() && !va.is_empty() {
                    return cosine(va, vb) >= cos_thresh;
                }
            }
            data[a].hash.dist(&data[b].hash) <= settings.similarity_threshold
        };

        let mut group_counter: usize = 0;
        for i in 0..successful_analyses.len() {
            if processed_indices[i] {
                continue;
            }

            let mut current_group_indices = vec![];
            let mut queue = VecDeque::new();

            processed_indices[i] = true;
            current_group_indices.push(i);
            queue.push_back(i);

            let mut rep_idx = i;
            while let Some(current_idx) = queue.pop_front() {
                let current_time = successful_analyses[current_idx].capture_time;

                for j in (current_idx + 1)..successful_analyses.len() {
                    if processed_indices[j] {
                        continue;
                    }

                    let time_diff = successful_analyses[j].capture_time - current_time;
                    if time_diff > 1500 {
                        break;
                    }

                    if current_group_indices.len() >= max_window_size {
                        break;
                    }

                    if is_similar(current_idx, j, &successful_analyses)
                        && is_similar(rep_idx, j, &successful_analyses)
                    {
                        processed_indices[j] = true;
                        current_group_indices.push(j);
                        queue.push_back(j);
                        if successful_analyses[j].result.calibrated_score
                            > successful_analyses[rep_idx].result.calibrated_score
                        {
                            rep_idx = j;
                        }
                    }
                }

                for j in (0..current_idx).rev() {
                    if processed_indices[j] {
                        continue;
                    }

                    let time_diff = current_time - successful_analyses[j].capture_time;
                    if time_diff > 1500 {
                        break;
                    }

                    if current_group_indices.len() >= max_window_size {
                        break;
                    }

                    if is_similar(current_idx, j, &successful_analyses)
                        && is_similar(rep_idx, j, &successful_analyses)
                    {
                        processed_indices[j] = true;
                        current_group_indices.push(j);
                        queue.push_back(j);
                        if successful_analyses[j].result.calibrated_score
                            > successful_analyses[rep_idx].result.calibrated_score
                        {
                            rep_idx = j;
                        }
                    }
                }
            }

            if current_group_indices.len() > 1 {
                group_counter += 1;
                let group_id = format!("g{}", group_counter);

                let representative_idx = current_group_indices
                    .iter()
                    .cloned()
                    .max_by(|&a, &b| {
                        successful_analyses[a]
                            .result
                            .calibrated_score
                            .partial_cmp(&successful_analyses[b].result.calibrated_score)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .unwrap_or(i);

                let best_score = successful_analyses[representative_idx]
                    .result
                    .calibrated_score;
                for &idx in &current_group_indices {
                    let is_cover = idx == representative_idx;
                    let res = &mut successful_analyses[idx].result;
                    res.group_id = Some(group_id.clone());
                    res.is_cover = is_cover;
                    res.score_breakdown
                        .insert("groupBest".to_string(), best_score);
                    if !is_cover {
                        if !res.reasons.iter().any(|r| r == "burstDuplicate") {
                            res.reasons.push("burstDuplicate".to_string());
                        }
                        let s = res.calibrated_score;
                        if best_score > 0.0 && s >= best_score * 0.95 {
                            res.suggested_rating = res.suggested_rating.max(3).min(4);
                        } else if best_score > 0.0 && s < best_score * 0.85 {
                            res.suggested_rating = res.suggested_rating.min(2);
                        } else {
                            res.suggested_rating = res.suggested_rating.min(3);
                        }
                    }
                }

                let mut duplicate_indices: Vec<usize> = current_group_indices
                    .iter()
                    .cloned()
                    .filter(|&idx| idx != representative_idx)
                    .collect();
                duplicate_indices.sort_by(|&a, &b| {
                    successful_analyses[b]
                        .result
                        .calibrated_score
                        .partial_cmp(&successful_analyses[a].result.calibrated_score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });

                suggestions.similar_groups.push(CullGroup {
                    representative: successful_analyses[representative_idx].result.clone(),
                    duplicates: duplicate_indices
                        .into_iter()
                        .map(|idx| successful_analyses[idx].result.clone())
                        .collect(),
                });
            }
        }
    }

    if settings.filter_blurry {
        for i in 0..successful_analyses.len() {
            let item = &successful_analyses[i];
            if item.result.sharpness_metric < settings.blur_threshold {
                suggestions.blurry_images.push(item.result.clone());
            }
        }
        suggestions.blurry_images.sort_by(|a, b| {
            a.sharpness_metric
                .partial_cmp(&b.sharpness_metric)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    if settings.check_expression {
        for i in 0..successful_analyses.len() {
            let item = &successful_analyses[i];
            if item
                .result
                .reasons
                .iter()
                .any(|r| r == "unnaturalExpression" || r == "blinkDetected")
            {
                suggestions.bad_expressions.push(item.result.clone());
            }
        }
        suggestions.bad_expressions.sort_by(|a, b| {
            let a_unnatural = a
                .score_breakdown
                .get("unnaturalProb")
                .copied()
                .unwrap_or(0.0);
            let b_unnatural = b
                .score_breakdown
                .get("unnaturalProb")
                .copied()
                .unwrap_or(0.0);
            b_unnatural
                .partial_cmp(&a_unnatural)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    let mut all_paths_with_results: Vec<(String, ImageAnalysisResult)> = Vec::new();
    for item in &successful_analyses {
        all_paths_with_results.push((item.result.path.clone(), item.result.clone()));
        if let Some(secondaries) = primary_to_secondaries.get(&item.result.path) {
            for sp in secondaries {
                let mut inherited = item.result.clone();
                inherited.path = sp.clone();
                inherited
                    .score_breakdown
                    .insert("inherited".to_string(), 1.0);
                all_paths_with_results.push((sp.clone(), inherited));
            }
        }
    }

    for (path, res) in &all_paths_with_results {
        let (_, sidecar_path) = parse_virtual_path(path);
        let mut metadata: ImageMetadata = if sidecar_path.exists() {
            std::fs::read_to_string(&sidecar_path)
                .ok()
                .and_then(|content| serde_json::from_str(&content).ok())
                .unwrap_or_default()
        } else {
            ImageMetadata::default()
        };

        let mut adjustments = metadata.adjustments;
        if adjustments.is_null() {
            adjustments = serde_json::json!({});
        }
        if let Some(map) = adjustments.as_object_mut() {
            map.insert(
                "rating".to_string(),
                serde_json::json!(res.suggested_rating),
            );
            map.insert(
                "autoCulling".to_string(),
                serde_json::json!({
                    "rating": res.suggested_rating,
                    "reasons": res.reasons.clone(),
                    "scoreBreakdown": res.score_breakdown.clone(),
                    "groupId": res.group_id.clone(),
                    "isCover": res.is_cover
                }),
            );
        }
        metadata.adjustments = adjustments;
        metadata.rating = res.suggested_rating;

        if let Ok(json_string) = serde_json::to_string_pretty(&metadata) {
            let _ = std::fs::write(&sidecar_path, json_string);
        }
    }

    let mut by_rating: HashMap<u8, Vec<String>> = HashMap::new();
    for (path, res) in &all_paths_with_results {
        by_rating
            .entry(res.suggested_rating)
            .or_default()
            .push(path.clone());
    }
    for (rating, paths) in by_rating {
        let _ = crate::file_management::apply_adjustments_to_paths(
            paths,
            serde_json::json!({ "rating": rating }),
            app_handle.clone(),
        )
        .await;
    }

    let _ = app_handle.emit("culling-complete", &suggestions);
    Ok(suggestions)
}
