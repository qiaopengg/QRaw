//! Synthetic signal/noise pairs only; no photographs or category labels are read.

use anyhow::{Result, bail, ensure};
use image::{DynamicImage, Luma, Rgb, Rgb32FImage, RgbImage};
use serde::{Deserialize, Serialize};

use super::linear_response::{FloatGray, from_rgb8};
use super::stress_variants::{self, noise_offset};

#[derive(Clone, Deserialize, Serialize)]
pub(super) struct Case {
    pub id: String,
    pub panel: String,
    pub side: u32,
    pub amplitude: i32,
    pub seed: u32,
    pub processing: String,
}

pub(super) struct Pair {
    pub clean: FloatGray,
    pub observed: FloatGray,
    pub original: FloatGray,
    pub clipped_channel_fraction: f64,
    pub source: &'static str,
}

pub(super) fn make(case: &Case) -> Result<Pair> {
    ensure!([128, 256, 512].contains(&case.side));
    ensure!([8, 24, 48].contains(&case.amplitude));
    let width = match case.panel.as_str() {
        "flat" | "checker" => 0.0,
        "step1" => 1.0,
        "step3" => 3.0,
        "step6" => 6.0,
        _ => bail!("unknown panel"),
    };
    let rgb = RgbImage::from_fn(case.side, case.side, |x, y| {
        let value = match case.panel.as_str() {
            "flat" => 128,
            "checker" => {
                if (x / 7 + y / 9) % 2 == 0 {
                    40
                } else {
                    200
                }
            }
            _ => (20.0
                + 215.0 * (1.0 + ((f64::from(x) - f64::from(case.side - 1) / 2.0) / width).tanh())
                    / 2.0)
                .round() as u8,
        };
        Rgb([value; 3])
    });
    let original = DynamicImage::ImageRgb8(rgb);
    let clean_float = from_rgb8(&original);
    let mut seed = case.seed;
    let mut clipped = 0_u64;
    let float_rgb = Rgb32FImage::from_fn(case.side, case.side, |x, y| {
        let clean = clean_float.get_pixel(x, y)[0];
        Rgb([0, 1, 2].map(|_| {
            let value = clean + f64::from(noise_offset(&mut seed, case.amplitude));
            clipped += u64::from(!(0.0..=255.0).contains(&value));
            value as f32 / 255.0
        }))
    });
    // Use the host's color-space-aware conversion for both paths. Pixel::to_luma
    // is a different legacy conversion, and RGB8 rounds rather than floors.
    let float_luma = DynamicImage::ImageRgb32F(float_rgb).to_luma32f();
    let float_noise = FloatGray::from_fn(case.side, case.side, |x, y| {
        Luma([f64::from(float_luma.get_pixel(x, y)[0]) * 255.0])
    });
    let clipped_channel_fraction = clipped as f64 / (f64::from(case.side).powi(2) * 3.0);
    if case.processing == "unclipped_float" {
        return Ok(Pair {
            clean: clean_float.clone(),
            original: clean_float,
            observed: float_noise,
            clipped_channel_fraction,
            source: "syntheticUnclippedRgb32fMeasuredF64",
        });
    }
    let noisy = stress_variants::make_with_seed(
        &original,
        [0.0, 0.0, case.side as f32, case.side as f32],
        &stress_variants::Case {
            id: "noise".into(),
            kind: "noise".into(),
            value: case.amplitude as f32,
        },
        case.seed,
    )?
    .0;
    let process = |image: &DynamicImage| -> Result<DynamicImage> {
        Ok(match case.processing.as_str() {
            "rgb8" => image.clone(),
            "rgb8_sharpen" => image.unsharpen(1.2, 0),
            "rgb8_resize256" | "rgb8_resize128" => {
                ensure!(case.side == 512, "resize reference must start at 512");
                let side = if case.processing == "rgb8_resize256" {
                    256
                } else {
                    128
                };
                image.thumbnail(side, side)
            }
            _ => bail!("unknown processing"),
        })
    };
    Ok(Pair {
        clean: from_rgb8(&process(&original)?),
        observed: from_rgb8(&process(&noisy)?),
        original: clean_float,
        clipped_channel_fraction,
        source: "syntheticProcessedRgb8",
    })
}

#[test]
fn unchanged_pixels_and_fixed_seed_match_the_existing_noise_generator() {
    let case = Case {
        id: "test".into(),
        panel: "flat".into(),
        side: 128,
        amplitude: 48,
        seed: 0x5145_5741,
        processing: "rgb8".into(),
    };
    let pair = make(&case).unwrap();
    assert_eq!(pair.clean, pair.original);
    assert_eq!(pair.clipped_channel_fraction, 0.0);
    let floating = make(&Case {
        processing: "unclipped_float".into(),
        ..case.clone()
    })
    .unwrap();
    for (rgb, float) in pair.observed.pixels().zip(floating.observed.pixels()) {
        // Current DynamicImage conversion uses f32 color coefficients and rounds
        // RGB8 to the nearest level. The small allowance covers f32 arithmetic.
        assert!((rgb[0] - float[0]).abs() <= 0.5001);
    }
    assert_eq!(pair.observed, make(&case).unwrap().observed);
    assert_ne!(
        pair.observed,
        make(&Case { seed: 1, ..case }).unwrap().observed
    );
}
