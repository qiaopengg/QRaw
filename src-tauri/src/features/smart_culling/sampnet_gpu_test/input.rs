//! Frozen little-endian tensors shared with the official Python reference.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use ndarray::Array4;
use serde_json::Value;

use super::super::models::verify_model;

pub(super) fn read_tensor(directory: &Path, spec: &Value, channels: usize) -> Result<Array4<f32>> {
    let name = spec["file"].as_str().context("input filename missing")?;
    ensure!(
        Path::new(name).file_name().and_then(|p| p.to_str()) == Some(name),
        "input must be a file inside the frozen baseline"
    );
    ensure!(
        spec["shape"] == serde_json::json!([1, channels, 224, 224]),
        "unverified input shape"
    );
    let path = directory.join(name);
    verify_model(
        &path,
        spec["sha256"].as_str().context("input hash missing")?,
    )?;
    let bytes = fs::read(path)?;
    ensure!(
        bytes.len() == channels * 224 * 224 * 4,
        "input length mismatch"
    );
    let values: Vec<f32> = bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap()))
        .collect();
    ensure!(
        values.iter().all(|value| value.is_finite()),
        "nonfinite input"
    );
    Ok(Array4::from_shape_vec((1, channels, 224, 224), values)?)
}

#[test]
fn rejects_paths_and_shapes_before_reading_any_input() {
    for name in ["../outside.f32le", "/outside.f32le"] {
        let spec = serde_json::json!({"file": name, "shape": [1, 3, 224, 224]});
        assert!(read_tensor(Path::new("."), &spec, 3).is_err());
    }
    let spec = serde_json::json!({"file": "image.f32le", "shape": [1, 1, 224, 224]});
    assert!(read_tensor(Path::new("."), &spec, 3).is_err());
}
