use std::collections::BTreeSet;
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::config::PluginConfig;
use crate::plugin::{theme_for, Env, InputKind, Plugin, ThemeChoice, Variant};

const GTK_CONFIG: &str = "org.kde.GtkConfig";

/// The GTK theme, through kde-gtk-config when it runs, else through xsettingsd.
pub struct Gtk {
    system_dirs: Vec<PathBuf>,
}

impl Gtk {
    pub fn new() -> Gtk {
        Gtk::with_system_dirs(vec!["/usr/share/themes".into(), "/usr/local/share/themes".into()])
    }

    pub fn with_system_dirs(system_dirs: Vec<PathBuf>) -> Gtk {
        Gtk { system_dirs }
    }
}

impl Default for Gtk {
    fn default() -> Self {
        Gtk::new()
    }
}

/// Runs a helper whose failure must not undo what was already done; problems are only logged.
fn best_effort(env: &Env, program: &str, args: &[&str]) {
    match env.exec.run(program, args) {
        Ok(output) if output.status != 0 => log::warn!("{program} exited with status {}", output.status),
        Ok(_) => {}
        Err(e) => log::warn!("{e:#}"),
    }
}

impl Plugin for Gtk {
    fn key(&self) -> &'static str {
        "gtk"
    }

    fn name(&self) -> &'static str {
        "GTK"
    }

    fn kind(&self) -> InputKind {
        InputKind::Choice
    }

    fn defaults(&self) -> (Option<&'static str>, Option<&'static str>) {
        (Some("Breeze"), Some("Breeze"))
    }

    fn available(&self, env: &Env) -> bool {
        env.kde
    }

    fn themes(&self, env: &Env) -> Vec<ThemeChoice> {
        let user_dirs = [env.home.join(".themes"), env.home.join(".local/share/themes")];
        let mut names = BTreeSet::new();
        for base in self.system_dirs.iter().chain(&user_dirs) {
            let Ok(entries) = fs::read_dir(base) else { continue };
            for dir in entries.flatten().map(|e| e.path()) {
                if dir.join("gtk-3.0").is_dir()
                    && let Some(name) = dir.file_name()
                {
                    names.insert(name.to_string_lossy().into_owned());
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

        // Without a bus we cannot ask, which is the same as the service not running.
        let registered = env
            .bus
            .service_names()
            .map(|names| names.iter().any(|name| name == GTK_CONFIG))
            .unwrap_or(false);
        if registered {
            return env.bus.call_str(GTK_CONFIG, "/GtkConfig", GTK_CONFIG, "setGtkTheme", theme);
        }

        log::warn!("kde-gtk-config is not running, trying xsettingsd");
        let conf = env.home.join(".config/xsettingsd/xsettingsd.conf");
        let text = match fs::read_to_string(&conf) {
            Ok(text) => text,
            Err(e) if e.kind() == ErrorKind::NotFound => {
                log::warn!("xsettingsd is not configured; the GTK theme is left alone");
                return Ok(());
            }
            Err(e) => return Err(e).with_context(|| format!("reading {}", conf.display())),
        };

        let mut replaced = false;
        let updated: String = text
            .split_inclusive('\n')
            .map(|line| {
                if !replaced && line.starts_with("Net/ThemeName") {
                    replaced = true;
                    format!("Net/ThemeName \"{theme}\"\n")
                } else {
                    line.to_string()
                }
            })
            .collect();
        if updated != text {
            fs::write(&conf, updated).with_context(|| format!("writing {}", conf.display()))?;
        }

        // xsettingsd re-reads its file on SIGHUP; gsettings covers the apps that use dconf.
        best_effort(env, "killall", &["-HUP", "xsettingsd"]);
        best_effort(env, "gsettings", &["set", "org.gnome.desktop.interface", "gtk-theme", theme]);
        let scheme = match variant {
            Variant::Dark => "prefer-dark",
            Variant::Light => "prefer-light",
        };
        best_effort(env, "gsettings", &["set", "org.gnome.desktop.interface", "color-scheme", scheme]);
        Ok(())
    }
}
