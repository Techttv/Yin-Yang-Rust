use std::path::PathBuf;

use anyhow::Result;

use super::edit_files;
use crate::config::PluginConfig;
use crate::edit::ini;
use crate::plugin::{theme_for, Env, InputKind, Plugin, ThemeChoice, Variant};

/// The ONLYOFFICE Desktop Editors interface theme.
pub struct OnlyOffice;

fn config_files(env: &Env) -> Vec<PathBuf> {
    vec![
        env.home.join(".config/onlyoffice/DesktopEditors.conf"),
        env.home.join(".var/app/org.onlyoffice.desktopeditors/config/onlyoffice/DesktopEditors.conf"),
    ]
}

impl Plugin for OnlyOffice {
    fn key(&self) -> &'static str {
        "onlyoffice"
    }

    fn name(&self) -> &'static str {
        "OnlyOffice"
    }

    fn kind(&self) -> InputKind {
        InputKind::Choice
    }

    fn defaults(&self) -> (Option<&'static str>, Option<&'static str>) {
        (Some("theme-light"), Some("theme-dark"))
    }

    fn available(&self, env: &Env) -> bool {
        config_files(env).iter().any(|p| p.is_file())
    }

    fn themes(&self, _env: &Env) -> Vec<ThemeChoice> {
        [
            ("theme-system", "System"),
            ("theme-light", "Light"),
            ("theme-classic-light", "Classic light"),
            ("theme-dark", "Dark"),
            ("theme-contrast-dark", "Dark contrast"),
        ]
        .into_iter()
        .map(|(id, label)| ThemeChoice { id: id.into(), label: label.into() })
        .collect()
    }

    fn apply(&self, env: &Env, variant: Variant, cfg: &PluginConfig) -> Result<()> {
        let theme = theme_for(cfg, variant)?;
        edit_files(&config_files(env), |text| Ok(ini::set_value(text, "General", "UITheme", theme)))?;
        Ok(())
    }
}
