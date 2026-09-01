//! Isolated HSEmotion expression-quality proof of concept.
//!
//! The two facial-expression classifiers are macOS Debug calibration inputs
//! only. Their raw outputs do not name a preferred emotion and cannot affect
//! the separately frozen eye-state contract.

use std::path::Path;
use std::sync::Mutex;

use anyhow::{Result, anyhow};
use image::{DynamicImage, GenericImageView, imageops::FilterType};
use ndarray::Array4;
use once_cell::sync::OnceCell;
use ort::session::Session;
use ort::value::Tensor;

use super::models::{gpu_session_with_optimization, validate_session_contract, verify_model};

const MTL_MODEL_FILENAME: &str = "expression_hsemotion_enet_b0_8_va_mtl_coreml_qraw_poc.onnx";
const VGAF_MODEL_FILENAME: &str = "expression_hsemotion_enet_b0_8_best_vgaf_coreml_qraw_poc.onnx";
const MTL_MODEL_SHA256: &str = "b11cd798683082eee26c1cc0871aeb5ee545bf7d4330db0b5de3091b00d0eed7";
const VGAF_MODEL_SHA256: &str = "52383e3d3757286c0ced73ee0aeb50839111b775c8235cac1e43bb6ff16c773e";
const MODEL_BATCH_DIMENSION: (&str, i64) = ("batch_size", 1);
const MODEL_INPUT_SIZE: usize = 224;
const MODEL_INPUT_SHAPE: [i64; 4] = [1, 3, 224, 224];
const MTL_OUTPUT_COUNT: usize = 10;
const VGAF_OUTPUT_COUNT: usize = 8;
const FACE_CROP_SCALE: f32 = 1.2;
const CHANNEL_MEANS: [f32; 3] = [0.485, 0.456, 0.406];
const CHANNEL_STDS: [f32; 3] = [0.229, 0.224, 0.225];

struct ExpressionQualityModels {
    mtl: Mutex<Session>,
    vgaf: Mutex<Session>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ExpressionQualityModelOutputs {
    pub(super) mtl: [f32; MTL_OUTPUT_COUNT],
    pub(super) vgaf: [f32; VGAF_OUTPUT_COUNT],
}

static CALIBRATION_MODELS: OnceCell<ExpressionQualityModels> = OnceCell::new();

#[cfg(test)]
pub(in crate::features::smart_culling) fn preflight_calibration_models() -> Result<()> {
    preflight_calibration_models_from(&super::face_motion_poc::model_asset_dir())
}

pub(in crate::features::smart_culling) fn preflight_calibration_models_from(
    model_dir: &Path,
) -> Result<()> {
    let models = CALIBRATION_MODELS.get_or_try_init(|| load_models(model_dir))?;
    let image = DynamicImage::new_rgb8(MODEL_INPUT_SIZE as u32, MODEL_INPUT_SIZE as u32);
    infer(
        &image,
        [0.0, 0.0, MODEL_INPUT_SIZE as f32, MODEL_INPUT_SIZE as f32],
        models,
    )?;
    Ok(())
}

pub(super) fn infer_calibration_face(
    image: &DynamicImage,
    bbox: [f32; 4],
) -> Result<ExpressionQualityModelOutputs> {
    infer(image, bbox, models()?)
}

fn models() -> Result<&'static ExpressionQualityModels> {
    CALIBRATION_MODELS.get_or_try_init(|| load_models(&super::face_motion_poc::model_asset_dir()))
}

fn load_models(model_dir: &Path) -> Result<ExpressionQualityModels> {
    let mtl_path = model_dir.join(MTL_MODEL_FILENAME);
    let vgaf_path = model_dir.join(VGAF_MODEL_FILENAME);
    verify_model(&mtl_path, MTL_MODEL_SHA256)?;
    verify_model(&vgaf_path, VGAF_MODEL_SHA256)?;

    let mtl = gpu_session_with_optimization(&mtl_path, Some(MODEL_BATCH_DIMENSION), None)?;
    validate_session_contract(&mtl, "input", &MODEL_INPUT_SHAPE, &["output"])?;
    let vgaf = gpu_session_with_optimization(&vgaf_path, Some(MODEL_BATCH_DIMENSION), None)?;
    validate_session_contract(&vgaf, "input", &MODEL_INPUT_SHAPE, &["output"])?;

    Ok(ExpressionQualityModels {
        mtl: Mutex::new(mtl),
        vgaf: Mutex::new(vgaf),
    })
}

fn infer(
    image: &DynamicImage,
    bbox: [f32; 4],
    models: &ExpressionQualityModels,
) -> Result<ExpressionQualityModelOutputs> {
    let input = prepare_input(image, bbox)?;
    Ok(ExpressionQualityModelOutputs {
        mtl: run_model::<MTL_OUTPUT_COUNT>(&input, &models.mtl, "HSEmotion MTL")?,
        vgaf: run_model::<VGAF_OUTPUT_COUNT>(&input, &models.vgaf, "HSEmotion VGAF")?,
    })
}

fn run_model<const OUTPUT_COUNT: usize>(
    input: &Array4<f32>,
    session: &Mutex<Session>,
    model_name: &str,
) -> Result<[f32; OUTPUT_COUNT]> {
    let tensor = Tensor::from_array(input.clone().into_dyn())?;
    let mut session = session
        .lock()
        .map_err(|_| anyhow!("{model_name} session lock is poisoned"))?;
    let outputs = session.run(ort::inputs!["input" => tensor])?;
    let output = outputs["output"].try_extract_array::<f32>()?.to_owned();
    decode_output::<OUTPUT_COUNT>(
        output
            .as_slice()
            .ok_or_else(|| anyhow!("{model_name} output must be contiguous"))?,
        model_name,
    )
}

fn prepare_input(image: &DynamicImage, bbox: [f32; 4]) -> Result<Array4<f32>> {
    if bbox.iter().any(|value| !value.is_finite()) || bbox[2] <= 0.0 || bbox[3] <= 0.0 {
        return Err(anyhow!("HSEmotion face box is invalid"));
    }

    let (image_width, image_height) = image.dimensions();
    let center_x = bbox[0] + bbox[2] * 0.5;
    let center_y = bbox[1] + bbox[3] * 0.5;
    let side = bbox[2].max(bbox[3]) * FACE_CROP_SCALE;
    let left = (center_x - side * 0.5).floor().max(0.0) as u32;
    let top = (center_y - side * 0.5).floor().max(0.0) as u32;
    let right = (center_x + side * 0.5).ceil().max(0.0) as u32;
    let bottom = (center_y + side * 0.5).ceil().max(0.0) as u32;
    let right = right.min(image_width);
    let bottom = bottom.min(image_height);
    if right <= left || bottom <= top {
        return Err(anyhow!("HSEmotion face box is outside the image"));
    }

    let crop = image.crop_imm(left, top, right - left, bottom - top);
    let resized = crop
        .resize_exact(
            MODEL_INPUT_SIZE as u32,
            MODEL_INPUT_SIZE as u32,
            FilterType::Triangle,
        )
        .to_rgb8();
    let mut input = Array4::<f32>::zeros((1, 3, MODEL_INPUT_SIZE, MODEL_INPUT_SIZE));
    for (x, y, pixel) in resized.enumerate_pixels() {
        let x = x as usize;
        let y = y as usize;
        // The official HSEmotion ONNX wrapper accepts OpenCV BGR input and
        // applies these channel constants in that order. Preserve that audited
        // contract instead of silently converting it to a different model.
        for (channel, value) in [pixel[2], pixel[1], pixel[0]].into_iter().enumerate() {
            input[[0, channel, y, x]] =
                (f32::from(value) / 255.0 - CHANNEL_MEANS[channel]) / CHANNEL_STDS[channel];
        }
    }
    Ok(input)
}

fn decode_output<const OUTPUT_COUNT: usize>(
    raw: &[f32],
    model_name: &str,
) -> Result<[f32; OUTPUT_COUNT]> {
    if raw.len() != OUTPUT_COUNT {
        return Err(anyhow!("{model_name} output contract mismatch"));
    }
    if raw.iter().any(|value| !value.is_finite()) {
        return Err(anyhow!("{model_name} output contains a non-finite value"));
    }
    raw.try_into()
        .map_err(|_| anyhow!("{model_name} output contract mismatch"))
}

#[cfg(test)]
mod tests {
    use image::{Rgb, RgbImage};

    use super::*;

    #[test]
    fn model_assets_match_the_frozen_hashes() {
        let model_dir = super::super::face_motion_poc::model_asset_dir();
        verify_model(&model_dir.join(MTL_MODEL_FILENAME), MTL_MODEL_SHA256).unwrap();
        verify_model(&model_dir.join(VGAF_MODEL_FILENAME), VGAF_MODEL_SHA256).unwrap();
    }

    #[test]
    fn preprocessing_preserves_the_official_bgr_channel_contract() {
        let image = DynamicImage::ImageRgb8(RgbImage::from_pixel(
            MODEL_INPUT_SIZE as u32,
            MODEL_INPUT_SIZE as u32,
            Rgb([255, 128, 0]),
        ));

        let input = prepare_input(
            &image,
            [0.0, 0.0, MODEL_INPUT_SIZE as f32, MODEL_INPUT_SIZE as f32],
        )
        .unwrap();

        assert!((input[[0, 0, 0, 0]] - (0.0 - CHANNEL_MEANS[0]) / CHANNEL_STDS[0]).abs() < 1e-6);
        assert!(
            (input[[0, 1, 0, 0]] - (128.0 / 255.0 - CHANNEL_MEANS[1]) / CHANNEL_STDS[1]).abs()
                < 1e-6
        );
        assert!((input[[0, 2, 0, 0]] - (1.0 - CHANNEL_MEANS[2]) / CHANNEL_STDS[2]).abs() < 1e-6);
    }

    #[test]
    fn invalid_or_outside_face_boxes_fail_loudly() {
        let image = DynamicImage::new_rgb8(32, 32);

        assert!(prepare_input(&image, [0.0, 0.0, 0.0, 1.0]).is_err());
        assert!(prepare_input(&image, [100.0, 100.0, 4.0, 4.0]).is_err());
    }

    #[test]
    fn output_contract_rejects_wrong_counts_and_non_finite_values() {
        assert!(decode_output::<8>(&[0.0; 7], "test").is_err());
        assert!(decode_output::<8>(&[f32::NAN; 8], "test").is_err());
        assert_eq!(decode_output::<8>(&[0.0; 8], "test").unwrap(), [0.0; 8]);
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[test]
    #[ignore = "explicit hardware POC gate; not part of production analysis"]
    fn strict_hardware_sessions_smoke_test_both_models() {
        const CHILD_ENV: &str = "QRAW_EXPRESSION_QUALITY_POC_CHILD";
        const TEST_NAME: &str = "features::smart_culling::expression_quality_poc::tests::strict_hardware_sessions_smoke_test_both_models";
        const PASS_MARKER: &str = "QRAW_EXPRESSION_QUALITY_POC_HARDWARE_PASS";

        if std::env::var_os(CHILD_ENV).is_none() {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", TEST_NAME, "--ignored", "--nocapture"])
                .env(CHILD_ENV, "1")
                .output()
                .unwrap();
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                output.status.success() && stdout.contains(PASS_MARKER),
                "isolated hardware POC failed\nstdout:\n{stdout}\nstderr:\n{stderr}"
            );
            return;
        }

        preflight_calibration_models().unwrap();
        println!("{PASS_MARKER}");

        #[cfg(target_os = "macos")]
        {
            use std::io::Write;

            std::io::stdout().flush().unwrap();
            std::io::stderr().flush().unwrap();
            // SAFETY: identical isolated-test workaround to the existing
            // face-motion hardware gate; application code never uses `_exit`.
            unsafe { libc::_exit(0) }
        }
        #[cfg(target_os = "windows")]
        std::process::exit(0);
    }
}
