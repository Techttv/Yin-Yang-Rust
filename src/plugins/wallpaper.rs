use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use anyhow::{bail, Result};

use crate::config::PluginConfig;
use crate::plugin::{theme_for, Env, InputKind, Plugin, ThemeChoice, Variant};

/// The Plasma wallpaper of every desktop; the "theme" is an image file path.
pub struct Wallpaper;

/// `file://` URL with everything but unreserved characters and `/` percent-encoded, so `#`, spaces
/// and non-ASCII work, and nothing in the path can end the string it is pasted into.
fn file_url(path: &Path) -> String {
    let mut url = String::from("file://");
    for &byte in path.as_os_str().as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                url.push(char::from(byte));
            }
            _ => url.push_str(&format!("%{byte:02X}")),
        }
    }
    url
}

impl Plugin for Wallpaper {
    fn key(&self) -> &'static str {
        "wallpaper"
    }

    fn name(&self) -> &'static str {
        "Wallpaper"
    }

    fn kind(&self) -> InputKind {
        InputKind::File
    }

    fn defaults(&self) -> (Option<&'static str>, Option<&'static str>) {
        (None, None)
    }

    fn available(&self, env: &Env) -> bool {
        env.kde
    }

    fn themes(&self, _env: &Env) -> Vec<ThemeChoice> {
        Vec::new()
    }

    fn apply(&self, env: &Env, variant: Variant, cfg: &PluginConfig) -> Result<()> {
        let image = Path::new(theme_for(cfg, variant)?);
        if !image.is_absolute() {
            bail!("the wallpaper path must be absolute: {}", image.display());
        }
        if !image.is_file() {
            bail!("the wallpaper image does not exist: {}", image.display());
        }

        let script = format!(
            "var Desktops = desktops();\
             for (let i = 0; i < Desktops.length; i++) {{\
                 let d = Desktops[i];\
                 d.wallpaperPlugin = \"org.kde.image\";\
                 d.currentConfigGroup = Array(\"Wallpaper\", \"org.kde.image\", \"General\");\
                 d.writeConfig(\"Image\", \"{}\");\
             }}",
            file_url(image)
        );
        env.bus.call_str(
            "org.kde.plasmashell",
            "/PlasmaShell",
            "org.kde.PlasmaShell",
            "evaluateScript",
            &script,
        )
    }
}
