use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;

use super::run_checked;
use crate::config::PluginConfig;
use crate::plugin::{theme_for, Env, InputKind, Plugin, ThemeChoice, Variant};

const PROGRAM: &str = "/usr/lib/plasma-changeicons";

/// The Plasma icon theme.
pub struct Icons {
    system_dir: PathBuf,
}

impl Icons {
    pub fn new() -> Icons {
        Icons::with_system_dir("/usr/share/icons".into())
    }

    pub fn with_system_dir(system_dir: PathBuf) -> Icons {
        Icons { system_dir }
    }
}

impl Default for Icons {
    fn default() -> Self {
        Icons::new()
    }
}

/// The display name of an icon theme directory, or `None` when it is not a usable icon theme:
/// it needs an `index.theme` and an `icon-theme.cache`, and some section with a `Size` key
/// (which leaves out cursor themes).
fn icon_theme_name(dir: &Path) -> Option<String> {
    if !dir.join("icon-theme.cache").is_file() {
        return None;
    }
    let index = fs::read_to_string(dir.join("index.theme")).ok()?;

    let (mut section, mut name, mut has_size) = (String::new(), None, false);
    for line in index.lines().map(str::trim) {
        if let Some(header) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            section = header.to_string();
        } else if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            has_size |= key.eq_ignore_ascii_case("Size");
            if section == "Icon Theme" && key.eq_ignore_ascii_case("Name") && name.is_none() {
                name = Some(value.trim().to_string());
            }
        }
    }

    has_size.then(|| name.filter(|n| !n.is_empty()).unwrap_or_else(|| dir_name(dir)))
}

fn dir_name(dir: &Path) -> String {
    dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

impl Plugin for Icons {
    fn key(&self) -> &'static str {
        "icons"
    }

    fn name(&self) -> &'static str {
        "Icons"
    }

    fn kind(&self) -> InputKind {
        InputKind::Choice
    }

    fn defaults(&self) -> (Option<&'static str>, Option<&'static str>) {
        (Some("breeze"), Some("breeze-dark"))
    }

    fn available(&self, env: &Env) -> bool {
        env.kde && env.exec.exists(PROGRAM)
    }

    fn themes(&self, env: &Env) -> Vec<ThemeChoice> {
        // The user's directory comes last so it wins over a system theme with the same id.
        let mut themes = BTreeMap::new();
        for base in [self.system_dir.clone(), env.home.join(".icons")] {
            let Ok(entries) = fs::read_dir(&base) else { continue };
            for dir in entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()) {
                if let Some(label) = icon_theme_name(&dir) {
                    themes.insert(dir_name(&dir), label);
                }
            }
        }
        themes.into_iter().map(|(id, label)| ThemeChoice { id, label }).collect()
    }

    fn apply(&self, env: &Env, variant: Variant, cfg: &PluginConfig) -> Result<()> {
        let theme = theme_for(cfg, variant)?;
        run_checked(env, PROGRAM, &[theme])?;
        Ok(())
    }
}
