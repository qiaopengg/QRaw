//! Paired-reference decomposition. Oracle residuals are unavailable in real single photos.

use anyhow::{Result, anyhow, ensure};
use image::Luma;
use serde::Serialize;
use serde_json::{Value, json};

use super::linear_detail;
use super::linear_response::{self, FloatGray, energy};
use super::mechanism_samples::Pair;
use super::robust_detail::noise_sigma_from_samples;

#[derive(Serialize)]
pub(super) struct PairTerms {
    pub signal_energy: f64,
    pub noise_energy: f64,
    pub observed_energy: f64,
    pub cross_term: f64,
    pub decomposition_error: f64,
    pub linearity_error: f64,
}

pub(super) fn paired_terms(
    signal: [&[f64]; 2],
    noise: [&[f64]; 2],
    observed: [&[f64]; 2],
) -> Result<PairTerms> {
    let count = signal[0].len();
    ensure!(
        count > 0
            && signal
                .iter()
                .chain(&noise)
                .chain(&observed)
                .all(|values| values.len() == count && values.iter().all(|v| v.is_finite())),
        "invalid paired response"
    );
    let moment = |values: [&[f64]; 2]| {
        values[0]
            .iter()
            .zip(values[1])
            .map(|(a, b)| a * b)
            .sum::<f64>()
            / count as f64
    };
    let signal_energy = moment(signal);
    let noise_energy = moment(noise);
    let observed_energy = moment(observed);
    let cross_term = moment([signal[0], noise[1]]) + moment([noise[0], signal[1]]);
    let decomposition_error = observed_energy - signal_energy - noise_energy - cross_term;
    let linearity_error = (0..2)
        .flat_map(|channel| {
            (0..count)
                .map(move |i| (observed[channel][i] - signal[channel][i] - noise[channel][i]).abs())
        })
        .fold(0.0, f64::max);
    ensure!(
        linearity_error <= 1e-8
            && decomposition_error.abs()
                <= 1e-8 * observed_energy.abs().max(signal_energy.abs()).max(1.0),
        "linear decomposition failed"
    );
    Ok(PairTerms {
        signal_energy,
        noise_energy,
        observed_energy,
        cross_term,
        decomposition_error,
        linearity_error,
    })
}

pub(super) fn measure(pair: &Pair) -> Result<Value> {
    ensure!(pair.clean.dimensions() == pair.observed.dimensions());
    let (width, height) = pair.observed.dimensions();
    let sigma = noise_sigma_from_samples(width, height, |x, y| pair.observed.get_pixel(x, y)[0])
        .ok_or_else(|| anyhow!("invalid noise estimate input"))?;
    let residual = FloatGray::from_fn(width, height, |x, y| {
        Luma([pair.observed.get_pixel(x, y)[0] - pair.clean.get_pixel(x, y)[0]])
    });
    let mean = residual.as_raw().iter().sum::<f64>() / residual.as_raw().len() as f64;
    let variance = residual
        .as_raw()
        .iter()
        .map(|p| (p - mean).powi(2))
        .sum::<f64>()
        / residual.as_raw().len() as f64;
    let signal =
        linear_response::planes(&pair.clean).ok_or_else(|| anyhow!("invalid clean plane"))?;
    let noise =
        linear_response::planes(&residual).ok_or_else(|| anyhow!("invalid residual plane"))?;
    let observed =
        linear_response::planes(&pair.observed).ok_or_else(|| anyhow!("invalid observed plane"))?;
    let original =
        linear_response::planes(&pair.original).ok_or_else(|| anyhow!("invalid original plane"))?;
    let mut scales = Vec::new();
    for index in 0..2 {
        let (s, n, o) = (
            &signal[index].values,
            &noise[index].values,
            &observed[index].values,
        );
        ensure!(s.len() == n.len() && n.len() == o.len());
        let PairTerms {
            signal_energy,
            noise_energy,
            observed_energy,
            cross_term,
            decomposition_error,
            linearity_error,
        } = paired_terms([s, s], [n, n], [o, o])?;
        let white_noise_energy = variance * observed[index].noise_gain;
        let estimated_noise_energy = sigma * sigma * observed[index].noise_gain;
        scales.push(json!({"order": observed[index].smoothing_order, "count": s.len(),
            "signal_energy": signal_energy, "noise_energy": noise_energy,
            "observed_energy": observed_energy, "cross_term": cross_term,
            "original_signal_energy": energy(&original[index].values),
            "oracle_residual_corrected": observed_energy - noise_energy,
            "true_variance_white_corrected": observed_energy - white_noise_energy,
            "estimated_variance_corrected": observed_energy - estimated_noise_energy,
            "white_noise_energy": white_noise_energy, "estimated_noise_energy": estimated_noise_energy,
            "white_model_error": white_noise_energy - noise_energy,
            "variance_estimation_error": estimated_noise_energy - white_noise_energy,
            "decomposition_error": decomposition_error, "linearity_error": linearity_error}));
    }
    Ok(
        json!({"dimensions": [width, height], "input_source": pair.source,
        "noise_sigma": sigma, "residual_mean": mean, "residual_variance": variance,
        "variance_estimation_relative_error": (variance > 0.0).then(|| (sigma * sigma - variance) / variance),
        "lag1_correlation": [correlation(&residual, mean, variance, 1, 0), correlation(&residual, mean, variance, 0, 1)],
        "clipped_channel_fraction_before_processing": pair.clipped_channel_fraction,
        "scales": scales, "linear_detail": linear_detail::from_responses(&observed, sigma)}),
    )
}

fn correlation(noise: &FloatGray, mean: f64, variance: f64, dx: u32, dy: u32) -> Option<f64> {
    if variance <= 0.0 {
        return None;
    }
    let mut covariance = 0.0;
    for y in 0..noise.height() - dy {
        for x in 0..noise.width() - dx {
            covariance +=
                (noise.get_pixel(x, y)[0] - mean) * (noise.get_pixel(x + dx, y + dy)[0] - mean);
        }
    }
    Some(covariance / f64::from((noise.width() - dx) * (noise.height() - dy)) / variance)
}

#[test]
fn energy_decomposition_keeps_cross_term_and_exposure_signal_change() {
    let clean = FloatGray::from_fn(64, 64, |x, y| {
        Luma([if (x / 7 + y / 9) % 2 == 0 {
            40.0
        } else {
            200.0
        }])
    });
    let observed = FloatGray::from_fn(64, 64, |x, y| Luma([clean.get_pixel(x, y)[0] * 1.2]));
    let result = measure(&Pair {
        clean: clean.clone(),
        observed,
        original: clean,
        clipped_channel_fraction: 0.0,
        source: "test",
    })
    .unwrap();
    for scale in result["scales"].as_array().unwrap() {
        assert!(scale["cross_term"].as_f64().unwrap() > 0.0);
        assert!(scale["decomposition_error"].as_f64().unwrap().abs() < 1e-8);
        assert_ne!(scale["oracle_residual_corrected"], scale["signal_energy"]);
    }
}
