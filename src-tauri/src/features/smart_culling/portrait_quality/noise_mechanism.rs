//! Explicit synthetic diagnostic, separate from all frozen photograph experiments.

use anyhow::{Result, anyhow, ensure};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use super::{mechanism_energy, mechanism_samples};

fn digest(path: &Path) -> Result<String> {
    Ok(hex::encode(Sha256::digest(fs::read(path)?)))
}

#[test]
#[ignore = "explicit known-signal noise mechanism diagnostic; no photographic accuracy labels"]
fn export_noise_mechanism() {
    run().unwrap();
}

fn run() -> Result<()> {
    let output = PathBuf::from(std::env::var("QRAW_NOISE_MECHANISM_DIR")?);
    ensure!(output.is_absolute());
    ensure!(
        !output.join("observations.json").exists(),
        "refusing to overwrite experiment"
    );
    let protocol_path = output.join("PROTOCOL.json");
    let protocol: Value = serde_json::from_slice(&fs::read(&protocol_path)?)?;
    ensure!(protocol["schema"] == "qraw-noise-mechanism-protocol-1");
    let feature = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/features/smart_culling");
    for (name, expected) in protocol["code_hashes"]
        .as_object()
        .ok_or_else(|| anyhow!("missing hashes"))?
    {
        ensure!(
            digest(&feature.join(name))? == expected.as_str().unwrap(),
            "code changed: {name}"
        );
    }
    ensure!(digest(&output.join("PROTOCOL.md"))? == protocol["design_sha256"].as_str().unwrap());
    let cases: Vec<mechanism_samples::Case> = serde_json::from_value(protocol["cases"].clone())?;
    ensure!(
        cases.len() == 495
            && cases
                .iter()
                .map(|case| &case.id)
                .collect::<HashSet<_>>()
                .len()
                == 495
    );
    let mut rows = Vec::new();
    for (index, case) in cases.iter().enumerate() {
        let measured = mechanism_energy::measure(&mechanism_samples::make(case)?)?;
        rows.push(json!({"case": case, "measurement": measured}));
        if (index + 1) % 45 == 0 {
            println!("measured {}/495 synthetic conditions", index + 1);
        }
    }
    let result = json!({"schema": "qraw-noise-mechanism-observations-1",
        "protocol_sha256": digest(&protocol_path)?, "code_hashes": protocol["code_hashes"],
        "rows": rows, "source_photographs_read": 0, "labels_read": 0, "rating_writes": 0,
        "new_candidate_created": false, "accuracy_validated": false});
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(output.join("observations.json"))?;
    serde_json::to_writer_pretty(&mut file, &result)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    Ok(())
}
