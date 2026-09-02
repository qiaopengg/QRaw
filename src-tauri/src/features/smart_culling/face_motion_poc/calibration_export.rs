//! Explicit local export of expression-calibration evidence.
//!
//! This ignored test reuses the current feature's audited preprocessing and
//! inference code. It is not compiled into production builds and refuses to
//! overwrite an existing output file.

mod dataset;
mod report;

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result, anyhow};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use self::dataset::{DatasetInput, ManifestSample};
use super::super::analysis::analyze_image_quality;
use super::super::expression_quality_poc::infer_calibration_face;
use super::super::face_identity::cosine_similarity;
use super::super::face_models::DetectedFace;
use super::super::models::load_face_models_for_test;
use super::super::types::FaceResult;
use super::extract_calibration_evidence;

const CHILD_ENV: &str = "QRAW_EXPRESSION_EVIDENCE_CHILD";
const IMAGE_DIR_ENV: &str = "QRAW_EXPRESSION_EVIDENCE_IMAGE_DIR";
const LABELS_ENV: &str = "QRAW_EXPRESSION_EVIDENCE_LABELS";
const MANIFEST_ENV: &str = "QRAW_EXPRESSION_EVIDENCE_MANIFEST";
const OUTPUT_ENV: &str = "QRAW_EXPRESSION_EVIDENCE_OUTPUT";
const IDENTITY_OUTPUT_ENV: &str = "QRAW_EXPRESSION_IDENTITY_OUTPUT";
const TEST_NAME: &str = "features::smart_culling::face_motion_poc::calibration_export::export_expression_calibration_evidence";
const PASS_MARKER: &str = "QRAW_EXPRESSION_EVIDENCE_EXPORT_PASS";
const EVIDENCE_SCHEMA: &str = "qraw-expression-calibration-evidence-1.0";

struct IdentityEmbedding {
    sample_id: String,
    embedding: Vec<f32>,
}

#[test]
#[ignore = "explicit local calibration export; never part of production analysis"]
fn export_expression_calibration_evidence() {
    if std::env::var_os(CHILD_ENV).is_none() {
        run_isolated_parent();
        return;
    }

    run_export().unwrap();
    println!("{PASS_MARKER}");
    std::io::stdout().flush().unwrap();
    std::io::stderr().flush().unwrap();

    #[cfg(target_os = "macos")]
    // SAFETY: all report data has been flushed and synced. This isolated child
    // exits here only to avoid the affected ORT logger destructor at teardown.
    unsafe {
        libc::_exit(0)
    }
    #[cfg(not(target_os = "macos"))]
    std::process::exit(0);
}

fn run_isolated_parent() {
    for required in [IMAGE_DIR_ENV, LABELS_ENV, OUTPUT_ENV, IDENTITY_OUTPUT_ENV] {
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
        "isolated expression evidence export failed\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

fn run_export() -> Result<()> {
    let DatasetInput {
        image_dir,
        mut labels,
        manifest_by_id,
        output_path,
        identity_output_path,
    } = dataset::load()?;

    let resource_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/smart_culling_models");
    let models = load_face_models_for_test(&resource_dir)?;
    let mut rows = Vec::with_capacity(labels.len());
    let mut identities = Vec::new();
    labels.sort_by(|left, right| left.image_ref.cmp(&right.image_ref));

    for label in labels {
        if !(1..=5).contains(&label.expression_label) {
            return Err(anyhow!(
                "manual expression label is outside 1..=5 for {}",
                label.image_ref
            ));
        }
        let manifest = manifest_by_id
            .get(&label.sample_id)
            .ok_or_else(|| anyhow!("manifest omitted sample {}", label.sample_id))?;
        if manifest.anonymous_ref != label.image_ref {
            return Err(anyhow!(
                "anonymous image reference mismatch for {}",
                label.sample_id
            ));
        }
        let image_path = image_dir.join(&label.image_ref);
        let image_bytes = fs::read(&image_path)
            .with_context(|| format!("cannot read image {}", image_path.display()))?;
        let image_sha256 = hex::encode(Sha256::digest(&image_bytes));
        if image_sha256 != label.sample_id || manifest.sample_id != label.sample_id {
            return Err(anyhow!("image hash mismatch for {}", label.image_ref));
        }
        let image = image::load_from_memory(&image_bytes)
            .with_context(|| format!("cannot decode image {}", image_path.display()))?;
        let quality = analyze_image_quality(&image, true, true, Some(&models), None)?;
        let Some(primary_face) = quality
            .faces
            .iter()
            .max_by(|left, right| face_area(left).total_cmp(&face_area(right)))
        else {
            rows.push(json!({
                "schemaVersion": EVIDENCE_SCHEMA,
                "sampleId": label.sample_id,
                "file": label.image_ref,
                "manualExpressionLabel": label.expression_label,
                "provenance": provenance_json(manifest),
                "status": "no_face",
                "faceCount": 0,
            }));
            continue;
        };
        if let Some(embedding) = primary_face.identity_embedding.clone() {
            identities.push(IdentityEmbedding {
                sample_id: label.sample_id.clone(),
                embedding,
            });
        }
        let detection = DetectedFace {
            bbox: primary_face.bbox,
            score: primary_face.detection_score,
            landmarks: primary_face.landmarks,
        };
        let evidence = extract_calibration_evidence(&image, &detection)?;
        let descriptor = evidence.expression_descriptor().ok();
        let non_eye_blendshapes = evidence
            .blendshapes
            .iter()
            .filter(|(name, _)| !name.starts_with("eye"))
            .map(|(name, value)| ((*name).to_string(), *value))
            .collect::<BTreeMap<_, _>>();
        if non_eye_blendshapes.len() != 38 {
            return Err(anyhow!(
                "non-eye blendshape count mismatch for {}: {}",
                label.image_ref,
                non_eye_blendshapes.len()
            ));
        }
        let expression_models = match infer_calibration_face(&image, detection.bbox) {
            Ok(outputs) => json!({"status": "ok", "mtl": outputs.mtl, "vgaf": outputs.vgaf}),
            Err(error) => json!({"status": "error", "detail": error.to_string()}),
        };
        rows.push(json!({
            "schemaVersion": EVIDENCE_SCHEMA,
            "sampleId": label.sample_id,
            "file": label.image_ref,
            "manualExpressionLabel": label.expression_label,
            "provenance": provenance_json(manifest),
            "status": "ok",
            "faceCount": quality.faces.len(),
            "primaryFace": {
                "bbox": primary_face.bbox,
                "detectionScore": primary_face.detection_score,
                "identityEmbeddingAvailable": primary_face.identity_embedding.is_some(),
            },
            "expressionEvidence": {
                "descriptorVersion": descriptor.as_ref().map(|value| value.descriptor_version()),
                "descriptorReliable": descriptor.as_ref().is_some_and(|value| value.is_reliable()),
                "facePresence": evidence.face_presence,
                "tongueOut": evidence.tongue_out,
                "headPitchDegrees": evidence.head_pitch_degrees,
                "headYawDegrees": evidence.head_yaw_degrees,
                "landmarkConsistencyError": evidence.landmark_consistency_error,
                "nonEyeBlendshapes": non_eye_blendshapes,
                "qualityModels": expression_models,
                "currentBinaryFusion": {
                    "state": primary_face.expression_state,
                    "score": primary_face.expression_score,
                    "confidence": primary_face.expression_confidence,
                    "reason": primary_face.expression_reason,
                },
            },
        }));
    }

    report::write_jsonl(&output_path, &rows)?;
    report::write_json(
        &identity_output_path,
        &json!({
            "schemaVersion": "qraw-expression-identity-similarities-1.0",
            "notice": "Calibration-only cosine similarities; no threshold is treated as a confirmed identity match.",
            "embeddingSampleCount": identities.len(),
            "missingEmbeddingCount": rows.len() - identities.len(),
            "pairs": identity_pairs(&identities),
        }),
    )?;
    println!(
        "exported {} evidence rows and {} identity embeddings",
        rows.len(),
        identities.len()
    );
    Ok(())
}

fn provenance_json(sample: &ManifestSample) -> Value {
    json!({
        "representativeSourceRef": sample.representative_source_ref,
        "sourceFamilyIds": sample.source_family_ids,
        "knownSourceTypes": sample.known_source_types,
    })
}

fn identity_pairs(identities: &[IdentityEmbedding]) -> Vec<Value> {
    let mut pairs = Vec::new();
    for (index, left) in identities.iter().enumerate() {
        for right in identities.iter().skip(index + 1) {
            if let Some(similarity) = cosine_similarity(&left.embedding, &right.embedding) {
                pairs.push(json!({
                    "leftSampleId": left.sample_id,
                    "rightSampleId": right.sample_id,
                    "cosineSimilarity": similarity,
                }));
            }
        }
    }
    pairs.sort_by(|left, right| {
        let left = left["cosineSimilarity"]
            .as_f64()
            .unwrap_or(f64::NEG_INFINITY);
        let right = right["cosineSimilarity"]
            .as_f64()
            .unwrap_or(f64::NEG_INFINITY);
        right.total_cmp(&left)
    });
    pairs
}

fn face_area(face: &FaceResult) -> f32 {
    face.bbox[2].max(0.0) * face.bbox[3].max(0.0)
}
