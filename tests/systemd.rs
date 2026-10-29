mod support;

use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use chrono::NaiveTime;
use support::FakeExec;
use tempfile::TempDir;
use yinyang::config::{Config, Mode};
use yinyang::paths::Paths;
use yinyang::systemd::{refresh, render_service, render_timer};

fn t(h: u32, m: u32, s: u32) -> NaiveTime {
    NaiveTime::from_hms_opt(h, m, s).unwrap()
}

const EXE: &str = "/home/u/.local/bin/yinyang";
const RELOAD: &str = "systemctl --user daemon-reload";
const ENABLE: &str = "systemctl --user enable --now yinyang.timer";
const DISABLE: &str = "systemctl --user disable --now yinyang.timer";

fn config(mode: Mode) -> Config {
    Config { mode, boot_offset: 10, ..Config::default() }
}

fn setup() -> (TempDir, Paths, Arc<FakeExec>) {
    let home = TempDir::new().unwrap();
    let paths = Paths::under(home.path());
    (home, paths, Arc::new(FakeExec::new()))
}

fn timer_file(paths: &Paths) -> std::path::PathBuf {
    paths.systemd_dir.join("yinyang.timer")
}

fn service_file(paths: &Paths) -> std::path::PathBuf {
    paths.systemd_dir.join("yinyang.service")
}

fn run_refresh(cfg: &Config, paths: &Paths, exec: &Arc<FakeExec>, times: Option<(NaiveTime, NaiveTime)>) -> anyhow::Result<()> {
    refresh(cfg, paths, exec.as_ref(), Path::new(EXE), times)
}

// ---- rendering ----------------------------------------------------------------------------

#[test]
fn the_timer_fires_at_both_times_at_start_up_and_catches_up() {
    assert_eq!(
        render_timer(t(7, 30, 36), t(18, 11, 24), 10),
        "[Unit]\nDescription=Automatic light and dark mode (Yin-Yang)\n\n[Timer]\nOnCalendar=07:30:36\nOnCalendar=18:11:24\nOnStartupSec=10\nPersistent=true\n\n[Install]\nWantedBy=timers.target\n"
    );
}

#[test]
fn the_timer_drops_fractions_of_a_second() {
    let with_nanos = NaiveTime::from_hms_nano_opt(7, 0, 1, 987_000_000).unwrap();
    assert!(render_timer(with_nanos, t(20, 0, 0), 5).contains("OnCalendar=07:00:01\n"));
}

#[test]
fn the_service_runs_apply_auto_once_the_session_is_up() {
    assert_eq!(
        render_service(Path::new(EXE)),
        "[Unit]\nDescription=Automatic light and dark mode (Yin-Yang)\nAfter=graphical-session.target\nStartLimitIntervalSec=10min\nStartLimitBurst=5\n\n[Service]\nType=oneshot\nExecStart=/home/u/.local/bin/yinyang apply --auto\nTimeoutStartSec=2min\nRestartForceExitStatus=75\nRestartSec=20\n"
    );
}

#[test]
fn the_service_quotes_an_executable_path_that_systemd_would_split() {
    assert!(render_service(Path::new("/home/u/My Apps/yinyang")).contains("ExecStart=\"/home/u/My Apps/yinyang\" apply --auto\n"));
    assert!(render_service(Path::new("/opt/100%/y")).contains("ExecStart=\"/opt/100%%/y\" apply --auto\n"));
    assert!(render_service(Path::new("/opt/a$b/y")).contains("ExecStart=\"/opt/a$$b/y\" apply --auto\n"));
}

// ---- refresh ------------------------------------------------------------------------------

#[test]
fn the_first_refresh_writes_both_units_reloads_and_starts_the_timer() {
    let (_home, paths, exec) = setup();
    run_refresh(&config(Mode::Scheduled), &paths, &exec, Some((t(7, 30, 36), t(18, 11, 24)))).unwrap();

    assert_eq!(fs::read_to_string(timer_file(&paths)).unwrap(), render_timer(t(7, 30, 36), t(18, 11, 24), 10));
    assert_eq!(fs::read_to_string(service_file(&paths)).unwrap(), render_service(Path::new(EXE)));
    assert_eq!(exec.calls(), [RELOAD, ENABLE]);
}

#[test]
fn refreshing_with_the_same_times_rewrites_nothing_but_still_starts_the_timer() {
    let (_home, paths, exec) = setup();
    let times = Some((t(7, 30, 36), t(18, 11, 24)));
    run_refresh(&config(Mode::FollowSun), &paths, &exec, times).unwrap();
    let modified = fs::metadata(timer_file(&paths)).unwrap().modified().unwrap();
    std::thread::sleep(Duration::from_millis(30));

    let second = Arc::new(FakeExec::new());
    run_refresh(&config(Mode::FollowSun), &paths, &second, times).unwrap();

    assert_eq!(second.calls(), [ENABLE], "no reload when no file changed");
    assert_eq!(fs::metadata(timer_file(&paths)).unwrap().modified().unwrap(), modified);
}

#[test]
fn tomorrows_sunrise_rewrites_the_timer_and_reloads() {
    let (_home, paths, exec) = setup();
    run_refresh(&config(Mode::FollowSun), &paths, &exec, Some((t(7, 30, 36), t(18, 11, 24)))).unwrap();

    let second = Arc::new(FakeExec::new());
    run_refresh(&config(Mode::FollowSun), &paths, &second, Some((t(7, 31, 40), t(18, 10, 5)))).unwrap();

    assert_eq!(fs::read_to_string(timer_file(&paths)).unwrap(), render_timer(t(7, 31, 40), t(18, 10, 5), 10));
    assert_eq!(second.calls(), [RELOAD, ENABLE]);
}

#[test]
fn a_new_boot_offset_or_executable_is_picked_up() {
    let (_home, paths, exec) = setup();
    let times = Some((t(7, 0, 0), t(20, 0, 0)));
    run_refresh(&config(Mode::Scheduled), &paths, &exec, times).unwrap();

    let second = Arc::new(FakeExec::new());
    let cfg = Config { boot_offset: 30, ..config(Mode::Scheduled) };
    run_refresh(&cfg, &paths, &second, times).unwrap();
    assert!(fs::read_to_string(timer_file(&paths)).unwrap().contains("OnStartupSec=30\n"));
    assert_eq!(second.calls(), [RELOAD, ENABLE]);

    let third = Arc::new(FakeExec::new());
    refresh(&cfg, &paths, third.as_ref(), Path::new("/usr/local/bin/yinyang"), times).unwrap();
    assert!(fs::read_to_string(service_file(&paths)).unwrap().contains("ExecStart=/usr/local/bin/yinyang apply --auto"));
    assert_eq!(third.calls(), [RELOAD, ENABLE]);
}

#[test]
fn unknown_times_leave_existing_units_alone() {
    let (_home, paths, exec) = setup();
    run_refresh(&config(Mode::FollowSun), &paths, &exec, Some((t(7, 30, 36), t(18, 11, 24)))).unwrap();
    let before = fs::read_to_string(timer_file(&paths)).unwrap();

    let second = Arc::new(FakeExec::new());
    run_refresh(&config(Mode::FollowSun), &paths, &second, None).unwrap();

    assert_eq!(fs::read_to_string(timer_file(&paths)).unwrap(), before);
    assert_eq!(second.calls(), [ENABLE]);
}

#[test]
fn unknown_times_and_no_units_is_an_error_that_starts_nothing() {
    let (_home, paths, exec) = setup();
    let err = run_refresh(&config(Mode::FollowSun), &paths, &exec, None).unwrap_err();
    assert!(err.to_string().contains("times"), "{err}");
    assert!(exec.calls().is_empty());
    assert!(!timer_file(&paths).exists());
}

#[test]
fn manual_mode_stops_an_installed_timer_and_keeps_the_files() {
    let (_home, paths, exec) = setup();
    run_refresh(&config(Mode::Scheduled), &paths, &exec, Some((t(7, 0, 0), t(20, 0, 0)))).unwrap();

    let second = Arc::new(FakeExec::new());
    run_refresh(&config(Mode::Manual), &paths, &second, Some((t(7, 0, 0), t(20, 0, 0)))).unwrap();

    assert_eq!(second.calls(), [DISABLE]);
    assert!(timer_file(&paths).is_file());
}

#[test]
fn manual_mode_with_nothing_installed_does_nothing() {
    let (_home, paths, exec) = setup();
    run_refresh(&config(Mode::Manual), &paths, &exec, None).unwrap();
    assert!(exec.calls().is_empty());
    assert!(!paths.systemd_dir.exists(), "no unit files are created for manual mode");
}

#[test]
fn a_failing_systemctl_reports_why() {
    let (_home, paths, exec) = setup();
    exec.respond_with_stderr("systemctl", 1, "", "Failed to connect to bus: No medium found");
    let err = run_refresh(&config(Mode::Scheduled), &paths, &exec, Some((t(7, 0, 0), t(20, 0, 0)))).unwrap_err();
    assert!(err.to_string().contains("Failed to connect to bus"), "{err}");
}

#[test]
fn a_missing_systemctl_is_an_error() {
    let (_home, paths, exec) = setup();
    exec.missing("systemctl");
    assert!(run_refresh(&config(Mode::Scheduled), &paths, &exec, Some((t(7, 0, 0), t(20, 0, 0)))).is_err());
}
