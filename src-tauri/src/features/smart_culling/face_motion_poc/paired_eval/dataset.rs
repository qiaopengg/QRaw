use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::formats::is_supported_image_file;

pub(super) const AI_RUN_ENV: &str = "QRAW_PAIRED_CULLING_EVAL_AI_RUN";

pub(super) struct PairedDataset {
    pub ai_run: String,
    pub source_dir: PathBuf,
    pub output_path: PathBuf,
    pub items: BTreeMap<String, PairedItem>,
}

pub(super) struct PairedItem {
    pub file_name: String,
    pub source_path: PathBuf,
    pub image_sha256: String,
    pub manual_rating: u8,
    pub manual_source: Option<String>,
    pub saved_ai_rating: Option<u8>,
}

impl PairedDataset {
    pub(super) fn from_environment(root_env: &str, output_env: &str) -> Result<Self> {
        let root = required_dir(root_env)?;
        let manual_dir = required_child_dir(&root, "manual-reference")?;
        Self::load(root, output_env, Some(&manual_dir))
    }

    pub(super) fn prelabel_from_environment(root_env: &str, output_env: &str) -> Result<Self> {
        let root = required_dir(root_env)?;
        if root.join("manual-reference").exists() {
            return Err(anyhow!(
                "manual-reference must remain absent while expression predictions are frozen"
            ));
        }
        Self::load(root, output_env, None)
    }

    fn load(root: PathBuf, output_env: &str, manual_dir: Option<&Path>) -> Result<Self> {
        let source_dir = required_child_dir(&root, "source")?;
        let ai_run = ai_run_name()?;
        let ai_dir = required_child_dir(&root, &ai_run)?;
        let output_path = PathBuf::from(required_value(output_env)?);

        let source_names = image_names(&source_dir)?;
        if source_names.is_empty() {
            return Err(anyhow!(
                "paired dataset source contains no supported images"
            ));
        }
        if let Some(manual_dir) = manual_dir {
            ensure_same_image_names(&source_names, manual_dir, "manual-reference")?;
        }
        ensure_same_image_names(&source_names, &ai_dir, &ai_run)?;
        ensure_source_has_no_sidecars(&source_dir)?;

        let mut items = BTreeMap::new();
        for file_name in source_names {
            let source_path = source_dir.join(&file_name);
            let ai_image = ai_dir.join(&file_name);
            let source_bytes = fs::read(&source_path)?;
            if source_bytes != fs::read(&ai_image)? {
                return Err(anyhow!("paired dataset image bytes differ for {file_name}"));
            }

            let (manual_rating, manual_source) = if let Some(manual_dir) = manual_dir {
                if source_bytes != fs::read(manual_dir.join(&file_name))? {
                    return Err(anyhow!("paired dataset image bytes differ for {file_name}"));
                }
                let manual = read_rating(&manual_dir.join(format!("{file_name}.rrdata")), true)?
                    .ok_or_else(|| anyhow!("manual sidecar is missing for {file_name}"))?;
                if manual.source.as_deref() != Some("manual")
                    && !(manual.rating == 0 && manual.source.is_none())
                {
                    return Err(anyhow!(
                        "manual label source is invalid for {file_name}: {:?}",
                        manual.source
                    ));
                }
                (manual.rating, manual.source)
            } else {
                (0, None)
            };
            let saved_ai = read_rating(&ai_dir.join(format!("{file_name}.rrdata")), false)?;
            if saved_ai
                .as_ref()
                .is_some_and(|rating| rating.source.as_deref() != Some("ai"))
            {
                return Err(anyhow!("saved AI source is invalid for {file_name}"));
            }

            items.insert(
                file_name.clone(),
                PairedItem {
                    file_name,
                    source_path,
                    image_sha256: hex::encode(Sha256::digest(&source_bytes)),
                    manual_rating,
                    manual_source,
                    saved_ai_rating: saved_ai.map(|rating| rating.rating),
                },
            );
        }

        Ok(Self {
            ai_run,
            source_dir,
            output_path,
            items,
        })
    }
}

pub(super) struct StoredRating {
    pub rating: u8,
    pub source: Option<String>,
    pub policy_version: Option<String>,
    pub model_version: Option<String>,
}

pub(super) fn read_rating(path: &Path, required: bool) -> Result<Option<StoredRating>> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && !required => return Ok(None),
        Err(error) => {
            return Err(error).with_context(|| format!("cannot read {}", path.display()));
        }
    };
    let value: Value = serde_json::from_slice(&bytes)
        .with_context(|| format!("invalid sidecar JSON: {}", path.display()))?;
    let rating = value
        .pointer("/featureData/smartCullingV2/rating")
        .and_then(Value::as_u64)
        .or_else(|| value.get("rating").and_then(Value::as_u64))
        .ok_or_else(|| anyhow!("sidecar has no rating: {}", path.display()))?;
    if rating > 5 {
        return Err(anyhow!("sidecar rating is outside 0..=5"));
    }
    let source = value
        .pointer("/featureData/smartCullingV2/source")
        .and_then(Value::as_str)
        .map(str::to_string);
    let policy_version = value
        .pointer("/featureData/smartCullingV2/policyVersion")
        .and_then(Value::as_str)
        .map(str::to_string);
    let model_version = value
        .pointer("/featureData/smartCullingV2/modelVersion")
        .and_then(Value::as_str)
        .map(str::to_string);
    Ok(Some(StoredRating {
        rating: rating as u8,
        source,
        policy_version,
        model_version,
    }))
}

pub(super) fn image_names(dir: &Path) -> Result<BTreeSet<String>> {
    fs::read_dir(dir)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_ok_and(|file_type| file_type.is_file()))
        .map(|entry| entry.path())
        .filter(|path| is_supported_image_file(path))
        .map(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(str::to_string)
                .ok_or_else(|| anyhow!("dataset filename is not valid UTF-8"))
        })
        .collect()
}

pub(super) fn ensure_same_image_names(
    expected: &BTreeSet<String>,
    dir: &Path,
    name: &str,
) -> Result<()> {
    let actual = image_names(dir)?;
    if &actual != expected {
        return Err(anyhow!(
            "{name} image names differ from source: expected {}, got {}",
            expected.len(),
            actual.len()
        ));
    }
    Ok(())
}

pub(super) fn ensure_source_has_no_sidecars(source: &Path) -> Result<()> {
    if fs::read_dir(source)?.filter_map(Result::ok).any(|entry| {
        entry
            .path()
            .extension()
            .and_then(|extension| extension.to_str())
            == Some("rrdata")
    }) {
        return Err(anyhow!("paired dataset source must not contain rrdata"));
    }
    Ok(())
}

pub(super) fn required_value(name: &str) -> Result<String> {
    std::env::var(name).map_err(|_| anyhow!("missing required environment variable {name}"))
}

pub(super) fn required_dir(name: &str) -> Result<PathBuf> {
    let path = PathBuf::from(required_value(name)?);
    if !path.is_dir() {
        return Err(anyhow!("{name} is not a directory: {}", path.display()));
    }
    Ok(path)
}

pub(super) fn required_child_dir(root: &Path, name: &str) -> Result<PathBuf> {
    let path = root.join(name);
    if !path.is_dir() {
        return Err(anyhow!(
            "paired dataset directory is missing: {}",
            path.display()
        ));
    }
    Ok(path)
}

pub(super) fn ai_run_name() -> Result<String> {
    let ai_run = std::env::var(AI_RUN_ENV).unwrap_or_else(|_| "ai-run-v001".to_string());
    if ai_run.is_empty()
        || Path::new(&ai_run).components().count() != 1
        || Path::new(&ai_run)
            .file_name()
            .and_then(|name| name.to_str())
            != Some(&ai_run)
    {
        return Err(anyhow!("paired AI run must be a direct child name"));
    }
    Ok(ai_run)
}
