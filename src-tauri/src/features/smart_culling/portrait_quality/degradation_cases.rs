//! Ordered, deterministic face-crop transformations, exclusively for offline probes.

use anyhow::{Result, bail, ensure};
use image::DynamicImage;
use serde::{Deserialize, Serialize};

use super::stress_variants;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Case {
    pub id: String,
    pub reference_id: String,
    pub seed: u32,
    pub stages: Vec<stress_variants::Case>,
}

pub(super) fn apply(crop: &DynamicImage, case: &Case) -> Result<DynamicImage> {
    ensure!(case.stages.len() <= 3, "too many degradation stages");
    let bbox = [0.0, 0.0, crop.width() as f32, crop.height() as f32];
    let mut output = crop.clone();
    for stage in &case.stages {
        ensure!(stage.value.is_finite(), "non-finite degradation value");
        match stage.kind.as_str() {
            "blur" => ensure!([1.0, 2.0, 4.0].contains(&stage.value), "invalid blur sigma"),
            "noise" | "shared_noise" => ensure!(
                [8.0, 24.0, 48.0].contains(&stage.value),
                "invalid noise amplitude"
            ),
            "unsharpen" => ensure!(stage.value == 1.2, "invalid sharpening scale"),
            _ => bail!("unsupported degradation stage: {}", stage.kind),
        }
        output = stress_variants::make_with_seed(&output, bbox, stage, case.seed)?.0;
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};

    #[test]
    fn stage_order_seed_and_shared_channel_noise_are_preserved() {
        let original = DynamicImage::ImageRgb8(RgbImage::from_pixel(100, 80, Rgb([128; 3])));
        let stage = |kind: &str, value| stress_variants::Case {
            id: kind.into(),
            kind: kind.into(),
            value,
        };
        let mut case = Case {
            id: "ordered".into(),
            reference_id: "clean".into(),
            seed: 1,
            stages: vec![stage("blur", 2.0), stage("shared_noise", 24.0)],
        };
        let first = apply(&original, &case).unwrap();
        assert_eq!(first, apply(&original, &case).unwrap());
        assert!(
            first
                .to_rgb8()
                .pixels()
                .all(|p| p[0] == p[1] && p[1] == p[2])
        );
        case.stages.reverse();
        assert_ne!(first, apply(&original, &case).unwrap());
        case.stages.reverse();
        case.seed = 2;
        assert_ne!(first, apply(&original, &case).unwrap());
        assert!(original.to_rgb8().pixels().all(|p| p.0 == [128; 3]));
        case.stages[0].value = f32::NAN;
        assert!(apply(&original, &case).is_err());
    }
}
