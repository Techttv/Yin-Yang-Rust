mod support;

use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;

use anyhow::{bail, Result};
use support::{FakeBus, FakeExec};
use yinyang::config::{Config, PluginConfig};
use yinyang::plugin::{
    registry, theme_for, Env, InputKind, Plugin, ThemeChoice, Variant,
};
use yinyang::runner::{apply_all, plan, Outcome};

type Log = Arc<Mutex<Vec<String>>>;

#[derive(Clone, Copy)]
enum Behavior {
    Succeed,
    Fail,
    Panic,
    SleepThenSucceed(u64),
}

struct Fake {
    key: &'static str,
    behavior: Behavior,
    available: bool,
    notification: bool,
    log: Log,
}

impl Fake {
    fn new(key: &'static str, log: &Log) -> Self {
        Fake {
            key,
            behavior: Behavior::Succeed,
            available: true,
            notification: false,
            log: log.clone(),
        }
    }

    fn behaving(mut self, behavior: Behavior) -> Self {
        self.behavior = behavior;
        self
    }

    fn unavailable(mut self) -> Self {
        self.available = false;
        self
    }

    fn notification(mut self) -> Self {
        self.notification = true;
        self
    }

    fn boxed(self) -> Box<dyn Plugin> {
        Box::new(self)
    }
}

impl Plugin for Fake {
    fn key(&self) -> &'static str {
        self.key
    }
    fn name(&self) -> &'static str {
        self.key
    }
    fn kind(&self) -> InputKind {
        InputKind::Choice
    }
    fn defaults(&self) -> (Option<&'static str>, Option<&'static str>) {
        (None, None)
    }
    fn available(&self, _env: &Env) -> bool {
        self.available
    }
    fn themes(&self, _env: &Env) -> Vec<ThemeChoice> {
        Vec::new()
    }
    fn is_notification(&self) -> bool {
        self.notification
    }
    fn apply(&self, _env: &Env, variant: Variant, cfg: &PluginConfig) -> Result<()> {
        let theme = theme_for(cfg, variant).unwrap_or("-");
        self.log
            .lock()
            .unwrap()
            .push(format!("start {} {variant:?} {theme}", self.key));
        match self.behavior {
            Behavior::Succeed => {}
            Behavior::Fail => bail!("{} failed", self.key),
            Behavior::Panic => panic!("{} blew up", self.key),
            Behavior::SleepThenSucceed(ms) => sleep(Duration::from_millis(ms)),
        }
        self.log.lock().unwrap().push(format!("end {}", self.key));
        Ok(())
    }
}

fn log() -> Log {
    Arc::new(Mutex::new(Vec::new()))
}

fn env() -> (Env, Arc<FakeExec>, Arc<FakeBus>) {
    let exec = Arc::new(FakeExec::new());
    let bus = Arc::new(FakeBus::new());
    let env = Env {
        home: "/nonexistent".into(),
        kde: true,
        exec: exec.clone(),
        bus: bus.clone(),
    };
    (env, exec, bus)
}

fn config(enabled: &[&str], disabled: &[&str]) -> Config {
    let mut cfg = Config::default();
    let entry = |enabled| PluginConfig {
        enabled,
        light_theme: Some("light-theme".into()),
        dark_theme: Some("dark-theme".into()),
    };
    for key in enabled {
        cfg.plugins.insert(key.to_string(), entry(true));
    }
    for key in disabled {
        cfg.plugins.insert(key.to_string(), entry(false));
    }
    cfg
}

fn describe(outcome: &Outcome) -> String {
    match outcome {
        Outcome::Applied => "applied".into(),
        Outcome::Skipped(_) => "skipped".into(),
        Outcome::Failed(e) => format!("failed: {e}"),
    }
}

fn summary(results: &[(String, Outcome)]) -> Vec<String> {
    results
        .iter()
        .map(|(key, outcome)| format!("{key}={}", describe(outcome)))
        .collect()
}

#[test]
fn failing_plugin_does_not_stop_others() {
    let log = log();
    let plugins = vec![
        Fake::new("a", &log).behaving(Behavior::Fail).boxed(),
        Fake::new("b", &log).boxed(),
    ];
    let (env, _, _) = env();
    let results = apply_all(&env, &config(&["a", "b"], &[]), &plugins, Variant::Dark, false);
    assert_eq!(summary(&results), ["a=failed: a failed", "b=applied"]);
}

#[test]
fn panicking_plugin_is_failed_not_propagated() {
    let log = log();
    let plugins = vec![
        Fake::new("a", &log).behaving(Behavior::Panic).boxed(),
        Fake::new("b", &log).boxed(),
    ];
    let (env, _, _) = env();
    let results = apply_all(&env, &config(&["a", "b"], &[]), &plugins, Variant::Dark, false);
    let described = summary(&results);
    assert!(described[0].starts_with("a=failed:") && described[0].contains("a blew up"), "{described:?}");
    assert_eq!(described[1], "b=applied");
}

#[test]
fn disabled_not_listed_unavailable_skipped() {
    let log = log();
    let plugins = vec![
        Fake::new("a", &log).unavailable().boxed(),
        Fake::new("b", &log).boxed(),
        Fake::new("c", &log).boxed(),
        Fake::new("d", &log).boxed(), // not in the config at all
    ];
    let (env, _, _) = env();
    let results = apply_all(&env, &config(&["a", "c"], &["b"]), &plugins, Variant::Dark, false);
    assert_eq!(summary(&results), ["a=skipped", "c=applied"]);
    let calls = log.lock().unwrap().clone();
    assert_eq!(calls, ["start c Dark dark-theme", "end c"]);
}

#[test]
fn quiet_skips_notification_only() {
    let log = log();
    let plugins = vec![
        Fake::new("a", &log).boxed(),
        Fake::new("notify", &log).notification().boxed(),
    ];
    let (env, _, _) = env();
    let cfg = config(&["a", "notify"], &[]);
    let quiet = apply_all(&env, &cfg, &plugins, Variant::Light, true);
    assert_eq!(summary(&quiet), ["a=applied"]);
    let loud = apply_all(&env, &cfg, &plugins, Variant::Light, false);
    assert_eq!(summary(&loud), ["a=applied", "notify=applied"]);
}

#[test]
fn plugin_receives_the_variant_and_its_own_config() {
    let log = log();
    let plugins = vec![Fake::new("a", &log).boxed()];
    let (env, _, _) = env();
    apply_all(&env, &config(&["a"], &[]), &plugins, Variant::Light, false);
    assert_eq!(log.lock().unwrap()[0], "start a Light light-theme");
}

#[test]
fn system_runs_before_all_others() {
    let log = log();
    let plugins = vec![
        Fake::new("colors", &log).boxed(),
        Fake::new("system", &log).behaving(Behavior::SleepThenSucceed(80)).boxed(),
        Fake::new("icons", &log).boxed(),
    ];
    let (env, _, _) = env();
    let results = apply_all(&env, &config(&["colors", "system", "icons"], &[]), &plugins, Variant::Dark, false);
    assert_eq!(
        summary(&results),
        ["colors=applied", "system=applied", "icons=applied"],
        "results keep registry order"
    );
    let events = log.lock().unwrap().clone();
    let system_end = events.iter().position(|e| e == "end system").unwrap();
    for other in ["start colors Dark dark-theme", "start icons Dark dark-theme"] {
        let at = events.iter().position(|e| e == other).unwrap();
        assert!(system_end < at, "{other} started before system finished: {events:?}");
    }
}

#[test]
fn others_run_in_parallel() {
    let log = log();
    let plugins = vec![
        Fake::new("a", &log).behaving(Behavior::SleepThenSucceed(150)).boxed(),
        Fake::new("b", &log).behaving(Behavior::SleepThenSucceed(150)).boxed(),
    ];
    let (env, _, _) = env();
    let started = std::time::Instant::now();
    apply_all(&env, &config(&["a", "b"], &[]), &plugins, Variant::Dark, false);
    assert!(
        started.elapsed() < Duration::from_millis(280),
        "plugins ran one after the other: {:?}",
        started.elapsed()
    );
}

#[test]
fn results_follow_registry_order() {
    let log = log();
    let plugins = vec![
        Fake::new("slow", &log).behaving(Behavior::SleepThenSucceed(80)).boxed(),
        Fake::new("fast", &log).boxed(),
    ];
    let (env, _, _) = env();
    let results = apply_all(&env, &config(&["slow", "fast"], &[]), &plugins, Variant::Dark, false);
    assert_eq!(summary(&results), ["slow=applied", "fast=applied"]);
}

#[test]
fn plan_executes_nothing() {
    let log = log();
    let plugins = vec![
        Fake::new("a", &log).boxed(),
        Fake::new("b", &log).boxed(),
        Fake::new("notify", &log).notification().boxed(),
    ];
    let (_, exec, bus) = env();
    let cfg = config(&["a", "notify"], &["b"]);
    assert_eq!(plan(&cfg, &plugins, Variant::Dark, false), ["a: dark-theme", "notify: dark-theme"]);
    assert_eq!(plan(&cfg, &plugins, Variant::Light, true), ["a: light-theme"]);
    assert!(log.lock().unwrap().is_empty());
    assert!(exec.calls().is_empty() && bus.calls().is_empty());
}

#[test]
fn theme_for_rejects_none_and_empty() {
    let cfg = |light: Option<&str>, dark: Option<&str>| PluginConfig {
        enabled: true,
        light_theme: light.map(Into::into),
        dark_theme: dark.map(Into::into),
    };
    assert_eq!(theme_for(&cfg(Some("l"), Some("d")), Variant::Light).unwrap(), "l");
    assert_eq!(theme_for(&cfg(Some("l"), Some("d")), Variant::Dark).unwrap(), "d");
    assert!(theme_for(&cfg(None, Some("d")), Variant::Light).is_err());
    assert!(theme_for(&cfg(Some("l"), Some("")), Variant::Dark).is_err());
}

#[test]
fn registry_keys_are_unique() {
    let keys: Vec<_> = registry().iter().map(|p| p.key()).collect();
    let mut unique = keys.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(keys.len(), unique.len(), "{keys:?}");
}
