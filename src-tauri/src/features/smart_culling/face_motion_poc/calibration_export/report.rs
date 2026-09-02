use std::fs::OpenOptions;
use std::io::{BufWriter, Write};
use std::path::Path;

use anyhow::{Context, Result};
use serde_json::Value;

pub(super) fn write_jsonl(output_path: &Path, rows: &[Value]) -> Result<()> {
    let mut writer = new_writer(output_path)?;
    for row in rows {
        serde_json::to_writer(&mut writer, row)?;
        writeln!(writer)?;
    }
    finish(writer)
}

pub(super) fn write_json(output_path: &Path, value: &Value) -> Result<()> {
    let mut writer = new_writer(output_path)?;
    serde_json::to_writer_pretty(&mut writer, value)?;
    writeln!(writer)?;
    finish(writer)
}

fn new_writer(output_path: &Path) -> Result<BufWriter<std::fs::File>> {
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output_path)
        .with_context(|| format!("cannot create output {}", output_path.display()))?;
    Ok(BufWriter::new(file))
}

fn finish(mut writer: BufWriter<std::fs::File>) -> Result<()> {
    writer.flush()?;
    writer.get_ref().sync_all()?;
    Ok(())
}
