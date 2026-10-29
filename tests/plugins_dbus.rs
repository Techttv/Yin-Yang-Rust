mod support;

use std::fs;
use std::path::Path;
use std::sync::Arc;

use support::{FakeBus, FakeExec};
use tempfile::TempDir;
use yinyang::config::PluginConfig;
use yinyang::plugin::{registry, Env, InputKind, Plugin, Variant};
use yinyang::plugins::gtk::Gtk;
use yinyang::plugins::notification::Notification;
use yinyang::plugins::wallpaper::Wallpaper;

fn env(home: &Path, kde: bool, bus: FakeBus) -> (Env, Arc<FakeExec>, Arc<FakeBus>) {
    let exec = Arc::new(FakeExec::new());
    let bus = Arc::new(bus);
    let env = Env { home: home.to_path_buf(), kde, exec: exec.clone(), bus: bus.clone() };
    (env, exec, bus)
}

fn cfg(light: &str, dark: &str) -> PluginConfig {
    PluginConfig { enabled: true, light_theme: Some(light.into()), dark_theme: Some(dark.into()) }
}

fn write(path: &Path, content: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

const GTK_CONFIG: &str = "org.kde.GtkConfig";
const SET_GTK: &str = "call_str org.kde.GtkConfig /GtkConfig org.kde.GtkConfig setGtkTheme";

// ---- gtk ----------------------------------------------------------------------------------

#[test]
fn gtk_uses_kde_gtk_config_when_it_is_on_the_bus() {
    let home = TempDir::new().unwrap();
    let (env, exec, bus) = env(home.path(), true, FakeBus::new().with_services(&[GTK_CONFIG, "org.kde.plasmashell"]));
    Gtk::new().apply(&env, Variant::Dark, &cfg("MacTahoe-Light", "MacTahoe-Dark")).unwrap();
    assert_eq!(bus.calls(), ["service_names".to_string(), format!("{SET_GTK} MacTahoe-Dark")]);
    assert!(exec.calls().is_empty());
}

#[test]
fn gtk_reports_a_failing_kde_gtk_config() {
    let home = TempDir::new().unwrap();
    let bus = FakeBus::new().with_services(&[GTK_CONFIG]);
    bus.fail_matching("call_str", "no reply");
    let (env, _, _) = env(home.path(), true, bus);
    assert!(Gtk::new().apply(&env, Variant::Dark, &cfg("l", "d")).is_err());
}

#[test]
fn gtk_falls_back_to_xsettingsd_keeping_the_rest_of_its_file() {
    let home = TempDir::new().unwrap();
    let conf = home.path().join(".config/xsettingsd/xsettingsd.conf");
    write(&conf, "Net/IconThemeName \"breeze\"\nNet/ThemeName \"Old\"\nGdk/WindowScalingFactor 1\n");
    let (env, exec, _) = env(home.path(), true, FakeBus::new());

    Gtk::new().apply(&env, Variant::Dark, &cfg("Mac-Light", "Mac-Dark")).unwrap();
    assert_eq!(
        fs::read_to_string(&conf).unwrap(),
        "Net/IconThemeName \"breeze\"\nNet/ThemeName \"Mac-Dark\"\nGdk/WindowScalingFactor 1\n"
    );
    assert_eq!(
        exec.calls(),
        [
            "killall -HUP xsettingsd",
            "gsettings set org.gnome.desktop.interface gtk-theme Mac-Dark",
            "gsettings set org.gnome.desktop.interface color-scheme prefer-dark",
        ]
    );

    let (env, exec, _) = self::env(home.path(), true, FakeBus::new());
    Gtk::new().apply(&env, Variant::Light, &cfg("Mac-Light", "Mac-Dark")).unwrap();
    assert!(fs::read_to_string(&conf).unwrap().contains("Net/ThemeName \"Mac-Light\""));
    assert_eq!(exec.calls()[2], "gsettings set org.gnome.desktop.interface color-scheme prefer-light");
}

#[test]
fn gtk_fallback_works_without_a_session_bus() {
    let home = TempDir::new().unwrap();
    let conf = home.path().join(".config/xsettingsd/xsettingsd.conf");
    write(&conf, "Net/ThemeName \"Old\"\n");
    let bus = FakeBus::new();
    bus.fail_all("no bus");
    let (env, exec, _) = env(home.path(), true, bus);
    Gtk::new().apply(&env, Variant::Dark, &cfg("l", "d")).unwrap();
    assert_eq!(fs::read_to_string(&conf).unwrap(), "Net/ThemeName \"d\"\n");
    assert_eq!(exec.calls().len(), 3);
}

#[test]
fn gtk_without_kde_gtk_config_or_xsettingsd_does_nothing() {
    let home = TempDir::new().unwrap();
    let (env, exec, _) = env(home.path(), true, FakeBus::new());
    Gtk::new().apply(&env, Variant::Dark, &cfg("l", "d")).unwrap();
    assert!(exec.calls().is_empty());
    assert!(!home.path().join(".config/xsettingsd").exists());
}

#[test]
fn gtk_fallback_survives_missing_helper_programs() {
    let home = TempDir::new().unwrap();
    write(&home.path().join(".config/xsettingsd/xsettingsd.conf"), "Net/ThemeName \"Old\"\n");
    let (env, exec, _) = env(home.path(), true, FakeBus::new());
    exec.missing("killall");
    exec.missing("gsettings");
    assert!(Gtk::new().apply(&env, Variant::Dark, &cfg("l", "d")).is_ok());
}

#[test]
fn gtk_rejects_an_unset_theme() {
    let home = TempDir::new().unwrap();
    let (env, _, bus) = env(home.path(), true, FakeBus::new().with_services(&[GTK_CONFIG]));
    let unset = PluginConfig { enabled: true, light_theme: None, dark_theme: None };
    assert!(Gtk::new().apply(&env, Variant::Dark, &unset).is_err());
    assert!(bus.calls().is_empty());
}

#[test]
fn gtk_lists_themes_that_have_a_gtk3_variant() {
    let home = TempDir::new().unwrap();
    let system_dir = TempDir::new().unwrap();
    fs::create_dir_all(system_dir.path().join("Breeze/gtk-3.0")).unwrap();
    fs::create_dir_all(system_dir.path().join("OnlyGtk4/gtk-4.0")).unwrap();
    fs::create_dir_all(home.path().join(".themes/Mine/gtk-3.0")).unwrap();
    fs::create_dir_all(home.path().join(".local/share/themes/Breeze/gtk-3.0")).unwrap();
    fs::create_dir_all(home.path().join(".local/share/themes/Local/gtk-3.0")).unwrap();

    let (env, _, _) = env(home.path(), true, FakeBus::new());
    let ids: Vec<_> = Gtk::with_system_dirs(vec![system_dir.path().into()])
        .themes(&env)
        .into_iter()
        .map(|t| (t.id, t.label))
        .collect();
    let pair = |s: &str| (s.to_string(), s.to_string());
    assert_eq!(ids, [pair("Breeze"), pair("Local"), pair("Mine")]);
}

#[test]
fn gtk_is_available_on_kde_only() {
    let home = TempDir::new().unwrap();
    assert!(Gtk::new().available(&env(home.path(), true, FakeBus::new()).0));
    assert!(!Gtk::new().available(&env(home.path(), false, FakeBus::new()).0));
}

// ---- wallpaper ----------------------------------------------------------------------------

fn evaluate_script_call(bus: &FakeBus) -> String {
    let calls = bus.calls();
    assert_eq!(calls.len(), 1, "{calls:?}");
    let prefix = "call_str org.kde.plasmashell /PlasmaShell org.kde.PlasmaShell evaluateScript ";
    calls[0].strip_prefix(prefix).unwrap_or_else(|| panic!("unexpected call {}", calls[0])).to_string()
}

#[test]
fn wallpaper_script_percent_encodes_the_path() {
    let home = TempDir::new().unwrap();
    let images = TempDir::new().unwrap();
    let image = images.path().join("#a b.png");
    fs::write(&image, "x").unwrap();
    let (env, _, bus) = env(home.path(), true, FakeBus::new());

    Wallpaper.apply(&env, Variant::Dark, &cfg("unused", image.to_str().unwrap())).unwrap();

    let script = evaluate_script_call(&bus);
    let url = format!("file://{}/%23a%20b.png", images.path().display());
    assert!(script.contains(&format!("d.writeConfig(\"Image\", \"{url}\");")), "{script}");
    assert!(script.contains("d.wallpaperPlugin = \"org.kde.image\";"), "{script}");
    assert!(script.contains("desktops()"), "{script}");
}

#[test]
fn wallpaper_path_cannot_break_out_of_the_script() {
    let home = TempDir::new().unwrap();
    let images = TempDir::new().unwrap();
    let image = images.path().join("a\"b\\\u{e9}.png");
    fs::write(&image, "x").unwrap();
    let (env, _, bus) = env(home.path(), true, FakeBus::new());

    Wallpaper.apply(&env, Variant::Dark, &cfg("unused", image.to_str().unwrap())).unwrap();

    let script = evaluate_script_call(&bus);
    assert!(script.contains("/a%22b%5C%C3%A9.png\""), "{script}");
    assert_eq!(script.matches('"').count(), 12, "unexpected quotes in {script}");
}

#[test]
fn wallpaper_rejects_missing_relative_and_unset_paths() {
    let home = TempDir::new().unwrap();
    let (env, _, bus) = env(home.path(), true, FakeBus::new());
    for bad in ["/nonexistent/wall.png", "relative.png", ""] {
        assert!(Wallpaper.apply(&env, Variant::Dark, &cfg("l", bad)).is_err(), "{bad:?}");
    }
    let unset = PluginConfig { enabled: true, light_theme: None, dark_theme: None };
    assert!(Wallpaper.apply(&env, Variant::Light, &unset).is_err());
    assert!(bus.calls().is_empty());
}

#[test]
fn wallpaper_reports_bus_errors() {
    let home = TempDir::new().unwrap();
    let image = home.path().join("w.png");
    fs::write(&image, "x").unwrap();
    let bus = FakeBus::new();
    bus.fail_all("plasmashell is not running");
    let (env, _, _) = env(home.path(), true, bus);
    let err = Wallpaper.apply(&env, Variant::Dark, &cfg("l", image.to_str().unwrap())).unwrap_err();
    assert!(err.to_string().contains("plasmashell"), "{err}");
}

#[test]
fn wallpaper_takes_a_file_and_is_available_on_kde_only() {
    let home = TempDir::new().unwrap();
    assert_eq!(Wallpaper.kind(), InputKind::File);
    assert!(Wallpaper.available(&env(home.path(), true, FakeBus::new()).0));
    assert!(!Wallpaper.available(&env(home.path(), false, FakeBus::new()).0));
    assert!(Wallpaper.themes(&env(home.path(), true, FakeBus::new()).0).is_empty());
}

// ---- notification -------------------------------------------------------------------------

#[test]
fn notification_announces_the_new_theme() {
    let home = TempDir::new().unwrap();
    let (env, _, bus) = env(home.path(), true, FakeBus::new());
    Notification.apply(&env, Variant::Dark, &cfg("Day", "Night")).unwrap();
    Notification.apply(&env, Variant::Light, &cfg("Day", "Night")).unwrap();
    assert_eq!(bus.calls(), ["notify Theme changed | Set the theme to Night", "notify Theme changed | Set the theme to Day"]);
}

#[test]
fn notification_reports_bus_errors_and_is_marked_as_notification() {
    let home = TempDir::new().unwrap();
    let bus = FakeBus::new();
    bus.fail_all("no daemon");
    let (env, _, _) = env(home.path(), true, bus);
    assert!(Notification.apply(&env, Variant::Dark, &cfg("Day", "Night")).is_err());
    assert!(Notification.is_notification());
    assert_eq!(Notification.kind(), InputKind::Text);
}

// ---- registry -----------------------------------------------------------------------------

#[test]
fn registry_places_the_dbus_plugins_in_gui_order() {
    let keys: Vec<_> = registry().iter().map(|p| p.key()).collect();
    let at = |key: &str| keys.iter().position(|k| *k == key).unwrap_or_else(|| panic!("{key} missing in {keys:?}"));
    assert!(at("colors") < at("gtk") && at("gtk") < at("icons"), "{keys:?}");
    assert!(at("kvantum") < at("wallpaper") && at("wallpaper") < at("notification"), "{keys:?}");
}

#[test]
fn defaults_match_the_original_program() {
    assert_eq!(Gtk::new().defaults(), (Some("Breeze"), Some("Breeze")));
    assert_eq!(Wallpaper.defaults(), (None, None));
    assert_eq!(Notification.defaults(), (Some("Day"), Some("Night")));
}
