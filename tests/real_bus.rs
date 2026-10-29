// Alone in its own test binary because it changes the process environment.
use yinyang::bus::{Bus, RealBus};

#[test]
fn real_bus_without_session_errors_not_panics() {
    // SAFETY: this is the only test in this binary, so no other thread reads the environment.
    unsafe { std::env::set_var("DBUS_SESSION_BUS_ADDRESS", "unix:path=/nonexistent/yinyang-test-bus") };

    let bus = RealBus::session();
    assert!(bus.service_names().is_err());
    assert!(bus.call_str("a.b", "/a", "a.b", "M", "x").is_err());
    assert!(bus.call_bool("a.b", "/a", "a.b", "M", true).is_err());
    assert!(bus.introspect("a.b", "/a").is_err());
    assert!(bus.notify("s", "b").is_err());
}
