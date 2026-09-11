use super::*;
use crate::features::smart_culling::portrait_quality::stress_variants::{Case, make_with_seed};
use image::{Rgb, RgbImage};

#[test]
fn grayscale_reuses_the_fixed_response_energy_and_flat_has_no_detail() {
    let image = DynamicImage::ImageRgb8(RgbImage::from_fn(100, 80, |x, y| {
        Rgb([if (x / 7 + y / 9) % 2 == 0 { 40 } else { 200 }; 3])
    }));
    let measured = measure(&image).unwrap();
    let reference = linear_response::planes(&linear_response::from_rgb8(&image)).unwrap();
    for (scale, response) in measured.scales.iter().zip(reference) {
        assert!((scale.common_energy - linear_response::energy(&response.values)).abs() < 1e-8);
    }
    let blurred = measure(&image.blur(2.0)).unwrap();
    assert!(blurred.scales[0].common_energy < measured.scales[0].common_energy);
    let flat = measure(&DynamicImage::new_rgb8(100, 80)).unwrap();
    assert_eq!(flat.scales[0].common_energy, 0.0);
    assert!(flat.coarse_to_fine_ratio.is_none());
    assert!(measure(&DynamicImage::new_rgb8(5, 80)).is_none());
}

#[test]
fn independent_noise_is_suppressed_but_shared_noise_is_a_counterexample() {
    let original = DynamicImage::ImageRgb8(RgbImage::from_pixel(320, 320, Rgb([128; 3])));
    for kind in ["noise", "shared_noise"] {
        let noisy = make_with_seed(
            &original,
            [0., 0., 320., 320.],
            &Case {
                id: kind.into(),
                kind: kind.into(),
                value: 48.0,
            },
            1,
        )
        .unwrap()
        .0;
        let measured = measure(&noisy).unwrap();
        for scale in measured.scales {
            let average = scale.channel_energies.iter().sum::<f64>() / 3.0;
            assert!(average > 1.0);
            if kind == "noise" {
                assert!(scale.common_energy.abs() < average * 0.05);
            } else {
                assert!((scale.common_energy - average).abs() < 1e-8);
            }
        }
    }
}

#[test]
fn opposed_color_edges_keep_signed_energy_instead_of_inventing_blur() {
    let image = DynamicImage::ImageRgb8(RgbImage::from_fn(100, 80, |x, _| {
        let red = if x % 8 < 4 { 40 } else { 200 };
        Rgb([red, 240 - red, 128])
    }));
    let measured = measure(&image).unwrap();
    assert!(
        measured
            .scales
            .iter()
            .all(|scale| scale.common_energy < 0.0)
    );
    assert!(measured.coarse_to_fine_ratio.is_none());
}
