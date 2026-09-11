//! Signed intermediates of the existing RGB8 path; no floating-point unsharpen API.

use anyhow::{Result, anyhow, ensure};
use image::{DynamicImage, Luma, imageops};

use super::linear_response::FloatGray;
use super::{cross_channel_detail, stress_variants};

pub(super) fn energies(channels: &[FloatGray; 3]) -> Result<[f64; 2]> {
    Ok(cross_channel_detail::measure_channels(channels)
        .ok_or_else(|| anyhow!("invalid intermediate"))?
        .scales
        .map(|scale| scale.common_energy))
}

pub(super) fn injected(
    before: &DynamicImage,
    stage: &stress_variants::Case,
    mut seed: u32,
) -> ([FloatGray; 3], f64) {
    let mut values = cross_channel_detail::rgb_channels(before);
    let mut clipped = 0_u64;
    for y in 0..before.height() {
        for x in 0..before.width() {
            let offsets = stress_variants::noise_pixel(
                &mut seed,
                stage.value as i32,
                stage.kind == "shared_noise",
            );
            for (plane, offset) in values.iter_mut().zip(offsets) {
                let value = plane.get_pixel_mut(x, y);
                value[0] += f64::from(offset);
                clipped += u64::from(!(0.0..=255.0).contains(&value[0]));
            }
        }
    }
    (
        values,
        clipped as f64 / (f64::from(before.width()) * f64::from(before.height()) * 3.0),
    )
}

pub(super) fn unclipped_sharpen(
    before: &DynamicImage,
    actual: &DynamicImage,
) -> Result<([FloatGray; 3], f64)> {
    ensure!(before.width() == actual.width() && before.height() == actual.height());
    let rgb = before.to_rgb8();
    // image::unsharpen RGB8 uses this same generic blur, then integer 2I-B and clipping.
    let blurred = imageops::blur(&rgb, 1.2);
    let values = [0, 1, 2].map(|channel| {
        FloatGray::from_fn(rgb.width(), rgb.height(), |x, y| {
            Luma([2.0 * f64::from(rgb.get_pixel(x, y)[channel])
                - f64::from(blurred.get_pixel(x, y)[channel])])
        })
    });
    let actual = actual.to_rgb8();
    let mut clipped = 0_u64;
    for y in 0..rgb.height() {
        for x in 0..rgb.width() {
            for (channel, plane) in values.iter().enumerate() {
                let value = plane.get_pixel(x, y)[0];
                ensure!(
                    value.clamp(0.0, 255.0) as u8 == actual.get_pixel(x, y)[channel],
                    "sharpen reconstruction differs from RGB8 path"
                );
                clipped += u64::from(!(0.0..=255.0).contains(&value));
            }
        }
    }
    Ok((
        values,
        clipped as f64 / (f64::from(rgb.width()) * f64::from(rgb.height()) * 3.0),
    ))
}
