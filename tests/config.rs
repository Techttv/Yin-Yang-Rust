use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

use chrono::NaiveTime;
use tempfile::TempDir;
use yinyang::config::{Config, Mode, PluginConfig};
use yinyang::paths::Paths;

const FIXTURE: &str = include_str!("fixtures/yin_yang_dev.json");

fn t(h: u32, m: u32) -> NaiveTime {
    NaiveTime::from_hms_opt(h, m, 0).unwrap()
}

fn write(home: &Path, rel: &str, content: &str) {
    let path = home.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn set_mtime(path: &Path, secs_after_epoch: u64) {
    let file = fs::OpenOptions::new().write(true).open(path).unwrap();
    file.set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(secs_after_epoch))
        .unwrap();
}

fn with_legacy_fixture() -> (TempDir, Paths) {
    let home = TempDir::new().unwrap();
    write(home.path(), ".config/yin_yang_dev.json", FIXTURE);
    let paths = Paths::under(home.path());
    (home, paths)
}

#[test]
fn loads_real_config_fixture() {
    let (_home, paths) = with_legacy_fixture();
    let c = Config::load(&paths).unwrap();
    assert_eq!(c.mode, Mode::FollowSun);
    assert!(c.update_location);
    assert_eq!(
        c.plugins["vs code"].dark_theme.as_deref(),
        Some("Solarized Dark")
    );
    assert_eq!(c.plugins["wallpaper"].light_theme, None);
}

#[test]
fn legacy_file_is_never_modified() {
    let (home, paths) = with_legacy_fixture();
    let legacy = home.path().join(".config/yin_yang_dev.json");
    let before = fs::read(&legacy).unwrap();
    let c = Config::load(&paths).unwrap();
    c.save(&paths).unwrap();
    assert_eq!(fs::read(&legacy).unwrap(), before);
    assert!(paths.config_file.is_file());
}

#[test]
fn missing_everything_gives_defaults() {
    let home = TempDir::new().unwrap();
    let c = Config::load(&Paths::under(home.path())).unwrap();
    assert_eq!(c.mode, Mode::Manual);
    assert!(!c.dark_mode);
    assert!(c.plugins.is_empty());
}

#[test]
fn newest_legacy_file_wins() {
    let home = TempDir::new().unwrap();
    let doc = |mode: &str| format!(r#"{{"version": 4.0, "mode": "{mode}"}}"#);
    write(home.path(), ".config/yin_yang_dev.json", &doc("manual"));
    write(home.path(), ".config/yin_yang.json", &doc("manual time"));
    set_mtime(&home.path().join(".config/yin_yang_dev.json"), 1_000);
    set_mtime(&home.path().join(".config/yin_yang.json"), 2_000);
    let c = Config::load(&Paths::under(home.path())).unwrap();
    assert_eq!(c.mode, Mode::Scheduled);
}

#[test]
fn old_version_gives_defaults() {
    let home = TempDir::new().unwrap();
    write(
        home.path(),
        ".config/yin_yang_dev.json",
        r#"{"version": 2.2, "followSun": true, "theme": "dark"}"#,
    );
    let c = Config::load(&Paths::under(home.path())).unwrap();
    assert_eq!(c, Config::default());
}

#[test]
fn unknown_plugin_entries_survive_roundtrip() {
    let (_home, paths) = with_legacy_fixture();
    let c = Config::load(&paths).unwrap();
    assert_eq!(
        c.plugins["firefox"].dark_theme.as_deref(),
        Some("firefox-compact-dark@mozilla.org")
    );
    c.save(&paths).unwrap();
    let again = Config::load(&paths).unwrap();
    assert_eq!(again.plugins["firefox"], c.plugins["firefox"]);
}

#[test]
fn save_is_atomic_and_roundtrips() {
    let (_home, paths) = with_legacy_fixture();
    let c = Config::load(&paths).unwrap();
    c.save(&paths).unwrap();
    let leftovers: Vec<_> = fs::read_dir(paths.config_file.parent().unwrap())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.contains("tmp"))
        .collect();
    assert!(leftovers.is_empty(), "leftover temp files: {leftovers:?}");
    assert_eq!(Config::load(&paths).unwrap(), c);
}

#[test]
fn accepts_both_time_formats() {
    let home = TempDir::new().unwrap();
    let paths = Paths::under(home.path());
    write(
        home.path(),
        ".config/yinyang.json",
        r#"{"version": 4.0, "times": ["07:00:00", "20:00"]}"#,
    );
    let c = Config::load(&paths).unwrap();
    assert_eq!(c.times, (t(7, 0), t(20, 0)));
    c.save(&paths).unwrap();
    let saved = fs::read_to_string(&paths.config_file).unwrap();
    assert!(saved.contains("\"07:00\"") && saved.contains("\"20:00\""), "{saved}");
}

#[test]
fn rejects_bad_time() {
    let home = TempDir::new().unwrap();
    write(
        home.path(),
        ".config/yinyang.json",
        r#"{"version": 4.0, "times": ["25:99", "20:00"]}"#,
    );
    assert!(Config::load(&Paths::under(home.path())).is_err());
}

#[test]
fn fill_defaults_adds_only_missing_disabled() {
    let (_home, paths) = with_legacy_fixture();
    let mut c = Config::load(&paths).unwrap();
    let existing = c.plugins["system"].clone();
    c.fill_defaults(&[
        ("system".into(), Some("x".into()), Some("y".into())),
        ("brandnew".into(), Some("l".into()), None),
    ]);
    assert_eq!(c.plugins["system"], existing);
    assert_eq!(
        c.plugins["brandnew"],
        PluginConfig {
            enabled: false,
            light_theme: Some("l".into()),
            dark_theme: None
        }
    );
}
