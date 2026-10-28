//! Keep the host decoder's EXIF cache writes away from portrait source sidecars.

use std::{fs, path::Path};

use anyhow::Context;
use image::DynamicImage;
use serde_json::Value;

use crate::app_settings::AppSettings;
use crate::image_loader::{load_base_image_from_bytes, prepare_base_image_cow};

pub(super) fn decode_portrait_snapshot(
    bytes: &[u8],
    source_path: &str,
    adjustments: &Value,
    settings: &AppSettings,
) -> anyhow::Result<DynamicImage> {
    // The host has no read-only decoder switch. A scoped source snapshot lets
    // us reuse its preprocessing, EXIF orientation and RAW fallback unchanged;
    // any EXIF cache writes/migration can only touch this private directory.
    let directory = tempfile::Builder::new()
        .prefix("qraw-portrait-input-")
        .tempdir()
        .context("Could not isolate portrait decoding")?;
    let snapshot = directory
        .path()
        .join("source")
        .with_extension(Path::new(source_path).extension().unwrap_or_default());
    fs::write(&snapshot, bytes).context("Could not write the portrait input snapshot")?;
    let decoded = load_base_image_from_bytes(
        bytes,
        snapshot
            .to_str()
            .context("Portrait snapshot path is not UTF-8")?,
        false,
        settings,
        None,
    );
    directory
        .close()
        .context("Could not remove the portrait input snapshot")?;
    let base = decoded?;
    // Cached effects are keyed by the real source path. Only decoding needs
    // isolation; preparing the already decoded pixels does not write EXIF.
    Ok(prepare_base_image_cow(&base, source_path, adjustments)?.into_owned())
}

#[cfg(test)]
#[path = "decode_snapshot_tests.rs"]
mod tests;
