use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{bail, Result};
use serde::Serialize;

use crate::bus::Bus;
use crate::config::PluginConfig;
use crate::exec::Exec;
use crate::plugins::{
    colors::Colors, gtk::Gtk, icons::Icons, kvantum::Kvantum, notification::Notification,
    okular::Okular, onlyoffice::OnlyOffice, system::System, vscode::Vscode, wallpaper::Wallpaper,
};

/// Everything a plugin may touch, so tests can swap it.
pub struct Env {
    pub home: PathBuf,
    /// Running under KDE Plasma; the desktop-dependent plugins are unavailable otherwise.
    pub kde: bool,
    pub exec: Arc<dyn Exec>,
    pub bus: Arc<dyn Bus>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    Light,
    Dark,
}

/// How the GUI lets the user pick a theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum InputKind {
    /// One of `Plugin::themes`.
    Choice,
    /// Free text.
    Text,
    /// A file path.
    File,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ThemeChoice {
    pub id: String,
    pub label: String,
}

pub trait Plugin: Send + Sync {
    /// Key in the config file's `plugins` map.
    fn key(&self) -> &'static str;
    /// Name shown in the GUI.
    fn name(&self) -> &'static str;
    fn kind(&self) -> InputKind;
    /// Light and dark theme a fresh config starts with.
    fn defaults(&self) -> (Option<&'static str>, Option<&'static str>);
    fn available(&self, env: &Env) -> bool;
    fn themes(&self, env: &Env) -> Vec<ThemeChoice>;
    /// Switches to the theme `cfg` names for `variant`. Plugins that need both themes read `cfg` directly.
    fn apply(&self, env: &Env, variant: Variant, cfg: &PluginConfig) -> Result<()>;
    /// What `apply` would do, for `--dry-run`.
    fn describe(&self, variant: Variant, cfg: &PluginConfig) -> String {
        format!("{}: {}", self.key(), theme_for(cfg, variant).unwrap_or("-"))
    }
    /// Notification plugins stay silent when the user explicitly applies the settings.
    fn is_notification(&self) -> bool {
        false
    }
}

/// The configured theme for `variant`; an unset or empty theme is an error.
pub fn theme_for(cfg: &PluginConfig, variant: Variant) -> Result<&str> {
    let theme = match variant {
        Variant::Light => &cfg.light_theme,
        Variant::Dark => &cfg.dark_theme,
    };
    match theme.as_deref() {
        Some(theme) if !theme.is_empty() => Ok(theme),
        _ => bail!("no {variant:?} theme is set"),
    }
}

/// Every plugin, in the order the GUI shows them.
pub fn registry() -> Vec<Box<dyn Plugin>> {
    vec![
        Box::new(System::new()),
        Box::new(Colors),
        Box::new(Gtk::new()),
        Box::new(Icons::new()),
        Box::new(Kvantum::new()),
        Box::new(Wallpaper),
        Box::new(Vscode::new()),
        Box::new(OnlyOffice),
        Box::new(Okular),
        Box::new(Notification),
    ]
}
