use std::env;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecOutput {
    /// Exit code, or -1 when the process was killed by a signal.
    pub status: i32,
    pub stdout: String,
}

/// Runs external programs; a trait so plugin tests can record the calls instead.
pub trait Exec: Send + Sync {
    /// Waits for the program to finish. A non-zero exit status is not an error; failing to start is.
    fn run(&self, program: &str, args: &[&str]) -> Result<ExecOutput>;

    /// Waits for the program, leaving its output attached to ours (the journal, under systemd), and
    /// returns its exit code. Unlike `run`, a background process the program leaves behind cannot
    /// block the caller by holding a captured pipe open.
    fn run_inherited(&self, program: &str, args: &[&str]) -> Result<i32>;

    /// Whether `program` can be started: an executable file, found in `PATH` unless it has a `/`.
    fn exists(&self, program: &str) -> bool;
}

pub struct RealExec;

impl Exec for RealExec {
    fn run(&self, program: &str, args: &[&str]) -> Result<ExecOutput> {
        let output = Command::new(program)
            .args(args)
            .output()
            .with_context(|| format!("starting {program}"))?;
        Ok(ExecOutput {
            status: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        })
    }

    fn run_inherited(&self, program: &str, args: &[&str]) -> Result<i32> {
        let status = Command::new(program)
            .args(args)
            .status()
            .with_context(|| format!("starting {program}"))?;
        Ok(status.code().unwrap_or(-1))
    }

    fn exists(&self, program: &str) -> bool {
        if program.contains('/') {
            return is_executable(Path::new(program));
        }
        env::var_os("PATH")
            .map(|path| env::split_paths(&path).any(|dir| is_executable(&dir.join(program))))
            .unwrap_or(false)
    }
}

fn is_executable(path: &Path) -> bool {
    path.metadata()
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}
