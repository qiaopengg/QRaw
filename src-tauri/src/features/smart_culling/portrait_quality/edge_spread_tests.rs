use super::*;
use image::{GrayImage, Luma};

use super::super::stress_variants::{Case, make_with_seed};

fn step(width: f64) -> DynamicImage {
    DynamicImage::ImageLuma8(GrayImage::from_fn(256, 256, |x, _| {
        Luma(
            [
                (20.0 + 215.0 * (1.0 + ((f64::from(x) - 127.5) / width).tanh()) / 2.0).round()
                    as u8,
            ],
        )
    }))
}

#[test]
fn known_edge_widths_increase_with_blur_and_rotate_with_the_pixels() {
    let mut previous = 0.0;
    for width in [1.0, 3.0, 6.0] {
        let input = step(width);
        let measured = measure(&input).unwrap();
        let actual = measured.width_p25.unwrap();
        assert!(actual > previous + 2.0, "width {width}: {actual}");
        assert!((measure(&input.rotate90()).unwrap().width_p25.unwrap() - actual).abs() < 1e-3);
        previous = actual;
    }
}

#[test]
fn known_steps_preserve_width_under_seeded_noise_and_sharpening() {
    let mut failures = Vec::new();
    for width in [1.0, 3.0, 6.0] {
        let input = step(width);
        let reference = measure(&input).unwrap().width_p25.unwrap();
        for seed in [0x51455741, 1, 20260910] {
            for (kind, value) in [
                ("noise", 8.0),
                ("noise", 24.0),
                ("noise", 48.0),
                ("noise_sharpen", 24.0),
            ] {
                let case = Case {
                    id: kind.into(),
                    kind: kind.into(),
                    value,
                };
                let (noisy, _) =
                    make_with_seed(&input, [0.0, 0.0, 256.0, 256.0], &case, seed).unwrap();
                let observed = measure(&noisy).unwrap().width_p25.unwrap();
                if (observed - reference).abs() >= 1.5 {
                    let fixed = edge_profile::measure(
                        &linear_response::from_rgb8(&noisy),
                        &Edge {
                            x: 128,
                            y: 128,
                            normal: [1.0, 0.0],
                            strength: 1.0,
                        },
                    )
                    .ok()
                    .map(|profile| profile.width);
                    failures.push(format!("width={width}, seed={seed}, {kind} {value}: {reference} => {observed}; known center {fixed:?}"));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn flat_low_contrast_and_noise_cannot_supply_focus_evidence() {
    let flat = DynamicImage::ImageLuma8(GrayImage::from_pixel(256, 256, Luma([128])));
    for seed in [0x51455741, 1, 20260910] {
        for (kind, value) in [("original", 0.0), ("noise", 48.0), ("noise_sharpen", 24.0)] {
            let case = Case {
                id: kind.into(),
                kind: kind.into(),
                value,
            };
            let (input, _) = make_with_seed(&flat, [0.0, 0.0, 256.0, 256.0], &case, seed).unwrap();
            assert!(
                measure(&input).unwrap().width_p25.is_none(),
                "{seed} {kind}"
            );
        }
    }
    let low = DynamicImage::ImageLuma8(GrayImage::from_fn(256, 256, |x, _| {
        Luma([if x < 128 { 120 } else { 130 }])
    }));
    assert!(measure(&low).unwrap().width_p25.is_none());
    assert!(measure(&DynamicImage::new_luma8(40, 40)).is_none());
}
