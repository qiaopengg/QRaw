//! macOS Vision quality signals used only by the Debug calibration path.
//!
//! The system observations are deliberately kept separate from the frozen eye
//! contract and from production scoring. Human detection may only suppress an
//! unsafe "no people" conclusion. Face capture quality may contribute a
//! confidence-capped, calibration-only person-clarity score; holistic
//! aesthetics remains observation-only until independent task validation.

mod macos;

use std::io::Cursor;

use anyhow::{Context, Result, anyhow};
use image::{DynamicImage, GenericImageView, ImageFormat};

use super::types::FaceResult;

const VISION_INPUT_DIMENSION: u32 = 1600;

#[derive(Clone, Debug, Default)]
pub(super) struct VisionQualitySignals {
    /// Apple Vision's native `[-1, 1]` holistic desirability score.
    pub(super) aesthetics_score: Option<f32>,
    pub(super) is_utility: Option<bool>,
    /// Same order as the YuNet faces supplied by the caller.
    pub(super) face_capture_qualities: Vec<Option<f32>>,
    pub(super) human_count: usize,
    pub(super) max_human_confidence: Option<f32>,
    pub(super) unavailable_reason: Option<String>,
}

impl VisionQualitySignals {
    pub(super) fn unavailable(face_count: usize, reason: impl Into<String>) -> Self {
        Self {
            face_capture_qualities: vec![None; face_count],
            unavailable_reason: Some(reason.into()),
            ..Self::default()
        }
    }
}

pub(super) fn preflight_calibration_models() -> Result<()> {
    if !macos::is_supported() {
        return Err(anyhow!(
            "Apple Vision aesthetics requires macOS 15 or newer"
        ));
    }
    let image = DynamicImage::ImageRgb8(image::RgbImage::from_fn(256, 256, |x, y| {
        image::Rgb([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8])
    }));
    let signals = analyze_calibration_image(&image, &[])?;
    if signals.aesthetics_score.is_some_and(f32::is_finite) {
        Ok(())
    } else {
        Err(anyhow!(
            "Apple Vision aesthetics preflight returned no finite score"
        ))
    }
}

pub(super) fn observe_calibration_image(
    image: &DynamicImage,
    faces: &[FaceResult],
) -> VisionQualitySignals {
    let face_boxes = faces.iter().map(|face| face.bbox).collect::<Vec<_>>();
    analyze_calibration_image(image, &face_boxes).unwrap_or_else(|error| {
        log::warn!("Smart-culling Apple Vision observation failed: {error}");
        VisionQualitySignals::unavailable(
            face_boxes.len(),
            format!("apple_vision_quality_failed:{error}"),
        )
    })
}

fn analyze_calibration_image(
    image: &DynamicImage,
    face_boxes: &[[f32; 4]],
) -> Result<VisionQualitySignals> {
    if !macos::is_supported() {
        return Ok(VisionQualitySignals::unavailable(
            face_boxes.len(),
            "apple_vision_quality_requires_macos_15",
        ));
    }
    let (width, height) = image.dimensions();
    if width == 0 || height == 0 {
        return Ok(VisionQualitySignals::unavailable(
            face_boxes.len(),
            "apple_vision_quality_empty_image",
        ));
    }
    let resized = image.thumbnail(VISION_INPUT_DIMENSION, VISION_INPUT_DIMENSION);
    let mut cursor = Cursor::new(Vec::new());
    resized
        .write_to(&mut cursor, ImageFormat::Png)
        .context("failed to encode Apple Vision calibration input")?;
    macos::perform_requests(cursor.get_ref(), face_boxes, width, height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "explicit local macOS Vision hardware POC"]
    fn vision_framework_returns_a_finite_aesthetics_score() {
        preflight_calibration_models().unwrap();
    }
}
