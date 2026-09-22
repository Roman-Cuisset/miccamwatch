use crate::{model::AccessEvent, settings};
use anyhow::{Context, Result};
use std::{fs, io::Write, path::PathBuf};

const MAX_HISTORY_BYTES: u64 = 5 * 1024 * 1024;
const HISTORY_FILES: usize = 3;

pub fn path() -> Result<PathBuf> {
    Ok(settings::data_dir()?.join("history.jsonl"))
}

pub fn append(event: &AccessEvent) -> Result<()> {
    let path = path()?;
    if path
        .metadata()
        .is_ok_and(|metadata| metadata.len() >= MAX_HISTORY_BYTES)
    {
        rotate(&path)?;
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("failed to open history {}", path.display()))?;
    serde_json::to_writer(&mut file, event)?;
    file.write_all(b"\n")?;
    Ok(())
}

pub fn clear() -> Result<()> {
    let base = path()?;
    for index in 0..=HISTORY_FILES {
        let candidate = if index == 0 {
            base.clone()
        } else {
            base.with_extension(format!("jsonl.{index}"))
        };
        if candidate.exists() {
            fs::remove_file(candidate)?;
        }
    }
    Ok(())
}

fn rotate(base: &std::path::Path) -> Result<()> {
    let oldest = base.with_extension(format!("jsonl.{HISTORY_FILES}"));
    if oldest.exists() {
        fs::remove_file(oldest)?;
    }
    for index in (1..HISTORY_FILES).rev() {
        let source = base.with_extension(format!("jsonl.{index}"));
        if source.exists() {
            fs::rename(source, base.with_extension(format!("jsonl.{}", index + 1)))?;
        }
    }
    if base.exists() {
        fs::rename(base, base.with_extension("jsonl.1"))?;
    }
    Ok(())
}
