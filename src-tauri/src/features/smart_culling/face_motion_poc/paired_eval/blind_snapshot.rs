use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::dataset::{
    ai_run_name, ensure_same_image_names, ensure_source_has_no_sidecars, image_names, read_rating,
    required_child_dir, required_dir, required_value,
};

const SNAPSHOT_SCHEMA_VERSION: u8 = 1;

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BlindSnapshot {
    schema_version: u8,
    ai_run: String,
    policy_version: String,
    model_version: String,
    entries: Vec<BlindEntry>,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BlindEntry {
    file: String,
    image_sha256: String,
    ai_rating: u8,
    sidecar_present: bool,
    ai_sidecar_sha256: Option<String>,
}

pub(super) fn freeze_from_environment(
    root_env: &str,
    output_env: &str,
    expected_policy: &str,
    expected_model: &str,
) -> Result<String> {
    let root = required_dir(root_env)?;
    let output = PathBuf::from(required_value(output_env)?);
    let ai_run = ai_run_name()?;
    freeze_snapshot(&root, &ai_run, &output, expected_policy, expected_model)
}

pub(super) fn verify_from_environment(
    root_env: &str,
    snapshot_env: &str,
    digest_env: &str,
    expected_policy: &str,
    expected_model: &str,
) -> Result<()> {
    let root = required_dir(root_env)?;
    let snapshot_path = PathBuf::from(required_value(snapshot_env)?);
    let expected_digest = required_value(digest_env)?;
    let ai_run = ai_run_name()?;
    verify_snapshot(
        &root,
        &ai_run,
        &snapshot_path,
        &expected_digest,
        expected_policy,
        expected_model,
    )
}

fn freeze_snapshot(
    root: &Path,
    ai_run: &str,
    output: &Path,
    expected_policy: &str,
    expected_model: &str,
) -> Result<String> {
    if root.join("manual-reference").exists() {
        return Err(anyhow!(
            "manual-reference must remain outside the blind root until the AI snapshot is frozen"
        ));
    }
    if !output.is_absolute() {
        return Err(anyhow!("blind snapshot output must be an absolute path"));
    }

    let snapshot = capture_snapshot(root, ai_run, expected_policy, expected_model)?;
    let mut bytes = serde_json::to_vec_pretty(&snapshot)?;
    bytes.push(b'\n');
    let digest = sha256(&bytes);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .with_context(|| {
            format!(
                "cannot create immutable blind snapshot {}; choose a new path",
                output.display()
            )
        })?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    Ok(digest)
}

fn verify_snapshot(
    root: &Path,
    ai_run: &str,
    snapshot_path: &Path,
    expected_digest: &str,
    expected_policy: &str,
    expected_model: &str,
) -> Result<()> {
    let expected_digest = normalize_digest(expected_digest)?;
    let bytes = fs::read(snapshot_path)
        .with_context(|| format!("cannot read blind snapshot {}", snapshot_path.display()))?;
    let actual_digest = sha256(&bytes);
    if actual_digest != expected_digest {
        return Err(anyhow!(
            "blind snapshot SHA-256 mismatch: expected {expected_digest}, got {actual_digest}"
        ));
    }

    let frozen: BlindSnapshot = serde_json::from_slice(&bytes)
        .with_context(|| format!("invalid blind snapshot JSON: {}", snapshot_path.display()))?;
    if frozen.schema_version != SNAPSHOT_SCHEMA_VERSION {
        return Err(anyhow!(
            "unsupported blind snapshot schema version {}",
            frozen.schema_version
        ));
    }
    if frozen.ai_run != ai_run {
        return Err(anyhow!(
            "blind snapshot AI run differs: expected {}, got {}",
            frozen.ai_run,
            ai_run
        ));
    }
    if frozen.policy_version != expected_policy || frozen.model_version != expected_model {
        return Err(anyhow!(
            "blind snapshot was produced by policy/model {}/{}, current evaluator expects {}/{}",
            frozen.policy_version,
            frozen.model_version,
            expected_policy,
            expected_model
        ));
    }

    let current = capture_snapshot(root, ai_run, expected_policy, expected_model)?;
    ensure_snapshot_unchanged(&frozen, &current)
}

fn capture_snapshot(
    root: &Path,
    ai_run: &str,
    expected_policy: &str,
    expected_model: &str,
) -> Result<BlindSnapshot> {
    let source_dir = required_child_dir(root, "source")?;
    let ai_dir = required_child_dir(root, ai_run)?;
    let source_names = image_names(&source_dir)?;
    if source_names.is_empty() {
        return Err(anyhow!("blind dataset source contains no supported images"));
    }
    ensure_same_image_names(&source_names, &ai_dir, ai_run)?;
    ensure_source_has_no_sidecars(&source_dir)?;

    let mut sidecar_count = 0usize;
    let mut entries = Vec::with_capacity(source_names.len());
    for file in source_names {
        let source_bytes = fs::read(source_dir.join(&file))?;
        if source_bytes != fs::read(ai_dir.join(&file))? {
            return Err(anyhow!("blind dataset image bytes differ for {file}"));
        }

        let sidecar_path = ai_dir.join(format!("{file}.rrdata"));
        let (ai_rating, sidecar_present, ai_sidecar_sha256) = match fs::read(&sidecar_path) {
            Ok(sidecar_bytes) => {
                sidecar_count += 1;
                let rating = read_rating(&sidecar_path, true)?
                    .ok_or_else(|| anyhow!("AI sidecar is missing for {file}"))?;
                if rating.source.as_deref() != Some("ai") {
                    return Err(anyhow!("saved AI source is invalid for {file}"));
                }
                if rating.policy_version.as_deref() != Some(expected_policy)
                    || rating.model_version.as_deref() != Some(expected_model)
                {
                    return Err(anyhow!(
                        "saved AI policy/model is invalid for {file}: {:?}/{:?}, expected {expected_policy}/{expected_model}",
                        rating.policy_version,
                        rating.model_version
                    ));
                }
                (rating.rating, true, Some(sha256(&sidecar_bytes)))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (0, false, None),
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("cannot read {}", sidecar_path.display()));
            }
        };
        entries.push(BlindEntry {
            file,
            image_sha256: sha256(&source_bytes),
            ai_rating,
            sidecar_present,
            ai_sidecar_sha256,
        });
    }
    if sidecar_count == 0 {
        return Err(anyhow!(
            "blind AI run contains no persisted AI sidecars; the run cannot be authenticated"
        ));
    }

    Ok(BlindSnapshot {
        schema_version: SNAPSHOT_SCHEMA_VERSION,
        ai_run: ai_run.to_string(),
        policy_version: expected_policy.to_string(),
        model_version: expected_model.to_string(),
        entries,
    })
}

fn ensure_snapshot_unchanged(frozen: &BlindSnapshot, current: &BlindSnapshot) -> Result<()> {
    if frozen.entries.len() != current.entries.len() {
        return Err(anyhow!(
            "blind dataset count changed after freeze: expected {}, got {}",
            frozen.entries.len(),
            current.entries.len()
        ));
    }
    for (expected, actual) in frozen.entries.iter().zip(&current.entries) {
        if expected != actual {
            return Err(anyhow!(
                "blind AI input or result changed after freeze near {}",
                expected.file
            ));
        }
    }
    Ok(())
}

fn normalize_digest(value: &str) -> Result<String> {
    let value = value.trim().to_ascii_lowercase();
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(anyhow!("blind snapshot SHA-256 must contain 64 hex digits"));
    }
    Ok(value)
}

fn sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::json;
    use tempfile::tempdir;

    use super::*;

    const POLICY: &str = "test-policy-1";
    const MODEL: &str = "test-model-1";

    fn fixture() -> (tempfile::TempDir, PathBuf) {
        let temp = tempdir().unwrap();
        let root = temp.path().join("blind");
        let source = root.join("source");
        let ai = root.join("ai-run-v001");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&ai).unwrap();
        fs::write(source.join("portrait.jpg"), b"same-image").unwrap();
        fs::write(ai.join("portrait.jpg"), b"same-image").unwrap();
        write_ai_sidecar(&ai.join("portrait.jpg.rrdata"), 4);
        (temp, root)
    }

    fn write_ai_sidecar(path: &Path, rating: u8) {
        fs::write(
            path,
            serde_json::to_vec_pretty(&json!({
                "rating": rating,
                "featureData": {
                    "smartCullingV2": {
                        "source": "ai",
                        "rating": rating,
                        "policyVersion": POLICY,
                        "modelVersion": MODEL,
                    }
                }
            }))
            .unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn freeze_rejects_visible_manual_reference() {
        let (temp, root) = fixture();
        fs::create_dir(root.join("manual-reference")).unwrap();
        let output = temp.path().join("snapshot.json");

        let error = freeze_snapshot(&root, "ai-run-v001", &output, POLICY, MODEL).unwrap_err();

        assert!(error.to_string().contains("manual-reference"));
        assert!(!output.exists());
    }

    #[test]
    fn freeze_refuses_to_overwrite_snapshot() {
        let (temp, root) = fixture();
        let output = temp.path().join("snapshot.json");
        fs::write(&output, b"keep-me").unwrap();

        let error = freeze_snapshot(&root, "ai-run-v001", &output, POLICY, MODEL).unwrap_err();

        assert!(error.to_string().contains("choose a new path"));
        assert_eq!(fs::read(output).unwrap(), b"keep-me");
    }

    #[test]
    fn verify_detects_sidecar_change_after_freeze() {
        let (temp, root) = fixture();
        let output = temp.path().join("snapshot.json");
        let digest = freeze_snapshot(&root, "ai-run-v001", &output, POLICY, MODEL).unwrap();
        write_ai_sidecar(&root.join("ai-run-v001/portrait.jpg.rrdata"), 2);

        let error =
            verify_snapshot(&root, "ai-run-v001", &output, &digest, POLICY, MODEL).unwrap_err();

        assert!(error.to_string().contains("changed after freeze"));
    }

    #[test]
    fn verify_detects_image_change_after_freeze() {
        let (temp, root) = fixture();
        let output = temp.path().join("snapshot.json");
        let digest = freeze_snapshot(&root, "ai-run-v001", &output, POLICY, MODEL).unwrap();
        fs::write(root.join("source/portrait.jpg"), b"changed-image").unwrap();
        fs::write(root.join("ai-run-v001/portrait.jpg"), b"changed-image").unwrap();

        let error =
            verify_snapshot(&root, "ai-run-v001", &output, &digest, POLICY, MODEL).unwrap_err();

        assert!(error.to_string().contains("changed after freeze"));
    }

    #[test]
    fn verify_rejects_unrecorded_snapshot_digest() {
        let (temp, root) = fixture();
        let output = temp.path().join("snapshot.json");
        freeze_snapshot(&root, "ai-run-v001", &output, POLICY, MODEL).unwrap();

        let error = verify_snapshot(
            &root,
            "ai-run-v001",
            &output,
            &"0".repeat(64),
            POLICY,
            MODEL,
        )
        .unwrap_err();

        assert!(error.to_string().contains("SHA-256 mismatch"));
    }

    #[test]
    fn freeze_rejects_wrong_policy_version() {
        let (temp, root) = fixture();
        let output = temp.path().join("snapshot.json");

        let error =
            freeze_snapshot(&root, "ai-run-v001", &output, "different-policy", MODEL).unwrap_err();

        assert!(error.to_string().contains("policy/model is invalid"));
        assert!(!output.exists());
    }

    #[test]
    fn verify_accepts_manual_reveal_without_reading_it() {
        let (temp, root) = fixture();
        let output = temp.path().join("snapshot.json");
        let digest = freeze_snapshot(&root, "ai-run-v001", &output, POLICY, MODEL).unwrap();
        fs::create_dir(root.join("manual-reference")).unwrap();
        fs::write(root.join("manual-reference/private-answer.txt"), b"unread").unwrap();

        verify_snapshot(&root, "ai-run-v001", &output, &digest, POLICY, MODEL).unwrap();
    }
}
