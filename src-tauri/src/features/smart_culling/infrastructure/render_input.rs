use std::borrow::Cow;
use std::fmt;
use std::fs;

use image::{DynamicImage, GenericImageView};
use tauri::{AppHandle, State};

use super::decode_snapshot::decode_portrait_snapshot;
use super::{ClarityInput, ClarityInputUnavailable};
use crate::AppState;
use crate::adjustment_utils::{apply_all_transformations, hydrate_adjustments};
use crate::app_settings::load_settings;
use crate::as_shot::AsShot;
use crate::cache_utils::calculate_full_job_hash;
use crate::exif_processing::load_sidecar;
use crate::file_management::{parse_virtual_path, read_file_mapped};
use crate::formats::is_raw_file;
use crate::gpu_processing::{
    RenderRequest, get_or_init_gpu_context, process_and_get_dynamic_image,
};
use crate::image_loader::load_and_composite;
use crate::image_processing::{get_all_adjustments_from_json, resolve_render_options};
use crate::lut_processing::get_or_load_lut;
use crate::mask_generation::{
    MaskDefinition, generate_mask_bitmap, resolve_warped_image_for_masks,
};

pub(crate) struct AnalysisRenderInput {
    pub image: DynamicImage,
    pub clarity: ClarityInput,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RenderInputError {
    Read(String),
    Decode(String),
    Gpu(String),
    Lut(String),
    Mask(String),
    TextureLimit {
        width: u32,
        height: u32,
        max_dimension: u32,
    },
}

impl fmt::Display for RenderInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(reason) => write!(formatter, "read failed: {reason}"),
            Self::Decode(reason) => write!(formatter, "decode failed: {reason}"),
            Self::Gpu(reason) => write!(formatter, "GPU render failed: {reason}"),
            Self::Lut(reason) => write!(formatter, "LUT load failed: {reason}"),
            Self::Mask(reason) => write!(formatter, "mask data is invalid: {reason}"),
            Self::TextureLimit {
                width,
                height,
                max_dimension,
            } => write!(
                formatter,
                "image {width}x{height} exceeds GPU texture limit {max_dimension}"
            ),
        }
    }
}

pub(crate) fn render_current_state(
    path: &str,
    state: &State<'_, AppState>,
    app_handle: &AppHandle,
) -> Result<DynamicImage, RenderInputError> {
    render_analysis_input(path, state, app_handle, false).map(|input| input.image)
}

pub(crate) fn render_analysis_input(
    path: &str,
    state: &State<'_, AppState>,
    app_handle: &AppHandle,
    capture_clarity: bool,
) -> Result<AnalysisRenderInput, RenderInputError> {
    let context = get_or_init_gpu_context(state, app_handle).map_err(RenderInputError::Gpu)?;
    let (source_path, sidecar_path) = parse_virtual_path(path);
    let source_path_string = source_path.to_string_lossy().to_string();
    let is_raw = is_raw_file(&source_path_string);
    let settings = load_settings(app_handle.clone()).unwrap_or_default();

    let mut adjustments = load_sidecar(&sidecar_path).adjustments;
    hydrate_adjustments(state, &mut adjustments);

    // Both planes use the same bytes and sidecar snapshot. No second file read
    // after GPU work, which could silently refer to a different edit/source.
    let decode = |bytes: &[u8]| {
        let base = if capture_clarity {
            decode_portrait_snapshot(bytes, &source_path_string, &adjustments, &settings)?
        } else {
            load_and_composite(
                bytes,
                &source_path_string,
                &adjustments,
                false,
                &settings,
                None,
            )?
        };
        let clarity = if capture_clarity {
            ClarityInput::from_bytes(bytes, is_raw, &adjustments)
        } else {
            ClarityInput::unavailable(ClarityInputUnavailable::NotRequested)
        };
        let as_shot = if is_raw {
            crate::raw_processing::read_as_shot(bytes).unwrap_or_else(AsShot::reference)
        } else {
            AsShot::reference()
        };
        Ok::<_, anyhow::Error>((base, clarity, as_shot))
    };
    let (base_image, mut clarity, as_shot) = match read_file_mapped(&source_path) {
        Ok(mapped) => {
            if capture_clarity {
                // Own the snapshot: an mmap alone can change between the two decoders.
                decode(&mapped.to_vec())
            } else {
                decode(&mapped)
            }
        }
        Err(_) => {
            let bytes = fs::read(&source_path)
                .map_err(|error| RenderInputError::Read(error.to_string()))?;
            decode(&bytes)
        }
    }
    .map_err(|error| RenderInputError::Decode(error.to_string()))?;

    let (transformed, crop_offset) =
        apply_all_transformations(Cow::Borrowed(&base_image), &adjustments);
    let (width, height) = transformed.dimensions();
    ensure_texture_dimensions(width, height, context.limits.max_texture_dimension_2d)?;

    let mask_definitions: Vec<MaskDefinition> = match adjustments.get("masks") {
        Some(masks) => serde_json::from_value(masks.clone())
            .map_err(|error| RenderInputError::Mask(error.to_string()))?,
        None => Vec::new(),
    };
    let warped_image = resolve_warped_image_for_masks(state, &adjustments, &mask_definitions);
    let mask_bitmaps = mask_definitions
        .iter()
        .filter_map(|definition| {
            generate_mask_bitmap(
                definition,
                width,
                height,
                1.0,
                crop_offset,
                warped_image.as_deref(),
            )
        })
        .collect::<Vec<_>>();

    let render = resolve_render_options(&settings, is_raw);
    let all_adjustments = get_all_adjustments_from_json(&adjustments, is_raw, as_shot, render);
    let lut = match adjustments.get("lutPath").and_then(|value| value.as_str()) {
        Some(path) if !path.is_empty() => {
            Some(get_or_load_lut(state, path).map_err(RenderInputError::Lut)?)
        }
        _ => None,
    };

    let job_hash = analysis_job_hash(&source_path_string, &adjustments, capture_clarity);
    let image = process_and_get_dynamic_image(
        &context,
        state,
        transformed.as_ref(),
        job_hash,
        RenderRequest {
            adjustments: all_adjustments,
            mask_bitmaps: &mask_bitmaps,
            mask_cache_key: None,
            lut,
            roi: None,
        },
        "smart_culling_render_input",
    )
    .map_err(RenderInputError::Gpu)?;
    clarity.check_render_dimensions(image.dimensions());
    Ok(AnalysisRenderInput { image, clarity })
}

fn analysis_job_hash(path: &str, adjustments: &serde_json::Value, capture_clarity: bool) -> u64 {
    if capture_clarity {
        // The host cache keys only path/edits/dimensions, not source bytes or
        // preprocessing settings. Force a fresh upload for this paired snapshot.
        let snapshot_key = format!("qraw-clarity:{}:{path}", uuid::Uuid::new_v4());
        calculate_full_job_hash(&snapshot_key, adjustments)
    } else {
        calculate_full_job_hash(path, adjustments)
    }
}

fn ensure_texture_dimensions(
    width: u32,
    height: u32,
    max_dimension: u32,
) -> Result<(), RenderInputError> {
    if width > max_dimension || height > max_dimension {
        return Err(RenderInputError::TextureLimit {
            width,
            height,
            max_dimension,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paired_clarity_render_cannot_reuse_a_stale_same_path_texture() {
        let edits = serde_json::json!({});
        let original = calculate_full_job_hash("same.png", &edits);
        let first = analysis_job_hash("same.png", &edits, true);
        let second = analysis_job_hash("same.png", &edits, true);
        assert_ne!(first, original);
        assert_ne!(first, second);
        assert_eq!(analysis_job_hash("same.png", &edits, false), original);
    }

    #[test]
    fn rejects_images_that_host_renderer_would_silently_bypass() {
        assert_eq!(
            ensure_texture_dimensions(20_000, 12_000, 16_384),
            Err(RenderInputError::TextureLimit {
                width: 20_000,
                height: 12_000,
                max_dimension: 16_384,
            })
        );
    }

    #[test]
    fn accepts_images_within_the_gpu_texture_limit() {
        assert_eq!(ensure_texture_dimensions(8_192, 5_464, 16_384), Ok(()));
    }

    #[test]
    fn malformed_mask_data_has_a_dedicated_render_error() {
        let error = serde_json::from_value::<Vec<MaskDefinition>>(serde_json::json!({}))
            .map_err(|error| RenderInputError::Mask(error.to_string()))
            .unwrap_err();

        assert!(matches!(error, RenderInputError::Mask(_)));
    }
}
