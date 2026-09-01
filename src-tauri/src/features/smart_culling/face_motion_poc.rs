//! Isolated FaceMeshV2 + BlendshapeV2 proof of concept.
//!
//! The models remain outside bundled resources. Tests can inspect raw evidence,
//! while macOS debug builds may use the conservative calibration adapter. Release
//! builds remain unchanged until dual-platform and independent-data gates pass.
//!
//! Eye-state decisions are a separately versioned, frozen contract. Expression
//! work may reuse the raw face-motion evidence, but it must not replace the
//! pinned model inputs or mutate the resulting eye assessment.

mod decision;
#[cfg(test)]
mod eval;
mod evidence;
#[cfg(test)]
mod expression_isolation_tests;
mod eye_policy;
#[cfg(test)]
mod paired_eval;
mod roi;

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{Result, anyhow};
use image::{DynamicImage, RgbImage};
use ndarray::{Array3, Array4};
use once_cell::sync::OnceCell;
use ort::session::{Session, builder::GraphOptimizationLevel};
use ort::value::Tensor;

use super::expression::{ExpressionDescriptor, ExpressionEvidence};
use super::face_models::DetectedFace;
use super::models::{gpu_session_with_optimization, validate_session_contract, verify_model};
use super::types::{EyeDisposition, EyeResult};

pub(in crate::features::smart_culling) const EYE_POLICY_VERSION: &str =
    eye_policy::EYE_POLICY_VERSION;
pub(in crate::features::smart_culling) const EYE_MODEL_CONTRACT_VERSION: &str =
    "qraw-eye-model-contract-1.0";

const FACE_MESH_MODEL_FILENAME: &str = "face_landmarks_detector_v2_qraw_poc.onnx";
const FACE_BLENDSHAPES_MODEL_FILENAME: &str = "face_blendshapes_v2_qraw_poc.onnx";
const FACE_MESH_SHA256: &str = "b047d95fab6702c327175e7b77eea71ffd2b2ef0110c7466eee9b6e2ae87b552";
const FACE_BLENDSHAPES_SHA256: &str =
    "b90ed4146dfdb43745c5988b1d411ed026d4b5e2ba9c1d7c271954fd1f5cb60e";
const FACE_MESH_INPUT_SIZE: usize = 256;
const FACE_MESH_LANDMARK_COUNT: usize = 478;

// Exact landmark selection from MediaPipe's FaceBlendshapesGraph.
const BLENDSHAPE_LANDMARK_INDICES: [usize; 146] = [
    0, 1, 4, 5, 6, 7, 8, 10, 13, 14, 17, 21, 33, 37, 39, 40, 46, 52, 53, 54, 55, 58, 61, 63, 65,
    66, 67, 70, 78, 80, 81, 82, 84, 87, 88, 91, 93, 95, 103, 105, 107, 109, 127, 132, 133, 136,
    144, 145, 146, 148, 149, 150, 152, 153, 154, 155, 157, 158, 159, 160, 161, 162, 163, 168, 172,
    173, 176, 178, 181, 185, 191, 195, 197, 234, 246, 249, 251, 263, 267, 269, 270, 276, 282, 283,
    284, 285, 288, 291, 293, 295, 296, 297, 300, 308, 310, 311, 312, 314, 317, 318, 321, 323, 324,
    332, 334, 336, 338, 356, 361, 362, 365, 373, 374, 375, 377, 378, 379, 380, 381, 382, 384, 385,
    386, 387, 388, 389, 390, 397, 398, 400, 402, 405, 409, 415, 454, 466, 468, 469, 470, 471, 472,
    473, 474, 475, 476, 477,
];

// Exact output order from MediaPipe's FaceBlendshapesGraph.
const BLENDSHAPE_NAMES: [&str; 52] = [
    "_neutral",
    "browDownLeft",
    "browDownRight",
    "browInnerUp",
    "browOuterUpLeft",
    "browOuterUpRight",
    "cheekPuff",
    "cheekSquintLeft",
    "cheekSquintRight",
    "eyeBlinkLeft",
    "eyeBlinkRight",
    "eyeLookDownLeft",
    "eyeLookDownRight",
    "eyeLookInLeft",
    "eyeLookInRight",
    "eyeLookOutLeft",
    "eyeLookOutRight",
    "eyeLookUpLeft",
    "eyeLookUpRight",
    "eyeSquintLeft",
    "eyeSquintRight",
    "eyeWideLeft",
    "eyeWideRight",
    "jawForward",
    "jawLeft",
    "jawOpen",
    "jawRight",
    "mouthClose",
    "mouthDimpleLeft",
    "mouthDimpleRight",
    "mouthFrownLeft",
    "mouthFrownRight",
    "mouthFunnel",
    "mouthLeft",
    "mouthLowerDownLeft",
    "mouthLowerDownRight",
    "mouthPressLeft",
    "mouthPressRight",
    "mouthPucker",
    "mouthRight",
    "mouthRollLower",
    "mouthRollUpper",
    "mouthShrugLower",
    "mouthShrugUpper",
    "mouthSmileLeft",
    "mouthSmileRight",
    "mouthStretchLeft",
    "mouthStretchRight",
    "mouthUpperUpLeft",
    "mouthUpperUpRight",
    "noseSneerLeft",
    "noseSneerRight",
];

#[derive(Debug)]
struct FaceMeshPocOutput {
    /// Landmarks normalized to the already-prepared 256x256 canonical ROI.
    landmarks: Vec<[f32; 3]>,
    face_presence: f32,
    tongue_out: f32,
}

#[derive(Debug)]
struct BlendshapeScore {
    name: &'static str,
    score: f32,
}

struct FaceMotionPocModels {
    face_mesh: Mutex<Session>,
    face_blendshapes: Mutex<Session>,
}

static CALIBRATION_MODELS: OnceCell<FaceMotionPocModels> = OnceCell::new();

pub(in crate::features::smart_culling) struct FaceMotionAnalysis {
    eye: decision::EyeAssessment,
    expression: ExpressionEvidence,
    expression_descriptor: Option<ExpressionDescriptor>,
}

impl FaceMotionAnalysis {
    pub(in crate::features::smart_culling) fn into_legacy_parts(
        self,
    ) -> (
        EyeResult,
        EyeResult,
        EyeDisposition,
        ExpressionEvidence,
        Option<ExpressionDescriptor>,
    ) {
        let (left_eye, right_eye, disposition) = self.eye.into_legacy_parts();
        (
            left_eye,
            right_eye,
            disposition,
            self.expression,
            self.expression_descriptor,
        )
    }
}

fn extract_calibration_evidence(
    image: &DynamicImage,
    detection: &DetectedFace,
) -> Result<evidence::FaceMotionEvidenceDump> {
    let models = CALIBRATION_MODELS.get_or_try_init(|| load_models(&model_asset_dir()))?;
    evidence::extract_face_motion_evidence(image, detection, models)
}

pub(in crate::features::smart_culling) fn preflight_calibration_models_from(
    model_dir: &Path,
) -> Result<()> {
    let models = CALIBRATION_MODELS.get_or_try_init(|| load_models(model_dir))?;
    let roi = RgbImage::new(FACE_MESH_INPUT_SIZE as u32, FACE_MESH_INPUT_SIZE as u32);
    let mesh = run_face_mesh(&roi, &models.face_mesh)?;
    let image_pixel_landmarks = mesh
        .landmarks
        .iter()
        .map(|point| {
            [
                point[0] * FACE_MESH_INPUT_SIZE as f32,
                point[1] * FACE_MESH_INPUT_SIZE as f32,
            ]
        })
        .collect::<Vec<_>>();
    let blendshapes = run_blendshapes(&image_pixel_landmarks, &models.face_blendshapes)?;
    if blendshapes.len() != BLENDSHAPE_NAMES.len() {
        return Err(anyhow!("BlendshapeV2 preflight output count mismatch"));
    }
    Ok(())
}

pub(super) fn analyze_calibration_face(
    image: &DynamicImage,
    detection: &DetectedFace,
    pose_suppresses_eye_state: bool,
) -> Result<FaceMotionAnalysis> {
    let evidence = extract_calibration_evidence(image, detection)?;
    // Complete the frozen eye assessment before invoking either expression
    // model. A later expression failure becomes unknown evidence and cannot
    // replace, fail, or mutate this eye result.
    let eye = decision::assess(&evidence, pose_suppresses_eye_state);
    let expression_descriptor = evidence.expression_descriptor().ok();
    let expression = match expression_descriptor.as_ref() {
        None => ExpressionEvidence::unavailable("expression_frame_evidence_invalid"),
        Some(descriptor) if !descriptor.is_reliable() => {
            ExpressionEvidence::unavailable("expression_frame_evidence_unreliable")
        }
        Some(descriptor) => {
            match super::expression_quality_poc::infer_calibration_face(image, detection.bbox) {
                Ok(model_outputs) => {
                    ExpressionEvidence::from_single_frame(descriptor, &model_outputs)
                }
                Err(error) => {
                    log::warn!("Expression-quality calibration inference failed: {error}");
                    ExpressionEvidence::unavailable("expression_quality_model_unavailable")
                }
            }
        }
    };

    Ok(FaceMotionAnalysis {
        eye,
        expression,
        expression_descriptor,
    })
}

pub(super) fn model_asset_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/features/smart_culling/model_assets")
}

fn load_models(model_dir: &Path) -> Result<FaceMotionPocModels> {
    let face_mesh_path = model_dir.join(FACE_MESH_MODEL_FILENAME);
    let face_blendshapes_path = model_dir.join(FACE_BLENDSHAPES_MODEL_FILENAME);
    verify_model(&face_mesh_path, FACE_MESH_SHA256)?;
    verify_model(&face_blendshapes_path, FACE_BLENDSHAPES_SHA256)?;

    let face_mesh = gpu_session_with_optimization(&face_mesh_path, None, None)?;
    validate_session_contract(
        &face_mesh,
        "input_12",
        &[1, 3, 256, 256],
        &["Identity", "Identity_1", "Identity_2"],
    )?;

    // ORT's level-1+ transpose optimizer rewrites the audited channel-axis
    // Concat back to an axis unsupported by Core ML. Disabling graph rewrites
    // preserves the verified graph; CPU fallback remains strictly disabled.
    let face_blendshapes = gpu_session_with_optimization(
        &face_blendshapes_path,
        None,
        Some(GraphOptimizationLevel::Disable),
    )?;
    validate_session_contract(&face_blendshapes, "input_points", &[1, 146, 2], &["output"])?;

    Ok(FaceMotionPocModels {
        face_mesh: Mutex::new(face_mesh),
        face_blendshapes: Mutex::new(face_blendshapes),
    })
}

fn run_face_mesh(canonical_roi: &RgbImage, session: &Mutex<Session>) -> Result<FaceMeshPocOutput> {
    let input = prepare_face_mesh_input(canonical_roi)?;
    let tensor = Tensor::from_array(input.into_dyn())?;
    let mut session = session
        .lock()
        .map_err(|_| anyhow!("FaceMeshV2 POC session lock is poisoned"))?;
    let outputs = session.run(ort::inputs!["input_12" => tensor])?;

    let landmarks = outputs["Identity"].try_extract_array::<f32>()?.to_owned();
    let presence = outputs["Identity_1"].try_extract_array::<f32>()?.to_owned();
    let tongue_out = outputs["Identity_2"].try_extract_array::<f32>()?.to_owned();
    decode_face_mesh_output(
        landmarks
            .as_slice()
            .ok_or_else(|| anyhow!("FaceMeshV2 landmark output must be contiguous"))?,
        presence
            .as_slice()
            .ok_or_else(|| anyhow!("FaceMeshV2 presence output must be contiguous"))?,
        tongue_out
            .as_slice()
            .ok_or_else(|| anyhow!("FaceMeshV2 tongue output must be contiguous"))?,
    )
}

fn prepare_face_mesh_input(canonical_roi: &RgbImage) -> Result<Array4<f32>> {
    if canonical_roi.dimensions() != (FACE_MESH_INPUT_SIZE as u32, FACE_MESH_INPUT_SIZE as u32) {
        return Err(anyhow!(
            "FaceMeshV2 POC requires an audited 256x256 canonical ROI; resizing a raw face box here would hide an unvalidated transform"
        ));
    }

    let mut input = Array4::<f32>::zeros((1, 3, FACE_MESH_INPUT_SIZE, FACE_MESH_INPUT_SIZE));
    for (x, y, pixel) in canonical_roi.enumerate_pixels() {
        let x = x as usize;
        let y = y as usize;
        input[[0, 0, y, x]] = pixel[0] as f32 / 255.0;
        input[[0, 1, y, x]] = pixel[1] as f32 / 255.0;
        input[[0, 2, y, x]] = pixel[2] as f32 / 255.0;
    }
    Ok(input)
}

fn decode_face_mesh_output(
    raw_landmarks: &[f32],
    raw_presence: &[f32],
    raw_tongue_out: &[f32],
) -> Result<FaceMeshPocOutput> {
    if raw_landmarks.len() != FACE_MESH_LANDMARK_COUNT * 3
        || raw_presence.len() != 1
        || raw_tongue_out.len() != 1
    {
        return Err(anyhow!("FaceMeshV2 POC output contract mismatch"));
    }
    if raw_landmarks.iter().any(|value| !value.is_finite())
        || !raw_presence[0].is_finite()
        || !raw_tongue_out[0].is_finite()
    {
        return Err(anyhow!("FaceMeshV2 POC output contains non-finite values"));
    }

    let scale = FACE_MESH_INPUT_SIZE as f32;
    let landmarks = raw_landmarks
        .chunks_exact(3)
        .map(|point| [point[0] / scale, point[1] / scale, point[2] / scale])
        .collect();
    let tongue_out = raw_tongue_out[0];
    if !(0.0..=1.0).contains(&tongue_out) {
        return Err(anyhow!("FaceMeshV2 tongue_out score is outside [0, 1]"));
    }

    Ok(FaceMeshPocOutput {
        landmarks,
        face_presence: stable_sigmoid(raw_presence[0]),
        tongue_out,
    })
}

fn run_blendshapes(
    image_pixel_landmarks: &[[f32; 2]],
    session: &Mutex<Session>,
) -> Result<Vec<BlendshapeScore>> {
    let input = prepare_blendshape_input(image_pixel_landmarks)?;
    let tensor = Tensor::from_array(input.into_dyn())?;
    let mut session = session
        .lock()
        .map_err(|_| anyhow!("BlendshapeV2 POC session lock is poisoned"))?;
    let outputs = session.run(ort::inputs!["input_points" => tensor])?;
    let output = outputs["output"].try_extract_array::<f32>()?.to_owned();
    decode_blendshape_output(
        output
            .as_slice()
            .ok_or_else(|| anyhow!("BlendshapeV2 output must be contiguous"))?,
    )
}

fn prepare_blendshape_input(image_pixel_landmarks: &[[f32; 2]]) -> Result<Array3<f32>> {
    if image_pixel_landmarks.len() != FACE_MESH_LANDMARK_COUNT {
        return Err(anyhow!(
            "BlendshapeV2 POC requires exactly 478 full-image landmarks"
        ));
    }
    if image_pixel_landmarks
        .iter()
        .flatten()
        .any(|value| !value.is_finite())
    {
        return Err(anyhow!("BlendshapeV2 input contains non-finite values"));
    }

    let mut input = Array3::<f32>::zeros((1, BLENDSHAPE_LANDMARK_INDICES.len(), 2));
    for (output_index, &landmark_index) in BLENDSHAPE_LANDMARK_INDICES.iter().enumerate() {
        input[[0, output_index, 0]] = image_pixel_landmarks[landmark_index][0];
        input[[0, output_index, 1]] = image_pixel_landmarks[landmark_index][1];
    }
    Ok(input)
}

fn decode_blendshape_output(raw_scores: &[f32]) -> Result<Vec<BlendshapeScore>> {
    if raw_scores.len() != BLENDSHAPE_NAMES.len() {
        return Err(anyhow!("BlendshapeV2 POC output contract mismatch"));
    }
    if raw_scores
        .iter()
        .any(|score| !score.is_finite() || !(0.0..=1.0).contains(score))
    {
        return Err(anyhow!(
            "BlendshapeV2 POC output contains an invalid probability"
        ));
    }

    Ok(BLENDSHAPE_NAMES
        .iter()
        .zip(raw_scores)
        .map(|(&name, &score)| BlendshapeScore { name, score })
        .collect())
}

fn stable_sigmoid(value: f32) -> f32 {
    if value >= 0.0 {
        1.0 / (1.0 + (-value).exp())
    } else {
        let exp = value.exp();
        exp / (1.0 + exp)
    }
}

#[cfg(test)]
mod tests {
    use image::Rgb;

    use super::*;

    #[test]
    fn model_assets_match_the_audited_hashes() {
        let model_dir = model_asset_dir();
        verify_model(&model_dir.join(FACE_MESH_MODEL_FILENAME), FACE_MESH_SHA256).unwrap();
        verify_model(
            &model_dir.join(FACE_BLENDSHAPES_MODEL_FILENAME),
            FACE_BLENDSHAPES_SHA256,
        )
        .unwrap();
    }

    #[test]
    fn eye_model_contract_identifiers_are_frozen() {
        assert_eq!(EYE_MODEL_CONTRACT_VERSION, "qraw-eye-model-contract-1.0");
        assert_eq!(EYE_POLICY_VERSION, "qraw-eye-policy-1.1");
        assert_eq!(
            FACE_MESH_MODEL_FILENAME,
            "face_landmarks_detector_v2_qraw_poc.onnx"
        );
        assert_eq!(
            FACE_BLENDSHAPES_MODEL_FILENAME,
            "face_blendshapes_v2_qraw_poc.onnx"
        );
        assert_eq!(
            FACE_MESH_SHA256,
            "b047d95fab6702c327175e7b77eea71ffd2b2ef0110c7466eee9b6e2ae87b552"
        );
        assert_eq!(
            FACE_BLENDSHAPES_SHA256,
            "b90ed4146dfdb43745c5988b1d411ed026d4b5e2ba9c1d7c271954fd1f5cb60e"
        );
        assert_eq!(FACE_MESH_INPUT_SIZE, 256);
        assert_eq!(FACE_MESH_LANDMARK_COUNT, 478);
        assert_eq!(BLENDSHAPE_LANDMARK_INDICES.len(), 146);
        assert_eq!(BLENDSHAPE_NAMES.len(), 52);
        assert_eq!(BLENDSHAPE_NAMES[9], "eyeBlinkLeft");
        assert_eq!(BLENDSHAPE_NAMES[10], "eyeBlinkRight");
    }

    #[test]
    fn face_mesh_input_matches_metadata_rgb_zero_to_one_contract() {
        let roi = RgbImage::from_pixel(256, 256, Rgb([255, 128, 0]));

        let input = prepare_face_mesh_input(&roi).unwrap();

        assert_eq!(input[[0, 0, 0, 0]], 1.0);
        assert!((input[[0, 1, 0, 0]] - 128.0 / 255.0).abs() < f32::EPSILON);
        assert_eq!(input[[0, 2, 0, 0]], 0.0);
    }

    #[test]
    fn face_mesh_rejects_a_raw_unprepared_face_box() {
        let raw_face_box = RgbImage::new(300, 200);

        let error = prepare_face_mesh_input(&raw_face_box).unwrap_err();

        assert!(error.to_string().contains("canonical ROI"));
    }

    #[test]
    fn face_mesh_decoder_normalizes_coordinates_and_presence() {
        let mut landmarks = vec![0.0; FACE_MESH_LANDMARK_COUNT * 3];
        landmarks[0..3].copy_from_slice(&[128.0, 64.0, -32.0]);

        let output = decode_face_mesh_output(&landmarks, &[0.0], &[0.25]).unwrap();

        assert_eq!(output.landmarks[0], [0.5, 0.25, -0.125]);
        assert_eq!(output.face_presence, 0.5);
        assert_eq!(output.tongue_out, 0.25);
    }

    #[test]
    fn blendshape_input_uses_the_official_146_landmark_order() {
        let landmarks: Vec<[f32; 2]> = (0..FACE_MESH_LANDMARK_COUNT)
            .map(|index| [index as f32, -(index as f32)])
            .collect();

        let input = prepare_blendshape_input(&landmarks).unwrap();

        assert_eq!(input[[0, 0, 0]], 0.0);
        assert_eq!(input[[0, 12, 0]], 33.0);
        assert_eq!(input[[0, 145, 0]], 477.0);
        assert_eq!(input[[0, 145, 1]], -477.0);
    }

    #[test]
    fn blendshape_decoder_preserves_official_names_without_deciding_expression() {
        let mut raw = vec![0.0; BLENDSHAPE_NAMES.len()];
        raw[9] = 0.75;
        raw[44] = 0.25;

        let scores = decode_blendshape_output(&raw).unwrap();

        assert_eq!(scores[9].name, "eyeBlinkLeft");
        assert_eq!(scores[9].score, 0.75);
        assert_eq!(scores[44].name, "mouthSmileLeft");
        assert_eq!(scores[44].score, 0.25);
    }

    #[test]
    fn malformed_model_evidence_fails_loudly() {
        assert!(decode_face_mesh_output(&[], &[0.0], &[0.0]).is_err());
        assert!(
            decode_face_mesh_output(
                &vec![0.0; FACE_MESH_LANDMARK_COUNT * 3],
                &[f32::NAN],
                &[0.0]
            )
            .is_err()
        );
        assert!(decode_blendshape_output(&[0.0; 51]).is_err());
        assert!(decode_blendshape_output(&[f32::NAN; 52]).is_err());
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[test]
    #[ignore = "explicit hardware POC gate; not part of production analysis"]
    fn strict_hardware_sessions_smoke_test_both_models() {
        const CHILD_ENV: &str = "QRAW_FACE_MOTION_POC_CHILD";
        const TEST_NAME: &str = "features::smart_culling::face_motion_poc::tests::strict_hardware_sessions_smoke_test_both_models";
        const PASS_MARKER: &str = "QRAW_FACE_MOTION_POC_HARDWARE_PASS";

        // ONNX Runtime 1.22 on macOS has an upstream logger-mutex crash during
        // process teardown. Keep that unrelated failure outside the parent test
        // while still requiring successful strict-provider inference.
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

        let models = load_models(&model_asset_dir()).unwrap();
        let roi = RgbImage::new(256, 256);

        let mesh = run_face_mesh(&roi, &models.face_mesh).unwrap();
        let image_pixel_landmarks: Vec<[f32; 2]> = mesh
            .landmarks
            .iter()
            .map(|point| [point[0] * 256.0, point[1] * 256.0])
            .collect();
        let blendshapes =
            run_blendshapes(&image_pixel_landmarks, &models.face_blendshapes).unwrap();

        assert_eq!(mesh.landmarks.len(), FACE_MESH_LANDMARK_COUNT);
        assert!(mesh.face_presence.is_finite());
        assert_eq!(blendshapes.len(), BLENDSHAPE_NAMES.len());
        println!("{PASS_MARKER}");

        #[cfg(target_os = "macos")]
        {
            use std::io::Write;

            std::io::stdout().flush().unwrap();
            std::io::stderr().flush().unwrap();
            // SAFETY: this is an isolated test child after all assertions and
            // output flushing. `_exit` bypasses the affected ORT 1.22 static
            // logger destructor; no application code uses this path.
            unsafe { libc::_exit(0) }
        }
        #[cfg(target_os = "windows")]
        std::process::exit(0);
    }
}
