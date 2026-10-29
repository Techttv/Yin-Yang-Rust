use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use anyhow::Result;

use super::edit_files;
use crate::config::PluginConfig;
use crate::edit::jsonc;
use crate::plugin::{theme_for, Env, InputKind, Plugin, ThemeChoice, Variant};

/// `settings.json` of each editor flavour, relative to the home directory (`~/.var/app` = Flatpak).
const SETTINGS_FILES: [&str; 7] = [
    ".config/VSCodium/User/settings.json",
    ".config/Code - OSS/User/settings.json",
    ".config/Code/User/settings.json",
    ".config/Code - Insiders/User/settings.json",
    ".var/app/com.visualstudio.code/config/Code/User/settings.json",
    ".var/app/com.visualstudio.code-oss/config/Code - OSS/User/settings.json",
    ".var/app/com.vscodium.codium/config/VSCodium/User/settings.json",
];

/// Extension directories relative to the home directory.
const HOME_EXTENSION_DIRS: [&str; 6] = [
    ".vscode/extensions",
    ".vscode-insiders/extensions",
    ".vscode-oss/extensions",
    ".var/app/com.visualstudio.code/data/vscode/extensions",
    ".var/app/com.visualstudio.code-oss/data/vscode/extensions",
    ".var/app/com.vscodium.codium/data/codium/extensions",
];

/// Where distributions, Snap and Flatpak keep the extensions bundled with the editor.
const SYSTEM_EXTENSION_DIRS: [&str; 16] = [
    "/usr/lib/code/extensions",
    "/usr/lib/code-insiders/extensions",
    "/usr/share/code/resources/app/extensions",
    "/usr/share/code-insiders/resources/app/extensions",
    "/usr/share/vscodium/resources/app/extensions",
    "/usr/share/vscodium-git/resources/app/extensions",
    "/usr/share/vscodium-insiders/resources/app/extensions",
    "/usr/share/vscodium-insiders-bin/resources/app/extensions",
    "/opt/visual-studio-code/resources/app/extensions",
    "/opt/visual-studio-code-insiders/resources/app/extensions",
    "/opt/vscodium-bin/resources/app/extensions",
    "/var/lib/snapd/snap/code/current/usr/share/code/resources/app/extensions",
    "/var/lib/snapd/snap/code-insiders/current/usr/share/code-insiders/resources/app/extensions",
    "/var/lib/flatpak/app/com.visualstudio.code/current/active/files/extra/vscode/resources/app/extensions",
    "/var/lib/flatpak/app/com.visualstudio.code-oss/current/active/files/main/resources/app/extensions",
    "/var/lib/flatpak/app/com.vscodium.codium/current/active/files/share/codium/resources/app/extensions",
];

/// The colour theme of VS Code and its variants (Code - OSS, VSCodium, Insiders, Flatpak).
pub struct Vscode {
    system_extension_dirs: Vec<PathBuf>,
}

impl Vscode {
    pub fn new() -> Vscode {
        Vscode::with_system_extension_dirs(SYSTEM_EXTENSION_DIRS.iter().map(PathBuf::from).collect())
    }

    pub fn with_system_extension_dirs(system_extension_dirs: Vec<PathBuf>) -> Vscode {
        Vscode { system_extension_dirs }
    }
}

impl Default for Vscode {
    fn default() -> Self {
        Vscode::new()
    }
}

/// Theme identifiers an extension's `package.json` contributes: `id`, else `label`.
fn contributed_themes(manifest: &serde_json::Value) -> Vec<String> {
    let contributes = &manifest["contributes"];
    let themes = contributes.get("themes").or_else(|| contributes.get("Themes"));
    themes
        .and_then(|t| t.as_array())
        .into_iter()
        .flatten()
        .filter_map(|theme| theme.get("id").or_else(|| theme.get("label"))?.as_str().map(String::from))
        .collect()
}

impl Plugin for Vscode {
    fn key(&self) -> &'static str {
        "vs code"
    }

    fn name(&self) -> &'static str {
        "VS Code"
    }

    fn kind(&self) -> InputKind {
        InputKind::Choice
    }

    fn defaults(&self) -> (Option<&'static str>, Option<&'static str>) {
        (Some("Default Light Modern"), Some("Default Dark Modern"))
    }

    fn available(&self, env: &Env) -> bool {
        !self.themes(env).is_empty()
    }

    fn themes(&self, env: &Env) -> Vec<ThemeChoice> {
        let home_dirs = HOME_EXTENSION_DIRS.iter().map(|rel| env.home.join(rel));
        let mut names = BTreeSet::new();
        for base in self.system_extension_dirs.iter().cloned().chain(home_dirs) {
            let Ok(entries) = fs::read_dir(&base) else { continue };
            for extension in entries.flatten() {
                if extension.file_name() == "node_modules" {
                    continue;
                }
                let manifest = extension.path().join("package.json");
                let Ok(text) = fs::read_to_string(&manifest) else { continue };
                match serde_json::from_str::<serde_json::Value>(&text) {
                    Ok(json) => names.extend(contributed_themes(&json)),
                    Err(e) => log::warn!("ignoring {}: {e}", manifest.display()),
                }
            }
        }
        names
            .into_iter()
            .map(|name| ThemeChoice { id: name.clone(), label: name })
            .collect()
    }

    fn apply(&self, env: &Env, variant: Variant, cfg: &PluginConfig) -> Result<()> {
        let theme = theme_for(cfg, variant)?;
        let files: Vec<PathBuf> = SETTINGS_FILES.iter().map(|rel| env.home.join(rel)).collect();
        let found = edit_files(&files, |text| jsonc::set_top_level_string(text, "workbench.colorTheme", theme))?;
        if found == 0 {
            log::warn!("no VS Code settings.json found; open VS Code once so it creates one");
        }
        Ok(())
    }
}
