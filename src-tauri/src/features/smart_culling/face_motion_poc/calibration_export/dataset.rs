use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use serde::Deserialize;

use super::{IDENTITY_OUTPUT_ENV, IMAGE_DIR_ENV, LABELS_ENV, MANIFEST_ENV, OUTPUT_ENV};

#[derive(Deserialize)]
struct LabelDocument {
    labels: Vec<ManualLabel>,
}

#[derive(Deserialize)]
pub(super) struct ManualLabel {
    pub sample_id: String,
    pub image_ref: String,
    pub expression_label: u8,
}

#[derive(Deserialize)]
struct BatchManifest {
    unique_samples: Vec<ManifestSample>,
}

#[derive(Deserialize)]
pub(super) struct ManifestSample {
    pub sample_id: String,
    pub anonymous_ref: String,
    pub representative_source_ref: String,
    pub source_family_ids: Vec<String>,
    pub known_source_types: Vec<String>,
}

pub(super) struct DatasetInput {
    pub image_dir: PathBuf,
    pub labels: Vec<ManualLabel>,
    pub manifest_by_id: HashMap<String, ManifestSample>,
    pub output_path: PathBuf,
    pub identity_output_path: PathBuf,
}

pub(super) fn load() -> Result<DatasetInput> {
    let image_dir = required_dir(IMAGE_DIR_ENV)?;
    let labels_path = required_file(LABELS_ENV)?;
    let output_path = required_new_output(OUTPUT_ENV)?;
    let identity_output_path = required_new_output(IDENTITY_OUTPUT_ENV)?;

    let labels: LabelDocument = serde_json::from_slice(&fs::read(&labels_path)?)
        .with_context(|| format!("cannot parse labels {}", labels_path.display()))?;
    let manifest_by_id = load_manifest(&labels.labels)?;
    if labels.labels.len() != manifest_by_id.len() {
        return Err(anyhow!(
            "label/manifest count mismatch: {} labels, {} manifest samples",
            labels.labels.len(),
            manifest_by_id.len()
        ));
    }

    Ok(DatasetInput {
        image_dir,
        labels: labels.labels,
        manifest_by_id,
        output_path,
        identity_output_path,
    })
}

fn load_manifest(labels: &[ManualLabel]) -> Result<HashMap<String, ManifestSample>> {
    let Some(manifest_value) = std::env::var_os(MANIFEST_ENV) else {
        return Ok(labels
            .iter()
            .map(|label| {
                (
                    label.sample_id.clone(),
                    ManifestSample {
                        sample_id: label.sample_id.clone(),
                        anonymous_ref: label.image_ref.clone(),
                        representative_source_ref: label.image_ref.clone(),
                        source_family_ids: vec!["legacy-batch-without-manifest".to_string()],
                        known_source_types: vec!["unknown".to_string()],
                    },
                )
            })
            .collect());
    };
    let manifest_path = PathBuf::from(manifest_value);
    if !manifest_path.is_absolute() || !manifest_path.is_file() {
        return Err(anyhow!(
            "{MANIFEST_ENV} must be an absolute file when provided"
        ));
    }
    let manifest: BatchManifest = serde_json::from_slice(&fs::read(&manifest_path)?)
        .with_context(|| format!("cannot parse manifest {}", manifest_path.display()))?;
    Ok(manifest
        .unique_samples
        .into_iter()
        .map(|sample| (sample.sample_id.clone(), sample))
        .collect())
}

fn required_value(name: &str) -> Result<String> {
    std::env::var(name).map_err(|_| anyhow!("missing required environment variable {name}"))
}

fn required_dir(name: &str) -> Result<PathBuf> {
    let directory = PathBuf::from(required_value(name)?);
    if !directory.is_absolute() || !directory.is_dir() {
        return Err(anyhow!("{name} must be an absolute directory"));
    }
    Ok(directory)
}

fn required_file(name: &str) -> Result<PathBuf> {
    let file = PathBuf::from(required_value(name)?);
    if !file.is_absolute() || !file.is_file() {
        return Err(anyhow!("{name} must be an absolute file"));
    }
    Ok(file)
}

fn required_new_output(name: &str) -> Result<PathBuf> {
    let output = PathBuf::from(required_value(name)?);
    if !output.is_absolute()
        || output.exists()
        || output.parent().is_none_or(|parent| !parent.is_dir())
    {
        return Err(anyhow!(
            "{name} must be a new absolute file inside an existing directory"
        ));
    }
    Ok(output)
}
