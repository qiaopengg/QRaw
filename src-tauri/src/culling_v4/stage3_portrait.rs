use image::{DynamicImage, GenericImageView, imageops};
use tauri::{AppHandle, Emitter};

use crate::ai_processing::{detect_faces_yunet, detect_faces_yolov8, run_expression_model};

use super::composition::score_composition;
use super::landmarks::*;
use super::types::*;

/// Stage 3: Portrait assessment
/// Only runs on Pass/Marginal assets that have faces
pub fn stage_3_portrait(
    registry: &AssetRegistry,
    verdicts: &[TechnicalVerdict],
    models: &CullingModelsV4,
    settings: &CullingSettingsV4,
    app_handle: &AppHandle,
) -> Vec<PortraitVerdict> {
    let total = registry.assets.len();
    let mut results = Vec::with_capacity(total);

    for (i, asset) in registry.assets.iter().enumerate() {
        let _ = app_handle.emit(
            "culling-progress",
            CullingProgressV4 {
                current: i + 1,
                total,
                stage: "Analyzing portraits...".into(),
            },
        );

        // Skip failed assets
        if verdicts[i].is_fail() {
            results.push(PortraitVerdict {
                asset_index: i,
                has_faces: false,
                primary_face_area_ratio: 0.0,
                faces: vec![],
                composition_score: 0.5,
            });
            continue;
        }

        let img = &*asset.thumbnail;
        let (width, height) = img.dimensions();
        let img_area = (width as f64) * (height as f64);

        // Step 1: Face detection (YuNet → YOLOv8 fallback)
        let face_boxes = detect_faces_for_culling(img, models);

        // Filter: area > 1%, take top 3
        let mut primary_faces: Vec<((f32, f32, f32, f32), f64)> = face_boxes
            .into_iter()
            .map(|(x1, y1, x2, y2)| {
                let area = ((x2 - x1).max(0.0) as f64) * ((y2 - y1).max(0.0) as f64);
                ((x1, y1, x2, y2), area / img_area)
            })
            .filter(|(_, ratio)| *ratio >= 0.01)
            .collect();
        primary_faces.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        primary_faces.truncate(3);

        if primary_faces.is_empty() {
            // No faces — still compute composition for non-face rules
            let comp = score_composition(&asset.gray_thumbnail, &[], &SceneType::Default);
            results.push(PortraitVerdict {
                asset_index: i,
                has_faces: false,
                primary_face_area_ratio: 0.0,
                faces: vec![],
                composition_score: comp,
            });
            continue;
        }

        let mut face_analyses = Vec::new();

        for (bbox, area_ratio) in &primary_faces {
            let mut analysis = FaceAnalysis {
                bbox: *bbox,
                area_ratio: *area_ratio,
                face_aspect_ratio: (bbox.2 - bbox.0) / (bbox.3 - bbox.1).max(1.0),
                ..Default::default()
            };

            // Extreme profile detection
            analysis.is_extreme_profile =
                analysis.face_aspect_ratio < 0.6 || analysis.face_aspect_ratio > 1.5;

            // Step 2: 106-point landmarks (if model available and not extreme profile)
            if let Some(lm_model) = &models.landmark_106 {
                if !analysis.is_extreme_profile {
                    if let Ok(landmarks) = run_landmark_106(img, *bbox, lm_model) {
                        // EAR blink detection
                        let ear_l = compute_ear_106(&landmarks, LEFT_EYE_START);
                        let ear_r = compute_ear_106(&landmarks, RIGHT_EYE_START);
                        analysis.ear_left = ear_l;
                        analysis.ear_right = ear_r;
                        analysis.is_eye_closed =
                            ear_l < settings.ear_threshold || ear_r < settings.ear_threshold;

                        // Mouth open ratio
                        analysis.mouth_open_ratio = compute_mouth_open(&landmarks);

                        // Brow furrow
                        analysis.brow_furrow = compute_brow_furrow(&landmarks);

                        // Mouth corner droop
                        analysis.mouth_corner_down = compute_mouth_corner_droop(&landmarks);
                    }
                }
            }

            // Step 3: Expression classification (HSEmotion or FerPlus fallback)
            let face_crop = crop_face_for_expression(img, *bbox, width, height);
            if let Some(expr_model) = &models.expression_model {
                // Try as HSEmotion first (224x224 color input)
                // If it fails, the model might be FerPlus (64x64 grayscale)
                match run_expression_model_v4(&face_crop, expr_model) {
                    Ok((smile, negative, label)) => {
                        analysis.smile_prob = smile;
                        analysis.negative_emotion_prob = negative;
                        analysis.emotion_label = label;
                    }
                    Err(_) => {
                        // Fallback: try existing FerPlus interface
                        if let Ok((blink, smile, unnatural)) =
                            run_expression_model(&face_crop, expr_model)
                        {
                            analysis.smile_prob = smile as f64;
                            analysis.negative_emotion_prob = unnatural as f64;
                            analysis.emotion_label = "ferplus_fallback".to_string();
                        }
                    }
                }
            }

            // Step 4: Composition indicators
            analysis.is_edge_cropped = is_face_edge_cropped(*bbox, width, height, 0.02);
            analysis.headroom_ratio = bbox.1 / height as f32;

            face_analyses.push(analysis);
        }

        // Step 5: Composition score
        let comp = score_composition(&asset.gray_thumbnail, &face_analyses, &SceneType::Default);

        results.push(PortraitVerdict {
            asset_index: i,
            has_faces: true,
            primary_face_area_ratio: primary_faces[0].1,
            faces: face_analyses,
            composition_score: comp,
        });
    }

    results
}

/// Detect faces using YuNet (preferred) or YOLOv8 (fallback)
fn detect_faces_for_culling(
    img: &DynamicImage,
    models: &CullingModelsV4,
) -> Vec<(f32, f32, f32, f32)> {
    // Try YuNet first
    if let Some(yunet) = &models.yunet_detector {
        if let Ok(faces) = detect_faces_yunet(img, yunet) {
            if !faces.is_empty() {
                return faces.into_iter().map(|f| (f.x1, f.y1, f.x2, f.y2)).collect();
            }
        }
    }
    // Fallback to YOLOv8
    if let Ok(faces) = detect_faces_yolov8(img, &models.face_detector) {
        return faces.into_iter().map(|f| (f.x1, f.y1, f.x2, f.y2)).collect();
    }
    vec![]
}

/// Crop face region for expression model input
fn crop_face_for_expression(
    img: &DynamicImage,
    bbox: (f32, f32, f32, f32),
    img_w: u32,
    img_h: u32,
) -> DynamicImage {
    let (x1, y1, x2, y2) = bbox;
    let bw = (x2 - x1).max(1.0);
    let bh = (y2 - y1).max(1.0);
    let side = bw.max(bh);
    let cx = (x1 + x2) * 0.5;
    let cy = (y1 + y2) * 0.5;
    let pad = (side * 0.15).max(8.0);
    let half = (side * 0.5) + pad;

    let crop_x1 = (cx - half).max(0.0) as u32;
    let crop_y1 = (cy - half).max(0.0) as u32;
    let crop_x2 = (cx + half).min(img_w as f32) as u32;
    let crop_y2 = (cy + half).min(img_h as f32) as u32;

    if crop_x2 <= crop_x1 + 1 || crop_y2 <= crop_y1 + 1 {
        return img.clone();
    }

    let crop = imageops::crop_imm(img, crop_x1, crop_y1, crop_x2 - crop_x1, crop_y2 - crop_y1)
        .to_image();
    DynamicImage::ImageRgba8(crop)
}

/// Run expression model (HSEmotion-style: 224x224 color, 8 classes)
fn run_expression_model_v4(
    face_crop: &DynamicImage,
    model: &std::sync::Mutex<ort::session::Session>,
) -> Result<(f64, f64, String), String> {
    let size = 224u32;
    let rgb = face_crop.to_rgb8();
    let resized = imageops::resize(&rgb, size, size, imageops::FilterType::Triangle);
    let mean = [0.485f32, 0.456, 0.406];
    let std_dev = [0.229f32, 0.224, 0.225];

    let mut arr = ndarray::Array4::<f32>::zeros((1, 3, size as usize, size as usize));
    for (x, y, p) in resized.enumerate_pixels() {
        arr[[0, 0, y as usize, x as usize]] = (p[0] as f32 / 255.0 - mean[0]) / std_dev[0];
        arr[[0, 1, y as usize, x as usize]] = (p[1] as f32 / 255.0 - mean[1]) / std_dev[1];
        arr[[0, 2, y as usize, x as usize]] = (p[2] as f32 / 255.0 - mean[2]) / std_dev[2];
    }

    let input = ort::value::Tensor::from_array(arr.into_dyn().as_standard_layout().into_owned())
        .map_err(|e| e.to_string())?;

    let output = {
        let mut sess = model.lock().unwrap();
        let outputs = sess.run(ort::inputs![input]).map_err(|e| e.to_string())?;
        outputs[0].try_extract_array::<f32>().map_err(|e| e.to_string())?.to_owned()
    };

    let flat = output.into_raw_vec_and_offset().0;
    if flat.len() < 8 {
        return Err("Expression model output too short".into());
    }

    let probs = softmax(&flat[..8]);
    let labels = [
        "anger", "contempt", "disgust", "fear", "happiness", "neutral", "sadness", "surprise",
    ];
    let max_idx = probs.iter().enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .map(|(i, _)| i)
        .unwrap_or(5);

    let smile_prob = probs.get(4).copied().unwrap_or(0.0) as f64;
    let negative = (probs.get(0).copied().unwrap_or(0.0)
        + probs.get(2).copied().unwrap_or(0.0)
        + probs.get(3).copied().unwrap_or(0.0)) as f64;

    Ok((smile_prob, negative, labels[max_idx].to_string()))
}

fn softmax(xs: &[f32]) -> Vec<f32> {
    if xs.is_empty() { return vec![]; }
    let max = xs.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let exps: Vec<f32> = xs.iter().map(|&x| (x - max).exp()).collect();
    let sum: f32 = exps.iter().sum();
    if sum <= 0.0 { return vec![0.0; xs.len()]; }
    exps.into_iter().map(|e| e / sum).collect()
}

fn is_face_edge_cropped(bbox: (f32, f32, f32, f32), img_w: u32, img_h: u32, _margin: f32) -> bool {
    let (x1, y1, x2, y2) = bbox;
    let w = img_w as f32;
    let h = img_h as f32;
    let face_w = (x2 - x1).max(1.0);
    let face_h = (y2 - y1).max(1.0);
    // Only flag as cropped if a significant portion of the face is outside the frame
    // (face box extends beyond image boundary by >15% of face size)
    let crop_left = (-x1).max(0.0) / face_w;
    let crop_top = (-y1).max(0.0) / face_h;
    let crop_right = (x2 - w).max(0.0) / face_w;
    let crop_bottom = (y2 - h).max(0.0) / face_h;
    let max_crop = crop_left.max(crop_top).max(crop_right).max(crop_bottom);
    max_crop > 0.15
}
