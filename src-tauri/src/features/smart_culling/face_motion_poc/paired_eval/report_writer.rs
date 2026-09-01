use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;

use anyhow::{Context, Result};
use serde_json::Value;

pub(super) fn write_report(path: &Path, rows: &[Value], exclusive: bool) -> Result<()> {
    let file = if exclusive {
        OpenOptions::new().write(true).create_new(true).open(path)
    } else {
        File::create(path)
    }
    .with_context(|| format!("cannot create paired replay report {}", path.display()))?;
    let mut writer = BufWriter::new(file);
    for row in rows {
        serde_json::to_writer(&mut writer, row)?;
        writeln!(writer)?;
    }
    writer.flush()?;
    writer.get_ref().sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::json;

    use super::*;

    #[test]
    fn refuses_to_overwrite_existing_blind_evidence() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("report.jsonl");
        let original = json!({"file": "first.jpg"});

        write_report(&path, std::slice::from_ref(&original), true).unwrap();
        let error = write_report(&path, &[json!({"file": "second.jpg"})], true).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("cannot create paired replay report")
        );
        assert_eq!(
            fs::read_to_string(path).unwrap(),
            format!("{}\n", serde_json::to_string(&original).unwrap())
        );
    }
}
