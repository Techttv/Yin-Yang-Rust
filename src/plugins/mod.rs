//! The plugins, one module each; `plugin::registry` lists them in GUI order.

pub mod colors;
pub mod custom;
pub mod gtk;
pub mod icons;
pub mod konsole;
pub mod kvantum;
pub mod notification;
pub mod okular;
pub mod onlyoffice;
pub mod system;
pub mod vscode;
pub mod wallpaper;

use std::env;
use std::fs;
use std::path::PathBuf;

use anyhow::{anyhow, bail, Result};

use crate::edit::write_preserving;
use crate::exec::ExecOutput;
use crate::plugin::Env;

/// Runs a program and fails on a non-zero exit status.
pub(crate) fn run_checked(env: &Env, program: &str, args: &[&str]) -> Result<ExecOutput> {
    let output = env.exec.run(program, args)?;
    if output.status != 0 {
        bail!("{program} exited with status {}", output.status);
    }
    Ok(output)
}

/// Locale names to look for in translated metadata, most specific first: `["it_IT", "it"]`.
pub(crate) fn locales_from_env() -> Vec<String> {
    let raw = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .find_map(|name| env::var(name).ok().filter(|value| !value.is_empty()))
        .unwrap_or_default();
    let name = raw.split(['.', '@']).next().unwrap_or_default();
    if name.is_empty() || name == "C" || name == "POSIX" {
        return Vec::new();
    }
    let mut locales = vec![name.to_string()];
    if let Some((language, _)) = name.split_once('_') {
        locales.push(language.to_string());
    }
    locales
}

/// Runs `edit` on the text of every file in `paths` that exists and rewrites the ones it changes,
/// keeping their permissions. A file that cannot be read or edited is left untouched and does not
/// stop the others; the first failure is returned at the end. Returns how many files existed.
pub(crate) fn edit_files(paths: &[PathBuf], edit: impl Fn(&str) -> Result<String>) -> Result<usize> {
    let mut found = 0;
    let mut first_error = None;
    for path in paths.iter().filter(|p| p.is_file()) {
        found += 1;
        let result = (|| -> Result<()> {
            let text = fs::read_to_string(path)?;
            let edited = edit(&text)?;
            if edited != text {
                write_preserving(path, &edited)?;
            }
            Ok(())
        })();
        if let Err(e) = result {
            first_error.get_or_insert(anyhow!("{}: {e:#}", path.display()));
        }
    }
    match first_error {
        Some(e) => Err(e),
        None => Ok(found),
    }
}
