use std::sync::Mutex;

use anyhow::{Result, anyhow};
use image::{DynamicImage, GenericImageView, Rgb, RgbImage, imageops, imageops::FilterType};
use ndarray::{Array, Array4};
use ort::session::Session;
use ort::value::Tensor;

pub const YUNET_INPUT_SIZE: u32 = 640;
const YUNET_STRIDES: [u32; 3] = [8, 16, 32];
const YUNET_CONF_THRESHOLD: f32 = 0.6;
const YUNET_NMS_THRESHOLD: f32 = 0.45;

/// Only run the extra tiled passes when tiles actually recover resolution the
/// whole-image pass had to throw away. Below this size the single pass already
/// keeps faces near their native scale, so tiling would only cost time.
const TILE_MIN_DIMENSION: u32 = 1280;
/// Each tile covers 60% of the frame per axis, giving a 2x2 grid with ~20%
/// overlap so a face straddling a tile seam is still fully inside one tile.
const TILE_FRACTION: f32 = 0.6;

const OCEC_INPUT_HEIGHT: u32 = 24;
const OCEC_INPUT_WIDTH: u32 = 40;
#[cfg(test)]
const OCEC_STRONG_CLOSED_THRESHOLD: f32 = 0.30;
#[cfg(test)]
const OCEC_CONFIDENT_OPEN_THRESHOLD: f32 = 0.70;

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
/// Two accuracy-relevant details are handled here:
///
/// 1. The 640x640 model input is a true letterbox: the region is scaled by a
///    single uniform factor and padded, never stretched to fill the square.
///    Anisotropic resizing distorts faces away from the geometry YuNet was
///    trained on, and it also skews the five landmarks that `face_identity`
///    relies on for SFace alignment, which degrades identity embeddings.
/// 2. Large frames additionally get a 2x2 overlapping tiled pass. A whole-image
///    pass shrinks a distant face in a group shot below the smallest stride,
///    so tiles are what make small faces recoverable at all. Results from every
///    pass are merged with a single global NMS, which also removes duplicates
///    across scales and across the tile overlap.
pub fn run_yunet_detection(
    image: &DynamicImage,
    session: &Mutex<Session>,
) -> Result<Vec<DetectedFace>> {
    let (orig_w, orig_h) = image.dimensions();
    if orig_w == 0 || orig_h == 0 {
        return Ok(Vec::new());
    }

    let mut candidates = detect_in_region(image, 0.0, 0.0, session)?;

    for (x, y, tile_w, tile_h) in tile_regions(orig_w, orig_h) {
        let tile = image.crop_imm(x, y, tile_w, tile_h);
        candidates.extend(detect_in_region(&tile, x as f32, y as f32, session)?);
    }

    Ok(non_max_suppression(candidates, YUNET_NMS_THRESHOLD))
}

/// Scales a region into the square model input with one uniform factor and
/// zero padding on the right/bottom. Returns the canvas, the applied scale and
/// the size of the real content inside the canvas so padded area can be ignored.
fn letterbox_region(region: &DynamicImage) -> Option<(RgbImage, f32, f32, f32)> {
    let (width, height) = region.dimensions();
    if width == 0 || height == 0 {
        return None;
    }
    let scale =
        (YUNET_INPUT_SIZE as f32 / width as f32).min(YUNET_INPUT_SIZE as f32 / height as f32);
    let scaled_width = ((width as f32 * scale).round() as u32).clamp(1, YUNET_INPUT_SIZE);
    let scaled_height = ((height as f32 * scale).round() as u32).clamp(1, YUNET_INPUT_SIZE);
    let resized = region
        .resize_exact(scaled_width, scaled_height, FilterType::Triangle)
        .to_rgb8();
    let mut canvas = RgbImage::from_pixel(YUNET_INPUT_SIZE, YUNET_INPUT_SIZE, Rgb([0, 0, 0]));
    imageops::replace(&mut canvas, &resized, 0, 0);
    Some((canvas, scale, scaled_width as f32, scaled_height as f32))
}

/// Returns the 2x2 overlapping tiles for a frame, or an empty list when the
/// frame is too small for tiling to recover any detail.
fn tile_regions(width: u32, height: u32) -> Vec<(u32, u32, u32, u32)> {
    if width.min(height) < TILE_MIN_DIMENSION {
        return Vec::new();
    }
    let tile_width = ((width as f32 * TILE_FRACTION).round() as u32).max(1);
    let tile_height = ((height as f32 * TILE_FRACTION).round() as u32).max(1);
    let mut regions = Vec::new();
    for y in [0, height.saturating_sub(tile_height)] {
        for x in [0, width.saturating_sub(tile_width)] {
            let region = (x, y, tile_width, tile_height);
            if !regions.contains(&region) {
                regions.push(region);
            }
        }
    }
    regions
}

/// Runs one inference pass over `region` and returns detections translated back
/// into original-image pixel coordinates via `offset_x` / `offset_y`.
fn detect_in_region(
    region: &DynamicImage,
    offset_x: f32,
    offset_y: f32,
    session: &Mutex<Session>,
) -> Result<Vec<DetectedFace>> {
    let Some((canvas, scale, content_width, content_height)) = letterbox_region(region) else {
        return Ok(Vec::new());
    };
    if scale <= 0.0 {
        return Ok(Vec::new());
    }
    let raw_pixels = canvas.as_raw();

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

    let mut session_guard = session
        .lock()
        .map_err(|_| anyhow!("YuNet inference session lock is poisoned"))?;
    let outputs = session_guard.run(ort::inputs!["input" => t_input])?;

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

            // Discard anything centred in the zero-padded margin; it cannot be
            // a real face from this region.
            if cx > content_width || cy > content_height {
                continue;
            }

            let x1 = (cx - w / 2.0) / scale + offset_x;
            let y1 = (cy - h / 2.0) / scale + offset_y;
            let box_w = w / scale;
            let box_h = h / scale;

            let mut landmarks = [(0.0f32, 0.0f32); 5];
            for j in 0..5 {
                let lx = kps_slice[i * 10 + j * 2] * stride as f32 + prior_x;
                let ly = kps_slice[i * 10 + j * 2 + 1] * stride as f32 + prior_y;
                landmarks[j] = (lx / scale + offset_x, ly / scale + offset_y);
            }

            candidates.push(DetectedFace {
                bbox: [x1, y1, box_w, box_h],
                score,
                landmarks,
            });
        }
    }

    Ok(candidates)
}

fn non_max_suppression(mut candidates: Vec<DetectedFace>, iou_threshold: f32) -> Vec<DetectedFace> {
    candidates.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

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

    if union <= 0.0 {
        0.0
    } else {
        inter_area / union
    }
}

/// Classifies whether the eyes in a cropped face region are open or closed
/// using the OCEC model. Returns the probability that the eyes are open
/// (0.0 = closed, 1.0 = open).
pub fn run_ocec_classification(face_crop: &DynamicImage, session: &Mutex<Session>) -> Result<f32> {
    let input_tensor = prepare_ocec_input(face_crop);
    let input_tensor_dyn = input_tensor.into_dyn();
    let t_input = Tensor::from_array(input_tensor_dyn.as_standard_layout().into_owned())?;

    let mut session_guard = session
        .lock()
        .map_err(|_| anyhow!("OCEC inference session lock is poisoned"))?;
    let outputs = session_guard.run(ort::inputs!["images" => t_input])?;
    let output_tensor = outputs[0].try_extract_array::<f32>()?.to_owned();
    let prob_open = *output_tensor.as_slice().unwrap().first().unwrap_or(&1.0);

    Ok(prob_open.clamp(0.0, 1.0))
}

fn prepare_ocec_input(face_crop: &DynamicImage) -> Array4<f32> {
    let resized = face_crop.resize_exact(OCEC_INPUT_WIDTH, OCEC_INPUT_HEIGHT, FilterType::Triangle);
    let rgb = resized.to_rgb8();
    let raw_pixels = rgb.as_raw();
    let h = OCEC_INPUT_HEIGHT as usize;
    let w = OCEC_INPUT_WIDTH as usize;
    let mut input_tensor = Array4::<f32>::zeros((1, 3, h, w));
    for y in 0..h {
        for x in 0..w {
            let idx = (y * w + x) * 3;
            // The official OCEC pipeline receives OpenCV BGR images before
            // transposing to NCHW. DynamicImage is RGB, so swap channels here
            // instead of feeding a different color contract to the model.
            input_tensor[[0, 0, y, x]] = raw_pixels[idx + 2] as f32 / 255.0;
            input_tensor[[0, 1, y, x]] = raw_pixels[idx + 1] as f32 / 255.0;
            input_tensor[[0, 2, y, x]] = raw_pixels[idx] as f32 / 255.0;
        }
    }
    input_tensor
}

#[cfg(test)]
pub fn summarize_eye_state(probabilities: &[f32]) -> (Option<f32>, bool) {
    if probabilities.len() != 2 || probabilities.iter().any(|value| !value.is_finite()) {
        return (None, false);
    }
    let weakest_eye = probabilities[0].min(probabilities[1]);
    if weakest_eye <= OCEC_STRONG_CLOSED_THRESHOLD {
        (Some(weakest_eye), true)
    } else if probabilities
        .iter()
        .all(|value| *value >= OCEC_CONFIDENT_OPEN_THRESHOLD)
    {
        (Some(weakest_eye), false)
    } else {
        (None, false)
    }
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

#[cfg(test)]
mod tests {
    use image::{Rgb, RgbImage};

    use super::*;

    #[test]
    fn letterbox_preserves_aspect_ratio_instead_of_stretching() {
        // A 3:2 frame must keep its 3:2 content inside the square input.
        let image = DynamicImage::ImageRgb8(RgbImage::from_pixel(3000, 2000, Rgb([10, 20, 30])));

        let (canvas, scale, content_width, content_height) = letterbox_region(&image).unwrap();

        assert_eq!(canvas.width(), YUNET_INPUT_SIZE);
        assert_eq!(canvas.height(), YUNET_INPUT_SIZE);
        assert_eq!(content_width, YUNET_INPUT_SIZE as f32);
        assert!((content_height - 2.0 / 3.0 * YUNET_INPUT_SIZE as f32).abs() <= 1.0);
        assert!((scale - YUNET_INPUT_SIZE as f32 / 3000.0).abs() < 1e-6);
    }

    #[test]
    fn letterbox_pads_rather_than_scaling_the_short_axis() {
        let image = DynamicImage::ImageRgb8(RgbImage::from_pixel(1200, 600, Rgb([255, 255, 255])));

        let (canvas, _, _, content_height) = letterbox_region(&image).unwrap();

        // Content occupies the top half; the padded bottom row stays black.
        assert_eq!(canvas.get_pixel(0, 0), &Rgb([255, 255, 255]));
        assert_eq!(
            canvas.get_pixel(0, YUNET_INPUT_SIZE - 1),
            &Rgb([0, 0, 0]),
            "padding must not contain resized content"
        );
        assert!(content_height < YUNET_INPUT_SIZE as f32);
    }

    #[test]
    fn small_frames_skip_tiling_and_large_frames_get_overlapping_tiles() {
        assert!(tile_regions(1024, 768).is_empty());

        let tiles = tile_regions(4000, 3000);

        assert_eq!(tiles.len(), 4);
        let tile_width = tiles[0].2;
        let tile_height = tiles[0].3;
        assert_eq!(tile_width, 2400);
        assert_eq!(tile_height, 1800);
        // Two tiles per axis covering 60% each must overlap, so together they
        // exceed the frame and leave no uncovered seam.
        assert!(tile_width * 2 > 4000);
        assert!(tile_height * 2 > 3000);
        assert!(tiles.contains(&(0, 0, tile_width, tile_height)));
        assert!(tiles.contains(&(
            4000 - tile_width,
            3000 - tile_height,
            tile_width,
            tile_height
        )));
    }

    #[test]
    fn tile_regions_are_unique_for_square_frames() {
        let tiles = tile_regions(2000, 2000);

        let mut deduped = tiles.clone();
        deduped.dedup();
        assert_eq!(tiles.len(), deduped.len());
    }

    #[test]
    fn ocec_input_matches_the_official_bgr_channel_contract() {
        let image = DynamicImage::ImageRgb8(RgbImage::from_pixel(2, 2, Rgb([255, 128, 0])));

        let input = prepare_ocec_input(&image);

        assert_eq!(input[[0, 0, 0, 0]], 0.0);
        assert!((input[[0, 1, 0, 0]] - 128.0 / 255.0).abs() < f32::EPSILON);
        assert_eq!(input[[0, 2, 0, 0]], 1.0);
    }

    #[test]
    fn eye_state_requires_strong_evidence_and_both_eyes() {
        assert_eq!(summarize_eye_state(&[0.1, 0.9]), (Some(0.1), true));
        assert_eq!(summarize_eye_state(&[0.8, 0.9]), (Some(0.8), false));
        assert_eq!(summarize_eye_state(&[0.45, 0.9]), (None, false));
        assert_eq!(summarize_eye_state(&[0.1]), (None, false));
    }
}
