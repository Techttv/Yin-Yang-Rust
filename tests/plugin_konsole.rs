mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use support::{FakeBus, FakeExec};
use tempfile::TempDir;
use yinyang::config::PluginConfig;
use yinyang::plugin::{registry, Env, Plugin, Variant};
use yinyang::plugins::konsole::Konsole;

struct Setup {
    home: TempDir,
    global: TempDir,
}

impl Setup {
    fn new() -> Setup {
        Setup { home: TempDir::new().unwrap(), global: TempDir::new().unwrap() }
    }

    fn env(&self, bus: FakeBus) -> (Env, Arc<FakeBus>) {
        let bus = Arc::new(bus);
        let env = Env {
            home: self.home.path().to_path_buf(),
            kde: true,
            exec: Arc::new(FakeExec::new()),
            bus: bus.clone(),
        };
        (env, bus)
    }

    fn plugin(&self) -> Konsole {
        Konsole::with_global_dir(self.global.path().into())
    }

    fn user_dir(&self) -> PathBuf {
        self.home.path().join(".local/share/konsole")
    }

    fn konsolerc(&self) -> PathBuf {
        self.home.path().join(".config/konsolerc")
    }

    fn write(&self, path: &Path, content: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    fn read_profile(&self, name: &str) -> String {
        fs::read_to_string(self.user_dir().join(name)).unwrap()
    }
}

fn cfg(light: &str, dark: &str) -> PluginConfig {
    PluginConfig { enabled: true, light_theme: Some(light.into()), dark_theme: Some(dark.into()) }
}

const MIO_PROFILE: &str = "[Appearance]\nColorScheme=Old\nFont=Mono,10\n\n[General]\nName=Mio\nParent=FALLBACK/\n";
const KONSOLERC: &str = "[Desktop Entry]\nDefaultProfile=Mio.profile\n\n[General]\nConfigVersion=1\n";

fn with_default_profile(s: &Setup) {
    s.write(&s.konsolerc(), KONSOLERC);
    s.write(&s.user_dir().join("Mio.profile"), MIO_PROFILE);
}

#[test]
fn konsole_creates_light_and_dark_profiles_from_the_default_one() {
    let s = Setup::new();
    with_default_profile(&s);
    let (env, _) = s.env(FakeBus::new());

    s.plugin().apply(&env, Variant::Dark, &cfg("BlackOnWhite", "Breeze")).unwrap();

    assert_eq!(
        s.read_profile("Light.profile"),
        "[Appearance]\nColorScheme=BlackOnWhite\nFont=Mono,10\n\n[General]\nName=Light\nParent=FALLBACK/\n"
    );
    assert_eq!(
        s.read_profile("Dark.profile"),
        "[Appearance]\nColorScheme=Breeze\nFont=Mono,10\n\n[General]\nName=Dark\nParent=FALLBACK/\n"
    );
    assert_eq!(s.read_profile("Mio.profile"), MIO_PROFILE, "the user's own profile is never touched");
}

#[test]
fn konsole_makes_the_new_default_profile_follow_the_variant() {
    let s = Setup::new();
    with_default_profile(&s);
    let (env, _) = s.env(FakeBus::new());

    s.plugin().apply(&env, Variant::Dark, &cfg("BlackOnWhite", "Breeze")).unwrap();
    assert_eq!(fs::read_to_string(s.konsolerc()).unwrap(), KONSOLERC.replace("Mio.profile", "Dark.profile"));

    s.plugin().apply(&env, Variant::Light, &cfg("BlackOnWhite", "Breeze")).unwrap();
    assert_eq!(fs::read_to_string(s.konsolerc()).unwrap(), KONSOLERC.replace("Mio.profile", "Light.profile"));
}

#[test]
fn konsole_updates_existing_profiles_in_place() {
    let s = Setup::new();
    with_default_profile(&s);
    s.write(
        &s.user_dir().join("Light.profile"),
        "[Appearance]\nColorScheme = Old\n\n[General]\nName = Light\nTabBarPosition=Top\n",
    );
    let (env, _) = s.env(FakeBus::new());

    s.plugin().apply(&env, Variant::Light, &cfg("Solarized", "Breeze")).unwrap();

    assert_eq!(
        s.read_profile("Light.profile"),
        "[Appearance]\nColorScheme = Solarized\n\n[General]\nName = Light\nTabBarPosition=Top\n",
        "a customised profile keeps everything but the colour scheme"
    );
    assert!(s.read_profile("Dark.profile").contains("ColorScheme=Breeze"), "the missing one is created");
}

#[test]
fn konsole_falls_back_to_the_first_profile_when_the_default_is_missing() {
    let s = Setup::new();
    s.write(&s.konsolerc(), "[Desktop Entry]\nDefaultProfile=Ghost.profile\n");
    s.write(&s.user_dir().join("B.profile"), "[Appearance]\nColorScheme=B\n\n[General]\nName=B\n");
    s.write(&s.user_dir().join("A.profile"), "[Appearance]\nColorScheme=A\nExtra=1\n\n[General]\nName=A\n");
    let (env, _) = s.env(FakeBus::new());

    s.plugin().apply(&env, Variant::Dark, &cfg("L", "D")).unwrap();

    assert_eq!(s.read_profile("Dark.profile"), "[Appearance]\nColorScheme=D\nExtra=1\n\n[General]\nName=Dark\n");
}

#[test]
fn konsole_creates_a_default_profile_when_there_is_none() {
    let s = Setup::new();
    s.write(&s.konsolerc(), "[Desktop Entry]\nDefaultProfile=Ghost.profile\n");
    fs::create_dir_all(s.user_dir()).unwrap();
    let (env, _) = s.env(FakeBus::new());

    s.plugin().apply(&env, Variant::Light, &cfg("BlackOnWhite", "Breeze")).unwrap();

    assert_eq!(
        s.read_profile("Default.profile"),
        "[Appearance]\nColorScheme=Breeze\n\n[General]\nCommand=/bin/bash\nName=YinYang\nParent=FALLBACK/\n"
    );
    assert_eq!(
        s.read_profile("Light.profile"),
        "[Appearance]\nColorScheme=BlackOnWhite\n\n[General]\nCommand=/bin/bash\nName=Light\nParent=FALLBACK/\n"
    );
    assert!(fs::read_to_string(s.konsolerc()).unwrap().contains("DefaultProfile=Light.profile"));
}

#[test]
fn konsole_does_not_invent_a_missing_konsolerc() {
    let s = Setup::new();
    s.write(&s.user_dir().join("Mio.profile"), MIO_PROFILE);
    let (env, _) = s.env(FakeBus::new());

    s.plugin().apply(&env, Variant::Dark, &cfg("L", "D")).unwrap();

    assert!(!s.konsolerc().exists());
    assert!(s.user_dir().join("Dark.profile").is_file());
}

#[test]
fn konsole_is_idempotent() {
    let s = Setup::new();
    with_default_profile(&s);
    let (env, _) = s.env(FakeBus::new());
    s.plugin().apply(&env, Variant::Dark, &cfg("L", "D")).unwrap();
    let snapshot = |s: &Setup| {
        ["Light.profile", "Dark.profile", "Mio.profile"].map(|n| s.read_profile(n)).join("|") + &fs::read_to_string(s.konsolerc()).unwrap()
    };
    let first = snapshot(&s);
    s.plugin().apply(&env, Variant::Dark, &cfg("L", "D")).unwrap();
    assert_eq!(snapshot(&s), first);
}

#[test]
fn konsole_rejects_an_unset_theme_for_the_applied_variant() {
    let s = Setup::new();
    with_default_profile(&s);
    let (env, _) = s.env(FakeBus::new());
    let unset = PluginConfig { enabled: true, light_theme: Some("L".into()), dark_theme: None };
    assert!(s.plugin().apply(&env, Variant::Dark, &unset).is_err());
    assert!(!s.user_dir().join("Dark.profile").exists(), "nothing is created on error");
    assert_eq!(fs::read_to_string(s.konsolerc()).unwrap(), KONSOLERC);
}

const NODES: &str = "<node>\n  <interface name=\"org.kde.konsole.Window\"/>\n  <node name=\"1\"/>\n  <node name=\"2\"/>\n</node>\n";

#[test]
fn konsole_switches_windows_and_sessions_of_every_konsole_app() {
    let s = Setup::new();
    with_default_profile(&s);
    let bus = FakeBus::new().with_services(&[
        "org.kde.konsole-1234",
        "org.kde.dolphin-55",
        "org.kde.yakuake",
        "org.kde.other",
        "org.kde.konsole2",
    ]);
    bus.set_introspection("org.kde.konsole-1234", "/Windows", NODES);
    bus.set_introspection("org.kde.konsole-1234", "/Sessions", "<node>\n  <node name=\"7\"/>\n</node>\n");
    bus.set_introspection("org.kde.yakuake", "/Sessions", "<node>\n  <node name=\"3\"/>\n</node>\n");
    let (env, bus) = s.env(bus);

    s.plugin().apply(&env, Variant::Dark, &cfg("L", "D")).unwrap();

    let window = |svc: &str, n: &str| format!("call_str {svc} /Windows/{n} org.kde.konsole.Window setDefaultProfile Dark");
    let session = |svc: &str, n: &str| format!("call_str {svc} /Sessions/{n} org.kde.konsole.Session setProfile Dark");
    assert_eq!(
        bus.calls(),
        [
            "service_names".to_string(),
            "introspect org.kde.konsole-1234 /Windows".into(),
            window("org.kde.konsole-1234", "1"),
            window("org.kde.konsole-1234", "2"),
            "introspect org.kde.konsole-1234 /Sessions".into(),
            session("org.kde.konsole-1234", "7"),
            "introspect org.kde.dolphin-55 /Windows".into(),
            "introspect org.kde.dolphin-55 /Sessions".into(),
            "introspect org.kde.yakuake /Windows".into(),
            "introspect org.kde.yakuake /Sessions".into(),
            session("org.kde.yakuake", "3"),
        ]
    );
}

#[test]
fn konsole_without_running_instances_or_a_bus_still_updates_the_files() {
    let s = Setup::new();
    with_default_profile(&s);
    let (env, bus) = s.env(FakeBus::new());
    s.plugin().apply(&env, Variant::Dark, &cfg("L", "D")).unwrap();
    assert_eq!(bus.calls(), ["service_names"]);

    let down = FakeBus::new();
    down.fail_all("no session bus");
    let (env, _) = s.env(down);
    s.plugin().apply(&env, Variant::Light, &cfg("L", "D")).unwrap();
    assert!(fs::read_to_string(s.konsolerc()).unwrap().contains("Light.profile"));

    let flaky = FakeBus::new().with_services(&["org.kde.konsole-1"]);
    flaky.set_introspection("org.kde.konsole-1", "/Windows", NODES);
    flaky.fail_matching("call_str", "konsole went away");
    let (env, _) = s.env(flaky);
    s.plugin().apply(&env, Variant::Dark, &cfg("L", "D")).unwrap();
}

#[test]
fn konsole_is_available_when_either_data_directory_exists() {
    let s = Setup::new();
    let (env, _) = s.env(FakeBus::new());
    let none = Konsole::with_global_dir("/nonexistent/konsole".into());
    assert!(!none.available(&env));
    assert!(none.themes(&env).is_empty());

    assert!(s.plugin().available(&env), "the global directory exists");
    fs::create_dir_all(s.user_dir()).unwrap();
    assert!(none.available(&env), "the user directory exists");
}

#[test]
fn konsole_lists_built_in_and_installed_color_schemes() {
    let s = Setup::new();
    s.write(&s.global.path().join("Breeze.colorscheme"), "[General]\nDescription=Breeze\n");
    s.write(&s.global.path().join("SolarizedLight.colorscheme"), "[General]\nDescription=Solarized Light (system)\n");
    s.write(&s.global.path().join("Nodesc.colorscheme"), "[General]\nColor=1\n");
    s.write(&s.global.path().join("readme.txt"), "x");
    s.write(&s.user_dir().join("Mine.colorscheme"), "[General]\nDescription=My scheme\n");
    let (env, _) = s.env(FakeBus::new());

    let themes = s.plugin().themes(&env);
    let ids: Vec<_> = themes.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "BlackOnLightYellow", "BlackOnRandomLight", "BlackOnWhite", "BlueOnBlack", "Breeze", "Campbell",
            "DarkPastels", "GreenOnBlack", "Linux", "Mine", "Nodesc", "RedOnBlack", "Solarized",
            "SolarizedLight", "WhiteOnBlack"
        ]
    );
    let label = |id: &str| themes.iter().find(|t| t.id == id).unwrap().label.clone();
    assert_eq!(label("Mine"), "My scheme");
    assert_eq!(label("Nodesc"), "Nodesc");
    assert_eq!(label("SolarizedLight"), "Solarized Light (system)", "an installed file wins over the built-in");
    assert_eq!(label("Linux"), "Linux Colors");
}

#[test]
fn konsole_identity_matches_the_original_program() {
    let s = Setup::new();
    assert_eq!(s.plugin().key(), "konsole");
    assert_eq!(s.plugin().name(), "Konsole");
    assert_eq!(s.plugin().defaults(), (Some("BlackOnWhite"), Some("Breeze")));
    let keys: Vec<_> = registry().iter().map(|p| p.key()).collect();
    let at = |key: &str| keys.iter().position(|k| *k == key).unwrap();
    assert!(at("okular") < at("konsole") && at("konsole") < at("notification"), "{keys:?}");
}
