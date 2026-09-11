use super::*;
use image::Rgb;

#[test]
fn flat_pixels_have_no_noise_and_tiny_inputs_fail() {
    let image = RgbImage::from_pixel(48, 40, Rgb([30, 80, 170]));
    let result = measure(&image).unwrap();
    for scale in result["scales"].as_array().unwrap() {
        assert_eq!(scale["proxy_decomposition"]["residual_common"], 0.0);
        assert_eq!(scale["sampling"]["standard_error"], 0.0);
        assert_eq!(scale["corrected_energy"], 0.0);
    }
    assert!(measure(&RgbImage::new(0, 0)).is_err());
    assert!(measure(&RgbImage::new(13, 80)).is_err());
}

#[test]
fn opposed_channel_impulses_preserve_negative_noise_estimates() {
    let image = RgbImage::from_fn(72, 64, |x, y| {
        let delta = if x % 7 == 0 && y % 7 == 0 { 30 } else { 0 };
        Rgb([128 + delta, 128 - delta, 128])
    });
    let original = image.clone();
    let result = measure(&image).unwrap();
    for scale in result["scales"].as_array().unwrap() {
        assert!(
            scale["proxy_decomposition"]["residual_common"]
                .as_f64()
                .unwrap()
                < 0.0
        );
        assert!(scale["corrected_energy"].as_f64().unwrap().abs() < 1e-8);
    }
    assert_eq!(image, original);
    assert_eq!(result, measure(&image).unwrap());
}

#[test]
fn real_thin_structure_can_leak_into_proxy_without_becoming_a_noise_verdict() {
    let image = RgbImage::from_fn(72, 64, |x, _| Rgb([if x % 7 == 0 { 200 } else { 80 }; 3]));
    let result = measure(&image).unwrap();
    assert!(
        result["scales"][0]["proxy_decomposition"]["residual_common"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    assert_eq!(result["clean_reference_used"], false);
    assert_eq!(result["confidence_interval_validated"], false);
    assert!(result.get("clear").is_none());
}
