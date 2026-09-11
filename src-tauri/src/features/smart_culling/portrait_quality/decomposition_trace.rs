//! Ordered counterfactual increments, not independent causal probabilities.

use anyhow::{Result, ensure};
use image::DynamicImage;
use serde_json::{Value, json};

use super::{
    cross_channel_detail::rgb_channels,
    degradation_cases::{self, Case},
    degradation_intermediates as intermediate,
};

pub(super) fn measure(
    crop: &DynamicImage,
    case: &Case,
    clean: &DynamicImage,
    observed: &DynamicImage,
) -> Result<Value> {
    let energy = |image: &DynamicImage| intermediate::energies(&rgb_channels(image));
    let reference = energy(clean)?;
    let measured = energy(observed)?;
    let difference = |left: [f64; 2], right: [f64; 2]| [0, 1].map(|i| left[i] - right[i]);
    let actual_delta = difference(measured, reference);
    let (mut injected, mut noise_clip, mut processing, mut sharpen_clip) =
        ([0.0; 2], [0.0; 2], [0.0; 2], [0.0; 2]);
    let mut clipping = [0.0; 3]; // injection, sharpened observed, sharpened reference
    if let Some(position) = case
        .stages
        .iter()
        .position(|s| matches!(s.kind.as_str(), "noise" | "shared_noise"))
    {
        let before = degradation_cases::apply(
            crop,
            &Case {
                stages: case.stages[..position].to_vec(),
                ..case.clone()
            },
        )?;
        let stage = &case.stages[position];
        let added = degradation_cases::apply(
            &before,
            &Case {
                stages: vec![stage.clone()],
                ..case.clone()
            },
        )?;
        let (unclipped, fraction) = intermediate::injected(&before, stage, case.seed);
        clipping[0] = fraction;
        let nominal = intermediate::energies(&unclipped)?;
        injected = difference(nominal, energy(&before)?);
        noise_clip = difference(energy(&added)?, nominal);
        let injection_delta = difference(energy(&added)?, energy(&before)?);
        ensure!(
            case.stages.len() <= position + 2,
            "unexpected post-noise processing chain"
        );
        if let Some(stage) = case.stages.get(position + 1) {
            ensure!(
                matches!(stage.kind.as_str(), "blur" | "unsharpen"),
                "unexpected post-noise stage"
            );
            if stage.kind == "unsharpen" {
                let (noisy_unclipped, noisy_fraction) =
                    intermediate::unclipped_sharpen(&added, observed)?;
                let (clean_unclipped, clean_fraction) =
                    intermediate::unclipped_sharpen(&before, clean)?;
                clipping[1] = noisy_fraction;
                clipping[2] = clean_fraction;
                let unclipped_delta = difference(
                    intermediate::energies(&noisy_unclipped)?,
                    intermediate::energies(&clean_unclipped)?,
                );
                processing = difference(unclipped_delta, injection_delta);
                sharpen_clip = difference(actual_delta, unclipped_delta);
            } else {
                processing = difference(actual_delta, injection_delta);
            }
        }
    }
    let error = [0, 1]
        .map(|i| actual_delta[i] - injected[i] - noise_clip[i] - processing[i] - sharpen_clip[i]);
    ensure!(
        (0..2).all(|i| error[i].abs() <= 1e-8 * measured[i].abs().max(reference[i].abs()).max(1.0)),
        "ordered attribution failed"
    );
    Ok(
        json!({"reference": reference, "observed": measured, "actual_delta": actual_delta,
        "terms": {"injected_noise": injected, "noise_clip": noise_clip, "processing": processing, "sharpen_clip": sharpen_clip},
        "clipped_channel_fractions_injection_observed_reference": clipping, "identity_error": error}),
    )
}
