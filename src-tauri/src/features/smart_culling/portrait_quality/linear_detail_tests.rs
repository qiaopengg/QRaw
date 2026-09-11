use super::*;
use image::{GrayImage, Luma};

#[test]
fn flat_tiny_and_invalid_noise_keep_absence_distinct_from_blur() {
    let flat = DynamicImage::ImageLuma8(GrayImage::from_pixel(64, 64, Luma([128])));
    let observed = measure(&flat, 0.0).unwrap();
    assert_eq!(observed.scales[0].observed_energy, 0.0);
    assert!(observed.coarse_to_fine_ratio.is_none());
    let overcorrected = measure(&flat, 10.0).unwrap();
    assert!(
        overcorrected
            .scales
            .iter()
            .all(|scale| scale.corrected_energy < 0.0)
    );
    assert!(overcorrected.coarse_to_fine_ratio.is_none());
    for noise in [-1.0, f64::NAN, f64::INFINITY, f64::MAX] {
        assert!(measure(&flat, noise).is_none());
    }
    assert!(measure(&DynamicImage::new_luma8(8, 8), 0.0).is_none());
}

#[test]
fn analytic_white_noise_energy_matches_a_known_variance_probe() {
    let mut seed = 0x5145_5741_u32;
    let noise = DynamicImage::ImageLuma8(GrayImage::from_fn(768, 768, |_, _| {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        Luma([(128 + ((seed >> 8) % 49) as i32 - 24) as u8])
    }));
    // Discrete uniform [-a,a] has variance a(a+1)/3; no estimator or face labels.
    let measured = measure(&noise, (24.0_f64 * 25.0 / 3.0).sqrt()).unwrap();
    for scale in measured.scales {
        assert!(scale.kernel_noise_gain > 0.0);
        let relative_residual = scale.corrected_energy.abs() / scale.expected_noise_energy;
        assert!(relative_residual < 0.03, "{relative_residual}");
    }
}

#[test]
fn blur_changes_structure_and_exposure_scaling_preserves_the_ratio() {
    let pattern = DynamicImage::ImageLuma8(GrayImage::from_fn(160, 160, |x, y| {
        Luma([if (x / 7 + y / 9) % 2 == 0 { 40 } else { 200 }])
    }));
    let original = measure(&pattern, 0.0).unwrap();
    let blurred = measure(&pattern.blur(3.0), 0.0).unwrap();
    assert!(original.scales[0].corrected_energy > blurred.scales[0].corrected_energy);
    assert!(original.coarse_to_fine_ratio < blurred.coarse_to_fine_ratio);
    let gray = pattern.to_luma8();
    let dark = DynamicImage::ImageLuma8(GrayImage::from_fn(160, 160, |x, y| {
        Luma([gray.get_pixel(x, y)[0] / 2])
    }));
    let dark = measure(&dark, 0.0).unwrap();
    assert_eq!(dark.coarse_to_fine_ratio, original.coarse_to_fine_ratio);
    assert_eq!(
        dark.scales[0].corrected_energy * 4.0,
        original.scales[0].corrected_energy
    );
}
