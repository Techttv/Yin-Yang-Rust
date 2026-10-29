use anyhow::{bail, Result};

use crate::config::PluginConfig;
use crate::plugin::{theme_for, Env, InputKind, Plugin, ThemeChoice, Variant};

/// The user's own shell commands: the "theme" of each variant is the command to run.
pub struct Custom;

impl Plugin for Custom {
    fn key(&self) -> &'static str {
        "custom"
    }

    fn name(&self) -> &'static str {
        "Custom"
    }

    fn kind(&self) -> InputKind {
        InputKind::Text
    }

    fn defaults(&self) -> (Option<&'static str>, Option<&'static str>) {
        (None, None)
    }

    fn available(&self, _env: &Env) -> bool {
        true
    }

    fn themes(&self, _env: &Env) -> Vec<ThemeChoice> {
        Vec::new()
    }

    /// Runs the command with `sh -c`, so pipes, quotes and `&` work as in a terminal. Its output goes
    /// to ours (the journal under systemd), where a failing script can be debugged.
    fn apply(&self, env: &Env, variant: Variant, cfg: &PluginConfig) -> Result<()> {
        let command = theme_for(cfg, variant)?;
        let status = env.exec.run_inherited("sh", &["-c", command])?;
        if status != 0 {
            bail!("custom {variant:?} command exited with status {status}");
        }
        Ok(())
    }
}
