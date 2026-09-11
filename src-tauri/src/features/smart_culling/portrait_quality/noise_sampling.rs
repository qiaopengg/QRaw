//! Fixed spatial delete-block sensitivity, not a calibrated confidence interval.

use anyhow::{Result, ensure};
use serde::Serialize;

#[derive(Serialize)]
pub(super) struct BlockSensitivity {
    pub counts: [usize; 16],
    pub means: [f64; 16],
    pub delete_means: [f64; 16],
    pub mean: f64,
    pub standard_error: f64,
}

pub(super) fn measure(values: &[f64], width: usize, height: usize) -> Result<BlockSensitivity> {
    ensure!(
        width >= 4 && height >= 4 && values.len() == width * height,
        "invalid block window"
    );
    ensure!(
        values.iter().all(|value| value.is_finite()),
        "non-finite block samples"
    );
    let mut counts = [0; 16];
    let mut sums = [0.0; 16];
    for (index, value) in values.iter().enumerate() {
        let block = (index / width * 4 / height) * 4 + (index % width * 4 / width);
        counts[block] += 1;
        sums[block] += value;
    }
    let total = sums.iter().sum::<f64>();
    let means = std::array::from_fn(|i| sums[i] / counts[i] as f64);
    let delete_means =
        std::array::from_fn(|i| (total - sums[i]) / (values.len() - counts[i]) as f64);
    let center = delete_means.iter().sum::<f64>() / 16.0;
    let standard_error = (15.0 / 16.0
        * delete_means
            .iter()
            .map(|v| (v - center).powi(2))
            .sum::<f64>())
    .sqrt();
    Ok(BlockSensitivity {
        counts,
        means,
        delete_means,
        mean: total / values.len() as f64,
        standard_error,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uneven_blocks_match_direct_deletion_and_keep_signed_values() {
        let (width, height) = (11, 9);
        let values = (0..width * height)
            .map(|i| i as f64 - 60.0)
            .collect::<Vec<_>>();
        let result = measure(&values, width, height).unwrap();
        assert_eq!(result.counts.iter().sum::<usize>(), values.len());
        assert!(result.mean < 0.0);
        for block in 0..16 {
            let kept = values
                .iter()
                .enumerate()
                .filter(|(i, _)| (i / width * 4 / height) * 4 + (i % width * 4 / width) != block)
                .map(|(_, value)| *value)
                .collect::<Vec<_>>();
            assert!(
                (result.delete_means[block] - kept.iter().sum::<f64>() / kept.len() as f64).abs()
                    < 1e-12
            );
        }
        assert!(result.standard_error > 0.0);
    }

    #[test]
    fn constant_blocks_have_zero_sensitivity_and_invalid_inputs_fail() {
        assert_eq!(measure(&[3.0; 64], 8, 8).unwrap().standard_error, 0.0);
        assert!(measure(&[], 8, 8).is_err());
        assert!(measure(&[f64::NAN; 64], 8, 8).is_err());
        assert!(measure(&[0.0; 24], 3, 8).is_err());
    }
}
