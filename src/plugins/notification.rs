use anyhow::Result;

use crate::config::PluginConfig;
use crate::plugin::{theme_for, Env, InputKind, Plugin, ThemeChoice, Variant};

/// A desktop notification announcing the switch; the "theme" is just the text to announce.
pub struct Notification;

impl Plugin for Notification {
    fn key(&self) -> &'static str {
        "notification"
    }

    fn name(&self) -> &'static str {
        "Notification"
    }

    fn kind(&self) -> InputKind {
        InputKind::Text
    }

    fn defaults(&self) -> (Option<&'static str>, Option<&'static str>) {
        (Some("Day"), Some("Night"))
    }

    fn available(&self, _env: &Env) -> bool {
        true
    }

    fn themes(&self, _env: &Env) -> Vec<ThemeChoice> {
        Vec::new()
    }

    fn apply(&self, env: &Env, variant: Variant, cfg: &PluginConfig) -> Result<()> {
        let theme = theme_for(cfg, variant)?;
        env.bus.notify("Theme changed", &format!("Set the theme to {theme}"))
    }

    fn is_notification(&self) -> bool {
        true
    }
}
