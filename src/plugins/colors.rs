use anyhow::Result;

use super::run_checked;
use crate::config::PluginConfig;
use crate::plugin::{theme_for, Env, InputKind, Plugin, ThemeChoice, Variant};

const PROGRAM: &str = "plasma-apply-colorscheme";

/// The Plasma colour scheme.
pub struct Colors;

impl Plugin for Colors {
    fn key(&self) -> &'static str {
        "colors"
    }

    fn name(&self) -> &'static str {
        "Colors"
    }

    fn kind(&self) -> InputKind {
        InputKind::Choice
    }

    fn defaults(&self) -> (Option<&'static str>, Option<&'static str>) {
        (None, None)
    }

    fn available(&self, env: &Env) -> bool {
        env.kde && env.exec.exists(PROGRAM)
    }

    fn themes(&self, env: &Env) -> Vec<ThemeChoice> {
        let Ok(output) = env.exec.run(PROGRAM, &["--list-schemes"]) else {
            return Vec::new();
        };
        if output.status != 0 {
            return Vec::new();
        }

        // The header line is translated; only the " * name" lines matter, and the active scheme
        // carries a translated "(...)" suffix.
        let mut names: Vec<&str> = output
            .stdout
            .lines()
            .filter_map(|line| line.trim_start().strip_prefix("* "))
            .map(|name| name.split('(').next().unwrap_or(name).trim())
            .filter(|name| !name.is_empty())
            .collect();
        names.sort_unstable();
        names.dedup();
        names
            .into_iter()
            .map(|name| ThemeChoice { id: name.to_string(), label: name.to_string() })
            .collect()
    }

    fn apply(&self, env: &Env, variant: Variant, cfg: &PluginConfig) -> Result<()> {
        let theme = theme_for(cfg, variant)?;
        run_checked(env, PROGRAM, &[theme])?;
        Ok(())
    }
}
