//! Editors that change one value in a user's file and leave every other byte alone.

pub mod ini;
pub mod jsonc;

use std::fs;
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result};
use tempfile::NamedTempFile;

/// Replaces the contents of an existing file atomically (temporary file in the same directory,
/// then rename), keeping its permissions and writing through a symbolic link instead of replacing it.
pub fn write_preserving(path: &Path, text: &str) -> Result<()> {
    let target = fs::canonicalize(path).with_context(|| format!("resolving {}", path.display()))?;
    let dir = target.parent().context("file has no parent directory")?;
    let permissions = fs::metadata(&target)?.permissions();

    let mut tmp = NamedTempFile::new_in(dir)?;
    tmp.write_all(text.as_bytes())?;
    tmp.as_file().set_permissions(permissions)?;
    tmp.persist(&target)
        .with_context(|| format!("writing {}", target.display()))?;
    Ok(())
}
