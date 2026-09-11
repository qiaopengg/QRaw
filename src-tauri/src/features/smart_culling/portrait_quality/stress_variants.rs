//! Deterministic in-memory probes, never exported as photographs or human truth.

use anyhow::{Result, anyhow};
use image::{DynamicImage, GenericImageView, Rgb, RgbImage, imageops};
use serde::{Deserialize, Serialize};

use crate::features::smart_culling::analysis::crop_pixel_bbox;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct Case {
    pub id: String,
    pub kind: String,
    pub value: f32,
}

pub(super) fn make(
    original: &DynamicImage,
    bbox: [f32; 4],
    case: &Case,
) -> Result<(DynamicImage, [f32; 4])> {
    make_with_seed(original, bbox, case, 0x5145_5741)
}

pub(super) fn noise_offset(seed: &mut u32, amplitude: i32) -> i32 {
    *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    ((*seed >> 8) % (2 * amplitude + 1) as u32) as i32 - amplitude
}

pub(super) fn noise_pixel(seed: &mut u32, amplitude: i32, shared: bool) -> [i32; 3] {
    let shared = shared.then(|| noise_offset(seed, amplitude));
    [0, 1, 2].map(|_| shared.unwrap_or_else(|| noise_offset(seed, amplitude)))
}

pub(super) fn make_with_seed(
    original: &DynamicImage,
    bbox: [f32; 4],
    case: &Case,
    mut seed: u32,
) -> Result<(DynamicImage, [f32; 4])> {
    let mut mapped = bbox;
    if case.kind == "original" {
        return Ok((original.clone(), mapped));
    }
    if case.kind == "box_shift" {
        mapped[0] += mapped[2] * case.value;
        return Ok((original.clone(), mapped));
    }
    if case.kind == "resize" {
        let limit = case.value as u32;
        let output = if original.width().max(original.height()) <= limit {
            original.clone()
        } else {
            original.thumbnail(limit, limit)
        };
        let sx = output.width() as f32 / original.width() as f32;
        let sy = output.height() as f32 / original.height() as f32;
        mapped = [bbox[0] * sx, bbox[1] * sy, bbox[2] * sx, bbox[3] * sy];
        return Ok((output, mapped));
    }
    let crop = crop_pixel_bbox(original, bbox).ok_or_else(|| anyhow!("missing face crop"))?;
    let (width, height) = original.dimensions();
    let mut output = original.to_rgb8();
    let changed = match case.kind.as_str() {
        "background" => {
            output = RgbImage::from_fn(width, height, |x, y| {
                let value = if case.value < 0.0 {
                    if (x + y) % 2 == 0 { 0 } else { 255 }
                } else {
                    case.value as u8
                };
                Rgb([value; 3])
            });
            crop
        }
        "blur" => crop.blur(case.value),
        "unsharpen" => crop.unsharpen(case.value, 0),
        "noise" | "noise_sharpen" | "shared_noise" => {
            let mut noisy = crop.to_rgb8();
            for pixel in noisy.pixels_mut() {
                let offsets =
                    noise_pixel(&mut seed, case.value as i32, case.kind == "shared_noise");
                for (channel, offset) in pixel.0.iter_mut().zip(offsets) {
                    *channel = (i32::from(*channel) + offset).clamp(0, 255) as u8;
                }
            }
            let noisy = DynamicImage::ImageRgb8(noisy);
            if case.kind == "noise_sharpen" {
                noisy.unsharpen(1.2, 0)
            } else {
                noisy
            }
        }
        "darken" => {
            DynamicImage::ImageRgb8(RgbImage::from_fn(crop.width(), crop.height(), |x, y| {
                let pixel = crop.get_pixel(x, y);
                Rgb([0, 1, 2].map(|channel| (f32::from(pixel[channel]) * case.value).round() as u8))
            }))
        }
        _ => return Err(anyhow!("unknown stress case: {}", case.kind)),
    };
    imageops::replace(
        &mut output,
        &changed.to_rgb8(),
        bbox[0].floor() as i64,
        bbox[1].floor() as i64,
    );
    Ok((DynamicImage::ImageRgb8(output), mapped))
}

#[test]
fn background_isolation_and_deterministic_noise_preserve_the_source() {
    let original = DynamicImage::ImageRgb8(RgbImage::from_pixel(100, 80, Rgb([100; 3])));
    let bbox = [20.3, 10.5, 50.2, 40.2];
    let background = Case {
        id: "test".into(),
        kind: "background".into(),
        value: -1.0,
    };
    let (changed, _) = make(&original, bbox, &background).unwrap();
    assert_eq!(
        crop_pixel_bbox(&changed, bbox).unwrap(),
        crop_pixel_bbox(&original, bbox).unwrap()
    );
    assert_ne!(changed.get_pixel(0, 0), original.get_pixel(0, 0));
    let noise = Case {
        id: "test".into(),
        kind: "noise".into(),
        value: 24.0,
    };
    let (left, _) = make(&original, bbox, &noise).unwrap();
    let (right, _) = make(&original, bbox, &noise).unwrap();
    assert_eq!(left, right);
    assert_ne!(left, original);
    assert_eq!(left.get_pixel(0, 0), original.get_pixel(0, 0));
    assert_eq!(original.get_pixel(30, 30).0, [100, 100, 100, 255]);
}

#[test]
fn resize_maps_geometry_and_never_upsamples() {
    let image = DynamicImage::new_rgb8(200, 100);
    let mut case = Case {
        id: "test".into(),
        kind: "resize".into(),
        value: 100.0,
    };
    let (small, bbox) = make(&image, [20.0, 10.0, 60.0, 40.0], &case).unwrap();
    assert_eq!(small.dimensions(), (100, 50));
    assert_eq!(bbox, [10.0, 5.0, 30.0, 20.0]);
    case.value = 400.0;
    assert_eq!(
        make(&image, bbox, &case).unwrap().0.dimensions(),
        image.dimensions()
    );
}
