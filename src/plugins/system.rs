use std::fs;
use std::path::PathBuf;

use anyhow::Result;

use super::{locales_from_env, run_checked};
use crate::config::PluginConfig;
use crate::plugin::{theme_for, Env, InputKind, Plugin, ThemeChoice, Variant};

const PROGRAM: &str = "lookandfeeltool";

/// The Plasma look-and-feel.
pub struct System {
    system_dir: PathBuf,
    locales: Vec<String>,
}

impl System {
    pub fn new() -> System {
        System::with_dirs("/usr/share/plasma/look-and-feel".into(), locales_from_env())
    }

    pub fn with_dirs(system_dir: PathBuf, locales: Vec<String>) -> System {
        System { system_dir, locales }
    }

    /// The translated name from the theme's metadata, or the id when there is none.
    fn label(&self, env: &Env, id: &str) -> String {
        let dirs = [
            env.home.join(".local/share/plasma/look-and-feel").join(id),
            self.system_dir.join(id),
        ];
        for dir in dirs {
            if let Some(name) = fs::read_to_string(dir.join("metadata.json"))
                .ok()
                .and_then(|text| json_name(&text, &self.locales))
            {
                return name;
            }
            if let Some(name) = fs::read_to_string(dir.join("metadata.desktop"))
                .ok()
                .and_then(|text| desktop_name(&text, &self.locales))
            {
                return name;
            }
        }
        id.to_string()
    }
}

impl Default for System {
    fn default() -> Self {
        System::new()
    }
}

fn json_name(text: &str, locales: &[String]) -> Option<String> {
    let json: serde_json::Value = serde_json::from_str(text).ok()?;
    let plugin = json.get("KPlugin")?;
    locales
        .iter()
        .map(|locale| format!("Name[{locale}]"))
        .chain(["Name".to_string()])
        .find_map(|key| plugin.get(&key)?.as_str().filter(|s| !s.is_empty()).map(String::from))
}

fn desktop_name(text: &str, locales: &[String]) -> Option<String> {
    locales
        .iter()
        .map(|locale| format!("Name[{locale}]="))
        .chain(["Name=".to_string()])
        .find_map(|prefix| {
            text.lines()
                .find_map(|line| line.strip_prefix(&prefix))
                .map(|name| name.trim().to_string())
                .filter(|name| !name.is_empty())
        })
}

impl Plugin for System {
    fn key(&self) -> &'static str {
        "system"
    }

    fn name(&self) -> &'static str {
        "System"
    }

    fn kind(&self) -> InputKind {
        InputKind::Choice
    }

    fn defaults(&self) -> (Option<&'static str>, Option<&'static str>) {
        (Some("org.kde.breeze.desktop"), Some("org.kde.breezedark.desktop"))
    }

    fn available(&self, env: &Env) -> bool {
        env.kde && env.exec.exists(PROGRAM)
    }

    fn themes(&self, env: &Env) -> Vec<ThemeChoice> {
        let Ok(output) = env.exec.run(PROGRAM, &["-l"]) else {
            return Vec::new();
        };
        if output.status != 0 {
            return Vec::new();
        }

        let mut ids: Vec<&str> = output.stdout.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
        ids.sort_unstable();
        ids.dedup();
        ids.into_iter()
            .map(|id| ThemeChoice { id: id.to_string(), label: self.label(env, id) })
            .collect()
    }

    fn apply(&self, env: &Env, variant: Variant, cfg: &PluginConfig) -> Result<()> {
        let theme = theme_for(cfg, variant)?;
        run_checked(env, PROGRAM, &["-a", theme])?;
        Ok(())
    }
}
