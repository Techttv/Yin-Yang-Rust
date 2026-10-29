use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use anyhow::{Context, Result};
use zbus::blocking::Connection;
use zbus::zvariant::Value;

/// The few session-bus operations the plugins need; a trait so tests can record them.
pub trait Bus: Send + Sync {
    /// Calls a method taking one string argument and returning nothing we care about.
    fn call_str(&self, dest: &str, path: &str, iface: &str, method: &str, arg: &str) -> Result<()>;

    /// Calls a method taking one boolean argument.
    fn call_bool(&self, dest: &str, path: &str, iface: &str, method: &str, arg: bool) -> Result<()>;

    /// The introspection XML of an object.
    fn introspect(&self, dest: &str, path: &str) -> Result<String>;

    /// Names of the services currently on the bus.
    fn service_names(&self) -> Result<Vec<String>>;

    /// Shows a desktop notification.
    fn notify(&self, summary: &str, body: &str) -> Result<()>;
}

/// The session bus, connected on first use: without a session every call fails, nothing else does.
pub struct RealBus {
    connection: Mutex<Option<Connection>>,
    timeout: Duration,
}

/// zbus waits for a reply forever by default; the Qt D-Bus API the original used gave up after 25 s.
const METHOD_TIMEOUT: Duration = Duration::from_secs(25);

impl RealBus {
    pub fn session() -> RealBus {
        RealBus::with_timeout(METHOD_TIMEOUT)
    }

    pub fn with_timeout(timeout: Duration) -> RealBus {
        RealBus { connection: Mutex::new(None), timeout }
    }

    fn connection(&self) -> Result<Connection> {
        let mut slot = self.connection.lock().unwrap();
        if let Some(connection) = &*slot {
            return Ok(connection.clone());
        }
        let connection = zbus::blocking::connection::Builder::session()
            .and_then(|builder| builder.method_timeout(self.timeout).build())
            .context("connecting to the session bus")?;
        *slot = Some(connection.clone());
        Ok(connection)
    }
}

impl Bus for RealBus {
    fn call_str(&self, dest: &str, path: &str, iface: &str, method: &str, arg: &str) -> Result<()> {
        self.connection()?
            .call_method(Some(dest), path, Some(iface), method, &(arg,))
            .with_context(|| format!("{dest} {method}"))?;
        Ok(())
    }

    fn call_bool(&self, dest: &str, path: &str, iface: &str, method: &str, arg: bool) -> Result<()> {
        self.connection()?
            .call_method(Some(dest), path, Some(iface), method, &(arg,))
            .with_context(|| format!("{dest} {method}"))?;
        Ok(())
    }

    fn introspect(&self, dest: &str, path: &str) -> Result<String> {
        let reply = self
            .connection()?
            .call_method(
                Some(dest),
                path,
                Some("org.freedesktop.DBus.Introspectable"),
                "Introspect",
                &(),
            )
            .with_context(|| format!("introspecting {dest} {path}"))?;
        Ok(reply.body().deserialize::<String>()?)
    }

    fn service_names(&self) -> Result<Vec<String>> {
        let reply = self
            .connection()?
            .call_method(
                Some("org.freedesktop.DBus"),
                "/org/freedesktop/DBus",
                Some("org.freedesktop.DBus"),
                "ListNames",
                &(),
            )
            .context("listing bus names")?;
        Ok(reply.body().deserialize::<Vec<String>>()?)
    }

    fn notify(&self, summary: &str, body: &str) -> Result<()> {
        let no_actions: Vec<&str> = Vec::new();
        let no_hints: HashMap<&str, Value> = HashMap::new();
        self.connection()?
            .call_method(
                Some("org.freedesktop.Notifications"),
                "/org/freedesktop/Notifications",
                Some("org.freedesktop.Notifications"),
                "Notify",
                &("yinyang", 0u32, "", summary, body, no_actions, no_hints, 5000i32),
            )
            .context("sending a notification")?;
        Ok(())
    }
}
