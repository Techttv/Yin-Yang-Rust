use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::edit_files;
use crate::config::PluginConfig;
use crate::edit::ini::{get_value, set_value};
use crate::plugin::{theme_for, Env, InputKind, Plugin, ThemeChoice, Variant};

/// Applications that embed Konsole; their windows and sessions are switched live.
const APPS_WITH_KONSOLE: [&str; 4] = ["org.kde.konsole", "org.kde.yakuake", "org.kde.dolphin", "org.kde.kate"];

/// Colour schemes compiled into Konsole, which no file lists.
const BUILT_IN_SCHEMES: [(&str, &str); 13] = [
    ("BlackOnLightYellow", "Black on Light Yellow"),
    ("BlackOnRandomLight", "Black on Random Light"),
    ("BlackOnWhite", "Black on White"),
    ("BlueOnBlack", "Blue on Black"),
    ("Breeze", "Breeze"),
    ("Campbell", "Campbell"),
    ("DarkPastels", "Dark Pastels"),
    ("GreenOnBlack", "Green on Black"),
    ("Linux", "Linux Colors"),
    ("RedOnBlack", "Red on Black"),
    ("Solarized", "Solarized"),
    ("SolarizedLight", "Solarized Light"),
    ("WhiteOnBlack", "White on Black"),
];

/// Profile used as the base when the user has none at all.
const FALLBACK_PROFILE: &str =
    "[Appearance]\nColorScheme=Breeze\n\n[General]\nCommand=/bin/bash\nName=YinYang\nParent=FALLBACK/\n";

/// Konsole's colour scheme. A scheme can only be switched live through a profile, so this keeps a
/// `Light` and a `Dark` profile, sets their colour schemes, makes the right one the default for
/// new windows and switches the running windows and sessions to it.
pub struct Konsole {
    global_dir: PathBuf,
}

impl Konsole {
    pub fn new() -> Konsole {
        Konsole::with_global_dir("/usr/share/konsole".into())
    }

    pub fn with_global_dir(global_dir: PathBuf) -> Konsole {
        Konsole { global_dir }
    }
}

impl Default for Konsole {
    fn default() -> Self {
        Konsole::new()
    }
}

fn user_dir(env: &Env) -> PathBuf {
    env.home.join(".local/share/konsole")
}

fn konsolerc(env: &Env) -> PathBuf {
    env.home.join(".config/konsolerc")
}

/// The profile the new ones are copied from: the user's default one, else the first profile there
/// is, else a minimal one we write.
fn base_profile(user_dir: &Path, konsolerc: &str) -> Result<String> {
    let configured = get_value(konsolerc, "Desktop Entry", "DefaultProfile")
        .filter(|name| !name.is_empty() && user_dir.join(name).is_file());
    if let Some(name) = configured {
        return Ok(name);
    }

    let mut profiles: Vec<String> = fs::read_dir(user_dir)?
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "profile"))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    profiles.sort();
    if let Some(first) = profiles.into_iter().next() {
        log::warn!("no usable default Konsole profile, using {first}");
        return Ok(first);
    }

    let fallback = user_dir.join("Default.profile");
    fs::write(&fallback, FALLBACK_PROFILE).with_context(|| format!("writing {}", fallback.display()))?;
    Ok("Default.profile".to_string())
}

/// The names of the child objects in an introspection document (`  <node name="1"/>` lines).
fn child_nodes(xml: &str) -> Vec<String> {
    xml.lines()
        .filter_map(|line| line.strip_prefix("  <node name=\"")?.split('"').next())
        .map(String::from)
        .collect()
}

/// Points every running Konsole window (new tabs) and session (open tabs) at `profile`.
/// Whatever cannot be reached is only logged: the files are what the next start reads.
fn switch_running(env: &Env, profile: &str) {
    let services = match env.bus.service_names() {
        Ok(services) => services,
        Err(e) => {
            log::warn!("cannot look for running Konsole windows: {e:#}");
            return;
        }
    };
    let konsole_services = services
        .iter()
        .filter(|name| APPS_WITH_KONSOLE.contains(&name.split('-').next().unwrap_or_default()));

    for service in konsole_services {
        for (root, interface, method) in [
            ("/Windows", "org.kde.konsole.Window", "setDefaultProfile"),
            ("/Sessions", "org.kde.konsole.Session", "setProfile"),
        ] {
            let xml = match env.bus.introspect(service, root) {
                Ok(xml) => xml,
                Err(e) => {
                    log::warn!("cannot list {service} {root}: {e:#}");
                    continue;
                }
            };
            for child in child_nodes(&xml) {
                if let Err(e) = env.bus.call_str(service, &format!("{root}/{child}"), interface, method, profile) {
                    log::warn!("cannot switch {service} {root}/{child}: {e:#}");
                }
            }
        }
    }
}

impl Plugin for Konsole {
    fn key(&self) -> &'static str {
        "konsole"
    }

    fn name(&self) -> &'static str {
        "Konsole"
    }

    fn kind(&self) -> InputKind {
        InputKind::Choice
    }

    fn defaults(&self) -> (Option<&'static str>, Option<&'static str>) {
        (Some("BlackOnWhite"), Some("Breeze"))
    }

    fn available(&self, env: &Env) -> bool {
        self.global_dir.is_dir() || user_dir(env).is_dir()
    }

    fn themes(&self, env: &Env) -> Vec<ThemeChoice> {
        if !self.available(env) {
            return Vec::new();
        }

        let mut schemes: BTreeMap<String, String> = BUILT_IN_SCHEMES
            .iter()
            .map(|(id, label)| (id.to_string(), label.to_string()))
            .collect();
        for dir in [self.global_dir.clone(), user_dir(env)] {
            let Ok(entries) = fs::read_dir(dir) else { continue };
            for path in entries.flatten().map(|e| e.path()) {
                if path.extension().is_none_or(|ext| ext != "colorscheme") || !path.is_file() {
                    continue;
                }
                let id = path.file_stem().unwrap_or_default().to_string_lossy().into_owned();
                let label = fs::read_to_string(&path)
                    .ok()
                    .and_then(|text| get_value(&text, "General", "Description"))
                    .filter(|description| !description.is_empty())
                    .unwrap_or_else(|| id.clone());
                schemes.insert(id, label);
            }
        }
        schemes.into_iter().map(|(id, label)| ThemeChoice { id, label }).collect()
    }

    fn apply(&self, env: &Env, variant: Variant, cfg: &PluginConfig) -> Result<()> {
        theme_for(cfg, variant)?;
        let set = |theme: &Option<String>| theme.clone().filter(|t| !t.is_empty());
        let schemes = [("Light", set(&cfg.light_theme)), ("Dark", set(&cfg.dark_theme))];

        let dir = user_dir(env);
        fs::create_dir_all(&dir)?;
        let rc_path = konsolerc(env);
        let rc_text = fs::read_to_string(&rc_path).unwrap_or_default();
        let base = base_profile(&dir, &rc_text)?;

        for (name, scheme) in &schemes {
            let path = dir.join(format!("{name}.profile"));
            if !path.exists() {
                let copy = fs::read_to_string(dir.join(&base))
                    .with_context(|| format!("reading the profile {base}"))?;
                fs::write(&path, set_value(&copy, "General", "Name", name))
                    .with_context(|| format!("writing {}", path.display()))?;
            }
            if let Some(scheme) = scheme {
                edit_files(&[path], |text| Ok(set_value(text, "Appearance", "ColorScheme", scheme)))?;
            }
        }

        let profile = match variant {
            Variant::Light => "Light",
            Variant::Dark => "Dark",
        };
        if rc_path.is_file() {
            let value = format!("{profile}.profile");
            edit_files(&[rc_path], |text| Ok(set_value(text, "Desktop Entry", "DefaultProfile", &value)))?;
        } else {
            log::warn!("no konsolerc: new Konsole windows keep their current default profile");
        }

        switch_running(env, profile);
        Ok(())
    }
}
