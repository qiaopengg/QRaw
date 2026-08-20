//! Explicit project-runtime gate for the local, untracked ARNIQA POC assets.

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use anyhow::Context;
    use ndarray::{Array2, Array4};
    use ort::session::builder::GraphOptimizationLevel;
    use ort::value::Tensor;

    use super::super::models::{
        gpu_session_with_optimization, validate_session_contract, verify_model,
    };

    const ENCODER: &str = "arniqa_encoder_224_poc.onnx";
    const HEADS: &str = "arniqa_three_heads_poc.onnx";
    const ENCODER_SHA256: &str = "a942e6aff3194d1111df41ee6513d471871f696c5d9e0df8360a84597d574dc5";
    const HEADS_SHA256: &str = "2a79928654c4b38c0375dbed596d393fc8d3d4b7ec9e7edb6097f0c0a192d441";
    const TEST_NAME: &str =
        "features::smart_culling::arniqa_poc::tests::strict_project_runtime_smoke_test";
    const CHILD_ENV: &str = "QRAW_ARNIQA_POC_CHILD";
    const PASS_MARKER: &str = "QRAW_ARNIQA_PROJECT_RUNTIME_PASS";

    #[test]
    #[ignore = "explicit local ARNIQA hardware POC; assets are legally blocked from redistribution"]
    fn strict_project_runtime_smoke_test() {
        if std::env::var_os(CHILD_ENV).is_none() {
            run_isolated_parent();
            return;
        }

        run_strict_inference(&model_dir()).unwrap();
        println!("{PASS_MARKER}");

        #[cfg(target_os = "macos")]
        {
            use std::io::Write;

            std::io::stdout().flush().unwrap();
            std::io::stderr().flush().unwrap();
            // SAFETY: this isolated test child has completed all assertions and
            // flushed output. `_exit` only bypasses ORT's affected static logger
            // destructor; application code never uses this path.
            unsafe { libc::_exit(0) }
        }
        #[cfg(target_os = "windows")]
        std::process::exit(0);
    }

    fn run_isolated_parent() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST_NAME, "--ignored", "--nocapture"])
            .env(CHILD_ENV, "1")
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            output.status.success() && stdout.contains(PASS_MARKER),
            "isolated ARNIQA project-runtime POC failed\nstdout:\n{stdout}\nstderr:\n{stderr}"
        );
    }

    fn run_strict_inference(model_dir: &Path) -> anyhow::Result<()> {
        let encoder_path = model_dir.join(ENCODER);
        let heads_path = model_dir.join(HEADS);
        verify_model(&encoder_path, ENCODER_SHA256)?;
        verify_model(&heads_path, HEADS_SHA256)?;

        let mut encoder = gpu_session_with_optimization(
            &encoder_path,
            None,
            Some(GraphOptimizationLevel::Disable),
        )
        .context("ARNIQA encoder strict provider session failed")?;
        validate_session_contract(&encoder, "normalized_rgb", &[1, 3, 224, 224], &["features"])?;
        let encoder_input = Tensor::from_array(Array4::<f32>::zeros((1, 3, 224, 224)).into_dyn())?;
        let encoder_outputs = encoder.run(ort::inputs!["normalized_rgb" => encoder_input])?;
        let features = encoder_outputs["features"].try_extract_array::<f32>()?;
        if features.len() != 2_048 || features.iter().any(|value| !value.is_finite()) {
            anyhow::bail!("ARNIQA encoder output contract mismatch");
        }

        let mut heads =
            gpu_session_with_optimization(&heads_path, None, Some(GraphOptimizationLevel::Disable))
                .context("ARNIQA regression heads strict provider session failed")?;
        validate_session_contract(
            &heads,
            "combined_embedding",
            &[1, 4096],
            &["raw_scores", "scaled_scores"],
        )?;
        let heads_input = Tensor::from_array(Array2::<f32>::zeros((1, 4096)).into_dyn())?;
        let heads_outputs = heads.run(ort::inputs!["combined_embedding" => heads_input])?;
        for name in ["raw_scores", "scaled_scores"] {
            let output = heads_outputs[name].try_extract_array::<f32>()?;
            if output.len() != 3 || output.iter().any(|value| !value.is_finite()) {
                anyhow::bail!("ARNIQA {name} output contract mismatch");
            }
        }
        Ok(())
    }

    fn model_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/features/smart_culling/model_assets/arniqa_poc")
    }
}
