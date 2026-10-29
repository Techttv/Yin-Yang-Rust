mod support;

use std::fs;
use std::sync::Arc;
use std::time::{Duration, Instant};

use support::{FakeBus, FakeExec};
use tempfile::TempDir;
use yinyang::config::PluginConfig;
use yinyang::exec::RealExec;
use yinyang::plugin::{registry, Env, InputKind, Plugin, Variant};
use yinyang::plugins::custom::Custom;

fn fake_env() -> (Env, Arc<FakeExec>) {
    let exec = Arc::new(FakeExec::new());
    let env = Env {
        home: "/nonexistent".into(),
        kde: true,
        exec: exec.clone(),
        bus: Arc::new(FakeBus::new()),
    };
    (env, exec)
}

fn real_env() -> Env {
    Env { home: "/nonexistent".into(), kde: true, exec: Arc::new(RealExec), bus: Arc::new(FakeBus::new()) }
}

fn cfg(light: &str, dark: &str) -> PluginConfig {
    PluginConfig { enabled: true, light_theme: Some(light.into()), dark_theme: Some(dark.into()) }
}

#[test]
fn custom_runs_the_command_of_the_variant_through_a_shell() {
    let (env, exec) = fake_env();
    let c = cfg("echo day", "notify-send ciao");
    Custom.apply(&env, Variant::Dark, &c).unwrap();
    Custom.apply(&env, Variant::Light, &c).unwrap();
    assert_eq!(exec.calls(), ["sh -c notify-send ciao", "sh -c echo day"]);
}

#[test]
fn custom_rejects_an_empty_or_unset_command_without_running_anything() {
    let (env, exec) = fake_env();
    let unset = PluginConfig { enabled: true, light_theme: None, dark_theme: Some(String::new()) };
    assert!(Custom.apply(&env, Variant::Light, &unset).is_err());
    assert!(Custom.apply(&env, Variant::Dark, &unset).is_err());
    assert!(exec.calls().is_empty());
}

#[test]
fn custom_reports_a_failing_command_and_a_missing_shell() {
    let (env, exec) = fake_env();
    exec.respond("sh", 3, "");
    let err = Custom.apply(&env, Variant::Dark, &cfg("a", "b")).unwrap_err();
    assert!(err.to_string().contains("status 3") && err.to_string().contains("Dark"), "{err}");

    let (env, exec) = fake_env();
    exec.missing("sh");
    assert!(Custom.apply(&env, Variant::Dark, &cfg("a", "b")).is_err());
}

#[test]
fn custom_is_a_free_text_plugin_that_is_always_available() {
    let (env, _) = fake_env();
    assert_eq!(Custom.key(), "custom");
    assert_eq!(Custom.name(), "Custom");
    assert_eq!(Custom.kind(), InputKind::Text);
    assert_eq!(Custom.defaults(), (None, None));
    assert!(Custom.available(&env));
    assert!(Custom.themes(&env).is_empty());
    assert_eq!(Custom.describe(Variant::Dark, &cfg("l", "notify-send ciao")), "custom: notify-send ciao");
}

#[test]
fn custom_sits_between_konsole_and_notification() {
    let keys: Vec<_> = registry().iter().map(|p| p.key()).collect();
    let at = |key: &str| keys.iter().position(|k| *k == key).unwrap();
    assert!(at("konsole") < at("custom") && at("custom") < at("notification"), "{keys:?}");
}

// ---- with a real shell --------------------------------------------------------------------

#[test]
fn a_real_shell_runs_pipes_quotes_and_redirections() {
    let dir = TempDir::new().unwrap();
    let out = dir.path().join("out.txt");
    let command = format!("echo \"a b\" | tr a-z A-Z > '{}'", out.display());
    Custom.apply(&real_env(), Variant::Dark, &cfg("", &command)).unwrap();
    assert_eq!(fs::read_to_string(out).unwrap(), "A B\n");
}

#[test]
fn a_real_failing_command_is_an_error_with_its_status() {
    let err = Custom.apply(&real_env(), Variant::Light, &cfg("exit 5", "")).unwrap_err();
    assert!(err.to_string().contains("status 5"), "{err}");
}

#[test]
fn a_command_that_leaves_a_background_process_does_not_block() {
    let started = Instant::now();
    Custom.apply(&real_env(), Variant::Dark, &cfg("", "sleep 2 &")).unwrap();
    assert!(started.elapsed() < Duration::from_millis(1500), "waited {:?}", started.elapsed());
}
