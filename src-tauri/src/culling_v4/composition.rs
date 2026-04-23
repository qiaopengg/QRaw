use image::GrayImage;
use imageproc::edges::canny;
use imageproc::hough::{LineDetectionOptions, detect_lines};

use super::types::{FaceAnalysis, SceneType};

/// Score composition using 10 rules (0.0 ~ 1.0)
/// Rules 1-5: face-dependent, Rules 6-10: general
pub fn score_composition(gray: &GrayImage, faces: &[FaceAnalysis], scene: &SceneType) -> f64 {
    let mut score: f64 = 0.5;
    let (w, h) = gray.dimensions();
    let wf = w as f32;
    let hf = h as f32;

    // ── Rule 1: Rule of thirds (face scenes) ──
    if let Some(face) = faces.first() {
        let cx = (face.bbox.0 + face.bbox.2) / 2.0;
        let cy = (face.bbox.1 + face.bbox.3) / 2.0;
        let dx = [wf / 3.0, wf * 2.0 / 3.0]
            .iter()
            .map(|t| (cx - t).abs())
            .fold(f32::MAX, f32::min);
        let dy = [hf / 3.0, hf * 2.0 / 3.0]
            .iter()
            .map(|t| (cy - t).abs())
            .fold(f32::MAX, f32::min);
        if dx < wf * 0.1 || dy < hf * 0.1 {
            score += 0.08;
        }
    }

    // ── Rule 2: Edge cropping penalty ──
    if faces
        .iter()
        .any(|f| f.is_edge_cropped && f.area_ratio > 0.05)
    {
        score -= 0.15;
    }

    // ── Rule 3: Headroom ──
    if let Some(f) = faces.first() {
        let margin = f.bbox.1 / hf;
        if margin < 0.03 {
            score -= 0.12;
        } else if margin < 0.08 {
            score -= 0.05;
        }
    }

    // ── Rule 4: Gaze direction space ──
    if let Some(f) = faces.first() {
        let fcx = (f.bbox.0 + f.bbox.2) / 2.0;
        let in_left = fcx < wf / 2.0;
        if in_left && f.bbox.0 / wf < 0.08 {
            score -= 0.08;
        }
        if !in_left && (wf - f.bbox.2) / wf < 0.08 {
            score -= 0.08;
        }
    }

    // ── Rule 5: Multi-face balance ──
    if faces.len() >= 2 {
        let mean_x: f32 = faces
            .iter()
            .map(|f| (f.bbox.0 + f.bbox.2) / 2.0)
            .sum::<f32>()
            / faces.len() as f32;
        if (mean_x - wf / 2.0).abs() / (wf / 2.0) < 0.15 {
            score += 0.05;
        }
    }

    // ── Rule 6: Horizon line (landscape scenes) ──
    if faces.is_empty() && matches!(scene, SceneType::Landscape | SceneType::Architecture) {
        let edges = canny(gray, 50.0, 150.0);
        let options = LineDetectionOptions {
            vote_threshold: 80,
            suppression_radius: 8,
        };
        let lines = detect_lines(&edges, options);
        for line in &lines {
            let a = line.angle_in_degrees as f32;
            if a < 5.0 || (a - 180.0).abs() < 5.0 {
                let sin_val = (a as f32).to_radians().sin().abs().max(0.01);
                let y_pos = (line.r.abs() as f32) / sin_val;
                let ratio = y_pos / hf;
                if (ratio - 0.33).abs() < 0.08 || (ratio - 0.67).abs() < 0.08 {
                    score += 0.08;
                    break;
                }
            }
        }
    }

    // ── Rule 7: Subject centering (no faces) ──
    if faces.is_empty() {
        let (vcx, vcy) = compute_visual_center(gray);
        let offset = ((vcx - 0.5).powi(2) + (vcy - 0.5).powi(2)).sqrt();
        if offset < 0.15 {
            score += 0.05;
        }
    }

    // ── Rule 8: Tilt detection ──
    if faces.is_empty() {
        let edges = canny(gray, 50.0, 150.0);
        let options = LineDetectionOptions {
            vote_threshold: 60,
            suppression_radius: 8,
        };
        let lines = detect_lines(&edges, options);
        if let Some(angle) = find_dominant_tilt(&lines) {
            if angle.abs() > 3.0 && angle.abs() < 15.0 {
                score -= 0.08;
            }
        }
    }

    // ── Rule 9: Foreground/background contrast ──
    let center_var = region_variance(gray, w / 4, h / 4, w / 2, h / 2);
    let global_var = region_variance(gray, 0, 0, w, h);
    if global_var > 0.0 && center_var > global_var * 1.3 {
        score += 0.05;
    }

    // ── Rule 10: Face proportion ──
    if let Some(f) = faces.first() {
        if f.area_ratio > 0.03 && f.area_ratio < 0.25 {
            score += 0.04;
        } else if f.area_ratio > 0.5 {
            score -= 0.03;
        }
    }

    score.clamp(0.0, 1.0)
}

fn compute_visual_center(gray: &GrayImage) -> (f64, f64) {
    let (w, h) = gray.dimensions();
    let mut sum_x = 0.0f64;
    let mut sum_y = 0.0f64;
    let mut total = 0.0f64;
    // Sample every 4th pixel for performance
    for y in (0..h).step_by(4) {
        for x in (0..w).step_by(4) {
            let weight = gray.get_pixel(x, y)[0] as f64;
            sum_x += x as f64 * weight;
            sum_y += y as f64 * weight;
            total += weight;
        }
    }
    if total < 1.0 {
        return (0.5, 0.5);
    }
    (sum_x / total / w as f64, sum_y / total / h as f64)
}

fn region_variance(gray: &GrayImage, x: u32, y: u32, w: u32, h: u32) -> f64 {
    let mut sum = 0.0f64;
    let mut sum_sq = 0.0f64;
    let mut count = 0u64;
    let x2 = (x + w).min(gray.width());
    let y2 = (y + h).min(gray.height());
    for yy in y..y2 {
        for xx in x..x2 {
            let v = gray.get_pixel(xx, yy)[0] as f64;
            sum += v;
            sum_sq += v * v;
            count += 1;
        }
    }
    if count == 0 {
        return 0.0;
    }
    let mean = sum / count as f64;
    (sum_sq / count as f64 - mean * mean).max(0.0)
}

fn find_dominant_tilt(lines: &[imageproc::hough::PolarLine]) -> Option<f32> {
    if lines.is_empty() {
        return None;
    }
    // Find the line with most votes
    // PolarLine has angle_in_degrees (u32) and r (f32)
    // We look for deviation from horizontal (0/180) or vertical (90/270)
    let mut best_dev: Option<f32> = None;
    for line in lines {
        let angle = line.angle_in_degrees as f32;
        let dev_h = angle.min((angle - 180.0).abs());
        let dev_v = (angle - 90.0).abs().min((angle - 270.0).abs());
        let min_dev = dev_h.min(dev_v);
        if min_dev > 1.0 && min_dev < 20.0 {
            if best_dev.is_none() || min_dev < best_dev.unwrap() {
                best_dev = Some(min_dev);
            }
        }
    }
    best_dev
}
