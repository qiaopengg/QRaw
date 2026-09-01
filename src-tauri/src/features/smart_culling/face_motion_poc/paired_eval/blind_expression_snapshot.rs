use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use serde_json::Value;

use super::blind_snapshot::{normalize_digest, sha256};
use super::dataset::{ensure_source_has_no_sidecars, image_names, required_dir, required_value};

#[derive(Debug)]
pub(super) struct FrozenExpressionPredictions {
    expression_by_file: BTreeMap<String, Value>,
}

impl FrozenExpressionPredictions {
    pub(super) fn expression_for(&self, file: &str) -> Result<&Value> {
        self.expression_by_file
            .get(file)
            .ok_or_else(|| anyhow!("frozen expression predictions omitted {file}"))
    }
}

pub(super) fn freeze_from_environment(
    root_env: &str,
    output_env: &str,
    expected_policy: &str,
    expected_model: &str,
) -> Result<String> {
    let root = required_dir(root_env)?;
    if root.join("manual-reference").exists() {
        return Err(anyhow!(
            "manual-reference must remain absent while expression predictions are frozen"
        ));
    }
    let output = PathBuf::from(required_value(output_env)?);
    if !output.is_absolute() {
        return Err(anyhow!(
            "blind expression prediction output must be an absolute path"
        ));
    }
    let bytes = fs::read(&output).with_context(|| {
        format!(
            "cannot read expression prediction freeze {}",
            output.display()
        )
    })?;
    validate_report(&root, &bytes, expected_policy, expected_model)?;
    Ok(sha256(&bytes))
}

pub(super) fn verify_from_environment(
    root_env: &str,
    snapshot_env: &str,
    digest_env: &str,
    expected_policy: &str,
    expected_model: &str,
) -> Result<FrozenExpressionPredictions> {
    let root = required_dir(root_env)?;
    let snapshot_path = PathBuf::from(required_value(snapshot_env)?);
    let expected_digest = normalize_digest(&required_value(digest_env)?)?;
    let bytes = fs::read(&snapshot_path).with_context(|| {
        format!(
            "cannot read frozen expression predictions {}",
            snapshot_path.display()
        )
    })?;
    let actual_digest = sha256(&bytes);
    if actual_digest != expected_digest {
        return Err(anyhow!(
            "frozen expression prediction SHA-256 mismatch: expected {expected_digest}, got {actual_digest}"
        ));
    }
    validate_report(&root, &bytes, expected_policy, expected_model)
}

fn validate_report(
    root: &Path,
    bytes: &[u8],
    expected_policy: &str,
    expected_model: &str,
) -> Result<FrozenExpressionPredictions> {
    let source_dir = root.join("source");
    if !source_dir.is_dir() {
        return Err(anyhow!(
            "paired dataset directory is missing: {}",
            source_dir.display()
        ));
    }
    ensure_source_has_no_sidecars(&source_dir)?;
    let source_names = image_names(&source_dir)?;
    if source_names.is_empty() {
        return Err(anyhow!(
            "blind expression prediction source contains no supported images"
        ));
    }

    let text = std::str::from_utf8(bytes)
        .context("frozen expression prediction report is not valid UTF-8")?;
    let mut expression_by_file = BTreeMap::new();
    for (index, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let row: Value = serde_json::from_str(line).with_context(|| {
            format!(
                "invalid frozen expression prediction JSON on line {}",
                index + 1
            )
        })?;
        let file = row
            .get("file")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("frozen expression prediction row has no file"))?;
        if !source_names.contains(file) {
            return Err(anyhow!(
                "frozen expression prediction contains an unexpected file: {file}"
            ));
        }
        let image_bytes = fs::read(source_dir.join(file))?;
        let recorded_image_hash = row
            .get("imageSha256")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("frozen expression prediction has no image hash for {file}"))?;
        if recorded_image_hash != sha256(&image_bytes) {
            return Err(anyhow!(
                "frozen expression prediction image hash differs for {file}"
            ));
        }
        if row.pointer("/manual/rating").and_then(Value::as_u64) != Some(0)
            || !row.pointer("/manual/source").is_none_or(Value::is_null)
        {
            return Err(anyhow!(
                "frozen expression prediction contains a revealed manual label for {file}"
            ));
        }
        if row
            .pointer("/contract/policyVersion")
            .and_then(Value::as_str)
            != Some(expected_policy)
            || row
                .pointer("/contract/modelVersion")
                .and_then(Value::as_str)
                != Some(expected_model)
        {
            return Err(anyhow!(
                "frozen expression prediction contract is invalid for {file}"
            ));
        }
        let expression = row
            .get("expressionComponent")
            .cloned()
            .ok_or_else(|| anyhow!("frozen expression prediction is missing for {file}"))?;
        if expression_by_file
            .insert(file.to_string(), expression)
            .is_some()
        {
            return Err(anyhow!(
                "frozen expression prediction contains duplicate file {file}"
            ));
        }
    }
    if expression_by_file.len() != source_names.len() {
        return Err(anyhow!(
            "frozen expression prediction count differs from source: expected {}, got {}",
            source_names.len(),
            expression_by_file.len()
        ));
    }
    Ok(FrozenExpressionPredictions { expression_by_file })
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tempfile::tempdir;

    use super::*;

    const POLICY: &str = "test-policy";
    const MODEL: &str = "test-model";

    fn fixture(manual_rating: u8) -> (tempfile::TempDir, PathBuf, Vec<u8>) {
        let temp = tempdir().unwrap();
        let root = temp.path().join("blind");
        let source = root.join("source");
        fs::create_dir_all(&source).unwrap();
        let image = b"same-image";
        fs::write(source.join("portrait.jpg"), image).unwrap();
        let mut bytes = serde_json::to_vec(&json!({
            "file": "portrait.jpg",
            "imageSha256": sha256(image),
            "manual": {"rating": manual_rating, "source": null},
            "contract": {"policyVersion": POLICY, "modelVersion": MODEL},
            "expressionComponent": {"state": "scored", "score": 0.75},
        }))
        .unwrap();
        bytes.push(b'\n');
        (temp, root, bytes)
    }

    #[test]
    fn validates_and_returns_unlabeled_prediction() {
        let (_temp, root, bytes) = fixture(0);
        let frozen = validate_report(&root, &bytes, POLICY, MODEL).unwrap();

        assert_eq!(
            frozen.expression_for("portrait.jpg").unwrap(),
            &json!({"state": "scored", "score": 0.75})
        );
        assert!(frozen.expression_for("missing.jpg").is_err());
    }

    #[test]
    fn rejects_report_that_already_contains_manual_truth() {
        let (_temp, root, bytes) = fixture(4);

        let error = validate_report(&root, &bytes, POLICY, MODEL).unwrap_err();

        assert!(error.to_string().contains("revealed manual label"));
    }
}
