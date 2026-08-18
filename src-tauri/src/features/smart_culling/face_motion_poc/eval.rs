//! Explicit real-photo evidence dump for the isolated face-motion POC.
//!
//! Run only with the three QRAW_FACE_MOTION_EVAL_* environment variables. The
//! image directory is the untouched control copy, while labels come from the
//! working directory's sidecars. The emitted eye candidate is calibration-only
//! and is not connected to production analysis or scoring.

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, anyhow};
use serde_json::{Value, json};

use super::super::face_models::run_yunet_detection;
use super::super::models::load_yunet_for_test;
use super::evidence::extract_face_motion_evidence;
use super::eye_policy::EyeUsability;
use super::{load_models, model_asset_dir};

const CHILD_ENV: &str = "QRAW_FACE_MOTION_EVAL_CHILD";
const IMAGE_DIR_ENV: &str = "QRAW_FACE_MOTION_EVAL_IMAGE_DIR";
const LABEL_DIR_ENV: &str = "QRAW_FACE_MOTION_EVAL_LABEL_DIR";
const OUTPUT_ENV: &str = "QRAW_FACE_MOTION_EVAL_OUTPUT";
const TEST_NAME: &str =
    "features::smart_culling::face_motion_poc::eval::real_photo_face_motion_evidence_dump";
const PASS_MARKER: &str = "QRAW_FACE_MOTION_EVAL_PASS";

#[test]
#[ignore = "explicit real-photo calibration gate; never part of production analysis"]
fn real_photo_face_motion_evidence_dump() {
    if std::env::var_os(CHILD_ENV).is_none() {
        run_isolated_parent();
        return;
    }

    run_dataset().unwrap();
    println!("{PASS_MARKER}");
    std::io::stdout().flush().unwrap();
    std::io::stderr().flush().unwrap();

    #[cfg(target_os = "macos")]
    // SAFETY: this is the isolated test child after all output has been fully
    // written. It only bypasses the affected ORT 1.22 logger destructor.
    unsafe {
        libc::_exit(0)
    }
    #[cfg(not(target_os = "macos"))]
    std::process::exit(0);
}

fn run_isolated_parent() {
    for required in [IMAGE_DIR_ENV, LABEL_DIR_ENV, OUTPUT_ENV] {
        assert!(
            std::env::var_os(required).is_some(),
            "missing required environment variable {required}"
        );
    }
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", TEST_NAME, "--ignored", "--nocapture"])
        .env(CHILD_ENV, "1")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    print!("{stdout}");
    eprint!("{stderr}");
    assert!(
        output.status.success() && stdout.contains(PASS_MARKER),
        "isolated real-photo POC failed\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

fn run_dataset() -> Result<()> {
    let image_dir = required_dir(IMAGE_DIR_ENV)?;
    let label_dir = required_dir(LABEL_DIR_ENV)?;
    let output_path = PathBuf::from(required_value(OUTPUT_ENV)?);
    let mut images = fs::read_dir(&image_dir)?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("png"))
        })
        .collect::<Vec<_>>();
    images.sort();
    if images.is_empty() {
        return Err(anyhow!(
            "real-photo evaluation directory contains no PNG files"
        ));
    }

    let resource_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/smart_culling_models");
    let yunet = load_yunet_for_test(&resource_dir)?;
    let models = load_models(&model_asset_dir())?;
    let mut writer = BufWriter::new(File::create(&output_path)?);
    let mut label_counts = BTreeMap::<String, usize>::new();

    for image_path in &images {
        let file_name = image_path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| anyhow!("evaluation image filename is not valid UTF-8"))?;
        let working_image = label_dir.join(file_name);
        ensure_matching_control_copy(image_path, &working_image)?;
        let label = read_label(&label_dir.join(format!("{file_name}.rrdata")))?;
        *label_counts.entry(label.clone()).or_default() += 1;

        let started = Instant::now();
        let image = image::open(image_path)
            .with_context(|| format!("failed to decode {}", image_path.display()))?;
        let detections = run_yunet_detection(&image, &yunet)?;
        if detections.is_empty() {
            serde_json::to_writer(
                &mut writer,
                &json!({"file": file_name, "label": label, "status": "no_face"}),
            )?;
            writeln!(writer)?;
            continue;
        }

        let detection = detections
            .iter()
            .max_by(|left, right| {
                let left_area = left.bbox[2].max(0.0) * left.bbox[3].max(0.0);
                let right_area = right.bbox[2].max(0.0) * right.bbox[3].max(0.0);
                left_area.total_cmp(&right_area)
            })
            .unwrap();
        let evidence = extract_face_motion_evidence(&image, detection, &models)?;
        ensure_green_eye_safety(file_name, &label, evidence.overall_eye)?;

        serde_json::to_writer(
            &mut writer,
            &json!({
                "file": file_name,
                "label": label,
                "status": "ok",
                "faceCount": detections.len(),
                "detectionScore": detection.score,
                "facePresence": evidence.face_presence,
                "tongueOut": evidence.tongue_out,
                "roi": {
                    "centerX": evidence.roi.center_x,
                    "centerY": evidence.roi.center_y,
                    "width": evidence.roi.width,
                    "height": evidence.roi.height,
                    "rotationDegrees": evidence.roi.rotation.to_degrees(),
                },
                "leftEyeAspectRatio": evidence.left_eye_aspect_ratio,
                "rightEyeAspectRatio": evidence.right_eye_aspect_ratio,
                "eyeCandidate": {
                    "overall": evidence.overall_eye.as_str(),
                    "left": evidence.left_eye.as_str(),
                    "right": evidence.right_eye.as_str(),
                },
                "blendshapes": evidence.blendshapes,
                "latencyMs": started.elapsed().as_secs_f64() * 1000.0,
            }),
        )?;
        writeln!(writer)?;
    }
    writer.flush()?;
    println!(
        "wrote {} frozen-label rows to {}: {:?}",
        images.len(),
        output_path.display(),
        label_counts
    );
    Ok(())
}

fn ensure_green_eye_safety(file_name: &str, label: &str, candidate: EyeUsability) -> Result<()> {
    if label == "green" && candidate == EyeUsability::Unusable {
        return Err(anyhow!(
            "eye candidate would reject a photographer-selected photo: {file_name} ({})",
            candidate.as_str()
        ));
    }
    Ok(())
}

fn required_value(name: &str) -> Result<String> {
    std::env::var(name).map_err(|_| anyhow!("missing required environment variable {name}"))
}

fn required_dir(name: &str) -> Result<PathBuf> {
    let path = PathBuf::from(required_value(name)?);
    if !path.is_dir() {
        return Err(anyhow!("{name} is not a directory: {}", path.display()));
    }
    Ok(path)
}

fn ensure_matching_control_copy(control: &Path, working: &Path) -> Result<()> {
    let control_bytes = fs::read(control)?;
    let working_bytes = fs::read(working)
        .with_context(|| format!("working copy is missing: {}", working.display()))?;
    if control_bytes != working_bytes {
        return Err(anyhow!(
            "control and working image differ: {}",
            control.display()
        ));
    }
    Ok(())
}

fn read_label(sidecar: &Path) -> Result<String> {
    let value: Value = serde_json::from_slice(
        &fs::read(sidecar)
            .with_context(|| format!("missing label sidecar: {}", sidecar.display()))?,
    )?;
    let label = value
        .pointer("/featureData/smartCullingV2/colorLabel")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("sidecar has no manual color label: {}", sidecar.display()))?;
    if !matches!(label, "green" | "yellow" | "red") {
        return Err(anyhow!("unsupported evaluation label '{label}'"));
    }
    Ok(label.to_string())
}
