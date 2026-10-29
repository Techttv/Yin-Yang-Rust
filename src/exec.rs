use std::env;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecOutput {
    /// Exit code, or -1 when the process was killed by a signal.
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

impl ExecOutput {
    /// Turns a non-zero exit status into an error naming `what` and, when there is one, the
    /// program's own explanation from stderr.
    pub fn check(self, what: &str) -> Result<ExecOutput> {
        if self.status == 0 {
            return Ok(self);
        }
        let reason: String = self.stderr.trim().chars().take(500).collect();
        if reason.is_empty() {
            bail!("{what} exited with status {}", self.status);
        }
        bail!("{what} exited with status {}: {reason}", self.status)
    }
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

/// How long a program may take before it is killed: a hung desktop service or a command waiting
/// for input must not hold the lock (and with it every later theme switch) forever.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

pub struct RealExec {
    timeout: Duration,
}

impl RealExec {
    pub fn new() -> RealExec {
        RealExec::with_timeout(DEFAULT_TIMEOUT)
    }

    pub fn with_timeout(timeout: Duration) -> RealExec {
        RealExec { timeout }
    }

    /// Waits for `child`; on timeout kills it and says so.
    fn wait(&self, child: &mut Child, program: &str) -> Result<std::process::ExitStatus> {
        let deadline = Instant::now() + self.timeout;
        loop {
            if let Some(status) = child.try_wait().with_context(|| format!("waiting for {program}"))? {
                return Ok(status);
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                bail!("{program} did not finish in {} s and was killed", self.timeout.as_secs());
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
}

impl Default for RealExec {
    fn default() -> Self {
        RealExec::new()
    }
}

fn drain<R: Read + Send + 'static>(mut pipe: R) -> thread::JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = pipe.read_to_end(&mut bytes);
        bytes
    })
}

impl Exec for RealExec {
    fn run(&self, program: &str, args: &[&str]) -> Result<ExecOutput> {
        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("starting {program}"))?;
        // Read on other threads so a full pipe cannot stall the program. After a timeout they are
        // left to end by themselves: a grandchild may still hold the pipes open.
        let stdout = drain(child.stdout.take().expect("piped"));
        let stderr = drain(child.stderr.take().expect("piped"));
        let status = self.wait(&mut child, program)?;
        let text = |reader: thread::JoinHandle<Vec<u8>>| String::from_utf8_lossy(&reader.join().unwrap_or_default()).into_owned();
        Ok(ExecOutput { status: status.code().unwrap_or(-1), stdout: text(stdout), stderr: text(stderr) })
    }

    fn run_inherited(&self, program: &str, args: &[&str]) -> Result<i32> {
        let mut child = Command::new(program)
            .args(args)
            .spawn()
            .with_context(|| format!("starting {program}"))?;
        Ok(self.wait(&mut child, program)?.code().unwrap_or(-1))
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
