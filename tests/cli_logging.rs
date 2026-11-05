//! Runs the real binary: the stderr lock lives in `main`, out of reach of the library tests.
use std::fs;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use tempfile::TempDir;
use yinyang::config::{Config, PluginConfig};
use yinyang::paths::Paths;

/// A program that does nothing and succeeds.
fn fake_program(dir: &Path, name: &str) {
    let path = dir.join(name);
    fs::write(&path, "#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
}

/// `apply` runs the plugins on their own threads, and with `debug` logging each of them writes to
/// stderr. If `main` holds the stderr lock for the whole command those writes wait for ever, and
/// the command never ends.
#[test]
fn plugin_threads_can_log_while_apply_runs() {
    let home = TempDir::new().unwrap();
    let bin = TempDir::new().unwrap();
    for program in ["lookandfeeltool", "plasma-apply-colorscheme", "systemctl"] {
        fake_program(bin.path(), program);
    }

    // Manual mode needs no network, and `system` runs alone first so `colors` runs on a thread of its own.
    let mut config = Config::default();
    for key in ["system", "colors"] {
        let plugin = PluginConfig { enabled: true, light_theme: Some("Light".into()), dark_theme: Some("Dark".into()) };
        config.plugins.insert(key.to_string(), plugin);
    }
    config.save(&Paths::under(home.path())).unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_yinyang"))
        .args(["apply", "--dark", "--force"])
        .env_clear()
        .env("HOME", home.path())
        .env("PATH", bin.path())
        .env("XDG_CURRENT_DESKTOP", "KDE")
        .env("RUST_LOG", "yinyang=debug")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let reader = thread::spawn(move || {
        let mut text = String::new();
        let _ = stderr.read_to_string(&mut text);
        text
    });

    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("apply did not finish: a plugin thread is stuck writing its log");
        }
        thread::sleep(Duration::from_millis(20));
    };

    let log = reader.join().unwrap();
    assert!(status.success(), "apply failed: {log}");
    assert!(log.contains("running plasma-apply-colorscheme"), "no log from the plugin thread: {log}");
}
