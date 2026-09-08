//! Audited eDifFIQA(T) observation. A quality regressor is not a blur verdict.

use std::path::Path;
use std::sync::Mutex;

use anyhow::{Result, anyhow};
use image::{DynamicImage, RgbImage};
use ndarray::Array4;
use once_cell::sync::OnceCell;
use ort::{session::Session, value::Tensor};

use super::super::face_identity::align_face;
use super::super::models::{
    gpu_session_with_optimization, validate_session_contract, verify_model,
};

const FILENAME: &str = "ediffiqa_tiny_jun2024_coreml_qraw.onnx";
const SHA256: &str = "e25bc043793a0bcda0384f88eb8a04a01d4c6c826ec4a86dc076eba33b863557";
static MODEL: OnceCell<Mutex<Session>> = OnceCell::new();

pub(super) fn is_available() -> bool {
    MODEL.get().is_some()
}

pub(crate) fn preflight(model_dir: &Path) -> Result<()> {
    MODEL.get_or_try_init(|| {
        let path = model_dir.join(FILENAME);
        verify_model(&path, SHA256)?;
        let mut session = gpu_session_with_optimization(&path, Some(("batch_size", 1)), None)?;
        validate_session_contract(&session, "input", &[1, 3, 112, 112], &["output"])?;
        // Publish only a session that has actually completed strict-provider inference.
        run(&mut session, Array4::zeros((1, 3, 112, 112)))?;
        Ok::<_, anyhow::Error>(Mutex::new(session))
    })?;
    Ok(())
}

pub(super) fn observe(image: &DynamicImage, landmarks: &[(f32, f32); 5]) -> Result<f32> {
    let session = MODEL
        .get()
        .ok_or_else(|| anyhow!("face_quality_model_unavailable"))?;
    let aligned = align_face(image, landmarks)?;
    let input = prepare_input(&aligned)?;
    let mut session = session
        .lock()
        .map_err(|_| anyhow!("face quality session lock is poisoned"))?;
    run(&mut session, input)
}

fn prepare_input(aligned: &RgbImage) -> Result<Array4<f32>> {
    if aligned.dimensions() != (112, 112) {
        return Err(anyhow!("face quality alignment must be 112x112"));
    }
    let mut input = Array4::zeros((1, 3, 112, 112));
    for (x, y, pixel) in aligned.enumerate_pixels() {
        for channel in 0..3 {
            // Official eDifFIQA consumes RGB NCHW, normalized to [-1, 1].
            input[[0, channel, y as usize, x as usize]] = f32::from(pixel[channel]) / 127.5 - 1.0;
        }
    }
    Ok(input)
}

fn run(session: &mut Session, input: Array4<f32>) -> Result<f32> {
    let outputs = session.run(ort::inputs!["input" => Tensor::from_array(input)?])?;
    let output = outputs["output"].try_extract_array::<f32>()?;
    if output.shape() != [1, 1] || !output[[0, 0]].is_finite() {
        return Err(anyhow!(
            "face quality output must be one finite [1,1] scalar"
        ));
    }
    // This is an unbounded regression score, not a calibrated probability.
    Ok(output[[0, 0]])
}

#[cfg(test)]
pub(super) fn official_reference(image: &DynamicImage, landmarks: &[(f32, f32); 5]) -> Result<f32> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/features/smart_culling/model_assets/ediffiqa_tiny_jun2024.onnx");
    verify_model(
        &path,
        "9426c899cc0f01665240cb7d9e7f98e18e24e456c178326c771a43da289bfc6a",
    )?;
    // Explicit offline reference only. Production still forbids ORT CPU fallback.
    let mut session = Session::builder()?
        .with_execution_providers([
            ort::execution_providers::CPUExecutionProvider::default().build()
        ])?
        .with_dimension_override("batch_size", 1)?
        .commit_from_file(path)?;
    run(&mut session, prepare_input(&align_face(image, landmarks)?)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgb;

    #[test]
    fn input_preserves_rgb_channels_and_official_normalization() {
        let input = prepare_input(&RgbImage::from_pixel(112, 112, Rgb([255, 0, 128]))).unwrap();
        assert_eq!(input[[0, 0, 0, 0]], 1.0);
        assert_eq!(input[[0, 1, 0, 0]], -1.0);
        assert!((input[[0, 2, 0, 0]] - 1.0 / 255.0).abs() < 1e-6);
        assert!(prepare_input(&RgbImage::new(64, 64)).is_err());
    }

    #[test]
    #[ignore = "explicit local strict GPU model verification"]
    fn ediffiqa_runs_with_the_project_gpu_runtime() {
        const CHILD: &str = "QRAW_FACE_QUALITY_GPU_TEST_CHILD";
        const TEST: &str = "features::smart_culling::portrait_quality::model::tests::ediffiqa_runs_with_the_project_gpu_runtime";
        if std::env::var_os(CHILD).is_some() {
            preflight(
                &Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("src/features/smart_culling/model_assets"),
            )
            .unwrap();
            let image = DynamicImage::ImageRgb8(RgbImage::from_fn(112, 112, |x, y| {
                Rgb([(x * 2) as u8, (y * 2) as u8, ((x + y) % 256) as u8])
            }));
            // Values from the official CPU graph, before seeing Core ML results.
            for (value, expected) in [(-1.0, 0.15504666), (0.0, 0.161402), (1.0, 0.10752232)] {
                let actual = run(
                    &mut MODEL.get().unwrap().lock().unwrap(),
                    Array4::from_elem((1, 3, 112, 112), value),
                )
                .unwrap();
                assert!(
                    (actual - expected).abs() < 1e-3,
                    "GPU/official parity: {actual} vs {expected}"
                );
            }
            let actual = run(
                &mut MODEL.get().unwrap().lock().unwrap(),
                prepare_input(&image.to_rgb8()).unwrap(),
            )
            .unwrap();
            assert!(
                (actual - 0.29172632).abs() < 1e-3,
                "GPU/official gradient parity: {actual}"
            );
            let landmarks = [
                (38.2946, 51.6963),
                (73.5318, 51.5014),
                (56.0252, 71.7366),
                (41.5493, 92.3655),
                (70.7299, 92.2041),
            ];
            let score = observe(&image, &landmarks).unwrap();
            assert!(score.is_finite());
            println!("QRAW_EDIFFIQA_GPU_PASS score={score}");
            #[cfg(target_os = "macos")]
            {
                use std::io::Write;
                std::io::stdout().flush().unwrap();
                std::io::stderr().flush().unwrap();
                // SAFETY: isolated test child after all assertions, as in the
                // existing face-motion gate. Bypass ORT's static logger teardown.
                unsafe { libc::_exit(0) }
            }
            #[cfg(not(target_os = "macos"))]
            std::process::exit(0);
        }
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--ignored", "--nocapture"])
            .env(CHILD, "1")
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            output.status.success() && stdout.contains("QRAW_EDIFFIQA_GPU_PASS"),
            "{stdout}\n{stderr}"
        );
        print!("{stdout}");
    }
}
