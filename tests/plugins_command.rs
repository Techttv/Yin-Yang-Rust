mod support;

use std::fs;
use std::path::Path;
use std::sync::Arc;

use support::{FakeBus, FakeExec};
use tempfile::TempDir;
use yinyang::config::PluginConfig;
use yinyang::plugin::{registry, Env, Plugin, ThemeChoice, Variant};
use yinyang::plugins::colors::Colors;
use yinyang::plugins::icons::Icons;
use yinyang::plugins::kvantum::Kvantum;
use yinyang::plugins::system::System;

fn env(home: &Path, kde: bool) -> (Env, Arc<FakeExec>) {
    let exec = Arc::new(FakeExec::new());
    let env = Env {
        home: home.to_path_buf(),
        kde,
        exec: exec.clone(),
        bus: Arc::new(FakeBus::new()),
    };
    (env, exec)
}

fn cfg(light: &str, dark: &str) -> PluginConfig {
    PluginConfig {
        enabled: true,
        light_theme: Some(light.into()),
        dark_theme: Some(dark.into()),
    }
}

fn write(path: &Path, content: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn choices(themes: Vec<ThemeChoice>) -> Vec<(String, String)> {
    themes.into_iter().map(|t| (t.id, t.label)).collect()
}

fn pair(id: &str, label: &str) -> (String, String) {
    (id.to_string(), label.to_string())
}

// ---- system -------------------------------------------------------------------------------

#[test]
fn system_applies_the_configured_theme() {
    let home = TempDir::new().unwrap();
    let (env, exec) = env(home.path(), true);
    let c = cfg("org.kde.breeze.desktop", "org.kde.breezedark.desktop");
    System::new().apply(&env, Variant::Dark, &c).unwrap();
    assert_eq!(exec.calls(), ["lookandfeeltool -a org.kde.breezedark.desktop"]);
}

#[test]
fn system_lists_themes_with_localized_labels() {
    let home = TempDir::new().unwrap();
    let system_dir = TempDir::new().unwrap();
    write(
        &system_dir.path().join("org.kde.breeze.desktop/metadata.json"),
        r#"{"KPlugin": {"Name": "Breeze", "Name[it]": "Brezza"}}"#,
    );
    write(
        &home.path().join(".local/share/plasma/look-and-feel/com.x.Mac-Dark/metadata.desktop"),
        "[Desktop Entry]\nName=Mac Dark\nName[it]=Mac Scuro\nComment=x\n",
    );
    let (env, exec) = env(home.path(), true);
    exec.respond("lookandfeeltool", 0, "org.kde.breeze.desktop\ncom.x.Mac-Dark\nnometa\n\n");

    let italian = System::with_dirs(system_dir.path().into(), vec!["it_IT".into(), "it".into()]);
    assert_eq!(
        choices(italian.themes(&env)),
        [
            pair("com.x.Mac-Dark", "Mac Scuro"),
            pair("nometa", "nometa"),
            pair("org.kde.breeze.desktop", "Brezza"),
        ]
    );

    let neutral = System::with_dirs(system_dir.path().into(), vec![]);
    assert_eq!(
        choices(neutral.themes(&env)),
        [
            pair("com.x.Mac-Dark", "Mac Dark"),
            pair("nometa", "nometa"),
            pair("org.kde.breeze.desktop", "Breeze"),
        ]
    );
}

#[test]
fn system_is_unavailable_off_kde_or_without_the_tool() {
    let home = TempDir::new().unwrap();
    let (env_off, _) = env(home.path(), false);
    assert!(!System::new().available(&env_off));

    let (env_on, exec) = env(home.path(), true);
    assert!(System::new().available(&env_on));
    exec.missing("lookandfeeltool");
    assert!(!System::new().available(&env_on));
}

#[test]
fn system_has_no_themes_when_the_tool_fails() {
    let home = TempDir::new().unwrap();
    let (env, exec) = env(home.path(), true);
    exec.fail("lookandfeeltool", "boom");
    assert!(System::new().themes(&env).is_empty());
    let (env, exec) = self::env(home.path(), true);
    exec.respond("lookandfeeltool", 1, "garbage");
    assert!(System::new().themes(&env).is_empty());
}

#[test]
fn system_rejects_unset_theme_and_failing_tool() {
    let home = TempDir::new().unwrap();
    let (env, exec) = env(home.path(), true);
    let unset = PluginConfig { enabled: true, light_theme: None, dark_theme: Some(String::new()) };
    assert!(System::new().apply(&env, Variant::Light, &unset).is_err());
    assert!(System::new().apply(&env, Variant::Dark, &unset).is_err());
    assert!(exec.calls().is_empty());

    exec.respond("lookandfeeltool", 2, "");
    let err = System::new().apply(&env, Variant::Dark, &cfg("a", "b")).unwrap_err();
    assert!(err.to_string().contains("status 2"), "{err}");
}

// ---- colors -------------------------------------------------------------------------------

#[test]
fn colors_applies_the_configured_scheme() {
    let home = TempDir::new().unwrap();
    let (env, exec) = env(home.path(), true);
    Colors.apply(&env, Variant::Dark, &cfg("MacTahoeLight", "MacTahoeDark")).unwrap();
    assert_eq!(exec.calls(), ["plasma-apply-colorscheme MacTahoeDark"]);
}

#[test]
fn colors_lists_schemes_from_a_translated_listing() {
    let home = TempDir::new().unwrap();
    let (env, exec) = env(home.path(), true);
    exec.respond(
        "plasma-apply-colorscheme",
        0,
        "Hai i seguenti schemi di colori sul tuo sistema:\n * BreezeLight\n * BreezeDark (schema di colori corrente)\n * Mac Tahoe\nnoise without a bullet\n",
    );
    assert_eq!(
        choices(Colors.themes(&env)),
        [pair("BreezeDark", "BreezeDark"), pair("BreezeLight", "BreezeLight"), pair("Mac Tahoe", "Mac Tahoe")]
    );
}

#[test]
fn colors_is_unavailable_off_kde_or_without_the_tool() {
    let home = TempDir::new().unwrap();
    let (env_off, _) = env(home.path(), false);
    assert!(!Colors.available(&env_off));
    let (env_on, exec) = env(home.path(), true);
    assert!(Colors.available(&env_on));
    exec.missing("plasma-apply-colorscheme");
    assert!(!Colors.available(&env_on));
}

// ---- icons --------------------------------------------------------------------------------

const BREEZE_INDEX: &str = "[Icon Theme]\nName=Breeze\nDirectories=16x16\n\n[16x16]\nSize=16\nContext=Actions\n";

fn icon_theme(dir: &Path, index: &str, cache: bool) {
    write(&dir.join("index.theme"), index);
    if cache {
        write(&dir.join("icon-theme.cache"), "x");
    }
}

#[test]
fn icons_applies_the_configured_theme() {
    let home = TempDir::new().unwrap();
    let (env, exec) = env(home.path(), true);
    Icons::new().apply(&env, Variant::Dark, &cfg("breeze", "breeze-dark")).unwrap();
    assert_eq!(exec.calls(), ["/usr/lib/plasma-changeicons breeze-dark"]);
}

#[test]
fn icons_lists_real_icon_themes_only() {
    let home = TempDir::new().unwrap();
    let system_dir = TempDir::new().unwrap();
    icon_theme(&system_dir.path().join("breeze"), BREEZE_INDEX, true);
    icon_theme(
        &system_dir.path().join("breeze_cursors"),
        "[Icon Theme]\nName=Breeze cursors\nDirectories=cursors\n\n[cursors]\nContext=Cursors\n",
        true,
    );
    icon_theme(&system_dir.path().join("nocache"), BREEZE_INDEX, false);
    icon_theme(&home.path().join(".icons/Mine"), &BREEZE_INDEX.replace("Name=Breeze", "Name=My Icons"), true);

    let (env, _) = env(home.path(), true);
    let icons = Icons::with_system_dir(system_dir.path().into());
    assert_eq!(choices(icons.themes(&env)), [pair("Mine", "My Icons"), pair("breeze", "Breeze")]);
}

#[test]
fn icons_is_unavailable_off_kde_or_without_the_binary() {
    let home = TempDir::new().unwrap();
    let (env_off, _) = env(home.path(), false);
    assert!(!Icons::new().available(&env_off));
    let (env_on, exec) = env(home.path(), true);
    assert!(Icons::new().available(&env_on));
    exec.missing("/usr/lib/plasma-changeicons");
    assert!(!Icons::new().available(&env_on));
}

// ---- kvantum ------------------------------------------------------------------------------

#[test]
fn kvantum_applies_the_configured_theme() {
    let home = TempDir::new().unwrap();
    let (env, exec) = env(home.path(), true);
    Kvantum::new().apply(&env, Variant::Dark, &cfg("MacTahoe", "MacTahoeDark")).unwrap();
    assert_eq!(exec.calls(), ["kvantummanager --set MacTahoeDark"]);
}

#[test]
fn kvantum_lists_themes_but_not_its_own_settings_file() {
    let home = TempDir::new().unwrap();
    let system_dir = TempDir::new().unwrap();
    write(&system_dir.path().join("KvAdapta/KvAdapta.kvconfig"), "");
    write(&system_dir.path().join("KvAdapta/KvAdapta.svg"), "");
    write(&system_dir.path().join("MacTahoe/MacTahoe.kvconfig"), "");
    write(&home.path().join(".config/Kvantum/MacTahoe/MacTahoe.kvconfig"), "");
    write(&home.path().join(".config/Kvantum/Deep/er/Nested.kvconfig"), "");
    write(&home.path().join(".config/Kvantum/kvantum.kvconfig"), "[General]\ntheme=MacTahoe\n");

    let (env, _) = env(home.path(), true);
    let kvantum = Kvantum::with_system_dir(system_dir.path().into());
    assert_eq!(
        choices(kvantum.themes(&env)),
        [pair("KvAdapta", "KvAdapta"), pair("MacTahoe", "MacTahoe"), pair("Nested", "Nested")]
    );
}

#[test]
fn kvantum_availability_depends_on_the_tool_only() {
    let home = TempDir::new().unwrap();
    let (env, exec) = env(home.path(), false);
    assert!(Kvantum::new().available(&env));
    exec.missing("kvantummanager");
    assert!(!Kvantum::new().available(&env));
}

// ---- registry -----------------------------------------------------------------------------

#[test]
fn registry_lists_command_plugins_in_gui_order() {
    let keys: Vec<_> = registry().iter().map(|p| p.key()).collect();
    let at = |key: &str| keys.iter().position(|k| *k == key).unwrap_or_else(|| panic!("{key} missing in {keys:?}"));
    assert!(at("system") < at("colors") && at("colors") < at("icons") && at("icons") < at("kvantum"), "{keys:?}");
}

#[test]
fn defaults_match_the_original_program() {
    assert_eq!(System::new().defaults(), (Some("org.kde.breeze.desktop"), Some("org.kde.breezedark.desktop")));
    assert_eq!(Icons::new().defaults(), (Some("breeze"), Some("breeze-dark")));
    assert_eq!(Kvantum::new().defaults(), (Some("KvFlatLight"), Some("KvFlat")));
    assert_eq!(Colors.defaults(), (None, None));
}
