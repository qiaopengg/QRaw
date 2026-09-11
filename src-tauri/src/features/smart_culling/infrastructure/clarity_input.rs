//! A native-pixel clarity plane aligned by lossless geometry only.

use image::{DynamicImage, GenericImageView};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::image_loader::load_image_with_orientation;
use crate::image_processing::{
    Crop, apply_coarse_rotation, apply_crop, apply_flip, get_geometry_params_from_json,
    is_geometry_identity,
};

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ClarityInputUnavailable {
    NotRequested,
    RawSourceUnvalidated,
    ContentEdited,
    GeometryUnvalidated,
    InvalidAdjustments,
    InvalidCrop,
    DecodeFailed,
    RenderDimensionsMismatch,
}

pub(crate) struct ClarityInput {
    pixels: Result<DynamicImage, ClarityInputUnavailable>,
    pub source_sha256: Option<String>,
    pub adjustments_sha256: Option<String>,
}

impl ClarityInput {
    pub(crate) fn unavailable(reason: ClarityInputUnavailable) -> Self {
        Self {
            pixels: Err(reason),
            source_sha256: None,
            adjustments_sha256: None,
        }
    }

    pub(crate) fn from_bytes(bytes: &[u8], is_raw: bool, adjustments: &Value) -> Self {
        Self {
            pixels: Self::decode(bytes, is_raw, adjustments),
            source_sha256: Some(hex::encode(Sha256::digest(bytes))),
            adjustments_sha256: Some(hex::encode(Sha256::digest(adjustments.to_string()))),
        }
    }

    fn decode(
        bytes: &[u8],
        is_raw: bool,
        adjustments: &Value,
    ) -> Result<DynamicImage, ClarityInputUnavailable> {
        use ClarityInputUnavailable as Reason;
        if is_raw {
            return Err(Reason::RawSourceUnvalidated);
        }
        // ImageMetadata::default() uses null for an unedited photo. The host
        // treats that snapshot as identity, just like an empty object.
        if !adjustments.is_object() && !adjustments.is_null() {
            return Err(Reason::InvalidAdjustments);
        }
        for name in [
            "rotation",
            "transformDistortion",
            "transformVertical",
            "transformHorizontal",
            "transformRotate",
            "transformAspect",
            "transformScale",
            "transformXOffset",
            "transformYOffset",
        ] {
            if !adjustments[name].is_null()
                && adjustments[name].as_f64().is_none_or(|v| !v.is_finite())
            {
                return Err(Reason::InvalidAdjustments);
            }
        }
        for name in ["flipHorizontal", "flipVertical", "lensBlurEnabled"] {
            if !adjustments[name].is_null() && !adjustments[name].is_boolean() {
                return Err(Reason::InvalidAdjustments);
            }
        }
        if adjustments.get("aiPatches").is_some_and(|patches| {
            patches.as_array().is_none_or(|items| {
                items
                    .iter()
                    .any(|patch| patch["visible"].as_bool() != Some(false))
            })
        }) || adjustments["lensBlurEnabled"].as_bool() == Some(true)
        {
            return Err(Reason::ContentEdited);
        }
        // Match apply_all_transformations/apply_rotation: the host rounds to f32
        // before checking full turns. Equal dimensions cannot detect this warp.
        let rotation = adjustments["rotation"].as_f64().unwrap_or(0.0) as f32;
        if !is_geometry_identity(&get_geometry_params_from_json(adjustments))
            || !rotation.is_finite()
            || rotation % 360.0 != 0.0
            || (!adjustments["orientationSteps"].is_null()
                && adjustments["orientationSteps"]
                    .as_u64()
                    .is_none_or(|steps| steps > 3))
        {
            return Err(Reason::GeometryUnvalidated);
        }
        // Reuse the host's EXIF orientation decoder, without invoking its RAW,
        // preprocessing, patch, EXIF-persistence or GPU paths.
        let decoded = match load_image_with_orientation(bytes, None) {
            Ok(image) => DynamicImage::ImageRgb8(image.to_rgb8()),
            Err(_) => return Err(Reason::DecodeFailed),
        };
        let rotated = apply_coarse_rotation(
            decoded,
            adjustments["orientationSteps"].as_u64().unwrap_or(0) as u8,
        );
        let flipped = apply_flip(
            rotated,
            adjustments["flipHorizontal"].as_bool().unwrap_or(false),
            adjustments["flipVertical"].as_bool().unwrap_or(false),
        );
        if !adjustments["crop"].is_null() {
            let crop: Crop = serde_json::from_value(adjustments["crop"].clone())
                .map_err(|_| Reason::InvalidCrop)?;
            if [crop.x, crop.y, crop.width, crop.height]
                .iter()
                .any(|v| !v.is_finite())
                || crop.x < 0.0
                || crop.y < 0.0
                || crop.x.round() >= f64::from(flipped.width())
                || crop.y.round() >= f64::from(flipped.height())
                || crop.width.round() < 1.0
                || crop.height.round() < 1.0
            {
                return Err(Reason::InvalidCrop);
            }
        }
        Ok(apply_crop(flipped, &adjustments["crop"]).into_owned())
    }

    pub(crate) fn check_render_dimensions(&mut self, dimensions: (u32, u32)) {
        if self
            .image()
            .is_some_and(|image| image.dimensions() != dimensions)
        {
            self.pixels = Err(ClarityInputUnavailable::RenderDimensionsMismatch);
        }
    }

    pub(crate) fn image(&self) -> Option<&DynamicImage> {
        self.pixels.as_ref().ok()
    }

    pub(crate) fn unavailable_reason(&self) -> Option<ClarityInputUnavailable> {
        self.pixels.as_ref().err().copied()
    }
}

#[cfg(test)]
#[path = "clarity_input_tests.rs"]
mod tests;
