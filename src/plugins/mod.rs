//! The plugins, one module each; `plugin::registry` lists them in GUI order.

pub mod colors;
pub mod gtk;
pub mod icons;
pub mod kvantum;
pub mod notification;
pub mod system;
pub mod wallpaper;

use std::env;

use anyhow::{bail, Result};

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
