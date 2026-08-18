//! Explicit paired replay of manual truth, persisted AI output, current
//! production scoring, and isolated face-motion evidence.

mod blind_snapshot;
mod dataset;

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use serde_json::{Value, json};

use self::dataset::PairedDataset;
use super::super::analysis::analyze_image_quality;
use super::super::face_models::DetectedFace;
use super::super::infrastructure::scan_catalog;
use super::super::mode_scoring::{evaluate_mode, normalize_focus, rating_for_mode};
use super::super::models::load_face_models_for_test;
use super::super::runner::analysis_hasher;
use super::super::scoring::{AnalysisCandidate, MODEL_VERSION, POLICY_VERSION, organize_results};
use super::super::types::FaceResult;
use super::extract_calibration_evidence;

const CHILD_ENV: &str = "QRAW_PAIRED_CULLING_EVAL_CHILD";
const ROOT_ENV: &str = "QRAW_PAIRED_CULLING_EVAL_ROOT";
const OUTPUT_ENV: &str = "QRAW_PAIRED_CULLING_EVAL_OUTPUT";
const REQUIRE_PERSISTENCE_ENV: &str = "QRAW_PAIRED_CULLING_EVAL_REQUIRE_PERSISTENCE";
const SNAPSHOT_OUTPUT_ENV: &str = "QRAW_BLIND_CULLING_SNAPSHOT_OUTPUT";
const SNAPSHOT_ENV: &str = "QRAW_BLIND_CULLING_SNAPSHOT";
const SNAPSHOT_SHA256_ENV: &str = "QRAW_BLIND_CULLING_SNAPSHOT_SHA256";
const REPLAY_TEST_NAME: &str =
    "features::smart_culling::face_motion_poc::paired_eval::paired_manual_ai_replay";
const BLIND_REVEAL_TEST_NAME: &str =
    "features::smart_culling::face_motion_poc::paired_eval::blind_manual_ai_reveal";
const PASS_MARKER: &str = "QRAW_PAIRED_CULLING_EVAL_PASS";

#[test]
#[ignore = "explicit local paired-dataset calibration; never part of production analysis"]
fn paired_manual_ai_replay() {
    run_replay_test(REPLAY_TEST_NAME, false);
}

#[test]
#[ignore = "freezes AI results before manual-reference is revealed"]
fn freeze_blind_ai_snapshot() {
    let digest = blind_snapshot::freeze_from_environment(
        ROOT_ENV,
        SNAPSHOT_OUTPUT_ENV,
        POLICY_VERSION,
        MODEL_VERSION,
    )
    .unwrap();
    println!("{SNAPSHOT_SHA256_ENV}={digest}");
}

#[test]
#[ignore = "strict blind reveal; requires a previously frozen AI snapshot"]
fn blind_manual_ai_reveal() {
    run_replay_test(BLIND_REVEAL_TEST_NAME, true);
}

fn run_replay_test(test_name: &str, blind_reveal: bool) {
    if std::env::var_os(CHILD_ENV).is_none() {
        run_isolated_parent(test_name, blind_reveal);
        return;
    }

    run_dataset(blind_reveal).unwrap();
    println!("{PASS_MARKER}");
    std::io::stdout().flush().unwrap();
    std::io::stderr().flush().unwrap();

    #[cfg(target_os = "macos")]
    // SAFETY: output is flushed and this isolated child exists only to avoid
    // the affected ORT 1.22 logger destructor during test process teardown.
    unsafe {
        libc::_exit(0)
    }
    #[cfg(not(target_os = "macos"))]
    std::process::exit(0);
}

fn run_isolated_parent(test_name: &str, blind_reveal: bool) {
    let mut required_envs = vec![ROOT_ENV, OUTPUT_ENV];
    if blind_reveal {
        required_envs.extend([SNAPSHOT_ENV, SNAPSHOT_SHA256_ENV]);
    }
    for required in required_envs {
        assert!(
            std::env::var_os(required).is_some(),
            "missing required environment variable {required}"
        );
    }
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", test_name, "--ignored", "--nocapture"])
        .env(CHILD_ENV, "1")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    print!("{stdout}");
    eprint!("{stderr}");
    assert!(
        output.status.success() && stdout.contains(PASS_MARKER),
        "isolated paired replay failed\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

fn run_dataset(blind_reveal: bool) -> Result<()> {
    if blind_reveal {
        blind_snapshot::verify_from_environment(
            ROOT_ENV,
            SNAPSHOT_ENV,
            SNAPSHOT_SHA256_ENV,
            POLICY_VERSION,
            MODEL_VERSION,
        )?;
    }
    let dataset = PairedDataset::from_environment(ROOT_ENV, OUTPUT_ENV)?;
    let resource_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/smart_culling_models");
    let production_models = load_face_models_for_test(&resource_dir)?;
    let catalog = scan_catalog(&dataset.source_dir).map_err(anyhow::Error::msg)?;
    if !catalog.failures.is_empty() || !catalog.skipped.is_empty() {
        return Err(anyhow!(
            "paired source catalog is not clean: {} failures, {} skipped",
            catalog.failures.len(),
            catalog.skipped.len()
        ));
    }
    if catalog.assets.len() != dataset.items.len() {
        return Err(anyhow!("paired source catalog count changed during replay"));
    }

    let hasher = analysis_hasher();
    let mut candidates = Vec::with_capacity(catalog.assets.len());
    let mut raw_by_file = BTreeMap::<String, Value>::new();
    for asset in catalog.assets {
        let file_name = utf8_file_name(&asset.display_path)?.to_string();
        let item = dataset
            .items
            .get(&file_name)
            .ok_or_else(|| anyhow!("catalog contains an unexpected image: {file_name}"))?;
        let image = image::open(&item.source_path)
            .with_context(|| format!("failed to decode {}", item.source_path.display()))?;
        let quality = analyze_image_quality(&image, true, false, Some(&production_models), None)?;
        let motion = face_motion_json(&image, &quality.faces);
        let candidate = AnalysisCandidate {
            result_id: file_name.clone(),
            path: asset.display_path,
            member_paths: asset.member_paths,
            hash: hasher.hash_image(&image.thumbnail(720, 720)),
            capture_time_millis: asset.capture_time_millis,
            capture_time_from_exif: asset.capture_time_from_exif,
            sequence_number: asset.sequence_number,
            sharpness_metric: quality.sharpness_metric,
            center_focus_metric: quality.center_focus_metric,
            exposure_metric: quality.exposure_metric,
            width: quality.width,
            height: quality.height,
            faces: quality.faces,
            key_person_evidence: Vec::new(),
        };
        let evaluation = evaluate_mode("portrait", &candidate);
        raw_by_file.insert(
            file_name,
            json!({
                "sharpnessMetric": candidate.sharpness_metric,
                "normalizedSharpness": normalize_focus(candidate.sharpness_metric),
                "centerFocusMetric": candidate.center_focus_metric,
                "normalizedCenterFocus": normalize_focus(candidate.center_focus_metric),
                "exposureMetric": candidate.exposure_metric,
                "width": candidate.width,
                "height": candidate.height,
                "faceCount": candidate.faces.len(),
                "faces": candidate.faces.iter().map(face_json).collect::<Vec<_>>(),
                "modeScore": evaluation.score,
                "modeConfidence": evaluation.confidence,
                "baseRating": rating_for_mode(&evaluation.resolved_mode, evaluation.score),
                "modeReason": evaluation.reason_code,
                "modeRequiresReview": evaluation.requires_human_review,
                "faceMotion": motion,
            }),
        );
        candidates.push(candidate);
    }

    let replay = organize_results(&dataset.source_dir, "portrait", candidates)
        .into_iter()
        .map(|result| {
            let name = Path::new(&result.path)
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| anyhow!("replay result filename is not valid UTF-8"))?;
            Ok((name.to_string(), result))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;

    let mut writer = BufWriter::new(File::create(&dataset.output_path)?);
    let mut replay_counts = BTreeMap::<u8, usize>::new();
    let mut persistence_mismatches = Vec::new();
    let mut labeled_count = 0usize;
    let mut exact_count = 0usize;
    let mut within_one_count = 0usize;
    let mut usable_match_count = 0usize;
    let mut absolute_error = 0usize;
    for (file_name, item) in &dataset.items {
        let result = replay
            .get(file_name)
            .ok_or_else(|| anyhow!("current replay omitted {file_name}"))?;
        *replay_counts.entry(result.rating).or_default() += 1;
        let persistence_equivalent = match item.saved_ai_rating {
            Some(saved) => saved == result.rating,
            None => result.rating == 0 && result.requires_human_review,
        };
        if !persistence_equivalent {
            persistence_mismatches.push(file_name.clone());
        }
        if item.manual_rating > 0 {
            let saved_ai_rating = item.saved_ai_rating.unwrap_or(0);
            let difference = item.manual_rating.abs_diff(saved_ai_rating) as usize;
            labeled_count += 1;
            exact_count += usize::from(difference == 0);
            within_one_count += usize::from(difference <= 1);
            usable_match_count += usize::from((item.manual_rating >= 3) == (saved_ai_rating >= 3));
            absolute_error += difference;
        }
        serde_json::to_writer(
            &mut writer,
            &json!({
                "file": item.file_name,
                "imageSha256": item.image_sha256,
                "manual": {"rating": item.manual_rating, "source": item.manual_source},
                "savedAi": {"rating": item.saved_ai_rating},
                "currentReplay": {
                    "rating": result.rating,
                    "confidence": result.confidence,
                    "requiresHumanReview": result.requires_human_review,
                    "reasonCodes": result.reason_codes,
                    "groupKind": result.group_kind,
                    "groupIndex": result.group_index,
                    "groupRank": result.group_rank,
                    "groupSize": result.group_size,
                },
                "rawEvidence": raw_by_file.get(file_name),
            }),
        )?;
        writeln!(writer)?;
    }
    writer.flush()?;
    let require_persistence = std::env::var(REQUIRE_PERSISTENCE_ENV)
        .map(|value| value != "0")
        .unwrap_or(true);
    if !persistence_mismatches.is_empty() && require_persistence {
        return Err(anyhow!(
            "current replay differs from persisted {} for: {}",
            dataset.ai_run,
            persistence_mismatches.join(", ")
        ));
    }

    println!(
        "paired replay wrote {} rows to {} with ratings {:?}",
        dataset.items.len(),
        dataset.output_path.display(),
        replay_counts
    );
    if labeled_count > 0 {
        println!(
            "frozen AI vs manual (manual 0 excluded): n={labeled_count}, exact={:.1}%, within1={:.1}%, mae={:.3}, usable={:.1}%",
            percentage(exact_count, labeled_count),
            percentage(within_one_count, labeled_count),
            absolute_error as f64 / labeled_count as f64,
            percentage(usable_match_count, labeled_count),
        );
    }
    if !persistence_mismatches.is_empty() {
        println!(
            "candidate replay differs from persisted AI output for {} rows",
            persistence_mismatches.len()
        );
    }
    Ok(())
}

fn percentage(count: usize, total: usize) -> f64 {
    count as f64 * 100.0 / total as f64
}

fn face_motion_json(image: &image::DynamicImage, faces: &[FaceResult]) -> Value {
    let Some(face) = faces
        .iter()
        .max_by(|left, right| face_area(left).total_cmp(&face_area(right)))
    else {
        return json!({"status": "no_face"});
    };
    let detection = DetectedFace {
        bbox: face.bbox,
        score: face.detection_score,
        landmarks: face.landmarks,
    };
    match extract_calibration_evidence(image, &detection) {
        Ok(evidence) => json!({
            "status": "ok",
            "facePresence": evidence.face_presence,
            "leftEyeAspectRatio": evidence.left_eye_aspect_ratio,
            "rightEyeAspectRatio": evidence.right_eye_aspect_ratio,
            "headPitchDegrees": evidence.head_pitch_degrees,
            "headYawDegrees": evidence.head_yaw_degrees,
            "landmarkConsistencyError": evidence.landmark_consistency_error,
            "eyeCandidate": {
                "overall": evidence.overall_eye.as_str(),
                "left": evidence.left_eye.as_str(),
                "right": evidence.right_eye.as_str(),
            },
            "blendshapes": evidence.blendshapes,
        }),
        Err(error) => json!({"status": "error", "detail": error.to_string()}),
    }
}

fn face_json(face: &FaceResult) -> Value {
    json!({
        "bbox": face.bbox,
        "detectionScore": face.detection_score,
        "sharpnessMetric": face.sharpness_metric,
        "normalizedSharpness": normalize_focus(face.sharpness_metric),
        "sharpnessConfidence": face.sharpness_confidence,
        "exposureMetric": face.exposure_metric,
        "exposureConfidence": face.exposure_confidence,
        "leftEye": {"state": face.left_eye.state, "reason": face.left_eye.reason},
        "rightEye": {"state": face.right_eye.state, "reason": face.right_eye.reason},
    })
}

fn face_area(face: &FaceResult) -> f32 {
    face.bbox[2].max(0.0) * face.bbox[3].max(0.0)
}

fn utf8_file_name(path: &PathBuf) -> Result<&str> {
    path.file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow!("dataset filename is not valid UTF-8"))
}
