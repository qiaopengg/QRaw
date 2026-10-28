//! Explicit offline GPU parity; compiled only for tests, never used for scoring.

mod input;

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};

use super::infrastructure::sampnet::{MODEL_SHA256, SampNetModel};
use super::models::verify_model;

fn verify() -> Result<Value> {
    let baseline = PathBuf::from(std::env::var("QRAW_SAMPNET_BASELINE_DIR")?);
    let export = PathBuf::from(std::env::var("QRAW_SAMPNET_EXPORT_DIR")?);
    let converted: Value = serde_json::from_slice(&fs::read(export.join("REPORT.json"))?)?;
    ensure!(converted["passed"] == true, "ONNX parity has not passed");
    verify_model(
        &baseline.join("REPORT.json"),
        converted["baseline_sha256"]
            .as_str()
            .context("baseline hash missing")?,
    )?;
    let reference: Value = serde_json::from_slice(&fs::read(baseline.join("REPORT.json"))?)?;
    ensure!(
        reference["passed"] == true,
        "official baseline has not passed"
    );
    let model = export.join("samp_net.onnx");
    ensure!(
        converted["model_sha256"] == MODEL_SHA256,
        "unaccepted SAMP-Net conversion"
    );
    let started = Instant::now();
    let mut model = SampNetModel::load(&model)?;
    let compilation_seconds = started.elapsed().as_secs_f64();
    let cases = reference["cases"]
        .as_array()
        .context("baseline cases missing")?;
    ensure!(cases.len() == 19, "all 19 declared cases are required");
    let mut rows = Vec::new();
    for case in cases {
        let image = input::read_tensor(&baseline, &case["inputs"]["image"], 3)?;
        let saliency = input::read_tensor(&baseline, &case["inputs"]["saliency"], 1)?;
        let started = Instant::now();
        let actual = model.observe_prepared(image, saliency)?;
        let mut errors = serde_json::Map::new();
        for (name, value) in [
            ("pattern_logits", actual.pattern_logits.as_slice()),
            ("attributes", actual.attributes.as_slice()),
            (
                "composition_distribution",
                actual.composition_distribution.as_slice(),
            ),
        ] {
            let expected = case["outputs"][name][0]
                .as_array()
                .context("reference output missing")?;
            ensure!(expected.len() == value.len(), "output shape mismatch");
            let mut maximum = 0.0_f64;
            for (value, expected) in value.iter().zip(expected) {
                let expected = expected.as_f64().context("invalid reference value")?;
                let difference = (f64::from(*value) - expected).abs();
                ensure!(
                    value.is_finite() && difference <= 0.001 + 0.00001 * expected.abs(),
                    "GPU parity failed: {} {name}: {value} vs {expected}",
                    case["id"]
                );
                maximum = maximum.max(difference);
            }
            errors.insert(name.to_owned(), json!(maximum));
        }
        rows.push(json!({"id": case["id"], "max_absolute_errors": errors,
                         "inference_seconds": started.elapsed().as_secs_f64()}));
    }
    Ok(
        json!({"passed": true, "scope": "strict project CoreML parity, not classification accuracy",
              "cpu_ep_fallback_disabled": true, "release_ready": false, "cases": rows,
              "compilation_seconds": compilation_seconds, "model_sha256": MODEL_SHA256}),
    )
}

#[test]
#[ignore = "explicit SAMP-Net strict project GPU verification with frozen external inputs"]
fn sampnet_matches_official_baseline_with_project_runtime() {
    const CHILD: &str = "QRAW_SAMPNET_GPU_CHILD";
    const TEST: &str = "features::smart_culling::sampnet_gpu_test::sampnet_matches_official_baseline_with_project_runtime";
    if std::env::var_os(CHILD).is_some() {
        let report = std::env::var("QRAW_SAMPNET_GPU_REPORT").unwrap();
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(report)
            .unwrap();
        let result = verify();
        let passed = result.is_ok();
        let report =
            result.unwrap_or_else(|error| json!({"passed": false, "error": format!("{error:#}")}));
        serde_json::to_writer_pretty(&mut file, &report).unwrap();
        file.write_all(b"\n").unwrap();
        if passed {
            println!("QRAW_SAMPNET_GPU_PASS cases=19");
        } else {
            eprintln!("{report}");
        }
        std::io::stdout().flush().unwrap();
        std::io::stderr().flush().unwrap();
        // SAFETY: dedicated child only; matches existing CoreML test teardown isolation.
        unsafe { libc::_exit(if passed { 0 } else { 1 }) }
    }
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", TEST, "--ignored", "--nocapture"])
        .env(CHILD, "1")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success() && stdout.contains("QRAW_SAMPNET_GPU_PASS"),
        "child status: {}\n{stdout}\n{stderr}",
        output.status
    );
    print!("{stdout}");
}
