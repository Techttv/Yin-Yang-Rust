//! The commands behind the CLI. Each takes a `Ctx` holding everything it touches, so tests can
//! swap the environment, the clock and the location.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::{DateTime, FixedOffset, NaiveTime};
use serde_json::{json, Value};

use crate::bus::RealBus;
use crate::cli::{ApplyArgs, Choice};
use crate::config::{Config, Mode};
use crate::exec::RealExec;
use crate::location::{self, LocationSource, NoNetworkLocation};
use crate::paths::Paths;
use crate::plugin::{Env, Plugin, Variant};
use crate::runner::{self, Outcome};
use crate::schedule::{effective_times, should_be_dark};
use crate::systemd;

pub struct Ctx<'a> {
    pub env: &'a Env,
    pub paths: &'a Paths,
    pub plugins: &'a [Box<dyn Plugin>],
    /// Where the current position comes from when the config asks for it.
    pub location: &'a dyn LocationSource,
    /// The executable the systemd service runs.
    pub exe: &'a Path,
    pub now: DateTime<FixedOffset>,
}

/// The real environment: this user's home, the session bus, real programs.
pub fn system_env(paths: &Paths) -> Env {
    Env {
        home: paths.home.clone(),
        kde: running_under_kde(),
        exec: Arc::new(RealExec),
        bus: Arc::new(RealBus::session()),
    }
}

fn running_under_kde() -> bool {
    let var = |name: &str| std::env::var(name).unwrap_or_default().to_lowercase();
    var("XDG_CURRENT_DESKTOP").split(':').any(|d| d == "kde")
        || ["DESKTOP_SESSION", "GDMSESSION"].iter().any(|name| matches!(var(name).as_str(), "plasma" | "kde" | "plasma5"))
}

// ---- shared pieces ------------------------------------------------------------------------

/// Serialises everything that writes: a timer run and the GUI must not interleave.
fn lock(paths: &Paths) -> Result<File> {
    fs::create_dir_all(&paths.state_dir)?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(paths.state_dir.join("lock"))
        .context("opening the lock file")?;
    file.lock().context("waiting for the lock")?;
    Ok(file)
}

fn load_config(ctx: &Ctx) -> Result<Config> {
    let mut config = Config::load(ctx.paths)?;
    let defaults: Vec<_> = ctx
        .plugins
        .iter()
        .map(|p| {
            let (light, dark) = p.defaults();
            (p.key().to_string(), light.map(String::from), dark.map(String::from))
        })
        .collect();
    config.fill_defaults(&defaults);
    Ok(config)
}

fn times_for(ctx: &Ctx, config: &Config, source: &dyn LocationSource) -> Result<(NaiveTime, NaiveTime)> {
    effective_times(config, source, ctx.paths, ctx.now.date_naive(), *ctx.now.offset())
}

/// Brings the timer in line with the config. A problem is reported, not fatal: it must not stop
/// the theme from switching. Returns whether all went well.
fn renew_timer(ctx: &Ctx, config: &Config, times: Option<(NaiveTime, NaiveTime)>, err: &mut dyn Write) -> bool {
    let installed = ctx.paths.systemd_dir.join("yinyang.timer").is_file();
    if times.is_none() && config.mode != Mode::Manual && !installed {
        log::warn!("the switch times are unknown, so no timer is installed yet");
        return true;
    }
    match systemd::refresh(config, ctx.paths, ctx.env.exec.as_ref(), ctx.exe, times) {
        Ok(()) => true,
        Err(e) => {
            let _ = writeln!(err, "timer: {e:#}");
            false
        }
    }
}

fn notify_failure(ctx: &Ctx, text: &str) {
    let _ = ctx.env.bus.notify("Yin-Yang", text);
}

struct Report {
    /// A plugin returned an error.
    failed: bool,
    /// Plugins were skipped because this is not a KDE session (yet): the attempt must be repeated.
    retry: bool,
    /// For the notification of an automatic run.
    summary: String,
}

/// Applies `variant` with the enabled plugins and prints one line per plugin.
fn run_plugins(ctx: &Ctx, config: &Config, variant: Variant, quiet: bool, err: &mut dyn Write) -> Report {
    let results = runner::apply_all(ctx.env, config, ctx.plugins, variant, quiet);
    let (mut failures, mut skipped) = (Vec::new(), 0);
    for (key, outcome) in &results {
        match outcome {
            Outcome::Applied => {
                let _ = writeln!(err, "{key}: applied");
            }
            Outcome::Skipped(reason) => {
                skipped += 1;
                let _ = writeln!(err, "{key}: skipped ({reason})");
            }
            Outcome::Failed(e) => {
                failures.push(format!("{key}: {e:#}"));
                let _ = writeln!(err, "{key}: FAILED: {e:#}");
            }
        }
    }

    let retry = !ctx.env.kde && skipped > 0;
    if retry {
        let _ = writeln!(err, "not running in a KDE session; the theme will be applied on the next run");
    }
    let summary = match failures.as_slice() {
        [] if retry => "Not running in a KDE session yet; will try again".to_string(),
        [] => String::new(),
        [only] => only.clone(),
        [first, rest @ ..] => format!("{first} (+{} more)", rest.len()),
    };
    Report { failed: !failures.is_empty(), retry, summary }
}

fn variant_name(variant: Variant) -> &'static str {
    match variant {
        Variant::Light => "light",
        Variant::Dark => "dark",
    }
}

// ---- commands -----------------------------------------------------------------------------

pub fn apply(ctx: &Ctx, args: &ApplyArgs, out: &mut dyn Write, err: &mut dyn Write) -> Result<i32> {
    if args.dry_run {
        return dry_run(ctx, args, out, err);
    }

    let _lock = lock(ctx.paths)?;
    let config = load_config(ctx)?;
    let times = times_for(ctx, &config, ctx.location);
    let auto = args.choice() == Choice::Auto;

    // The sun moves a little every day, so the timer is renewed on every run, theme change or not.
    let timer_ok = renew_timer(ctx, &config, times.as_ref().ok().copied(), err);

    let variant = match (args.choice(), &times) {
        (Choice::Dark, _) => Variant::Dark,
        (Choice::Light, _) => Variant::Light,
        (Choice::Auto, Ok((light, dark))) => {
            if should_be_dark(ctx.now.time(), *light, *dark) { Variant::Dark } else { Variant::Light }
        }
        (Choice::Auto, Err(e)) => {
            let _ = writeln!(err, "cannot tell which theme is due: {e:#}");
            notify_failure(ctx, &format!("Cannot tell which theme is due: {e:#}"));
            return Ok(1);
        }
    };

    let wanted_dark = variant == Variant::Dark;
    if !args.force && wanted_dark == config.dark_mode {
        return Ok(if timer_ok { 0 } else { 1 });
    }

    let report = run_plugins(ctx, &config, variant, args.force, err);
    if !report.failed && !report.retry {
        let mut config = config;
        config.dark_mode = wanted_dark;
        config.save(ctx.paths)?;
    }
    if auto && !report.summary.is_empty() {
        notify_failure(ctx, &report.summary);
    }
    Ok(if report.failed || report.retry || !timer_ok { 1 } else { 0 })
}

fn dry_run(ctx: &Ctx, args: &ApplyArgs, out: &mut dyn Write, err: &mut dyn Write) -> Result<i32> {
    let config = load_config(ctx)?;
    // Only what is already known: asking for the position would write it to disk.
    let times = times_for(ctx, &config, &NoNetworkLocation);
    let variant = match (args.choice(), times) {
        (Choice::Dark, _) => Variant::Dark,
        (Choice::Light, _) => Variant::Light,
        (Choice::Auto, Ok((light, dark))) => {
            if should_be_dark(ctx.now.time(), light, dark) { Variant::Dark } else { Variant::Light }
        }
        (Choice::Auto, Err(e)) => {
            writeln!(err, "cannot tell which theme is due: {e:#}")?;
            return Ok(1);
        }
    };

    writeln!(out, "variant: {}", variant_name(variant))?;
    if !args.force && (variant == Variant::Dark) == config.dark_mode {
        writeln!(out, "nothing to do: it is already {}", variant_name(variant))?;
        return Ok(0);
    }
    for line in runner::plan(&config, ctx.plugins, variant, args.force) {
        writeln!(out, "{line}")?;
    }
    Ok(0)
}

pub fn toggle(ctx: &Ctx, _out: &mut dyn Write, err: &mut dyn Write) -> Result<i32> {
    let _lock = lock(ctx.paths)?;
    let mut config = load_config(ctx)?;
    config.mode = Mode::Manual;
    let wanted_dark = !config.dark_mode;
    let variant = if wanted_dark { Variant::Dark } else { Variant::Light };

    let report = run_plugins(ctx, &config, variant, false, err);
    if !report.failed && !report.retry {
        config.dark_mode = wanted_dark;
    }
    config.save(ctx.paths)?;
    let timer_ok = renew_timer(ctx, &config, None, err);
    Ok(if report.failed || report.retry || !timer_ok { 1 } else { 0 })
}

pub fn status(ctx: &Ctx, out: &mut dyn Write) -> Result<i32> {
    let config = load_config(ctx)?;
    let hhmm = |time: NaiveTime| time.format("%H:%M").to_string();
    let times = times_for(ctx, &config, &NoNetworkLocation).ok().map(|(l, d)| json!([hhmm(l), hhmm(d)]));
    let position = location::resolve(&config, ctx.paths, &NoNetworkLocation).ok().map(|(lat, lon)| json!([lat, lon]));
    let status = json!({
        "dark_mode": config.dark_mode,
        "mode": config.mode,
        "times": times.unwrap_or(Value::Null),
        "location": position.unwrap_or(Value::Null),
    });
    writeln!(out, "{status}")?;
    Ok(0)
}

pub fn plugins_json(ctx: &Ctx, out: &mut dyn Write) -> Result<i32> {
    let list: Vec<Value> = ctx
        .plugins
        .iter()
        .map(|plugin| {
            let available = plugin.available(ctx.env);
            let themes = if available { plugin.themes(ctx.env) } else { Vec::new() };
            json!({
                "key": plugin.key(),
                "name": plugin.name(),
                "kind": plugin.kind(),
                "available": available,
                "themes": themes,
            })
        })
        .collect();
    writeln!(out, "{}", serde_json::to_string_pretty(&list)?)?;
    Ok(0)
}

pub fn config_show(ctx: &Ctx, out: &mut dyn Write) -> Result<i32> {
    let config = load_config(ctx)?;
    writeln!(out, "{}", serde_json::to_string_pretty(&config)?)?;
    Ok(0)
}

/// Replaces the settings with the JSON read from `input`. The theme state (`dark_mode`) is the
/// timer's to change, not a window that has been open for hours, so the one in the file stays.
pub fn config_save(ctx: &Ctx, input: &mut dyn Read, err: &mut dyn Write) -> Result<i32> {
    let mut text = String::new();
    input.read_to_string(&mut text).context("reading the configuration from the input")?;
    let mut incoming: Config = match serde_json::from_str(&text) {
        Ok(config) => config,
        Err(e) => {
            writeln!(err, "invalid config: {e}")?;
            return Ok(2);
        }
    };

    let _lock = lock(ctx.paths)?;
    incoming.dark_mode = Config::load(ctx.paths)?.dark_mode;
    incoming.version = Config::default().version;
    incoming.save(ctx.paths)?;

    let times = times_for(ctx, &incoming, ctx.location).ok();
    Ok(if renew_timer(ctx, &incoming, times, err) { 0 } else { 1 })
}

pub fn timer_refresh(ctx: &Ctx, err: &mut dyn Write) -> Result<i32> {
    let _lock = lock(ctx.paths)?;
    let config = load_config(ctx)?;
    let times = times_for(ctx, &config, ctx.location).ok();
    Ok(if renew_timer(ctx, &config, times, err) { 0 } else { 1 })
}
