//! Replay every v1 condition and export signed RGB error terms to a new directory.

use anyhow::Result;
use serde_json::json;
use std::{collections::HashMap, path::Path};

use super::super::{
    cross_channel_detail, decomposition_trace, degradation_cases, noise_decomposition,
};
use super::paired_probe_io as io;

#[test]
#[ignore = "explicit paired noise decomposition, not an online denoiser or classifier"]
fn export_noise_decomposition() {
    run(Path::new(
        &std::env::var("QRAW_NOISE_DECOMPOSITION_DIR").unwrap(),
    ))
    .unwrap();
}

fn run(directory: &Path) -> Result<()> {
    let inputs = io::load(directory, "qraw-noise-decomposition-protocol-1")?;
    let parent = Path::new(inputs.protocol["parent_path"].as_str().unwrap());
    io::verify_hashes(parent, &inputs.protocol["parent_file_hashes"])?;
    io::verify_hashes(
        Path::new(inputs.protocol["parent_code_root"].as_str().unwrap()),
        &inputs.protocol["parent_code_hashes"],
    )?;
    let mut rows = Vec::new();
    for item in &inputs.items {
        let crop = io::crop(item)?;
        let references = inputs
            .cases
            .iter()
            .filter(|case| case.id == case.reference_id)
            .map(|case| Ok((case.id.as_str(), degradation_cases::apply(&crop, case)?)))
            .collect::<Result<HashMap<_, _>>>()?;
        let mut variants = Vec::new();
        for case in &inputs.cases {
            let observed = degradation_cases::apply(&crop, case)?;
            let clean = &references[case.reference_id.as_str()];
            variants.push(
                json!({"case_id": case.id, "crop_dimensions": [observed.width(), observed.height()],
                "cross_channel": cross_channel_detail::measure(&observed),
                "decomposition": noise_decomposition::measure(clean, &observed)?,
                "trace": decomposition_trace::measure(&crop, case, clean, &observed)?}),
            );
        }
        io::verify_source(item)?;
        println!(
            "decomposed {}: {} ordered cases",
            item["image_ref"],
            variants.len()
        );
        rows.push(io::sample(item, variants)?);
    }
    io::save(
        directory,
        &json!({"schema": "qraw-noise-decomposition-observations-1",
        "protocol_sha256": io::digest(&directory.join("PROTOCOL.json"))?, "code_hashes": inputs.protocol["code_hashes"],
        "rows": rows, "source_images_written": false, "reference_used_for_diagnosis_only": true,
        "candidate_fitted": false, "accuracy_validated": false, "runtime_enabled": false}),
    )
}
