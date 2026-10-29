//! Test doubles for the process and D-Bus abstractions, shared by every plugin test.
#![allow(dead_code)]

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use anyhow::{anyhow, bail, Result};
use yinyang::bus::Bus;
use yinyang::exec::{Exec, ExecOutput};

#[derive(Default)]
pub struct FakeExec {
    calls: Mutex<Vec<String>>,
    outputs: Mutex<HashMap<String, ExecOutput>>,
    missing: Mutex<HashSet<String>>,
    failing: Mutex<HashMap<String, String>>,
}

impl FakeExec {
    pub fn new() -> Self {
        Self::default()
    }

    /// `run(program, ..)` answers with this status and stdout (the default is status 0, no output).
    pub fn respond(&self, program: &str, status: i32, stdout: &str) {
        self.outputs.lock().unwrap().insert(
            program.into(),
            ExecOutput { status, stdout: stdout.into() },
        );
    }

    /// The program is not installed: `exists` is false and `run` fails.
    pub fn missing(&self, program: &str) {
        self.missing.lock().unwrap().insert(program.into());
    }

    /// `run(program, ..)` fails with this message.
    pub fn fail(&self, program: &str, message: &str) {
        self.failing.lock().unwrap().insert(program.into(), message.into());
    }

    /// Every `run`, as `"program arg1 arg2"`, in order.
    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

impl Exec for FakeExec {
    fn run(&self, program: &str, args: &[&str]) -> Result<ExecOutput> {
        let mut line = program.to_string();
        for arg in args {
            line.push(' ');
            line.push_str(arg);
        }
        self.calls.lock().unwrap().push(line);

        if self.missing.lock().unwrap().contains(program) {
            bail!("{program}: command not found");
        }
        if let Some(message) = self.failing.lock().unwrap().get(program) {
            return Err(anyhow!(message.clone()));
        }
        Ok(self
            .outputs
            .lock()
            .unwrap()
            .get(program)
            .cloned()
            .unwrap_or(ExecOutput { status: 0, stdout: String::new() }))
    }

    fn run_inherited(&self, program: &str, args: &[&str]) -> Result<i32> {
        Ok(self.run(program, args)?.status)
    }

    fn exists(&self, program: &str) -> bool {
        !self.missing.lock().unwrap().contains(program)
    }
}

#[derive(Default)]
pub struct FakeBus {
    calls: Mutex<Vec<String>>,
    services: Mutex<Vec<String>>,
    introspections: Mutex<HashMap<(String, String), String>>,
    failure: Mutex<Option<(String, String)>>,
}

impl FakeBus {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_services(self, names: &[&str]) -> Self {
        *self.services.lock().unwrap() = names.iter().map(|n| n.to_string()).collect();
        self
    }

    /// `introspect(dest, path)` answers with this XML (the default is an empty `<node>`).
    pub fn set_introspection(&self, dest: &str, path: &str, xml: &str) {
        self.introspections
            .lock()
            .unwrap()
            .insert((dest.into(), path.into()), xml.into());
    }

    /// Every method fails with this message from now on.
    pub fn fail_all(&self, message: &str) {
        self.fail_matching("", message);
    }

    /// Calls whose recorded line starts with `prefix` (e.g. `"call_str"`) fail with this message.
    pub fn fail_matching(&self, prefix: &str, message: &str) {
        *self.failure.lock().unwrap() = Some((prefix.into(), message.into()));
    }

    /// Calls in order: `"call_str dest path iface method arg"`, `"call_bool …"`,
    /// `"introspect dest path"`, `"notify summary | body"`.
    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }

    fn record(&self, line: String) -> Result<()> {
        let failure = self.failure.lock().unwrap().clone();
        self.calls.lock().unwrap().push(line.clone());
        match failure {
            Some((prefix, message)) if line.starts_with(&prefix) => Err(anyhow!(message)),
            _ => Ok(()),
        }
    }
}

impl Bus for FakeBus {
    fn call_str(&self, dest: &str, path: &str, iface: &str, method: &str, arg: &str) -> Result<()> {
        self.record(format!("call_str {dest} {path} {iface} {method} {arg}"))
    }

    fn call_bool(&self, dest: &str, path: &str, iface: &str, method: &str, arg: bool) -> Result<()> {
        self.record(format!("call_bool {dest} {path} {iface} {method} {arg}"))
    }

    fn introspect(&self, dest: &str, path: &str) -> Result<String> {
        self.record(format!("introspect {dest} {path}"))?;
        Ok(self
            .introspections
            .lock()
            .unwrap()
            .get(&(dest.to_string(), path.to_string()))
            .cloned()
            .unwrap_or_else(|| "<node></node>".into()))
    }

    fn service_names(&self) -> Result<Vec<String>> {
        self.record("service_names".into())?;
        Ok(self.services.lock().unwrap().clone())
    }

    fn notify(&self, summary: &str, body: &str) -> Result<()> {
        self.record(format!("notify {summary} | {body}"))
    }
}
