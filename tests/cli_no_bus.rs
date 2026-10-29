// Alone in its own test binary because it changes the process environment.
use std::path::Path;
use std::sync::Arc;

use chrono::{FixedOffset, TimeZone};
use tempfile::TempDir;
use yinyang::app::{self, Ctx};
use yinyang::bus::RealBus;
use yinyang::exec::RealExec;
use yinyang::location::NoNetworkLocation;
use yinyang::paths::Paths;
use yinyang::plugin::{registry, Env};

#[test]
fn listing_plugins_and_showing_the_config_work_without_a_session_bus() {
    // SAFETY: the only test in this binary, so no other thread reads the environment.
    unsafe { std::env::set_var("DBUS_SESSION_BUS_ADDRESS", "unix:path=/nonexistent/yinyang-test-bus") };

    let home = TempDir::new().unwrap();
    let paths = Paths::under(home.path());
    let env = Env { home: home.path().into(), kde: true, exec: Arc::new(RealExec), bus: Arc::new(RealBus::session()) };
    let plugins = registry();
    let ctx = Ctx {
        env: &env,
        paths: &paths,
        plugins: &plugins,
        location: &NoNetworkLocation,
        exe: Path::new("/usr/bin/yinyang"),
        now: FixedOffset::east_opt(0).unwrap().with_ymd_and_hms(2026, 6, 21, 12, 0, 0).unwrap(),
    };

    let mut out = Vec::new();
    assert_eq!(app::plugins_json(&ctx, &mut out).unwrap(), 0);
    assert_eq!(serde_json::from_slice::<serde_json::Value>(&out).unwrap().as_array().unwrap().len(), 12);

    let mut out = Vec::new();
    assert_eq!(app::config_show(&ctx, false, &mut out).unwrap(), 0);
    assert!(!out.is_empty());
}
