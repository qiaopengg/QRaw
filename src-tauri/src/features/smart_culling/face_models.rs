use std::sync::Mutex;

use anyhow::Result;
use image::{DynamicImage, GenericImageView, imageops::FilterType};
use ndarray::Array;
use ort::session::Session;
use ort::value::Tensor;

pub const YUNET_INPUT_SIZE: u32 = 640;
const YUNET_STRIDES: [u32; 3] = [8, 16, 32];
const YUNET_CONF_THRESHOLD: f32 = 0.6;
const YUNET_NMS_THRESHOLD: f32 = 0.45;

const OCEC_INPUT_HEIGHT: u32 = 24;
const OCEC_INPUT_WIDTH: u32 = 40;
const OCEC_OPEN_THRESHOLD: f32 = 0.5;

#[derive(Debug, Clone)]
pub struct DetectedFace {
    /// Bounding box in pixel coordinates of the input image: [x, y, width, height]
    pub bbox: [f32; 4],
    pub score: f32,
    /// 5 facial landmarks in pixel coordinates: right eye, left eye, nose tip,
    /// right mouth corner, left mouth corner.
    pub landmarks: [(f32, f32); 5],
}

/// Runs YuNet face detection on a full-resolution image.
///
/// The model has a fixed 640x640 input. The image is letterboxed (resized to
/// fit, preserving aspect ratio, no padding needed since we scale detections
/// back using independent x/y scale factors) before inference, then detections
/// are scaled back to the original image dimensions.
pub fn run_yunet_detection(
    image: &DynamicImage,
    session: &Mutex<Session>,
) -> Result<Vec<DetectedFace>> {
    let (orig_w, orig_h) = image.dimensions();
    if orig_w == 0 || orig_h == 0 {
        return Ok(Vec::new());
    }

    let resized = image.resize_exact(YUNET_INPUT_SIZE, YUNET_INPUT_SIZE, FilterType::Triangle);
    let rgb = resized.to_rgb8();
    let raw_pixels = rgb.as_raw();

    let size = YUNET_INPUT_SIZE as usize;
    let mut input_tensor: Array<f32, _> = Array::zeros((1, 3, size, size));
    for y in 0..size {
        for x in 0..size {
            let idx = (y * size + x) * 3;
            input_tensor[[0, 0, y, x]] = raw_pixels[idx] as f32;
            input_tensor[[0, 1, y, x]] = raw_pixels[idx + 1] as f32;
            input_tensor[[0, 2, y, x]] = raw_pixels[idx + 2] as f32;
        }
    }

    let input_tensor_dyn = input_tensor.into_dyn();
    let t_input = Tensor::from_array(input_tensor_dyn.as_standard_layout().into_owned())?;

    let mut session_guard = session.lock().unwrap();
    let outputs = session_guard.run(ort::inputs!["input" => t_input])?;

    let scale_x = orig_w as f32 / YUNET_INPUT_SIZE as f32;
    let scale_y = orig_h as f32 / YUNET_INPUT_SIZE as f32;

    let mut candidates: Vec<DetectedFace> = Vec::new();

    for &stride in YUNET_STRIDES.iter() {
        let feature_size = (YUNET_INPUT_SIZE / stride) as usize;

        let cls = outputs[format!("cls_{}", stride).as_str()]
            .try_extract_array::<f32>()?
            .to_owned();
        let obj = outputs[format!("obj_{}", stride).as_str()]
            .try_extract_array::<f32>()?
            .to_owned();
        let bbox = outputs[format!("bbox_{}", stride).as_str()]
            .try_extract_array::<f32>()?
            .to_owned();
        let kps = outputs[format!("kps_{}", stride).as_str()]
            .try_extract_array::<f32>()?
            .to_owned();

        let cls_slice = cls.as_slice().unwrap();
        let obj_slice = obj.as_slice().unwrap();
        let bbox_slice = bbox.as_slice().unwrap();
        let kps_slice = kps.as_slice().unwrap();

        let num_priors = feature_size * feature_size;
        for i in 0..num_priors {
            let score = cls_slice[i] * obj_slice[i];
            if score < YUNET_CONF_THRESHOLD {
                continue;
            }

            let col = (i % feature_size) as f32;
            let row = (i / feature_size) as f32;
            let prior_x = col * stride as f32;
            let prior_y = row * stride as f32;

            let bx = bbox_slice[i * 4];
            let by = bbox_slice[i * 4 + 1];
            let bw = bbox_slice[i * 4 + 2];
            let bh = bbox_slice[i * 4 + 3];

            let cx = bx * stride as f32 + prior_x;
            let cy = by * stride as f32 + prior_y;
            let w = bw.exp() * stride as f32;
            let h = bh.exp() * stride as f32;

            let x1 = (cx - w / 2.0) * scale_x;
            let y1 = (cy - h / 2.0) * scale_y;
            let box_w = w * scale_x;
            let box_h = h * scale_y;

            let mut landmarks = [(0.0f32, 0.0f32); 5];
            for j in 0..5 {
                let lx = kps_slice[i * 10 + j * 2] * stride as f32 + prior_x;
                let ly = kps_slice[i * 10 + j * 2 + 1] * stride as f32 + prior_y;
                landmarks[j] = (lx * scale_x, ly * scale_y);
            }

            candidates.push(DetectedFace {
                bbox: [x1, y1, box_w, box_h],
                score,
                landmarks,
            });
        }
    }

    Ok(non_max_suppression(candidates, YUNET_NMS_THRESHOLD))
}

fn non_max_suppression(mut candidates: Vec<DetectedFace>, iou_threshold: f32) -> Vec<DetectedFace> {
    candidates.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

    let mut kept: Vec<DetectedFace> = Vec::new();
    for face in candidates {
        let overlaps = kept
            .iter()
            .any(|kept_face| box_iou(&kept_face.bbox, &face.bbox) >= iou_threshold);
        if !overlaps {
            kept.push(face);
        }
    }
    kept
}

fn box_iou(a: &[f32; 4], b: &[f32; 4]) -> f32 {
    let ax1 = a[0];
    let ay1 = a[1];
    let ax2 = a[0] + a[2];
    let ay2 = a[1] + a[3];
    let bx1 = b[0];
    let by1 = b[1];
    let bx2 = b[0] + b[2];
    let by2 = b[1] + b[3];

    let inter_x1 = ax1.max(bx1);
    let inter_y1 = ay1.max(by1);
    let inter_x2 = ax2.min(bx2);
    let inter_y2 = ay2.min(by2);

    let inter_w = (inter_x2 - inter_x1).max(0.0);
    let inter_h = (inter_y2 - inter_y1).max(0.0);
    let inter_area = inter_w * inter_h;

    let area_a = (a[2]).max(0.0) * (a[3]).max(0.0);
    let area_b = (b[2]).max(0.0) * (b[3]).max(0.0);
    let union = area_a + area_b - inter_area;

    if union <= 0.0 { 0.0 } else { inter_area / union }
}

/// Classifies whether the eyes in a cropped face region are open or closed
/// using the OCEC model. Returns the probability that the eyes are open
/// (0.0 = closed, 1.0 = open).
pub fn run_ocec_classification(
    face_crop: &DynamicImage,
    session: &Mutex<Session>,
) -> Result<f32> {
    let resized = face_crop.resize_exact(OCEC_INPUT_WIDTH, OCEC_INPUT_HEIGHT, FilterType::Triangle);
    let rgb = resized.to_rgb8();
    let raw_pixels = rgb.as_raw();

    let h = OCEC_INPUT_HEIGHT as usize;
    let w = OCEC_INPUT_WIDTH as usize;
    let mut input_tensor: Array<f32, _> = Array::zeros((1, 3, h, w));
    for y in 0..h {
        for x in 0..w {
            let idx = (y * w + x) * 3;
            input_tensor[[0, 0, y, x]] = raw_pixels[idx] as f32 / 255.0;
            input_tensor[[0, 1, y, x]] = raw_pixels[idx + 1] as f32 / 255.0;
            input_tensor[[0, 2, y, x]] = raw_pixels[idx + 2] as f32 / 255.0;
        }
    }

    let input_tensor_dyn = input_tensor.into_dyn();
    let t_input = Tensor::from_array(input_tensor_dyn.as_standard_layout().into_owned())?;

    let mut session_guard = session.lock().unwrap();
    let outputs = session_guard.run(ort::inputs!["images" => t_input])?;
    let output_tensor = outputs[0].try_extract_array::<f32>()?.to_owned();
    let prob_open = *output_tensor.as_slice().unwrap().first().unwrap_or(&1.0);

    Ok(prob_open.clamp(0.0, 1.0))
}

pub fn is_eye_closed(prob_open: f32) -> bool {
    prob_open < OCEC_OPEN_THRESHOLD
}

/// Crops a square region around a single eye landmark, sized relative to the
/// inter-ocular distance so the crop tightly frames the eye regardless of
/// face size or distance from the camera.
pub fn crop_eye_region(
    image: &DynamicImage,
    eye_position: (f32, f32),
    inter_ocular_distance: f32,
) -> Option<DynamicImage> {
    if inter_ocular_distance <= 0.0 {
        return None;
    }

    let half_size = (inter_ocular_distance * 0.4).max(4.0);
    let (img_w, img_h) = image.dimensions();

    let x1 = (eye_position.0 - half_size).max(0.0) as u32;
    let y1 = (eye_position.1 - half_size).max(0.0) as u32;
    let x2 = ((eye_position.0 + half_size) as u32).min(img_w);
    let y2 = ((eye_position.1 + half_size) as u32).min(img_h);

    if x2 <= x1 || y2 <= y1 {
        return None;
    }

    Some(image.crop_imm(x1, y1, x2 - x1, y2 - y1))
}
