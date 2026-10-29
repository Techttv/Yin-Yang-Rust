use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;

use super::run_checked;
use crate::config::PluginConfig;
use crate::plugin::{theme_for, Env, InputKind, Plugin, ThemeChoice, Variant};

const PROGRAM: &str = "kvantummanager";

/// The Kvantum widget style theme.
pub struct Kvantum {
    system_dir: PathBuf,
}

impl Kvantum {
    pub fn new() -> Kvantum {
        Kvantum::with_system_dir("/usr/share/Kvantum".into())
    }

    pub fn with_system_dir(system_dir: PathBuf) -> Kvantum {
        Kvantum { system_dir }
    }
}

impl Default for Kvantum {
    fn default() -> Self {
        Kvantum::new()
    }
}

/// Collects the names of the `*.kvconfig` files under `dir`, except Kvantum's own settings file
/// (`kvantum.kvconfig` directly in the base directory), which is not a theme.
fn collect_themes(dir: &Path, base: bool, themes: &mut BTreeSet<String>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_dir() {
            collect_themes(&path, false, themes);
        } else if let Some(name) = entry.file_name().to_string_lossy().strip_suffix(".kvconfig")
            && !(base && name == "kvantum")
        {
            themes.insert(name.to_string());
        }
    }
}

impl Plugin for Kvantum {
    fn key(&self) -> &'static str {
        "kvantum"
    }

    fn name(&self) -> &'static str {
        "Kvantum"
    }

    fn kind(&self) -> InputKind {
        InputKind::Choice
    }

    fn defaults(&self) -> (Option<&'static str>, Option<&'static str>) {
        (Some("KvFlatLight"), Some("KvFlat"))
    }

    fn available(&self, env: &Env) -> bool {
        env.exec.exists(PROGRAM)
    }

    fn themes(&self, env: &Env) -> Vec<ThemeChoice> {
        let mut names = BTreeSet::new();
        collect_themes(&self.system_dir, true, &mut names);
        collect_themes(&env.home.join(".config/Kvantum"), true, &mut names);
        names
            .into_iter()
            .map(|name| ThemeChoice { id: name.clone(), label: name })
            .collect()
    }

    fn apply(&self, env: &Env, variant: Variant, cfg: &PluginConfig) -> Result<()> {
        let theme = theme_for(cfg, variant)?;
        run_checked(env, PROGRAM, &["--set", theme])?;
        Ok(())
    }
}
