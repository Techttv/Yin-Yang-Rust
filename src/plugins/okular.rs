use std::path::PathBuf;

use anyhow::Result;

use super::edit_files;
use crate::config::PluginConfig;
use crate::edit::ini::{remove_key, remove_section_if_empty, set_value};
use crate::plugin::{Env, InputKind, Plugin, ThemeChoice, Variant};

const SECTION: &str = "Document";
const SERVICE_PREFIX: &str = "org.kde.okular-";

/// Okular's accessibility colour change: the light "theme" is always "leave the colours alone",
/// the dark one is the render mode used while dark. An empty or missing dark theme is the plain
/// "Invert colors" mode.
pub struct Okular;

fn config_files(env: &Env) -> Vec<PathBuf> {
    vec![
        env.home.join(".config/okularpartrc"),
        env.home.join(".var/app/org.kde.okular/config/okularpartrc"),
    ]
}

/// Tells the running Okular windows to switch now; they only read the config at start-up.
/// Failing to reach them is not a failure: the config is what matters for the next start.
fn switch_running_instances(env: &Env, dark: bool) {
    let services = match env.bus.service_names() {
        Ok(services) => services,
        Err(e) => {
            log::warn!("cannot look for running Okular windows: {e:#}");
            return;
        }
    };
    for service in services.iter().filter(|s| s.starts_with(SERVICE_PREFIX)) {
        if let Err(e) = env.bus.call_bool(service, "/okular", "org.kde.okular", "slotSetChangeColors", dark) {
            log::warn!("cannot switch {service}: {e:#}");
        }
    }
}

impl Plugin for Okular {
    fn key(&self) -> &'static str {
        "okular"
    }

    fn name(&self) -> &'static str {
        "Okular"
    }

    fn kind(&self) -> InputKind {
        InputKind::Choice
    }

    fn defaults(&self) -> (Option<&'static str>, Option<&'static str>) {
        (Some(""), Some("InvertLightness"))
    }

    fn available(&self, env: &Env) -> bool {
        config_files(env).iter().any(|p| p.is_file())
    }

    fn themes(&self, _env: &Env) -> Vec<ThemeChoice> {
        [
            ("", "Invert colors"),
            ("InvertLightness", "Invert lightness"),
            ("InvertLuma", "Invert luma (sRGB linear)"),
            ("InvertLumaSymmetric", "Invert luma (symmetrical)"),
        ]
        .into_iter()
        .map(|(id, label)| ThemeChoice { id: id.into(), label: label.into() })
        .collect()
    }

    fn apply(&self, env: &Env, variant: Variant, cfg: &PluginConfig) -> Result<()> {
        let dark = variant == Variant::Dark;
        let render_mode = cfg.dark_theme.as_deref().unwrap_or("");

        switch_running_instances(env, dark);

        edit_files(&config_files(env), |text| {
            let text = if render_mode.is_empty() {
                remove_section_if_empty(&remove_key(text, SECTION, "RenderMode"), SECTION)
            } else {
                set_value(text, SECTION, "RenderMode", render_mode)
            };
            Ok(if dark {
                set_value(&text, SECTION, "ChangeColors", "true")
            } else {
                remove_section_if_empty(&remove_key(&text, SECTION, "ChangeColors"), SECTION)
            })
        })?;
        Ok(())
    }

    fn describe(&self, variant: Variant, cfg: &PluginConfig) -> String {
        match variant {
            Variant::Light => "okular: colors unchanged".to_string(),
            Variant::Dark => match cfg.dark_theme.as_deref().unwrap_or("") {
                "" => "okular: render mode Invert colors".to_string(),
                mode => format!("okular: render mode {mode}"),
            },
        }
    }
}
