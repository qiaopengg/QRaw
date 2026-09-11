//! Shared I/O for the two fixed face-crop experiments; never writes source images.

use anyhow::{Result, anyhow, ensure};
use image::DynamicImage;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::HashSet, fs, io::Write, path::Path};

use super::super::{degradation_cases, face};
pub(super) use super::stress_probe::digest;

pub(super) struct Inputs {
    pub protocol: Value,
    pub items: Vec<Value>,
    pub cases: Vec<degradation_cases::Case>,
}

pub(super) fn verify_hashes(root: &Path, hashes: &Value) -> Result<()> {
    for (name, expected) in hashes
        .as_object()
        .ok_or_else(|| anyhow!("missing hashes"))?
    {
        ensure!(
            digest(&root.join(name))?
                == expected.as_str().ok_or_else(|| anyhow!("invalid hash"))?,
            "file changed: {name}"
        );
    }
    Ok(())
}

pub(super) fn load(directory: &Path, schema: &str) -> Result<Inputs> {
    ensure!(
        directory.is_absolute(),
        "use an absolute experiment directory"
    );
    ensure!(
        !directory.join("observations.json").exists(),
        "refusing to overwrite experiment"
    );
    let protocol: Value = serde_json::from_slice(&fs::read(directory.join("PROTOCOL.json"))?)?;
    ensure!(protocol["schema"] == schema);
    ensure!(digest(&directory.join("DESIGN.md"))? == protocol["design_sha256"].as_str().unwrap());
    let feature = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/features/smart_culling");
    verify_hashes(&feature, &protocol["code_hashes"])?;
    let frozen = Path::new(
        protocol["frozen_path"]
            .as_str()
            .ok_or_else(|| anyhow!("missing frozen path"))?,
    );
    ensure!(frozen.is_absolute());
    verify_hashes(frozen, &protocol["frozen_file_hashes"])?;
    let manifest: Value = serde_json::from_slice(&fs::read(frozen.join("manifest.json"))?)?;
    let items = manifest["items"]
        .as_array()
        .ok_or_else(|| anyhow!("missing items"))?
        .clone();
    let cases: Vec<degradation_cases::Case> = serde_json::from_value(protocol["cases"].clone())?;
    let case_ids = cases
        .iter()
        .map(|case| case.id.as_str())
        .collect::<HashSet<_>>();
    ensure!(items.len() == protocol["sample_count"].as_u64().unwrap() as usize);
    ensure!(
        case_ids.len() == cases.len() && cases.len() == 206,
        "invalid case matrix"
    );
    ensure!(
        cases
            .iter()
            .all(|case| case_ids.contains(case.reference_id.as_str())),
        "missing clean reference"
    );
    let samples = items
        .iter()
        .map(|item| &item["sample_id"])
        .collect::<HashSet<_>>();
    ensure!(samples.len() == items.len(), "duplicate sample");
    Ok(Inputs {
        protocol,
        items,
        cases,
    })
}

pub(super) fn crop(item: &Value) -> Result<DynamicImage> {
    let source = Path::new(item["source_path"].as_str().unwrap());
    let bytes = fs::read(source)?;
    ensure!(
        hex::encode(Sha256::digest(&bytes)) == item["sample_id"],
        "source changed before probe"
    );
    let native = crate::features::smart_culling::infrastructure::ClarityInput::from_bytes(
        &bytes,
        false,
        &json!({}),
    );
    let original = native
        .image()
        .ok_or_else(|| anyhow!("native input unavailable"))?;
    ensure!(json!([original.width(), original.height()]) == item["dimensions"]);
    face::validated_crop(original, bbox(item)?)
        .map_err(|reason| anyhow!("invalid frozen face: {reason:?}"))
}

pub(super) fn verify_source(item: &Value) -> Result<()> {
    ensure!(
        digest(Path::new(item["source_path"].as_str().unwrap()))? == item["sample_id"],
        "source changed during probe"
    );
    Ok(())
}

pub(super) fn sample(item: &Value, variants: Vec<Value>) -> Result<Value> {
    Ok(
        json!({"sample_id": item["sample_id"], "image_ref": item["image_ref"], "bbox": bbox(item)?, "variants": variants}),
    )
}

fn bbox(item: &Value) -> Result<[f32; 4]> {
    // Restore the same typed coordinates used for cropping and the original v1 export.
    // Forwarding Value's parsed f64 may change a decimal by one ULP on reserialization.
    Ok(serde_json::from_value(item["bbox"].clone())?)
}

pub(super) fn save(directory: &Path, result: &Value) -> Result<()> {
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(directory.join("observations.json"))?;
    serde_json::to_writer_pretty(&mut file, result)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    Ok(())
}

#[test]
fn exported_face_coordinates_keep_the_original_f32_contract() {
    let original = "[447.02410888671875,375.941650390625,88.3038558959961,112.64225006103516]";
    let item: Value = serde_json::from_str(&format!("{{\"bbox\":{original}}}")).unwrap();
    let row = sample(&item, vec![]).unwrap();
    assert_eq!(serde_json::to_string(&row["bbox"]).unwrap(), original);
}
