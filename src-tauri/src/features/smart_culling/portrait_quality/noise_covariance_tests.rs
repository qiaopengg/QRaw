use super::*;
use image::Luma;

#[test]
fn spatial_split_matches_direct_double_kernel_sum_on_the_exact_valid_window() {
    let residual = [0, 1, 2].map(|channel| {
        FloatGray::from_fn(20, 22, |x, y| {
            Luma([((x * 13 + y * 19 + channel * 5) % 23) as f64 - 11.0 + channel as f64 * 40.0])
        })
    });
    let responses = [0, 1, 2].map(|c| linear_response::planes(&residual[c]).unwrap());
    let measured = measure(&residual, &responses[0]).unwrap();
    let centered = [0, 1, 2].map(|c| {
        let mut inner = linear_response::inner(&residual[c]).unwrap();
        for pixel in inner.pixels_mut() {
            pixel[0] -= measured.roi_means[c];
        }
        inner
    });
    let (width, height) = centered[0].dimensions();
    for scale in 0..2 {
        let response = &responses[0][scale];
        let side = response.smoothing_order + 3;
        let radius = side / 2;
        for (pair, (a, b)) in PAIRS.into_iter().enumerate() {
            let (mut same, mut other) = (0.0, 0.0);
            for y in 3..height - 3 {
                for x in 3..width - 3 {
                    for (i, ki) in response.kernel.iter().enumerate() {
                        for (j, kj) in response.kernel.iter().enumerate() {
                            let left = centered[a].get_pixel(
                                (x as usize + i % side - radius) as u32,
                                (y as usize + i / side - radius) as u32,
                            )[0];
                            let right = centered[b].get_pixel(
                                (x as usize + j % side - radius) as u32,
                                (y as usize + j / side - radius) as u32,
                            )[0];
                            if i == j {
                                same += ki * kj * left * right;
                            } else {
                                other += ki * kj * left * right;
                            }
                        }
                    }
                }
            }
            let count = response.values.len() as f64;
            let filtered = responses[a][scale]
                .values
                .iter()
                .zip(&responses[b][scale].values)
                .map(|(a, b)| a * b)
                .sum::<f64>()
                / count;
            assert!((same / count - measured.same_position[scale][pair]).abs() < 1e-8);
            assert!((same / count + other / count - filtered).abs() < 1e-8);
        }
    }
}

#[test]
fn constant_residual_is_centered_and_invalid_inputs_are_rejected() {
    let residual = [0, 1, 2].map(|c| FloatGray::from_pixel(32, 28, Luma([20.0 + c as f64])));
    let response = linear_response::planes(&residual[0]).unwrap();
    let moments = measure(&residual, &response).unwrap();
    assert_eq!(moments.same_position, [[0.0; 3]; 2]);
    assert_eq!(moments.zero_lag_pairs, [0.0; 3]);
    let mut invalid = residual;
    invalid[1] = FloatGray::new(34, 28);
    assert!(measure(&invalid, &response).is_err());
    invalid[1] = FloatGray::from_pixel(32, 28, Luma([f64::NAN]));
    assert!(measure(&invalid, &response).is_err());
}
