use super::super::{
    cross_channel_detail, decomposition_trace,
    degradation_cases::{self, Case},
    mechanism_energy, noise_decomposition, stress_variants,
};
use image::{DynamicImage, Rgb, RgbImage};

fn patterned() -> DynamicImage {
    DynamicImage::ImageRgb8(RgbImage::from_fn(72, 64, |x, y| {
        Rgb([if (x / 5 + y / 7) % 2 == 0 { 8 } else { 247 }; 3])
    }))
}

#[test]
fn signed_rgb_terms_keep_correlated_signal_and_negative_residual_products() {
    let terms = mechanism_energy::paired_terms(
        [&[1.0, 2.0], &[1.0, 2.0]],
        [&[1.0, 2.0], &[1.0, 2.0]],
        [&[2.0, 4.0], &[2.0, 4.0]],
    )
    .unwrap();
    assert_eq!(terms.cross_term, 5.0);
    assert_ne!(
        terms.observed_energy - terms.noise_energy,
        terms.signal_energy
    );
    assert!(mechanism_energy::paired_terms([&[], &[]], [&[], &[]], [&[], &[]]).is_err());
    assert!(
        mechanism_energy::paired_terms([&[f64::NAN], &[0.0]], [&[0.0], &[0.0]], [&[0.0], &[0.0]])
            .is_err()
    );
    let clean = DynamicImage::ImageRgb8(RgbImage::from_pixel(72, 64, Rgb([128; 3])));
    let observed = DynamicImage::ImageRgb8(RgbImage::from_fn(72, 64, |x, y| {
        let delta = if (x / 4 + y / 5) % 2 == 0 { 20 } else { -20 };
        Rgb([(128 + delta) as u8, (128 - delta) as u8, 128])
    }));
    let measured = noise_decomposition::measure(&clean, &observed).unwrap();
    for scale in measured["scales"].as_array().unwrap() {
        assert!(scale["residual_common"].as_f64().unwrap() < 0.0);
        assert!(scale["signal_noise_cross"].as_f64().unwrap().abs() < 1e-8);
        assert!(scale["identity_error"].as_f64().unwrap().abs() < 1e-8);
    }
    assert!(noise_decomposition::measure(&clean, &DynamicImage::new_rgb8(73, 64)).is_err());
}

#[test]
fn all_processing_orders_reconstruct_noise_and_both_sharpen_clips() {
    let original = patterned();
    let original_bytes = original.to_rgb8();
    let stage = |kind: &str, value| stress_variants::Case {
        id: kind.into(),
        kind: kind.into(),
        value,
    };
    for noise in ["noise", "shared_noise"] {
        for stages in [
            vec![
                stage("blur", 2.0),
                stage(noise, 48.0),
                stage("unsharpen", 1.2),
            ],
            vec![stage(noise, 48.0), stage("unsharpen", 1.2)],
            vec![stage(noise, 48.0), stage("blur", 2.0)],
            vec![stage(noise, 48.0)],
            vec![],
        ] {
            let case = Case {
                id: "test".into(),
                reference_id: "clean".into(),
                seed: 1,
                stages,
            };
            let clean_case = Case {
                stages: case
                    .stages
                    .iter()
                    .filter(|s| !matches!(s.kind.as_str(), "noise" | "shared_noise"))
                    .cloned()
                    .collect(),
                ..case.clone()
            };
            let clean = degradation_cases::apply(&original, &clean_case).unwrap();
            let observed = degradation_cases::apply(&original, &case).unwrap();
            let trace = decomposition_trace::measure(&original, &case, &clean, &observed).unwrap();
            let measured = noise_decomposition::measure(&clean, &observed).unwrap();
            for scale in 0..2 {
                assert!(trace["identity_error"][scale].as_f64().unwrap().abs() < 1e-8);
                let delta = measured["scales"][scale]["observed"].as_f64().unwrap()
                    - measured["scales"][scale]["reference"].as_f64().unwrap();
                assert!((trace["actual_delta"][scale].as_f64().unwrap() - delta).abs() < 1e-8);
                if case.stages.is_empty() {
                    assert_eq!(delta, 0.0);
                }
            }
            if case.stages.first().is_some_and(|stage| stage.kind == noise) {
                assert!(
                    trace["clipped_channel_fractions_injection_observed_reference"][0]
                        .as_f64()
                        .unwrap()
                        > 0.0
                );
                if case.stages.last().unwrap().kind == "unsharpen" {
                    assert!(
                        trace["clipped_channel_fractions_injection_observed_reference"][1]
                            .as_f64()
                            .unwrap()
                            > 0.0
                    );
                    assert!(
                        trace["clipped_channel_fractions_injection_observed_reference"][2]
                            .as_f64()
                            .unwrap()
                            > 0.0
                    );
                }
            }
            assert_eq!(original.to_rgb8(), original_bytes);
        }
    }
    let channels = cross_channel_detail::rgb_channels(&original);
    let mut invalid = channels.clone();
    invalid[0] = super::super::linear_response::FloatGray::new(18, 18);
    assert!(cross_channel_detail::measure_channels(&invalid).is_none());
}
