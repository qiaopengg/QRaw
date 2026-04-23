use image::DynamicImage;
use ndarray::Array4;
use ort::value::Tensor;

use crate::ai_processing::ClipModels;

/// Compute image quality score using CLIP joint model
/// Compares image against positive/negative quality prompts
/// Returns 0.0~1.0 (higher = better quality)
pub fn compute_clip_quality(image: &DynamicImage, clip: &ClipModels) -> Option<f64> {
    let prompts = vec![
        "a sharp well-focused high quality photograph".to_string(),
        "a blurry low quality poorly composed photograph".to_string(),
    ];

    // Tokenize prompts
    let encodings = match clip.tokenizer.encode_batch(prompts.clone(), true) {
        Ok(v) => v,
        Err(_) => return None,
    };
    if encodings.is_empty() {
        return None;
    }

    let max_len = encodings
        .iter()
        .map(|e| e.get_ids().len())
        .max()
        .unwrap_or(1)
        .max(1);
    let n = prompts.len();

    let mut ids_data = Vec::with_capacity(n * max_len);
    let mut mask_data = Vec::with_capacity(n * max_len);
    for enc in &encodings {
        let mut ids: Vec<i64> = enc.get_ids().iter().map(|&i| i as i64).collect();
        let mut mask: Vec<i64> = enc.get_attention_mask().iter().map(|&m| m as i64).collect();
        ids.resize(max_len, 0);
        mask.resize(max_len, 0);
        ids_data.extend_from_slice(&ids);
        mask_data.extend_from_slice(&mask);
    }

    // Preprocess image (same as existing CLIP usage in culling.rs)
    let image_input = preprocess_clip_image(image);

    let ids_array = ndarray::Array::from_shape_vec((n, max_len), ids_data)
        .ok()?
        .into_dyn();
    let mask_array = ndarray::Array::from_shape_vec((n, max_len), mask_data)
        .ok()?
        .into_dyn();
    let image_dyn = image_input.into_dyn();

    let image_val = Tensor::from_array(image_dyn.as_standard_layout().into_owned()).ok()?;
    let ids_val = Tensor::from_array(ids_array.as_standard_layout().into_owned()).ok()?;
    let mask_val = Tensor::from_array(mask_array.as_standard_layout().into_owned()).ok()?;

    let logits = {
        let mut sess = clip.model.lock().unwrap();
        let outputs = sess.run(ort::inputs![ids_val, image_val, mask_val]).ok()?;
        outputs[0].try_extract_array::<f32>().ok()?.to_owned()
    };

    let flat = logits.into_raw_vec_and_offset().0;
    if flat.len() < 2 {
        return None;
    }

    // Softmax to get probabilities
    let max_val = flat.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let exps: Vec<f32> = flat.iter().map(|&x| (x - max_val).exp()).collect();
    let sum: f32 = exps.iter().sum();
    if sum <= 0.0 {
        return None;
    }
    let probs: Vec<f32> = exps.iter().map(|&e| e / sum).collect();

    // probs[0] = match with "high quality", probs[1] = match with "low quality"
    Some(probs[0] as f64)
}

/// Preprocess image for CLIP (224x224, normalized)
fn preprocess_clip_image(image: &DynamicImage) -> Array4<f32> {
    let input_size: u32 = 224;
    let resized = image.resize_to_fill(
        input_size,
        input_size,
        image::imageops::FilterType::Triangle,
    );
    let rgb = resized.to_rgb8();
    let mean = [0.48145466f32, 0.4578275f32, 0.40821073f32];
    let std = [0.26862954f32, 0.2613026f32, 0.2757771f32];
    let mut arr = Array4::<f32>::zeros((1, 3, input_size as usize, input_size as usize));
    for (x, y, p) in rgb.enumerate_pixels() {
        arr[[0, 0, y as usize, x as usize]] = (p[0] as f32 / 255.0 - mean[0]) / std[0];
        arr[[0, 1, y as usize, x as usize]] = (p[1] as f32 / 255.0 - mean[1]) / std[1];
        arr[[0, 2, y as usize, x as usize]] = (p[2] as f32 / 255.0 - mean[2]) / std[2];
    }
    arr
}
