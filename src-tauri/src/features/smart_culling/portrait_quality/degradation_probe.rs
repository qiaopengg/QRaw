//! Paired crop-only degradation export; no candidate, face inference or image writes.

use anyhow::Result;
use serde_json::json;
use std::path::Path;

use super::super::{
    cross_channel_detail, degradation_cases, detail, edge_spread, linear_detail, robust_detail,
};
use super::paired_probe_io as io;

#[test]
#[ignore = "explicit paired degradation measurement; not human clarity classification"]
fn export_paired_degradation() {
    let directory = std::path::PathBuf::from(std::env::var("QRAW_PAIRED_DEGRADATION_DIR").unwrap());
    run(&directory).unwrap();
}

fn run(directory: &Path) -> Result<()> {
    let inputs = io::load(directory, "qraw-paired-degradation-protocol-2")?;
    let mut rows = Vec::new();
    for item in &inputs.items {
        let crop = io::crop(item)?;
        let mut measured = Vec::new();
        for case in &inputs.cases {
            let input = degradation_cases::apply(&crop, case)?;
            let robust = robust_detail::measure(&input);
            let linear = robust
                .as_ref()
                .and_then(|value| linear_detail::measure(&input, value.noise_sigma));
            measured.push(json!({"case_id": case.id, "crop_dimensions": [input.width(), input.height()],
                "detail": detail::measure(&input), "robust_detail": robust, "linear_detail": linear,
                "edge_spread": edge_spread::measure(&input), "cross_channel": cross_channel_detail::measure(&input)}));
        }
        io::verify_source(item)?;
        println!(
            "measured {}: {} ordered cases",
            item["image_ref"],
            measured.len()
        );
        rows.push(io::sample(item, measured)?);
    }
    io::save(
        directory,
        &json!({"schema": "qraw-paired-degradation-observations-2", "protocol_sha256": io::digest(&directory.join("PROTOCOL.json"))?,
        "code_hashes": inputs.protocol["code_hashes"], "rows": rows, "source_images_written": false,
        "detection_rerun": false, "fiqa_rerun": false, "gpu_render_exercised": false,
        "candidate_fitted": false, "accuracy_validated": false}),
    )
}
