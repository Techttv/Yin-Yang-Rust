mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use support::{FakeBus, FakeExec};
use tempfile::TempDir;
use yinyang::config::PluginConfig;
use yinyang::plugin::{registry, Env, Plugin, Variant};
use yinyang::plugins::okular::Okular;
use yinyang::plugins::onlyoffice::OnlyOffice;
use yinyang::plugins::vscode::Vscode;

fn env(home: &Path, bus: FakeBus) -> (Env, Arc<FakeBus>) {
    let bus = Arc::new(bus);
    let env = Env {
        home: home.to_path_buf(),
        kde: true,
        exec: Arc::new(FakeExec::new()),
        bus: bus.clone(),
    };
    (env, bus)
}

fn cfg(light: &str, dark: &str) -> PluginConfig {
    PluginConfig { enabled: true, light_theme: Some(light.into()), dark_theme: Some(dark.into()) }
}

fn write(path: &Path, content: &str) -> PathBuf {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
    path.to_path_buf()
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap()
}

fn ids(plugin: &dyn Plugin, env: &Env) -> Vec<String> {
    plugin.themes(env).into_iter().map(|t| t.id).collect()
}

// ---- vs code ------------------------------------------------------------------------------

/// A VS Code with no system-wide extension directories, so only the test's home counts.
fn vscode() -> Vscode {
    Vscode::with_system_extension_dirs(Vec::new())
}

const SETTINGS: &str = "// my settings\n{\n\t\"editor.fontSize\": 14,\n\t\"workbench.colorTheme\": \"Solarized Light\", // theme\n}\n";

#[test]
fn vscode_changes_only_the_theme_value() {
    let home = TempDir::new().unwrap();
    let file = write(&home.path().join(".config/Code/User/settings.json"), SETTINGS);
    let (env, _) = env(home.path(), FakeBus::new());

    vscode().apply(&env, Variant::Dark, &cfg("Solarized Light", "Solarized Dark")).unwrap();

    assert_eq!(read(&file), SETTINGS.replacen("Solarized Light", "Solarized Dark", 1));
}

#[test]
fn vscode_updates_every_installed_editor_including_flatpak() {
    let home = TempDir::new().unwrap();
    let files: Vec<PathBuf> = [
        ".config/Code/User/settings.json",
        ".config/VSCodium/User/settings.json",
        ".var/app/com.vscodium.codium/config/VSCodium/User/settings.json",
    ]
    .iter()
    .map(|rel| write(&home.path().join(rel), SETTINGS))
    .collect();
    let (env, _) = env(home.path(), FakeBus::new());

    vscode().apply(&env, Variant::Dark, &cfg("l", "Dark Theme")).unwrap();

    for file in files {
        assert!(read(&file).contains("\"workbench.colorTheme\": \"Dark Theme\""), "{}", file.display());
    }
}

#[test]
fn vscode_leaves_a_corrupt_file_alone_and_still_updates_the_others() {
    let home = TempDir::new().unwrap();
    let broken = write(&home.path().join(".config/Code/User/settings.json"), "this is not json at all");
    let fine = write(&home.path().join(".config/VSCodium/User/settings.json"), SETTINGS);
    let (env, _) = env(home.path(), FakeBus::new());

    let err = vscode().apply(&env, Variant::Dark, &cfg("l", "d")).unwrap_err();

    assert!(err.to_string().contains("Code/User/settings.json"), "{err}");
    assert_eq!(read(&broken), "this is not json at all");
    assert!(read(&fine).contains("\"workbench.colorTheme\": \"d\""));
}

#[test]
fn vscode_without_settings_files_succeeds_and_creates_nothing() {
    let home = TempDir::new().unwrap();
    let (env, _) = env(home.path(), FakeBus::new());
    vscode().apply(&env, Variant::Dark, &cfg("l", "d")).unwrap();
    assert_eq!(fs::read_dir(home.path()).unwrap().count(), 0);
}

#[test]
fn vscode_rejects_an_unset_theme_without_touching_files() {
    let home = TempDir::new().unwrap();
    let file = write(&home.path().join(".config/Code/User/settings.json"), SETTINGS);
    let (env, _) = env(home.path(), FakeBus::new());
    let unset = PluginConfig { enabled: true, light_theme: None, dark_theme: Some(String::new()) };
    assert!(vscode().apply(&env, Variant::Light, &unset).is_err());
    assert!(vscode().apply(&env, Variant::Dark, &unset).is_err());
    assert_eq!(read(&file), SETTINGS);
}

fn theme_package(label_or_id: &str, key: &str, with_id: bool) -> String {
    let theme = if with_id {
        format!(r#"{{"id": "{label_or_id}", "label": "ignored label", "path": "./t.json"}}"#)
    } else {
        format!(r#"{{"label": "{label_or_id}", "path": "./t.json"}}"#)
    };
    format!(r#"{{"name": "x", "contributes": {{"{key}": [{theme}]}}}}"#)
}

#[test]
fn vscode_lists_themes_from_every_extension_directory() {
    let home = TempDir::new().unwrap();
    let system = TempDir::new().unwrap();
    write(&system.path().join("theme-solarized-dark/package.json"), &theme_package("Solarized Dark", "themes", true));
    write(&system.path().join("theme-capital/package.json"), &theme_package("Capital Key", "Themes", false));
    write(&system.path().join("no-themes/package.json"), r#"{"contributes": {"commands": []}}"#);
    write(&system.path().join("broken/package.json"), "{ not json");
    write(&system.path().join("node_modules/package.json"), &theme_package("Hidden", "themes", true));
    write(&system.path().join("loose-file.txt"), "x");
    write(&home.path().join(".vscode/extensions/pub.ext-1.0/package.json"), &theme_package("User Theme", "themes", false));
    write(&home.path().join(".var/app/com.vscodium.codium/data/codium/extensions/p.e/package.json"), &theme_package("Flatpak Theme", "themes", true));

    let (env, _) = env(home.path(), FakeBus::new());
    let plugin = Vscode::with_system_extension_dirs(vec![system.path().into(), "/nonexistent/dir".into()]);

    assert_eq!(ids(&plugin, &env), ["Capital Key", "Flatpak Theme", "Solarized Dark", "User Theme"]);
    assert!(plugin.available(&env));
}

#[test]
fn vscode_is_unavailable_without_any_theme() {
    let home = TempDir::new().unwrap();
    let (env, _) = env(home.path(), FakeBus::new());
    assert!(ids(&vscode(), &env).is_empty());
    assert!(!vscode().available(&env));
}

#[test]
fn vscode_identity_matches_the_original_program() {
    assert_eq!(vscode().key(), "vs code");
    assert_eq!(vscode().name(), "VS Code");
    assert_eq!(vscode().defaults(), (Some("Default Light Modern"), Some("Default Dark Modern")));
}

// ---- onlyoffice ---------------------------------------------------------------------------

const OO_CONF: &str = "[General]\nUITheme=theme-light\nlang=it\n\n[Other]\nUITheme=keep\n";

#[test]
fn onlyoffice_sets_the_ui_theme_and_keeps_the_rest() {
    let home = TempDir::new().unwrap();
    let file = write(&home.path().join(".config/onlyoffice/DesktopEditors.conf"), OO_CONF);
    let (env, _) = env(home.path(), FakeBus::new());

    OnlyOffice.apply(&env, Variant::Dark, &cfg("theme-light", "theme-dark")).unwrap();

    assert_eq!(read(&file), OO_CONF.replacen("UITheme=theme-light", "UITheme=theme-dark", 1));
}

#[test]
fn onlyoffice_updates_the_flatpak_copy_and_adds_a_missing_section() {
    let home = TempDir::new().unwrap();
    let file = write(&home.path().join(".var/app/org.onlyoffice.desktopeditors/config/onlyoffice/DesktopEditors.conf"), "[Other]\nx=1\n");
    let (env, _) = env(home.path(), FakeBus::new());

    OnlyOffice.apply(&env, Variant::Light, &cfg("theme-classic-light", "d")).unwrap();

    assert_eq!(read(&file), "[Other]\nx=1\n\n[General]\nUITheme=theme-classic-light\n");
}

#[test]
fn onlyoffice_needs_a_config_file_and_creates_none() {
    let home = TempDir::new().unwrap();
    let (env, _) = env(home.path(), FakeBus::new());
    assert!(!OnlyOffice.available(&env));
    OnlyOffice.apply(&env, Variant::Dark, &cfg("l", "theme-dark")).unwrap();
    assert_eq!(fs::read_dir(home.path()).unwrap().count(), 0);

    write(&home.path().join(".config/onlyoffice/DesktopEditors.conf"), OO_CONF);
    assert!(OnlyOffice.available(&env));
}

#[test]
fn onlyoffice_offers_its_five_themes_and_rejects_an_empty_one() {
    let home = TempDir::new().unwrap();
    let (env, _) = env(home.path(), FakeBus::new());
    assert_eq!(
        ids(&OnlyOffice, &env),
        ["theme-system", "theme-light", "theme-classic-light", "theme-dark", "theme-contrast-dark"]
    );
    assert_eq!(OnlyOffice.defaults(), (Some("theme-light"), Some("theme-dark")));
    write(&home.path().join(".config/onlyoffice/DesktopEditors.conf"), OO_CONF);
    assert!(OnlyOffice.apply(&env, Variant::Dark, &cfg("l", "")).is_err());
}

// ---- okular -------------------------------------------------------------------------------

const CHANGE_COLORS: &str = "slotSetChangeColors";

fn okular_call(service: &str, dark: bool) -> String {
    format!("call_bool {service} /okular org.kde.okular {CHANGE_COLORS} {dark}")
}

#[test]
fn okular_dark_switches_running_instances_and_the_config() {
    let home = TempDir::new().unwrap();
    let file = write(&home.path().join(".config/okularpartrc"), "[General]\nx=1\n");
    let bus = FakeBus::new().with_services(&["org.kde.okular-4242", "org.kde.okular-77", "org.kde.other", "org.kde.okular"]);
    let (env, bus) = env(home.path(), bus);

    Okular.apply(&env, Variant::Dark, &cfg("", "InvertLightness")).unwrap();

    assert_eq!(
        bus.calls(),
        ["service_names".to_string(), okular_call("org.kde.okular-4242", true), okular_call("org.kde.okular-77", true)]
    );
    assert_eq!(read(&file), "[General]\nx=1\n\n[Document]\nRenderMode=InvertLightness\nChangeColors=true\n");
}

#[test]
fn okular_light_stops_changing_colors_but_remembers_the_render_mode() {
    let home = TempDir::new().unwrap();
    let file = write(
        &home.path().join(".config/okularpartrc"),
        "[General]\nx=1\n\n[Document]\nRenderMode=InvertLightness\nChangeColors=true\n",
    );
    let (env, bus) = env(home.path(), FakeBus::new().with_services(&["org.kde.okular-9"]));

    Okular.apply(&env, Variant::Light, &cfg("", "InvertLightness")).unwrap();

    assert_eq!(bus.calls()[1], okular_call("org.kde.okular-9", false));
    assert_eq!(read(&file), "[General]\nx=1\n\n[Document]\nRenderMode=InvertLightness\n");
}

#[test]
fn okular_plain_invert_leaves_no_trace_once_light_again() {
    let home = TempDir::new().unwrap();
    let original = "[General]\nx=1\n";
    let file = write(&home.path().join(".config/okularpartrc"), original);
    let (env, _) = env(home.path(), FakeBus::new());

    Okular.apply(&env, Variant::Dark, &cfg("", "")).unwrap();
    assert_eq!(read(&file), "[General]\nx=1\n\n[Document]\nChangeColors=true\n");

    Okular.apply(&env, Variant::Light, &cfg("", "")).unwrap();
    assert_eq!(read(&file), original);
}

#[test]
fn okular_unset_themes_mean_plain_invert() {
    let home = TempDir::new().unwrap();
    let file = write(&home.path().join(".config/okularpartrc"), "[General]\nx=1\n");
    let (env, _) = env(home.path(), FakeBus::new());
    let unset = PluginConfig { enabled: true, light_theme: None, dark_theme: None };
    Okular.apply(&env, Variant::Dark, &unset).unwrap();
    assert!(read(&file).contains("ChangeColors=true"));
    assert!(!read(&file).contains("RenderMode"));
}

#[test]
fn okular_without_running_instances_still_updates_the_config() {
    let home = TempDir::new().unwrap();
    let file = write(&home.path().join(".config/okularpartrc"), "[General]\nx=1\n");
    let (env, bus) = env(home.path(), FakeBus::new());
    Okular.apply(&env, Variant::Dark, &cfg("", "InvertLuma")).unwrap();
    assert_eq!(bus.calls(), ["service_names"]);
    assert!(read(&file).contains("ChangeColors=true"));
}

#[test]
fn okular_live_switching_failures_do_not_fail_the_plugin() {
    let home = TempDir::new().unwrap();
    let file = write(&home.path().join(".config/okularpartrc"), "[General]\nx=1\n");

    let failing_call = FakeBus::new().with_services(&["org.kde.okular-1"]);
    failing_call.fail_matching("call_bool", "okular went away");
    let (env, _) = env(home.path(), failing_call);
    Okular.apply(&env, Variant::Dark, &cfg("", "InvertLuma")).unwrap();
    assert!(read(&file).contains("ChangeColors=true"));

    let no_bus = FakeBus::new();
    no_bus.fail_all("no session bus");
    let (env, _) = self::env(home.path(), no_bus);
    Okular.apply(&env, Variant::Light, &cfg("", "InvertLuma")).unwrap();
    assert!(!read(&file).contains("ChangeColors"));
}

#[test]
fn okular_updates_the_flatpak_config_and_is_available_only_with_a_file() {
    let home = TempDir::new().unwrap();
    let (env, _) = env(home.path(), FakeBus::new());
    assert!(!Okular.available(&env));
    Okular.apply(&env, Variant::Dark, &cfg("", "InvertLuma")).unwrap();
    assert_eq!(fs::read_dir(home.path()).unwrap().count(), 0, "must not create files");

    let file = write(&home.path().join(".var/app/org.kde.okular/config/okularpartrc"), "[General]\nx=1\n");
    assert!(Okular.available(&env));
    Okular.apply(&env, Variant::Dark, &cfg("", "InvertLuma")).unwrap();
    assert!(read(&file).contains("ChangeColors=true"));
}

#[test]
fn okular_offers_the_four_color_modes_and_describes_itself() {
    let home = TempDir::new().unwrap();
    let (env, _) = env(home.path(), FakeBus::new());
    assert_eq!(ids(&Okular, &env), ["", "InvertLightness", "InvertLuma", "InvertLumaSymmetric"]);
    assert_eq!(Okular.defaults(), (Some(""), Some("InvertLightness")));
    assert_eq!(Okular.describe(Variant::Light, &cfg("", "InvertLuma")), "okular: colors unchanged");
    assert_eq!(Okular.describe(Variant::Dark, &cfg("", "InvertLuma")), "okular: render mode InvertLuma");
    assert_eq!(Okular.describe(Variant::Dark, &cfg("", "")), "okular: render mode Invert colors");
}

// ---- registry -----------------------------------------------------------------------------

#[test]
fn registry_places_the_file_plugins_in_gui_order() {
    let keys: Vec<_> = registry().iter().map(|p| p.key()).collect();
    let at = |key: &str| keys.iter().position(|k| *k == key).unwrap_or_else(|| panic!("{key} missing in {keys:?}"));
    assert!(at("wallpaper") < at("vs code") && at("vs code") < at("onlyoffice"), "{keys:?}");
    assert!(at("onlyoffice") < at("okular") && at("okular") < at("notification"), "{keys:?}");
}
