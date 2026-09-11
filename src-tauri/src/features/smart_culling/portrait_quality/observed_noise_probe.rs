//! Fixed matrix exporter. Oracle values stay in the separate Python evaluation path.

use anyhow::Result;
use serde_json::json;
use std::path::Path;

use super::super::{cross_channel_detail, degradation_cases, observed_noise};
use super::paired_probe_io as io;

#[test]
#[ignore = "explicit observed-pixel-only noise estimation; no classification truth"]
fn export_observed_noise() {
    run(Path::new(
        &std::env::var("QRAW_OBSERVED_NOISE_DIR").unwrap(),
    ))
    .unwrap();
}

fn run(directory: &Path) -> Result<()> {
    let inputs = io::load(directory, "qraw-observed-noise-protocol-1")?;
    let parent = Path::new(inputs.protocol["parent_path"].as_str().unwrap());
    io::verify_hashes(parent, &inputs.protocol["parent_file_hashes"])?;
    io::verify_hashes(
        Path::new(inputs.protocol["parent_code_root"].as_str().unwrap()),
        &inputs.protocol["parent_code_hashes"],
    )?;
    let mut rows = Vec::new();
    for item in &inputs.items {
        let crop = io::crop(item)?;
        let mut variants = Vec::new();
        for case in &inputs.cases {
            let observed = degradation_cases::apply(&crop, case)?;
            // Only generated pixels cross the estimator boundary. No clean reference is loaded.
            let estimate = observed_noise::measure(&observed.to_rgb8())?;
            variants.push(json!({"case_id": case.id,
                "crop_dimensions": [observed.width(), observed.height()],
                "cross_channel": cross_channel_detail::measure(&observed), "estimate": estimate}));
        }
        io::verify_source(item)?;
        println!(
            "estimated {}: {} ordered cases",
            item["image_ref"],
            variants.len()
        );
        rows.push(io::sample(item, variants)?);
    }
    io::save(
        directory,
        &json!({"schema": "qraw-observed-noise-observations-1",
        "protocol_sha256": io::digest(&directory.join("PROTOCOL.json"))?,
        "code_hashes": inputs.protocol["code_hashes"], "rows": rows,
        "source_images_written": false, "estimator_clean_reference_used": false,
        "candidate_fitted": false, "accuracy_validated": false, "runtime_enabled": false}),
    )
}
