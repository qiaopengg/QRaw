//! Native SAMP-Net candidate inference from verified, already prepared tensors.
//! No image preprocessing, optical verdict, composition pass line or rating.

mod contract;

use std::path::Path;

use anyhow::{Result, ensure};
use ndarray::Array4;
use ort::{session::Session, value::Tensor};

use super::super::models::{
    gpu_session_with_optimization, validate_session_contract, verify_model,
};

pub(crate) const MODEL_SHA256: &str =
    "e7bf8240854da50e9f44cc5710cf69baf9d4139c3c8b312576bb6924fc654a3a";
pub(crate) const OUTPUT_NAMES: [&str; 3] =
    ["pattern_logits", "attributes", "composition_distribution"];

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SampNetObservation {
    pub pattern_logits: [f32; 8],
    /// Unbounded attribute regressions, not probabilities or optical decisions.
    pub attributes: [f32; 6],
    /// Official probabilities for composition score levels 1 through 5.
    pub composition_distribution: [f32; 5],
}

pub(crate) struct SampNetModel {
    session: Session,
}

impl SampNetModel {
    pub(crate) fn load(path: &Path) -> Result<Self> {
        // Pin the accepted conversion independently of editable report metadata.
        verify_model(path, MODEL_SHA256)?;
        ensure!(
            cfg!(target_os = "macos"),
            "SAMP-Net GPU runtime is validated only on macOS"
        );
        let session = gpu_session_with_optimization(&path.to_path_buf(), None, None)?;
        ensure!(
            session.inputs.len() == 2 && session.outputs.len() == 3,
            "SAMP-Net requires exactly two inputs and three outputs"
        );
        validate_session_contract(&session, "image", &[1, 3, 224, 224], &OUTPUT_NAMES)?;
        validate_session_contract(&session, "saliency", &[1, 1, 224, 224], &OUTPUT_NAMES)?;
        Ok(Self { session })
    }

    pub(crate) fn observe_prepared(
        &mut self,
        image: Array4<f32>,
        saliency: Array4<f32>,
    ) -> Result<SampNetObservation> {
        contract::validate_inputs(&image, &saliency)?;
        let outputs = self.session.run(ort::inputs![
            "image" => Tensor::from_array(image)?,
            "saliency" => Tensor::from_array(saliency)?
        ])?;
        contract::parse_outputs(
            outputs[OUTPUT_NAMES[0]].try_extract_array::<f32>()?,
            outputs[OUTPUT_NAMES[1]].try_extract_array::<f32>()?,
            outputs[OUTPUT_NAMES[2]].try_extract_array::<f32>()?,
        )
    }
}

#[cfg(test)]
mod tests;
