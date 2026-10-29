use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result};
use chrono::NaiveTime;
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

use crate::paths::Paths;

const CONFIG_VERSION: f64 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    #[serde(rename = "manual")]
    Manual,
    #[serde(rename = "manual time")]
    Scheduled,
    #[serde(rename = "sunset to sunrise")]
    FollowSun,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginConfig {
    pub enabled: bool,
    #[serde(default)]
    pub light_theme: Option<String>,
    #[serde(default)]
    pub dark_theme: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub version: f64,
    pub running: bool,
    pub dark_mode: bool,
    pub mode: Mode,
    pub coordinates: (f64, f64),
    pub update_location: bool,
    pub update_interval: u32,
    #[serde(with = "time_pair")]
    pub times: (NaiveTime, NaiveTime),
    pub boot_offset: u32,
    pub plugins: BTreeMap<String, PluginConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            version: CONFIG_VERSION,
            running: false,
            dark_mode: false,
            mode: Mode::Manual,
            coordinates: (0.0, 0.0),
            update_location: false,
            update_interval: 60,
            times: (
                NaiveTime::from_hms_opt(7, 0, 0).unwrap(),
                NaiveTime::from_hms_opt(20, 0, 0).unwrap(),
            ),
            boot_offset: 10,
            plugins: BTreeMap::new(),
        }
    }
}

impl Config {
    /// Loads our own file; failing that, imports the newest legacy file (never modifying it);
    /// failing that, returns the defaults.
    pub fn load(paths: &Paths) -> Result<Config> {
        if paths.config_file.is_file() {
            return read(&paths.config_file);
        }

        let newest = paths
            .legacy_config_files()
            .into_iter()
            .filter_map(|p| Some((fs::metadata(&p).ok()?.modified().ok()?, p)))
            .max_by_key(|(modified, _)| *modified);

        match newest {
            Some((_, path)) => read(&path).or_else(|e| {
                log::warn!("ignoring unreadable legacy config {}: {e:#}", path.display());
                Ok(Config::default())
            }),
            None => Ok(Config::default()),
        }
    }

    /// Atomic write: temporary file in the same directory, then rename.
    pub fn save(&self, paths: &Paths) -> Result<()> {
        let dir = paths
            .config_file
            .parent()
            .context("config file has no parent directory")?;
        fs::create_dir_all(dir)?;
        let mut tmp = NamedTempFile::new_in(dir)?;
        serde_json::to_writer_pretty(&mut tmp, self)?;
        tmp.write_all(b"\n")?;
        tmp.persist(&paths.config_file)
            .with_context(|| format!("writing {}", paths.config_file.display()))?;
        Ok(())
    }

    /// Adds, disabled, the plugins missing from the config.
    pub fn fill_defaults(&mut self, defaults: &[(String, Option<String>, Option<String>)]) {
        for (key, light, dark) in defaults {
            self.plugins
                .entry(key.clone())
                .or_insert_with(|| PluginConfig {
                    enabled: false,
                    light_theme: light.clone(),
                    dark_theme: dark.clone(),
                });
        }
    }
}

fn read(path: &Path) -> Result<Config> {
    let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let value: serde_json::Value =
        serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
    let version = value.get("version").and_then(|v| v.as_f64()).unwrap_or(0.0);
    if version < CONFIG_VERSION {
        log::warn!(
            "{} has config version {version}; older versions are not migrated, using defaults",
            path.display()
        );
        return Ok(Config::default());
    }
    serde_json::from_value(value).with_context(|| format!("parsing {}", path.display()))
}

/// `["HH:MM", "HH:MM"]` on write; `HH:MM` and `HH:MM:SS` (the original's format) on read.
mod time_pair {
    use chrono::NaiveTime;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(
        times: &(NaiveTime, NaiveTime),
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let fmt = |t: &NaiveTime| t.format("%H:%M").to_string();
        (fmt(&times.0), fmt(&times.1)).serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<(NaiveTime, NaiveTime), D::Error> {
        let (a, b) = <(String, String)>::deserialize(deserializer)?;
        let parse = |s: &str| {
            NaiveTime::parse_from_str(s, "%H:%M")
                .or_else(|_| NaiveTime::parse_from_str(s, "%H:%M:%S"))
                .map_err(serde::de::Error::custom)
        };
        Ok((parse(&a)?, parse(&b)?))
    }
}
