//! Read-only photo probe. Reports are create-new JSON; source images/labels are never written.

use super::super::{analysis::analyze_image_quality, models::load_face_models_for_test};
use anyhow::{Context, Result, anyhow};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{fs, io::Write, path::Path, sync::atomic::AtomicBool};

const TEST: &str = "features::smart_culling::portrait_quality::calibration_export::export_portrait_quality_observations";
const CHILD: &str = "QRAW_PORTRAIT_QUALITY_EXPORT_CHILD";
const INPUT: &str = "QRAW_PORTRAIT_QUALITY_IMAGE_DIR";
const OUTPUT: &str = "QRAW_PORTRAIT_QUALITY_OUTPUT";
const CALIBRATION: &str = "QRAW_PORTRAIT_QUALITY_CALIBRATION";
const PASS: &str = "QRAW_PORTRAIT_QUALITY_EXPORT_PASS";

#[test]
#[ignore = "explicit local photo observation export, not an accuracy benchmark"]
fn export_portrait_quality_observations() {
    if std::env::var_os(CHILD).is_none() {
        for name in [INPUT, OUTPUT] {
            assert!(std::env::var_os(name).is_some(), "missing {name}");
        }
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--ignored", "--nocapture"])
            .env(CHILD, "1")
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            output.status.success() && stdout.contains(PASS),
            "{stdout}\n{stderr}"
        );
        print!("{stdout}");
        return;
    }
    run_export().unwrap();
    println!("{PASS}");
    std::io::stdout().flush().unwrap();
    std::io::stderr().flush().unwrap();
    // SAFETY: isolated test child, after all assertions and report sync; matches
    // the existing calibration test workaround for ORT's logger destructor.
    unsafe { libc::_exit(0) }
}

fn run_export() -> Result<()> {
    let directory = fs::canonicalize(std::env::var(INPUT)?)?;
    let output_path = std::path::PathBuf::from(std::env::var(OUTPUT)?);
    if output_path.exists() || !output_path.is_absolute() {
        return Err(anyhow!("output must be an absent absolute path"));
    }
    let mut paths = fs::read_dir(&directory)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    paths.retain(|path| {
        path.is_file()
            && path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| {
                    matches!(
                        extension.to_ascii_lowercase().as_str(),
                        "jpg" | "jpeg" | "png"
                    )
                })
    });
    paths.sort();
    let calibration = std::env::var(CALIBRATION).as_deref() == Ok("1");
    if calibration {
        // Explicit small-batch export: never silently truncate a labelled batch.
        if paths.len() > 200 {
            return Err(anyhow!("calibration export is limited to 200 images"));
        }
    } else {
        paths.truncate(12);
    }
    if paths.is_empty() {
        return Err(anyhow!("no JPEG/PNG probe images"));
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let models = load_face_models_for_test(&root.join("resources/smart_culling_models"))?;
    super::model::preflight(&root.join("src/features/smart_culling/model_assets"))?;
    let mut rows = Vec::new();
    let mut scored_faces = 0;
    let mut max_reference_error = 0.0_f32;
    for path in paths {
        let bytes = fs::read(&path)?;
        let source_hash = hex::encode(Sha256::digest(&bytes));
        let image = image::load_from_memory(&bytes).with_context(|| path.display().to_string())?;
        let analyzed = analyze_image_quality(&image, true, false, Some(&models), None)?;
        let mut variants = Vec::new();
        let sigmas: &[f32] = if calibration {
            &[0.0]
        } else {
            &[0.0, 2.0, 6.0]
        };
        for &sigma in sigmas {
            let started = std::time::Instant::now();
            let input = if sigma == 0.0 {
                image.clone()
            } else {
                image.blur(sigma)
            };
            // Freeze original detections for blur sensitivity: detection failure
            // must not silently remove a difficult face from a comparison.
            let mut evidence = super::observe(&input, &analyzed.faces, &AtomicBool::new(false))?;
            evidence.input_source = "decodedRgb8";
            let observation_millis = started.elapsed().as_millis();
            for face in &evidence.faces {
                if let Some(score) = face.quality_score {
                    let reference = super::model::official_reference(
                        &input,
                        &analyzed.faces[face.face_index].landmarks,
                    )?;
                    let error = (score - reference).abs();
                    if error >= 1e-3 {
                        return Err(anyhow!(
                            "Core ML/reference mismatch: {score} vs {reference}"
                        ));
                    }
                    max_reference_error = max_reference_error.max(error);
                    scored_faces += 1;
                }
            }
            variants.push(json!({"blurSigma": sigma, "observationMillis": observation_millis, "evidence": evidence}));
        }
        println!(
            "observed {} ({} faces)",
            path.file_name().unwrap().to_string_lossy(),
            analyzed.faces.len()
        );
        // Only geometry/quality; do not serialize identity or expression descriptors.
        let detections = analyzed.faces.iter().enumerate().map(|(index, face)| {
            json!({"faceIndex": index, "bbox": face.bbox, "detectionScore": face.detection_score,
                "sharpnessMetric": face.sharpness_metric, "sharpnessConfidence": face.sharpness_confidence})
        }).collect::<Vec<_>>();
        rows.push(
            json!({"path": path, "sha256": source_hash, "variants": variants,
            "detections": detections, "wholeImageSharpness": analyzed.sharpness_metric}),
        );
    }
    if scored_faces == 0 {
        return Err(anyhow!("probe produced no real-photo quality scores"));
    }
    let report = json!({"schema": "qraw-portrait-quality-probe-2", "accuracyValidated": false,
        "exportPurpose": if calibration { "clarityCalibration" } else { "smokeProbe" },
        "modelVersion": super::super::scoring::MODEL_VERSION, "policyVersion": super::super::scoring::POLICY_VERSION,
        "scoredFaceVariants": scored_faces, "maxOfficialReferenceError": max_reference_error, "rows": rows});
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output_path)?;
    serde_json::to_writer_pretty(&mut file, &report)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    println!(
        "scored_face_variants={scored_faces} max_official_reference_error={max_reference_error} report={}",
        output_path.display()
    );
    Ok(())
}
