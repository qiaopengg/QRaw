//! Tensor structure and numerical validity, independent of classification accuracy.

use anyhow::{Result, ensure};
use ndarray::{Array4, ArrayViewD};

use super::SampNetObservation;

// Same float32 softmax sum tolerance as network_baseline.py, not a quality threshold.
const DISTRIBUTION_SUM_TOLERANCE: f64 = 1e-6;

pub(super) fn validate_inputs(image: &Array4<f32>, saliency: &Array4<f32>) -> Result<()> {
    for (name, input, channels) in [("image", image, 3), ("saliency", saliency, 1)] {
        ensure!(
            input.shape() == [1, channels, 224, 224],
            "SAMP-Net {name} input must have shape [1,{channels},224,224]"
        );
        ensure!(
            input.iter().all(|value| value.is_finite()),
            "SAMP-Net {name} input contains nonfinite values"
        );
    }
    ensure!(
        saliency.iter().all(|value| (0.0..=1.0).contains(value)),
        "SAMP-Net saliency input must be in [0,1]"
    );
    Ok(())
}

pub(super) fn parse_outputs(
    pattern_logits: ArrayViewD<'_, f32>,
    attributes: ArrayViewD<'_, f32>,
    composition_distribution: ArrayViewD<'_, f32>,
) -> Result<SampNetObservation> {
    let pattern_logits = finite_row("pattern_logits", pattern_logits)?;
    let attributes = finite_row("attributes", attributes)?;
    let composition_distribution =
        finite_row::<5>("composition_distribution", composition_distribution)?;
    ensure!(
        composition_distribution
            .iter()
            .all(|value| (0.0..=1.0).contains(value)),
        "SAMP-Net composition distribution values must be in [0,1]"
    );
    let sum: f64 = composition_distribution
        .iter()
        .map(|value| f64::from(*value))
        .sum();
    ensure!(
        (sum - 1.0).abs() <= DISTRIBUTION_SUM_TOLERANCE,
        "SAMP-Net composition distribution must sum to one, got {sum}"
    );
    Ok(SampNetObservation {
        pattern_logits,
        attributes,
        composition_distribution,
    })
}

fn finite_row<const WIDTH: usize>(name: &str, values: ArrayViewD<'_, f32>) -> Result<[f32; WIDTH]> {
    ensure!(
        values.shape() == [1, WIDTH],
        "SAMP-Net {name} output must have shape [1,{WIDTH}]"
    );
    ensure!(
        values.iter().all(|value| value.is_finite()),
        "SAMP-Net {name} output contains nonfinite values"
    );
    let mut row = [0.0; WIDTH];
    for (target, value) in row.iter_mut().zip(values.iter()) {
        *target = *value;
    }
    Ok(row)
}
