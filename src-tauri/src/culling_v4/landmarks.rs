use image::{DynamicImage, GenericImageView, imageops, imageops::FilterType};
use ndarray::Array4;
use ort::session::Session;
use ort::value::Tensor;
use std::sync::Mutex;

/// InsightFace 2d106det landmark indices (from official UniFace documentation)
///
/// Range   | Group         | Points
/// --------|---------------|-------
/// 0-32    | Face contour  | 33
/// 33-50   | Eyebrows      | 18 (left 33-41, right 42-50)
/// 51-62   | Nose          | 12
/// 63-86   | Eyes          | 24 (left 63-74, right 75-86)
/// 87-105  | Mouth         | 19 (outer 87-100, inner 101-105)
///
/// IMPORTANT: The internal ordering of points within each eye (which point is
/// upper lid vs lower lid) must be verified by running a visualization script
/// on test images before hardcoding EAR indices. The indices below are best-effort
/// based on the standard 12-point eye contour convention (upper lid first, then lower).

// Eye landmark indices — 12 points per eye
// Convention: points 0-5 = upper lid (left corner → right corner)
//             points 6-11 = lower lid (right corner → left corner)
// These MUST be verified with visualization before production use.
pub const LEFT_EYE_START: usize = 63;
pub const RIGHT_EYE_START: usize = 75;
pub const POINTS_PER_EYE: usize = 12;

// Mouth landmark indices
pub const MOUTH_LEFT_CORNER: usize = 87;
pub const MOUTH_RIGHT_CORNER: usize = 93;
pub const MOUTH_UPPER_LIP_MID: usize = 90;
pub const MOUTH_LOWER_LIP_MID: usize = 96;

// Eyebrow indices
pub const LEFT_BROW_MID: usize = 37;
pub const LEFT_EYE_TOP_MID: usize = 65; // upper lid midpoint of left eye

// Face contour
pub const FACE_TOP: usize = 0;
pub const FACE_CHIN: usize = 16;

/// Run InsightFace 2d106det landmark model
pub fn run_landmark_106(
    image: &DynamicImage,
    face_bbox: (f32, f32, f32, f32), // (x1, y1, x2, y2)
    model: &Mutex<Session>,
) -> Result<Vec<(f32, f32)>, String> {
    let (img_w, img_h) = image.dimensions();
    let (fx1, fy1, fx2, fy2) = face_bbox;

    // Crop face region with 20% padding
    let pad = ((fx2 - fx1).max(fy2 - fy1)) * 0.2;
    let x1 = (fx1 - pad).max(0.0) as u32;
    let y1 = (fy1 - pad).max(0.0) as u32;
    let x2 = (fx2 + pad).min(img_w as f32) as u32;
    let y2 = (fy2 + pad).min(img_h as f32) as u32;

    if x2 <= x1 || y2 <= y1 {
        return Err("Invalid face crop dimensions".into());
    }

    let crop = imageops::crop_imm(image, x1, y1, x2 - x1, y2 - y1).to_image();
    let crop_dyn = DynamicImage::ImageRgba8(crop);

    // Resize to 192x192
    let size = 192u32;
    let rgb = crop_dyn.to_rgb8();
    let resized = imageops::resize(&rgb, size, size, FilterType::Triangle);

    // Normalize: (pixel - 127.5) / 128.0
    let mut arr = Array4::<f32>::zeros((1, 3, size as usize, size as usize));
    for (px, py, p) in resized.enumerate_pixels() {
        arr[[0, 0, py as usize, px as usize]] = (p[0] as f32 - 127.5) / 128.0;
        arr[[0, 1, py as usize, px as usize]] = (p[1] as f32 - 127.5) / 128.0;
        arr[[0, 2, py as usize, px as usize]] = (p[2] as f32 - 127.5) / 128.0;
    }

    let input = Tensor::from_array(arr.into_dyn().as_standard_layout().into_owned())
        .map_err(|e| e.to_string())?;

    let output = {
        let mut sess = model.lock().unwrap();
        let outputs = sess.run(ort::inputs![input]).map_err(|e| e.to_string())?;
        outputs[0].try_extract_array::<f32>().map_err(|e| e.to_string())?.to_owned()
    };

    let flat = output.into_raw_vec_and_offset().0;
    if flat.len() < 212 {
        return Err(format!("Landmark output too short: {} (expected >= 212)", flat.len()));
    }

    // De-normalize to original image coordinates
    let crop_w = (x2 - x1) as f32;
    let crop_h = (y2 - y1) as f32;
    let landmarks: Vec<(f32, f32)> = (0..106).map(|i| {
        let lx = flat[i * 2] * crop_w + x1 as f32;
        let ly = flat[i * 2 + 1] * crop_h + y1 as f32;
        (lx, ly)
    }).collect();

    Ok(landmarks)
}

/// Compute Eye Aspect Ratio using 12-point eye contour
/// EAR = (v1 + v2 + v3) / (3 * h)
/// where v1,v2,v3 are vertical distances and h is horizontal distance
pub fn compute_ear_106(landmarks: &[(f32, f32)], eye_start: usize) -> f64 {
    if landmarks.len() < eye_start + POINTS_PER_EYE {
        return 0.3; // Default open-eye value if landmarks insufficient
    }

    let p = |offset: usize| landmarks[eye_start + offset];

    // Vertical distances: upper lid vs lower lid
    // Using convention: 0-5 upper, 6-11 lower (mirrored)
    let v1 = dist(p(2), p(10));
    let v2 = dist(p(3), p(9));
    let v3 = dist(p(4), p(8));

    // Horizontal distance: left corner to right corner
    let h = dist(p(0), p(6));

    if h < 1e-6 { return 0.0; }
    (v1 + v2 + v3) / (3.0 * h)
}

/// Compute mouth open ratio
pub fn compute_mouth_open(landmarks: &[(f32, f32)]) -> f64 {
    if landmarks.len() <= MOUTH_LOWER_LIP_MID { return 0.0; }

    let upper = landmarks[MOUTH_UPPER_LIP_MID];
    let lower = landmarks[MOUTH_LOWER_LIP_MID];
    let left = landmarks[MOUTH_LEFT_CORNER];
    let right = landmarks[MOUTH_RIGHT_CORNER];

    let vertical = dist(upper, lower);
    let horizontal = dist(left, right);
    if horizontal < 1e-6 { 0.0 } else { vertical / horizontal }
}

/// Compute brow furrow (how close eyebrows are to eyes)
pub fn compute_brow_furrow(landmarks: &[(f32, f32)]) -> f64 {
    if landmarks.len() <= LEFT_EYE_TOP_MID { return 0.0; }

    let brow = landmarks[LEFT_BROW_MID];
    let eye_top = landmarks[LEFT_EYE_TOP_MID];

    if landmarks.len() <= FACE_CHIN { return 0.0; }
    let face_height = dist(landmarks[FACE_TOP], landmarks[FACE_CHIN]);
    if face_height < 1e-6 { return 0.0; }

    let brow_eye_dist = dist(brow, eye_top);
    // Smaller distance = more furrowed
    (1.0 - (brow_eye_dist / face_height * 4.0).min(1.0)).max(0.0)
}

/// Compute mouth corner droop
pub fn compute_mouth_corner_droop(landmarks: &[(f32, f32)]) -> f64 {
    if landmarks.len() <= MOUTH_LOWER_LIP_MID { return 0.0; }

    let left_corner = landmarks[MOUTH_LEFT_CORNER];
    let right_corner = landmarks[MOUTH_RIGHT_CORNER];
    let lower_center = landmarks[MOUTH_LOWER_LIP_MID];

    let corner_avg_y = (left_corner.1 + right_corner.1) / 2.0;
    if corner_avg_y > lower_center.1 {
        ((corner_avg_y - lower_center.1) / 20.0).min(1.0) as f64
    } else {
        0.0
    }
}

fn dist(a: (f32, f32), b: (f32, f32)) -> f64 {
    (((a.0 - b.0) as f64).powi(2) + ((a.1 - b.1) as f64).powi(2)).sqrt()
}
