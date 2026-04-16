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
        // Use larger image for face detection (720px thumbnails may be too small)
        let detect_img = if width < 1000 || height < 1000 {
            // Upscale thumbnail for better face detection
            let scale = 1280.0 / (width.max(height) as f32);
            let new_w = (width as f32 * scale) as u32;
            let new_h = (height as f32 * scale) as u32;
            img.resize(new_w, new_h, image::imageops::FilterType::Triangle)
        } else {
            img.clone()
        };
        let (det_w, det_h) = detect_img.dimensions();
        let face_boxes_raw = detect_faces_for_culling(&detect_img, models);
        // Scale face boxes back to thumbnail coordinates
        let scale_x = width as f32 / det_w as f32;
        let scale_y = height as f32 / det_h as f32;
        let face_boxes: Vec<(f32,f32,f32,f32)> = face_boxes_raw.into_iter()
            .map(|(x1,y1,x2,y2)| (x1*scale_x, y1*scale_y, x2*scale_x, y2*scale_y))
            .collect();

        let _ = app_handle.emit("culling-debug", format!(
            "[Stage3] {} → raw_faces={} img={}x{}",
            asset.path.split('/').last().unwrap_or(&asset.path),
            face_boxes.len(), width, height
        ));

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

            // Debug logging - emit to frontend
            let debug_msg = format!(
                "[Stage3] {} face: area={:.3} ear_l={:.3} ear_r={:.3} closed={} smile={:.3} neg={:.3} emotion={} profile={}",
                asset.path.split('/').last().unwrap_or(&asset.path),
                analysis.area_ratio, analysis.ear_left, analysis.ear_right,
                analysis.is_eye_closed, analysis.smile_prob, analysis.negative_emotion_prob,
                analysis.emotion_label, analysis.is_extreme_profile
            );
            let _ = app_handle.emit("culling-debug", &debug_msg);

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

/// Detect faces using YuNet with multi-output format (12 tensors)
/// This handles the OpenCV Zoo YuNet model which outputs separate tensors per scale
fn detect_faces_yunet_multi_output(
    img: &DynamicImage,
    yunet: &std::sync::Mutex<ort::session::Session>,
) -> Vec<(f32, f32, f32, f32, f32)> {
    let (img_w, img_h) = img.dimensions();
    if img_w == 0 || img_h == 0 { return vec![]; }

    let input_size = 640u32;
    let rgb = img.to_rgb8();
    let resized = imageops::resize(&rgb, input_size, input_size, imageops::FilterType::Triangle);

    // NCHW format, 0-255 range
    let mut arr = ndarray::Array4::<f32>::zeros((1, 3, input_size as usize, input_size as usize));
    for (x, y, p) in resized.enumerate_pixels() {
        arr[[0, 0, y as usize, x as usize]] = p[0] as f32;
        arr[[0, 1, y as usize, x as usize]] = p[1] as f32;
        arr[[0, 2, y as usize, x as usize]] = p[2] as f32;
    }

    let input = match ort::value::Tensor::from_array(arr.into_dyn().as_standard_layout().into_owned()) {
        Ok(t) => t,
        Err(_) => return vec![],
    };

    let raw_outputs: Vec<(String, Vec<f32>, Vec<usize>)> = {
        let mut sess = yunet.lock().unwrap();
        let outputs = match sess.run(ort::inputs![input]) {
            Ok(o) => o,
            Err(_) => return vec![],
        };
        let mut result = vec![];
        let names = ["cls_8","cls_16","cls_32","obj_8","obj_16","obj_32","bbox_8","bbox_16","bbox_32"];
        for name in names {
            if let Some(v) = outputs.get(name) {
                if let Ok(arr) = v.try_extract_array::<f32>() {
                    let shape = arr.shape().to_vec();
                    let data = arr.to_owned().into_raw_vec_and_offset().0;
                    result.push((name.to_string(), data, shape));
                }
            }
        }
        result
    };

    // Parse multi-scale outputs
    let sx = img_w as f32 / input_size as f32;
    let sy = img_h as f32 / input_size as f32;
    let conf_thresh = 0.5f32;
    let mut faces: Vec<(f32, f32, f32, f32, f32)> = vec![];

    let strides = [8u32, 16, 32];
    let grid_sizes = [80u32, 40, 20]; // 640/8, 640/16, 640/32

    let find_output = |name: &str| -> Option<&Vec<f32>> {
        raw_outputs.iter().find(|(n, _, _)| n == name).map(|(_, d, _)| d)
    };

    let output_names = [
        ["cls_8", "cls_16", "cls_32"],
        ["obj_8", "obj_16", "obj_32"],
        ["bbox_8", "bbox_16", "bbox_32"],
    ];

    for scale_idx in 0..3 {
        let cls = match find_output(output_names[0][scale_idx]) {
            Some(v) => v,
            None => continue,
        };
        let obj = match find_output(output_names[1][scale_idx]) {
            Some(v) => v,
            None => continue,
        };
        let bbox = match find_output(output_names[2][scale_idx]) {
            Some(v) => v,
            None => continue,
        };

        let grid = grid_sizes[scale_idx] as usize;
        let stride = strides[scale_idx] as f32;
        let num_anchors = grid * grid;

        for i in 0..num_anchors {
            if i >= cls.len() || i >= obj.len() { break; }
            let score = cls[i] * obj[i];
            if score < conf_thresh { continue; }

            let row = i / grid;
            let col = i % grid;
            let cx = (col as f32 + 0.5) * stride;
            let cy = (row as f32 + 0.5) * stride;

            let bi = i * 4;
            if bi + 3 >= bbox.len() { continue; }
            let bx = bbox[bi] * stride;
            let by = bbox[bi + 1] * stride;
            let bw = bbox[bi + 2].exp() * stride;
            let bh = bbox[bi + 3].exp() * stride;

            let x1 = ((cx + bx - bw / 2.0) * sx).clamp(0.0, img_w as f32);
            let y1 = ((cy + by - bh / 2.0) * sy).clamp(0.0, img_h as f32);
            let x2 = ((cx + bx + bw / 2.0) * sx).clamp(0.0, img_w as f32);
            let y2 = ((cy + by + bh / 2.0) * sy).clamp(0.0, img_h as f32);

            if x2 > x1 + 5.0 && y2 > y1 + 5.0 {
                faces.push((x1, y1, x2, y2, score));
            }
        }
    }

    // Simple NMS
    faces.sort_by(|a, b| b.4.partial_cmp(&a.4).unwrap_or(std::cmp::Ordering::Equal));
    let mut kept: Vec<(f32, f32, f32, f32, f32)> = vec![];
    for f in &faces {
        let dominated = kept.iter().any(|k| {
            let ix1 = f.0.max(k.0);
            let iy1 = f.1.max(k.1);
            let ix2 = f.2.min(k.2);
            let iy2 = f.3.min(k.3);
            let inter = (ix2 - ix1).max(0.0) * (iy2 - iy1).max(0.0);
            let area_a = (f.2 - f.0) * (f.3 - f.1);
            let area_b = (k.2 - k.0) * (k.3 - k.1);
            let union = area_a + area_b - inter;
            union > 0.0 && inter / union > 0.3
        });
        if !dominated { kept.push(*f); }
    }
    kept
}

/// Detect faces using YuNet multi-output or standard YuNet/YOLOv8
fn detect_faces_for_culling(
    img: &DynamicImage,
    models: &CullingModelsV4,
) -> Vec<(f32, f32, f32, f32)> {
    // Try YuNet with multi-output format first
    if let Some(yunet) = &models.yunet_detector {
        let faces = detect_faces_yunet_multi_output(img, yunet);
        if !faces.is_empty() {
            return faces.into_iter().map(|(x1, y1, x2, y2, _)| (x1, y1, x2, y2)).collect();
        }
        // Also try standard format as fallback
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
