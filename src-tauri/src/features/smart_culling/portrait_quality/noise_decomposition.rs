//! Paired RGB diagnosis only. The clean reference is never available to a clarity gate.

use anyhow::{Result, anyhow, ensure};
use image::{DynamicImage, Luma};
use serde_json::{Value, json};

use super::linear_response::FloatGray;
use super::{cross_channel_detail, mechanism_energy, noise_covariance};

pub(super) fn measure(clean: &DynamicImage, observed: &DynamicImage) -> Result<Value> {
    ensure!(
        clean.width() == observed.width() && clean.height() == observed.height(),
        "mismatched paired crops"
    );
    let clean = cross_channel_detail::rgb_channels(clean);
    let observed = cross_channel_detail::rgb_channels(observed);
    let residual = [0, 1, 2].map(|c| {
        FloatGray::from_fn(clean[c].width(), clean[c].height(), |x, y| {
            Luma([observed[c].get_pixel(x, y)[0] - clean[c].get_pixel(x, y)[0]])
        })
    });
    let signal = cross_channel_detail::response_planes(&clean)
        .ok_or_else(|| anyhow!("invalid clean response"))?;
    let noise = cross_channel_detail::response_planes(&residual)
        .ok_or_else(|| anyhow!("invalid residual response"))?;
    let measured = cross_channel_detail::response_planes(&observed)
        .ok_or_else(|| anyhow!("invalid observed response"))?;
    let spatial = noise_covariance::measure(&residual, &noise[0])?;
    let mut scales = Vec::new();
    for index in 0..2 {
        let pairs = noise_covariance::PAIRS
            .into_iter()
            .map(|(a, b)| {
                mechanism_energy::paired_terms(
                    [&signal[a][index].values, &signal[b][index].values],
                    [&noise[a][index].values, &noise[b][index].values],
                    [&measured[a][index].values, &measured[b][index].values],
                )
            })
            .collect::<Result<Vec<_>>>()?;
        let same = spatial.same_position[index];
        let different = [0, 1, 2].map(|p| pairs[p].noise_energy - same[p]);
        let mean = |values: [f64; 3]| values.iter().sum::<f64>() / 3.0;
        let reference = mean([0, 1, 2].map(|p| pairs[p].signal_energy));
        let observed = mean([0, 1, 2].map(|p| pairs[p].observed_energy));
        let signal_noise_cross = mean([0, 1, 2].map(|p| pairs[p].cross_term));
        let residual_common = mean([0, 1, 2].map(|p| pairs[p].noise_energy));
        let error = observed - reference - mean(same) - mean(different) - signal_noise_cross;
        ensure!(
            error.abs() <= 1e-8 * observed.abs().max(reference.abs()).max(1.0),
            "spatial energy identity failed"
        );
        scales.push(json!({"order": signal[0][index].smoothing_order, "count": signal[0][index].values.len(),
            "pairs_rg_rb_gb": pairs, "reference": reference, "observed": observed,
            "residual_common": residual_common, "signal_noise_cross": signal_noise_cross,
            "same_position_pairs": same, "different_position_pairs": different,
            "same_position": mean(same), "different_position": mean(different), "identity_error": error}));
    }
    Ok(
        json!({"spatial_moments": spatial, "scales": scales, "reference_used_for_diagnosis_only": true}),
    )
}
