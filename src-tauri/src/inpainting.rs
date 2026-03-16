use image::{DynamicImage, GenericImageView, GrayImage, RgbImage, RgbaImage};
use rand::seq::SliceRandom;
use rayon::prelude::*;
use std::cmp::Ordering;
use std::collections::BinaryHeap;

const PIXEL_KNOWN: u8 = 0;
const PIXEL_HOLE: u8 = 1;
const PIXEL_FRONT: u8 = 2;

#[derive(Debug, Copy, Clone, PartialEq)]
struct FloatOrd(f32);
impl Eq for FloatOrd {}
impl PartialOrd for FloatOrd {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}
impl Ord for FloatOrd {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
struct FloatOrdF64(f64);
impl Eq for FloatOrdF64 {}
impl PartialOrd for FloatOrdF64 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}
impl Ord for FloatOrdF64 {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}

struct HeapItem {
    priority: FloatOrd,
    x: u32,
    y: u32,
    confidence: f32,
}
impl Ord for HeapItem {
    fn cmp(&self, other: &Self) -> Ordering {
        other.priority.cmp(&self.priority)
    }
}
impl PartialOrd for HeapItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl PartialEq for HeapItem {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority
    }
}
impl Eq for HeapItem {}

fn inpaint_criminisi(source_image: &RgbImage, mask: &GrayImage, patch_radius: u32) -> RgbImage {
    let (width, height) = source_image.dimensions();
    let mut output = source_image.clone();
    let mut pixel_states = vec![PIXEL_KNOWN; (width * height) as usize];
    let mut confidence = vec![0.0f32; (width * height) as usize];
    let mut narrow_band = BinaryHeap::new();

    let mut float_output = vec![[0.0f32; 3]; (width * height) as usize];
    let mut total_weights = vec![0.0f32; (width * height) as usize];

    let gaussian_kernel = get_gaussian_kernel(patch_radius, patch_radius as f32 / 2.0);

    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) as usize;
            if mask.get_pixel(x, y)[0] > 0 {
                pixel_states[idx] = PIXEL_HOLE;
            } else {
                confidence[idx] = 1.0;
                let p = source_image.get_pixel(x, y);
                float_output[idx] = [p[0] as f32, p[1] as f32, p[2] as f32];
                total_weights[idx] = 1.0;
            }
        }
    }

    let mut sat = vec![0u32; (width * height) as usize];
    for y in 0..height {
        let mut row_sum = 0;
        for x in 0..width {
            if pixel_states[(y * width + x) as usize] != PIXEL_KNOWN {
                row_sum += 1;
            }
            let prev_col = if y > 0 {
                sat[((y - 1) * width + x) as usize]
            } else {
                0
            };
            sat[(y * width + x) as usize] = prev_col + row_sum;
        }
    }

    let mut num_unknowns = vec![0u32; (width * height) as usize];
    if height > 2 * patch_radius && width > 2 * patch_radius {
        for y in patch_radius..=height.saturating_sub(patch_radius + 1) {
            for x in patch_radius..=width.saturating_sub(patch_radius + 1) {
                let y1 = y + patch_radius;
                let x1 = x + patch_radius;

                let mut count = sat[(y1 * width + x1) as usize] as i64;
                if y > patch_radius {
                    count -= sat[((y - patch_radius - 1) * width + x1) as usize] as i64;
                }
                if x > patch_radius {
                    count -= sat[(y1 * width + x - patch_radius - 1) as usize] as i64;
                }
                if y > patch_radius && x > patch_radius {
                    count += sat[((y - patch_radius - 1) * width + x - patch_radius - 1) as usize]
                        as i64;
                }
                num_unknowns[(y * width + x) as usize] = count as u32;
            }
        }
    }

    let mut front_points = Vec::new();

    for y in 1..height.saturating_sub(1) {
        for x in 1..width.saturating_sub(1) {
            if pixel_states[(y * width + x) as usize] == PIXEL_HOLE
                && get_neighbors(x, y, width, height)
                    .iter()
                    .any(|(nx, ny)| pixel_states[(ny * width + nx) as usize] == PIXEL_KNOWN)
            {
                pixel_states[(y * width + x) as usize] = PIXEL_FRONT;
                front_points.push((x, y));
            }
        }
    }

    loop {
        front_points.retain(|&(x, y)| pixel_states[(y * width + x) as usize] == PIXEL_FRONT);

        if front_points.is_empty() {
            break;
        }

        narrow_band.clear();
        let out_slice = output.as_raw();

        for &(x, y) in &front_points {
            let mut avg_normal = (0.0, 0.0);
            let mut count = 0;
            for dy in -2..=2 {
                for dx in -2..=2 {
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;
                    if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
                        let nx = nx as u32;
                        let ny = ny as u32;
                        if pixel_states[(ny * width + nx) as usize] == PIXEL_FRONT {
                            let normal = calculate_normal(&pixel_states, width, height, nx, ny);
                            avg_normal.0 += normal.0;
                            avg_normal.1 += normal.1;
                            count += 1;
                        }
                    }
                }
            }

            let normal = if count > 0 {
                let mag = (avg_normal.0 * avg_normal.0 + avg_normal.1 * avg_normal.1).sqrt();
                if mag > 1e-6 {
                    (avg_normal.0 / mag, avg_normal.1 / mag)
                } else {
                    calculate_normal(&pixel_states, width, height, x, y)
                }
            } else {
                calculate_normal(&pixel_states, width, height, x, y)
            };

            let (priority, confidence_term) = calculate_priority(
                out_slice,
                &pixel_states,
                &confidence,
                width,
                height,
                x,
                y,
                patch_radius,
                normal,
            );
            narrow_band.push(HeapItem {
                priority: FloatOrd(priority),
                x,
                y,
                confidence: confidence_term,
            });
        }

        if narrow_band.is_empty() {
            break;
        }

        let num_patches_per_iteration = 1;

        for _ in 0..num_patches_per_iteration {
            if let Some(p_hat_item) = narrow_band.pop() {
                let (px, py) = (p_hat_item.x, p_hat_item.y);
                let p_idx = (py * width + px) as usize;

                if pixel_states[p_idx] != PIXEL_FRONT {
                    continue;
                }

                let p_hat_confidence = p_hat_item.confidence;
                let search_radius = (patch_radius * 7).max(30);
                let max_samples = 500;

                let (best_match_x, best_match_y) = find_best_match_local(
                    output.as_raw(),
                    &pixel_states,
                    &num_unknowns,
                    width,
                    height,
                    px,
                    py,
                    patch_radius,
                    search_radius,
                    max_samples,
                    &gaussian_kernel,
                );

                let r = patch_radius as i32;
                let patch_diameter = (patch_radius * 2 + 1) as usize;
                let mut filled_pixels_coords = Vec::new();
                let mask_slice = mask.as_raw();
                let out_slice_mut = output.as_mut();

                for dy in -r..=r {
                    for dx in -r..=r {
                        let target_x =
                            (px as i32 + dx).clamp(0, (width.saturating_sub(1)) as i32) as u32;
                        let target_y =
                            (py as i32 + dy).clamp(0, (height.saturating_sub(1)) as i32) as u32;
                        let idx = (target_y * width + target_x) as usize;

                        if mask_slice[idx] > 0 {
                            let source_x = (best_match_x as i32 + dx)
                                .clamp(0, (width.saturating_sub(1)) as i32)
                                as u32;
                            let source_y = (best_match_y as i32 + dy)
                                .clamp(0, (height.saturating_sub(1)) as i32)
                                as u32;

                            let weight = gaussian_kernel
                                [((dy + r) as usize * patch_diameter) + (dx + r) as usize];

                            let src_idx = ((source_y * width + source_x) * 3) as usize;
                            let src_r = out_slice_mut[src_idx] as f32;
                            let src_g = out_slice_mut[src_idx + 1] as f32;
                            let src_b = out_slice_mut[src_idx + 2] as f32;

                            float_output[idx][0] += src_r * weight;
                            float_output[idx][1] += src_g * weight;
                            float_output[idx][2] += src_b * weight;
                            total_weights[idx] += weight;

                            if total_weights[idx] > 0.0 {
                                let target_idx = idx * 3;
                                out_slice_mut[target_idx] =
                                    (float_output[idx][0] / total_weights[idx]).clamp(0.0, 255.0)
                                        as u8;
                                out_slice_mut[target_idx + 1] =
                                    (float_output[idx][1] / total_weights[idx]).clamp(0.0, 255.0)
                                        as u8;
                                out_slice_mut[target_idx + 2] =
                                    (float_output[idx][2] / total_weights[idx]).clamp(0.0, 255.0)
                                        as u8;
                            }

                            if pixel_states[idx] != PIXEL_KNOWN {
                                confidence[idx] = p_hat_confidence;
                                pixel_states[idx] = PIXEL_KNOWN;
                                filled_pixels_coords.push((target_x, target_y));

                                let cy_min =
                                    (target_y.saturating_sub(patch_radius)).max(patch_radius);
                                let cy_max = (target_y + patch_radius)
                                    .min(height.saturating_sub(patch_radius + 1));
                                let cx_min =
                                    (target_x.saturating_sub(patch_radius)).max(patch_radius);
                                let cx_max = (target_x + patch_radius)
                                    .min(width.saturating_sub(patch_radius + 1));

                                if cy_min <= cy_max && cx_min <= cx_max {
                                    for cy in cy_min..=cy_max {
                                        let row_offset = (cy * width) as usize;
                                        for cx in cx_min..=cx_max {
                                            let unk_idx = row_offset + cx as usize;
                                            num_unknowns[unk_idx] =
                                                num_unknowns[unk_idx].saturating_sub(1);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                for &(x_filled, y_filled) in &filled_pixels_coords {
                    for (nx, ny) in get_neighbors(x_filled, y_filled, width, height) {
                        let n_idx = (ny * width + nx) as usize;
                        if pixel_states[n_idx] == PIXEL_HOLE {
                            pixel_states[n_idx] = PIXEL_FRONT;
                            front_points.push((nx, ny));
                        }
                    }
                }
            } else {
                break;
            }
        }
    }
    output
}

fn get_gaussian_kernel(radius: u32, sigma: f32) -> Vec<f32> {
    let diameter = (radius * 2 + 1) as usize;
    let mut kernel = vec![0.0; diameter * diameter];
    let r_i32 = radius as i32;
    let sigma2 = 2.0 * sigma * sigma;
    let mut sum = 0.0;

    for dy in -r_i32..=r_i32 {
        for dx in -r_i32..=r_i32 {
            let distance_sq = (dx * dx + dy * dy) as f32;
            let val = (-distance_sq / sigma2).exp();
            kernel[((dy + r_i32) as usize * diameter) + (dx + r_i32) as usize] = val;
            sum += val;
        }
    }
    if sum > 0.0 {
        kernel.iter_mut().for_each(|v| *v /= sum);
    }
    kernel
}

fn get_neighbors(x: u32, y: u32, width: u32, height: u32) -> Vec<(u32, u32)> {
    let mut neighbors = Vec::with_capacity(8);
    for dy in -1..=1 {
        for dx in -1..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let nx = x as i32 + dx;
            let ny = y as i32 + dy;
            if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
                neighbors.push((nx as u32, ny as u32));
            }
        }
    }
    neighbors
}

fn calculate_normal(pixel_states: &[u8], width: u32, height: u32, x: u32, y: u32) -> (f32, f32) {
    let x_p1 = (x + 1).min(width.saturating_sub(1));
    let x_m1 = x.saturating_sub(1);
    let y_p1 = (y + 1).min(height.saturating_sub(1));
    let y_m1 = y.saturating_sub(1);

    let state_at = |cx, cy| {
        if pixel_states[(cy * width + cx) as usize] == PIXEL_KNOWN {
            0
        } else {
            1
        }
    };

    let grad_x = (state_at(x_p1, y) as i32 - state_at(x_m1, y) as i32) as f32;
    let grad_y = (state_at(x, y_p1) as i32 - state_at(x, y_m1) as i32) as f32;
    let mag = (grad_x * grad_x + grad_y * grad_y).sqrt();
    if mag > 1e-6 {
        (-grad_y / mag, grad_x / mag)
    } else {
        (0.0, 0.0)
    }
}

fn get_gradient_at_point(
    out_slice: &[u8],
    pixel_states: &[u8],
    width: u32,
    height: u32,
    x: u32,
    y: u32,
) -> (f32, f32) {
    let x_p1 = (x + 1).min(width.saturating_sub(1));
    let x_m1 = x.saturating_sub(1);
    let y_p1 = (y + 1).min(height.saturating_sub(1));
    let y_m1 = y.saturating_sub(1);

    let get_luma = |cx: u32, cy: u32| {
        let idx = ((cy * width + cx) * 3) as usize;
        0.299 * out_slice[idx] as f32
            + 0.587 * out_slice[idx + 1] as f32
            + 0.114 * out_slice[idx + 2] as f32
    };

    let mut grad_x = 0.0;
    if pixel_states[(y * width + x_p1) as usize] == PIXEL_KNOWN
        && pixel_states[(y * width + x_m1) as usize] == PIXEL_KNOWN
    {
        grad_x = (get_luma(x_p1, y) - get_luma(x_m1, y)) / 2.0;
    } else if pixel_states[(y * width + x_p1) as usize] == PIXEL_KNOWN {
        grad_x = get_luma(x_p1, y) - get_luma(x, y);
    } else if pixel_states[(y * width + x_m1) as usize] == PIXEL_KNOWN {
        grad_x = get_luma(x, y) - get_luma(x_m1, y);
    }

    let mut grad_y = 0.0;
    if pixel_states[(y_p1 * width + x) as usize] == PIXEL_KNOWN
        && pixel_states[(y_m1 * width + x) as usize] == PIXEL_KNOWN
    {
        grad_y = (get_luma(x, y_p1) - get_luma(x, y_m1)) / 2.0;
    } else if pixel_states[(y_p1 * width + x) as usize] == PIXEL_KNOWN {
        grad_y = get_luma(x, y_p1) - get_luma(x, y);
    } else if pixel_states[(y_m1 * width + x) as usize] == PIXEL_KNOWN {
        grad_y = get_luma(x, y) - get_luma(x, y_m1);
    }

    (-grad_y, grad_x)
}

fn calculate_priority(
    out_slice: &[u8],
    pixel_states: &[u8],
    confidence: &[f32],
    width: u32,
    height: u32,
    px: u32,
    py: u32,
    patch_radius: u32,
    normal: (f32, f32),
) -> (f32, f32) {
    let r = patch_radius as i32;
    let mut confidence_sum = 0.0;
    let mut count = 0;

    for dy in -r..=r {
        for dx in -r..=r {
            let qx = (px as i32 + dx).clamp(0, (width.saturating_sub(1)) as i32) as u32;
            let qy = (py as i32 + dy).clamp(0, (height.saturating_sub(1)) as i32) as u32;
            let idx = (qy * width + qx) as usize;
            if pixel_states[idx] == PIXEL_KNOWN {
                confidence_sum += confidence[idx];
                count += 1;
            }
        }
    }

    let confidence_term = if count > 0 {
        confidence_sum / count as f32
    } else {
        0.0
    };

    let (normal_x, normal_y) = normal;
    let (isophote_x, isophote_y) =
        get_gradient_at_point(out_slice, pixel_states, width, height, px, py);

    let data_term = (isophote_x * normal_x + isophote_y * normal_y).abs() / 255.0;
    let priority = confidence_term * data_term + 0.001;
    (priority, confidence_term)
}

fn find_best_match_local(
    out_slice: &[u8],
    pixel_states: &[u8],
    num_unknowns: &[u32],
    width: u32,
    height: u32,
    px: u32,
    py: u32,
    patch_radius: u32,
    search_radius: u32,
    max_samples: usize,
    kernel: &[f32],
) -> (u32, u32) {
    let r = patch_radius as i32;
    let sr = search_radius as i32;

    let x_min = (px as i32 - sr).max(r) as u32;
    let x_max = (px as i32 + sr).min((width as i32 - 1 - r).max(0)) as u32;
    let y_min = (py as i32 - sr).max(r) as u32;
    let y_max = (py as i32 + sr).min((height as i32 - 1 - r).max(0)) as u32;

    let mut local_candidates = Vec::new();
    if x_max >= x_min && y_max >= y_min {
        for y in (y_min..=y_max).step_by(2) {
            let row_offset = (y * width) as usize;
            for x in (x_min..=x_max).step_by(2) {
                if num_unknowns[row_offset + x as usize] == 0 {
                    local_candidates.push((x, y));
                }
            }
        }
    }

    if local_candidates.is_empty() {
        return (px, py);
    }

    let mut rng = rand::rng();
    let search_sample: Vec<_> = if local_candidates.len() > max_samples {
        let mut shuffled = local_candidates;
        shuffled.shuffle(&mut rng);
        shuffled.truncate(max_samples);
        shuffled
    } else {
        local_candidates
    };

    struct TargetPixel {
        dx: i32,
        dy: i32,
        color: [f64; 3],
        weight: f64,
    }

    let mut target_pixels = Vec::with_capacity((patch_radius * 2 + 1).pow(2) as usize);
    let mut total_weight = 0.0;
    let diameter = (patch_radius * 2 + 1) as usize;

    for dy in -r..=r {
        for dx in -r..=r {
            let target_x = (px as i32 + dx).clamp(0, (width.saturating_sub(1)) as i32) as u32;
            let target_y = (py as i32 + dy).clamp(0, (height.saturating_sub(1)) as i32) as u32;
            let idx = (target_y * width + target_x) as usize;

            if pixel_states[idx] == PIXEL_KNOWN {
                let p_idx = idx * 3;
                let weight = kernel[((dy + r) as usize * diameter) + (dx + r) as usize] as f64;
                target_pixels.push(TargetPixel {
                    dx,
                    dy,
                    color: [
                        out_slice[p_idx] as f64,
                        out_slice[p_idx + 1] as f64,
                        out_slice[p_idx + 2] as f64,
                    ],
                    weight,
                });
                total_weight += weight;
            }
        }
    }

    let best_match = search_sample
        .par_iter()
        .map(|&(cx, cy)| {
            let mut ssd = 0.0;
            for tp in &target_pixels {
                let source_x = (cx as i32 + tp.dx) as u32;
                let source_y = (cy as i32 + tp.dy) as u32;
                let idx = ((source_y * width + source_x) * 3) as usize;

                let diff0 = tp.color[0] - out_slice[idx] as f64;
                let diff1 = tp.color[1] - out_slice[idx + 1] as f64;
                let diff2 = tp.color[2] - out_slice[idx + 2] as f64;
                ssd += (diff0 * diff0 + diff1 * diff1 + diff2 * diff2) * tp.weight;
            }

            let score =
                if total_weight == 0.0 {
                    f64::MAX
                } else {
                    ssd / total_weight
                } + ((px as i64 - cx as i64).pow(2) + (py as i64 - cy as i64).pow(2)) as f64 * 0.05;

            (FloatOrdF64(score), cx, cy)
        })
        .min_by(|a, b| a.0.cmp(&b.0));

    best_match.map(|v| (v.1, v.2)).unwrap_or((px, py))
}

pub fn perform_fast_inpaint(
    source_image: &DynamicImage,
    mask: &GrayImage,
    patch_radius: u32,
) -> Result<RgbaImage, String> {
    if patch_radius == 0 {
        return Err("Patch radius must be greater than 0.".to_string());
    }

    let (width, height) = source_image.dimensions();
    if width <= 2 * patch_radius + 1 || height <= 2 * patch_radius + 1 {
        return Err(format!(
            "Image bounds ({}x{}) are too small for a patch radius of {}.",
            width, height, patch_radius
        ));
    }

    let source_rgb = source_image.to_rgb8();
    let inpainted_rgb = inpaint_criminisi(&source_rgb, mask, patch_radius);

    let mut final_image = source_image.to_rgba8();
    let final_slice = final_image.as_mut();
    let inpaint_slice = inpainted_rgb.as_raw();

    for i in 0..(width * height) as usize {
        final_slice[i * 4] = inpaint_slice[i * 3];
        final_slice[i * 4 + 1] = inpaint_slice[i * 3 + 1];
        final_slice[i * 4 + 2] = inpaint_slice[i * 3 + 2];
    }

    Ok(final_image)
}
