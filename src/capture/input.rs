use anyhow::{Context, Result};
use std::io::Read;
use std::path::Path;

pub fn read_stdin() -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    std::io::stdin()
        .lock()
        .read_to_end(&mut bytes)
        .context("failed to read stdin")?;
    Ok(bytes)
}

pub fn read_file(path: &Path) -> Result<Vec<u8>> {
    std::fs::read(path).with_context(|| format!("failed to read {}", path.display()))
}
