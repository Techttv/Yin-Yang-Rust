//! The systemd user timer that switches the theme at the right times.

use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};
use chrono::NaiveTime;

use crate::config::{Config, Mode};
use crate::exec::Exec;
use crate::paths::Paths;

const TIMER_UNIT: &str = "yinyang.timer";
const SERVICE_UNIT: &str = "yinyang.service";
const DESCRIPTION: &str = "Automatic light and dark mode (Yin-Yang)";

/// The path as one `ExecStart=` word: quoted when systemd would split it, with `%` and `$`
/// doubled because systemd expands them.
fn exec_start_word(exe: &Path) -> String {
    let path = exe.to_string_lossy();
    let escaped = path.replace('%', "%%").replace('$', "$$");
    let needs_quotes = escaped != path || path.chars().any(|c| c.is_whitespace() || matches!(c, '"' | '\'' | '\\'));
    if needs_quotes {
        format!("\"{}\"", escaped.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        escaped
    }
}

pub fn render_service(exe: &Path) -> String {
    format!(
        "[Unit]\nDescription={DESCRIPTION}\nAfter=graphical-session.target\n\n[Service]\nType=oneshot\nExecStart={} apply --auto\n",
        exec_start_word(exe)
    )
}

/// Fires at both switch times, shortly after the user manager starts, and, `Persistent`, right
/// away if a switch time passed while the machine was off.
pub fn render_timer(light: NaiveTime, dark: NaiveTime, boot_offset: u32) -> String {
    let at = |time: NaiveTime| time.format("%H:%M:%S");
    format!(
        "[Unit]\nDescription={DESCRIPTION}\n\n[Timer]\nOnCalendar={}\nOnCalendar={}\nOnStartupSec={boot_offset}\nPersistent=true\n\n[Install]\nWantedBy=timers.target\n",
        at(light),
        at(dark)
    )
}

fn systemctl(exec: &dyn Exec, args: &[&str]) -> Result<()> {
    let mut full = vec!["--user"];
    full.extend_from_slice(args);
    exec.run("systemctl", &full)?
        .check(&format!("systemctl {}", full.join(" ")))?;
    Ok(())
}

/// Writes the file only when its content differs; returns whether it did.
fn write_if_changed(path: &Path, content: &str) -> Result<bool> {
    if fs::read_to_string(path).is_ok_and(|current| current == content) {
        return Ok(false);
    }
    fs::write(path, content).with_context(|| format!("writing {}", path.display()))?;
    Ok(true)
}

/// Brings the timer in line with the config. In manual mode it is stopped. Otherwise the units are
/// rewritten if `times` (the local times the light and the dark theme start) or the config changed,
/// systemd reloads them if so, and the timer is started; that last step always runs so a timer that
/// was stopped or never enabled comes back. With `times` unknown, existing units are left alone.
pub fn refresh(
    cfg: &Config,
    paths: &Paths,
    exec: &dyn Exec,
    exe: &Path,
    times: Option<(NaiveTime, NaiveTime)>,
) -> Result<()> {
    let timer_path = paths.systemd_dir.join(TIMER_UNIT);

    if cfg.mode == Mode::Manual {
        if timer_path.is_file() {
            systemctl(exec, &["disable", "--now", TIMER_UNIT])?;
        }
        return Ok(());
    }

    let mut changed = false;
    match times {
        Some((light, dark)) => {
            fs::create_dir_all(&paths.systemd_dir)?;
            changed |= write_if_changed(&paths.systemd_dir.join(SERVICE_UNIT), &render_service(exe))?;
            changed |= write_if_changed(&timer_path, &render_timer(light, dark, cfg.boot_offset))?;
        }
        None if !timer_path.is_file() => {
            bail!("cannot create the timer: the switch times are unknown");
        }
        None => {}
    }

    if changed {
        systemctl(exec, &["daemon-reload"])?;
    }
    systemctl(exec, &["enable", "--now", TIMER_UNIT])
}
