//! Offline replay through the actual subject selector and clarity gate, not a Python copy.
//! Loaded only inside mode_scoring's tests so existing fixtures can be reused unchanged.

use crate::features::smart_culling::{
    mode_evidence::clarity_evidence, mode_policy::ClarityGateTarget,
    mode_scoring::portrait_subjects, quality_evidence::ClarityState,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{fs, io::Write, path::Path};

fn replay(row: &Value) -> Value {
    let dimensions = &row["variants"][0]["evidence"]["inputDimensions"];
    let faces = row["detections"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .map(|(index, detected)| {
            assert_eq!(detected["faceIndex"].as_u64().unwrap(), index as u64);
            let mut face = super::face(0.0, 1.0, "unknown", "unknown");
            face.bbox = serde_json::from_value(detected["bbox"].clone()).unwrap();
            face.detection_score = detected["detectionScore"].as_f64().unwrap() as f32;
            face.sharpness_metric = detected["sharpnessMetric"].as_f64().unwrap();
            face.sharpness_confidence = detected["sharpnessConfidence"].as_f64().unwrap() as f32;
            face
        })
        .collect();
    let mut item = super::candidate(faces);
    item.width = u32::try_from(dimensions[0].as_u64().unwrap()).unwrap();
    item.height = u32::try_from(dimensions[1].as_u64().unwrap()).unwrap();
    item.sharpness_metric = row["wholeImageSharpness"].as_f64().unwrap();
    let subjects = portrait_subjects(&item).unwrap_or_default();
    let indices: Vec<_> = subjects
        .iter()
        .map(|subject| {
            item.faces
                .iter()
                .position(|face| std::ptr::eq(face, *subject))
                .unwrap()
        })
        .collect();
    let gate = clarity_evidence(ClarityGateTarget::Person, &item, &subjects);
    let state = match gate.state {
        ClarityState::Clear => "clear",
        ClarityState::Unclear => "unclear",
        ClarityState::Uncertain => "uncertain",
    };
    json!({"sha256": row["sha256"], "state": state, "subjectFaceIndices": indices,
        "reason": gate.evidence.reason, "sourceVersion": gate.evidence.source_version})
}

#[test]
#[ignore = "explicit read-only frozen observation replay; no model or label fitting"]
fn export_frozen_clarity_gate_replay() {
    let batch = std::path::PathBuf::from(std::env::var("QRAW_CLARITY_FROZEN_BATCH").unwrap());
    let output = std::path::PathBuf::from(std::env::var("QRAW_CLARITY_REPLAY_OUTPUT").unwrap());
    assert!(batch.is_absolute() && output.is_absolute() && !output.exists());
    let frozen: Value =
        serde_json::from_slice(&fs::read(batch.join("FREEZE_REPORT.json")).unwrap()).unwrap();
    let feature = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/features/smart_culling");
    for name in ["mode_evidence.rs", "mode_scoring.rs"] {
        let digest = hex::encode(Sha256::digest(fs::read(feature.join(name)).unwrap()));
        assert_eq!(frozen["code_hashes"][name].as_str().unwrap(), digest);
    }
    let bytes = fs::read(batch.join("observations.json")).unwrap();
    let digest = hex::encode(Sha256::digest(&bytes));
    assert_eq!(
        frozen["file_hashes"]["observations.json"].as_str().unwrap(),
        digest
    );
    let observations: Value = serde_json::from_slice(&bytes).unwrap();
    let rows: Vec<_> = observations["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(replay)
        .collect();
    assert_eq!(rows.len(), frozen["total"].as_u64().unwrap() as usize);
    let report = json!({"schema": "qraw-clarity-runtime-replay-1", "inputSource": "decodedRgb8",
        "observationsSha256": digest, "runtimeCodeHashes": frozen["code_hashes"], "rows": rows,
        "accuracyValidated": false, "usesActualRuntimeGate": true});
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output)
        .unwrap();
    serde_json::to_writer_pretty(&mut file, &report).unwrap();
    file.write_all(b"\n").unwrap();
    file.sync_all().unwrap();
    println!(
        "replayed {} frozen observations through the runtime gate",
        rows.len()
    );
}

#[test]
fn replay_preserves_the_actual_gate_and_subject_selection() {
    let mut row = json!({"sha256": "fixture", "wholeImageSharpness": 100.0,
        "variants": [{"evidence": {"inputDimensions": [1000, 1000]}}], "detections": [
            {"faceIndex": 0, "bbox": [100.0, 100.0, 100.0, 100.0], "detectionScore": 0.95,
             "sharpnessMetric": 300.0, "sharpnessConfidence": 0.9},
            {"faceIndex": 1, "bbox": [500.0, 100.0, 300.0, 300.0], "detectionScore": 0.95,
             "sharpnessMetric": 1.0, "sharpnessConfidence": 0.9}]});
    assert_eq!(replay(&row)["subjectFaceIndices"], json!([1]));
    assert_eq!(replay(&row)["state"], "uncertain");
    row["wholeImageSharpness"] = json!(1.0);
    assert_eq!(replay(&row)["state"], "unclear");
    row["detections"][1]["sharpnessMetric"] = json!(300.0);
    assert_eq!(replay(&row)["state"], "clear");
    row["detections"] = json!([]);
    assert_eq!(replay(&row)["state"], "uncertain");
}
