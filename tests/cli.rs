mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::{DateTime, FixedOffset, NaiveTime, TimeZone};
use clap::Parser;
use support::{FakeBus, FakeExec};
use tempfile::TempDir;
use yinyang::app::{self, Ctx};
use yinyang::cli::{ApplyArgs, Cli, Command};
use yinyang::config::{Config, Mode};
use yinyang::location::{LocationSource, NoNetworkLocation};
use yinyang::paths::Paths;
use yinyang::plugin::{registry, Env, Plugin};

const FIXTURE: &str = include_str!("fixtures/yin_yang_dev.json");

struct FakeLoc(Option<(f64, f64)>);

impl LocationSource for FakeLoc {
    fn current(&self) -> Option<(f64, f64)> {
        self.0
    }
}

fn at(year: i32, month: u32, day: u32, hour: u32) -> DateTime<FixedOffset> {
    FixedOffset::east_opt(0).unwrap().with_ymd_and_hms(year, month, day, hour, 0, 0).unwrap()
}

fn noon() -> DateTime<FixedOffset> {
    at(2026, 6, 21, 12)
}

fn t(h: u32, m: u32) -> NaiveTime {
    NaiveTime::from_hms_opt(h, m, 0).unwrap()
}

/// A fake home with the author's real config (scheduled 07:00-20:00, light, no location),
/// VS Code installed with a settings file, and every external call recorded.
struct Fixture {
    home: TempDir,
    paths: Paths,
    exec: Arc<FakeExec>,
    bus: Arc<FakeBus>,
    plugins: Vec<Box<dyn Plugin>>,
    kde: bool,
}

struct Run {
    code: i32,
    out: String,
    err: String,
}

impl Fixture {
    fn new() -> Fixture {
        let home = TempDir::new().unwrap();
        let paths = Paths::under(home.path());
        write(&home.path().join(".config/yin_yang_dev.json"), FIXTURE);
        write(
            &home.path().join(".vscode/extensions/pub.themes/package.json"),
            r#"{"contributes": {"themes": [{"id": "Solarized Dark"}, {"id": "Solarized Light"}]}}"#,
        );
        write(
            &home.path().join(".config/Code/User/settings.json"),
            "{\n\t\"workbench.colorTheme\": \"Solarized Light\"\n}\n",
        );
        let bus = FakeBus::new().with_services(&["org.kde.GtkConfig"]);
        let fixture = Fixture {
            home,
            paths,
            exec: Arc::new(FakeExec::new()),
            bus: Arc::new(bus),
            plugins: registry(),
            kde: true,
        };
        fixture.update_config(|c| {
            c.mode = Mode::Scheduled;
            c.times = (t(7, 0), t(20, 0));
            c.dark_mode = false;
            c.update_location = false;
            c.coordinates = (0.0, 0.0);
        });
        fixture
    }

    fn env(&self) -> Env {
        Env { home: self.home.path().into(), kde: self.kde, exec: self.exec.clone(), bus: self.bus.clone() }
    }

    fn config(&self) -> Config {
        Config::load(&self.paths).unwrap()
    }

    fn update_config(&self, change: impl FnOnce(&mut Config)) {
        let mut config = self.config();
        change(&mut config);
        config.save(&self.paths).unwrap();
    }

    fn settings(&self) -> String {
        fs::read_to_string(self.home.path().join(".config/Code/User/settings.json")).unwrap()
    }

    fn timer_file(&self) -> PathBuf {
        self.paths.systemd_dir.join("yinyang.timer")
    }

    /// Calls made to external programs other than systemctl.
    fn plugin_calls(&self) -> Vec<String> {
        self.exec.calls().into_iter().filter(|c| !c.starts_with("systemctl")).collect()
    }

    fn systemctl_calls(&self) -> Vec<String> {
        self.exec.calls().into_iter().filter(|c| c.starts_with("systemctl")).collect()
    }

    fn with_ctx<R>(&self, now: DateTime<FixedOffset>, loc: &dyn LocationSource, f: impl FnOnce(&Ctx) -> R) -> R {
        let env = self.env();
        let ctx = Ctx {
            env: &env,
            paths: &self.paths,
            plugins: &self.plugins,
            location: loc,
            exe: Path::new("/home/u/.local/bin/yinyang"),
            now,
        };
        f(&ctx)
    }

    fn apply(&self, args: &[&str], now: DateTime<FixedOffset>, loc: &dyn LocationSource) -> Run {
        let parsed = match Cli::try_parse_from([&["yinyang", "apply"], args].concat()).unwrap().command {
            Command::Apply(a) => a,
            _ => unreachable!(),
        };
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = self.with_ctx(now, loc, |ctx| app::apply(ctx, &parsed, &mut out, &mut err).unwrap());
        Run { code, out: String::from_utf8(out).unwrap(), err: String::from_utf8(err).unwrap() }
    }
}

fn write(path: &Path, content: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn tree(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                files.push((path.clone(), fs::read(&path).unwrap()));
            }
        }
    }
    files.sort();
    files
}

const DARK_SYSTEM: &str = "lookandfeeltool -a com.github.vinceliuice.MacTahoe-Dark";

// ---- argument parsing ---------------------------------------------------------------------

#[test]
fn the_command_line_accepts_the_documented_forms() {
    let parse = |args: &[&str]| Cli::try_parse_from([&["yinyang"], args].concat());
    for ok in [
        &["apply"][..],
        &["apply", "--auto"],
        &["apply", "--dark", "--force", "--dry-run"],
        &["apply", "--light"],
        &["toggle"],
        &["status"],
        &["plugins", "--json"],
        &["plugins"],
        &["config", "show"],
        &["config", "save"],
        &["timer", "refresh"],
    ] {
        assert!(parse(ok).is_ok(), "{ok:?}");
    }
    for bad in [&["apply", "--dark", "--light"][..], &["apply", "--auto", "--dark"], &["native-host"], &["config"]] {
        assert!(parse(bad).is_err(), "{bad:?}");
    }
}

#[test]
fn apply_without_a_flag_means_auto() {
    let Command::Apply(args) = Cli::try_parse_from(["yinyang", "apply"]).unwrap().command else { panic!() };
    assert_eq!(args.choice(), ApplyArgs { auto: true, ..Default::default() }.choice());
}

// ---- apply: choosing the theme ------------------------------------------------------------

#[test]
fn apply_dark_runs_the_enabled_plugins_system_first_and_saves_the_state() {
    let f = Fixture::new();
    let run = f.apply(&["--dark"], noon(), &FakeLoc(None));

    assert_eq!(run.code, 0, "{}", run.err);
    let calls = f.plugin_calls();
    assert_eq!(calls[0], DARK_SYSTEM, "system goes first: {calls:?}");
    assert!(calls.contains(&"plasma-apply-colorscheme MacTahoeDark".to_string()), "{calls:?}");
    assert!(calls.contains(&"kvantummanager --set MacTahoeDark".to_string()), "{calls:?}");
    assert_eq!(calls.len(), 3, "only the enabled plugins run: {calls:?}");
    let bus = f.bus.calls();
    assert!(bus.contains(&"call_str org.kde.GtkConfig /GtkConfig org.kde.GtkConfig setGtkTheme MacTahoe-Dark".to_string()), "{bus:?}");
    assert!(bus.contains(&"notify Theme changed | Set the theme to Night".to_string()), "{bus:?}");
    assert!(f.settings().contains("\"workbench.colorTheme\": \"Solarized Dark\""));
    assert!(f.config().dark_mode);
    assert!(run.err.contains("system: applied"), "{}", run.err);
}

#[test]
fn applying_the_current_theme_is_a_no_op_unless_forced_and_force_is_silent() {
    let f = Fixture::new();
    f.update_config(|c| c.dark_mode = true);

    let run = f.apply(&["--dark"], noon(), &FakeLoc(None));
    assert_eq!(run.code, 0);
    assert!(f.plugin_calls().is_empty(), "{:?}", f.plugin_calls());
    assert!(f.bus.calls().iter().all(|c| !c.starts_with("notify")));

    f.apply(&["--dark", "--force"], noon(), &FakeLoc(None));
    assert_eq!(f.plugin_calls()[0], DARK_SYSTEM);
    assert!(f.bus.calls().iter().all(|c| !c.starts_with("notify")), "force skips the notification");
}

#[test]
fn auto_picks_the_theme_from_the_clock() {
    let f = Fixture::new();
    f.apply(&["--auto"], at(2026, 6, 21, 21), &FakeLoc(None));
    assert_eq!(f.plugin_calls()[0], DARK_SYSTEM);
    assert!(f.config().dark_mode);

    let g = Fixture::new();
    g.update_config(|c| c.dark_mode = true);
    g.apply(&[], noon(), &FakeLoc(None));
    assert_eq!(g.plugin_calls()[0], "lookandfeeltool -a com.github.vinceliuice.MacTahoe-Light");
    assert!(!g.config().dark_mode);
}

#[test]
fn auto_without_a_location_fails_before_touching_any_plugin() {
    let f = Fixture::new();
    f.update_config(|c| {
        c.mode = Mode::FollowSun;
        c.update_location = true;
    });
    let run = f.apply(&["--auto"], at(2026, 6, 21, 21), &FakeLoc(None));

    assert_eq!(run.code, 1);
    assert!(run.err.contains("location"), "{}", run.err);
    assert!(f.plugin_calls().is_empty());
    assert!(!f.config().dark_mode, "the state is unchanged");
    assert!(f.bus.calls().iter().any(|c| c.starts_with("notify Yin-Yang | ")), "{:?}", f.bus.calls());
}

// ---- apply: the timer ---------------------------------------------------------------------

#[test]
fn every_run_renews_the_timer_even_when_the_theme_does_not_change() {
    let f = Fixture::new();
    f.update_config(|c| {
        c.mode = Mode::FollowSun;
        c.coordinates = (41.9028, 12.4964);
    });

    let run = f.apply(&["--auto"], noon(), &FakeLoc(None));

    assert_eq!(run.code, 0, "{}", run.err);
    assert!(f.plugin_calls().is_empty(), "noon is light and the state is light");
    let timer = fs::read_to_string(f.timer_file()).unwrap();
    assert!(timer.contains("OnCalendar=03:34:49\n") && timer.contains("OnCalendar=18:48:38\n"), "{timer}");
    assert_eq!(f.systemctl_calls(), ["systemctl --user daemon-reload", "systemctl --user enable --now yinyang.timer"]);

    // Tomorrow the sun moves; the same command, with nothing to switch, moves the timer too.
    let g = Fixture::new();
    g.update_config(|c| {
        c.mode = Mode::FollowSun;
        c.coordinates = (41.9028, 12.4964);
    });
    g.apply(&["--auto"], at(2026, 6, 21, 12), &FakeLoc(None));
    let before = fs::read_to_string(g.timer_file()).unwrap();
    g.apply(&["--auto"], at(2026, 12, 21, 12), &FakeLoc(None));
    assert_ne!(fs::read_to_string(g.timer_file()).unwrap(), before);
}

#[test]
fn an_explicit_theme_with_unknown_times_still_works_without_a_timer() {
    let f = Fixture::new();
    f.update_config(|c| {
        c.mode = Mode::FollowSun;
        c.update_location = true;
    });
    let run = f.apply(&["--dark"], noon(), &FakeLoc(None));
    assert_eq!(run.code, 0, "{}", run.err);
    assert_eq!(f.plugin_calls()[0], DARK_SYSTEM);
    assert!(!f.timer_file().exists());
}

#[test]
fn a_broken_systemctl_is_reported_but_does_not_stop_the_theme_switch() {
    let f = Fixture::new();
    f.exec.respond_with_stderr("systemctl", 1, "", "Failed to connect to bus");
    let run = f.apply(&["--dark"], noon(), &FakeLoc(None));
    assert_eq!(run.code, 1);
    assert!(run.err.contains("Failed to connect to bus"), "{}", run.err);
    assert_eq!(f.plugin_calls()[0], DARK_SYSTEM);
    assert!(f.config().dark_mode, "the theme did switch");
}

// ---- apply: failures ----------------------------------------------------------------------

#[test]
fn a_failing_plugin_gives_exit_1_the_others_still_run_and_the_state_stays() {
    let f = Fixture::new();
    f.exec.fail("kvantummanager", "boom");
    let run = f.apply(&["--dark"], noon(), &FakeLoc(None));

    assert_eq!(run.code, 1);
    assert!(run.err.contains("kvantum: FAILED") && run.err.contains("boom"), "{}", run.err);
    assert!(f.plugin_calls().contains(&"plasma-apply-colorscheme MacTahoeDark".to_string()));
    assert!(!f.config().dark_mode, "so that the next run tries again");
    assert!(f.bus.calls().iter().all(|c| !c.starts_with("notify Yin-Yang")), "only --auto notifies");
}

#[test]
fn an_automatic_run_that_fails_tells_the_user() {
    let f = Fixture::new();
    f.exec.fail("kvantummanager", "boom");
    let run = f.apply(&["--auto"], at(2026, 6, 21, 21), &FakeLoc(None));
    assert_eq!(run.code, 1);
    let notices: Vec<_> = f.bus.calls().into_iter().filter(|c| c.starts_with("notify Yin-Yang | ")).collect();
    assert_eq!(notices.len(), 1, "{notices:?}");
    assert!(notices[0].contains("kvantum") && notices[0].contains("boom"), "{notices:?}");
}

#[test]
fn outside_a_kde_session_the_skipped_plugins_make_the_run_retry_later() {
    let mut f = Fixture::new();
    f.kde = false;
    let run = f.apply(&["--dark"], noon(), &FakeLoc(None));
    assert_eq!(run.code, 1);
    assert!(run.err.contains("KDE"), "{}", run.err);
    assert!(!f.config().dark_mode, "an early run before the session is up must not mark the theme as applied");

    let g = Fixture::new();
    g.exec.missing("kvantummanager");
    let run = g.apply(&["--dark"], noon(), &FakeLoc(None));
    assert_eq!(run.code, 0, "a tool that is simply not installed is not a failure: {}", run.err);
    assert!(run.err.contains("kvantum: skipped"));
    assert!(g.config().dark_mode);
}

// ---- apply --dry-run ----------------------------------------------------------------------

#[test]
fn dry_run_describes_the_switch_and_changes_nothing() {
    let f = Fixture::new();
    let before = tree(f.home.path());
    let run = f.apply(&["--dark", "--dry-run"], noon(), &FakeLoc(Some((45.0, 9.0))));

    assert_eq!(run.code, 0, "{}", run.err);
    assert!(run.out.starts_with("variant: dark\n"), "{}", run.out);
    for line in [
        "system: com.github.vinceliuice.MacTahoe-Dark",
        "colors: MacTahoeDark",
        "kvantum: MacTahoeDark",
        "vs code: Solarized Dark",
        "notification: Night",
    ] {
        assert!(run.out.lines().any(|l| l == line), "missing {line:?} in\n{}", run.out);
    }
    assert!(f.exec.calls().is_empty() && f.bus.calls().is_empty());
    assert_eq!(tree(f.home.path()), before, "no file may be created or changed");
}

#[test]
fn dry_run_respects_the_no_change_rule_and_force() {
    let f = Fixture::new();
    f.update_config(|c| c.dark_mode = true);
    let run = f.apply(&["--dark", "--dry-run"], noon(), &FakeLoc(None));
    assert!(run.out.contains("nothing to do") && !run.out.contains("system:"), "{}", run.out);
    let forced = f.apply(&["--dark", "--force", "--dry-run"], noon(), &FakeLoc(None));
    assert!(forced.out.contains("system:") && !forced.out.contains("notification:"), "{}", forced.out);
}

// ---- toggle -------------------------------------------------------------------------------

#[test]
fn toggle_goes_manual_flips_the_theme_and_stops_the_timer() {
    let f = Fixture::new();
    f.apply(&["--light", "--force"], noon(), &FakeLoc(None)); // installs the timer
    assert!(f.timer_file().is_file());
    let exec_calls_before = f.exec.calls().len();

    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = f.with_ctx(noon(), &FakeLoc(None), |ctx| app::toggle(ctx, &mut out, &mut err).unwrap());

    assert_eq!(code, 0, "{}", String::from_utf8_lossy(&err));
    let saved = f.config();
    assert_eq!(saved.mode, Mode::Manual);
    assert!(saved.dark_mode);
    let calls = &f.exec.calls()[exec_calls_before..];
    assert!(calls.contains(&DARK_SYSTEM.to_string()), "{calls:?}");
    assert!(calls.contains(&"systemctl --user disable --now yinyang.timer".to_string()), "{calls:?}");
}

// ---- status / plugins / config ------------------------------------------------------------

#[test]
fn status_reports_state_times_and_location_without_the_network() {
    let f = Fixture::new();
    let mut out = Vec::new();
    f.with_ctx(noon(), &FakeLoc(Some((1.0, 1.0))), |ctx| app::status(ctx, &mut out).unwrap());
    let json: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(json["dark_mode"], false);
    assert_eq!(json["mode"], "manual time");
    assert_eq!(json["times"], serde_json::json!(["07:00", "20:00"]));
    assert_eq!(json["location"], serde_json::Value::Null, "(0, 0) is not a location, and the fake source is never asked");

    f.update_config(|c| {
        c.mode = Mode::FollowSun;
        c.coordinates = (41.9028, 12.4964);
    });
    let mut out = Vec::new();
    f.with_ctx(noon(), &FakeLoc(None), |ctx| app::status(ctx, &mut out).unwrap());
    let json: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(json["times"], serde_json::json!(["03:34", "18:48"]));
    assert_eq!(json["location"], serde_json::json!([41.9028, 12.4964]));
}

#[test]
fn plugins_lists_every_plugin_in_gui_order() {
    let f = Fixture::new();
    f.exec.respond("lookandfeeltool", 0, "org.kde.breeze.desktop\n");
    let mut out = Vec::new();
    f.with_ctx(noon(), &NoNetworkLocation, |ctx| app::plugins_json(ctx, &mut out).unwrap());
    let json: serde_json::Value = serde_json::from_slice(&out).unwrap();
    let list = json.as_array().unwrap();

    let keys: Vec<_> = list.iter().map(|p| p["key"].as_str().unwrap()).collect();
    let expected: Vec<_> = registry().iter().map(|p| p.key()).collect();
    assert_eq!(keys, expected);
    assert_eq!(list.len(), 12);
    let by_key = |key: &str| list.iter().find(|p| p["key"] == key).unwrap();
    assert_eq!(by_key("wallpaper")["kind"], "file");
    assert_eq!(by_key("notification")["kind"], "text");
    assert_eq!(by_key("kvantum")["kind"], "choice");
    assert_eq!(by_key("system")["available"], true);
    assert_eq!(by_key("system")["themes"][0]["id"], "org.kde.breeze.desktop");
    assert_eq!(by_key("okular")["available"], false);
    assert_eq!(by_key("okular")["themes"], serde_json::json!([]), "no themes are listed for an unavailable plugin");
    for plugin in list {
        assert!(plugin["name"].is_string() && plugin["themes"].is_array());
    }
}

#[test]
fn config_show_defaults_prints_the_fresh_config_and_ignores_the_file() {
    let f = Fixture::new();
    f.update_config(|c| {
        c.mode = Mode::FollowSun;
        c.boot_offset = 99;
    });
    let mut out = Vec::new();
    f.with_ctx(noon(), &NoNetworkLocation, |ctx| app::config_show(ctx, true, &mut out).unwrap());
    let shown: Config = serde_json::from_slice(&out).unwrap();
    assert_eq!(shown.mode, Mode::Manual);
    assert_eq!(shown.boot_offset, 10);
    assert!(!shown.plugins["system"].enabled, "plugins start disabled");
    assert_eq!(shown.plugins["kvantum"].light_theme.as_deref(), Some("KvFlatLight"));
    assert_eq!(shown.plugins["kvantum"].dark_theme.as_deref(), Some("KvFlat"));
    assert_eq!(shown.plugins["custom"].light_theme, None);
    assert!(!shown.plugins.contains_key("firefox"));
}

#[test]
fn config_show_prints_the_config_completed_with_every_plugin() {
    let f = Fixture::new();
    let mut out = Vec::new();
    f.with_ctx(noon(), &NoNetworkLocation, |ctx| app::config_show(ctx, false, &mut out).unwrap());
    let shown: Config = serde_json::from_slice(&out).unwrap();
    for plugin in registry() {
        assert!(shown.plugins.contains_key(plugin.key()), "{} missing", plugin.key());
    }
    assert!(shown.plugins.contains_key("firefox"), "entries of other versions survive");
    assert!(shown.plugins["system"].enabled);

    let empty = Fixture::new();
    fs::remove_file(&empty.paths.config_file).unwrap();
    fs::remove_file(empty.home.path().join(".config/yin_yang_dev.json")).unwrap();
    let mut out = Vec::new();
    empty.with_ctx(noon(), &NoNetworkLocation, |ctx| app::config_show(ctx, false, &mut out).unwrap());
    let fresh: Config = serde_json::from_slice(&out).unwrap();
    assert_eq!(fresh.plugins.len(), 12);
    assert!(fresh.plugins.values().all(|p| !p.enabled));
    assert_eq!(fresh.plugins["system"].dark_theme.as_deref(), Some("org.kde.breezedark.desktop"));
}

fn save(f: &Fixture, json: &str) -> (i32, String) {
    let mut err = Vec::new();
    let code = f.with_ctx(noon(), &FakeLoc(None), |ctx| app::config_save(ctx, &mut json.as_bytes(), &mut err).unwrap());
    (code, String::from_utf8(err).unwrap())
}

#[test]
fn config_save_stores_the_settings_but_keeps_the_state_in_the_file() {
    let f = Fixture::new();
    let mut edited = f.config();
    edited.times = (t(6, 30), t(21, 15));
    edited.dark_mode = true; // a stale value from a window opened before the timer ran
    edited.plugins.get_mut("system").unwrap().enabled = false;

    let (code, err) = save(&f, &serde_json::to_string(&edited).unwrap());

    assert_eq!(code, 0, "{err}");
    let saved = f.config();
    assert_eq!(saved.times, (t(6, 30), t(21, 15)));
    assert!(!saved.plugins["system"].enabled);
    assert!(!saved.dark_mode, "dark_mode in the file wins over the incoming one");
    let timer = fs::read_to_string(f.timer_file()).unwrap();
    assert!(timer.contains("OnCalendar=06:30:00") && timer.contains("OnCalendar=21:15:00"), "{timer}");
}

#[test]
fn config_save_rejects_invalid_input_and_leaves_the_file_alone() {
    let f = Fixture::new();
    let before = fs::read(&f.paths.config_file).unwrap();
    for bad in [
        "not json",
        "",
        r#"{"mode": "nonsense"}"#,
        r#"{"times": ["25:99", "20:00"]}"#,
        r#"{"plugins": {"system": {"enabled": "yes"}}}"#,
    ] {
        let (code, err) = save(&f, bad);
        assert_eq!(code, 2, "{bad:?}");
        assert!(err.contains("invalid"), "{err}");
    }
    assert_eq!(fs::read(&f.paths.config_file).unwrap(), before);
    assert!(f.exec.calls().is_empty());
}

#[test]
fn timer_refresh_installs_or_stops_the_timer_from_the_config() {
    let f = Fixture::new();
    let mut err = Vec::new();
    let code = f.with_ctx(noon(), &FakeLoc(None), |ctx| app::timer_refresh(ctx, &mut err).unwrap());
    assert_eq!(code, 0, "{}", String::from_utf8_lossy(&err));
    assert!(f.timer_file().is_file());

    f.update_config(|c| c.mode = Mode::Manual);
    let code = f.with_ctx(noon(), &FakeLoc(None), |ctx| app::timer_refresh(ctx, &mut err).unwrap());
    assert_eq!(code, 0);
    assert_eq!(f.systemctl_calls().last().unwrap(), "systemctl --user disable --now yinyang.timer");
}

// ---- concurrency --------------------------------------------------------------------------

#[test]
fn two_simultaneous_runs_are_serialised_and_the_second_sees_the_first() {
    let f = Fixture::new();
    let args = ApplyArgs { dark: true, ..Default::default() };
    std::thread::scope(|scope| {
        for _ in 0..2 {
            scope.spawn(|| {
                let (mut out, mut err) = (Vec::new(), Vec::new());
                f.with_ctx(noon(), &FakeLoc(None), |ctx| app::apply(ctx, &args, &mut out, &mut err).unwrap());
            });
        }
    });

    let system_runs = f.plugin_calls().iter().filter(|c| c.starts_with("lookandfeeltool")).count();
    assert_eq!(system_runs, 1, "the second run found the theme already applied: {:?}", f.plugin_calls());
    assert!(f.config().dark_mode);
    serde_json::from_slice::<serde_json::Value>(&fs::read(&f.paths.config_file).unwrap()).unwrap();
}
