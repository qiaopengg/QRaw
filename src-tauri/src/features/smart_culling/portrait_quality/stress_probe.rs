//! Replay the existing local face measurements; no model, scoring or disk image changes.

use anyhow::{Result, anyhow, ensure};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{fs, io::Write, path::Path};

use super::super::stress_variants as variants;

pub(super) fn digest(path: &Path) -> Result<String> {
    Ok(hex::encode(Sha256::digest(fs::read(path)?)))
}

#[test]
#[ignore = "explicit frozen-parameter synthetic sensitivity probe; not an accuracy benchmark"]
fn export_frozen_clarity_stress() {
    run().unwrap();
}

fn run() -> Result<()> {
    let directory = std::path::PathBuf::from(std::env::var("QRAW_CLARITY_STRESS_DIR")?);
    ensure!(
        directory.is_absolute(),
        "use an absolute experiment directory"
    );
    let output_path = directory.join("observations.json");
    ensure!(
        !output_path.exists(),
        "refusing to overwrite a completed experiment"
    );
    let protocol_path = directory.join("PROTOCOL.json");
    let protocol: Value = serde_json::from_slice(&fs::read(&protocol_path)?)?;
    let edge = protocol["measurement"].as_str() == Some("edgeSpreadV1");
    let linear = protocol["measurement"].as_str() == Some("linearNoiseDetailV1");
    let robust = edge || linear || protocol["measurement"].as_str() == Some("robustDetailV1");
    let frozen = Path::new(
        protocol["frozen_path"]
            .as_str()
            .ok_or_else(|| anyhow!("missing frozen path"))?,
    );
    ensure!(frozen.is_absolute(), "invalid frozen path");
    let feature = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/features/smart_culling");
    for (name, expected) in protocol["measurement_code_hashes"].as_object().unwrap() {
        ensure!(
            digest(&feature.join(name))? == expected.as_str().unwrap(),
            "measurement code changed: {name}"
        );
    }
    for (name, expected) in protocol["frozen_file_hashes"].as_object().unwrap() {
        ensure!(
            digest(&frozen.join(name))? == expected.as_str().unwrap(),
            "frozen input changed: {name}"
        );
    }
    if robust && protocol["phase"].as_str() == Some("baseline") {
        ensure!(
            protocol["cases"]
                .as_array()
                .is_some_and(|cases| cases.len() == 1 && cases[0]["kind"] == "original")
        );
    } else {
        ensure!(
            digest(&directory.join("candidate.json"))?
                == protocol["candidate_sha256"].as_str().unwrap()
        );
    }
    let manifest: Value = serde_json::from_slice(&fs::read(frozen.join("manifest.json"))?)?;
    let cases: Vec<variants::Case> = serde_json::from_value(protocol["cases"].clone())?;
    let items = manifest["items"].as_array().unwrap();
    ensure!(items.len() == protocol["sample_count"].as_u64().unwrap() as usize);
    let mut rows = Vec::new();
    for item in items {
        let source_path = Path::new(item["source_path"].as_str().unwrap());
        let bytes = fs::read(source_path)?;
        ensure!(hex::encode(Sha256::digest(&bytes)) == item["sample_id"].as_str().unwrap());
        let original = if robust {
            crate::features::smart_culling::infrastructure::ClarityInput::from_bytes(
                &bytes,
                false,
                &json!({}),
            )
            .image()
            .cloned()
            .ok_or_else(|| anyhow!("native clarity input unavailable"))?
        } else {
            image::load_from_memory(&bytes)?
        };
        ensure!(json!([original.width(), original.height()]) == item["dimensions"]);
        let bbox = serde_json::from_value(item["bbox"].clone())?;
        let mut measured = Vec::new();
        for case in &cases {
            let (input, target) = variants::make(&original, bbox, case)?;
            let mut face = super::sample_face();
            face.bbox = target;
            // Deliberately skip FIQA inference, but use the actual face-box validity
            // checks and detail measurement. No invented landmark/identity evidence.
            face.landmarks = [(f32::NAN, f32::NAN); 5];
            let observation = super::super::face::observe(&input, &face, 0);
            let source = match case.kind.as_str() {
                "original" if robust => "decodedRgb8LosslessGeometry",
                "original" => "decodedRgb8",
                "resize" => "resampledRgb8",
                _ => "controlledStressRgb8",
            };
            let mut measurement = json!({"case_id": case.id, "input_source": source,
                "input_dimensions": [input.width(), input.height()], "bbox": target,
                "crop_dimensions": observation.crop_dimensions, "detail": observation.detail});
            if robust {
                let native = super::super::clarity_observation::observe_face(&input, &face, 0);
                if edge {
                    let spread = (native.unavailable_reason.is_none())
                        .then(|| super::super::face::validated_crop(&input, face.bbox).ok())
                        .flatten()
                        .and_then(|crop| super::super::edge_spread::measure(&crop));
                    measurement["edge_spread"] = serde_json::to_value(spread)?;
                }
                if linear {
                    let corrected = native.robust_detail.as_ref().and_then(|robust| {
                        let crop = super::super::face::validated_crop(&input, face.bbox).ok()?;
                        super::super::linear_detail::measure(&crop, robust.noise_sigma)
                    });
                    measurement["linear_detail"] = serde_json::to_value(corrected)?;
                }
                measurement["robust_detail"] = serde_json::to_value(native.robust_detail)?;
                measurement["unavailable_reason"] =
                    serde_json::to_value(native.unavailable_reason)?;
            }
            measured.push(measurement);
        }
        ensure!(
            digest(source_path)? == item["sample_id"].as_str().unwrap(),
            "source changed during probe"
        );
        println!(
            "measured {}: {} fixed cases",
            item["image_ref"],
            cases.len()
        );
        rows.push(json!({"sample_id": item["sample_id"], "image_ref": item["image_ref"], "variants": measured}));
    }
    let schema = if edge {
        "qraw-clarity-stress-observations-4"
    } else if linear {
        "qraw-clarity-stress-observations-3"
    } else if robust {
        "qraw-clarity-stress-observations-2"
    } else {
        "qraw-clarity-stress-observations-1"
    };
    let report = json!({"schema": schema, "protocol_sha256": digest(&protocol_path)?,
        "measurement_code_hashes": protocol["measurement_code_hashes"], "rows": rows,
        "detection_rerun": false, "fiqa_rerun": false, "gpu_render_exercised": false,
        "source_images_written": false, "accuracy_validated": false});
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output_path)?;
    serde_json::to_writer_pretty(&mut file, &report)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    Ok(())
}
